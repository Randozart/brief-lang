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
        TopLevel::Constant(c) => { collect_strings_expr(&c.expr, seen, out); }
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

/// Returns true if the expression is a direct reference to one of the given
/// trigger names (i.e., the precondition is `trg_name` with no operators).
fn is_trigger_gated(pre: &Expr, trigger_names: &std::collections::HashSet<&str>) -> bool {
    match pre {
        Expr::Identifier(name) => trigger_names.contains(name.as_str()),
        // And(trigger, bounded_pre) — the common pattern for
        // trigger-gated counter transactions in benchmarks.
        Expr::And(l, r) => {
            is_trigger_gated(l, trigger_names) || is_trigger_gated(r, trigger_names)
        }
        _ => false,
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
    constants: HashMap<String, (Type, Expr)>,
    fused_to_first: HashMap<String, String>,
    sampled_triggers: HashMap<String, String>,
    txn_write_masks: HashMap<String, u64>,
    optimize_budget: u64,
    optimize_report: bool,
    optimize_size: Option<u64>,
    report_lines: Vec<String>,
    has_async_txns: bool,
    async_txn_names: Vec<String>,
    async_thread_pool_size: u32,
    exit_condition: Option<Box<Expr>>,
    warnings: Vec<String>,
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
            constants: HashMap::new(),
            fused_to_first: HashMap::new(),
            sampled_triggers: HashMap::new(),
            txn_write_masks: HashMap::new(),
            optimize_budget: 256,
            optimize_report: false,
            optimize_size: None,
            report_lines: Vec::new(),
            has_async_txns: false,
            async_txn_names: Vec::new(),
            async_thread_pool_size: 0,
            exit_condition: None,
            warnings: Vec::new(),
        }
    }

    pub fn with_spec(mut self, spec: crate::target_spec::TargetSpec) -> Self {
        self.spec = Some(spec);
        self
    }

    pub fn with_optimize_budget(mut self, budget: u64) -> Self {
        self.optimize_budget = budget;
        self
    }

    pub fn with_optimize_report(mut self, report: bool) -> Self {
        self.optimize_report = report;
        self
    }

    pub fn with_optimize_size(mut self, byte_limit: u64) -> Self {
        self.optimize_size = Some(byte_limit);
        self.optimize_report = true;
        self
    }

    pub fn generate(&mut self, program: &Program) -> String {
        let mut analysis = crate::backend::analyze_program(program, false);

        analysis.region_analyzer.compose_chains();
        analysis.region_analyzer.build_budget_plan(self.optimize_budget);

        let precomputed_final_values = if analysis.region_analyzer.is_fully_precomputable(self.optimize_budget) {
            analysis.region_analyzer.collect_final_values(program)
        } else { None };

        let cg = &analysis.call_graph;
        self.has_cycles = cg.has_cycle();

        self.exit_condition = program.exit_condition.clone();
        self.build_field_index(program);
        self.triggers.clear();
        self.trigger_names.clear();
        self.program_txns.clear();
        self.defn_params.clear();
        self.constants.clear();
        self.string_constants = collect_strings(program);

        let mut txns: Vec<(String, &crate::ast::Transaction)> = Vec::new();
        for item in &program.items {
            match item {
                TopLevel::Constant(c) => {
                    self.constants.insert(c.name.clone(), (c.ty.clone(), c.expr.clone()));
                }
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

        // Verify all #!exit identifiers exist as state fields or constants.
        if let Some(ref cond) = self.exit_condition {
            let errors = self.check_exit_condition_idents(cond);
            if !errors.is_empty() {
                for err in &errors {
                    eprintln!("{}", err);
                }
                std::process::exit(1);
            }
        }

        // Auto-select Parallel dispatch when all reactive transactions
        // are proven conflict-free. The proof engine's check_mutual_exclusion
        // already validates this for async txns; here we extend the check to
        // ALL reactive txns. If no pair has read/write or write/write conflicts
        // with overlapping preconditions, parallel dispatch is safe.
        let dispatch_mode = if program.dispatch_mode == crate::ast::DispatchMode::Sequential {
            let reactive_txns: Vec<&crate::ast::Transaction> = txns.iter()
                .filter(|(_, t)| t.is_reactive)
                .map(|(_, t)| *t)
                .collect();
            let mut cf = true;
            for i in 0..reactive_txns.len() {
                for j in (i + 1)..reactive_txns.len() {
                    let a = reactive_txns[i];
                    let b = reactive_txns[j];
                    let a_writes: std::collections::HashSet<String> =
                        crate::backend::collect_assigned_identifiers(&a.body)
                            .into_iter().collect();
                    let b_writes: std::collections::HashSet<String> =
                        crate::backend::collect_assigned_identifiers(&b.body)
                            .into_iter().collect();
                    let a_reads = crate::backend::collect_read_identifiers(&a.body);
                    let b_reads = crate::backend::collect_read_identifiers(&b.body);
                    // Write/write conflict?
                    if !a_writes.is_disjoint(&b_writes) { cf = false; break; }
                    // Write/read conflict?
                    let mut a_pre_ids = std::collections::HashSet::new();
                    crate::backend::collect_expr_identifiers(&a.contract.pre_condition, &mut a_pre_ids);
                    let mut b_pre_ids = std::collections::HashSet::new();
                    crate::backend::collect_expr_identifiers(&b.contract.pre_condition, &mut b_pre_ids);
                    if !a_pre_ids.is_disjoint(&b_pre_ids) {
                        if !a_writes.is_disjoint(&b_reads) { cf = false; break; }
                        if !b_writes.is_disjoint(&a_reads) { cf = false; break; }
                    }
                }
                if !cf { break; }
            }
            if cf {
                crate::ast::DispatchMode::Parallel
            } else {
                program.dispatch_mode
            }
        } else {
            program.dispatch_mode
        };

        // Auto-categorize reactive transactions for dispatch path:
        //   - enum_txns: trigger-gated + bounded value sets → switch dispatch
        //   - async_txns: conflict-free pairwise → concurrent thread pool
        //   - sequential_txns: everything else → main-thread sequential
        // Priority: enum > async > sequential (enum is O(1) folded loops)
        let has_wake_triggers = self.triggers.values().any(|t| t.is_wake);
        let enumerable: Option<Vec<(String, Option<u64>)>> = {
            let region = &analysis.region_analyzer;
            if !self.trigger_names.is_empty() {
                let mut sizes = Vec::new();
                let mut total: u64 = 1;
                let mut ok = true;
                for tn in &self.trigger_names {
                    let sz = region.value_set_size_of(tn);
                    if let Some(s) = sz {
                        total = total.saturating_mul(s);
                        if total > self.optimize_budget { ok = false; break; }
                        sizes.push((tn.clone(), sz));
                    } else {
                        ok = false;
                        break;
                    }
                }
                if ok { Some(sizes) } else { None }
            } else { None }
        };
        // Determine which txns are enum candidates based on trigger data
        let enum_txn_names: std::collections::HashSet<String> = if let Some(ref en) = enumerable {
            let enum_trigger_names: std::collections::HashSet<&str> =
                en.iter().map(|(n, _)| n.as_str()).collect();
            txns.iter()
                .filter(|(_, t)| {
                    t.is_reactive
                        && is_trigger_gated(&t.contract.pre_condition, &enum_trigger_names)
                })
                .map(|(n, _)| n.clone())
                .collect()
        } else { std::collections::HashSet::new() };
        // Async candidates: conflict-free reactive txns not claimed by enum dispatch
        let async_candidates: Vec<&crate::ast::Transaction> = txns.iter()
            .filter(|(n, t)| t.is_reactive && !enum_txn_names.contains(n.as_str()))
            .map(|(_, t)| *t)
            .collect();
        let ac_writes: Vec<std::collections::HashSet<String>> = async_candidates.iter()
            .map(|t| crate::backend::collect_assigned_identifiers(&t.body).into_iter().collect())
            .collect();
        let ac_reads: Vec<std::collections::HashSet<String>> = async_candidates.iter()
            .map(|t| crate::backend::collect_read_identifiers(&t.body))
            .collect();
        let mut is_async_eligible: Vec<bool> = vec![true; async_candidates.len()];
        for i in 0..async_candidates.len() {
            for j in (i + 1)..async_candidates.len() {
                let has_conflict = !ac_writes[i].is_disjoint(&ac_writes[j])
                    || !ac_writes[i].is_disjoint(&ac_reads[j])
                    || !ac_writes[j].is_disjoint(&ac_reads[i]);
                if has_conflict {
                    is_async_eligible[i] = false;
                    is_async_eligible[j] = false;
                }
            }
        }
        let all_async_eligible = async_candidates.len() >= 2 && is_async_eligible.iter().all(|&x| x);
        let mut async_txn_names: std::collections::HashSet<String> = std::collections::HashSet::new();
        if all_async_eligible {
            for ac in &async_candidates {
                async_txn_names.insert(ac.name.clone());
            }
        }
        // Store for use by emit_main / emit_enum_main / metadata
        self.has_async_txns = !async_txn_names.is_empty();
        self.async_txn_names = async_txn_names.iter().cloned().collect();
        self.async_thread_pool_size = self.async_txn_names.len() as u32;

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

        // Emit constant globals for TopLevel::Constant declarations
        for (name, (ty, expr)) in &self.constants {
            let llvm_ty = match ty {
                Type::Float => "float",
                Type::Int | Type::UInt => "i64",
                Type::Bool => "i1",
                _ => "i64",
            };
            let val_str = match expr {
                Expr::Float(f) => format!("bitcast (i32 {} to float)", float_to_llvm_hex(*f)),
                Expr::Integer(n) => n.to_string(),
                Expr::Bool(b) => (if *b { "true" } else { "false" }).to_string(),
                Expr::Neg(inner) => match inner.as_ref() {
                    Expr::Float(f) => format!("bitcast (i32 {} to float)", float_to_llvm_hex(-*f)),
                    Expr::Integer(n) => format!("-{}", n),
                    _ => "0".to_string(),
                },
                Expr::String(_) => "null".to_string(),
                _ => "0".to_string(),
            };
            if *ty == Type::Float {
                writeln!(out, "@{} = constant {} {}",
                    name, llvm_ty, val_str).ok();
            } else {
                writeln!(out, "@{} = constant {} {}", name, llvm_ty, val_str).ok();
            }
        }
        if !self.constants.is_empty() { writeln!(out).ok(); }

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
        // Async body functions — simple pre→fire wrapper for worker threads
        for (name, txn) in &txns {
            if async_txn_names.contains(name.as_str()) {
                self.emit_async_body(&mut out, txn, name);
                writeln!(out).ok();
            }
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
        // Composed chain functions
        // Extract all-internal counter info before taking composed_chains.
        let all_internal_counter: HashMap<String, (usize, i64)> = analysis.region_analyzer.composed_chains
            .iter()
            .filter(|cc| cc.all_internal)
            .filter_map(|cc| {
                let cv = cc.counter_var.as_ref()?;
                let ci = *self.field_index_map.get(cv)?;
                let bound = analysis.region_analyzer.iteration_bound_of(&cc.chain[0])? as i64;
                let base = format!("{}_fused_txn", cc.chain.join("_"));
                let fn_name = if let Some(ref tv) = cc.trigger_values {
                    let suffix: String = tv.iter().map(|(_, v)| v.to_string()).collect::<Vec<_>>().join("_");
                    format!("{}_trg_{}", base, suffix)
                } else { base };
                Some((fn_name, (ci, bound)))
            })
            .collect();

        let composed_chains = std::mem::take(&mut analysis.region_analyzer.composed_chains);
        let mut composed_fn_map: HashMap<String, String> = HashMap::new();
        let mut composed_by_trig: HashMap<String, Vec<(i64, String)>> = HashMap::new();
        for cc in &composed_chains {
            let base = format!("{}_fused_txn", cc.chain.join("_"));
            let fused_name = if let Some(ref tv) = cc.trigger_values {
                let variant_suffix: String = tv.iter()
                    .map(|(_, v)| v.to_string())
                    .collect::<Vec<_>>().join("_");
                format!("{}_trg_{}", base, variant_suffix)
            } else {
                base.clone()
            };
            composed_fn_map.insert(cc.chain[0].clone(), fused_name.clone());
            if let Some(ref tv) = cc.trigger_values {
                for (_, val) in tv {
                    composed_by_trig.entry(cc.chain[0].clone()).or_default().push((*val, fused_name.clone()));
                }
            }
            // Skip emitting fused function for all-internal chains — the
            // per-case arms in emit_enum_main will store the final counter
            // value directly instead of calling the folded loop.
            if !cc.all_internal {
                self.emit_fused_composed(&mut out, &cc.composed_body, &fused_name);
                writeln!(out).ok();
            }
        }
        // Init
        self.emit_init_state(&mut out);
        writeln!(out).ok();
        // Reactor — sequential or parallel
        // Enumeration and wake trigger detection were computed above
        // in the auto-categorization step (lines ~312-380).

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
                if let Some(&counter_idx) = self.field_index_map.get(&bp.var) {
                    let total_idx = self.field_index_map.get(&bp.bound_var).copied();
                    let total_const_name: Option<&str> = if total_idx.is_none() {
                        if self.constants.contains_key(&bp.bound_var) {
                            Some(bp.bound_var.as_str())
                        } else { None }
                    } else { None };
                    if total_idx.is_some() || total_const_name.is_some() {
                        if node.is_pure_body {
                            let total_val = self.field_initializers
                                .get(&bp.bound_var)
                                .and_then(|e| e.as_ref())
                                .and_then(|e| {
                                    if let Expr::Integer(n) = e { Some(*n) } else { None }
                                })
                                .or_else(|| {
                                    self.constants.get(&bp.bound_var).and_then(|(_, e)| {
                                        if let Expr::Integer(n) = e { Some(*n) } else { None }
                                    })
                                })
                                .unwrap_or(1);
                            self.emit_folded_pure_counter(&mut out, counter_idx, total_val);
                            true
                        } else {
                            self.emit_folded_main(&mut out, &node.name, counter_idx, total_idx, total_const_name);
                            true
                        }
                    } else { false }
                } else { false }
            } else { false }
        } else { false };

        if !folded {
            let precomputed = if let Some(ref final_values) = precomputed_final_values {
                self.emit_precomputed_main(&mut out, final_values);
                true
            } else { false };

            if !precomputed {
                if let Some(ref enum_sizes) = enumerable {
                // Enumerable triggers — emit switch-dispatch main
                // This path handles triggers with small compile-time-known value sets.
                // We emit a single @main that samples triggers once, then switch
                // dispatches to per-value folded loops.

                // Build per-txn folding params for all enum-candidate txns.
                // Each trigger-gated bounded-counter txn gets its own folded
                // loop in the case arm.  Multi-txn programs (e.g. async_counters)
                // need this to converge in O(1) ticks instead of one increment
                // per tick via reactor_tick.
                let enum_fold_params: HashMap<String, (usize, Option<usize>, Option<String>)> = {
                    let mut m = HashMap::new();
                    for txn_name in &enum_txn_names {
                        if let Some(node) = graph.nodes.iter().find(|n| n.name == *txn_name) {
                            if let Some(ref bp) = node.bounded_pre {
                                if let Some(&cidx) = self.field_index_map.get(&bp.var) {
                                    let tidx = self.field_index_map.get(&bp.bound_var).copied();
                                    let tcname = if tidx.is_none() {
                                        if self.constants.contains_key(&bp.bound_var) {
                                            Some(bp.bound_var.clone())
                                        } else { None }
                                    } else { None };
                                    let inc = node.increments.as_ref();
                                    if inc.map_or(false, |i| i.var == bp.var && i.delta > 0) {
                                        m.insert(txn_name.clone(), (cidx, tidx, tcname));
                                    }
                                }
                            }
                        }
                    }
                    m
                };
                // Legacy single-txn params for chain composition (unchanged)
                let (enum_ci, enum_ti, enum_tcn): (usize, Option<usize>, Option<String>) = if graph.nodes.len() == 1 && txns.len() == 1 {
                    if let Some(bp) = graph.nodes[0].bounded_pre.as_ref() {
                        if let Some(&cidx) = self.field_index_map.get(&bp.var) {
                            let tidx = self.field_index_map.get(&bp.bound_var).copied();
                            let tcname = if tidx.is_none() {
                                if self.constants.contains_key(&bp.bound_var) {
                                    Some(bp.bound_var.clone())
                                } else { None }
                            } else { None };
                            (cidx, tidx, tcname)
                        } else { (0, None, None) }
                    } else { (0, None, None) }
                } else { (0, None, None) };

                // Emit the reactor_tick function (needed for residual fallback path)
                match dispatch_mode {
                    DispatchMode::Parallel => {
                        self.build_write_masks(program);
                        self.emit_parallel_reactor(&mut out, &txns, &fusable);
                    }
                    DispatchMode::Sequential => {
                        self.emit_reactor(&mut out, &txns, &fusable);
                    }
                }

                // Emit enum main with switch dispatch
                let enum_tcn_ref = enum_tcn.as_deref();
                let composed_fn = txns.first()
                    .and_then(|(n, _)| composed_fn_map.get(n))
                    .map(|s| s.as_str());
                let composed_trig_ref: Option<&HashMap<String, Vec<(i64, String)>>> = if !composed_by_trig.is_empty() {
                    Some(&composed_by_trig)
                } else { None };
                let all_int_ref: Option<&HashMap<String, (usize, i64)>> = if all_internal_counter.is_empty() {
                    None
                } else {
                    Some(&all_internal_counter)
                };
                self.emit_enum_main(
                    &mut out,
                    &txns,
                    enum_sizes,
                    &enum_fold_params,
                    enum_ci,
                    enum_ti,
                    enum_tcn_ref,
                    composed_fn,
                    composed_trig_ref,
                    all_int_ref,
                    has_wake_triggers,
                );

                if has_wake_triggers {
                    self.emit_wake_metadata(&mut out);
                }
                self.emit_thread_pool_metadata(&mut out);
            } else if !txns.is_empty() {
                match dispatch_mode {
                    DispatchMode::Parallel => {
                        self.build_write_masks(program);
                        self.emit_parallel_reactor(&mut out, &txns, &fusable);
                    }
                    DispatchMode::Sequential => {
                        self.emit_reactor(&mut out, &txns, &fusable);
                    }
                }
                // Main
                self.emit_main(&mut out, has_wake_triggers);
                // Wake trigger metadata
                if has_wake_triggers {
                    self.emit_wake_metadata(&mut out);
                }
                self.emit_thread_pool_metadata(&mut out);
            } else {
                writeln!(out, "define void @reactor_tick() local_unnamed_addr #2 {{").ok();
                writeln!(out, "  entry:").ok();
                writeln!(out, "  ret void").ok();
                writeln!(out, "}}").ok();
                writeln!(out).ok();
                // Main
                self.emit_main(&mut out, false);
            }
            }
        }

        // ── EXIT CONDITION DIAGNOSTICS ───────────────────────

        // Warning: #!exit on a one-shot program that never checks it.
        // Folded and precomputed paths exit without a tick loop; enum dispatch without
        // wake has no exit_check label. The standard reactor path (emit_main) always
        // checks, so we warn only when we know the check is unreachable.
        if self.exit_condition.is_some() {
            let is_one_shot = folded
                || precomputed_final_values.is_some()
                || (enumerable.is_some() && !has_wake_triggers);
            if is_one_shot {
                self.warnings.push(format!(
                    "warning: #!exit declared but program has no tick loop\n\
                      note: the exit condition will never be checked\n\
                      help: add an @link trigger to make the program reactive, or remove #!exit"
                ));
            }
        }

        // Warning: wake-triggered program without any exit path.
        // Without #!exit or natural death, the program will idle forever after all
        // reactive transactions converge.
        if has_wake_triggers && self.exit_condition.is_none() {
            self.warnings.push(format!(
                "warning: program has wake triggers but no exit path\n\
                  note: after all transactions converge, the program will spin forever\n\
                  help: add `#!exit <condition>;` at the top of the file"
            ));
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

        // Build optimization report if requested
        if self.optimize_report {
            self.report_lines.push("=== Optimization Report ===".to_string());
            self.report_lines.push(format!("Optimize budget: {}", self.optimize_budget));
            let enum_count = enumerable.as_ref().map(|e| e.len()).unwrap_or(0);
            self.report_lines.push(format!("Triggers found: {} (enumerable: {})", self.trigger_names.len(), enum_count));
            if let Some(ref sizes) = enumerable {
                let mut total_combos: u64 = 1;
                self.report_lines.push("".to_string());
                self.report_lines.push("Trigger variable value sets:".to_string());
                for (tn, sz) in sizes {
                    let s = sz.unwrap_or(0);
                    self.report_lines.push(format!("  {}: {} values", tn, s));
                    total_combos = total_combos.saturating_mul(s);
                }
                self.report_lines.push(format!("Total combinations: {}", total_combos));
                if total_combos <= self.optimize_budget {
                    self.report_lines.push(format!("  ✅ Within budget ({} ≤ {})", total_combos, self.optimize_budget));
                    self.report_lines.push("  → Switch-dispatch enumeration enabled".to_string());
                } else {
                    self.report_lines.push(format!("  ❌ Exceeds budget ({} > {})", total_combos, self.optimize_budget));
                    self.report_lines.push("  → Standard reactor path used".to_string());
                }

                // Size estimation when --optimize-size is set
                if let Some(byte_limit) = self.optimize_size {
                    self.report_lines.push("".to_string());
                    self.report_lines.push("Size estimation:".to_string());
                    let bytes_per_combo: u64 = 80; // approximate bytes per switch case + folded loop
                    let base_estimate: u64 = 5000;
                    let enum_estimate = base_estimate + total_combos.saturating_mul(bytes_per_combo);
                    self.report_lines.push(format!("  Base binary (standard reactor): ~{} KB", base_estimate / 1024));
                    self.report_lines.push(format!("  With {} enumerated combos: ~{} KB", total_combos, enum_estimate / 1024));
                    if enum_estimate <= byte_limit {
                        self.report_lines.push(format!("  ✅ Enumeration fits within {} KB limit", byte_limit / 1024));
                        self.report_lines.push(format!("  Recommended budget: {}", total_combos));
                    } else {
                        self.report_lines.push(format!("  ❌ Enumeration exceeds {} KB limit", byte_limit / 1024));
                        let max_fit: u64 = if bytes_per_combo > 0 {
                            (byte_limit.saturating_sub(base_estimate)) / bytes_per_combo
                        } else { 0 };
                        self.report_lines.push(format!("  Max combos within limit: {}", max_fit));
                        self.report_lines.push(format!("  Recommended budget: {} (partial enumeration)", max_fit));
                    }
                }
            }
            if let Some(ref graph) = Some(&analysis.transition_graph) {
                self.report_lines.push("".to_string());
                self.report_lines.push("Transaction graph:".to_string());
                self.report_lines.push(format!("  Nodes: {}", graph.nodes.len()));
                self.report_lines.push(format!("  Has triggers: {}", graph.has_triggers));
                if let Some(node) = graph.nodes.first() {
                    if let Some(ref bp) = node.bounded_pre {
                        self.report_lines.push(format!("  Bounded pre: {} < {}", bp.var, bp.bound_var));
                    }
                    self.report_lines.push(format!("  Is reactive: {}", node.is_reactive));
                    self.report_lines.push(format!("  Pure body: {}", node.is_pure_body));
                    if let Some(ref inc) = node.increments {
                        self.report_lines.push(format!("  Increment: {} += {}", inc.var, inc.delta));
                    }
                }
            }
            let chains = &analysis.region_analyzer.linear_chains;
            if !chains.is_empty() {
                self.report_lines.push("".to_string());
                self.report_lines.push("Linear transaction chains detected:".to_string());
                for (i, chain) in chains.iter().enumerate() {
                    let chain_str: String = chain.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(" -> ");
                    self.report_lines.push(format!("  Chain {}: {}", i + 1, chain_str));
                }
            }
            let scores = &analysis.region_analyzer.region_scores;
            if !scores.is_empty() {
                self.report_lines.push("".to_string());
                self.report_lines.push("Optimization priority ranking:".to_string());
                for (rank, score) in scores.iter().enumerate() {
                    let class_str = format!("{:?}", score.complexity);
                    let gpu = if score.gpu_eligible { " GPU" } else { "" };
                    let chain_tag = if score.chain_composed { " Chain" } else { "" };
                    let score_str = if score.optimization_score.is_infinite() || score.optimization_score <= 0.0 {
                        "—".to_string()
                    } else {
                        format!("{:.1}", score.optimization_score)
                    };
                    let vs_size = match score.value_set_size {
                        Some(n) => format!("{}", n),
                        None => "∞".to_string(),
                    };
                    let txn_list = score.txn_names.join(",");
                    self.report_lines.push(format!(
                        "  #{:<3} R{:<4} {:<20} {:<9} {:<7} {:<8} {:<8} {:<8} {}{}",
                        rank + 1, score.region_id, txn_list, class_str,
                        score.body_weight, score.iteration_count, vs_size, score_str,
                        chain_tag, gpu
                    ));
                }
            }
            if let Some(ref plan) = analysis.region_analyzer.budget_plan {
                self.report_lines.push("".to_string());
                self.report_lines.push(format!("Budget plan (budget={}):", plan.total_budget));
                let spent: u64 = plan.total_budget - plan.residual_budget;
                self.report_lines.push(format!("  Allocated: {} regions, spent {}/{}", plan.allocated.len(), spent, plan.total_budget));
                for (rid, cls, cost, score) in &plan.allocated {
                    self.report_lines.push(format!("    R{}: {:?}, cost={}, score={:.1}", rid, cls, cost, score));
                }
                self.report_lines.push(format!("  Residual: {} budget units", plan.residual_budget));
                if !plan.skipped.is_empty() {
                    self.report_lines.push(format!("  Skipped: {} regions (unbounded or exceeds budget)", plan.skipped.len()));
                    for (rid, cls, _) in &plan.skipped {
                        self.report_lines.push(format!("    R{}: {:?}", rid, cls));
                    }
                }
            }
            if !composed_chains.is_empty() {
                self.report_lines.push("".to_string());
                self.report_lines.push("Composed chains:".to_string());
                for cc in &composed_chains {
                    let chain_str: String = cc.chain.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(" → ");
                    let triggers = if cc.root_triggers.is_empty() {
                        "none".to_string()
                    } else {
                        cc.root_triggers.join(", ")
                    };
                    let tv_str = match &cc.trigger_values {
                        Some(tv) => tv.iter().map(|(n, v)| format!("{}={}", n, v)).collect::<Vec<_>>().join(", "),
                        None => if cc.root_triggers.is_empty() { "—".to_string() } else { "unbranched".to_string() },
                    };
                    let internal = if cc.all_internal { " all-internal" } else { "" };
                    self.report_lines.push(format!(
                        "  {} (link vars: {}, triggers: {}, values: [{}], fused weight: {}{})",
                        chain_str, cc.link_vars.join(","), triggers, tv_str, cc.fused_weight, internal
                    ));
                }
            }
            if precomputed_final_values.is_some() {
                self.report_lines.push("".to_string());
                self.report_lines.push("Precomputed (compile-time evaluation):".to_string());
                self.report_lines.push("  All state values determined at compile time — O(1) runtime.".to_string());
                if let Some(ref fv) = precomputed_final_values {
                    self.report_lines.push(format!("  {} chains precomputed.", fv.len()));
                    for (chain, bindings) in fv {
                        let chain_str: String = chain.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(" → ");
                        let vars: Vec<String> = bindings.iter()
                            .map(|(k, v)| format!("{}={}", k, v))
                            .collect();
                        self.report_lines.push(format!("    {} → final state: [{}]", chain_str, vars.join(", ")));
                    }
                }
            }
        }
        out
    }

    /// Return the optimization report lines collected during `generate()`.
    pub fn report(&self) -> &[String] {
        &self.report_lines
    }

    pub fn warnings(&self) -> &[String] {
        &self.warnings
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
        // Thread pool entry points — provided by brief_rt.c when BRIEF_THREAD_POOL is defined.
        // If not linked, the linker will error. The metadata section @llvm.thread_pool
        // tells the compiler driver to add -DBRIEF_THREAD_POOL.
        writeln!(out, "declare void @brief_thread_pool_init(i32, i8**) local_unnamed_addr").ok();
        writeln!(out, "declare void @brief_barrier_release() local_unnamed_addr").ok();
        writeln!(out, "declare void @brief_barrier_wait() local_unnamed_addr").ok();
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

    // ── ASYNC BODY FUNCTION ──────────────────────────────────
    /// Emit a worker-thread body for an async transaction.
    /// Structure: evaluate precondition, if true fire the txn body, return.
    /// Called once per tick per worker thread. Uses `#4` attribute
    /// (always returns from a single tick, called in a loop).
    fn emit_async_body(&mut self, out: &mut String, txn: &crate::ast::Transaction, name: &str) {
        let async_name = format!("async_body_{}", name);
        writeln!(out, "define void @{}(%State* noalias nocapture %state) local_unnamed_addr #0 {{", async_name).ok();
        writeln!(out, "  entry:").ok();
        self.txn_counter = 0;
        self.let_bindings.clear();
        // Evaluate precondition
        let cond = self.emit_expr(out, &txn.contract.pre_condition, "  ");
        let i1 = format!("%ri{}", self.txn_counter); self.txn_counter += 1;
        writeln!(out, "  {} = icmp ne i64 {}, 0", i1, cond).ok();
        let txn_fire_l = format!("txn_fire_{}", self.txn_counter + 1);
        writeln!(out, "  br i1 {}, label %{}, label %{}_done", i1, txn_fire_l, async_name).ok();
        writeln!(out, "{}:", txn_fire_l).ok();
        // Fire the txn body
        self.terminated = false;
        self.returns_i64 = false;
        for s in &txn.body { self.emit_stmt(out, s, "  "); }
        if !self.terminated { writeln!(out, "  ret void").ok(); }
        writeln!(out, "{}_done:", async_name).ok();
        writeln!(out, "  ret void").ok();
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

    fn emit_fused_composed(&mut self, out: &mut String, body: &[Statement], name: &str) {
        writeln!(out, "define void @{}(%State* noalias nocapture %state) local_unnamed_addr #0 {{", name).ok();
        writeln!(out, "  entry:").ok();
        self.txn_counter = 0; self.let_bindings.clear(); self.terminated = false; self.returns_i64 = false;
        for s in body { self.emit_stmt(out, s, "  "); }
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
                } else if let Some((ty, _)) = self.constants.get(name) {
                    let ll_ty = match ty {
                        Type::Float => "float",
                        Type::Int | Type::UInt => "i64",
                        Type::Bool => "i8",
                        _ => "i64",
                    };
                    let ld = format!("%il{}", self.txn_counter); self.txn_counter += 1;
                    writeln!(out, "{}{} = load {}, {}* @{}, align {}", indent, ld, ll_ty, ll_ty, name, self.align_of(ll_ty)).ok();
                    match ty {
                        t if t == &Type::Float => {
                            let i = format!("%if{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = bitcast float {} to i32", indent, i, ld).ok();
                            writeln!(out, "{}{} = zext i32 {} to i64", indent, v, i).ok();
                        }
                        Type::Bool => {
                            let z = format!("%iz{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = zext i8 {} to i64", indent, z, ld).ok();
                            writeln!(out, "{}{} = add i64 0, {}", indent, v, z).ok();
                        }
                        _ => {
                            writeln!(out, "{}{} = add i64 0, {}", indent, v, ld).ok();
                        }
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

    // ── EXIT CONDITION EXPRESSION ────────────────────────────

    /// Verify all identifiers in the exit condition refer to known state fields or constants.
    /// Returns a list of error messages for unknown identifiers.
    fn check_exit_condition_idents(&self, expr: &Expr) -> Vec<String> {
        let mut errors = Vec::new();
        self.check_exit_condition_idents_inner(expr, &mut errors);
        errors
    }

    fn check_exit_condition_idents_inner(&self, expr: &Expr, errors: &mut Vec<String>) {
        match expr {
            Expr::Identifier(name) => {
                if !self.field_index_map.contains_key(name)
                    && !self.constants.contains_key(name)
                {
                    errors.push(format!(
                        "error: #!exit references unknown variable '{}'\n  note: '{}' is not a state field or a constant",
                        name, name
                    ));
                }
            }
            Expr::Eq(l, r) | Expr::Ne(l, r) | Expr::Lt(l, r) | Expr::Le(l, r)
            | Expr::Gt(l, r) | Expr::Ge(l, r) | Expr::And(l, r) | Expr::Or(l, r) => {
                self.check_exit_condition_idents_inner(l, errors);
                self.check_exit_condition_idents_inner(r, errors);
            }
            Expr::Not(e) => self.check_exit_condition_idents_inner(e, errors),
            _ => {}
        }
    }

    /// Recursively evaluate a boolean expression for the exit condition check.
    /// All values are emitted as `i64` for uniformity; comparisons are zext'd from `i1`.
    fn emit_exit_expr(&mut self, out: &mut String, expr: &Expr, indent: &str) -> String {
        let v = format!("%t{}", self.txn_counter);
        self.txn_counter += 1;
        match expr {
            Expr::Integer(n) => {
                writeln!(out, "{}{} = add i64 0, {}", indent, v, n).ok();
                v
            }
            Expr::Bool(b) => {
                writeln!(out, "{}{} = add i64 0, {}", indent, v, if *b { 1 } else { 0 }).ok();
                v
            }
            Expr::Identifier(name) => {
                if let Some(&idx) = self.field_index_map.get(name) {
                    let p = format!("%gep_exit_{}", self.txn_counter);
                    self.txn_counter += 1;
                    writeln!(out, "{}{} = getelementptr inbounds %State, %State* @global_state, i32 0, i32 {}", indent, p, idx).ok();
                    writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, v, p).ok();
                } else if self.constants.contains_key(name) {
                    writeln!(out, "{}{} = load i64, i64* @{}, align 8", indent, v, name).ok();
                } else {
                    writeln!(out, "{}{} = add i64 0, 0 ; unknown id '{}'", indent, v, name).ok();
                }
                v
            }
            Expr::Eq(l, r) => {
                let lv = self.emit_exit_expr(out, l, indent);
                let rv = self.emit_exit_expr(out, r, indent);
                let cmp = format!("%t{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = icmp eq i64 {}, {}", indent, cmp, lv, rv).ok();
                writeln!(out, "{}{} = zext i1 {} to i64", indent, v, cmp).ok();
                v
            }
            Expr::Ne(l, r) => {
                let lv = self.emit_exit_expr(out, l, indent);
                let rv = self.emit_exit_expr(out, r, indent);
                let cmp = format!("%t{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = icmp ne i64 {}, {}", indent, cmp, lv, rv).ok();
                writeln!(out, "{}{} = zext i1 {} to i64", indent, v, cmp).ok();
                v
            }
            Expr::Lt(l, r) => {
                let lv = self.emit_exit_expr(out, l, indent);
                let rv = self.emit_exit_expr(out, r, indent);
                let cmp = format!("%t{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = icmp slt i64 {}, {}", indent, cmp, lv, rv).ok();
                writeln!(out, "{}{} = zext i1 {} to i64", indent, v, cmp).ok();
                v
            }
            Expr::Le(l, r) => {
                let lv = self.emit_exit_expr(out, l, indent);
                let rv = self.emit_exit_expr(out, r, indent);
                let cmp = format!("%t{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = icmp sle i64 {}, {}", indent, cmp, lv, rv).ok();
                writeln!(out, "{}{} = zext i1 {} to i64", indent, v, cmp).ok();
                v
            }
            Expr::Gt(l, r) => {
                let lv = self.emit_exit_expr(out, l, indent);
                let rv = self.emit_exit_expr(out, r, indent);
                let cmp = format!("%t{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = icmp sgt i64 {}, {}", indent, cmp, lv, rv).ok();
                writeln!(out, "{}{} = zext i1 {} to i64", indent, v, cmp).ok();
                v
            }
            Expr::Ge(l, r) => {
                let lv = self.emit_exit_expr(out, l, indent);
                let rv = self.emit_exit_expr(out, r, indent);
                let cmp = format!("%t{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = icmp sge i64 {}, {}", indent, cmp, lv, rv).ok();
                writeln!(out, "{}{} = zext i1 {} to i64", indent, v, cmp).ok();
                v
            }
            Expr::And(l, r) => {
                let lv = self.emit_exit_expr(out, l, indent);
                let rv = self.emit_exit_expr(out, r, indent);
                writeln!(out, "{}{} = and i64 {}, {}", indent, v, lv, rv).ok();
                v
            }
            Expr::Or(l, r) => {
                let lv = self.emit_exit_expr(out, l, indent);
                let rv = self.emit_exit_expr(out, r, indent);
                writeln!(out, "{}{} = or i64 {}, {}", indent, v, lv, rv).ok();
                v
            }
            Expr::Not(e) => {
                let inner = self.emit_exit_expr(out, e, indent);
                writeln!(out, "{}{} = xor i64 {}, 1", indent, v, inner).ok();
                v
            }
            _ => {
                writeln!(out, "{}{} = add i64 0, 0 ; unsupported exit expr", indent, v).ok();
                v
            }
        }
    }

    // ── MAIN FUNCTION ─────────────────────────────────────────
    fn emit_main(&mut self, out: &mut String, has_wake_triggers: bool) {
        writeln!(out, "define i32 @main() local_unnamed_addr #3 {{").ok();
        writeln!(out, "  entry:").ok();
        writeln!(out, "  call void @init_state()").ok();
        if has_wake_triggers {
            writeln!(out, "  call void @__rt_init()").ok();
        }
        if self.has_async_txns {
            let count = self.async_txn_names.len() as i32;
            writeln!(out, "  %tp_fn_ptr = bitcast [{} x void (%State*)*]* @thread_pool_fns to i8**", self.async_txn_names.len()).ok();
            writeln!(out, "  call void @brief_thread_pool_init(i32 {}, i8** %tp_fn_ptr)", count).ok();
        }
        writeln!(out, "  br label %tick").ok();
        writeln!(out, "  tick:").ok();
        if self.has_async_txns {
            self.emit_async_phase(out);
        } else {
            writeln!(out, "  call void @reactor_tick()").ok();
        }
        let has_exit = self.exit_condition.is_some();
        if has_exit {
            let cond = self.exit_condition.clone().unwrap();
            let val = self.emit_exit_expr(out, &cond, "  ");
            let tr = format!("%t{}", self.txn_counter); self.txn_counter += 1;
            writeln!(out, "  {} = trunc i64 {} to i1", tr, val).ok();
            if has_wake_triggers {
                writeln!(out, "  br i1 {}, label %done, label %wait", tr).ok();
                writeln!(out, "  wait:").ok();
                writeln!(out, "  call void @__rt_wait()").ok();
                writeln!(out, "  br label %tick").ok();
            } else {
                writeln!(out, "  br i1 {}, label %done, label %tick", tr).ok();
            }
            writeln!(out, "  done:").ok();
            writeln!(out, "  ret i32 0").ok();
        } else {
            if has_wake_triggers {
                writeln!(out, "  call void @__rt_wait()").ok();
            }
            writeln!(out, "  br label %tick").ok();
        }
        writeln!(out, "}}").ok();
        writeln!(out).ok();
    }

    /// Emit the folded while-loop body (without `@init_state()` or the enclosing
    /// `define` / `ret`).  Used by both `emit_folded_main` and the enum dispatch path.
    fn emit_folded_loop(
        &self,
        out: &mut String,
        txn_name: &str,
        counter_idx: usize,
        total_idx: Option<usize>,
        total_const_name: Option<&str>,
        label_prefix: &str,
    ) {
        let c0 = self.txn_counter;
        if let Some(ti) = total_idx {
            writeln!(out, "  %gt{}_{} = getelementptr inbounds %State, %State* @global_state, i32 0, i32 {}", label_prefix, c0, ti).ok();
            writeln!(out, "  %lt{}_{} = load i64, i64* %gt{}_{}, align 8", label_prefix, c0, label_prefix, c0).ok();
        } else if let Some(cn) = total_const_name {
            writeln!(out, "  %lt{}_{} = load i64, i64* @{}, align 8", label_prefix, c0, cn).ok();
        } else {
            writeln!(out, "  %lt{}_{} = add i64 0, 0", label_prefix, c0).ok();
        }
        writeln!(out, "  br label %{}_hdr", label_prefix).ok();
        writeln!(out, "{}_hdr:", label_prefix).ok();
        writeln!(out, "  %gp{}_{} = getelementptr inbounds %State, %State* @global_state, i32 0, i32 {}", label_prefix, c0 + 1, counter_idx).ok();
        writeln!(out, "  %lp{}_{} = load i64, i64* %gp{}_{}, align 8", label_prefix, c0 + 1, label_prefix, c0 + 1).ok();
        writeln!(out, "  %cp{}_{} = icmp slt i64 %lp{}_{}, %lt{}_{}", label_prefix, c0 + 2, label_prefix, c0 + 1, label_prefix, c0).ok();
        writeln!(out, "  br i1 %cp{}_{}, label %{}_body, label %{}_done", label_prefix, c0 + 2, label_prefix, label_prefix).ok();
        writeln!(out, "{}_body:", label_prefix).ok();
        writeln!(out, "  call void @{}(%State* @global_state)", txn_name).ok();
        writeln!(out, "  br label %{}_hdr", label_prefix).ok();
        writeln!(out, "{}_done:", label_prefix).ok();
    }

    fn emit_folded_main(
        &self,
        out: &mut String,
        txn_name: &str,
        counter_idx: usize,
        total_idx: Option<usize>,
        total_const_name: Option<&str>,
    ) {
        writeln!(out, "define i32 @main() local_unnamed_addr #0 {{").ok();
        writeln!(out, "  entry:").ok();
        writeln!(out, "  call void @init_state()").ok();
        self.emit_folded_loop(out, txn_name, counter_idx, total_idx, total_const_name, "case");
        writeln!(out, "  ret i32 0").ok();
        writeln!(out, "}}").ok();
        writeln!(out).ok();
    }

    /// Emit a `main()` that samples enumerable triggers once and switch-dispatches
    /// to per-value folded loops.  Each trigger combination gets its own while-loop
    /// that runs the folded transaction body.
    ///
    /// `fold_params` maps txn_name → (counter_idx, total_idx, total_const_name)
    /// for all enum-candidate transactions that have proven bounded convergence.
    /// Each case arm emits one folded loop per entry, allowing multi-txn programs
    /// (e.g. `async_counters`) to converge all counters in a single tick.
    fn emit_enum_main(
        &mut self,
        out: &mut String,
        txns: &[(String, &crate::ast::Transaction)],
        enum_sizes: &[(String, Option<u64>)],
        fold_params: &HashMap<String, (usize, Option<usize>, Option<String>)>,
        counter_idx: usize,
        total_idx: Option<usize>,
        total_const_name: Option<&str>,
        composed_fn: Option<&str>,
        composed_trig_map: Option<&HashMap<String, Vec<(i64, String)>>>,
        all_internal_map: Option<&HashMap<String, (usize, i64)>>,
        has_wake: bool,
    ) {
        // #0 = willreturn, mustprogress (one-shot). #3 = no willreturn, no mustprogress (wake loop).
        let attr = if has_wake { "#3" } else { "#0" };
        writeln!(out, "define i32 @main() local_unnamed_addr {} {{", attr).ok();
        writeln!(out, "  entry:").ok();
        writeln!(out, "  call void @init_state()").ok();
        if has_wake {
            writeln!(out, "  call void @__rt_init()").ok();
        }
        if self.has_async_txns {
            let count = self.async_txn_names.len() as i32;
            writeln!(out, "  %tp_fn_ptr = bitcast [{} x void (%State*)*]* @thread_pool_fns to i8**", self.async_txn_names.len()).ok();
            writeln!(out, "  call void @brief_thread_pool_init(i32 {}, i8** %tp_fn_ptr)", count).ok();
        }
        if has_wake {
            writeln!(out, "  br label %tick").ok();
            writeln!(out, "tick:").ok();
        }

        // Sample triggers (clone trigger data to avoid borrow conflict)
        let trigger_data: Vec<(String, String, bool, crate::ast::Type)> = enum_sizes.iter()
            .filter_map(|(tn, _)| {
                self.triggers.get(tn).map(|t| {
                    let rn = format!("%sz_{}", tn);
                    let (addr_str, addr_is_ptr) = match &t.address {
                        crate::ast::LinkRef::Explicit(a) => (a.to_string(), false),
                        crate::ast::LinkRef::Linked(s) => (format!("@{}", s), true),
                    };
                    (rn, addr_str, addr_is_ptr, t.ty.clone())
                })
            })
            .collect();
        for (rn, addr_str, addr_is_ptr, ty) in &trigger_data {
            self.emit_trg_load(out, "  ", rn, addr_str, *addr_is_ptr, ty);
        }

        // Build switch dispatch
        let txn_name = composed_fn.unwrap_or(
            txns.first().map(|(n, _)| n.as_str()).unwrap_or("__missing")
        );

        // Build per-trigger-value composed function lookup (for chain branching)
        let root_txn = txns.first().map(|(n, _)| n.as_str()).unwrap_or("");
        let mut trig_to_fn: HashMap<i64, String> = HashMap::new();
        if let Some(ctm) = composed_trig_map {
            if let Some(entries) = ctm.get(root_txn) {
                for (val, fname) in entries {
                    trig_to_fn.insert(*val, fname.clone());
                }
            }
        }

        let total_combos: u64 = enum_sizes.iter().map(|(_, s)| s.unwrap_or(1)).product();

        // Helper: check if a function name maps to an all-internal
        // (pure counter) case and return its (ci, total_val) if so.
        let all_internal_lookup = |fn_name: &str| -> Option<(usize, i64)> {
            all_internal_map.and_then(|m| m.get(fn_name).copied())
        };

        // "Done" label for each branch — in wake mode this is either exit_check
        // (when #!exit is declared), async_phase (when async txns exist), or do_wait.
        // In one-shot mode this is "exit" (ret i32 0).
        // All case arms branch to done_label; done_label routes through the
        // exit condition check (if present) before reaching the wait loop.
        let done_label = if has_wake {
            if self.exit_condition.is_some() { "exit_check" }
            else if self.has_async_txns { "async_phase" }
            else { "do_wait" }
        } else { "exit" };
        if !has_wake { writeln!(out, "  br label %dispatch").ok(); writeln!(out, "dispatch:").ok(); }

        /// Emit one or more per-txn folded loops for a case arm.
        /// When fold_params contains entries, emits one loop per entry;
        /// otherwise falls back to the legacy single-txn params.
        let emit_case_folded_loops = |this: &mut LlvmBackend,
                                      out: &mut String,
                                      prefix: &str,
                                      fn_name: &str,
                                      ci: usize,
                                      ti: Option<usize>,
                                      tcn: Option<&str>|
        {
            if !fold_params.is_empty() {
                // Multi-txn: emit one folded loop per bounded-counter txn
                for (ptxn_name, &(pci, pti, ref ptcn)) in fold_params.iter() {
                    let sub_prefix = format!("{}_{}", prefix, ptxn_name);
                    let ptcn_ref = ptcn.as_deref();
                    this.emit_folded_loop(out, ptxn_name, pci, pti, ptcn_ref, &sub_prefix);
                }
            } else {
                // Single-txn (legacy): use the caller-provided params
                this.emit_folded_loop(out, fn_name, ci, ti, tcn, prefix);
            }
        };

        if total_combos == 1 && enum_sizes.len() == 1 {
            // Single-value trigger: just fall through to the loop
            let fn_name = trig_to_fn.get(&0).map(|s| s.as_str()).unwrap_or(txn_name);
            if let Some((ci, tv)) = all_internal_lookup(fn_name) {
                writeln!(out, "  %pc_sc = getelementptr inbounds %State, %State* @global_state, i32 0, i32 {}", ci).ok();
                writeln!(out, "  store i64 {}, i64* %pc_sc, align 8", tv).ok();
            } else {
                emit_case_folded_loops(self, out, "sc", fn_name, counter_idx, total_idx, total_const_name);
            }
            if has_wake {
                writeln!(out, "  br label %{}", done_label).ok();
            } else {
                writeln!(out, "  ret i32 0").ok();
            }
        } else if enum_sizes.len() == 1 {
            // Single enumerable trigger — one switch axis
            let tn = &enum_sizes[0].0;
            let n = enum_sizes[0].1.unwrap_or(2);
            let native_name = txn_name.to_string();
            writeln!(out, "  switch i64 %sz_{}, label %{}_residual [", tn, tn).ok();
            for val in 0..n as i64 {
                writeln!(out, "    i64 {}, label %{}_case_{}", val, tn, val).ok();
            }
            writeln!(out, "  ]").ok();
            for val in 0..n as i64 {
                let prefix = format!("{}_{}", tn, val);
                let fn_name = trig_to_fn.get(&val).map(|s| s.as_str()).unwrap_or(&native_name);
                writeln!(out, "{}_case_{}:", tn, val).ok();
                if let Some((ci, tv)) = all_internal_lookup(fn_name) {
                    writeln!(out, "  %pc_{} = getelementptr inbounds %State, %State* @global_state, i32 0, i32 {}", prefix, ci).ok();
                    writeln!(out, "  store i64 {}, i64* %pc_{}, align 8", tv, prefix).ok();
                } else {
                    emit_case_folded_loops(self, out, &prefix, fn_name, counter_idx, total_idx, total_const_name);
                }
                if has_wake {
                    writeln!(out, "  br label %{}", done_label).ok();
                } else {
                    writeln!(out, "  ret i32 0").ok();
                }
            }
            writeln!(out, "{}_residual:", tn).ok();
            writeln!(out, "  call void @reactor_tick()").ok();
            if has_wake {
                writeln!(out, "  br label %{}", done_label).ok();
            } else {
                writeln!(out, "  br label %{}_residual_loop", tn).ok();
                writeln!(out, "{}_residual_loop:", tn).ok();
                writeln!(out, "  call void @reactor_tick()").ok();
                writeln!(out, "  br label %{}_residual_loop", tn).ok();
            }
        } else {
            // Multi-trigger case: just fall through to standard reactor
            if has_wake {
                writeln!(out, "  call void @reactor_tick()").ok();
                writeln!(out, "  br label %{}", done_label).ok();
            } else {
                writeln!(out, "  br label %residual_entry").ok();
                writeln!(out, "residual_entry:").ok();
                writeln!(out, "  call void @init_state()").ok();
                writeln!(out, "  br label %residual_loop").ok();
                writeln!(out, "residual_loop:").ok();
                writeln!(out, "  call void @reactor_tick()").ok();
                writeln!(out, "  br label %residual_loop").ok();
            }
        }

        if has_wake {
            let has_exit = self.exit_condition.is_some();
            if has_exit {
                let cond = self.exit_condition.clone().unwrap();
                writeln!(out, "exit_check:").ok();
                let val = self.emit_exit_expr(out, &cond, "  ");
                let tr = format!("%t{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "  {} = trunc i64 {} to i1", tr, val).ok();
                if self.has_async_txns {
                    writeln!(out, "  br i1 {}, label %done, label %async_phase", tr).ok();
                } else {
                    writeln!(out, "  br i1 {}, label %done, label %do_wait", tr).ok();
                }
            }
            if self.has_async_txns {
                writeln!(out, "async_phase:").ok();
                self.emit_async_phase(out);
                writeln!(out, "  br label %do_wait").ok();
            }
            writeln!(out, "do_wait:").ok();
            writeln!(out, "  call void @__rt_wait()").ok();
            writeln!(out, "  br label %tick").ok();
            if has_exit {
                writeln!(out, "done:").ok();
                writeln!(out, "  ret i32 0").ok();
            }
        }

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

    fn emit_precomputed_main(
        &self,
        out: &mut String,
        final_values: &[(Vec<String>, std::collections::HashMap<String, i64>)],
    ) {
        writeln!(out, "define i32 @main() local_unnamed_addr #0 {{").ok();
        writeln!(out, "  entry:").ok();
        writeln!(out, "  call void @init_state()").ok();
        let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for (_, bindings) in final_values {
            for (var, val) in bindings {
                if !seen.insert(var) { continue; }
                if let Some(&idx) = self.field_index_map.get(var) {
                    writeln!(out, "  %gp_{} = getelementptr inbounds %State, %State* @global_state, i32 0, i32 {}", var, idx).ok();
                    writeln!(out, "  store i64 {}, i64* %gp_{}, align 8", val, var).ok();
                }
            }
        }
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
        writeln!(out, "@llvm.wake_triggers = constant [{} x i8*] [{}]", count, sym_list).ok();
        writeln!(out, "!llvm.wake_triggers = !{{!0}}").ok();
        write!(out, "!0 = !{{").ok();
        for (i, sym) in wake_symbols.iter().enumerate() {
            if i > 0 { write!(out, ", ").ok(); }
            write!(out, "!\"{}\"", sym).ok();
        }
        writeln!(out, "}}").ok();
    }

    // ── THREAD POOL METADATA ────────────────────────────────
    fn emit_thread_pool_metadata(&self, out: &mut String) {
        if !self.has_async_txns { return; }
        let count = self.async_txn_names.len();
        let fn_list: Vec<String> = self.async_txn_names.iter()
            .map(|n| format!("i8* bitcast (void (%State*)* @async_body_{} to i8*)", n))
            .collect();
        writeln!(out, "@llvm.thread_pool = constant [{} x i8*] [{}]",
            count, fn_list.join(", ")).ok();
        // Emit a packed array of function pointers for brief_thread_pool_init
        writeln!(out, "@thread_pool_fns = private constant [{} x void (%State*)*] [{}]",
            count,
            self.async_txn_names.iter()
                .map(|n| format!("void (%State*)* @async_body_{}", n))
                .collect::<Vec<_>>().join(", "),
        ).ok();
    }

    /// Emit the async phase calls in main: release workers, run sequential
    /// reactor, wait for workers. Used by emit_main and emit_enum_main.
    fn emit_async_phase(&self, out: &mut String) {
        if !self.has_async_txns { return; }
        writeln!(out, "  call void @brief_barrier_release()").ok();
        // Sequential reactor runs in main thread concurrently with workers
        writeln!(out, "  call void @reactor_tick()").ok();
        writeln!(out, "  call void @brief_barrier_wait()").ok();
    }

    // ── FUSABLE PAIRS ────────────────────────────────────────
    fn resolve_fusable_pairs(&self, txns: &[(String, &crate::ast::Transaction)]) -> Vec<(String, String)> {
        let prg = crate::ast::Program {
            items: txns.iter().map(|(_, t)| crate::ast::TopLevel::Transaction((*t).clone())).collect(),
            comments: vec![], reactor_speed: None, attrs: vec![], ffi: None, strict_mode: crate::ast::StrictMode::Off, dispatch_mode: crate::ast::DispatchMode::Sequential, exit_condition: None,
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
                } else if let Some((ty, _)) = self.constants.get(name.as_str()) {
                    Some(ty.clone())
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
                } else if let Some((ty, _)) = self.constants.get(name) {
                    *ty == Type::Float
                } else {
                    false
                }
            }
            Expr::OwnedRef(name) => {
                if let Some(reg) = self.let_bindings.get(name.as_str()) {
                    self.register_types.get(reg) == Some(&Type::Float)
                } else if let Some(&idx) = self.field_index_map.get(name.as_str()) {
                    self.field_types[idx] == "float"
                } else if let Some((ty, _)) = self.constants.get(name.as_str()) {
                    *ty == Type::Float
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
            exit_condition: None,
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
            exit_condition: None,
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
            exit_condition: None,
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
            exit_condition: None,
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
            exit_condition: None,
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
            exit_condition: None,
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
            exit_condition: None,
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
            exit_condition: None,
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
            exit_condition: None,
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
        assert!(output.contains("@llvm.wake_triggers = constant [1 x i8*] [i8* @__sigint_flag]"),
            "Single wake trigger → constant global with one symbol");
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
        // Use Int trigger (non-enumerable) to force standard reactor path
        let program = make_wake_trg_program("sig", "__sigint_flag", Type::Int, true);
        let output = LlvmBackend::new().generate(&program);
        assert!(output.contains("call void @__rt_init()"),
            "main() calls __rt_init() when wake triggers exist");
        assert!(output.contains("call void @__rt_wait()"),
            "main() calls __rt_wait() after reactor_tick");
    }

    #[test]
    fn test_enum_with_wake_triggers_hybrid() {
        // Bool trigger with is_wake → enters enum dispatch in hybrid wake mode.
        // Previously this bypassed enum entirely (Phase A gate). Now enum dispatch
        // is active, with @__rt_init()/__rt_wait() wrapping the switch arms.
        let program = make_wake_trg_program("sig", "__sigint_flag", Type::Bool, true);
        let output = LlvmBackend::new().generate(&program);
        assert!(output.contains("call void @__rt_wait()"),
            "Wake triggers get __rt_wait between ticks");
        assert!(output.contains("call void @__rt_init()"),
            "Wake triggers get __rt_init at startup");
        assert!(output.contains("switch i64"),
            "Enum dispatch IS used with wake triggers (hybrid mode — switch arms loop back via __rt_wait)");
        assert!(output.contains("load volatile"),
            "Triggers are volatile-loaded for sampling");
        assert!(output.contains("define i32 @main() local_unnamed_addr #3"),
            "Wake hybrid uses #3 attribute (no willreturn, no mustprogress) for infinite tick loop");
    }

    #[test]
    fn test_main_no_init_wait_without_wake_triggers() {
        // Use Int trigger (non-enumerable) to force standard reactor path
        let program = make_wake_trg_program("sig", "__sigint_flag", Type::Int, false);
        let output = LlvmBackend::new().generate(&program);
        assert!(!output.contains("call void @__rt_init()"),
            "main() does not call __rt_init() without wake triggers");
        assert!(!output.contains("call void @__rt_wait()"),
            "main() does not call __rt_wait() without wake triggers");
    }

    #[test]
    fn test_rt_declares_present() {
        // Use Int trigger (non-enumerable) to force standard reactor path
        let program = make_wake_trg_program("sig", "__sigint_flag", Type::Int, false);
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
            exit_condition: None,
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
            exit_condition: None,
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
            exit_condition: None,
        };
        let output = backend.generate(&program);
        assert!(output.contains("fadd float"),
            "Float binary add should emit fadd float");
    }

    #[test]
    fn test_main_and_reactor_use_non_willreturn_attr() {
        let program = make_wake_trg_program("sig", "__sigint_flag", Type::Int, true);
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

    // ── Integration: optimization report & chain composition ──

    fn make_chain_program(
        txns: Vec<(&str, Vec<Statement>)>,
        trigger: Option<(&str, Type)>,
        consts: &[(&str, i64)],
        states: &[(&str, i64)],
    ) -> Program {
        let mut items: Vec<TopLevel> = Vec::new();
        for (name, val) in consts {
            items.push(TopLevel::Constant(Constant {
                name: name.to_string(),
                ty: Type::Int,
                expr: Expr::Integer(*val),
            }));
        }
        for (name, val) in states {
            items.push(TopLevel::StateDecl(StateDecl {
                name: name.to_string(),
                ty: Type::Int,
                expr: Some(Expr::Integer(*val)),
                address: None, bit_range: None, is_override: false,
                os_mode: false, span: None, attrs: vec![],
            }));
        }
        if let Some((trg_name, trg_ty)) = trigger {
            items.push(TopLevel::Trigger(TriggerDeclaration {
                name: trg_name.to_string(), ty: trg_ty,
                address: LinkRef::Explicit(0), bit_range: None,
                stages: vec![], condition: None, is_wake: false, span: None,
            }));
        }
        for (txn_name, body) in txns {
            let pre = Expr::Lt(
                Box::new(Expr::Identifier("count".to_string())),
                Box::new(Expr::Identifier("total".to_string())),
            );
            items.push(TopLevel::Transaction(Transaction {
                name: txn_name.to_string(), parameters: vec![],
                contract: Contract {
                    pre_condition: pre,
                    post_condition: Expr::Bool(true),
                    span: None, watchdog: None,
                },
                body, is_async: false, is_reactive: true, reactor_speed: None,
                span: None, is_lambda: false, dependencies: vec![],
                attrs: vec![], modifiers: vec![], variant_bodies: vec![],
            }));
        }
        Program {
            items, comments: vec![], reactor_speed: None, attrs: Vec::new(),
            ffi: None, strict_mode: StrictMode::Off,
            dispatch_mode: Default::default(),
            exit_condition: None,
        }
    }

    fn ident_s(s: &str) -> Expr { Expr::Identifier(s.to_string()) }
    fn int_s(v: i64) -> Expr { Expr::Integer(v) }

    #[test]
    fn test_report_shows_ranking() {
        let program = make_chain_program(
            vec![("t1", vec![
                Statement::Assignment { lhs: ident_s("x"), expr: ident_s("sensor"), timeout: None, modifiers: vec![] },
                Statement::Assignment { lhs: ident_s("count"), expr: Expr::Add(Box::new(ident_s("count")), Box::new(int_s(1))), timeout: None, modifiers: vec![] },
            ])],
            Some(("sensor", Type::Bool)),
            &[("total", 100)], &[("count", 0), ("x", 0)],
        );
        let mut backend = LlvmBackend::new()
            .with_optimize_budget(256).with_optimize_report(true);
        let _output = backend.generate(&program);
        let report: Vec<&str> = backend.report().iter().map(|s| s.as_str()).collect();
        let joined = report.join("\n");
        assert!(joined.contains("Optimization priority ranking"),
            "Report should contain priority ranking section");
    }

    #[test]
    fn test_report_shows_budget() {
        let program = make_chain_program(
            vec![("t1", vec![
                Statement::Assignment { lhs: ident_s("x"), expr: ident_s("sensor"), timeout: None, modifiers: vec![] },
                Statement::Assignment { lhs: ident_s("count"), expr: Expr::Add(Box::new(ident_s("count")), Box::new(int_s(1))), timeout: None, modifiers: vec![] },
            ])],
            Some(("sensor", Type::Bool)),
            &[("total", 100)], &[("count", 0), ("x", 0)],
        );
        let mut backend = LlvmBackend::new()
            .with_optimize_budget(10).with_optimize_report(true);
        let _output = backend.generate(&program);
        let report: Vec<&str> = backend.report().iter().map(|s| s.as_str()).collect();
        let joined = report.join("\n");
        assert!(joined.contains("Budget plan"),
            "Report should contain budget plan section");
    }

    #[test]
    fn test_report_shows_size() {
        let program = make_chain_program(
            vec![("t1", vec![
                Statement::Assignment { lhs: ident_s("x"), expr: ident_s("sensor"), timeout: None, modifiers: vec![] },
                Statement::Assignment { lhs: ident_s("count"), expr: Expr::Add(Box::new(ident_s("count")), Box::new(int_s(1))), timeout: None, modifiers: vec![] },
            ])],
            Some(("sensor", Type::Bool)),
            &[("total", 100)], &[("count", 0), ("x", 0)],
        );
        let mut backend = LlvmBackend::new()
            .with_optimize_budget(256).with_optimize_report(true)
            .with_optimize_size(10000);
        let _output = backend.generate(&program);
        let report: Vec<&str> = backend.report().iter().map(|s| s.as_str()).collect();
        let joined = report.join("\n");
        assert!(joined.contains("Size estimation") || joined.contains("Base binary"),
            "Report should contain size estimation section");
    }

    #[test]
    fn test_report_shows_chains() {
        let program = make_chain_program(
            vec![
                ("step_a", vec![
                    Statement::Assignment { lhs: ident_s("x"), expr: ident_s("sensor"), timeout: None, modifiers: vec![] },
                    Statement::Assignment { lhs: ident_s("count"), expr: Expr::Add(Box::new(ident_s("count")), Box::new(int_s(1))), timeout: None, modifiers: vec![] },
                ]),
                ("step_b", vec![
                    Statement::Assignment { lhs: ident_s("y"), expr: Expr::Add(Box::new(ident_s("x")), Box::new(int_s(1))), timeout: None, modifiers: vec![] },
                    Statement::Assignment { lhs: ident_s("count"), expr: Expr::Add(Box::new(ident_s("count")), Box::new(int_s(1))), timeout: None, modifiers: vec![] },
                ]),
            ],
            Some(("sensor", Type::Bool)),
            &[("total", 100)],
            &[("count", 0), ("x", 0), ("y", 0)],
        );
        let mut backend = LlvmBackend::new()
            .with_optimize_budget(256).with_optimize_report(true);
        let _output = backend.generate(&program);
        let report: Vec<&str> = backend.report().iter().map(|s| s.as_str()).collect();
        let joined = report.join("\n");
        assert!(joined.contains("Linear transaction chains")
            || joined.contains("Composed chains"),
            "Report should detect multi-txn chains");
    }

    #[test]
    fn test_enum_with_composed_chain() {
        let program = make_chain_program(
            vec![
                ("step_a", vec![
                    Statement::Assignment { lhs: ident_s("x"), expr: ident_s("sensor"), timeout: None, modifiers: vec![] },
                    Statement::Assignment { lhs: ident_s("count"), expr: Expr::Add(Box::new(ident_s("count")), Box::new(int_s(1))), timeout: None, modifiers: vec![] },
                ]),
                ("step_b", vec![
                    Statement::Assignment { lhs: ident_s("y"), expr: Expr::Add(Box::new(ident_s("x")), Box::new(int_s(1))), timeout: None, modifiers: vec![] },
                    Statement::Assignment { lhs: ident_s("count"), expr: Expr::Add(Box::new(ident_s("count")), Box::new(int_s(1))), timeout: None, modifiers: vec![] },
                ]),
            ],
            Some(("sensor", Type::Bool)),
            &[("total", 100)],
            &[("count", 0), ("x", 0), ("y", 0)],
        );
        let output = LlvmBackend::new().with_optimize_budget(256).generate(&program);
        // All-internal chains skip fused fn emission; pure counter store
        // is emitted directly in the per-case switch arm.
        assert!(output.contains("switch i64"),
            "Should emit switch dispatch for enumerable trigger");
        assert!(output.contains("@main"),
            "Should emit main function with enum dispatch");
    }

    #[test]
    fn test_all_internal_pure_counter_emitted() {
        let program = make_chain_program(
            vec![
                ("step_a", vec![
                    Statement::Assignment { lhs: ident_s("_trig"), expr: ident_s("sensor"), timeout: None, modifiers: vec![] },
                    Statement::Assignment { lhs: ident_s("internal"), expr: int_s(42), timeout: None, modifiers: vec![] },
                    Statement::Assignment { lhs: ident_s("count"), expr: Expr::Add(Box::new(ident_s("count")), Box::new(int_s(1))), timeout: None, modifiers: vec![] },
                ]),
                ("step_b", vec![
                    Statement::Assignment { lhs: ident_s("result"), expr: Expr::Add(Box::new(ident_s("internal")), Box::new(int_s(1))), timeout: None, modifiers: vec![] },
                    Statement::Assignment { lhs: ident_s("count"), expr: Expr::Add(Box::new(ident_s("count")), Box::new(int_s(1))), timeout: None, modifiers: vec![] },
                ]),
            ],
            Some(("sensor", Type::Bool)),
            &[("total", 100)],
            &[("count", 0), ("internal", 0), ("result", 0), ("_trig", 0)],
        );
        let output = LlvmBackend::new().with_optimize_budget(256).generate(&program);
        assert!(output.contains("@main"),
            "Should emit main function");
    }

    #[test]
    fn test_precompute_pure_counter() {
        let program = make_chain_program(
            vec![
                ("step_a", vec![
                    Statement::Assignment { lhs: ident_s("x"), expr: int_s(42), timeout: None, modifiers: vec![] },
                    Statement::Assignment { lhs: ident_s("count"), expr: Expr::Add(Box::new(ident_s("count")), Box::new(int_s(1))), timeout: None, modifiers: vec![] },
                ]),
                ("step_b", vec![
                    Statement::Assignment { lhs: ident_s("y"), expr: Expr::Add(Box::new(ident_s("x")), Box::new(int_s(1))), timeout: None, modifiers: vec![] },
                    Statement::Assignment { lhs: ident_s("count"), expr: Expr::Add(Box::new(ident_s("count")), Box::new(int_s(1))), timeout: None, modifiers: vec![] },
                ]),
            ],
            None,
            &[("total", 100)],
            &[("count", 0), ("x", 0), ("y", 0)],
        );
        let output = LlvmBackend::new().with_optimize_budget(256).generate(&program);
        assert!(output.contains("call void @init_state()"),
            "Should call init_state");
        assert!(!output.contains("switch i64"),
            "No enum dispatch for precomputed path");
        assert!(!output.contains("@reactor_tick"),
            "No reactor_tick for precomputed path");
        assert!(output.contains("ret i32 0"),
            "Should return normally");
    }

    #[test]
    fn test_precompute_budget_exceeded_fallback() {
        let program = make_chain_program(
            vec![
                ("step_a", vec![
                    Statement::Assignment { lhs: ident_s("x"), expr: int_s(42), timeout: None, modifiers: vec![] },
                    Statement::Assignment { lhs: ident_s("count"), expr: Expr::Add(Box::new(ident_s("count")), Box::new(int_s(1))), timeout: None, modifiers: vec![] },
                ]),
                ("step_b", vec![
                    Statement::Assignment { lhs: ident_s("y"), expr: Expr::Add(Box::new(ident_s("x")), Box::new(int_s(1))), timeout: None, modifiers: vec![] },
                    Statement::Assignment { lhs: ident_s("count"), expr: Expr::Add(Box::new(ident_s("count")), Box::new(int_s(1))), timeout: None, modifiers: vec![] },
                ]),
            ],
            None,
            &[("total", 100)],
            &[("count", 0), ("x", 0), ("y", 0)],
        );
        let output = LlvmBackend::new().with_optimize_budget(0).generate(&program);
        assert!(!output.contains("switch i64"),
            "No enum dispatch without triggers");
        assert!(output.contains("@reactor_tick"),
            "Falls back to reactor_tick when budget exceeded");
    }

    #[test]
    fn test_iir_filter_folded_path_regression() {
        let program = make_chain_program(
            vec![("process", vec![
                Statement::Assignment { lhs: ident_s("x"), expr: int_s(42), timeout: None, modifiers: vec![] },
                Statement::Assignment { lhs: ident_s("count"), expr: Expr::Add(Box::new(ident_s("count")), Box::new(int_s(1))), timeout: None, modifiers: vec![] },
            ])],
            None,
            &[("total", 50000000)],
            &[("count", 0), ("x", 0)],
        );
        let output = LlvmBackend::new().generate(&program);
        assert!(!output.contains("switch i64"),
            "Single-txn convergence should use folded path, not enum dispatch");
        assert!(!output.contains("@reactor_tick"),
            "Single-txn convergence should use folded path, not standard reactor");
        assert!(output.contains("icmp slt i64"),
            "Folded main should contain counter comparison");
        assert!(output.contains("br label"),
            "Folded main should contain while-loop branches");
        assert!(output.contains("ret i32 0"),
            "Should return normally after loop");
    }

    fn make_async_pair_program() -> Program {
        Program {
            items: vec![
                TopLevel::StateDecl(StateDecl {
                    name: "a".to_string(),
                    ty: Type::Int,
                    expr: Some(int_s(0)),
                    address: None, bit_range: None, is_override: false,
                    os_mode: false, span: None, attrs: vec![],
                }),
                TopLevel::StateDecl(StateDecl {
                    name: "b".to_string(),
                    ty: Type::Int,
                    expr: Some(int_s(0)),
                    address: None, bit_range: None, is_override: false,
                    os_mode: false, span: None, attrs: vec![],
                }),
                TopLevel::Transaction(Transaction {
                    name: "inc_a".to_string(),
                    parameters: vec![],
                    contract: Contract {
                        pre_condition: Expr::Bool(true),
                        post_condition: Expr::Bool(true),
                        span: None,
                        watchdog: None,
                    },
                    body: vec![
                        Statement::Assignment {
                            lhs: Expr::OwnedRef("a".to_string()),
                            expr: Expr::Add(Box::new(ident_s("a")), Box::new(int_s(1))),
                            timeout: None,
                            modifiers: vec![],
                        },
                        Statement::Term { values: vec![], modifiers: vec![] },
                    ],
                    is_async: true,
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
                    name: "inc_b".to_string(),
                    parameters: vec![],
                    contract: Contract {
                        pre_condition: Expr::Bool(true),
                        post_condition: Expr::Bool(true),
                        span: None,
                        watchdog: None,
                    },
                    body: vec![
                        Statement::Assignment {
                            lhs: Expr::OwnedRef("b".to_string()),
                            expr: Expr::Add(Box::new(ident_s("b")), Box::new(int_s(1))),
                            timeout: None,
                            modifiers: vec![],
                        },
                        Statement::Term { values: vec![], modifiers: vec![] },
                    ],
                    is_async: true,
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
            exit_condition: None,
        }
    }

    #[test]
    fn test_async_body_functions_emitted() {
        let program = make_async_pair_program();
        let output = LlvmBackend::new().generate(&program);
        assert!(output.contains("@async_body_inc_a"),
            "Async body function for inc_a should be emitted");
        assert!(output.contains("@async_body_inc_b"),
            "Async body function for inc_b should be emitted");
    }

    #[test]
    fn test_thread_pool_metadata_emitted() {
        let program = make_async_pair_program();
        let output = LlvmBackend::new().generate(&program);
        assert!(output.contains("@llvm.thread_pool"),
            "Thread pool metadata should be emitted for async txns");
        assert!(output.contains("@thread_pool_fns"),
            "Thread pool function pointer array should be emitted");
    }

    #[test]
    fn test_async_barrier_calls_in_main() {
        let program = make_async_pair_program();
        let output = LlvmBackend::new().generate(&program);
        assert!(output.contains("call void @brief_thread_pool_init"),
            "Main should call thread_pool_init");
        assert!(output.contains("call void @brief_barrier_release"),
            "Main should call barrier_release");
        assert!(output.contains("call void @brief_barrier_wait"),
            "Main should call barrier_wait");
    }

    #[test]
    fn test_no_thread_pool_without_async_txns() {
        let program = make_wake_trg_program("sig", "__sigint_flag", Type::Bool, false);
        let output = LlvmBackend::new().generate(&program);
        assert!(!output.contains("@llvm.thread_pool"),
            "No thread pool metadata without async txns");
        assert!(!output.contains("call void @brief_barrier"),
            "No barrier calls without async txns");
        assert!(!output.contains("call void @brief_thread_pool_init"),
            "No thread pool init without async txns");
    }

    // ── Exit condition tests ──────────────────────────────────

    fn make_exit_program(exit_expr: Option<Expr>, trg_ty: Type, is_wake: bool) -> Program {
        let trg_name = "io_pending";
        let mut items = vec![
            TopLevel::StateDecl(StateDecl {
                name: "ops".to_string(),
                ty: Type::Int,
                expr: Some(int_s(0)),
                address: None, bit_range: None, is_override: false,
                os_mode: false, span: None, attrs: vec![],
            }),
        ];
        items.push(TopLevel::Constant(Constant {
            name: "N".to_string(),
            ty: Type::Int,
            expr: int_s(100),
        }));
        items.push(TopLevel::Trigger(TriggerDeclaration {
            name: trg_name.to_string(),
            ty: trg_ty,
            address: LinkRef::Linked("__io_pending".to_string()),
            bit_range: None, stages: vec![], condition: None,
            is_wake, span: None,
        }));
        let pre = Expr::And(
            Box::new(Expr::Identifier(trg_name.to_string())),
            Box::new(Expr::Lt(
                Box::new(Expr::Identifier("ops".to_string())),
                Box::new(Expr::Identifier("N".to_string())),
            )),
        );
        items.push(TopLevel::Transaction(Transaction {
            name: "work".to_string(),
            parameters: vec![],
            contract: Contract {
                pre_condition: pre,
                post_condition: Expr::Bool(true),
                span: None, watchdog: None,
            },
            body: vec![
                Statement::Assignment {
                    lhs: Expr::OwnedRef("ops".to_string()),
                    expr: Expr::Add(Box::new(ident_s("ops")), Box::new(int_s(1))),
                    timeout: None, modifiers: vec![],
                },
                Statement::Term { values: vec![], modifiers: vec![] },
            ],
            is_async: false, is_reactive: true, reactor_speed: None,
            span: None, is_lambda: false, dependencies: vec![],
            attrs: vec![], modifiers: vec![], variant_bodies: vec![],
        }));
        Program {
            items,
            comments: vec![], reactor_speed: None, attrs: vec![],
            ffi: None, strict_mode: StrictMode::Off,
            dispatch_mode: Default::default(),
            exit_condition: exit_expr.map(Box::new),
        }
    }

    #[test]
    fn test_exit_pragma_in_wake_main() {
        // #!exit ops == N; with Int trigger (standard reactor path)
        let exit_cond = Expr::Eq(
            Box::new(Expr::Identifier("ops".to_string())),
            Box::new(Expr::Identifier("N".to_string())),
        );
        let program = make_exit_program(Some(exit_cond), Type::Int, true);
        let output = LlvmBackend::new().generate(&program);
        // Exit check should appear before __rt_wait
        assert!(output.contains("trunc i64"),
            "Exit condition should trunc i64 to i1");
        assert!(output.contains("br i1"),
            "Exit condition should branch on icmp result");
        assert!(output.contains("done:"),
            "Exit condition should emit done label");
        assert!(output.contains("wait:"),
            "Wake main should emit wait label after exit check");
        assert!(output.contains("ret i32 0"),
            "done label should return 0");
    }

    #[test]
    fn test_exit_pragma_without_wake_no_change() {
        // #!exit ops == N; with Int trigger but is_wake=false → no __rt_wait
        let exit_cond = Expr::Eq(
            Box::new(Expr::Identifier("ops".to_string())),
            Box::new(Expr::Identifier("N".to_string())),
        );
        let program = make_exit_program(Some(exit_cond), Type::Int, false);
        let output = LlvmBackend::new().generate(&program);
        // Exit check still emitted, but no wait label
        assert!(output.contains("trunc i64"),
            "Exit condition should trunc i64 to i1 even without wake");
        assert!(output.contains("br i1"),
            "Exit condition should branch");
        assert!(output.contains("done:"),
            "Exit condition should emit done label");
        assert!(!output.contains("wait:"),
            "No wait label without wake triggers");
        assert!(output.contains("ret i32 0"),
            "done label should return 0");
    }

    #[test]
    fn test_no_exit_without_pragma() {
        let program = make_exit_program(None, Type::Int, true);
        let output = LlvmBackend::new().generate(&program);
        assert!(!output.contains("trunc i64"),
            "No trunc without exit condition");
        assert!(!output.contains("done:"),
            "No done label without exit condition");
    }

    #[test]
    fn test_exit_in_enum_main() {
        // Bool trigger → enum dispatch path, no wake → one-shot: exit check not applicable
        let exit_cond = Expr::Eq(
            Box::new(Expr::Identifier("ops".to_string())),
            Box::new(Expr::Identifier("N".to_string())),
        );
        let program = make_exit_program(Some(exit_cond), Type::Bool, false);
        let output = LlvmBackend::new().with_optimize_budget(256).generate(&program);
        // One-shot enum dispatch: no tick loop, no exit check needed
        assert!(output.contains("switch i64"),
            "Bool trigger should use enum dispatch");
        assert!(output.contains("ret i32 0"),
            "One-shot path returns 0 at each case arm");
        assert!(!output.contains("exit_check:"),
            "No exit check label in one-shot path (no tick loop)");
    }

    #[test]
    fn test_exit_in_enum_hybrid_wake() {
        // Bool trigger with is_wake → hybrid path (enum + wake)
        let exit_cond = Expr::Eq(
            Box::new(Expr::Identifier("ops".to_string())),
            Box::new(Expr::Identifier("N".to_string())),
        );
        let program = make_exit_program(Some(exit_cond), Type::Bool, true);
        let output = LlvmBackend::new().with_optimize_budget(256).generate(&program);
        assert!(output.contains("switch i64"),
            "Bool trigger should use enum dispatch in hybrid mode");
        assert!(output.contains("exit_check:"),
            "Hybrid mode should emit exit_check label");
        assert!(output.contains("do_wait:"),
            "Hybrid mode should still have do_wait for wake path");
        assert!(output.contains("call void @__rt_wait()"),
            "Hybrid mode should have __rt_wait");
        assert!(output.contains("ret i32 0"),
            "Should return 0 on exit");
    }

    // ── Exit diagnostic tests ──────────────────────────────────

    #[test]
    fn test_check_exit_condition_idents_valid() {
        // Known identifiers (state field + constant) should produce no errors
        let mut backend = LlvmBackend::new();
        backend.field_index_map.insert("ops".to_string(), 0);
        backend.constants.insert("N".to_string(), (Type::Int, Expr::Integer(100)));

        let expr = Expr::Eq(
            Box::new(Expr::Identifier("ops".to_string())),
            Box::new(Expr::Identifier("N".to_string())),
        );
        let errors = backend.check_exit_condition_idents(&expr);
        assert!(errors.is_empty(),
            "No errors for known identifiers: {:?}", errors);
    }

    #[test]
    fn test_check_exit_condition_idents_invalid() {
        // Unknown identifier should produce an error
        let mut backend = LlvmBackend::new();
        backend.field_index_map.insert("ops".to_string(), 0);
        backend.constants.insert("N".to_string(), (Type::Int, Expr::Integer(100)));

        let expr = Expr::Eq(
            Box::new(Expr::Identifier("ops".to_string())),
            Box::new(Expr::Identifier("bogus_var".to_string())),
        );
        let errors = backend.check_exit_condition_idents(&expr);
        assert!(!errors.is_empty(),
            "Should report error for unknown identifier");
        assert!(errors[0].contains("bogus_var"),
            "Error should reference the unknown name: {}", errors[0]);
    }

    #[test]
    fn test_one_shot_exit_warning_enum() {
        // Bool trigger without wake → enum dispatch → one-shot → warning
        let exit_cond = Expr::Eq(
            Box::new(Expr::Identifier("ops".to_string())),
            Box::new(Expr::Identifier("N".to_string())),
        );
        let program = make_exit_program(Some(exit_cond), Type::Bool, false);
        let mut backend = LlvmBackend::new().with_optimize_budget(256);
        let _output = backend.generate(&program);
        let has_warning = backend.warnings().iter().any(|w| {
            w.contains("#!exit declared but program has no tick loop")
        });
        assert!(has_warning,
            "Expected one-shot warning for enum dispatch with #!exit");
    }

    #[test]
    fn test_no_one_shot_warning_in_wake_main() {
        // Int trigger with wake → standard reactor → checks exit → no warning
        let exit_cond = Expr::Eq(
            Box::new(Expr::Identifier("ops".to_string())),
            Box::new(Expr::Identifier("N".to_string())),
        );
        let program = make_exit_program(Some(exit_cond), Type::Int, true);
        let mut backend = LlvmBackend::new();
        let _output = backend.generate(&program);
        let has_warning = backend.warnings().iter().any(|w| {
            w.contains("#!exit declared but program has no tick loop")
        });
        assert!(!has_warning,
            "No one-shot warning for standard reactor wake main with #!exit");
    }

    #[test]
    fn test_no_exit_path_warning_for_wake_program() {
        // Wake program without #!exit should warn about missing exit path
        let program = make_exit_program(None, Type::Int, true);
        let mut backend = LlvmBackend::new();
        let _output = backend.generate(&program);
        let has_warning = backend.warnings().iter().any(|w| {
            w.contains("has wake triggers but no exit path")
        });
        assert!(has_warning,
            "Expected no-exit-path warning for wake program without #!exit");
    }

    #[test]
    fn test_no_no_exit_path_warning_when_exit_present() {
        // Wake program WITH #!exit should NOT warn about missing exit path
        let exit_cond = Expr::Eq(
            Box::new(Expr::Identifier("ops".to_string())),
            Box::new(Expr::Identifier("N".to_string())),
        );
        let program = make_exit_program(Some(exit_cond), Type::Int, true);
        let mut backend = LlvmBackend::new();
        let _output = backend.generate(&program);
        let has_warning = backend.warnings().iter().any(|w| {
            w.contains("has wake triggers but no exit path")
        });
        assert!(!has_warning,
            "No no-exit-path warning when #!exit is present");
    }

    #[test]
    fn test_no_exit_path_warning_for_non_wake_program() {
        // Non-wake program without #!exit should NOT warn (one-shot is fine)
        let program = make_exit_program(None, Type::Int, false);
        let mut backend = LlvmBackend::new();
        let _output = backend.generate(&program);
        let has_warning = backend.warnings().iter().any(|w| {
            w.contains("has wake triggers but no exit path")
        });
        assert!(!has_warning,
            "No no-exit-path warning for non-wake program");
    }
}