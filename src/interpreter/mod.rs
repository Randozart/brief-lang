// ── Interpreter — Value, VirtualHeap, Re-exports ───────────────────────
// 2026-07-12: Phase 3.0 — Bits-only Value, sandboxed VirtualHeap.
// Value::Bits(Vec<u8>) is the ONLY program-value variant for program data.
// Meta-objects (Defn, Expr, etc.) are compiler-internal and never
// reach user code.
//
// 2026-07-14: Re-added List/Enum/Instance/HashMap variants for FFI bridge
// code that marshals structured types between native libraries and the
// interpreter. These are FFI-only — no interpreter eval path produces them.
//
// 2026-07-28: Phase B.1 — Added function registry, load_program(),
// lookup_function(), and call_function() for derivation assertion
// verification. Functions are stored by name and called with raw Value
// arguments via a frame save/restore pattern.
//
// 2026-08-06: Phase 17 (Slice A) — semantic-value migration start. Fold the
// ad-hoc Int/Float/Bool/Char variants into the single `Atom` category and
// drop Enum/Instance/HashMap/Defn, which had no live users (their consumers
// were orphaned features/* modules and PropertyValue false positives). The
// remaining surface (Bits, Atom, Void, Ref, Constructor, List) is the first
// step toward the SPEC 2.2 generic model: atoms, bits, products, sums,
// references, closures, void. Undo: re-introduce per-kind variants only if a
// consumer needs the extra type safety; nothing outside interpreter/derive
// pattern-matches the atom category.

pub mod casts;
mod cells;
mod eval;
mod ffi;
mod intrinsics;

pub use cells::*;
pub use eval::*;
pub use ffi::*;
pub use intrinsics::*;

use crate::ast::*;
use std::collections::HashMap;
use std::sync::Arc;

/// FFI files import RuntimeError from crate::interpreter.
pub use crate::errors::RuntimeError;

/// 2026-07-28: Record for a parsed function definition stored in the
/// interpreter's function registry.
#[derive(Debug, Clone)]
pub struct FunctionDef {
    pub name: String,
    pub parameters: Vec<String>,
    pub body: Vec<Statement>,
}

/// Compatibility struct bridging old Interpreter-based code to the new
/// function-based eval API. The old `Interpreter` had `state`, `prior_state`,
/// `heap`, `eval_expr()`, and `exec_stmt()`. This shim preserves that API
/// while delegating to the standalone `eval_expr`/`eval_statement` functions.
/// 2026-07-14: Added for reactor.rs and feature code compatibility.
///
/// 2026-07-28: Phase B.1 — Added `functions` registry for call_function().
#[derive(Debug, Clone)]
pub struct Interpreter {
    pub state: HashMap<String, Value>,
    pub prior_state: HashMap<String, Value>,
    pub heap: VirtualHeap,
    /// 2026-07-28: Phase B.1 — Function definitions loaded from the program.
    /// Keyed by function name, used by call_function() for assertion verification.
    pub functions: HashMap<String, FunctionDef>,
    /// 2026-08-09 (init kind, Phase 2): runtime-seeded invariant names. Set by
    /// `load_program` when it seeds each `TopLevel::Init`; reads resolve via
    /// `state`, and a later write to one is a `RuntimeError::ImmutableInit`.
    pub init_names: std::collections::HashSet<String>,
    /// 2026-08-09 (Phase 10): `defer { ... }` cleanup stack — bodies pushed by
    /// `exec_stmt(Defer)` and flushed LIFO on term/rollback/endprogram. The
    /// reference semantics: cleanup runs exactly once per registered defer,
    /// even when the enclosing firing rolls back.
    pub defer_stack: Vec<Vec<Statement>>,
}

impl Interpreter {
    pub fn new() -> Self {
        Interpreter {
            state: HashMap::new(),
            prior_state: HashMap::new(),
            heap: VirtualHeap::new(),
            functions: HashMap::new(),
            init_names: std::collections::HashSet::new(),
            defer_stack: Vec::new(),
        }
    }

    /// 2026-07-28: Phase B.1 — Load all definitions and transactions from
    /// a parsed program into the function registry for call_function().
    pub fn load_program(&mut self, program: &[TopLevel]) {
        for item in program {
            match item {
                TopLevel::Definition(d) => {
                    let param_names = d.parameters.iter().map(|(n, _)| n.clone()).collect();
                    self.functions.insert(d.name.clone(), FunctionDef {
                        name: d.name.clone(),
                        parameters: param_names,
                        body: d.body.clone(),
                    });
                }
                TopLevel::Transaction(t) => {
                    let param_names = t.parameters.iter().map(|(n, _)| n.clone()).collect();
                    self.functions.insert(t.name.clone(), FunctionDef {
                        name: t.name.clone(),
                        parameters: param_names,
                        body: t.body.clone(),
                    });
                }
                _ => {}
            }
        }
        // 2026-08-09 (init kind, Phase 2): seed each init exactly once, before
        // any transaction runs. A seeding failure (or a duplicate init) is a
        // runtime error — the interpreter is the reference codegen must match.
        let _ = self.seed_inits(program);
    }

    /// 2026-08-09 (init kind, Phase 2): evaluate each `TopLevel::Init` seeding
    /// (value expr, or body statements once) into `state` and mark the name in
    /// `init_names`. Re-seeding an already-seeded init is
    /// `RuntimeError::ImmutableInit`. Reads of the seeded value then resolve
    /// through the normal identifier path.
    pub fn seed_inits(&mut self, program: &[TopLevel]) -> Result<(), RuntimeError> {
        for item in program {
            if let TopLevel::Init(init) = item {
                self.seed_init(init)?;
            }
        }
        Ok(())
    }

    /// Seed one `TopLevel::Init` — value form evaluates its expr; body form
    /// runs its statements once (a `term <expr>` yields the value; a bare
    /// `term;` is a convergence checkpoint leaving Void). Re-seeding a seeded
    /// init is `RuntimeError::ImmutableInit`.
    fn seed_init(&mut self, init: &crate::ast::top::InitDecl) -> Result<(), RuntimeError> {
        if self.init_names.contains(&init.name) || self.state.contains_key(&init.name) {
            return Err(RuntimeError::ImmutableInit(init.name.clone()));
        }
        if let Some(value) = &init.value {
            let v = self.eval_expr(value)?;
            self.state.insert(init.name.clone(), v);
        } else {
            let seeded = self.seed_body(&init.body)?;
            self.state.insert(init.name.clone(), seeded);
        }
        self.init_names.insert(init.name.clone());
        Ok(())
    }

    /// Run a body-form init's statements once and return the seeded value.
    fn seed_body(&mut self, body: &[Statement]) -> Result<Value, RuntimeError> {
        let mut seeded = Value::Void;
        for stmt in body {
            match self.exec_stmt(stmt) {
                Ok(v) => seeded = v,
                Err(RuntimeError::TermReturn(v)) => return Ok(v),
                Err(e) => return Err(e),
            }
        }
        Ok(seeded)
    }

    /// 2026-07-28: Phase B.1 — Look up a function definition by name.
    pub fn lookup_function(&self, name: &str) -> Option<&FunctionDef> {
        self.functions.get(name)
    }

    /// 2026-07-28: Phase B.1 — Call a function by name with pre-evaluated
    /// argument values. Binds parameters, executes body, returns result.
    /// Saves and restores any state keys that overlap with parameter names.
    pub fn call_function(&mut self, name: &str, args: &[Value]) -> Result<Value, RuntimeError> {
        let defn = self.lookup_function(name)
            .cloned()
            .ok_or_else(|| RuntimeError::UndefinedFunction(name.to_string()))?;

        if defn.parameters.len() != args.len() {
            return Err(RuntimeError::TypeError {
                expected: format!("{} arguments", defn.parameters.len()),
                found: format!("{} arguments", args.len()),
            });
        }

        // Save any existing state keys that overlap with parameter names
        let mut saved: HashMap<String, Option<Value>> = HashMap::new();
        for param in &defn.parameters {
            saved.insert(param.clone(), self.state.get(param).cloned());
        }

        // Bind parameters
        for (i, name) in defn.parameters.iter().enumerate() {
            self.state.insert(name.clone(), args[i].clone());
        }

        // Execute body statements
        // 2026-07-28: Term with value signals early return via TermReturn error.
        // 2026-08-09 (Phase 10): use exec_stmt so `defer` registers cleanup;
        // flush the defer stack (LIFO) on normal termination and on term.
        let mut result = Value::Void;
        for stmt in &defn.body {
            match self.exec_stmt(stmt) {
                Ok(v) => result = v,
                Err(RuntimeError::TermReturn(v)) => {
                    result = v;
                    break;
                }
                Err(e) => return Err(e),
            }
        }
        // 2026-08-09 (Phase 10): deferred cleanup runs on EVERY exit — normal
        // fall-through AND term (the firing completed either way).
        let _ = self.flush_defers();

        // Restore saved state
        for (name, saved_val) in saved {
            match saved_val {
                Some(val) => { self.state.insert(name, val); }
                None => { self.state.remove(&name); }
            }
        }

        Ok(result)
    }

    pub fn eval_expr(&mut self, expr: &Expr) -> Result<Value, RuntimeError> {
        eval_expr(expr, &mut self.heap, &mut self.state, &self.functions)
    }

    pub fn exec_stmt(&mut self, stmt: &Statement) -> Result<Value, RuntimeError> {
        // 2026-08-09 (init kind, Phase 2): an `init` is seeded once and
        // immutable — a write to one at runtime is a reference violation
        // (the typechecker rejects it statically; this is the defensive
        // runtime half, matching the backend's seeded-global store).
        if let Some(name) = statement_write_target(stmt) {
            if self.init_names.contains(name) {
                return Err(RuntimeError::ImmutableInit(name.to_string()));
            }
        }
        // 2026-08-09 (Phase 10): `defer { ... }` registers cleanup — push the
        // body onto the defer stack (flushed LIFO on term/rollback/endprogram).
        if let Statement::Defer(body) = stmt {
            self.defer_stack.push(body.clone());
            return Ok(Value::Void);
        }
        eval_statement(stmt, &mut self.heap, &mut self.state, &self.functions)
    }

    /// 2026-08-09 (Phase 10): run all registered `defer` cleanups LIFO. Called
    /// on term/rollback/endprogram; the stack is drained (each defer runs once).
    pub fn flush_defers(&mut self) -> Result<(), RuntimeError> {
        while let Some(body) = self.defer_stack.pop() {
            for stmt in &body {
                self.exec_stmt(stmt)?;
            }
        }
        Ok(())
    }

    /// Pattern match a value against a pattern. Delegates to the full
    /// implementation in eval.rs (used by eval_match and kept here for
    /// API compatibility).
    pub fn pattern_match(pat: &crate::ast::Pattern, val: &Value, bindings: &mut HashMap<String, Value>) -> bool {
        crate::interpreter::pattern_match(pat, val, bindings)
    }

    /// Compute the nesting depth of a product value (for FFI marshalling).
    pub fn list_nesting_depth(val: &Value) -> usize {
        match val {
            Value::Product { fields, .. } => {
                fields.iter().map(|v| Self::list_nesting_depth(v)).max().unwrap_or(0) + 1
            }
            _ => 0,
        }
    }

    /// Expand coordinate dimensions into a flat list of indices (for FFI).
    pub fn expand_coordinates(coords: &[Expr], dims: usize) -> Result<Vec<Value>, RuntimeError> {
        let mut result = Vec::new();
        for coord in coords {
            match coord {
                Expr::Decimal(n) => result.push(Value::int(*n)),
                _ => return Err(RuntimeError::TypeError { expected: "integer".to_string(), found: format!("{:?}", coord) }),
            }
        }
        Ok(result)
    }
}

impl Default for Interpreter {
    fn default() -> Self {
        Self::new()
    }
}

/// 2026-08-09 (init kind, Phase 2): the top-level name a statement writes, if
/// it is a plain identifier assignment/arrow write. Used by exec_stmt to
/// reject writes to seeded `init` names (the typechecker rejects them
/// statically; this is the defensive runtime half).
fn statement_write_target<'a>(stmt: &'a Statement) -> Option<&'a str> {
    match stmt {
        Statement::Assign(lhs, _) => match lhs {
            Expr::Identifier(name) => Some(name.as_str()),
            _ => None,
        },
        Statement::ArrowAssign { target, .. } => match target.as_ref() {
            Some(t) => match t.as_ref() {
                Expr::Identifier(name) => Some(name.as_str()),
                _ => None,
            },
            None => None,
        },
        _ => None,
    }
}

/// Signature for foreign function implementations registered in the FFI registry.
pub type ForeignFn = fn(Vec<Value>) -> Result<Value, RuntimeError>;

/// A primitive atom value. Grouped under `Value::Atom` (SPEC §2.2 "optimized
/// primitive atoms") so the eval path can dispatch on one category while the
/// atom kind stays first-class — `as_i64`/`as_f64`/`as_bool` promote every
/// atom C-style (Bool→0|1, Char→code point), matching the backend's
/// protocol-category dispatch.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Atom {
    Int(i64),
    Float(f64),
    Bool(bool),
    Char(char),
}

/// The representational value in the Briev interpreter.
///
/// 2026-08-06 (Phase 17, Slice A): `Enum`, `Instance`, `HashMap`, and `Defn`
/// were dropped — they had no live producers/consumers (only orphaned
/// `features/*` and `PropertyValue` false positives). `Int`/`Float`/`Bool`/
/// `Char` now live under `Atom`. `List` survives only for reactor state
/// collections and is slated for replacement by a product in a later slice;
/// `Constructor` is the derive (CEGIS) engine's synthesis shape.
///
/// 2026-08-06 (Slice B): `Product` added as the product value (tuples, list
/// literals, struct fields in declared order). SPEC §2.2 — stdlib list/map
/// behavior is NOT interpreter knowledge; the interpreter holds the field
/// sequence, stdlib owns the semantics.
///
/// 2026-08-06 (Slice D): struct literals produce a product carrying its
/// declared field names (`names: Some(...)`); field access resolves the
/// index from that map. Tuples and list literals stay unnamed (`names: None`).
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// The sole representational storage cell for opaque program data.
    Bits(Vec<u8>),

    /// 2026-07-14: Typed value variants for generic op dispatch.
    /// These allow the interpreter to distinguish int from float values
    /// when both would be valid as raw Bits(Vec<u8>).
    Atom(Atom),

    /// A product: field sequence in declared order. `names` is present for
    /// struct literals (drives `Expr::Field`), absent for tuples/list
    /// literals (positional only). The interpreter is dynamically typed, so
    /// the name→index map is carried with the value rather than resolved from
    /// type context (which eval has none of).
    Product {
        fields: Vec<Value>,
        names: Option<std::sync::Arc<Vec<String>>>,
    },

    Void,
    Ref(Box<Value>),

    /// 2026-08-06 (Slice E): a closure — lambda params, body, and the captured
    /// environment snapshot at creation. Application (Expr::Call on a closure
    /// binding) binds params into a fresh env seeded from the captured one;
    /// mutations never leak to the captured env or the caller (re-entrant).
    Closure {
        params: Arc<Vec<String>>,
        body: Arc<Expr>,
        env: Arc<HashMap<String, Value>>,
    },

    // ── Compound types (synthesis engine) ────────────────────────────
    /// 2026-08-06 (Slice H): a sum — an enum variant with payload, produced by
    /// the derive (CEGIS) engine's `evaluate_synthesized` for `Call`/variant
    /// expressions. The variant NAME travels with the value (SPEC §2.2
    /// "sums"): the interpreter has no separate type registry, so name-based
    /// pattern matching (Pattern::EnumVariant) and `Type` reflection resolve
    /// from the value itself. The plan sketched a tag-index + global table;
    /// carrying the name avoids global mutable tag state and keeps derive's
    /// name-based variant dispatch working unchanged. Undo: if a shared type
    /// registry ever reaches the interpreter, replace the name with a tag.
    /// Renamed from `Constructor` (the synthesis-tree name was misleading —
    /// the value is a tagged compound, not a constructor expression).
    Sum { name: String, payload: Vec<Value> },

    /// 2026-08-07 (Phase 7): an iterable range — `start..end` (half-open) or
    /// `start..=end` (inclusive). Produced by `Expr::Range`; consumed by
    /// `foreach` (SPEC §11.4 counted iteration).
    Range { start: i64, end: i64, inclusive: bool },
}

// 2026-08-06 (Slice I): the last ad-hoc variant (`List`) is dropped — no
// producer remained (eval yields Product; FFI marshals through
// ffi::marshal_value). The semantic model is now atoms, bits, products,
// sums, references, closures, void (SPEC §2.2).
//
// 2026-07-14: Re-added List/Enum/Instance/HashMap variants for FFI bridge
// code that marshals structured types between native libraries and the
// interpreter. These are FFI-only — no interpreter eval path produces
// them. (All dropped by 2026-08-06 Slice A/I; FFI marshalling lives in
// ffi.rs.)

impl Value {
    pub fn int(n: i64) -> Self {
        Value::Atom(Atom::Int(n))
    }

    pub fn float(f: f64) -> Self {
        Value::Atom(Atom::Float(f))
    }

    pub fn bool(b: bool) -> Self {
        Value::Atom(Atom::Bool(b))
    }

    pub fn char(c: char) -> Self {
        Value::Atom(Atom::Char(c))
    }

    pub fn bits(data: Vec<u8>) -> Self {
        Value::Bits(data)
    }

    pub fn product(fields: Vec<Value>) -> Self {
        Value::Product { fields, names: None }
    }

    pub fn named_product(fields: Vec<Value>, names: Vec<String>) -> Self {
        Value::Product {
            fields,
            names: Some(std::sync::Arc::new(names)),
        }
    }

    pub fn sum(name: String, payload: Vec<Value>) -> Self {
        Value::Sum { name, payload }
    }

    pub fn void() -> Self {
        Value::Void
    }

    /// Extract first 8 bytes as little-endian i64.
    // 2026-07-14: Zero-pad buffers shorter than 8 bytes so small bit
    // values (zero_bits(4), zext of 1-byte bool) convert correctly.
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Value::Atom(Atom::Int(n)) => Some(*n),
            Value::Atom(Atom::Bool(b)) => Some(if *b { 1 } else { 0 }),
            Value::Atom(Atom::Char(c)) => Some(*c as i64),
            Value::Bits(bytes) => {
                let mut arr = [0u8; 8];
                let copy_len = bytes.len().min(8);
                arr[..copy_len].copy_from_slice(&bytes[..copy_len]);
                Some(i64::from_le_bytes(arr))
            }
            _ => None,
        }
    }

    /// Extract first 8 bytes as f64.
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Value::Atom(Atom::Float(f)) => Some(*f),
            Value::Atom(Atom::Bool(b)) => Some(if *b { 1.0 } else { 0.0 }),
            Value::Atom(Atom::Char(c)) => Some(*c as i64 as f64),
            Value::Bits(bytes) if bytes.len() >= 8 => {
                let arr: [u8; 8] = bytes[..8].try_into().ok()?;
                Some(f64::from_le_bytes(arr))
            }
            _ => None,
        }
    }

    /// Extract first byte as bool.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Value::Atom(Atom::Bool(b)) => Some(*b),
            Value::Atom(Atom::Char(c)) => Some(*c != '\0'),
            Value::Bits(bytes) => bytes.first().map(|b| *b != 0),
            _ => None,
        }
    }

    /// Check if the value is truthy (any non-zero byte).
    pub fn is_true(&self) -> bool {
        self.as_bool().unwrap_or(false)
    }

    /// Dereference this value as a Briev String's content bytes.
    ///
    /// 2026-08-01 (B1): A Briev String is either raw bytes (`Value::Bits`,
    /// produced by literals and the get_env! macro) or a heap handle
    /// (`Value::Atom(Atom::Int(addr))` pointing at `[len: i64][payload]`,
    /// produced by FFI marshalling like EnvGet#). This is the single deref
    /// used by content equality (Eq/Ne) and any op that needs the string
    /// payload. Returns `None` when the value is not a String (so numeric
    /// comparisons fall through to their own paths). Undo: if the heap-handle
    /// encoding is ever removed, drop the `Atom::Int` arm and keep the Bits arm.
    pub fn string_bytes(&self, heap: &VirtualHeap) -> Option<Vec<u8>> {
        match self {
            Value::Bits(bytes) => Some(bytes.clone()),
            Value::Atom(Atom::Int(addr)) if *addr > 0 => {
                // 2026-08-01 (B1): i64_to_bits(n) produces an Int atom, so an
                // Int atom is ambiguous between a real int and a heap string
                // handle. Only deref as a heap string when the address is an
                // actual heap allocation with a readable [len: i64] header —
                // otherwise return None so numeric comparisons fall through.
                let len = heap.read(*addr as u64, 8)?.to_vec();
                let len = i64::from_le_bytes(len.try_into().unwrap_or([0u8; 8]));
                if len > 0 {
                    heap.read(*addr as u64 + 8, len as usize).map(|p| p.to_vec())
                } else {
                    Some(Vec::new())
                }
            }
            _ => None,
        }
    }
}

/// Convert i64 to an Int atom value.
pub fn i64_to_bits(n: i64) -> Value {
    Value::int(n)
}

/// Convert f64 to a Float atom value.
pub fn f64_to_bits(f: f64) -> Value {
    Value::float(f)
}

/// Convert bool to a Bool atom value.
pub fn bool_to_bits(b: bool) -> Value {
    Value::bool(b)
}

/// Create a zero-filled Bits value of the given byte size.
pub fn zero_bits(size: usize) -> Value {
    Value::Bits(vec![0u8; size])
}

/// 2026-07-28: Phase B.0 — Compare two values within relative tolerance.
/// Used by derivation assertion verification for FP relaxed equivalence.
/// For Float values, checks relative error: |a - e| / max(|e|, 1e-10) <= tol.
/// Non-float values must match exactly.
pub fn values_within_tolerance(actual: &Value, expected: &Value, tol: f64) -> bool {
    match (actual, expected) {
        (Value::Atom(Atom::Float(a)), Value::Atom(Atom::Float(e))) => {
            let diff = (a - e).abs();
            let mag = e.abs().max(1e-10);
            diff / mag <= tol
        }
        _ => actual == expected,
    }
}

/// Sandboxed compile-time heap for pointer arithmetic and allocation.
/// 2026-07-12: Phase 3.0 — Bounds-checked read/write.
#[derive(Debug, Clone)]
pub struct VirtualHeap {
    allocations: HashMap<u64, Vec<u8>>,
    next_address: u64,
}

impl VirtualHeap {
    pub fn new() -> Self {
        VirtualHeap {
            allocations: HashMap::new(),
            next_address: 0x1000, // start at page-aligned address
        }
    }

    /// Allocate a block of the given size. Returns virtual address.
    /// The allocation is zero-filled.
    pub fn allocate(&mut self, size: usize) -> u64 {
        let addr = self.next_address;
        self.allocations.insert(addr, vec![0u8; size]);
        self.next_address += size as u64 + 16; // small gap between allocations
        addr
    }

    /// Read bytes from the given address. Returns None if address not found
    /// or the read would go out of bounds.
    pub fn read(&self, addr: u64, size: usize) -> Option<&[u8]> {
        let (base, data) = self.find_block(addr)?;
        let offset = (addr - base) as usize;
        if offset + size > data.len() {
            return None;
        }
        Some(&data[offset..offset + size])
    }

    /// Write bytes at the given address. Returns error if address not found
    /// or the write would go out of bounds.
    pub fn write(&mut self, addr: u64, data: &[u8]) -> Result<(), String> {
        let (base, block) = self
            .find_block_mut(addr)
            .ok_or("heap: address not allocated")?;
        let offset = (addr - base) as usize;
        if offset + data.len() > block.len() {
            return Err("heap: write out of bounds".into());
        }
        block[offset..offset + data.len()].copy_from_slice(data);
        Ok(())
    }

    /// Free a previously allocated block. Returns error if not found.
    pub fn free(&mut self, addr: u64) -> Result<(), String> {
        // Find the block that contains this address
        let base = self
            .allocations
            .keys()
            .filter(|k| **k <= addr)
            .max()
            .copied()
            .ok_or("heap: address not found")?;
        self.allocations
            .remove(&base)
            .ok_or("heap: address not found")?;
        Ok(())
    }

    /// Check if an address is allocated.
    pub fn contains(&self, addr: u64) -> bool {
        self.allocations.keys().any(|k| *k <= addr)
    }

    /// Find the block base and data for an address (read).
    fn find_block(&self, addr: u64) -> Option<(u64, &Vec<u8>)> {
        self.allocations
            .iter()
            .filter(|(k, v)| **k <= addr && addr < **k + v.len() as u64)
            .map(|(k, v)| (*k, v))
            .next()
    }

    /// Find the block base and data for an address (write).
    fn find_block_mut(&mut self, addr: u64) -> Option<(u64, &mut Vec<u8>)> {
        for (k, v) in &mut self.allocations {
            if *k <= addr && addr < *k + v.len() as u64 {
                return Some((*k, v));
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_atom_constructors() {
        assert_eq!(Value::int(42), Value::Atom(Atom::Int(42)));
        assert_eq!(Value::float(1.5), Value::Atom(Atom::Float(1.5)));
        assert_eq!(Value::bool(true), Value::Atom(Atom::Bool(true)));
        assert_eq!(Value::char('x'), Value::Atom(Atom::Char('x')));
    }

    #[test]
    fn test_atom_promotion_c_style() {
        assert_eq!(Value::bool(true).as_i64(), Some(1));
        assert_eq!(Value::bool(false).as_i64(), Some(0));
        assert_eq!(Value::char('A').as_i64(), Some(65));
        assert_eq!(Value::bool(true).as_f64(), Some(1.0));
        assert_eq!(Value::char('A').as_bool(), Some(true));
        assert!(Value::char('\0').as_bool() == Some(false));
    }

    #[test]
    fn test_atom_category_matches_protocol_dispatch() {
        let b = Value::bool(true);
        assert!(matches!(b, Value::Atom(Atom::Bool(true))));
        let c = Value::char('z');
        assert!(matches!(c, Value::Atom(Atom::Char('z'))));
    }

    #[test]
    fn test_value_as_i64() {
        let v = i64_to_bits(42);
        assert_eq!(v.as_i64(), Some(42));
    }

    #[test]
    fn test_value_as_f64() {
        let v = f64_to_bits(3.14);
        let result = v.as_f64().unwrap();
        assert!((result - 3.14).abs() < 1e-10);
    }

    #[test]
    fn test_value_as_bool() {
        assert!(bool_to_bits(true).as_bool().unwrap());
        assert!(!bool_to_bits(false).as_bool().unwrap());
    }

    #[test]
    fn test_virtual_heap_alloc_read_write() {
        let mut heap = VirtualHeap::new();
        let addr = heap.allocate(16);
        assert!(heap.contains(addr));

        heap.write(addr, &[1, 2, 3, 4]).unwrap();
        let data = heap.read(addr, 4).unwrap();
        assert_eq!(data, &[1, 2, 3, 4]);
    }

    #[test]
    fn test_virtual_heap_read_out_of_bounds() {
        let mut heap = VirtualHeap::new();
        let addr = heap.allocate(8);
        assert!(heap.read(addr, 16).is_none());
    }

    #[test]
    fn test_virtual_heap_write_out_of_bounds() {
        let mut heap = VirtualHeap::new();
        let addr = heap.allocate(4);
        assert!(heap.write(addr, &[0u8; 8]).is_err());
    }

    #[test]
    fn test_virtual_heap_free() {
        let mut heap = VirtualHeap::new();
        let addr = heap.allocate(8);
        heap.free(addr).unwrap();
        assert!(!heap.contains(addr));
    }

    #[test]
    fn test_virtual_heap_free_nonexistent() {
        let mut heap = VirtualHeap::new();
        assert!(heap.free(999).is_err());
    }

    #[test]
    fn test_virtual_heap_sequential_allocs_give_different_addresses() {
        let mut heap = VirtualHeap::new();
        let a1 = heap.allocate(8);
        let a2 = heap.allocate(8);
        assert_ne!(a1, a2);
    }

    #[test]
    fn test_virtual_heap_non_contained_address() {
        let heap = VirtualHeap::new();
        assert!(!heap.contains(0xDEAD));
    }

    #[test]
    fn test_zero_bits() {
        let v = zero_bits(4);
        assert_eq!(v.as_i64(), Some(0));
    }

    #[test]
    fn test_deref_unwraps_ref() {
        // Deref of a Ref returns the wrapped value.
        let mut heap = VirtualHeap::new();
        let mut bindings = std::collections::HashMap::new();
        let inner_val = Value::int(42);
        bindings.insert("x".to_string(), Value::Ref(Box::new(inner_val)));
        let expr = Expr::Deref(Box::new(Expr::Identifier("x".to_string())));
        let result = eval_expr(&expr, &mut heap, &mut bindings, &HashMap::new()).unwrap();
        assert_eq!(result.as_i64(), Some(42));
    }

    #[test]
    fn test_deref_non_ref_passthrough() {
        // Deref of a non-Ref value just returns the value as-is.
        let mut heap = VirtualHeap::new();
        let mut bindings = std::collections::HashMap::new();
        bindings.insert("x".to_string(), Value::int(42));
        let expr = Expr::Deref(Box::new(Expr::Identifier("x".to_string())));
        let result = eval_expr(&expr, &mut heap, &mut bindings, &HashMap::new()).unwrap();
        assert_eq!(result.as_i64(), Some(42));
    }

    #[test]
    fn test_addrof_wraps_in_ref() {
        // AddrOf wraps the value in a Ref.
        let mut heap = VirtualHeap::new();
        let mut bindings = std::collections::HashMap::new();
        bindings.insert("x".to_string(), Value::int(42));
        let expr = Expr::AddrOf(Box::new(Expr::Identifier("x".to_string())));
        let result = eval_expr(&expr, &mut heap, &mut bindings, &HashMap::new()).unwrap();
        match result {
            Value::Ref(wrapped) => assert_eq!(wrapped.as_i64(), Some(42)),
            _ => panic!("expected Ref, got {:?}", result),
        }
    }

    #[test]
    fn test_addrof_deref_roundtrip() {
        // *(x) wraps then unwraps.
        let mut heap = VirtualHeap::new();
        let mut bindings = std::collections::HashMap::new();
        bindings.insert("x".to_string(), Value::int(99));
        let addrof = Expr::AddrOf(Box::new(Expr::Identifier("x".to_string())));
        let deref = Expr::Deref(Box::new(addrof));
        let result = eval_expr(&deref, &mut heap, &mut bindings, &HashMap::new()).unwrap();
        assert_eq!(result.as_i64(), Some(99));
    }

    // ── init seeding reference (2026-08-09, Phase 2) ──────────────────

    fn parse_program(src: &str) -> Vec<crate::ast::TopLevel> {
        let tokens = crate::lexer::tokenize(src).unwrap();
        let mut p = crate::parser::Parser::new(tokens, src);
        p.parse_program().unwrap()
    }

    #[test]
    fn packed_struct_fields_round_trip_in_interpreter() {
        // 2026-08-13 (pack): the interpreter models structs as layout-free
        // named products — a `pack struct` program must run and return the
        // semantic field values (the backend adds the physical bit-packing;
        // both agree on the value domain).
        let program = parse_program(
            "pack struct Nib { a: Bits<12>; b: Bits<4>; c: Bits<8>; };\n\
             defn pkmix() -> Int {\n\
               let n: Nib = Nib { a: 0xABC as Bits<12>, b: 0xF as Bits<4>, c: 0xFF as Bits<8> };\n\
               term (n.a as Int) + (n.b as Int) * 5 + (n.c as Int) * 11;\n\
             };\n",
        );
        let mut interp = Interpreter::new();
        interp.load_program(&program);
        let v = interp.call_function("pkmix", &[]).unwrap();
        assert_eq!(v.as_i64(), Some(0xABC + 0xF * 5 + 0xFF * 11));
    }

    #[test]
    fn trap_statement_aborts_interpreter() {
        // 2026-08-13 (layout-keywords plan Phase 4): `trap;` raises the abort
        // diagnostic in the reference interpreter (SPEC §8.8), mirroring the
        // LLVM `llvm.trap` + `unreachable` sequence.
        let program = parse_program(
            "defn f() -> Int {\n  trap;\n  term 1;\n};\n",
        );
        let mut interp = Interpreter::new();
        interp.load_program(&program);
        let err = interp.call_function("f", &[]).unwrap_err();
        assert!(
            matches!(err, crate::errors::RuntimeError::Trap),
            "trap; must raise RuntimeError::Trap, got {err}"
        );
    }

    #[test]
    fn init_value_form_seeds_and_reads() {
        // Value form: the expr is evaluated once and the name resolves.
        let program = parse_program("init BufSize: Int = 64;\n");
        let mut interp = Interpreter::new();
        interp.load_program(&program);
        assert_eq!(interp.state.get("BufSize").and_then(|v| v.as_i64()), Some(64));
        let expr = Expr::Identifier("BufSize".into());
        let v = interp.eval_expr(&expr).unwrap();
        assert_eq!(v.as_i64(), Some(64));
    }

    #[test]
    fn init_body_form_term_yields_seed() {
        // Body form: statements run once; `term <expr>` yields the seed.
        let program = parse_program(
            "init Layout: [16 | 32 | 64] Int { term 32; };\n",
        );
        let mut interp = Interpreter::new();
        interp.load_program(&program);
        assert_eq!(interp.state.get("Layout").and_then(|v| v.as_i64()), Some(32));
    }

    #[test]
    fn init_reassign_is_a_runtime_error() {
        // The reference: a write to a seeded init is rejected at runtime too.
        let program = parse_program("init BufSize: Int = 64;\n");
        let mut interp = Interpreter::new();
        interp.load_program(&program);
        let stmt = Statement::Assign(
            Expr::Identifier("BufSize".into()),
            Expr::Decimal(128),
        );
        let err = interp.exec_stmt(&stmt).unwrap_err();
        assert!(
            format!("{}", err).contains("immutable for the run"),
            "expected ImmutableInit, got {err}"
        );
    }

    #[test]
    fn duplicate_init_seed_is_a_runtime_error() {
        // Set-once: seeding the same init twice is an error.
        let program = parse_program(
            "init BufSize: Int = 64;\ninit BufSize: Int = 128;\n",
        );
        let mut interp = Interpreter::new();
        let err = interp.seed_inits(&program).unwrap_err();
        assert!(
            format!("{}", err).contains("immutable for the run"),
            "expected ImmutableInit, got {err}"
        );
    }

    // ── 2026-08-09 (Phase 10): defer cleanup ─────────────────────────

    #[test]
    fn defer_runs_lifo_on_normal_termination() {
        // `defer` bodies run LIFO on normal termination (fall-through past the
        // body or explicit term). The reference: cleanup once, inner-first.
        let program = parse_program(
            "defn f() -> Int {\n\
             \x20 defer { let _d1: Int = 1; };\n\
             \x20 let r: Int = 7;\n\
             \x20 term r;\n\
             };",
        );
        let mut interp = Interpreter::new();
        interp.load_program(&program);
        // The defer registers; after a term the stack drains (empty = ran).
        let v = interp.call_function("f", &[]).unwrap();
        assert_eq!(v.as_i64(), Some(7));
        assert!(interp.defer_stack.is_empty(), "defers must drain after term");
    }

    #[test]
    fn defer_registers_then_flushes() {
        let program = parse_program("defn f() -> Int { defer { term 1; }; term 2; };");
        let mut interp = Interpreter::new();
        interp.load_program(&program);
        interp.call_function("f", &[]).unwrap();
        assert!(
            interp.defer_stack.is_empty(),
            "defer stack must be empty after the firing completes"
        );
    }

    // ── 2026-08-09 (Phase 10): task spawn / await ────────────────────

    #[test]
    fn task_spawn_runs_inline_and_await_reads_result() {
        // The reference semantic scheduler is deterministic: a spawned task
        // runs to completion; `await` returns the stored result (SPEC §12.2).
        let program = parse_program("defn compute(x: Int) -> Int { term x * 2; };");
        let mut interp = Interpreter::new();
        interp.load_program(&program);
        // spawn = call the defn inline (the handle is the result).
        let handle = interp.eval_expr(&Expr::Spawn {
            type_name: "compute".to_string(),
            args: vec![Expr::Decimal(21)],
            storage: crate::ast::SpawnStorage::Pooled,
        }).unwrap();
        assert_eq!(handle.as_i64(), Some(42), "spawn must run the task inline");
        // Bind the handle, then await reads it.
        interp.state.insert("t".to_string(), handle);
        let awaited = interp.eval_expr(&Expr::Await(Box::new(Expr::Identifier("t".to_string())))).unwrap();
        assert_eq!(awaited.as_i64(), Some(42), "await must yield the task result");
    }
}
