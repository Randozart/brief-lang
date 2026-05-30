fn escape_llvm_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for byte in s.bytes() {
        match byte {
            b'\\' => out.push_str("\\\\"),
            b'"' => out.push_str("\\22"),
            b'\n' => out.push_str("\\0a"),
            b'\r' => out.push_str("\\0d"),
            b'\t' => out.push_str("\\09"),
            0x20..=0x7e => out.push(byte as char),
            b => { let _ = write!(out, "\\{:02x}", b); }
        }
    }
    out
}

fn float_to_llvm_hex(f: f64) -> String {
    let f32_val = f as f32;
    let bits = f32_val.to_bits();
    format!("{}", bits)
}
use crate::ast::{
    DispatchMode, Expr, ForeignSignature, MatchPattern, Program, Statement, TopLevel, Type,
};
use std::collections::HashMap;
use std::fmt::Write;

/// Collect all unique string literal values from the program for global emission.
fn collect_strings(program: &Program) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for item in &program.items {
        collect_strings_tl(item, &mut seen, &mut out);
    }
    out
}
fn collect_strings_tl(tl: &TopLevel, seen: &mut std::collections::HashSet<String>, out: &mut Vec<String>) {
    match tl {
        TopLevel::Transaction(t) => { for s in &t.body { collect_strings_stmt(s, seen, out); } }
        TopLevel::Definition(d) => { for s in &d.body { collect_strings_stmt(s, seen, out); } }
        _ => {}
    }
}
fn collect_strings_stmt(stmt: &Statement, seen: &mut std::collections::HashSet<String>, out: &mut Vec<String>) {
    match stmt {
        Statement::Let { expr, .. } => { if let Some(e) = expr { collect_strings_expr(e, seen, out); } }
        Statement::Assignment { expr, .. } => { collect_strings_expr(expr, seen, out); }
        Statement::Expression(e) => { collect_strings_expr(e, seen, out); }
        Statement::Term { values, .. } => { for v in values.iter().flatten() { collect_strings_expr(v, seen, out); } }
        Statement::Guarded { condition, statements, .. } => {
            collect_strings_expr(condition, seen, out);
            for s in statements { collect_strings_stmt(s, seen, out); }
        }
        Statement::Unification { expr, .. } => { collect_strings_expr(expr, seen, out); }
        _ => {}
    }
}
fn collect_strings_expr(expr: &Expr, seen: &mut std::collections::HashSet<String>, out: &mut Vec<String>) {
    use Expr::*;
    match expr {
        String(s) => {
            if !seen.contains(s) {
                seen.insert(s.clone());
                out.push(s.clone());
            }
        }
        Add(l, r) | Sub(l, r) | Mul(l, r) | Div(l, r) | Mod(l, r) | Eq(l, r) | Ne(l, r)
        | Lt(l, r) | Le(l, r) | Gt(l, r) | Ge(l, r) | And(l, r) | Or(l, r)
        | BitAnd(l, r) | BitOr(l, r) | BitXor(l, r) | Shl(l, r) | Shr(l, r)
        | Concat(l, r) => {
            collect_strings_expr(l, seen, out);
            collect_strings_expr(r, seen, out);
        }
        Not(e) | Neg(e) | BitNot(e) | Cast(e, _) | Exists { expr: e, .. } => {
            collect_strings_expr(e, seen, out);
        }
        Block(stmts, last) => {
            for s in stmts { collect_strings_stmt(s, seen, out); }
            collect_strings_expr(last, seen, out);
        }
        Match { value, arms } => {
            collect_strings_expr(value, seen, out);
            for arm in arms { collect_strings_expr(&arm.body, seen, out); }
        }
        PatternMatch { value, .. } => { collect_strings_expr(value, seen, out); }
        Call(_, args) => { for a in args { collect_strings_expr(a, seen, out); } }
        ListLiteral(elems) => { for e in elems { collect_strings_expr(e, seen, out); } }
        ListIndex(l, i) => { collect_strings_expr(l, seen, out); collect_strings_expr(i, seen, out); }
        Slice { value, start, .. } => {
            collect_strings_expr(value, seen, out);
            if let Some(s) = start { collect_strings_expr(s, seen, out); }
        }
        MultiSlice { value, .. } => { collect_strings_expr(value, seen, out); }
        Tuple(elems) => { for e in elems { collect_strings_expr(e, seen, out); } }
        TupleDestructure(_, e) => { collect_strings_expr(e, seen, out); }
        StructInstance(_, fields) => { for (_, e) in fields { collect_strings_expr(e, seen, out); } }
        ObjectLiteral(fields) => { for (_, e) in fields { collect_strings_expr(e, seen, out); } }
        FieldAccess(o, _) => { collect_strings_expr(o, seen, out); }
        ForAll { expr, .. } => {
            collect_strings_expr(expr, seen, out);
        }
        _ => {}
    }
}

/// LLVM IR backend — the definitive compiler from Brief AST to `.ll`.
///
/// Every lesson from phases 0–5.5 integrated into one coherent pass:
/// - `noalias nocapture` on all `%State*` — LLVM sees no pointer aliasing
/// - i64-centric expression system — strings/lists become `i64` via `ptrtoint`/`inttoptr`
/// - Bool (i8) fields trunc on store, zext on load; floats via bitcast+zext; char via zext
/// - Unique guard labels, `returns_i64` flag, fused txn terminator filtering
/// - All Expr/Statement/TopLevel variants emit valid IR
/// - Contracts: `!range`, `@llvm.assume` (debug panic / release assume)
/// - Match→switch with phi merge, unification, pattern match
/// - FFI declare+call with C ABI, bootstrap intrinsics (`__print`, `__exit`)
/// - Transition fusing, trigger sampling by MMIO/linked address
/// - Precondition extraction → internal `i1` functions, dispatch chain
/// - User-provided `frgn __wait_for_event` + `rct txn [true]` for sleep
/// - `@ link` triggers → `external global` + `load volatile`

/// LLVM storage type for an `@ link` trigger global.
/// The C runtime provides `char` (Bool→i8), `int64_t` (Int→i64),
/// and `char*` (String→i8*).
fn trg_llvm_storage_ty(ty: &Type) -> &str {
    match ty {
        Type::Bool => "i8",
        Type::Int | Type::UInt => "i64",
        Type::Char => "i32",
        Type::String | Type::Data => "i8*",
        _ => "i8", // fallback for unsupported types
    }
}

pub struct LlvmBackend {
    spec: Option<crate::target_spec::TargetSpec>,
    field_index_map: HashMap<String, usize>,
    field_types: Vec<String>,
    field_initializers: HashMap<String, Option<Expr>>,
    txn_counter: usize,
    has_cycles: bool,
    pending_cleanup: Vec<Statement>,
    let_bindings: HashMap<String, String>,
    register_types: HashMap<String, Type>,
    terminated: bool,
    returns_i64: bool,
    range_bounds: HashMap<String, (i64, i64)>,
    field_to_meta_idx: HashMap<String, usize>,
    triggers: HashMap<String, crate::ast::TriggerDeclaration>,
    trigger_names: Vec<String>,
    program_txns: Vec<String>,
    frgn_map: HashMap<String, ForeignSignature>,
    defn_params: HashMap<String, Vec<Type>>,
    string_constants: Vec<String>,
    fused_to_first: HashMap<String, String>,
    sampled_triggers: HashMap<String, String>,
    txn_write_masks: HashMap<String, u64>,
}

impl LlvmBackend {
    pub fn new() -> Self {
        LlvmBackend {
            spec: None,
            field_index_map: HashMap::new(),
            field_types: Vec::new(),
            field_initializers: HashMap::new(),
            txn_counter: 0,
            has_cycles: false,
            pending_cleanup: Vec::new(),
            let_bindings: HashMap::new(),
            register_types: HashMap::new(),
            terminated: false,
            returns_i64: false,
            range_bounds: HashMap::new(),
            field_to_meta_idx: HashMap::new(),
            triggers: HashMap::new(),
            trigger_names: Vec::new(),
            program_txns: Vec::new(),
            frgn_map: HashMap::new(),
            defn_params: HashMap::new(),
            string_constants: Vec::new(),
            fused_to_first: HashMap::new(),
            sampled_triggers: HashMap::new(),
            txn_write_masks: HashMap::new(),
        }
    }

    pub fn with_spec(mut self, spec: crate::target_spec::TargetSpec) -> Self {
        self.spec = Some(spec);
        self
    }

    pub fn generate(&mut self, program: &Program) -> String {
        let analysis = crate::backend::analyze_program(program, false);
        let cg = &analysis.call_graph;
        self.has_cycles = cg.has_cycle();

        self.build_field_index(program);
        self.triggers.clear();
        self.trigger_names.clear();
        self.program_txns.clear();
        self.defn_params.clear();
        self.string_constants = collect_strings(program);

        let mut txns: Vec<(String, &crate::ast::Transaction)> = Vec::new();
        for item in &program.items {
            match item {
                TopLevel::Transaction(t) => {
                    txns.push((t.name.clone(), t));
                    self.program_txns.push(t.name.clone());
                }
                TopLevel::Trigger(t) => {
                    self.triggers.insert(t.name.clone(), t.clone());
                    self.trigger_names.push(t.name.clone());
                }
                TopLevel::Definition(d) => {
                    let tys: Vec<Type> = d.parameters.iter().map(|(_, t)| t.clone()).collect();
                    self.defn_params.insert(d.name.clone(), tys);
                }
                TopLevel::ForeignBinding { name, signature, .. } => {
                    self.frgn_map.insert(name.clone(), signature.clone());
                }
                _ => {}
            }
        }

        let mut out = String::new();
        self.emit_header(&mut out);
self.emit_declares(&mut out);

        // Emit foreign declares inline (frgn_map is populated from the scan above)
        for (name, sig) in &self.frgn_map {
            let ret_ty = match sig.result_type {
                crate::ast::ResultType::VoidType | crate::ast::ResultType::TrueAssertion => "void",
                crate::ast::ResultType::Projection(ref ts) => if ts.is_empty() { "void" } else { "i64" },
            };
            let param_tys: Vec<&str> = sig.inputs.iter().map(|(_, t)| match t {
                Type::Int | Type::UInt => "i64",
                Type::Bool => "i32",
                Type::Char => "i32",
                Type::String | Type::Data => "i8*",
                _ => "i64",
            }).collect();
            write!(out, "declare {} @{}(", ret_ty, name).ok();
            for (pi, pt) in param_tys.iter().enumerate() {
                if pi > 0 { write!(out, ", ").ok(); }
                write!(out, "{}", pt).ok();
            }
            writeln!(out, ") #1").ok();
        }

        // Emit external global declarations for linked triggers (fixes bug 4B)
        for (name, trg) in &self.triggers {
            if let crate::ast::LinkRef::Linked(sym) = &trg.address {
                let store_ty = trg_llvm_storage_ty(&trg.ty);
                let align = if store_ty == "i64" { 8 } else if store_ty == "i32" { 4 } else { 1 };
                writeln!(out, "@{} = external global {}, align {}", sym, store_ty, align).ok();
                // Warn on unsupported trigger types
                match &trg.ty {
                    Type::Bool | Type::Int | Type::UInt | Type::Char | Type::String | Type::Data => {}
                    _ => {
                        eprintln!("warning:{}:{}: trigger '{}' has type {:?} which the LLVM runtime does not fully support; using i8 storage",
                            trg.span.as_ref().map(|s| s.line).unwrap_or(0),
                            trg.span.as_ref().map(|s| s.column).unwrap_or(0),
                            name, trg.ty);
                    }
                }
            }
        }
        if self.triggers.iter().any(|(_, t)| matches!(t.address, crate::ast::LinkRef::Linked(_))) {
            writeln!(out).ok();
        }

        // Declare C stdlib functions for bootstrap intrinsics
        let has_print = self.frgn_map.contains_key("__print");
        let has_exit = self.frgn_map.contains_key("__exit");
        if has_print {
            writeln!(out, "declare i64 @write(i32, i8*, i64) #1").ok();
            writeln!(out, "declare i64 @strlen(i8*) #1").ok();
        }
        if has_exit {
            writeln!(out, "declare void @exit(i32) #1").ok();
        }
        writeln!(out).ok();

        self.declare_state_type(&mut out);
        writeln!(out, "@global_state = global %State zeroinitializer\n").ok();

        // Emit string constants
        for (si, s) in self.string_constants.iter().enumerate() {
            let escaped = escape_llvm_string(s);
            writeln!(out, "@str.{} = private unnamed_addr constant [{} x i8] c\"{}\\00\", align 1", si, s.len() + 1, escaped).ok();
        }
        if !self.string_constants.is_empty() { writeln!(out).ok(); }

        let mut range_meta: Vec<String> = Vec::new();

        // Definitions
        for item in &program.items {
            if let TopLevel::Definition(d) = item {
                self.emit_definition(&mut out, d);
                writeln!(out).ok();
            }
        }
        // Transactions
        for (name, txn) in &txns {
            self.emit_transaction(&mut out, txn, name, &mut range_meta);
            writeln!(out).ok();
        }
        // Precondition functions
        for (name, txn) in &txns {
            self.emit_pre_function(&mut out, txn, name);
        }
        // Fused transactions
        let fusable = self.resolve_fusable_pairs(&txns);
        for (a, b) in &fusable {
            if let (Some(ta), Some(tb)) = (
                txns.iter().find(|(n, _)| n == a).map(|(_, t)| t),
                txns.iter().find(|(n, _)| n == b).map(|(_, t)| t),
            ) {
                self.emit_fused(&mut out, ta, tb, &format!("{}_{}_fused", a, b));
                writeln!(out).ok();
            }
        }
        // Init
        self.emit_init_state(&mut out);
        writeln!(out).ok();
        // Reactor — sequential or parallel
        // Reactor tick — use folded path when a single bounded-counter txn
        // with no triggers can be collapsed into a canonical while loop.
        let graph = &analysis.transition_graph;
        let foldable = graph.nodes.len() == 1
            && !graph.has_triggers
            && txns.len() == 1
            && graph.nodes[0].bounded_pre.is_some()
            && graph.nodes[0].increments.is_some()
            && graph.nodes[0].is_reactive;

        let folded = if foldable {
            let node = &graph.nodes[0];
            let bp = node.bounded_pre.as_ref().unwrap();
            let inc = node.increments.as_ref().unwrap();
            if bp.var == inc.var {
                if let (Some(&total_idx), Some(&counter_idx)) = (
                    self.field_index_map.get(&bp.bound_var),
                    self.field_index_map.get(&bp.var),
                ) {
                    if node.is_pure_body {
                        let total_val = self.field_initializers
                            .get(&bp.bound_var)
                            .and_then(|e| e.as_ref())
                            .and_then(|e| {
                                if let Expr::Integer(n) = e { Some(*n) } else { None }
                            })
                            .unwrap_or(1);
                        self.emit_folded_pure_counter(&mut out, counter_idx, total_val);
                        true
                    } else {
                        self.emit_folded_main(&mut out, &node.name, counter_idx, total_idx);
                        true
                    }
                } else { false }
            } else { false }
        } else { false };

        if !folded {
            if !txns.is_empty() {
                match program.dispatch_mode {
                    DispatchMode::Parallel => {
                        self.build_write_masks(program);
                        self.emit_parallel_reactor(&mut out, &txns, &fusable);
                    }
                    DispatchMode::Sequential => {
                        self.emit_reactor(&mut out, &txns, &fusable);
                    }
                }
            } else {
                writeln!(out, "define void @reactor_tick() local_unnamed_addr #2 {{").ok();
                writeln!(out, "  entry:").ok();
                writeln!(out, "  ret void").ok();
                writeln!(out, "}}").ok();
                writeln!(out).ok();
            }
            // Main
            let has_wake_triggers = self.triggers.values().any(|t| t.is_wake);
            self.emit_main(&mut out, has_wake_triggers);
            // Wake trigger metadata
            if has_wake_triggers {
                self.emit_wake_metadata(&mut out);
            }
        }
        // Attributes
        writeln!(out).ok();
        writeln!(out, "attributes #0 = {{").ok();
        writeln!(out, "    mustprogress nofree norecurse nosync nounwind willreturn").ok();
        writeln!(out, "    memory(argmem: readwrite)").ok();
        writeln!(out, "}}").ok();
        writeln!(out, "attributes #1 = {{ nocallback nofree nosync nounwind willreturn memory(argmem: write) }}").ok();
        writeln!(out, "attributes #2 = {{ mustprogress nofree norecurse nosync nounwind memory(readwrite) }}").ok();
        writeln!(out, "attributes #3 = {{ nofree norecurse nosync nounwind memory(readwrite) }}").ok();
        // Range metadata
        if !range_meta.is_empty() {
            writeln!(out).ok();
            for m in &range_meta {
                writeln!(out, "{}", m).ok();
            }
        }
        out
    }

    // ── Header ────────────────────────────────────────────────
    fn emit_header(&self, out: &mut String) {
        writeln!(out, "; ModuleID = 'program.ll'").ok();
        writeln!(out, "source_filename = \"program.bv\"").ok();
        writeln!(out, "target datalayout = \"e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-i128:128-f80:128-n8:16:32:64-S128\"").ok();
        writeln!(out, "target triple = \"x86_64-unknown-linux-gnu\"").ok();
    }

    fn emit_declares(&self, out: &mut String) {
        writeln!(out).ok();
        writeln!(out, "declare void @llvm.assume(i1) #1").ok();
        // __wait_for_event is no longer a built-in intrinsic.
        // Users declare it via frgn __wait_for_event() -> Void from "libruntime";
        // and the normal FFI path handles the declare emission.
        // __rt_init and __rt_wait are weak symbols provided by the C runtime.
        // If not linked, these resolve to a no-op stub or linker error.
        writeln!(out, "declare void @__rt_init() local_unnamed_addr").ok();
        writeln!(out, "declare void @__rt_wait() local_unnamed_addr").ok();
    }

    fn emit_foreign_declares(&mut self, out: &mut String) {
        self.frgn_map.clear();
        // Collect from the program items (called in generate, after header)
        // This is just the declare section — actual foreign binding iteration
        // is done in generate() before this function would be called.
        // We emit declares for known bootstrap intrinsics conditionally.
    }

    // ── Field index ───────────────────────────────────────────
    fn build_field_index(&mut self, program: &Program) {
        self.field_index_map.clear();
        self.field_types.clear();
        self.field_initializers.clear();
        for item in &program.items {
            if let TopLevel::StateDecl(s) = item {
                self.field_index_map
                    .insert(s.name.clone(), self.field_types.len());
                self.field_types.push(self.llvm_type(&s.ty).to_string());
                self.field_initializers.insert(s.name.clone(), s.expr.clone());
            }
        }
    }

    fn llvm_type(&self, ty: &Type) -> &str {
        match ty {
            Type::Int | Type::UInt => "i64",
            Type::Bool => "i8",
            Type::Float => "float",
            Type::Char => "i32",
            Type::String | Type::Data => "i8*",
            Type::Void => "void",
            _ => "i64",
        }
    }

    /// LLVM storage type for an `@ link` trigger global.
    /// The C runtime provides `char` (Bool→i8), `int64_t` (Int→i64),
    /// and `char*` (String→i8*).
    /// Emit a volatile load of a trigger into an i64 register.
    /// Different source types need different load+convert sequences.
    fn emit_trg_load(
        &mut self,
        out: &mut String,
        indent: &str,
        dst: &str,
        addr_src: &str,
        addr_is_ptr: bool,
        trg_ty: &Type,
    ) {
        let store_ty = trg_llvm_storage_ty(trg_ty);
        let tr_counter = self.txn_counter;
        self.txn_counter += 1;
        let raw = format!("%tr{}", tr_counter);
        if addr_is_ptr {
            writeln!(out, "{}{} = load volatile {}, {}* {}", indent, raw, store_ty, store_ty, addr_src).ok();
        } else {
            writeln!(out, "{}{} = load volatile {}, {}* inttoptr (i64 {} to {}*), align 1", indent, raw, store_ty, store_ty, addr_src, store_ty).ok();
        }
        // Convert to i64
        match trg_ty {
            Type::Bool => {
                let zc = self.txn_counter; self.txn_counter += 1;
                let z = format!("%tz{}", zc);
                writeln!(out, "{}{} = zext i8 {} to i64", indent, z, raw).ok();
                writeln!(out, "{}{} = add i64 0, {}", indent, dst, z).ok();
            }
            Type::Int | Type::UInt => {
                writeln!(out, "{}{} = add i64 0, {}", indent, dst, raw).ok();
            }
            Type::Char => {
                let zc = self.txn_counter; self.txn_counter += 1;
                let z = format!("%tz{}", zc);
                writeln!(out, "{}{} = zext i32 {} to i64", indent, z, raw).ok();
                writeln!(out, "{}{} = add i64 0, {}", indent, dst, z).ok();
            }
            Type::String | Type::Data => {
                let pc = self.txn_counter; self.txn_counter += 1;
                let p = format!("%tp{}", pc);
                writeln!(out, "{}{} = ptrtoint {} {} to i64", indent, p, store_ty, raw).ok();
                writeln!(out, "{}{} = add i64 0, {}", indent, dst, p).ok();
            }
            _ => {
                // fallback for unsupported types — handle as i8
                let zc = self.txn_counter; self.txn_counter += 1;
                let z = format!("%tz{}", zc);
                writeln!(out, "{}{} = zext i8 {} to i64", indent, z, raw).ok();
                writeln!(out, "{}{} = add i64 0, {}", indent, dst, z).ok();
            }
        }
    }

    fn align_of(&self, ty: &str) -> u32 {
        match ty {
            "i64" => 8,
            "float" => 4,
            "i8" => 1,
            "i32" => 4,
            _ => 8,
        }
    }

    fn declare_state_type(&mut self, out: &mut String) {
        if self.field_types.is_empty() {
            writeln!(out, "%State = type {{ i64 }}").ok();
            return;
        }
        write!(out, "%State = type {{ ").ok();
        for (i, f) in self.field_types.iter().enumerate() {
            if i > 0 { write!(out, ", ").ok(); }
            write!(out, "{}", f).ok();
        }
        writeln!(out, " }}").ok();
    }

    // ── INIT STATE ────────────────────────────────────────────
    fn emit_init_state(&mut self, out: &mut String) {
        writeln!(out, "define void @init_state() local_unnamed_addr #0 {{").ok();
        writeln!(out, "  entry:").ok();
        let mut reg = 0u32;
        let mut fields: Vec<(&String, &usize)> = self.field_index_map.iter().collect();
        fields.sort_by_key(|&(_, &idx)| idx);
        for (name, &idx) in fields {
            let ty = &self.field_types[idx];
            let p = format!("%ip{}", reg); reg += 1;
            writeln!(out, "  {} = getelementptr inbounds %State, %State* @global_state, i32 0, i32 {}", p, idx).ok();
            let init = self.field_initializers.get(name).and_then(|e| e.as_ref());
            let val_str = match init {
                Some(Expr::Integer(n)) => n.to_string(),
                Some(Expr::Float(f)) => float_to_llvm_hex(*f),
                Some(Expr::Neg(inner)) => match inner.as_ref() {
                    Expr::Float(f) => float_to_llvm_hex(-*f),
                    Expr::Integer(n) => format!("-{}", n),
                    _ => "0".to_string(),
                },
                Some(Expr::Bool(b)) => if *b { "1".to_string() } else { "0".to_string() },
                Some(Expr::String(_)) => "null".to_string(),
                Some(Expr::Char(c)) => (*c as i32).to_string(),
                _ => if ty == "i8*" { "null".to_string() } else { "0".to_string() },
            };
            let is_float_init = matches!(init, Some(Expr::Float(_)) | Some(Expr::Neg(_)));
            if is_float_init {
                let bits_reg = format!("%ip{}b", reg - 1);
                writeln!(out, "  {} = bitcast i32 {} to float", bits_reg, val_str).ok();
                writeln!(out, "  store volatile float {}, float* {}, align {}", bits_reg, p, self.align_of("float")).ok();
            } else {
                writeln!(out, "  store volatile {} {}, {}* {}, align {}", ty, val_str, ty, p, self.align_of(ty)).ok();
            }
        }
        writeln!(out, "  ret void").ok();
        writeln!(out, "}}").ok();
    }

    // ── DEFINITION ────────────────────────────────────────────
    fn emit_definition(&mut self, out: &mut String, d: &crate::ast::Definition) {
        self.pending_cleanup.clear();
        self.let_bindings.clear();
        write!(out, "define i64 @{}(", d.name).ok();
        for (i, (n, t)) in d.parameters.iter().enumerate() {
            if i > 0 { write!(out, ", ").ok(); }
            write!(out, "{} %arg{}", self.llvm_type(t), i).ok();
        }
        writeln!(out, ") local_unnamed_addr #0 {{").ok();
        writeln!(out, "  entry:").ok();
        // Param conversions
        for (i, (n, t)) in d.parameters.iter().enumerate() {
            let raw = format!("%arg{}", i);
            let conv = format!("%ac{}", i);
            match t {
                Type::Bool => { writeln!(out, "  {} = zext i8 {} to i64", conv, raw).ok(); }
                Type::Char => { writeln!(out, "  {} = zext i32 {} to i64", conv, raw).ok(); }
                Type::String | Type::Data => { writeln!(out, "  {} = ptrtoint i8* {} to i64", conv, raw).ok(); }
                Type::Float => {
                    let m = format!("%ai{}", i);
                    writeln!(out, "  {} = bitcast float {} to i32", m, raw).ok();
                    writeln!(out, "  {} = zext i32 {} to i64", conv, m).ok();
                }
                _ => {}
            }
            if !matches!(t, Type::Int | Type::UInt) {
                self.let_bindings.insert(n.clone(), conv.clone());
            } else {
                self.let_bindings.insert(n.clone(), raw.clone());
            }
        }
        self.txn_counter = 0;
        self.terminated = false;
        self.returns_i64 = true;
        for s in &d.body { self.emit_stmt(out, s, "  "); }
        if !self.terminated { writeln!(out, "  ret i64 0").ok(); }
        writeln!(out, "}}").ok();
    }

    // ── TRANSACTION ───────────────────────────────────────────
    fn emit_transaction(&mut self, out: &mut String, txn: &crate::ast::Transaction, name: &str, range_meta: &mut Vec<String>) {
        self.pending_cleanup.clear();
        self.range_bounds = Self::extract_ranges(&txn.contract.pre_condition);
        self.field_to_meta_idx.clear();
        for (f, &(lo, hi)) in &self.range_bounds {
            if hi < i64::MAX {
                let mi = range_meta.len();
                let dlo = if lo > i64::MIN { lo } else { i64::MIN };
                range_meta.push(format!("!{} = !{{ i64 {}, i64 {} }}", mi, dlo, hi));
                self.field_to_meta_idx.insert(f.clone(), mi);
            }
        }
        let alwaysinline = if !self.has_cycles { " alwaysinline" } else { "" };
        writeln!(out, "define void @{}(%State* noalias nocapture %state) local_unnamed_addr #0{} {{", name, alwaysinline).ok();
        writeln!(out, "  entry:").ok();
        self.txn_counter = 0;
        self.let_bindings.clear();
        self.terminated = false;
        self.returns_i64 = false;
        // Precondition
        if !matches!(txn.contract.pre_condition, Expr::Bool(true)) {
            self.emit_precondition_check(out, &txn.contract.pre_condition, "  ");
        }
        for s in &txn.body { self.emit_stmt(out, s, "  "); }
        if !self.terminated { writeln!(out, "  ret void").ok(); }
        writeln!(out, "}}").ok();
    }

    // ── PRECONDITION CHECK (inline) ───────────────────────────
    fn emit_precondition_check(&mut self, out: &mut String, pre: &Expr, indent: &str) {
        let cond = self.emit_expr(out, pre, indent);
        let i1 = format!("%pi{}", self.txn_counter); self.txn_counter += 1;
        writeln!(out, "{}{} = icmp ne i64 {}, 0", indent, i1, cond).ok();
        let panic_l = format!("pp{}", self.txn_counter); self.txn_counter += 1;
        let safe_l = format!("ps{}", self.txn_counter); self.txn_counter += 1;
        writeln!(out, "{}br i1 {}, label %{}, label %{}", indent, i1, safe_l, panic_l).ok();
        writeln!(out, "{}{}:", indent, panic_l).ok();
        writeln!(out, "{}  unreachable", indent).ok();
        writeln!(out, "{}{}:", indent, safe_l).ok();
        // Feed the proven precondition to LLVM's optimizer
        writeln!(out, "{}call void @llvm.assume(i1 {})", indent, i1).ok();
    }

    // ── PRECONDITION FUNCTION ────────────────────────────────
    fn emit_pre_function(&mut self, out: &mut String, txn: &crate::ast::Transaction, name: &str) {
        if matches!(txn.contract.pre_condition, Expr::Bool(true)) { return; }
        writeln!(out, "define internal i1 @pre_{}(%State* noalias nocapture %state) #0 {{", name).ok();
        writeln!(out, "  entry:").ok();
        self.txn_counter = 0;
        self.let_bindings.clear();
        let cond = self.emit_expr(out, &txn.contract.pre_condition, "  ");
        let i1 = format!("%ri{}", self.txn_counter); self.txn_counter += 1;
        writeln!(out, "  {} = icmp ne i64 {}, 0", i1, cond).ok();
        writeln!(out, "  ret i1 {}", i1).ok();
        writeln!(out, "}}").ok();
    }

    // ── FUSED TRANSACTION ─────────────────────────────────────
    fn emit_fused(&mut self, out: &mut String, a: &crate::ast::Transaction, b: &crate::ast::Transaction, name: &str) {
        let body_a: Vec<Statement> = a.body.iter()
            .filter(|s| !matches!(s, Statement::Term { .. } | Statement::Escape(_)))
            .cloned().collect();
        let combined: Vec<Statement> = body_a.into_iter().chain(b.body.iter().cloned()).collect();
        writeln!(out, "define void @{}(%State* noalias nocapture %state) local_unnamed_addr #0 {{", name).ok();
        writeln!(out, "  entry:").ok();
        self.txn_counter = 0; self.let_bindings.clear(); self.terminated = false; self.returns_i64 = false;
        for s in &combined { self.emit_stmt(out, s, "  "); }
        if !self.terminated { writeln!(out, "  ret void").ok(); }
        writeln!(out, "}}").ok();
    }

    // ── STATEMENTS ────────────────────────────────────────────
    fn emit_stmt(&mut self, out: &mut String, stmt: &Statement, indent: &str) {
        match stmt {
            Statement::Term { values, .. } => {
                let c = self.pending_cleanup.clone();
                for s in &c { self.emit_stmt(out, s, indent); }
                if let Some(Some(v)) = values.first() {
                    let r = self.emit_expr(out, v, indent);
                    writeln!(out, "{}ret i64 {}", indent, r).ok();
                } else if self.returns_i64 {
                    writeln!(out, "{}ret i64 0", indent).ok();
                } else {
                    writeln!(out, "{}ret void", indent).ok();
                }
                self.terminated = true;
            }
            Statement::Escape(e) => {
                let c = self.pending_cleanup.clone();
                for s in &c { self.emit_stmt(out, s, indent); }
                if let Some(v) = e {
                    let r = self.emit_expr(out, v, indent);
                    writeln!(out, "{}ret i64 {}", indent, r).ok();
                } else {
                    writeln!(out, "{}ret void", indent).ok();
                }
                self.terminated = true;
            }
            Statement::Let { name, expr, address_expr, .. } => {
                if let Some(e) = expr {
                    let r = self.emit_expr(out, e, indent);
                    self.let_bindings.insert(name.clone(), r.clone());
                    writeln!(out, "{}; let {} = {}", indent, name, r).ok();
                } else {
                    writeln!(out, "{}; let {} = undef", indent, name).ok();
                }
            }
            Statement::Assignment { lhs, expr, modifiers, .. } => {
                let val = self.emit_expr(out, expr, indent);
                let fname = match lhs {
                    Expr::Identifier(n) | Expr::OwnedRef(n) => n.clone(),
                    _ => { writeln!(out, "{}; assign {}", indent, val).ok(); return; }
                };
                let is_volatile = modifiers.iter().any(|h| h.name == "volatile");
                if let Some(&idx) = self.field_index_map.get(&fname) {
                    let ty = &self.field_types[idx];
                    let p = format!("%ap{}", self.txn_counter); self.txn_counter += 1;
                    writeln!(out, "{}{} = getelementptr inbounds %State, %State* %state, i32 0, i32 {}", indent, p, idx).ok();
                    let vol_str = if is_volatile { " volatile" } else { "" };
                    match ty.as_str() {
                        "i8" => {
                            let tr = format!("%tr{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = trunc i64 {} to i8", indent, tr, val).ok();
                            writeln!(out, "{}store{} i8 {}, i8* {}, align {}", indent, vol_str, tr, p, self.align_of(ty)).ok();
                        }
                        "float" => {
                            let tr = format!("%ftr{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = trunc i64 {} to i32", indent, tr, val).ok();
                            let fl = format!("%ffl{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = bitcast i32 {} to float", indent, fl, tr).ok();
                            writeln!(out, "{}store{} float {}, float* {}, align {}", indent, vol_str, fl, p, self.align_of(ty)).ok();
                        }
                        _ => {
                            writeln!(out, "{}store{} {} {}, {}* {}, align {}", indent, vol_str, ty, val, ty, p, self.align_of(ty)).ok();
                        }
                    }
                } else {
                    writeln!(out, "{}; assign {} to {}", indent, val, fname).ok();
                }
            }
            Statement::Guarded { condition, statements, .. } => {
                let cond = self.emit_expr(out, condition, indent);
                let i1 = format!("%gc{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = icmp ne i64 {}, 0", indent, i1, cond).ok();

                // Guard→select if single assignment
                if statements.len() == 1 {
                    if let Statement::Assignment { lhs, expr, modifiers, .. } = &statements[0] {
                        if let Expr::Identifier(n) | Expr::OwnedRef(n) = lhs {
                            if let Some(&idx) = self.field_index_map.get(n) {
                                let g_is_volatile = modifiers.iter().any(|h| h.name == "volatile");
                                let gvol = if g_is_volatile { " volatile" } else { "" };
                                let p = format!("%gp{}", self.txn_counter); self.txn_counter += 1;
                                let av = self.emit_expr(out, expr, indent);
                                let ty = &self.field_types[idx];
                                writeln!(out, "{}{} = getelementptr inbounds %State, %State* %state, i32 0, i32 {}", indent, p, idx).ok();
                                let se = format!("%gs{}", self.txn_counter); self.txn_counter += 1;
                                match ty.as_str() {
                                    "i8" => {
                                        let ld = format!("%gl{}", self.txn_counter); self.txn_counter += 1;
                                        writeln!(out, "{}{} = load i8, i8* {}, align {}", indent, ld, p, self.align_of(ty)).ok();
                                        let av_tr = format!("%gatr{}", self.txn_counter); self.txn_counter += 1;
                                        writeln!(out, "{}{} = trunc i64 {} to i8", indent, av_tr, av).ok();
                                        writeln!(out, "{}{} = select i1 {}, i8 {}, i8 {}", indent, se, i1, av_tr, ld).ok();
                                        writeln!(out, "{}store{} i8 {}, i8* {}, align {}", indent, gvol, se, p, self.align_of(ty)).ok();
                                    }
                                    "float" => {
                                        let ld = format!("%gl{}", self.txn_counter); self.txn_counter += 1;
                                        writeln!(out, "{}{} = load float, float* {}, align {}", indent, ld, p, self.align_of(ty)).ok();
                                        let av_tr = format!("%gatr{}", self.txn_counter); self.txn_counter += 1;
                                        writeln!(out, "{}{} = trunc i64 {} to i32", indent, av_tr, av).ok();
                                        let av_fl = format!("%gafl{}", self.txn_counter); self.txn_counter += 1;
                                        writeln!(out, "{}{} = bitcast i32 {} to float", indent, av_fl, av_tr).ok();
                                        writeln!(out, "{}{} = select i1 {}, float {}, float {}", indent, se, i1, av_fl, ld).ok();
                                        writeln!(out, "{}store{} float {}, float* {}, align {}", indent, gvol, se, p, self.align_of(ty)).ok();
                                    }
                                    _ => {
                                        let ld = format!("%gl{}", self.txn_counter); self.txn_counter += 1;
                                        writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, ld, p).ok();
                                        writeln!(out, "{}{} = select i1 {}, i64 {}, i64 {}", indent, se, i1, av, ld).ok();
                                        writeln!(out, "{}store{} i64 {}, i64* {}, align {}", indent, gvol, se, p, self.align_of(ty)).ok();
                                    }
                                }
                                return;
                            }
                        }
                    }
                }

                // Standard guarded block with unique labels
                let gid = format!("g{}", self.txn_counter); self.txn_counter += 1;
                let then_l = format!("{}_t", gid);
                let end_l = format!("{}_e", gid);
                let prev_terminated = self.terminated;
                self.terminated = false;
                writeln!(out, "{}br i1 {}, label %{}, label %{}", indent, i1, then_l, end_l).ok();
                writeln!(out, "{}{}:", indent, then_l).ok();
                for s in statements { self.emit_stmt(out, s, &format!("{}  ", indent)); }
                if !self.terminated { writeln!(out, "{}  br label %{}", indent, end_l).ok(); }
                writeln!(out, "{}{}:", indent, end_l).ok();
                self.terminated = prev_terminated;
            }
            Statement::Unification { name, pattern, expr } => {
                let val = self.emit_expr(out, expr, indent);
                let disc = format!("%ud{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = and i64 {}, 255", indent, disc, val).ok();
                let arm_l = format!("ua{}", self.txn_counter); self.txn_counter += 1;
                let def_l = format!("ud{}", self.txn_counter); self.txn_counter += 1;
                let merge_l = format!("um{}", self.txn_counter); self.txn_counter += 1;
                let target = if name == "None" || name == "Err" { 0u64 } else { 1u64 };
                writeln!(out, "{}switch i64 {}, label %{} [ i64 {}, label %{} ]", indent, disc, def_l, target, arm_l).ok();
                writeln!(out, "{}{}:", indent, arm_l).ok();
                let pay = format!("%up{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = lshr i64 {}, 8", indent, pay, val).ok();
                self.let_bindings.insert(pattern.clone(), pay.clone());
                writeln!(out, "{}br label %{}", indent, merge_l).ok();
                writeln!(out, "{}{}:", indent, def_l).ok();
                writeln!(out, "{}  unreachable", indent).ok();
                writeln!(out, "{}{}:", indent, merge_l).ok();
            }
            Statement::Expression(e) => { let _ = self.emit_expr(out, e, indent); }
            Statement::LocalTrigger { .. } => { writeln!(out, "{}; trg!", indent).ok(); }
            Statement::OnExit { body, .. } => { self.pending_cleanup.extend(body.iter().cloned()); }
            Statement::Alka(b) => { for l in b.content.lines() { let _ = writeln!(out, "{}{}", indent, l); } }
            Statement::InlineAsm { asm_string, .. } => { writeln!(out, "{}{}", indent, asm_string).ok(); }
        }
    }

    // ── EXPRESSIONS ───────────────────────────────────────────
    fn emit_expr(&mut self, out: &mut String, expr: &Expr, indent: &str) -> String {
        let v = format!("%t{}", self.txn_counter);
        self.txn_counter += 1;
        match expr {
            Expr::Integer(n) => { writeln!(out, "{}{} = add i64 0, {}", indent, v, n).ok(); self.register_types.insert(v.clone(), Type::Int); }
            Expr::Bool(b) => { writeln!(out, "{}{} = add i64 0, {}", indent, v, if *b { 1 } else { 0 }).ok(); self.register_types.insert(v.clone(), Type::Bool); }
            Expr::Float(f) => {
                let bits = float_to_llvm_hex(*f);
                let fl = format!("%ff{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = bitcast i32 {} to float", indent, fl, bits).ok();
                let i32 = format!("%fi{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = bitcast float {} to i32", indent, i32, fl).ok();
                writeln!(out, "{}{} = zext i32 {} to i64", indent, v, i32).ok();
                self.register_types.insert(v.to_string(), Type::Float);
            }
            Expr::String(s) => {
                // Find the index of this string in pre-collected constants
                let si = self.string_constants.iter().position(|x| x == s).unwrap_or(0);
                let g = format!("@str.{}", si);
                let p = format!("%sp{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr inbounds [{} x i8], [{} x i8]* {}, i64 0, i64 0", indent, p, s.len() + 1, s.len() + 1, g).ok();
                writeln!(out, "{}{} = ptrtoint i8* {} to i64", indent, v, p).ok();
                self.register_types.insert(v.clone(), Type::String);
            }
            Expr::Char(c) => {
                let ci = format!("%cc{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = add i32 0, {}", indent, ci, *c as i32).ok();
                writeln!(out, "{}{} = zext i32 {} to i64", indent, v, ci).ok();
            }
            Expr::Term => { writeln!(out, "{}{} = add i64 0, 0", indent, v).ok(); }
Expr::Identifier(name) => {
                if let Some(reg) = self.let_bindings.get(name) {
                    writeln!(out, "{}{} = add i64 0, {}", indent, v, reg).ok();
                } else if self.trigger_names.contains(name) {
                    if let Some(sampled) = self.sampled_triggers.get(name) {
                        writeln!(out, "{}{} = add i64 0, {}", indent, v, sampled).ok();
                    } else if let Some(t) = self.triggers.get(name).cloned() {
                        let addr_str = match &t.address {
                            crate::ast::LinkRef::Explicit(a) => a.to_string(),
                            crate::ast::LinkRef::Linked(s) => format!("@{}", s),
                        };
                        let addr_is_ptr = matches!(t.address, crate::ast::LinkRef::Linked(_));
                        self.emit_trg_load(out, indent, &v, &addr_str, addr_is_ptr, &t.ty);
                    } else {
                        writeln!(out, "{}{} = add i64 0, 0", indent, v).ok();
                    }
                } else if let Some(&idx) = self.field_index_map.get(name) {
                    let ty = &self.field_types[idx];
                    let p = format!("%fdp{}", self.txn_counter); self.txn_counter += 1;
                    writeln!(out, "{}{} = getelementptr inbounds %State, %State* %state, i32 0, i32 {}", indent, p, idx).ok();
                    let ld = format!("%il{}", self.txn_counter); self.txn_counter += 1;
                    let rng = self.field_to_meta_idx.get(name).map(|m| format!(", !range !{}", m)).unwrap_or_default();
                    writeln!(out, "{}{} = load {}, {}* {}, align {}{}", indent, ld, ty, ty, p, self.align_of(ty), rng).ok();
                    match ty {
                        s if s == "i8" => { let z = format!("%iz{}", self.txn_counter); self.txn_counter += 1; writeln!(out, "{}{} = zext i8 {} to i64", indent, z, ld).ok(); writeln!(out, "{}{} = add i64 0, {}", indent, v, z).ok(); }
                        s if s == "float" => { let i = format!("%if{}", self.txn_counter); self.txn_counter += 1; writeln!(out, "{}{} = bitcast float {} to i32", indent, i, ld).ok(); writeln!(out, "{}{} = zext i32 {} to i64", indent, v, i).ok(); }
                        s if s == "i8*" => { writeln!(out, "{}{} = ptrtoint i8* {} to i64", indent, v, ld).ok(); }
                        _ => { writeln!(out, "{}{} = add i64 0, {}", indent, v, ld).ok(); }
                    }
                }
            }
            Expr::OwnedRef(name) => {
                // Redirect to Identifier — same semantics for LLVM
                return self.emit_expr(out, &Expr::Identifier(name.clone()), indent);
            }
            Expr::PriorState(name) => {
                writeln!(out, "{}{} = add i64 0, 0 ; @{}", indent, v, name).ok();
            }
            // Binary ops
            Expr::Add(l, r) => { self.emit_binop(out, indent, &v, l, r, "add", "fadd"); }
            Expr::Sub(l, r) => { self.emit_binop(out, indent, &v, l, r, "sub", "fsub"); }
            Expr::Mul(l, r) => { self.emit_binop(out, indent, &v, l, r, "mul", "fmul"); }
            Expr::Div(l, r) => { self.emit_binop(out, indent, &v, l, r, "sdiv", "fdiv"); }
            Expr::Mod(l, r) => { let (a, b) = (self.emit_expr(out, l, indent), self.emit_expr(out, r, indent)); writeln!(out, "{}{} = srem i64 {}, {}", indent, v, a, b).ok(); }
            // Comparisons
            Expr::Eq(l, r) => { self.emit_fcmp(out, indent, &v, l, r, "oeq"); }
            Expr::Ne(l, r) => { self.emit_fcmp(out, indent, &v, l, r, "one"); }
            Expr::Lt(l, r) => { self.emit_fcmp(out, indent, &v, l, r, "olt"); }
            Expr::Le(l, r) => { self.emit_fcmp(out, indent, &v, l, r, "ole"); }
            Expr::Gt(l, r) => { self.emit_fcmp(out, indent, &v, l, r, "ogt"); }
            Expr::Ge(l, r) => { self.emit_fcmp(out, indent, &v, l, r, "oge"); }
            // Logical
            Expr::And(l, r) => { let (a, b) = (self.emit_expr(out, l, indent), self.emit_expr(out, r, indent)); writeln!(out, "{}{} = and i64 {}, {}", indent, v, a, b).ok(); }
            Expr::Or(l, r) => { let (a, b) = (self.emit_expr(out, l, indent), self.emit_expr(out, r, indent)); writeln!(out, "{}{} = or i64 {}, {}", indent, v, a, b).ok(); }
            Expr::Not(e) => { let inner = self.emit_expr(out, e, indent); writeln!(out, "{}{} = xor i64 {}, 1", indent, v, inner).ok(); }
            Expr::Neg(e) => {
                let inner = self.emit_expr(out, e, indent);
                if self.is_float_expr(e) {
                    let tr = format!("%ntr{}", self.txn_counter); self.txn_counter += 1;
                    let fl = format!("%nfl{}", self.txn_counter); self.txn_counter += 1;
                    let fs = format!("%nfs{}", self.txn_counter); self.txn_counter += 1;
                    let fi = format!("%nfi{}", self.txn_counter); self.txn_counter += 1;
                    writeln!(out, "{}{} = trunc i64 {} to i32", indent, tr, inner).ok();
                    writeln!(out, "{}{} = bitcast i32 {} to float", indent, fl, tr).ok();
                    writeln!(out, "{}{} = fsub float -0.0, {}", indent, fs, fl).ok();
                    writeln!(out, "{}{} = bitcast float {} to i32", indent, fi, fs).ok();
                    writeln!(out, "{}{} = zext i32 {} to i64", indent, v, fi).ok();
                    self.register_types.insert(v.to_string(), Type::Float);
                } else {
                    writeln!(out, "{}{} = sub i64 0, {}", indent, v, inner).ok();
                }
            }
            // Bitwise
            Expr::BitAnd(l, r) => { let (a, b) = (self.emit_expr(out, l, indent), self.emit_expr(out, r, indent)); writeln!(out, "{}{} = and i64 {}, {}", indent, v, a, b).ok(); }
            Expr::BitOr(l, r) => { let (a, b) = (self.emit_expr(out, l, indent), self.emit_expr(out, r, indent)); writeln!(out, "{}{} = or i64 {}, {}", indent, v, a, b).ok(); }
            Expr::BitXor(l, r) => { let (a, b) = (self.emit_expr(out, l, indent), self.emit_expr(out, r, indent)); writeln!(out, "{}{} = xor i64 {}, {}", indent, v, a, b).ok(); }
            Expr::BitNot(e) => { let inner = self.emit_expr(out, e, indent); writeln!(out, "{}{} = xor i64 {}, -1", indent, v, inner).ok(); }
            Expr::Shl(l, r) => { let (a, b) = (self.emit_expr(out, l, indent), self.emit_expr(out, r, indent)); writeln!(out, "{}{} = shl i64 {}, {}", indent, v, a, b).ok(); }
            Expr::Shr(l, r) => { let (a, b) = (self.emit_expr(out, l, indent), self.emit_expr(out, r, indent)); writeln!(out, "{}{} = lshr i64 {}, {}", indent, v, a, b).ok(); }
            Expr::Concat(l, r) => { let (a, b) = (self.emit_expr(out, l, indent), self.emit_expr(out, r, indent)); writeln!(out, "{}{} = add i64 {}, {} ; concat", indent, v, a, b).ok(); }
            // Call
            Expr::Call(name, args) => {
                // Clone foreign info upfront to avoid borrow conflict with emit_expr
                let frgn_sig: Option<Vec<(String, Type)>> = self.frgn_map.get(name).map(|s| s.inputs.clone());
                if let Some(inputs) = frgn_sig {
                    let mut marshaled: Vec<String> = Vec::new();
                    for (i, (_, arg_ty)) in inputs.iter().enumerate() {
                        if i < args.len() {
                            let raw = self.emit_expr(out, &args[i], indent);
                            match arg_ty {
                                Type::Int | Type::UInt => marshaled.push(format!("i64 {}", raw)),
                                Type::Bool => { let z = format!("%fz{}", self.txn_counter); self.txn_counter += 1; writeln!(out, "{}{} = trunc i64 {} to i32", indent, z, raw).ok(); marshaled.push(format!("i32 {}", z)); }
                                Type::Char => { let z = format!("%fz{}", self.txn_counter); self.txn_counter += 1; writeln!(out, "{}{} = trunc i64 {} to i32", indent, z, raw).ok(); marshaled.push(format!("i32 {}", z)); }
                                Type::Float => {
                                    let tr = format!("%fftr{}", self.txn_counter); self.txn_counter += 1;
                                    let fl = format!("%ffl{}", self.txn_counter); self.txn_counter += 1;
                                    writeln!(out, "{}{} = trunc i64 {} to i32", indent, tr, raw).ok();
                                    writeln!(out, "{}{} = bitcast i32 {} to float", indent, fl, tr).ok();
                                    marshaled.push(format!("float {}", fl));
                                }
                                Type::String | Type::Data => { let p = format!("%fp{}", self.txn_counter); self.txn_counter += 1; writeln!(out, "{}{} = inttoptr i64 {} to i8*", indent, p, raw).ok(); marshaled.push(format!("i8* {}", p)); }
                                _ => marshaled.push(format!("i64 {}", raw)),
                            }
                        }
                    }
                    // Bootstrap intrinsics
                    match name.as_str() {
                        "__print" => {
                            // First arg is already marshaled to i8* by the String/Data path
                            let p = marshaled[0].clone(); // "i8* %fp14"
                            let l = format!("%bl{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = call i64 @strlen({})", indent, l, p).ok();
                            writeln!(out, "{}{} = call i64 @write(i32 1, {}, i64 {})", indent, v, p, l).ok();
                        }
                        "__exit" => { writeln!(out, "{}{} = call void @exit(i32 0)", indent, v).ok(); writeln!(out, "{}{} = add i64 0, 0", indent, v).ok(); }
                        _ => {
                            let args_str = marshaled.join(", ");
                            writeln!(out, "{}{} = call i64 @{}({})", indent, v, name, args_str).ok();
                        }
                    }
                } else {
                    // Internal call — marshal i64 back to real types per definition
                    let def_tys: Option<Vec<Type>> = self.defn_params.get(name).cloned();
                    let mut a_strs = Vec::new();
                    for (ai, arg) in args.iter().enumerate() {
                        let raw = self.emit_expr(out, arg, indent);
                        if let Some(ref tys) = def_tys {
                            if ai < tys.len() {
                                match &tys[ai] {
                                    Type::Bool => {
                                        let tr = format!("%ctr{}", self.txn_counter); self.txn_counter += 1;
                                        writeln!(out, "{}{} = trunc i64 {} to i8", indent, tr, raw).ok();
                                        a_strs.push(format!("i8 {}", tr));
                                    }
                                    Type::String | Type::Data => {
                                        let p = format!("%cip{}", self.txn_counter); self.txn_counter += 1;
                                        writeln!(out, "{}{} = inttoptr i64 {} to i8*", indent, p, raw).ok();
                                        a_strs.push(format!("i8* {}", p));
                                    }
                                    _ => a_strs.push(format!("i64 {}", raw)),
                                }
                            } else {
                                a_strs.push(format!("i64 {}", raw));
                            }
                        } else {
                            a_strs.push(format!("i64 {}", raw));
                        }
                    }
                    if name.starts_with(|c: char| c.is_uppercase()) && !self.program_txns.contains(name) {
                        let n_slots = a_strs.len() + 1;
                        let p = format!("%cop{}", self.txn_counter); self.txn_counter += 1;
                        let disc_val = if name == "None" || name == "Err" { 0u64 } else { 1u64 };
                        writeln!(out, "{}{} = alloca i64, i64 {}", indent, p, n_slots).ok();
                        let disc_gep = format!("%cdg{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 0", indent, disc_gep, p).ok();
                        writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, disc_val, disc_gep).ok();
                        for (ai, arg_reg) in a_strs.iter().enumerate() {
                            let pay_gep = format!("%cpg{}", self.txn_counter); self.txn_counter += 1;
                            let parts: Vec<&str> = arg_reg.splitn(2, ' ').collect();
                            let rn = if parts.len() == 2 { parts[1] } else { arg_reg };
                            writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, pay_gep, p, ai + 1).ok();
                            writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, rn, pay_gep).ok();
                        }
                        writeln!(out, "{}{} = ptrtoint i64* {} to i64", indent, v, p).ok();
                    } else {
                        writeln!(out, "{}{} = call i64 @{}({})", indent, v, name, a_strs.join(", ")).ok();
                    }
                }
            }
            // Lists
            Expr::ListLiteral(elems) => {
                let n = elems.len();
                let p = format!("%llp{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = alloca i64, i64 {}", indent, p, n).ok();
                for (ei, e) in elems.iter().enumerate() {
                    let ev = self.emit_expr(out, e, indent);
                    let ep = format!("%lep{}", self.txn_counter); self.txn_counter += 1;
                    writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, ep, p, ei).ok();
                    writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, ev, ep).ok();
                }
                writeln!(out, "{}{} = ptrtoint i64* {} to i64", indent, v, p).ok();
            }
            Expr::ListIndex(list, idx) => {
                let l = self.emit_expr(out, list, indent);
                let i = self.emit_expr(out, idx, indent);
                let p = format!("%lip{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, p, l).ok();
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, v, p, i).ok();
            }
            Expr::ListLen(_) => { writeln!(out, "{}{} = add i64 0, 0", indent, v).ok(); }
            Expr::Slice { value, start, .. } => {
                let l = self.emit_expr(out, value, indent);
                let p = format!("%slp{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, p, l).ok();
                if let Some(s) = start {
                    let sv = self.emit_expr(out, s, indent);
                    writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, v, p, sv).ok();
                } else { writeln!(out, "{}{} = add i64 0, {} ; slice", indent, v, l).ok(); }
            }
            Expr::MultiSlice { value, .. } => {
                let l = self.emit_expr(out, value, indent);
                writeln!(out, "{}{} = add i64 0, {} ; multi-slice", indent, v, l).ok();
            }
            // Containers
            Expr::Tuple(elems) => { for e in elems { let _ = self.emit_expr(out, e, indent); } writeln!(out, "{}{} = add i64 0, 0 ; tuple", indent, v).ok(); }
            Expr::TupleDestructure(_, expr) => { let inner = self.emit_expr(out, expr, indent); writeln!(out, "{}{} = add i64 0, {} ; destructure", indent, v, inner).ok(); }
            Expr::StructInstance(_, fields) => { for (_, e) in fields { let _ = self.emit_expr(out, e, indent); } writeln!(out, "{}{} = add i64 0, 0 ; struct", indent, v).ok(); }
            Expr::ObjectLiteral(fields) => { for (_, e) in fields { let _ = self.emit_expr(out, e, indent); } writeln!(out, "{}{} = add i64 0, 0 ; object", indent, v).ok(); }
            Expr::FieldAccess(obj, f) => { let o = self.emit_expr(out, obj, indent); writeln!(out, "{}{} = add i64 0, {} ; field", indent, v, o).ok(); }
            // Cast
            Expr::Cast(inner, target_ty) => {
                let src_reg = self.emit_expr(out, inner, indent);
                let src_ty = self.resolve_source_type(inner, &src_reg);
                self.emit_cast_convert(out, indent, &v, &src_reg, src_ty, target_ty);
                self.register_types.insert(v.clone(), target_ty.clone());
            }
            // Block
            Expr::Block(stmts, last) => {
                for s in stmts { self.emit_stmt(out, s, indent); }
                let r = self.emit_expr(out, last, indent);
                writeln!(out, "{}{} = add i64 0, {}", indent, v, r).ok();
            }
            // Match
            Expr::Match { value, arms } => {
                let inner = self.emit_expr(out, value, indent);
                let disc = format!("%md{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = and i64 {}, 255", indent, disc, inner).ok();
                let merge = format!("mm{}", self.txn_counter); self.txn_counter += 1;
                let has_wc = arms.iter().any(|a| a.pattern == MatchPattern::Wildcard);
                let def_l = if has_wc { format!("mdf{}", self.txn_counter) } else { format!("mur{}", self.txn_counter) };
                self.txn_counter += 1;
                let mid = self.txn_counter;
                self.txn_counter += 1;
                writeln!(out, "{}switch i64 {}, label %{} [", indent, disc, def_l).ok();
                let mut vi = 0u64;
                for arm in arms { if let MatchPattern::Variant { .. } = &arm.pattern { writeln!(out, "{}  i64 {}, label %ma{}_{}", indent, vi, mid, vi).ok(); vi += 1; } }
                writeln!(out, "{}]", indent).ok();
                let mut phi_v: Vec<String> = Vec::new();
                let mut phi_l: Vec<String> = Vec::new();
                vi = 0;
                for arm in arms {
                    if let MatchPattern::Variant { .. } = &arm.pattern {
                        writeln!(out, "{}ma{}_{}:", indent, mid, vi).ok();
                        let av = self.emit_expr(out, &arm.body, indent);
                        phi_v.push(av); phi_l.push(format!("%%ma{}_{}", mid, vi));
                        writeln!(out, "{}br label %{}", indent, merge).ok();
                        vi += 1;
                    }
                }
                if has_wc {
                    if let Some(wc) = arms.iter().find(|a| a.pattern == MatchPattern::Wildcard) {
                        writeln!(out, "{}:", def_l).ok();
                        let wv = self.emit_expr(out, &wc.body, indent);
                        phi_v.push(wv); phi_l.push(format!("%%{}", def_l));
                        writeln!(out, "{}br label %{}", indent, merge).ok();
                    }
                } else {
                    writeln!(out, "{}:", def_l).ok();
                    writeln!(out, "{}  unreachable", indent).ok();
                }
                writeln!(out, "{}:", merge).ok();
                if phi_v.len() == 1 { writeln!(out, "{}{} = add i64 0, {}", indent, v, phi_v[0]).ok(); }
                else {
                    let ps: Vec<String> = phi_v.iter().enumerate().map(|(i, r)| format!("[i64 {}, {}]", r, phi_l[i])).collect();
                    writeln!(out, "{}{} = phi i64 {}", indent, v, ps.join(", ")).ok();
                }
            }
            // PatternMatch guard
            Expr::PatternMatch { value, variant, .. } => {
                let inner = self.emit_expr(out, value, indent);
                let disc = format!("%pd{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = and i64 {}, 255", indent, disc, inner).ok();
                let target = if variant == "None" || variant == "Err" { 0u64 } else { 1u64 };
                let cmp = format!("%pc{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = icmp eq i64 {}, {}", indent, cmp, disc, target).ok();
                writeln!(out, "{}{} = zext i1 {} to i64", indent, v, cmp).ok();
            }
            // Quantifiers
            Expr::ForAll { .. } => { writeln!(out, "{}{} = add i64 0, 1 ; forall", indent, v).ok(); }
            Expr::Exists { expr, .. } => {
                let inner = self.emit_expr(out, expr, indent);
                let cmp = format!("%ec{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = icmp ne i64 {}, 0", indent, cmp, inner).ok();
                writeln!(out, "{}{} = zext i1 {} to i64", indent, v, cmp).ok();
            }
            // Fallback
            _ => { writeln!(out, "{}{} = add i64 0, 0 ; expr", indent, v).ok(); }
        }
        // Universal float type-propagation catch-all
        if self.is_float_expr(expr) {
            self.register_types.insert(v.to_string(), Type::Float);
        }
        v
    }

    // ── RANGE EXTRACTION ──────────────────────────────────────
    fn extract_ranges(pre: &Expr) -> HashMap<String, (i64, i64)> {
        let mut r = HashMap::new();
        Self::extract_ranges_inner(pre, &mut r);
        r
    }
    fn extract_ranges_inner(expr: &Expr, r: &mut HashMap<String, (i64, i64)>) {
        match expr {
            Expr::And(l, rgt) => { Self::extract_ranges_inner(l, r); Self::extract_ranges_inner(rgt, r); }
            Expr::Lt(l, rgt) => { if let Expr::Identifier(n) = l.as_ref() { if let Expr::Integer(v) = rgt.as_ref() { let e = r.entry(n.clone()).or_insert((i64::MIN, i64::MAX)); if *v < e.1 { e.1 = *v; } } } }
            Expr::Ge(l, rgt) => { if let Expr::Identifier(n) = l.as_ref() { if let Expr::Integer(v) = rgt.as_ref() { let e = r.entry(n.clone()).or_insert((i64::MIN, i64::MAX)); if *v > e.0 { e.0 = *v; } } } }
            Expr::Gt(l, rgt) => { if let Expr::Identifier(n) = l.as_ref() { if let Expr::Integer(v) = rgt.as_ref() { let e = r.entry(n.clone()).or_insert((i64::MIN, i64::MAX)); if v + 1 > e.0 { e.0 = v + 1; } } } }
            _ => {}
        }
    }

    fn resolve_dispatch_first_txn(&self, name: &str) -> String {
        self.fused_to_first.get(name).cloned().unwrap_or_else(|| name.to_string())
    }

    fn dispatch_has_pre(&self, txns: &[(String, &crate::ast::Transaction)], name: &str) -> bool {
        let first = self.resolve_dispatch_first_txn(name);
        txns.iter().find(|(n, _)| n == &first).map(|(_, t)| !matches!(t.contract.pre_condition, Expr::Bool(true))).unwrap_or(false)
    }

    // ── REACTOR LOOP ──────────────────────────────────────────
    fn emit_reactor(&mut self, out: &mut String, txns: &[(String, &crate::ast::Transaction)], fusable: &[(String, String)]) {
        self.fused_to_first.clear();
        for (a, b) in fusable {
            let fn_ = format!("{}_{}_fused", a, b);
            self.fused_to_first.insert(fn_, a.clone());
        }
        let mut used_fused: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut dispatch: Vec<String> = Vec::new();
        let mut fused_txns: std::collections::HashSet<String> = std::collections::HashSet::new();
        for (a, b) in fusable {
            let fn_ = format!("{}_{}_fused", a, b);
            if used_fused.contains(&fn_) { continue; }
            used_fused.insert(fn_.clone());
            fused_txns.insert(a.clone()); fused_txns.insert(b.clone());
            dispatch.push(fn_);
        }
        for (n, _) in txns { if !fused_txns.contains(n) { dispatch.push(n.clone()); } }

        writeln!(out, "define void @reactor_tick() local_unnamed_addr #2 {{").ok();
        writeln!(out, "  entry:").ok();
        // Trigger sampling — load volatile into named registers
        self.sampled_triggers.clear();
        let trigger_snapshot: Vec<(String, crate::ast::TriggerDeclaration)> = self.trigger_names
            .iter()
            .filter_map(|tn| self.triggers.get(tn).map(|t| (tn.clone(), t.clone())))
            .collect();
        for (tn, t) in &trigger_snapshot {
            let sz = format!("%sz_{}", tn);
            let (addr_str, addr_is_ptr) = match &t.address {
                crate::ast::LinkRef::Explicit(a) => (a.to_string(), false),
                crate::ast::LinkRef::Linked(s) => (format!("@{}", s), true),
            };
            self.emit_trg_load(out, "  ", &sz, &addr_str, addr_is_ptr, &t.ty);
            self.sampled_triggers.insert(tn.clone(), sz);
        }

        if dispatch.is_empty() {
            writeln!(out, "  ret void").ok();
        } else {
            // First dispatch branch
            let first = &dispatch[0];
            let has_pre = self.dispatch_has_pre(txns, first);
            let check0 = format!("ck0");
            if has_pre {
                let first_txn = self.resolve_dispatch_first_txn(first);
                writeln!(out, "  %pr0 = call i1 @pre_{}(%State* @global_state)", first_txn).ok();
                writeln!(out, "  br i1 %pr0, label %b0, label %{}", check0).ok();
            } else {
                writeln!(out, "  br i1 true, label %b0, label %{}", check0).ok();
            }

            for (i, txn_name) in dispatch.iter().enumerate() {
                let b = format!("b{}", i);
                let c = format!("ck{}", i);
                writeln!(out, "{}:", b).ok();
                writeln!(out, "  call void @{}(%State* @global_state)", txn_name).ok();
                // Fall through to this transaction's check label, which evaluates
                // the NEXT transaction's precondition. Matches the interpreter model
                // where all dirty transactions are evaluated sequentially in one tick.
                // NOTE: this br is dead LLVM IR when the body ends in `term` (always true).
                //       LLVM -O3 eliminates it. We emit it for correct uni-cyclic IR.
                writeln!(out, "  br label %{}", c).ok();

                if i + 1 < dispatch.len() {
                    let next = &dispatch[i + 1];
                    writeln!(out, "{}:", c).ok();
                    let has_next_pre = self.dispatch_has_pre(txns, next);
                    let next_check = format!("ck{}", i + 1);
                    if has_next_pre {
                        let next_txn = self.resolve_dispatch_first_txn(next);
                        writeln!(out, "  %pr{} = call i1 @pre_{}(%State* @global_state)", i + 1, next_txn).ok();
                        writeln!(out, "  br i1 %pr{}, label %b{}, label %{}", i + 1, i + 1, next_check).ok();
                    } else {
                        writeln!(out, "  br i1 true, label %b{}, label %{}", i + 1, next_check).ok();
                    }
                }
            }
            let last_check = format!("ck{}", dispatch.len() - 1);
            writeln!(out, "{}:", last_check).ok();
            writeln!(out, "  ret void").ok();
        }
        writeln!(out, "}}").ok();
        writeln!(out).ok();
    }

    // ── WRITE MASKS (Parallel Dispatch) ──────────────────────
    fn build_write_masks(&mut self, program: &Program) {
        self.txn_write_masks.clear();
        for item in &program.items {
            if let TopLevel::Transaction(t) = item {
                let writes = crate::backend::collect_assigned_identifiers(&t.body);
                let mut mask = 0u64;
                for w in &writes {
                    if let Some(&idx) = self.field_index_map.get(w.as_str()) {
                        if idx < 64 { mask |= 1u64 << idx; }
                    }
                }
                self.txn_write_masks.insert(t.name.clone(), mask);
            }
        }
    }

    // ── PARALLEL DISPATCH REACTOR ────────────────────────────
    /// Fires multiple non-conflicting transactions per tick.
    /// Phase 1: evaluate ALL preconditions upfront into %pr0..%prN.
    /// Phase 2: fire each transaction if its precondition is true AND
    ///          its write mask doesn't overlap with the fired_mask.
    fn emit_parallel_reactor(&mut self, out: &mut String, txns: &[(String, &crate::ast::Transaction)],
                             fusable: &[(String, String)]) {
        self.fused_to_first.clear();
        for (a, b) in fusable {
            let fn_ = format!("{}_{}_fused", a, b);
            self.fused_to_first.insert(fn_, a.clone());
        }
        let mut used_fused: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut dispatch: Vec<String> = Vec::new();
        let mut fused_txns: std::collections::HashSet<String> = std::collections::HashSet::new();
        for (a, b) in fusable {
            let fn_ = format!("{}_{}_fused", a, b);
            if used_fused.contains(&fn_) { continue; }
            used_fused.insert(fn_.clone());
            fused_txns.insert(a.clone()); fused_txns.insert(b.clone());
            dispatch.push(fn_);
        }
        for (n, _) in txns { if !fused_txns.contains(n) { dispatch.push(n.clone()); } }

        writeln!(out, "define void @reactor_tick() local_unnamed_addr #2 {{").ok();
        writeln!(out, "  entry:").ok();
        // Trigger sampling
        self.sampled_triggers.clear();
        let trigger_snapshot: Vec<(String, crate::ast::TriggerDeclaration)> = self.trigger_names
            .iter()
            .filter_map(|tn| self.triggers.get(tn).map(|t| (tn.clone(), t.clone())))
            .collect();
        for (tn, t) in &trigger_snapshot {
            let sz = format!("%sz_{}", tn);
            let (addr_str, addr_is_ptr) = match &t.address {
                crate::ast::LinkRef::Explicit(a) => (a.to_string(), false),
                crate::ast::LinkRef::Linked(s) => (format!("@{}", s), true),
            };
            self.emit_trg_load(out, "  ", &sz, &addr_str, addr_is_ptr, &t.ty);
            self.sampled_triggers.insert(tn.clone(), sz);
        }

        // fired_mask: tracks which fields have been written by fired txns
        writeln!(out, "  %fired_mask = alloca i64, align 8").ok();
        writeln!(out, "  store i64 0, i64* %fired_mask").ok();

        if dispatch.is_empty() {
            writeln!(out, "  ret void").ok();
        } else {
            let n = dispatch.len();
            // Phase 1: evaluate all preconditions upfront
            for (i, txn_name) in dispatch.iter().enumerate() {
                let has_pre = self.dispatch_has_pre(txns, txn_name);
                if has_pre {
                    let first_txn = self.resolve_dispatch_first_txn(txn_name);
                    writeln!(out, "  %pr{} = call i1 @pre_{}(%State* @global_state)", i, first_txn).ok();
                } else {
                    writeln!(out, "  %pr{} = add i1 0, 1", i).ok();
                }
            }

            // Phase 2: dispatch chain — all preconds known, fire if true + no conflict
            for i in 0..n {
                let txn_name = &dispatch[i];
                let b = format!("b{}", i);
                let next_c = format!("ck{}", i + 1);

                if i == 0 {
                    // First txn: from entry, no conflict check needed
                    writeln!(out, "  br i1 %pr0, label %b0, label %ck1").ok();
                } else {
                    let c = format!("ck{}", i);
                    writeln!(out, "{}:", c).ok();
                    let wm = self.txn_write_masks.get(txn_name).copied().unwrap_or(0);
                    if wm == 0 {
                        // No writes → never conflicts with anything
                        writeln!(out, "  br i1 %pr{}, label %{}, label %{}", i, b, next_c).ok();
                    } else {
                        let fm = format!("%fm{}", i);
                        let ca = format!("%ca{}", i);
                        let nc = format!("%nc{}", i);
                        writeln!(out, "  {} = load i64, i64* %fired_mask", fm).ok();
                        writeln!(out, "  {} = and i64 {}, {}", ca, fm, wm).ok();
                        writeln!(out, "  {} = icmp eq i64 {}, 0", nc, ca).ok();
                        writeln!(out, "  %can{} = and i1 %pr{}, {}", i, i, nc).ok();
                        writeln!(out, "  br i1 %can{}, label %{}, label %{}", i, b, next_c).ok();
                    }
                }
            }

            // Body blocks + fired_mask updates
            for i in 0..n {
                let txn_name = &dispatch[i];
                let b = format!("b{}", i);
                let next_c = format!("ck{}", i + 1);
                let wm = self.txn_write_masks.get(txn_name).copied().unwrap_or(0);
                writeln!(out, "{}:", b).ok();
                writeln!(out, "  call void @{}(%State* @global_state)", txn_name).ok();
                if wm != 0 {
                    let fm = format!("%fm{}a", i);
                    let fmu = format!("%fm{}b", i);
                    writeln!(out, "  {} = load i64, i64* %fired_mask", fm).ok();
                    writeln!(out, "  {} = or i64 {}, {}", fmu, fm, wm).ok();
                    writeln!(out, "  store i64 {}, i64* %fired_mask", fmu).ok();
                }
                writeln!(out, "  br label %{}", next_c).ok();
            }

            // Last check label → ret void
            writeln!(out, "ck{}:", n).ok();
            writeln!(out, "  ret void").ok();
        }
        writeln!(out, "}}").ok();
        writeln!(out).ok();
    }

    // ── MAIN FUNCTION ─────────────────────────────────────────
    fn emit_main(&self, out: &mut String, has_wake_triggers: bool) {
        writeln!(out, "define i32 @main() local_unnamed_addr #3 {{").ok();
        writeln!(out, "  entry:").ok();
        writeln!(out, "  call void @init_state()").ok();
        if has_wake_triggers {
            writeln!(out, "  call void @__rt_init()").ok();
        }
        writeln!(out, "  br label %tick").ok();
        writeln!(out, "  tick:").ok();
        writeln!(out, "  call void @reactor_tick()").ok();
        if has_wake_triggers {
            writeln!(out, "  call void @__rt_wait()").ok();
        }
        writeln!(out, "  br label %tick").ok();
        writeln!(out, "}}").ok();
        writeln!(out).ok();
    }

    fn emit_folded_main(&self, out: &mut String, txn_name: &str, counter_idx: usize, total_idx: usize) {
        writeln!(out, "define i32 @main() local_unnamed_addr #0 {{").ok();
        writeln!(out, "  entry:").ok();
        writeln!(out, "  call void @init_state()").ok();
        let tp = format!("%ft0"); let c0 = self.txn_counter;
        writeln!(out, "  %gt{} = getelementptr inbounds %State, %State* @global_state, i32 0, i32 {}", c0, total_idx).ok();
        writeln!(out, "  %lt{} = load i64, i64* %gt{}, align 8", c0, c0).ok();
        writeln!(out, "  br label %hdr").ok();
        writeln!(out, "hdr:").ok();
        writeln!(out, "  %gp{} = getelementptr inbounds %State, %State* @global_state, i32 0, i32 {}", c0 + 1, counter_idx).ok();
        writeln!(out, "  %lp{} = load i64, i64* %gp{}, align 8", c0 + 1, c0 + 1).ok();
        writeln!(out, "  %cp{} = icmp slt i64 %lp{}, %lt{}", c0 + 2, c0 + 1, c0).ok();
        writeln!(out, "  br i1 %cp{}, label %body, label %done", c0 + 2).ok();
        writeln!(out, "body:").ok();
        writeln!(out, "  call void @{}(%State* @global_state)", txn_name).ok();
        writeln!(out, "  br label %hdr").ok();
        writeln!(out, "done:").ok();
        writeln!(out, "  ret i32 0").ok();
        writeln!(out, "}}").ok();
        writeln!(out).ok();
    }

    fn emit_folded_pure_counter(&self, out: &mut String, counter_idx: usize, total_value: i64) {
        writeln!(out, "define i32 @main() local_unnamed_addr #0 {{").ok();
        writeln!(out, "  entry:").ok();
        writeln!(out, "  call void @init_state()").ok();
        writeln!(out, "  %gp = getelementptr inbounds %State, %State* @global_state, i32 0, i32 {}", counter_idx).ok();
        writeln!(out, "  store i64 {}, i64* %gp, align 8", total_value).ok();
        writeln!(out, "  ret i32 0").ok();
        writeln!(out, "}}").ok();
        writeln!(out).ok();
    }

    // ── WAKE TRIGGER METADATA ─────────────────────────────────
    fn emit_wake_metadata(&self, out: &mut String) {
        let wake_symbols: Vec<&str> = self.triggers.values()
            .filter(|t| t.is_wake)
            .filter_map(|t| match &t.address {
                crate::ast::LinkRef::Linked(s) => Some(s.as_str()),
                _ => None,
            })
            .collect();
        if wake_symbols.is_empty() { return; }
        let count = wake_symbols.len();
        let sym_list = wake_symbols.iter().map(|s| format!("i8* @{}", s)).collect::<Vec<_>>().join(", ");
        writeln!(out, "@llvm.wake_triggers = appending global [{} x i8*] [{}]", count, sym_list).ok();
        writeln!(out, "!llvm.wake_triggers = !{{!0}}").ok();
        write!(out, "!0 = !{{").ok();
        for (i, sym) in wake_symbols.iter().enumerate() {
            if i > 0 { write!(out, ", ").ok(); }
            write!(out, "!\"{}\"", sym).ok();
        }
        writeln!(out, "}}").ok();
    }

    // ── FUSABLE PAIRS ────────────────────────────────────────
    fn resolve_fusable_pairs(&self, txns: &[(String, &crate::ast::Transaction)]) -> Vec<(String, String)> {
        let prg = crate::ast::Program {
            items: txns.iter().map(|(_, t)| crate::ast::TopLevel::Transaction((*t).clone())).collect(),
            comments: vec![], reactor_speed: None, attrs: vec![], ffi: None, strict_mode: crate::ast::StrictMode::Off, dispatch_mode: crate::ast::DispatchMode::Sequential,
        };
        let mut pairs = crate::backend::detect_fusable_pairs(&prg);
        pairs.retain(|(a, b)| {
            if let (Some((_, ta)), Some((_, tb))) = (txns.iter().find(|(n, _)| n == a), txns.iter().find(|(n, _)| n == b)) {
                if ta.is_async || tb.is_async { return false; }
                let aw = crate::backend::collect_assigned_identifiers(&ta.body);
                let bw = crate::backend::collect_assigned_identifiers(&tb.body);
                if aw.iter().any(|w| bw.contains(w)) { return false; }
                if self.trg_in_pre(&tb.contract.pre_condition) { return false; }
                true
            } else { false }
        });
        pairs
    }
        fn trg_in_pre(&self, pre: &Expr) -> bool {
        let mut ids = std::collections::HashSet::new();
        crate::backend::collect_expr_identifiers(pre, &mut ids);
        ids.iter().any(|id| self.trigger_names.contains(id))
    }

    fn resolve_source_type(&self, expr: &Expr, reg: &str) -> Option<Type> {
        if let Some(ty) = self.register_types.get(reg) {
            return Some(ty.clone());
        }
        match expr {
            Expr::Integer(_) => Some(Type::Int),
            Expr::Float(_) => Some(Type::Float),
            Expr::Bool(_) => Some(Type::Bool),
            Expr::String(_) => Some(Type::String),
            Expr::Char(_) => Some(Type::Char),
            Expr::Identifier(name) | Expr::OwnedRef(name) => {
                if let Some(&idx) = self.field_index_map.get(name.as_str()) {
                    let ll_ty = &self.field_types[idx];
                    Some(match ll_ty.as_str() {
                        "i8" => Type::Bool,
                        "float" => Type::Float,
                        "i32" => Type::Char,
                        "i8*" => Type::String,
                        _ => Type::Int,
                    })
                } else if let Some(let_reg) = self.let_bindings.get(name.as_str()) {
                    self.register_types.get(let_reg).cloned()
                } else {
                    None
                }
            }
            Expr::Cast(_, ty) => Some(ty.clone()),
            Expr::Block(_, last) => self.resolve_source_type(last, reg),
            _ => {
                if self.is_float_expr(expr) { Some(Type::Float) }
                else { Some(Type::Int) }
            }
        }
    }

    fn emit_cast_convert(&mut self, out: &mut String, indent: &str, dst: &str, src: &str, src_ty: Option<Type>, target: &Type) {
        let src_ty = match src_ty {
            Some(t) => t,
            None => {
                let _ = writeln!(out, "{}{} = add i64 0, {}", indent, dst, src);
                return;
            }
        };
        if &src_ty == target {
            let _ = writeln!(out, "{}{} = add i64 0, {}", indent, dst, src);
            return;
        }
        match (&src_ty, target) {
            (Type::Int | Type::UInt, Type::Float) => {
                let si = format!("%csf{}", self.txn_counter); self.txn_counter += 1;
                let fi = format!("%cfi{}", self.txn_counter); self.txn_counter += 1;
                let _ = writeln!(out, "{}{} = sitofp i64 {} to float", indent, si, src);
                let _ = writeln!(out, "{}{} = bitcast float {} to i32", indent, fi, si);
                let _ = writeln!(out, "{}{} = zext i32 {} to i64", indent, dst, fi);
            }
            (Type::Float, Type::Int | Type::UInt) => {
                let tr = format!("%ctr{}", self.txn_counter); self.txn_counter += 1;
                let fl = format!("%cfl{}", self.txn_counter); self.txn_counter += 1;
                let _ = writeln!(out, "{}{} = trunc i64 {} to i32", indent, tr, src);
                let _ = writeln!(out, "{}{} = bitcast i32 {} to float", indent, fl, tr);
                let _ = writeln!(out, "{}{} = fptosi float {} to i64", indent, dst, fl);
            }
            (Type::Int | Type::UInt, Type::Bool) => {
                let ci = format!("%ccb{}", self.txn_counter); self.txn_counter += 1;
                let _ = writeln!(out, "{}{} = icmp ne i64 {}, 0", indent, ci, src);
                let _ = writeln!(out, "{}{} = zext i1 {} to i64", indent, dst, ci);
            }
            (Type::Bool, Type::Int | Type::UInt) => {
                let _ = writeln!(out, "{}{} = add i64 0, {}", indent, dst, src);
            }
            (Type::Float, Type::Bool) => {
                let tr = format!("%cfbtr{}", self.txn_counter); self.txn_counter += 1;
                let fl = format!("%cfbfl{}", self.txn_counter); self.txn_counter += 1;
                let ci = format!("%cfbci{}", self.txn_counter); self.txn_counter += 1;
                let _ = writeln!(out, "{}{} = trunc i64 {} to i32", indent, tr, src);
                let _ = writeln!(out, "{}{} = bitcast i32 {} to float", indent, fl, tr);
                let _ = writeln!(out, "{}{} = fcmp une float {}, 0.0", indent, ci, fl);
                let _ = writeln!(out, "{}{} = zext i1 {} to i64", indent, dst, ci);
            }
            (Type::Bool, Type::Float) => {
                let ci = format!("%cbfci{}", self.txn_counter); self.txn_counter += 1;
                let fl = format!("%cbffl{}", self.txn_counter); self.txn_counter += 1;
                let fi = format!("%cbffi{}", self.txn_counter); self.txn_counter += 1;
                let _ = writeln!(out, "{}{} = icmp ne i64 {}, 0", indent, ci, src);
                let _ = writeln!(out, "{}{} = select i1 {}, float 1.000000e+00, float 0.000000e+00", indent, fl, ci);
                let _ = writeln!(out, "{}{} = bitcast float {} to i32", indent, fi, fl);
                let _ = writeln!(out, "{}{} = zext i32 {} to i64", indent, dst, fi);
            }
            _ => {
                let _ = writeln!(out, "{}{} = add i64 0, {}", indent, dst, src);
            }
        }
    }

    fn is_float_expr(&self, expr: &Expr) -> bool {
        match expr {
            Expr::Float(_) => true,
            Expr::Identifier(name) => {
                if let Some(reg) = self.let_bindings.get(name) {
                    self.register_types.get(reg) == Some(&Type::Float)
                } else if let Some(&idx) = self.field_index_map.get(name) {
                    self.field_types[idx] == "float"
                } else {
                    false
                }
            }
            Expr::OwnedRef(name) => {
                if let Some(reg) = self.let_bindings.get(name.as_str()) {
                    self.register_types.get(reg) == Some(&Type::Float)
                } else if let Some(&idx) = self.field_index_map.get(name.as_str()) {
                    self.field_types[idx] == "float"
                } else {
                    false
                }
            }
            Expr::Add(l, r) | Expr::Sub(l, r) | Expr::Mul(l, r) | Expr::Div(l, r) => {
                self.is_float_expr(l) || self.is_float_expr(r)
            }
            Expr::Neg(e) => self.is_float_expr(e),
            Expr::Cast(_, ty) => ty == &Type::Float,
            Expr::Block(_, last) => self.is_float_expr(last),
            _ => false,
        }
    }

    fn emit_binop(&mut self, out: &mut String, indent: &str, v: &str, l: &Expr, r: &Expr, int_op: &str, float_op: &str) {
        let (a, b) = (self.emit_expr(out, l, indent), self.emit_expr(out, r, indent));
        if self.is_float_expr(l) || self.is_float_expr(r) {
            let fa = format!("%bfa{}", self.txn_counter); self.txn_counter += 1;
            let fb = format!("%bfb{}", self.txn_counter); self.txn_counter += 1;
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, fa, a).ok();
            writeln!(out, "{}{} = bitcast i32 {} to float", indent, fb, fa).ok();
            let fc = format!("%bfc{}", self.txn_counter); self.txn_counter += 1;
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, fc, b).ok();
            let fd = format!("%bfd{}", self.txn_counter); self.txn_counter += 1;
            writeln!(out, "{}{} = bitcast i32 {} to float", indent, fd, fc).ok();
            let fr = format!("%bfr{}", self.txn_counter); self.txn_counter += 1;
            writeln!(out, "{}{} = {} float {}, {}", indent, fr, float_op, fb, fd).ok();
            let fi = format!("%bfi{}", self.txn_counter); self.txn_counter += 1;
            writeln!(out, "{}{} = bitcast float {} to i32", indent, fi, fr).ok();
            writeln!(out, "{}{} = zext i32 {} to i64", indent, v, fi).ok();
            self.register_types.insert(v.to_string(), Type::Float);
        } else {
            writeln!(out, "{}{} = {} i64 {}, {}", indent, v, int_op, a, b).ok();
        }
    }

    fn emit_fcmp(&mut self, out: &mut String, indent: &str, v: &str, l: &Expr, r: &Expr, cond: &str) {
        let (a, b) = (self.emit_expr(out, l, indent), self.emit_expr(out, r, indent));
        let c = format!("%c{}", self.txn_counter); self.txn_counter += 1;
        if self.is_float_expr(l) || self.is_float_expr(r) {
            let fa = format!("%cfa{}", self.txn_counter); self.txn_counter += 1;
            let fb = format!("%cfb{}", self.txn_counter); self.txn_counter += 1;
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, fa, a).ok();
            writeln!(out, "{}{} = bitcast i32 {} to float", indent, fb, fa).ok();
            let fc = format!("%cfc{}", self.txn_counter); self.txn_counter += 1;
            writeln!(out, "{}{} = trunc i64 {} to i32", indent, fc, b).ok();
            let fd = format!("%cfd{}", self.txn_counter); self.txn_counter += 1;
            writeln!(out, "{}{} = bitcast i32 {} to float", indent, fd, fc).ok();
            writeln!(out, "{}{} = fcmp {} float {}, {}", indent, c, cond, fb, fd).ok();
        } else {
            let icmp_cond = match cond {
                "oeq" => "eq",
                "one" => "ne",
                "olt" => "slt",
                "ole" => "sle",
                "ogt" => "sgt",
                "oge" => "sge",
                _ => cond,
            };
            writeln!(out, "{}{} = icmp {} i64 {}, {}", indent, c, icmp_cond, a, b).ok();
        }
        writeln!(out, "{}{} = zext i1 {} to i64", indent, v, c).ok();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::*;

    fn empty_program() -> Program {
        Program {
            items: vec![],
            comments: vec![],
            reactor_speed: None,
            attrs: Vec::new(),
            ffi: None,
            strict_mode: StrictMode::Off,
            dispatch_mode: Default::default(),
        }
    }

    #[test]
    fn test_llvm_generates_module() {
        let mut backend = LlvmBackend::new();
        let output = backend.generate(&empty_program());
        assert!(output.contains("ModuleID"));
        assert!(output.contains("target triple"));
    }

    #[test]
    fn test_llvm_generates_state_type() {
        let mut backend = LlvmBackend::new();
        let program = Program {
            items: vec![
                TopLevel::StateDecl(StateDecl {
                    name: "counter".to_string(),
                    ty: Type::Int,
                    expr: Some(Expr::Integer(0)),
                    address: None,
                    bit_range: None,
                    is_override: false,
                    os_mode: false,
                    span: None,
                    attrs: vec![],
                }),
            ],
            comments: vec![],
            reactor_speed: None,
            attrs: Vec::new(),
            ffi: None,
            strict_mode: StrictMode::Off,
            dispatch_mode: Default::default(),
        };
        let output = backend.generate(&program);
        assert!(output.contains("%State"));
        assert!(output.contains("i64"));
        assert!(output.contains("global_state"));
    }

    #[test]
    fn test_llvm_generates_transaction() {
        let mut backend = LlvmBackend::new();
        let program = Program {
            items: vec![
                TopLevel::StateDecl(StateDecl {
                    name: "count".to_string(),
                    ty: Type::Int,
                    expr: Some(Expr::Integer(0)),
                    address: None,
                    bit_range: None,
                    is_override: false,
                    os_mode: false,
                    span: None,
                    attrs: vec![],
                }),
                TopLevel::Transaction(Transaction {
                    name: "increment".to_string(),
                    parameters: vec![],
                    contract: Contract {
                        pre_condition: Expr::Bool(true),
                        post_condition: Expr::Bool(true),
                        span: None,
                        watchdog: None,
                    },
                    body: vec![
                        Statement::Assignment {
                            lhs: Expr::Identifier("count".to_string()),
                            expr: Expr::Add(
                                Box::new(Expr::Identifier("count".to_string())),
                                Box::new(Expr::Integer(1)),
                            ),
                            timeout: None,
                            modifiers: vec![],
                        },
                        Statement::Term { values: vec![], modifiers: vec![] },
                    ],
                    is_async: false,
                    is_reactive: true,
                    reactor_speed: None,
                    span: None,
                    is_lambda: false,
                    dependencies: vec![],
                    attrs: vec![],
                    modifiers: vec![],
                    variant_bodies: vec![],
                }),
            ],
            comments: vec![],
            reactor_speed: None,
            attrs: Vec::new(),
            ffi: None,
            strict_mode: StrictMode::Off,
            dispatch_mode: Default::default(),
        };
        let output = backend.generate(&program);
        assert!(output.contains("@increment("));
    }

    #[test]
    fn test_llvm_has_noalias() {
        let mut backend = LlvmBackend::new();
        let program = Program {
            items: vec![
                TopLevel::StateDecl(StateDecl {
                    name: "count".to_string(),
                    ty: Type::Int,
                    expr: Some(Expr::Integer(0)),
                    address: None,
                    bit_range: None,
                    is_override: false,
                    os_mode: false,
                    span: None,
                    attrs: vec![],
                }),
                TopLevel::Transaction(Transaction {
                    name: "increment".to_string(),
                    parameters: vec![],
                    contract: Contract {
                        pre_condition: Expr::Bool(true),
                        post_condition: Expr::Bool(true),
                        span: None,
                        watchdog: None,
                    },
                    body: vec![Statement::Term { values: vec![], modifiers: vec![] }],
                    is_async: false,
                    is_reactive: true,
                    reactor_speed: None,
                    span: None,
                    is_lambda: false,
                    dependencies: vec![],
                    attrs: vec![],
                    modifiers: vec![],
                    variant_bodies: vec![],
                }),
            ],
            comments: vec![],
            reactor_speed: None,
            attrs: Vec::new(),
            ffi: None,
            strict_mode: StrictMode::Off,
            dispatch_mode: Default::default(),
        };
        let output = backend.generate(&program);
        assert!(output.contains("noalias"), "Transaction should have noalias");
        assert!(output.contains("nocapture"), "Transaction should have nocapture");
        assert!(output.contains("local_unnamed_addr"), "Should have local_unnamed_addr");
        assert!(output.contains("attributes #0"), "Should have attribute block");
        assert!(output.contains("mustprogress"), "Should have mustprogress");
        assert!(output.contains("llvm.assume"), "Should declare llvm.assume intrinsic");
    }

    #[test]
    fn test_llvm_acyclic_annotation() {
        let mut backend = LlvmBackend::new();
        let output = backend.generate(&empty_program());
        assert!(!output.is_empty());
    }

    #[test]
    fn test_llvm_event_model_lowering() {
        let mut backend = LlvmBackend::new();
        let program = Program {
            items: vec![
                TopLevel::Trigger(TriggerDeclaration {
                    name: "io_pending".to_string(),
                    ty: Type::Bool,
                    address: LinkRef::Linked("__io_pending".to_string()),
                    bit_range: None,
                    stages: vec![],
                    condition: None,
                    is_wake: false,
                    span: None,
                }),
                TopLevel::StateDecl(StateDecl {
                    name: "event_count".to_string(),
                    ty: Type::Int,
                    expr: Some(Expr::Integer(0)),
                    address: None,
                    bit_range: None,
                    is_override: false,
                    os_mode: false,
                    span: None,
                    attrs: vec![],
                }),
                TopLevel::Transaction(Transaction {
                    name: "pump".to_string(),
                    parameters: vec![],
                    contract: Contract {
                        pre_condition: Expr::Bool(true),
                        post_condition: Expr::Bool(true),
                        span: None,
                        watchdog: None,
                    },
                    body: vec![Statement::Term { values: vec![], modifiers: vec![] }],
                    is_async: false,
                    is_reactive: true,
                    reactor_speed: None,
                    span: None,
                    is_lambda: false,
                    dependencies: vec![],
                    attrs: vec![],
                    modifiers: vec![],
                    variant_bodies: vec![],
                }),
                TopLevel::Transaction(Transaction {
                    name: "sleep".to_string(),
                    parameters: vec![],
                    contract: Contract {
                        pre_condition: Expr::Bool(true),
                        post_condition: Expr::Bool(true),
                        span: None,
                        watchdog: None,
                    },
                    body: vec![Statement::Term { values: vec![], modifiers: vec![] }],
                    is_async: false,
                    is_reactive: true,
                    reactor_speed: None,
                    span: None,
                    is_lambda: false,
                    dependencies: vec![],
                    attrs: vec![],
                    modifiers: vec![],
                    variant_bodies: vec![],
                }),
            ],
            comments: vec![],
            reactor_speed: None,
            attrs: Vec::new(),
            ffi: None,
            strict_mode: StrictMode::Off,
            dispatch_mode: Default::default(),
        };
        let output = backend.generate(&program);

        // @ link trigger emits external global
        assert!(output.contains("external global"), "Should declare external globals for @ link");
        assert!(output.contains("__io_pending"), "Should contain trigger global name");

        // Fall-through dispatch: body blocks don't end with ret void
        assert!(output.contains("reactor_tick"), "Should have reactor_tick");
        assert!(output.contains("global_state"), "Should reference global state");
        assert!(output.contains("__io_pending"), "Should reference trigger");

        // Trigger sampling emits load volatile
        assert!(output.contains("load volatile"), "Should have volatile trigger loads");

        // Must not have __wait_for_event as hardcoded intrinsic
        assert!(!output.contains("declare void @__wait_for_event()"),
            "Should NOT have hardcoded __wait_for_event declaration");
    }

    // ── Phase 4: Backend correctness tests ──────────────────────────

    #[test]
    fn test_escape_non_ascii_string() {
        let output = escape_llvm_string("héllo");
        // 'é' is U+00E9 → bytes C3 A9
        assert!(output.contains("\\c3"), "Should hex-escape byte C3");
        assert!(output.contains("\\a9"), "Should hex-escape byte A9");
        // ASCII 'h' 'e' 'l' 'l' 'o' should be preserved as-is
        assert!(output.contains("h"), "ASCII 'h' should be preserved");
        assert!(output.contains("llo"), "ASCII 'llo' should be preserved after escape bytes");
    }

    #[test]
    fn test_unification_payload_discriminant() {
        let mut backend = LlvmBackend::new();
        let program = Program {
            items: vec![
                TopLevel::StateDecl(StateDecl {
                    name: "s".to_string(),
                    ty: Type::Int,
                    expr: Some(Expr::Integer(0)),
                    address: None,
                    bit_range: None,
                    is_override: false,
                    os_mode: false,
                    span: None,
                    attrs: vec![],
                }),
                TopLevel::Transaction(Transaction {
                    name: "t".to_string(),
                    parameters: vec![],
                    contract: Contract {
                        pre_condition: Expr::Bool(true),
                        post_condition: Expr::Bool(true),
                        span: None,
                        watchdog: None,
                    },
                    body: vec![
                        Statement::Unification {
                            name: "Some".to_string(),
                            pattern: "v".to_string(),
                            expr: Expr::Integer(1),
                        },
                        Statement::Term { values: vec![], modifiers: vec![] },
                    ],
                    is_async: false,
                    is_reactive: false,
                    reactor_speed: None,
                    span: None,
                    is_lambda: false,
                    dependencies: vec![],
                    attrs: vec![],
                    modifiers: vec![],
                    variant_bodies: vec![],
                }),
            ],
            comments: vec![],
            reactor_speed: None,
            attrs: Vec::new(),
            ffi: None,
            strict_mode: StrictMode::Off,
            dispatch_mode: Default::default(),
        };
        let output = backend.generate(&program);
        // Payload variant Some → discriminant 1
        assert!(output.contains("i64 1, label"),
            "Unification of 'Some' should target discriminant 1");
    }

    #[test]
    fn test_no_range_lower_bound_defaults_to_i64_min() {
        let mut backend = LlvmBackend::new();
        let program = Program {
            items: vec![
                TopLevel::StateDecl(StateDecl {
                    name: "x".to_string(),
                    ty: Type::Int,
                    expr: Some(Expr::Integer(0)),
                    address: None,
                    bit_range: None,
                    is_override: false,
                    os_mode: false,
                    span: None,
                    attrs: vec![],
                }),
                TopLevel::Transaction(Transaction {
                    name: "t".to_string(),
                    parameters: vec![],
                    contract: Contract {
                        pre_condition: Expr::Lt(
                            Box::new(Expr::Identifier("x".to_string())),
                            Box::new(Expr::Integer(100)),
                        ),
                        post_condition: Expr::Bool(true),
                        span: None,
                        watchdog: None,
                    },
                    body: vec![
                        Statement::Term { values: vec![], modifiers: vec![] },
                    ],
                    is_async: false,
                    is_reactive: false,
                    reactor_speed: None,
                    span: None,
                    is_lambda: false,
                    dependencies: vec![],
                    attrs: vec![],
                    modifiers: vec![],
                    variant_bodies: vec![],
                }),
            ],
            comments: vec![],
            reactor_speed: None,
            attrs: Vec::new(),
            ffi: None,
            strict_mode: StrictMode::Off,
            dispatch_mode: Default::default(),
        };
        let output = backend.generate(&program);
        // Lower bound should be i64::MIN = -9223372036854775808
        assert!(output.contains("-9223372036854775808"),
            "Range with no lower bound should use i64::MIN");
    }

    #[test]
    fn test_binop_no_nuw_nsw() {
        let mut backend = LlvmBackend::new();
        let program = Program {
            items: vec![
                TopLevel::StateDecl(StateDecl {
                    name: "x".to_string(),
                    ty: Type::Int,
                    expr: Some(Expr::Integer(0)),
                    address: None,
                    bit_range: None,
                    is_override: false,
                    os_mode: false,
                    span: None,
                    attrs: vec![],
                }),
                TopLevel::StateDecl(StateDecl {
                    name: "y".to_string(),
                    ty: Type::Int,
                    expr: Some(Expr::Integer(0)),
                    address: None,
                    bit_range: None,
                    is_override: false,
                    os_mode: false,
                    span: None,
                    attrs: vec![],
                }),
                TopLevel::Transaction(Transaction {
                    name: "t".to_string(),
                    parameters: vec![],
                    contract: Contract {
                        pre_condition: Expr::And(
                            Box::new(Expr::And(
                                Box::new(                Expr::Ge(
                                    Box::new(Expr::Identifier("x".to_string())),
                                    Box::new(Expr::Integer(0)),
                                )),
                                Box::new(Expr::Lt(
                                    Box::new(Expr::Identifier("x".to_string())),
                                    Box::new(Expr::Integer(10)),
                                )),
                            )),
                            Box::new(Expr::Lt(
                                Box::new(Expr::Identifier("y".to_string())),
                                Box::new(Expr::Integer(10)),
                            )),
                        ),
                        post_condition: Expr::Bool(true),
                        span: None,
                        watchdog: None,
                    },
                    body: vec![
                        Statement::Assignment {
                            lhs: Expr::OwnedRef("x".to_string()),
                            expr: Expr::Add(
                                Box::new(Expr::Identifier("x".to_string())),
                                Box::new(Expr::Identifier("y".to_string())),
                            ),
                            timeout: None,
                            modifiers: vec![],
                        },
                        Statement::Term { values: vec![], modifiers: vec![] },
                    ],
                    is_async: false,
                    is_reactive: false,
                    reactor_speed: None,
                    span: None,
                    is_lambda: false,
                    dependencies: vec![],
                    attrs: vec![],
                    modifiers: vec![],
                    variant_bodies: vec![],
                }),
            ],
            comments: vec![],
            reactor_speed: None,
            attrs: Vec::new(),
            ffi: None,
            strict_mode: StrictMode::Off,
            dispatch_mode: Default::default(),
        };
        let output = backend.generate(&program);
        // Must NOT emit nuw nsw — we removed manual emission
        assert!(!output.contains("nuw nsw"),
            "add on bounded variables should NOT emit nuw nsw (LLVM infers from !range)");
    }

    // ── Phase 5: Wake trigger and blocking wait tests ────────────────

    fn make_wake_trg_program(trg_name: &str, sym: &str, ty: Type, is_wake: bool) -> Program {
        Program {
            items: vec![
                TopLevel::Trigger(TriggerDeclaration {
                    name: trg_name.to_string(),
                    ty,
                    address: LinkRef::Linked(sym.to_string()),
                    bit_range: None,
                    stages: vec![],
                    condition: None,
                    is_wake,
                    span: None,
                }),
                TopLevel::Transaction(Transaction {
                    name: "t".to_string(),
                    parameters: vec![],
                    contract: Contract {
                        pre_condition: Expr::Identifier(trg_name.to_string()),
                        post_condition: Expr::Bool(true),
                        span: None,
                        watchdog: None,
                    },
                    body: vec![
                        Statement::Term { values: vec![], modifiers: vec![] },
                    ],
                    is_async: false,
                    is_reactive: true,
                    reactor_speed: None,
                    span: None,
                    is_lambda: false,
                    dependencies: vec![],
                    attrs: vec![],
                    modifiers: vec![],
                    variant_bodies: vec![],
                }),
            ],
            comments: vec![],
            reactor_speed: None,
            attrs: Vec::new(),
            ffi: None,
            strict_mode: StrictMode::Off,
            dispatch_mode: Default::default(),
        }
    }

    #[test]
    fn test_no_wake_triggers_no_metadata() {
        let program = make_wake_trg_program("sig", "__sigint_flag", Type::Bool, false);
        let output = LlvmBackend::new().generate(&program);
        assert!(!output.contains("@llvm.wake_triggers"),
            "No wake triggers → no @llvm.wake_triggers metadata");
        assert!(!output.contains("call void @__rt_init()"),
            "No wake triggers → no __rt_init call");
        assert!(!output.contains("call void @__rt_wait()"),
            "No wake triggers → no __rt_wait call");
    }

    #[test]
    fn test_single_wake_trigger_metadata() {
        let program = make_wake_trg_program("sig", "__sigint_flag", Type::Bool, true);
        let output = LlvmBackend::new().generate(&program);
        assert!(output.contains("@llvm.wake_triggers = appending global [1 x i8*] [i8* @__sigint_flag]"),
            "Single wake trigger → appending global with one symbol");
        assert!(output.contains("!llvm.wake_triggers = !{!0}"),
            "Named metadata node present");
        assert!(output.contains("!0 = !{!\"__sigint_flag\"}"),
            "Metadata references __sigint_flag");
    }

    #[test]
    fn test_multiple_wake_triggers_metadata() {
        let mut p1 = make_wake_trg_program("sigint", "__sigint_flag", Type::Bool, true);
        p1.items.insert(1, TopLevel::Trigger(TriggerDeclaration {
            name: "stdin".to_string(),
            ty: Type::Bool,
            address: LinkRef::Linked("__stdin_ready".to_string()),
            bit_range: None,
            stages: vec![],
            condition: None,
            is_wake: true,
            span: None,
        }));
        let output = LlvmBackend::new().generate(&p1);
        assert!(output.contains("[2 x i8*]"),
            "Multiple wake triggers → array size 2");
        assert!(output.contains("__sigint_flag"),
            "First symbol present");
        assert!(output.contains("__stdin_ready"),
            "Second symbol present");
    }

    #[test]
    fn test_main_calls_rt_init_and_rt_wait_with_wake_triggers() {
        let program = make_wake_trg_program("sig", "__sigint_flag", Type::Bool, true);
        let output = LlvmBackend::new().generate(&program);
        assert!(output.contains("call void @__rt_init()"),
            "main() calls __rt_init() when wake triggers exist");
        assert!(output.contains("call void @__rt_wait()"),
            "main() calls __rt_wait() after reactor_tick");
    }

    #[test]
    fn test_main_no_init_wait_without_wake_triggers() {
        let program = make_wake_trg_program("sig", "__sigint_flag", Type::Bool, false);
        let output = LlvmBackend::new().generate(&program);
        assert!(!output.contains("call void @__rt_init()"),
            "main() does not call __rt_init() without wake triggers");
        assert!(!output.contains("call void @__rt_wait()"),
            "main() does not call __rt_wait() without wake triggers");
    }

    #[test]
    fn test_rt_declares_present() {
        let program = make_wake_trg_program("sig", "__sigint_flag", Type::Bool, false);
        let output = LlvmBackend::new().generate(&program);
        assert!(output.contains("declare void @__rt_init()"),
            "__rt_init always declared");
        assert!(output.contains("declare void @__rt_wait()"),
            "__rt_wait always declared");
    }

    #[test]
    fn test_wake_non_link_trigger_no_metadata() {
        // MMIO triggers with #wake should not appear in metadata (parse-time error, but belt-and-suspenders)
        let program = Program {
            items: vec![
                TopLevel::Trigger(TriggerDeclaration {
                    name: "mmio".to_string(),
                    ty: Type::Bool,
                    address: LinkRef::Explicit(0x4000),
                    bit_range: None,
                    stages: vec![],
                    condition: None,
                    is_wake: true,
                    span: None,
                }),
                TopLevel::Transaction(Transaction {
                    name: "t".to_string(),
                    parameters: vec![],
                    contract: Contract { pre_condition: Expr::Bool(true), post_condition: Expr::Bool(true), span: None, watchdog: None },
                    body: vec![Statement::Term { values: vec![], modifiers: vec![] }],
                    is_async: false, is_reactive: true, reactor_speed: None, span: None,
                    is_lambda: false, dependencies: vec![], attrs: vec![], modifiers: vec![], variant_bodies: vec![],
                }),
            ],
            comments: vec![],
            reactor_speed: None,
            attrs: Vec::new(),
            ffi: None,
            strict_mode: StrictMode::Off,
            dispatch_mode: Default::default(),
        };
        let output = LlvmBackend::new().generate(&program);
        // MMIO triggers with is_wake → metadata only includes LinkRef::Linked symbols, not Explicit
        assert!(!output.contains("@llvm.wake_triggers"),
            "MMIO wake trigger should not produce metadata (not a linked symbol)");
    }

    // ── Plan C: Local float binding tests ─────────────────────────

    #[test]
    fn test_local_float_binding() {
        let mut backend = LlvmBackend::new();
        let program = Program {
            items: vec![
                TopLevel::StateDecl(StateDecl {
                    name: "x".to_string(),
                    ty: Type::Float,
                    expr: Some(Expr::Float(1.5)),
                    address: None, bit_range: None,
                    is_override: false, os_mode: false,
                    span: None, attrs: vec![],
                }),
                TopLevel::Transaction(Transaction {
                    name: "t".to_string(),
                    parameters: vec![],
                    contract: Contract {
                        pre_condition: Expr::Bool(true),
                        post_condition: Expr::Bool(true),
                        span: None, watchdog: None,
                    },
                    body: vec![
                        Statement::Assignment {
                            lhs: Expr::Identifier("x".to_string()),
                            expr: Expr::Float(2.0),
                            timeout: None, modifiers: vec![],
                        },
                        Statement::Term { values: vec![], modifiers: vec![] },
                    ],
                    is_async: false, is_reactive: false,
                    reactor_speed: None, span: None,
                    is_lambda: false, dependencies: vec![],
                    attrs: vec![], modifiers: vec![],
                    variant_bodies: vec![],
                }),
            ],
            comments: vec![],
            reactor_speed: None,
            attrs: Vec::new(),
            ffi: None,
            strict_mode: StrictMode::Off,
            dispatch_mode: Default::default(),
        };
        let output = backend.generate(&program);
        assert!(output.contains("bitcast float"),
            "Float expression should emit bitcast float to i32");
        assert!(output.contains("bitcast i32"),
            "Float literal should appear as bitcast i32 to float in IR");
    }

    #[test]
    fn test_float_binary_add() {
        let mut backend = LlvmBackend::new();
        let program = Program {
            items: vec![
                TopLevel::StateDecl(StateDecl {
                    name: "x".to_string(),
                    ty: Type::Float,
                    expr: Some(Expr::Float(1.0)),
                    address: None, bit_range: None,
                    is_override: false, os_mode: false,
                    span: None, attrs: vec![],
                }),
                TopLevel::Transaction(Transaction {
                    name: "t".to_string(),
                    parameters: vec![],
                    contract: Contract {
                        pre_condition: Expr::Bool(true),
                        post_condition: Expr::Bool(true),
                        span: None, watchdog: None,
                    },
                    body: vec![
                        Statement::Assignment {
                            lhs: Expr::Identifier("x".to_string()),
                            expr: Expr::Add(
                                Box::new(Expr::Identifier("x".to_string())),
                                Box::new(Expr::Float(2.0)),
                            ),
                            timeout: None, modifiers: vec![],
                        },
                        Statement::Term { values: vec![], modifiers: vec![] },
                    ],
                    is_async: false, is_reactive: false,
                    reactor_speed: None, span: None,
                    is_lambda: false, dependencies: vec![],
                    attrs: vec![], modifiers: vec![],
                    variant_bodies: vec![],
                }),
            ],
            comments: vec![],
            reactor_speed: None,
            attrs: Vec::new(),
            ffi: None,
            strict_mode: StrictMode::Off,
            dispatch_mode: Default::default(),
        };
        let output = backend.generate(&program);
        assert!(output.contains("fadd float"),
            "Float binary add should emit fadd float");
    }

    #[test]
    fn test_main_and_reactor_use_non_willreturn_attr() {
        let program = make_wake_trg_program("sig", "__sigint_flag", Type::Bool, true);
        let output = LlvmBackend::new().generate(&program);
        assert!(output.contains("attributes #2"),
            "Should emit attributes #2 for reactor_tick");
        assert!(output.contains("attributes #3"),
            "Should emit attributes #3 for main (no mustprogress)");
        assert!(!output.contains("define i32 @main() local_unnamed_addr #2"),
            "main() should NOT use mustprogress attribute #2");
        assert!(output.contains("define i32 @main() local_unnamed_addr #3"),
            "main() should use non-mustprogress attribute #3");
        assert!(output.contains("define void @reactor_tick() local_unnamed_addr #2"),
            "reactor_tick() should use non-willreturn attribute #2");
        assert!(output.contains("attributes #0"),
            "attributes #0 should still be present for terminating functions");
        assert!(output.contains("define void @init_state() local_unnamed_addr #0"),
            "init_state() should still use #0 with willreturn");
    }
}