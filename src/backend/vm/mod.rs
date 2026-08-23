// 2026-07-25: VM backend — emits .lair bytecode from typed AST.
// This backend implements BackendKind::Vm, compiling Briev programs
// to portable stack-based bytecode for the tamer VM interpreter.

pub mod assembler;
pub mod emit_expr;
pub mod emit_stmt;
pub mod emit_toplevel;
pub mod normalizer;
pub mod tests;

pub use assembler::Assembler;

use crate::ast::*;
use crate::ast::top::*;
use std::collections::{HashMap, HashSet};

/// 2026-08-23 (Plan 0.2): the tamer VM's declared surface — what emit_expr/
/// emit_stmt genuinely lower today. Integer stack machine: no floats, no
/// strings (OP_PUSH_STR has no emitter yet), no collections/foreach, no
/// concurrency sections. Anything outside this surface is a compile error
/// via backend::capabilities::validate_program, never a silent runtime trap.
/// To undo: remove this const + the validate_program call in src/compile.rs.
pub const CAPABILITIES: crate::backend::capabilities::BackendCapabilities =
    crate::backend::capabilities::BackendCapabilities {
        name: "tamer VM (.lair)",
        nature: "the tamer VM is an integer stack machine that finishes \
                 compilation on the install machine",
        int_literals: true,
        bool_char_literals: true,
        int_ops: true,
        unary_ops: true,
        calls: true,
        intrinsics: true,
        if_expr: true,
        match_expr: true,
        block_expr: true,
        field_access: true,
        index: true,
        casts: true,
        deref_addr_of: true,
        let_stmt: true,
        assign_stmt: true,
        guarded_stmt: true,
        term_endprogram: true,
        match_stmt: true,
        ..crate::backend::capabilities::BackendCapabilities::NONE
    };

/// 2026-07-30: Compute byte size of a field type for VM struct layout.
/// The VM is untyped: only Int (8 bytes), Ptr (8 bytes), and fixed arrays.
fn vm_field_size(ty: &Type, struct_fields: &HashMap<String, Vec<(String, u64)>>) -> u64 {
    match ty {
        Type::Ptr(_) | Type::PtrConst(_) => 8,
        Type::Custom(name) => {
            // Sub-struct (e.g., Frame inside VMFrames)
            let sub = struct_fields.get(name);
            sub.map(|f| f.iter().map(|(_, s)| *s).sum::<u64>()).unwrap_or(8)
        }
        Type::Vector(inner, dims) => {
            let elem_size = vm_field_size(inner, struct_fields);
            let count: u64 = dims.iter().map(|d| match d {
                crate::ast::Dimension::Anonymous(n) => *n as u64,
                _ => 1,
            }).product();
            elem_size * count
        }
        _ => 8,  // Default: Int size
    }
}

/// VM backend: compiles typed AST to .lair bytecode.
///
/// Usage:
///   let mut vm = VmBackend::new();
///   let lair = vm.generate(items, universe);
///   // lair is a .lair bytecode buffer ready for the tamer interpreter.
pub struct VmBackend {
    pub(crate) asm: Assembler,
    pub(crate) local_slots: HashMap<String, u8>,
    /// 2026-07-30: Map local variable names to their struct type names.
    pub(crate) local_types: HashMap<String, String>,
    /// 2026-07-25: Track which local slots hold Ptr<Int> values.
    /// Used by BinaryOp(Add) to scale Int offsets by 8.
    pub(crate) ptr_slots: HashSet<u8>,
    pub(crate) next_local_slot: u8,
    pub(crate) host_fn_ids: HashMap<String, u32>,
    pub(crate) fn_indices: HashMap<String, u16>,
    pub(crate) label_counter: u32,
    pub(crate) fn_index_counter: u16,
    /// 2026-07-30: Struct field offsets for Field expression compilation.
    /// Populated from struct definitions during collect_declarations.
    pub(crate) struct_fields: HashMap<String, Vec<(String, u64)>>,
    /// 2026-08-23 (Plan 0.2): constructs that reached codegen outside
    /// CAPABILITIES and were emitted as traps. The pipeline turns non-empty
    /// into a hard compile error — a silent wrong-behavior trap at install
    /// time is never acceptable.
    pub errors: Vec<String>,
    /// 2026-08-23 (Plan 0.2): enclosing defn/txn name for diagnostics.
    pub(crate) current_fn: String,
}

impl VmBackend {
    pub fn new() -> Self {
        VmBackend {
            asm: Assembler::new(),
            local_slots: HashMap::new(),
            local_types: HashMap::new(),
            ptr_slots: HashSet::new(),
            next_local_slot: 0,
            host_fn_ids: HashMap::new(),
            fn_indices: HashMap::new(),
            label_counter: 0,
            fn_index_counter: 0,
            struct_fields: HashMap::new(),
            errors: Vec::new(),
            current_fn: String::new(),
        }
    }

    /// 2026-08-23 (Plan 0.2): record an unsupported construct that would
    /// otherwise become a silent bytecode trap. `what` names the construct,
    /// `ctx` the enclosing function/txn.
    pub(crate) fn record_unsupported(&mut self, what: &str, ctx: &str) {
        self.errors.push(format!(
            "error: {} does not support {} (in '{}')\n  why: {}.\n  fix: rewrite \
             without {}, or build for a target that supports it (e.g. the native \
             LLVM target).",
            CAPABILITIES.name, what, ctx, CAPABILITIES.nature, what
        ));
    }

    /// Generate .lair bytecode from a typed program.
    ///
    /// Takes the typed AST items and type universe, returns a complete
    /// .lair binary buffer suitable for the tamer VM interpreter.
    pub fn generate(&mut self, items: &[crate::ast::TopLevel], _universe: &crate::type_universe::TypeUniverse) -> Vec<u8> {
        // First pass: collect function names and host function IDs
        self.collect_declarations(items);

        // Second pass: emit bytecode for each function
        for item in items {
            self.emit_toplevel(item);
        }

        // Finalize and return the .lair binary
        self.asm.assemble()
    }

    /// First pass: collect function names and host function declarations.
    fn collect_declarations(&mut self, items: &[crate::ast::TopLevel]) {
        for item in items {
            // 2026-07-25: Unwrap export wrappers to register exported defns.
            let inner = match item {
                crate::ast::TopLevel::Export(e) => &e.inner,
                other => other,
            };
            match inner {
                crate::ast::TopLevel::Definition(d) => {
                    let idx = self.fn_index_counter;
                    self.fn_index_counter += 1;
                    self.fn_indices.insert(d.name.clone(), idx);
                }
                crate::ast::TopLevel::Transaction(t) => {
                    let idx = self.fn_index_counter;
                    self.fn_index_counter += 1;
                    self.fn_indices.insert(t.name.clone(), idx);
                }
                crate::ast::TopLevel::Constant(c) => {
                    let idx = self.fn_index_counter;
                    self.fn_index_counter += 1;
                    self.fn_indices.insert(c.name.clone(), idx);
                }
                crate::ast::TopLevel::ForeignBinding(fb) => {
                    // Register host function IDs for frgn declarations.
                    // 2026-08-23 (Plan 1 HCALL slice): key by the BRIEV name
                    // (what call sites use — the old code keyed by
                    // foreign_name, so plain-named frgn calls like
                    // `__print_int(x)` fell through to fn_indices and died
                    // as "unknown function"). Arity rides into the .lair
                    // host table so the tamer pops the right slot count.
                    // briev_name is Option — fall back to foreign_name.
                    let local_name = fb
                        .briev_name
                        .clone()
                        .unwrap_or_else(|| fb.foreign_name.clone());
                    if !self.host_fn_ids.contains_key(local_name.as_str()) {
                        let id = match canonical_host_id(&local_name) {
                            Some(cid) => cid,
                            None => {
                                HOST_ID_CANONICAL_MAX + self.host_fn_ids.len() as u32
                            }
                        };
                        let arity = fb.inputs.len() as u32;
                        self.asm.register_host_fn_with_arity(&local_name, id, arity);
                        self.host_fn_ids.insert(local_name, id);
                    }
                }
                crate::ast::TopLevel::Obj(sd) => {
                    // 2026-07-30: Compute field offsets for struct types.
                    let mut running = 0u64;
                    let mut field_offsets = Vec::new();
                    for slot in &sd.fields {
                        let field_size = vm_field_size(&slot.ty, &self.struct_fields);
                        field_offsets.push((slot.name.clone(), running));
                        running += field_size;
                    }
                    self.struct_fields.insert(sd.name.clone(), field_offsets);
                }
                _ => {}
            }
        }
    }

    /// Register a host function declaration (used by the bounty builder).
    pub fn register_host_fn(&mut self, name: &str, id: u32) {
        self.host_fn_ids.insert(name.to_string(), id);
        self.asm.register_host_fn(name, id);
    }

    /// 2026-07-30: Compute byte offset for a struct field access.
    pub(crate) fn field_offset(&self, struct_name: Option<&str>, field_name: &str) -> u64 {
        let sname = match struct_name {
            Some(s) => s,
            None => return self.field_offset_any(field_name),
        };
        let fields = match self.struct_fields.get(sname) {
            Some(f) => f,
            None => return self.field_offset_any(field_name),
        };
        for (name, offset) in fields {
            if name == field_name {
                return *offset;
            }
        }
        self.field_offset_any(field_name)
    }

    /// Fallback: find a field by name in any struct.
    fn field_offset_any(&self, field_name: &str) -> u64 {
        for (_, fields) in self.struct_fields.iter() {
            for (name, offset) in fields {
                if name == field_name {
                    return *offset;
                }
            }
        }
        0
    }
}

/// 2026-08-23 (Plan 1 HCALL slice): canonical host-service ids. STABLE
/// across programs — the tamer dispatches by number, so a service keeps its
/// id no matter what else a user program calls. Names still ride in the
/// .lair host table for diagnostics. Unknown services get ids ≥
/// HOST_ID_CANONICAL_MAX (first-use order among themselves); the tamer
/// rejects those at run time with a helpful message.
/// To undo: revert emit_expr to `id = host_fn_ids.len()` and delete this fn.
pub fn canonical_host_id(name: &str) -> Option<u32> {
    match name {
        "Print#" | "PrintInt#" => Some(HOST_ID_PRINT_INT),
        "Log#" => Some(HOST_ID_LOG_INT),
        _ => None,
    }
}

pub const HOST_ID_PRINT_INT: u32 = 0;
pub const HOST_ID_LOG_INT: u32 = 1;
/// Unknown services start here (first-use order among themselves).
pub const HOST_ID_CANONICAL_MAX: u32 = 1000;
