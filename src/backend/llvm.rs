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

/// Recursively evaluate a constant expression tree to a concrete f64.
/// Used to fold `const m0: Float = 4.0 * pi * pi` into a literal before
/// global emission, avoiding the `constant float 0` bug.
fn try_eval_cfloat(expr: &Expr, constants: &HashMap<String, (Type, Expr)>) -> Option<f64> {
    match expr {
        Expr::Float(f) => Some(*f),
        Expr::Identifier(name) => {
            if let Some((Type::Float, inner)) = constants.get(name) {
                try_eval_cfloat(inner, constants)
            } else {
                None
            }
        }
        Expr::Add(l, r) => Some(try_eval_cfloat(l, constants)? + try_eval_cfloat(r, constants)?),
        Expr::Sub(l, r) => Some(try_eval_cfloat(l, constants)? - try_eval_cfloat(r, constants)?),
        Expr::Mul(l, r) => Some(try_eval_cfloat(l, constants)? * try_eval_cfloat(r, constants)?),
        Expr::Div(l, r) => Some(try_eval_cfloat(l, constants)? / try_eval_cfloat(r, constants)?),
        Expr::Neg(inner) => Some(-try_eval_cfloat(inner, constants)?),
        _ => None,
    }
}
use crate::ast::{
    ArrowDir, DispatchMode, Expr, ForeignSignature, MatchPattern, Program, ProjectionTarget, Statement, TopLevel, Type,
};

#[derive(Debug, Clone)]
pub struct TypedRegister {
    pub name: String,
    pub ty: Type,
}

pub struct FoldParam {
    pub counter_idx: usize,
    pub bound_field_idx: Option<usize>,
    pub bound_const_name: Option<String>,
    pub is_decreasing: bool,
    pub bound_literal: Option<i64>,
}

impl std::fmt::Display for TypedRegister {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name)
    }
}
use std::collections::{HashMap, HashSet};
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
        TopLevel::StateDecl(s) => { if let Some(ref e) = s.expr { collect_strings_expr(e, seen, out); } }
        _ => {}
    }
}
fn collect_strings_stmt(stmt: &Statement, seen: &mut std::collections::HashSet<String>, out: &mut Vec<String>) {
    match stmt {
        Statement::Let { expr, .. } => { if let Some(e) = expr { collect_strings_expr(e, seen, out); } }
        Statement::Assignment { expr, .. } => { collect_strings_expr(expr, seen, out); }
        Statement::Expression(e) => { collect_strings_expr(e, seen, out); }
        Statement::Term { values, .. } | Statement::TermBang { values, .. } => { for v in values.iter().flatten() { collect_strings_expr(v, seen, out); } }
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
        Not(e) | Neg(e) | BitNot(e) | Cast(e, _) => {
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
/// - FFI declare+call with C ABI (transparent, no compiler magic)
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
        Expr::Eq(l, r) => {
            matches!(l.as_ref(), Expr::Identifier(name) if trigger_names.contains(name.as_str()))
                || matches!(r.as_ref(), Expr::Identifier(name) if trigger_names.contains(name.as_str()))
        }
        Expr::And(l, r) => {
            is_trigger_gated(l, trigger_names) || is_trigger_gated(r, trigger_names)
        }
        _ => false,
    }
}

fn extract_trigger_keys(pre: &Expr, trigger_names: &std::collections::HashSet<&str>) -> Option<Vec<i64>> {
    let mut keys = Vec::new();
    match pre {
        Expr::Eq(l, r) | Expr::Eq(r, l) => {
            let (ident, val) = if let (Expr::Identifier(name), Expr::Integer(n)) = (l.as_ref(), r.as_ref()) {
                (name.clone(), *n)
            } else if let (Expr::Integer(n), Expr::Identifier(name)) = (l.as_ref(), r.as_ref()) {
                (name.clone(), *n)
            } else {
                return None;
            };
            if trigger_names.contains(ident.as_str()) {
                keys.push(val);
            } else {
                return None;
            }
        }
        Expr::Or(l, r) => {
            keys.extend(extract_trigger_keys(l, trigger_names)?);
            keys.extend(extract_trigger_keys(r, trigger_names)?);
        }
        Expr::And(l, r) => {
            if let Some(k) = extract_trigger_keys(l, trigger_names) {
                keys.extend(k);
            } else if let Some(k) = extract_trigger_keys(r, trigger_names) {
                keys.extend(k);
            } else {
                return None;
            }
        }
        _ => return None,
    }
    keys.sort_unstable();
    keys.dedup();
    if keys.len() < 2 { None } else { Some(keys) }
}

/// Compute the sparsity ratio of a sorted key set.
/// Dense sets (gap ratio < 4) don't need perfect hashing — standard
/// switch dispatch with consecutive offsets works fine.
fn sparsity_ratio(keys: &[i64]) -> f64 {
    if keys.len() < 2 { return 0.0; }
    let gaps: Vec<u64> = keys.windows(2).map(|w| (w[1] - w[0]) as u64).collect();
    let min_gap = *gaps.iter().min().unwrap_or(&1);
    let max_gap = *gaps.iter().max().unwrap_or(&0);
    if min_gap == 0 { return f64::MAX; }
    max_gap as f64 / min_gap as f64
}

/// Find a multiplicative perfect hash for a set of sparse keys.
/// Returns (multiplier, shift) such that h(k) = (k * M) >> S maps
/// each input key to a unique slot in [0, next_power_of_two(n)).
/// Guaranteed termination: capped at 10,000 iterations.
fn find_perfect_hash(keys: &[i64]) -> Option<(u64, u32)> {
    let n = keys.len();
    let num_slots = n.next_power_of_two();
    let shift = 64 - num_slots.trailing_zeros();
    let mut rng: u64 = 123456789;
    for _ in 0..10000 {
        rng = rng.wrapping_mul(2862933555777941757).wrapping_add(3037000493);
        let multiplier = rng | 1;
        let mut seen = vec![false; num_slots];
        let mut ok = true;
        for &k in keys {
            let hash = (k.wrapping_mul(multiplier as i64) as u64) >> shift;
            if seen[hash as usize] { ok = false; break; }
            seen[hash as usize] = true;
        }
        if ok { return Some((multiplier, shift)); }
    }
    None
}

pub struct LlvmBackend {
    spec: Option<crate::target_spec::TargetSpec>,
    field_index_map: HashMap<String, usize>,
    field_types: Vec<String>,
    field_initializers: HashMap<String, Option<Expr>>,
    mmio_fields: HashMap<String, u64>,
    mmio_initializers: HashMap<String, Option<Expr>>,
    mmio_prepopulated: bool,
    schema_aliases: HashMap<String, crate::dbrief::DbriefType>,
    pgo_profile: Option<crate::analysis::pgo::PgoProfile>,
    pgo_guard_idx: usize,
    txn_counter: usize,
    has_cycles: bool,
    pending_cleanup: Vec<Statement>,
    let_bindings: HashMap<String, String>,
    /// Types of let-bound expressions — needed so FieldAccess can GEP into
    /// struct instances held in local variables.
    let_binding_types: HashMap<String, Type>,
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
    is_lightweight_async: bool,
    exit_condition: Option<Box<Expr>>,
    has_natural_exit: bool,
    dead_info_disabled: bool,
    warnings: Vec<String>,
    ssa_state_reg: Option<String>,
    llvm_extra_flags: Vec<String>,
    slp_hazard_fns: HashSet<String>,
    reg_float_cache: HashMap<String, String>,
    /// Type of each emitted register — used for pointer-vs-list dispatch, etc.
    reg_type_cache: HashMap<String, Type>,
    state_reg_name: String,
    ssa_old_float_regs: HashMap<String, String>,
    ssa_old_int_regs: HashMap<String, String>,
    /// User-defined struct types: name → Vec<(field_name, field_type)>.
    /// Used by StructInstance to emit field layout and FieldAccess to GEP into instances.
    struct_types: HashMap<String, Vec<(String, Type)>>,
    /// User-defined enum types: name → EnumDefinition.
    /// Used to resolve discriminant values and variant field counts for constructors,
    /// PatternMatch guards, and Match arm dispatch.
    enum_types: HashMap<String, crate::ast::EnumDefinition>,
    /// Reverse mapping: variant name → (enum_name, discriminant_index, field_count).
    /// Built during generate() from enum_types. Used by Expr::Call to look up
    /// discriminant values without scanning all enums.
    variant_disc: HashMap<String, (String, u64, usize)>,
    /// --explain flag: print detailed compilation decisions
    explain: bool,
}

impl LlvmBackend {
    pub fn new() -> Self {
        LlvmBackend {
            spec: None,
            field_index_map: HashMap::new(),
            field_types: Vec::new(),
            field_initializers: HashMap::new(),
            mmio_fields: HashMap::new(),
            mmio_initializers: HashMap::new(),
            mmio_prepopulated: false,
            schema_aliases: HashMap::new(),
            pgo_profile: None,
            pgo_guard_idx: 0,
            txn_counter: 0,
            has_cycles: false,
            pending_cleanup: Vec::new(),
            let_bindings: HashMap::new(),
            let_binding_types: HashMap::new(),
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
            is_lightweight_async: false,
            exit_condition: None,
            has_natural_exit: false,
            dead_info_disabled: false,
            warnings: Vec::new(),
            ssa_state_reg: None,
            llvm_extra_flags: Vec::new(),
            slp_hazard_fns: HashSet::new(),
            reg_float_cache: HashMap::new(),
            reg_type_cache: HashMap::new(),
            state_reg_name: "%state".to_string(),
            ssa_old_float_regs: HashMap::new(),
            ssa_old_int_regs: HashMap::new(),
            struct_types: HashMap::new(),
            enum_types: HashMap::new(),
            variant_disc: HashMap::new(),
            explain: false,
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

    pub fn with_dead_info_disabled(mut self, disabled: bool) -> Self {
        self.dead_info_disabled = disabled;
        self
    }

    pub fn with_explain(mut self, explain: bool) -> Self {
        self.explain = explain;
        self
    }

    /// Pre-populate MMIO address map from a resolved DBV target binding.
    /// Each alias name maps to a physical u64 address for volatile MMIO access.
    pub fn with_mmio_addresses(mut self, addresses: HashMap<String, u64>) -> Self {
        self.mmio_fields = addresses;
        self.mmio_prepopulated = true;
        self
    }

    pub fn with_schema_aliases(mut self, aliases: HashMap<String, crate::dbrief::DbriefType>) -> Self {
        self.schema_aliases = aliases;
        self
    }

    pub fn with_pgo_profile(mut self, profile: crate::analysis::pgo::PgoProfile) -> Self {
        self.pgo_profile = Some(profile);
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
        self.validate_schema_types();
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
                TopLevel::Struct(s) => {
                    let fields: Vec<(String, Type)> = s.fields.iter()
                        .map(|f| (f.name.clone(), f.ty.clone()))
                        .collect();
                    self.struct_types.insert(s.name.clone(), fields);
                }
                TopLevel::Enum(e) => {
                    self.enum_types.insert(e.name.clone(), e.clone());
                }
                _ => {}
            }
        }

        // Build variant → (enum_name, discriminant, field_count) mapping.
        for (enum_name, edef) in &self.enum_types {
            let mut next_disc: u64 = 1;
            for v in &edef.variants {
                let (vname, field_count) = match v {
                    crate::ast::EnumVariant::Unit(n) => (n.clone(), 0),
                    crate::ast::EnumVariant::Tuple(n, fields) => (n.clone(), fields.len()),
                    crate::ast::EnumVariant::Struct(n, fields) => (n.clone(), fields.len()),
                };
                let disc = match vname.as_str() {
                    "None" | "Err" => 0,
                    _ => { let d = next_disc; next_disc += 1; d }
                };
                self.variant_disc.insert(vname, (enum_name.clone(), disc, field_count));
            }
        }

        // Fold complex float constant expressions (e.g. const m0: Float = 4.0 * pi * pi)
        // into simple Expr::Float(f64) literals so the global emission path
        // produces valid LLVM IR instead of `constant float 0`.
        let consts_snapshot: Vec<(String, (Type, Expr))> = self.constants.iter()
            .map(|(k, v)| (k.clone(), v.clone())).collect();
        for (name, (ty, expr)) in consts_snapshot {
            if ty == Type::Float {
                if let Some(val) = try_eval_cfloat(&expr, &self.constants) {
                    self.constants.insert(name, (Type::Float, Expr::Float(val)));
                }
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
        // Try region-analyzer first; fall back to key extraction for triggers
        // whose value sets aren't known to the region analyzer but are used
        // in precondition Eq/Or comparisons (e.g. `sensor == 101 || sensor == 204`).
        let (enumerable, enum_keys): (Option<Vec<(String, Option<u64>)>>, HashMap<String, Vec<i64>>) = {
            let region = &analysis.region_analyzer;
            if !self.trigger_names.is_empty() {
                let mut sizes = Vec::new();
                let mut total: u64 = 1;
                let mut ok = true;
                let mut fallback_triggers = Vec::new();
                for tn in &self.trigger_names {
                    let sz = region.value_set_size_of(tn);
                    if let Some(s) = sz {
                        total = total.saturating_mul(s);
                        if total > self.optimize_budget { ok = false; break; }
                        sizes.push((tn.clone(), sz));
                    } else {
                        fallback_triggers.push(tn.clone());
                    }
                }
                if ok && sizes.len() == self.trigger_names.len() {
                    (Some(sizes), HashMap::new())
                } else if !fallback_triggers.is_empty() {
                    // Fallback: try key extraction from all reactive txns' preconditions
                    let trigger_set: std::collections::HashSet<&str> =
                        self.trigger_names.iter().map(|s| s.as_str()).collect();
                    let mut keys_map = HashMap::new();
                    for tn in &fallback_triggers {
                        for (_, txn) in &txns {
                            if !txn.is_reactive { continue; }
                            if let Some(keys) = extract_trigger_keys(
                                &txn.contract.pre_condition, &trigger_set
                            ) {
                                keys_map.insert(tn.clone(), keys);
                                break;
                            }
                        }
                    }
                    if !keys_map.is_empty() {
                        let mut combined_sizes = sizes;
                        let mut combined_total = total;
                        let mut all_ok = true;
                        for tn in &self.trigger_names {
                            if combined_sizes.iter().any(|(n, _)| n == tn) { continue; }
                            if let Some(keys) = keys_map.get(tn) {
                                let s = keys.len() as u64;
                                combined_total = combined_total.saturating_mul(s);
                                if combined_total > self.optimize_budget { all_ok = false; break; }
                                combined_sizes.push((tn.clone(), Some(s)));
                            } else {
                                all_ok = false; break;
                            }
                        }
                        if all_ok { (Some(combined_sizes), keys_map) } else { (None, HashMap::new()) }
                    } else {
                        (None, HashMap::new())
                    }
                } else {
                    (None, HashMap::new())
                }
            } else { (None, HashMap::new()) }
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

        // Lightweight async: all async txns are effectively pure with runtime-
        // variable bounds.  Skip thread pool + barriers; use sequential
        // reactor_tick() instead.  The barrier is always a net loss when the
        // txn body is a single add+store (~2ns) and the barrier costs ~1µs.
        if !async_txn_names.is_empty() {
            let all_lightweight = async_txn_names.iter().all(|name| {
                analysis.transition_graph.nodes.iter().find(|n| n.name == *name).map_or(false, |node| {
                    let is_pure = node.is_pure_body || node.is_effectively_pure;
                    if !is_pure { return false; }
                    if let Some(ref bp) = node.bounded_pre {
                        let is_const = self.field_initializers.get(&bp.bound_var)
                            .and_then(|e| e.as_ref())
                            .map_or(false, |e| matches!(e, Expr::Integer(_)))
                            || self.constants.get(&bp.bound_var)
                                .map_or(false, |(_, e)| matches!(e, Expr::Integer(_)));
                        !is_const
                    } else { false }
                })
            });
            if all_lightweight {
                self.is_lightweight_async = true;
            }
        }

        let mut out = String::new();
        self.emit_header(&mut out);
self.emit_declares(&mut out);

        // Emit foreign declares inline (frgn_map is populated from the scan above)
        for (name, sig) in &self.frgn_map {
            let ret_ty = match sig.result_type {
                crate::ast::ResultType::VoidType | crate::ast::ResultType::TrueAssertion => "void",
                crate::ast::ResultType::Projection(ref ts) => {
                    if ts.is_empty() { "void" }
                    else if ts.iter().any(|t| matches!(t, Type::Float)) { "float" }
                    else { "i64" }
                }
            };
            let param_tys: Vec<&str> = sig.inputs.iter().map(|(_, t)| match t {
                Type::Int | Type::UInt => "i64",
                Type::Bool => "i32",
                Type::Char => "i32",
                Type::Float => "float",
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

        // Emit constant globals for TopLevel::Constant declarations.
        // Deduplicate identical constants to avoid redundant cache lines.
        // LLVM `@alias` maps multiple names to the same global without
        // allocating separate storage.
        let mut dedup_map: HashMap<String, String> = HashMap::new(); // key → canonical_name
        let mut alias_map: HashMap<String, String> = HashMap::new(); // name → canonical_name
        for (name, (ty, expr)) in &self.constants {
            let llvm_ty = match ty {
                Type::Float => "float", Type::Int | Type::UInt => "i64",
                Type::Bool => "i1", _ => "i64",
            };
            let key = match expr {
                Expr::Float(f) => format!("{}:bitcast(i32 {} to float)", llvm_ty, float_to_llvm_hex(*f)),
                Expr::Integer(n) => format!("{}:{}", llvm_ty, n),
                Expr::Bool(b) => format!("{}:{}", llvm_ty, if *b { "true" } else { "false" }),
                Expr::Neg(inner) => match inner.as_ref() {
                    Expr::Float(f) => format!("{}:bitcast(i32 {} to float)", llvm_ty, float_to_llvm_hex(-*f)),
                    Expr::Integer(n) => format!("{}:-{}", llvm_ty, n),
                    _ => format!("{}:0", llvm_ty),
                },
                Expr::String(_) => format!("{}:null", llvm_ty),
                _ => format!("{}:0", llvm_ty),
            };
            if let Some(canonical) = dedup_map.get(&key) {
                alias_map.insert(name.clone(), canonical.clone());
            } else {
                dedup_map.insert(key, name.clone());
                alias_map.insert(name.clone(), name.clone());
            }
        }
        // Emit declaration for canonical names only; emit alias for duplicates
        for (name, (ty, expr)) in &self.constants {
            let canonical = alias_map.get(name).cloned().unwrap_or_else(|| name.clone());
            if canonical != *name {
                let llvm_ty = match ty {
                    Type::Float => "float", Type::Int | Type::UInt => "i64",
                    Type::Bool => "i1", _ => "i64",
                };
                writeln!(out, "@{} = alias {}, {}* @{}", name, llvm_ty, llvm_ty, canonical).ok();
                continue;
            }
            let llvm_ty = match ty {
                Type::Float => "float", Type::Int | Type::UInt => "i64",
                Type::Bool => "i1", _ => "i64",
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
                writeln!(out, "@{} = constant {} {}", name, llvm_ty, val_str).ok();
            } else {
                writeln!(out, "@{} = constant {} {}", name, llvm_ty, val_str).ok();
            }
        }
        if !self.constants.is_empty() { writeln!(out).ok(); }

        self.declare_state_type(&mut out);
        // %State no longer has a module-level global. Instead, main()
        // allocates it on the stack as an alloca and passes it to all
        // internal functions as a noalias nocapture parameter. This
        // guarantees SROA promotes all fields to scalar registers.
        writeln!(out, "; %State is allocated on the stack in main() as %state = alloca %State").ok();
        writeln!(out).ok();

        // Emit string constants
        for (si, s) in self.string_constants.iter().enumerate() {
            let escaped = escape_llvm_string(s);
            writeln!(out, "@str.{} = private unnamed_addr constant [{} x i8] c\"{}\\00\", align 1", si, s.len() + 1, escaped).ok();
        }
        if !self.string_constants.is_empty() { writeln!(out).ok(); }

        // Run SLP hazard analysis before emitting function definitions and attributes.
        // This populates slp_hazard_fns so that slp_attr() returns the correct attribute
        // group (#4/#5) for hazardous functions, and the attributes section emits #4/#5.
        self.estimate_slp_hazard(&txns);

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
            if async_txn_names.contains(name.as_str()) && !self.is_lightweight_async {
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
        self.txn_counter = 0;
        self.emit_init_state(&mut out);
        writeln!(out).ok();
        // Reactor — sequential or parallel
        // Enumeration and wake trigger detection were computed above
        // in the auto-categorization step (lines ~312-380).

        // Reactor tick — use folded path when a single bounded-counter txn
        // with no triggers can be collapsed into a canonical while loop.
        let graph = &analysis.transition_graph;

        // Natural death: auto-exit for wake-triggered programs where ALL reactive
        // transactions have proven bounded convergence (bounded_pre + increments).
        // When every reactive txn is foldable, the program is guaranteed to converge
        // and can safely exit without an explicit #!exit pragma.
        //
        // We build a synthetic exit condition: for each foldable bounded-counter txn,
        // check `counter >= bound`. When ALL counters reach their bounds, no txn can
        // fire again, and main() returns 0.
        self.has_natural_exit = false;
        if self.exit_condition.is_none() && has_wake_triggers {
            let has_persistent_txn = txns.iter().any(|(name, t)| {
                t.is_reactive && !graph.nodes.iter()
                    .filter(|n| n.name == *name)
                    .any(|n| n.bounded_pre.is_some() && n.increments.is_some())
            });
            if !has_persistent_txn {
                let mut checks: Vec<Expr> = Vec::new();
                for (name, t) in &txns {
                    if !t.is_reactive { continue; }
                    if let Some(node) = graph.nodes.iter().find(|n| n.name == *name) {
                        if let Some(ref bp) = node.bounded_pre {
                            if let Some(ref inc) = node.increments {
                                if bp.var == inc.var {
                                    checks.push(Expr::Ge(
                                        Box::new(Expr::Identifier(bp.var.clone())),
                                        Box::new(Expr::Identifier(bp.bound_var.clone())),
                                    ));
                                }
                            }
                        }
                    }
                }
                if !checks.is_empty() {
                    let combined = checks.into_iter()
                        .reduce(|a, b| Expr::And(Box::new(a), Box::new(b)))
                        .unwrap();
                    self.exit_condition = Some(Box::new(combined));
                    self.has_natural_exit = true;
                }
            }
        }

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
                        if node.is_pure_body || node.is_effectively_pure {
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
                                });
                            if let Some(tv) = total_val {
                                // Compile-time constant total — emit O(1) store
                                self.emit_folded_pure_counter(&mut out, counter_idx, tv);
                                true
                            } else {
                                // Pure body + runtime-variable bound → phi-node register pipeline
                                self.emit_folded_main(&mut out, &node.name, counter_idx, total_idx, total_const_name, true, None);
                                true
                            }
                        } else {
                            // Non-pure body → SSA mode (load/store state once, emit body inline)
                            self.emit_folded_main(&mut out, &node.name, counter_idx, total_idx, total_const_name, false, Some(&txns[0].1.body));
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
                // Multi-txn all-pure folding: when NO triggers exist and ALL
                // reactive async txns have bounded_pre + increments with pure
                // bodies, fold them into a single register-pipeline main loop.
                let multi_foldable = enumerable.is_none()
                    && !has_wake_triggers
                    && !async_txn_names.is_empty()
                    && async_txn_names.iter().all(|name| {
                        graph.nodes.iter().find(|n| n.name == *name).map_or(false, |node| {
                            (node.is_pure_body || node.is_effectively_pure)
                            && node.bounded_pre.is_some()
                            && node.increments.is_some()
                        })
                    });
                let mut multi_fold_params: HashMap<String, FoldParam> = HashMap::new();
                if multi_foldable {
                    for txn_name in &async_txn_names {
                        if let Some(node) = graph.nodes.iter().find(|n| n.name == *txn_name) {
                            if let Some(ref bp) = node.bounded_pre {
                                if let Some(&cidx) = self.field_index_map.get(&bp.var) {
                                    let tidx = self.field_index_map.get(&bp.bound_var).copied();
                                    let tcname = if tidx.is_none() {
                                        if self.constants.contains_key(&bp.bound_var) {
                                            Some(bp.bound_var.clone())
                                        } else { None }
                                    } else { None };
                                    multi_fold_params.insert(txn_name.clone(), FoldParam {
                                        counter_idx: cidx,
                                        bound_field_idx: tidx,
                                        bound_const_name: tcname,
                                        is_decreasing: bp.direction == crate::analysis::transition_graph::ConvergeDirection::Decreasing,
                                        bound_literal: bp.bound_literal,
                                    });
                                }
                            }
                        }
                    }
                }
                if !multi_fold_params.is_empty() {
                    self.emit_folded_multi_main(&mut out, &txns, &[], &HashMap::new(), &multi_fold_params,
                        &HashMap::new(), 0, None, None, None, None, None, false);
                    self.emit_thread_pool_metadata(&mut out);
                } else if dispatch_mode == DispatchMode::Sequential && !txns.is_empty()
                    && enumerable.is_none() && !has_wake_triggers
                    && txns.iter().filter(|(_, t)| t.is_reactive).all(|(name, _)| {
                        graph.nodes.iter().find(|n| n.name == *name)
                            .map_or(false, |n| n.bounded_pre.is_some() && n.increments.is_some())
                    })
                {
                    self.emit_ssa_main(&mut out, &txns);
                } else if let Some(ref enum_sizes) = enumerable {
                // Enumerable triggers — emit switch-dispatch main
                // This path handles triggers with small compile-time-known value sets.
                // We emit a single @main that samples triggers once, then switch
                // dispatches to per-value folded loops.

                // Build per-txn folding params for all enum-candidate txns.
                // Each trigger-gated bounded-counter txn gets its own folded
                // loop in the case arm.  Multi-txn programs (e.g. async_counters)
                // need this to converge in O(1) ticks instead of one increment
                // per tick via reactor_tick.
                let enum_fold_params: HashMap<String, FoldParam> = {
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
                                        m.insert(txn_name.clone(), FoldParam {
                                            counter_idx: cidx,
                                            bound_field_idx: tidx,
                                            bound_const_name: tcname,
                                            is_decreasing: bp.direction == crate::analysis::transition_graph::ConvergeDirection::Decreasing,
                                            bound_literal: bp.bound_literal,
                                        });
                                    }
                                }
                            }
                        }
                    }
                    m
                };
                // Companion map: pure-body flag + total value for each foldable txn.
                // When a txn is pure and its bound is a compile-time constant, the
                // case arm can store the total directly instead of looping 50M times.
                let enum_fold_pure: HashMap<String, (bool, Option<i64>)> = {
                    let mut m = HashMap::new();
                    for txn_name in &enum_txn_names {
                        if let Some(node) = graph.nodes.iter().find(|n| n.name == *txn_name) {
                            let total_val = node.bounded_pre.as_ref().and_then(|bp| {
                                self.field_initializers.get(&bp.bound_var)
                                    .and_then(|e| e.as_ref())
                                    .and_then(|e| if let Expr::Integer(n) = e { Some(*n) } else { None })
                                    .or_else(|| {
                                        self.constants.get(&bp.bound_var).and_then(|(_, e)| {
                                            if let Expr::Integer(n) = e { Some(*n) } else { None }
                                        })
                                    })
                            });
                            m.insert(txn_name.clone(), (node.is_pure_body, total_val));
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
                self.emit_folded_multi_main(
                    &mut out,
                    &txns,
                    enum_sizes,
                    &enum_keys,
                    &enum_fold_params,
                    &enum_fold_pure,
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
        writeln!(out, "define void @reactor_tick(%State* noalias nocapture %state) local_unnamed_addr #2 {{").ok();
                writeln!(out, "  entry:").ok();
                writeln!(out, "  ret void").ok();
                writeln!(out, "}}").ok();
                writeln!(out).ok();
                // Main
                self.emit_main(&mut out, false);
            }
            }
        }

        // ── DEAD-FIELD INFO DIAGNOSTICS (A002/A003) ─────────
        if !self.dead_info_disabled {
            for node in &graph.nodes {
                let dead_fields: Vec<&String> = node.write_set.iter()
                    .filter(|f| !graph.live_fields.contains(*f))
                    .collect();

                if !dead_fields.is_empty() {
                    let dead_list: Vec<String> = dead_fields.iter()
                        .map(|f| format!("'{}'", f))
                        .collect();
                    if node.is_effectively_pure {
                        // Folded txn — these stores are genuinely eliminated
                        self.warnings.push(format!(
                            "info: field(s) {} written by txn '{}' are never read — stores eliminated\n\
                              note: not referenced by any precondition or #!exit condition",
                            dead_list.join(", "),
                            node.name,
                        ));
                    } else {
                        // Non-folded txn — stores still execute but value is wasted
                        self.warnings.push(format!(
                            "info: field(s) {} written by txn '{}' are never read — wasted work\n\
                              note: not referenced by any precondition or #!exit; values computed but have no effect",
                            dead_list.join(", "),
                            node.name,
                        ));
                    }
                }

                // A003: pure-counter fold info
                if node.is_effectively_pure {
                    let inc = node.increments.as_ref().unwrap();
                    let bp = node.bounded_pre.as_ref().unwrap();
                    let total_str = self.constants.get(&bp.bound_var)
                        .and_then(|(_, e)| if let Expr::Integer(n) = e { Some(n.to_string()) } else { None })
                        .or_else(|| self.field_initializers.get(&bp.bound_var)
                            .and_then(|e| e.as_ref())
                            .and_then(|e| if let Expr::Integer(n) = e { Some(n.to_string()) } else { None }));
                    let iterations_msg = match &total_str {
                        Some(s) => format!(" — {} iterations replaced by single store", s),
                        None => String::new(),
                    };
                    let dead_list: Vec<String> = dead_fields.iter()
                        .map(|f| format!("'{}'", f))
                        .collect();
                    let mut msg = format!(
                        "info: txn '{}' folded to O(1){}",
                        node.name, iterations_msg,
                    );
                    msg.push_str(&format!(
                        "\n  info: counter '{}' retains its store (the only live write)",
                        inc.var,
                    ));
                    if !dead_list.is_empty() {
                        msg.push_str(&format!("\n  info: dead fields: {}", dead_list.join(", ")));
                    }
                    self.warnings.push(msg);
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
        // reactive transactions converge. Natural death (auto-exit) is automatically
        // applied when all reactive txns have bounded convergence, so this warning
        // only fires for programs with persistent (non-foldable) reactive txns.
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
        writeln!(out, "attributes #1 = {{ nocallback nofree nosync nounwind willreturn }}").ok();
        writeln!(out, "attributes #2 = {{ mustprogress nofree norecurse nosync nounwind memory(readwrite) }}").ok();
        writeln!(out, "attributes #3 = {{ nofree norecurse nosync nounwind memory(readwrite) }}").ok();
        // SLP-safe attribute variants: #4 = #0 + disable-slp, #5 = #3 + disable-slp.
        // Dual attributes (disable-slp-vectorize + no-vectorize-slp) ensure LLVM
        // compatibility across versions 15–22+. Emitted only when needed.
        if !self.slp_hazard_fns.is_empty() {
            writeln!(out, "attributes #4 = {{").ok();
            writeln!(out, "    mustprogress nofree norecurse nosync nounwind willreturn").ok();
            writeln!(out, "    memory(argmem: readwrite)").ok();
            writeln!(out, "    \"disable-slp-vectorize\"=\"true\" \"no-vectorize-slp\"=\"true\"").ok();
            writeln!(out, "}}").ok();
            writeln!(out, "attributes #5 = {{").ok();
            writeln!(out, "    nofree norecurse nosync nounwind memory(readwrite)").ok();
            writeln!(out, "    \"disable-slp-vectorize\"=\"true\" \"no-vectorize-slp\"=\"true\"").ok();
            writeln!(out, "}}").ok();
        }
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

    /// Return any extra flags needed for `opt`. Currently emits
    /// `-slp-vectorize-hor=false` when SLP hazards exist, since LLVM 18's
    /// per-function `"disable-slp-vectorize"` attribute is not always respected
    /// by the new pass manager. This is a safeguard: the per-function attribute
    /// works on LLVM 15-17 and 22+; the global flag covers LLVM 18-21.
    pub fn llvm_extra_flags(&self) -> Vec<String> {
        let mut flags = Vec::new();
        if !self.slp_hazard_fns.is_empty() {
            flags.push("-slp-vectorize-hor=false".to_string());
        }
        flags
    }

    /// Return the attribute group for a function, using `#4` (SLP-disabled)
    /// instead of `#0` if the function is hazardous, or `#5` instead of `#3`.
    fn slp_attr(&self, fn_name: &str, default: &str) -> String {
        if self.slp_hazard_fns.contains(fn_name) {
            match default {
                "#0" => "#4".to_string(),
                "#3" => "#5".to_string(),
                _ => default.to_string(),
            }
        } else {
            default.to_string()
        }
    }

    // ── SLP Vectorization Hazard Analysis ─────────────────────
    //
    // Three critical guarantees make this analysis watertight:
    //   1. Local variable tracking: we walk body statements FIRST, collecting
    //      let-bound float names into `local_floats` before they're referenced.
    //   2. Operand-aware counting: any float binary op with ≥1 non-trivial
    //      operand (variable, constant, or literal) counts as a cross-op.
    //   3. Constant-load accounting: global float constants (matrix coefficients,
    //      filter taps) are counted and packed into the peak register demand.

    fn is_float_field(&self, name: &str) -> bool {
        self.field_index_map.get(name)
            .map(|&idx| self.field_types[idx] == "float")
            .unwrap_or(false)
    }

    fn is_float_expr_pre_cg(&self, expr: &Expr, local_floats: &std::collections::HashSet<String>) -> bool {
        match expr {
            Expr::Float(_) => true,
            Expr::Identifier(name) | Expr::OwnedRef(name) => {
                self.is_float_field(name)
                    || local_floats.contains(name.as_str())
                    || self.constants.get(name.as_str()).map_or(false, |(t, _)| *t == Type::Float)
            }
            Expr::Add(l, r) | Expr::Sub(l, r) | Expr::Mul(l, r) | Expr::Div(l, r) => {
                self.is_float_expr_pre_cg(l, local_floats) || self.is_float_expr_pre_cg(r, local_floats)
            }
            Expr::Neg(e) => self.is_float_expr_pre_cg(e, local_floats),
            Expr::Cast(_, ty) => *ty == Type::Float,
            Expr::Block(_, last) => self.is_float_expr_pre_cg(last, local_floats),
            _ => false,
        }
    }

    fn count_cross_float_ops(&self, expr: &Expr, local_floats: &std::collections::HashSet<String>) -> u32 {
        match expr {
            Expr::Add(l, r) | Expr::Sub(l, r) | Expr::Mul(l, r) | Expr::Div(l, r) => {
                let is_float = self.is_float_expr_pre_cg(l, local_floats) || self.is_float_expr_pre_cg(r, local_floats);
                // Count cross-field ops only when both operands reference distinctly
                // named float fields/constants (not literals like Float(1.0)). In-lane
                // operations (f[i] + 1.0, f[i] + f[i]) create no shuffle pressure.
                let is_cross_field = match (l.as_ref(), r.as_ref()) {
                    (Expr::Identifier(n1), Expr::Identifier(n2)) | (Expr::OwnedRef(n1), Expr::OwnedRef(n2)) => n1 != n2,
                    (Expr::Identifier(_), Expr::OwnedRef(n)) | (Expr::OwnedRef(n), Expr::Identifier(_)) => true,
                    _ => false,
                };
                let mut count = if is_cross_field && is_float { 1 } else { 0 };
                count += self.count_cross_float_ops(l, local_floats);
                count += self.count_cross_float_ops(r, local_floats);
                count
            }
            Expr::Neg(e) => self.count_cross_float_ops(e, local_floats),
            Expr::Block(_, last) => self.count_cross_float_ops(last, local_floats),
            _ => 0,
        }
    }

    fn collect_local_floats_and_temps(&self, body: &[Statement], local_floats: &mut std::collections::HashSet<String>) -> u32 {
        let mut temp_count = 0;
        for stmt in body {
            match stmt {
                Statement::Let { name, ty, expr, .. } => {
                    let is_float = ty.as_ref() == Some(&Type::Float)
                        || expr.as_ref().map_or(false, |e| self.is_float_expr_pre_cg(e, local_floats));
                    if is_float {
                        local_floats.insert(name.clone());
                        temp_count += 1;
                    }
                }
                Statement::Guarded { statements, .. } => {
                    temp_count += self.collect_local_floats_and_temps(statements, local_floats);
                }
                _ => {}
            }
        }
        temp_count
    }

    fn target_hardware(&self, spec: &crate::target_spec::TargetSpec) -> (u32, u32) {
        if spec.has_capability("avx512f") {
            (32, 16)
        } else if spec.has_capability("avx2") {
            (16, 8)
        } else if spec.has_capability("neon") {
            (32, 4)
        } else if spec.has_capability("sse") {
            (16, 4)
        } else {
            (16, 1)
        }
    }

    fn estimate_slp_hazard(&mut self, txns: &[(String, &crate::ast::Transaction)]) {
        let (r, w) = match self.spec.as_ref() {
            Some(spec) => self.target_hardware(spec),
            None => (16, 4), // default: x86_64 SSE (matches emit_header target triple)
        };
        if w <= 1 {
            return;
        }

        let mut float_fields: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut accessed_constants: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut total_cross_ops: u32 = 0;
        let mut max_float_temps: u32 = 0;

        for (_, txn) in txns.iter().filter(|(_, t)| t.is_reactive) {
            let mut local_floats = std::collections::HashSet::new();
            let temps = self.collect_local_floats_and_temps(&txn.body, &mut local_floats);
            max_float_temps = max_float_temps.max(temps);

            let reads = crate::backend::collect_read_identifiers(&txn.body);
            let writes: std::collections::HashSet<String> =
                crate::backend::collect_assigned_identifiers(&txn.body)
                    .into_iter().collect();

            for f in reads.union(&writes) {
                if self.is_float_field(f) {
                    float_fields.insert(f.clone());
                }
            }

            for f in reads.iter() {
                if self.constants.get(f.as_str()).map_or(false, |(t, _)| *t == Type::Float) {
                    accessed_constants.insert(f.clone());
                }
            }

            for stmt in &txn.body {
                match stmt {
                    Statement::Assignment { expr, .. } => {
                        total_cross_ops += self.count_cross_float_ops(expr, &local_floats);
                    }
                    Statement::Let { expr: Some(e), .. } => {
                        total_cross_ops += self.count_cross_float_ops(e, &local_floats);
                    }
                    Statement::Guarded { statements, .. } => {
                        for s in statements {
                            match s {
                                Statement::Assignment { expr, .. } => {
                                    total_cross_ops += self.count_cross_float_ops(expr, &local_floats);
                                }
                                Statement::Let { expr: Some(e), .. } => {
                                    total_cross_ops += self.count_cross_float_ops(e, &local_floats);
                                }
                                _ => {}
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        let n = float_fields.len();
        if n == 0 {
            return;
        }

        let packed_phis = (n + w as usize - 1) / w as usize;
        let c = total_cross_ops as usize;
        let shuffle_pressure = std::cmp::min(c, n as usize * 2);
        let const_packed = (accessed_constants.len() + w as usize - 1) / w as usize;
        let peak = (packed_phis + shuffle_pressure + max_float_temps as usize + const_packed + 2) as u32;

        if peak >= r {
            // Mark affected functions — SLP is disabled per-function via
            // attribute "disable-slp-vectorize" on #4 (derived from #0) or
            // #5 (derived from #3). This keeps SLP enabled on all other
            // functions, unlike the old global -vectorize-slp=false flag.
            self.slp_hazard_fns.insert("main".to_string());
            for (txn_name, _) in txns {
                self.slp_hazard_fns.insert(txn_name.clone());
            }
        } else {
            // Register pressure is fine — check ASR profitability.
            // SLP vectorization only pays off when enough arithmetic ops exist
            // per field to amortize the packing overhead. Below ~1.5 ops/field,
            // the shuffle pipeline (Port 5) saturates before any throughput
            // gain materializes.
            //
            // Skip the check when there are no cross-field ops (all in-lane):
            // SLP benefits immediately with zero shuffle overhead.
            if total_cross_ops > 0 {
                let total_float_ops = self.count_all_float_ops(&txns);
                if total_float_ops > 0 && n > 0 {
                    let ops_per_field = total_float_ops as f64 / n as f64;
                    if ops_per_field < 1.5 {
                        self.slp_hazard_fns.insert("main".to_string());
                        for (txn_name, _) in txns {
                            self.slp_hazard_fns.insert(txn_name.clone());
                        }
                    }
                }
            }
        }
    }

    fn count_all_float_ops(&self, txns: &[(String, &crate::ast::Transaction)]) -> u32 {
        let mut count = 0;
        for (_, txn) in txns.iter().filter(|(_, t)| t.is_reactive) {
            let mut local_floats = std::collections::HashSet::new();
            self.collect_local_floats_and_temps(&txn.body, &mut local_floats);
            for stmt in &txn.body {
                match stmt {
                    Statement::Assignment { expr, .. } | Statement::Let { expr: Some(expr), .. } => {
                        count += self.count_float_arith_ops(expr, &local_floats);
                    }
                    Statement::Guarded { statements, .. } => {
                        for s in statements {
                            match s {
                                Statement::Assignment { expr, .. } | Statement::Let { expr: Some(expr), .. } => {
                                    count += self.count_float_arith_ops(expr, &local_floats);
                                }
                                _ => {}
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
        count
    }

    fn count_float_arith_ops(&self, expr: &Expr, local_floats: &std::collections::HashSet<String>) -> u32 {
        match expr {
            Expr::Add(l, r) | Expr::Sub(l, r) | Expr::Mul(l, r) | Expr::Div(l, r) => {
                let is_float = self.is_float_expr_pre_cg(l, local_floats)
                    || self.is_float_expr_pre_cg(r, local_floats);
                let mut c = if is_float { 1 } else { 0 };
                c += self.count_float_arith_ops(l, local_floats);
                c += self.count_float_arith_ops(r, local_floats);
                c
            }
            Expr::Neg(e) => {
                if self.is_float_expr_pre_cg(e, local_floats) {
                    1 + self.count_float_arith_ops(e, local_floats)
                } else {
                    self.count_float_arith_ops(e, local_floats)
                }
            }
            Expr::Block(_, last) => self.count_float_arith_ops(last, local_floats),
            _ => 0,
        }
    }

    /// Check if an expression produces a `Ptr<T>` value.
    /// Used by `ListIndex` to decide between direct pointer GEP vs 2-slot header load.
    fn is_ptr_expr(&self, expr: &Expr) -> bool {
        match expr {
            Expr::Projection { target, .. } => matches!(target, ProjectionTarget::Ptr),
            Expr::Identifier(name) => {
                self.let_binding_types.get(name)
                    .map(|t| matches!(t, Type::Applied(n, _) if n == "Ptr"))
                    .unwrap_or(false)
            }
            Expr::OwnedRef(name) => {
                self.let_binding_types.get(name)
                    .map(|t| matches!(t, Type::Applied(n, _) if n == "Ptr"))
                    .unwrap_or(false)
            }
            _ => false,
        }
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
        // LLVM bit manipulation intrinsics
        writeln!(out, "declare i64 @llvm.ctpop.i64(i64) #1").ok();
        writeln!(out, "declare i64 @llvm.ctlz.i64(i64, i1) #1").ok();
        writeln!(out, "declare i64 @llvm.cttz.i64(i64, i1) #1").ok();
        writeln!(out, "declare i64 @llvm.abs.i64(i64, i1) #1").ok();
        writeln!(out, "declare double @llvm.fabs.f64(double) #1").ok();
        writeln!(out, "declare i64 @llvm.bitreverse.i64(i64) #1").ok();
        // TODO (eliminate-magic Phase C2-C3): Remove these hardcoded runtime declares
        // once the codegen call sites are migrated to use self.frgn_map lookups.
        // When std/rt.bv is fully integrated, these will come from user imports.
        writeln!(out, "declare void @__rt_init() local_unnamed_addr").ok();
        writeln!(out, "declare void @__rt_poll() local_unnamed_addr").ok();
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
        if !self.mmio_prepopulated {
            self.mmio_fields.clear();
            self.mmio_initializers.clear();
        }
        for item in &program.items {
            if let TopLevel::StateDecl(s) = item {
                if let Some(addr) = s.address {
                    self.mmio_fields.insert(s.name.clone(), addr);
                    self.mmio_initializers.insert(s.name.clone(), s.expr.clone());
                } else if self.mmio_prepopulated && self.mmio_fields.contains_key(&s.name) {
                    if self.schema_aliases.is_empty() || self.schema_aliases.contains_key(&s.name) {
                        self.mmio_initializers.insert(s.name.clone(), s.expr.clone());
                    } else {
                        // Not in any imported schema — remove from mmio_fields to prevent
                        // accidental MMIO routing in reads/writes.
                        self.mmio_fields.remove(&s.name);
                        self.field_index_map
                            .insert(s.name.clone(), self.field_types.len());
                        self.field_types.push(self.llvm_type(&s.ty).to_string());
                        self.field_initializers.insert(s.name.clone(), s.expr.clone());
                    }
                } else {
                    self.field_index_map
                        .insert(s.name.clone(), self.field_types.len());
                    self.field_types.push(self.llvm_type(&s.ty).to_string());
                    self.field_initializers.insert(s.name.clone(), s.expr.clone());
                }
            }
        }
    }

    fn validate_schema_types(&mut self) {
        if self.schema_aliases.is_empty() {
            return;
        }
        for (name, schema_type) in &self.schema_aliases.clone() {
            let brief_type = match schema_type.to_brief_type_name() {
                Ok(t) => t,
                Err(e) => {
                    self.warnings.push(format!(
                        "warning: schema type incompatibility for '{}': {}. Field will NOT be treated as MMIO.",
                        name, e
                    ));
                    continue;
                }
            };
            let name_clone = name.clone();
            for item_name in self.field_index_map.keys().chain(self.mmio_initializers.keys()) {
                if item_name == &name_clone {
                    if brief_type == "Int" {
                        if brief_type == "Int" && schema_type.is_unsigned_int() {
                            self.warnings.push(format!(
                                "warning: schema declares '{}' as unsigned but Brief uses Int. 64-bit target makes this safe.",
                                name
                            ));
                        }
                    }
                    break;
                }
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

    /// Return a native float register for `val_reg` if one exists in the
    /// float cache. If not, emit the trunc+bitcast boxing chain and return
    /// the resulting float register name.
    fn native_float_or_box(
        &mut self,
        out: &mut String,
        indent: &str,
        val_reg: &str,
    ) -> String {
        if let Some(cached) = self.reg_float_cache.get(val_reg) {
            return cached.clone();
        }
        let tr = format!("%nftr{}", self.txn_counter); self.txn_counter += 1;
        let fl = format!("%nffl{}", self.txn_counter); self.txn_counter += 1;
        writeln!(out, "{}{} = trunc i64 {} to i32", indent, tr, val_reg).ok();
        writeln!(out, "{}{} = bitcast i32 {} to float", indent, fl, tr).ok();
        fl
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
        writeln!(out, "define void @init_state(%State* noalias nocapture %state) local_unnamed_addr #0 {{").ok();
        writeln!(out, "  entry:").ok();
        let mut reg = 0u32;
        let mut fields: Vec<(String, usize, String)> = self.field_index_map.iter()
            .map(|(name, &idx)| (name.clone(), idx, self.field_types[idx].clone()))
            .collect();
        fields.sort_by_key(|&(_, idx, _)| idx);
        for (name, idx, ty) in fields {
            let p = format!("%ip{}", reg); reg += 1;
            writeln!(out, "  {} = getelementptr inbounds %State, %State* %state, i32 0, i32 {}", p, idx).ok();
            let init_clone = self.field_initializers.get(&name).and_then(|e| e.clone());
            match init_clone {
                Some(Expr::Integer(n)) => {
                    writeln!(out, "  store i64 {}, i64* {}, align {}", n, p, self.align_of("i64")).ok();
                }
                Some(Expr::Float(f)) => {
                    let h = float_to_llvm_hex(f);
                    let bits_reg = format!("%ip{}b", reg - 1);
                    writeln!(out, "  {} = bitcast i32 {} to float", bits_reg, h).ok();
                    writeln!(out, "  store float {}, float* {}, align {}", bits_reg, p, self.align_of("float")).ok();
                }
                Some(Expr::Neg(ref inner)) => {
                    let s = match inner.as_ref() {
                        Expr::Float(f) => float_to_llvm_hex(-*f),
                        Expr::Integer(n) => format!("-{}", n),
                        _ => "0".to_string(),
                    };
                    writeln!(out, "  store i64 {}, i64* {}, align {}", s, p, self.align_of("i64")).ok();
                }
                Some(Expr::Bool(b)) => {
                    let v = if b { "1" } else { "0" };
                    writeln!(out, "  store i8 {}, i8* {}, align {}", v, p, self.align_of("i8")).ok();
                }
                Some(Expr::String(_)) => {
                    writeln!(out, "  store i8* null, i8** {}, align {}", p, self.align_of("i8*")).ok();
                }
                Some(Expr::Char(c)) => {
                    let v = c as i32;
                    writeln!(out, "  store i32 {}, i32* {}, align {}", v, p, self.align_of("i32")).ok();
                }
                Some(expr) => {
                    // Non-literal initializer — e.g. __get_env_int("BOUND").
                    // Emit the expression and store the result. The expression
                    // always produces i64; truncate/bitcast for non-Int types.
                    let val_reg = self.emit_expr(out, &expr, "  ");
                    match ty.as_str() {
                        "i8" => {
                            let t = format!("%ip{}t", reg); reg += 1;
                            writeln!(out, "  {} = trunc i64 {} to i8", t, val_reg).ok();
                            writeln!(out, "  store i8 {}, i8* {}, align {}", t, p, self.align_of("i8")).ok();
                        }
                        "i32" => {
                            let t = format!("%ip{}t", reg); reg += 1;
                            writeln!(out, "  {} = trunc i64 {} to i32", t, val_reg).ok();
                            writeln!(out, "  store i32 {}, i32* {}, align {}", t, p, self.align_of("i32")).ok();
                        }
                        "float" => {
                            let fl = self.native_float_or_box(out, "  ", &val_reg.to_string());
                            writeln!(out, "  store float {}, float* {}, align {}", fl, p, self.align_of("float")).ok();
                        }
                        "i8*" => {
                            let t = format!("%ip{}t", reg); reg += 1;
                            writeln!(out, "  {} = inttoptr i64 {} to i8*", t, val_reg).ok();
                            writeln!(out, "  store i8* {}, i8** {}, align {}", t, p, self.align_of("i8*")).ok();
                        }
                        _ => {
                            writeln!(out, "  store i64 {}, {}* {}, align {}", val_reg, ty, p, self.align_of(&ty)).ok();
                        }
                    }
                }
                None => {
                    let default = if ty == "i8*" { "null".to_string() } else { "0".to_string() };
                    writeln!(out, "  store {} {}, {}* {}, align {}", ty, default, ty, p, self.align_of(&ty)).ok();
                }
            }
        }
        // Initialize MMIO fields — only if an explicit initial value was given
        let mmio_inits: Vec<(u64, Expr)> = {
            let mut v = Vec::new();
            for (name, &addr) in &self.mmio_fields {
                if let Some(Some(expr)) = self.mmio_initializers.get(name).cloned() {
                    v.push((addr, expr.clone()));
                }
            }
            v
        };
        for (addr, expr) in mmio_inits {
            let p = format!("%mio{}", reg); reg += 1;
            writeln!(out, "  {} = inttoptr i64 {} to i64*", p, addr).ok();
            let val_reg = self.emit_expr(out, &expr, "  ");
            writeln!(out, "  store volatile i64 {}, i64* {}, align 1", val_reg, p).ok();
        }
        writeln!(out, "  ret void").ok();
        writeln!(out, "}}").ok();
    }

    // ── DEFINITION ────────────────────────────────────────────
    fn emit_definition(&mut self, out: &mut String, d: &crate::ast::Definition) {
        self.pending_cleanup.clear();
        self.let_bindings.clear(); self.let_binding_types.clear(); self.reg_float_cache.clear(); self.reg_type_cache.clear();
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
        let txn_attr = self.slp_attr(name, "#0");

        // Check for #assume_shape pragma — extract rollback action
        let assume_action: Option<&str> = txn.modifiers.iter()
            .find(|m| m.name == "assume_shape")
            .and_then(|m| m.value.as_ref())
            .and_then(|v| {
                let parts: Vec<&str> = v.splitn(2, ", ").collect();
                if parts.len() == 2 {
                    let action = parts[1].trim();
                    if action == "run" || action == "exit" { Some(action) } else { Some("escape") }
                } else {
                    Some("escape") // default
                }
            });

        if let Some(action) = assume_action {
            writeln!(out, "define void @{}(%State* noalias nocapture %state) local_unnamed_addr {}{} {{", name, txn_attr, alwaysinline).ok();
            writeln!(out, "  entry:").ok();
            writeln!(out, "  br i1 true, label %body, label %rollback").ok();
            writeln!(out, "  body:").ok();
            self.txn_counter = 0;
            self.let_bindings.clear(); self.let_binding_types.clear(); self.reg_float_cache.clear(); self.reg_type_cache.clear();
            self.terminated = false;
            self.returns_i64 = false;
            if !matches!(txn.contract.pre_condition, Expr::Bool(true)) {
                self.emit_precondition_check(out, &txn.contract.pre_condition, "  ");
            }
            for s in &txn.body { self.emit_stmt(out, s, "  "); }
            if !self.terminated { writeln!(out, "  ret void").ok(); }
            writeln!(out, "  rollback:").ok();
            match action {
                "exit" => {
                    writeln!(out, "    call void @__exit(i64 1)").ok();
                    writeln!(out, "    unreachable").ok();
                }
                "run" => {
                    writeln!(out, "    br label %body").ok();
                }
                _ => {
                    writeln!(out, "    ret void").ok();
                }
            }
            writeln!(out, "}}").ok();
        } else {
            writeln!(out, "define void @{}(%State* noalias nocapture %state) local_unnamed_addr {}{} {{", name, txn_attr, alwaysinline).ok();
            writeln!(out, "  entry:").ok();
            self.txn_counter = 0;
            self.let_bindings.clear(); self.let_binding_types.clear(); self.reg_float_cache.clear(); self.reg_type_cache.clear();
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
        self.let_bindings.clear(); self.let_binding_types.clear(); self.reg_float_cache.clear(); self.reg_type_cache.clear();
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
        let async_attr = self.slp_attr(&async_name, "#0");
        writeln!(out, "define void @{}(%State* noalias nocapture %state) local_unnamed_addr {} {{", async_name, async_attr).ok();
        writeln!(out, "  entry:").ok();
        self.txn_counter = 0;
        self.let_bindings.clear(); self.let_binding_types.clear(); self.reg_float_cache.clear(); self.reg_type_cache.clear();
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
            .filter(|s| !matches!(s, Statement::Term { .. } | Statement::TermBang { .. } | Statement::Escape(_)))
            .cloned().collect();
        let combined: Vec<Statement> = body_a.into_iter().chain(b.body.iter().cloned()).collect();
        let fused_attr = self.slp_attr(name, "#0");
        writeln!(out, "define void @{}(%State* noalias nocapture %state) local_unnamed_addr {} {{", name, fused_attr).ok();
        writeln!(out, "  entry:").ok();
        self.txn_counter = 0; self.let_bindings.clear(); self.let_binding_types.clear(); self.reg_float_cache.clear(); self.reg_type_cache.clear(); self.terminated = false; self.returns_i64 = false;
        for s in &combined { self.emit_stmt(out, s, "  "); }
        if !self.terminated { writeln!(out, "  ret void").ok(); }
        writeln!(out, "}}").ok();
    }

    /// Emit a txn body wrapped with #assume_shape guard check.
    /// For now the guard is a constant `true` — guard expression parsing
    /// from the pragma string to Expr is future work.
    fn emit_shape_guarded_body(&mut self, out: &mut String, body: &[Statement], name: &str, action: &str) {
        let fused_attr = self.slp_attr(name, "#0");
        writeln!(out, "define void @{}(%State* noalias nocapture %state) local_unnamed_addr {} {{", name, fused_attr).ok();
        writeln!(out, "  entry:").ok();
        writeln!(out, "  br i1 true, label %body, label %rollback").ok();
        writeln!(out, "  body:").ok();
        self.txn_counter = 0; self.let_bindings.clear(); self.let_binding_types.clear(); self.reg_float_cache.clear(); self.reg_type_cache.clear(); self.terminated = false; self.returns_i64 = false;
        for s in body { self.emit_stmt(out, s, "  "); }
        if !self.terminated { writeln!(out, "  ret void").ok(); }
        writeln!(out, "  rollback:").ok();
        match action {
            "exit" => {
                writeln!(out, "    call void @__exit(i64 1)").ok();
                writeln!(out, "    unreachable").ok();
            }
            "run" => {
                writeln!(out, "    br label %body").ok();
            }
            _ => { // escape
                writeln!(out, "    ret void").ok();
            }
        }
        writeln!(out, "}}").ok();
    }

    fn emit_fused_composed(&mut self, out: &mut String, body: &[Statement], name: &str) {
        let fused_attr = self.slp_attr(name, "#0");
        writeln!(out, "define void @{}(%State* noalias nocapture %state) local_unnamed_addr {} {{", name, fused_attr).ok();
        writeln!(out, "  entry:").ok();
        self.txn_counter = 0; self.let_bindings.clear(); self.let_binding_types.clear(); self.reg_float_cache.clear(); self.reg_type_cache.clear(); self.terminated = false; self.returns_i64 = false;
        for s in body { self.emit_stmt(out, s, "  "); }
        if !self.terminated { writeln!(out, "  ret void").ok(); }
        writeln!(out, "}}").ok();
    }

    // ── STATEMENTS ────────────────────────────────────────────
    fn emit_stmt(&mut self, out: &mut String, stmt: &Statement, indent: &str) {
        match stmt {
            Statement::Term { values, swan_song, .. } => {
                let c = self.pending_cleanup.clone();
                for s in &c { self.emit_stmt(out, s, indent); }
                if let Some(swan) = swan_song {
                    self.emit_stmt(out, swan, indent);
                }
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
            Statement::TermBang { values, swan_song, .. } => {
                let c = self.pending_cleanup.clone();
                for s in &c { self.emit_stmt(out, s, indent); }
                if let Some(swan) = swan_song {
                    self.emit_stmt(out, swan, indent);
                }
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
            Statement::Let { name, expr, ty, address_expr, .. } => {
                if let Some(e) = expr {
                    let r = self.emit_expr(out, e, indent);
                    self.let_bindings.insert(name.clone(), r.name.clone());
                    // Use type annotation if available (preserves Ptr<T> etc), otherwise fall back to emitted type
                    let resolved_ty = ty.clone().unwrap_or_else(|| r.ty.clone());
                    self.let_binding_types.insert(name.clone(), resolved_ty);
                    writeln!(out, "{}; let {} = {}", indent, name, r).ok();
                } else {
                    writeln!(out, "{}; let {} = undef", indent, name).ok();
                }
            }
            Statement::Assignment { lhs, expr, modifiers, .. } => {
                let val = self.emit_expr(out, expr, indent);
                let fname = match lhs {
                    Expr::Identifier(n) | Expr::OwnedRef(n) => n.clone(),
                    Expr::ListIndex(list_expr, index_expr) => {
                        let val_reg = val.name.clone();
                        let list_name = match &**list_expr {
                            Expr::Identifier(n) | Expr::OwnedRef(n) => n.clone(),
                            _ => { writeln!(out, "{}; assign list[idx] = {}", indent, val_reg).ok(); return; }
                        };
                        let idx_val = self.emit_expr(out, index_expr, indent);
                        // Resolve the list pointer from state (SSA or non-SSA) or let bindings
                        let list_ptr: Option<String> =
                            if let Some(ref ssa_reg) = self.ssa_state_reg.clone() {
                                if let Some(&field_idx) = self.field_index_map.get(&list_name) {
                                    let ev = format!("%lev{}", self.txn_counter); self.txn_counter += 1;
                                    writeln!(out, "{}{} = extractvalue %State {}, {}", indent, ev, ssa_reg, field_idx).ok();
                                    Some(ev)
                                } else if let Some(reg) = self.let_bindings.get(&list_name).cloned() {
                                    Some(reg)
                                } else {
                                    None
                                }
                            } else if let Some(reg) = self.let_bindings.get(&list_name).cloned() {
                                Some(reg)
                            } else if let Some(&field_idx) = self.field_index_map.get(&list_name) {
                                let p = format!("%lgp{}", self.txn_counter); self.txn_counter += 1;
                                writeln!(out, "{}{} = getelementptr inbounds %State, %State* %state, i32 0, i32 {}", indent, p, field_idx).ok();
                                let ld = format!("%lld{}", self.txn_counter); self.txn_counter += 1;
                                writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, ld, p).ok();
                                Some(ld)
                            } else {
                                None
                            };
                        let Some(list_ptr) = list_ptr else {
                            writeln!(out, "{}; assign list[idx] = {} (unknown list '{}')", indent, val_reg, list_name).ok();
                            return;
                        };
                        let hp = format!("%lhp{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, hp, list_ptr).ok();
                        let dp = format!("%ldp{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, dp, hp).ok();
                        let de = format!("%lde{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, de, dp).ok();
                        let ep = format!("%lep{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, ep, de, idx_val.name).ok();
                        writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, val_reg, ep).ok();
                        return;
                    }
                    _ => { writeln!(out, "{}; assign {}", indent, val).ok(); return; }
                };
                let is_volatile = modifiers.iter().any(|h| h.name == "volatile");
                // SSA mode: use insertvalue instead of GEP + store
                if let Some(ssa_reg) = self.ssa_state_reg.clone() {
                    if let Some(&idx) = self.field_index_map.get(&fname) {
                        if !is_volatile {
                            let ty = self.field_types[idx].clone();
                            let new_reg = format!("%in{}", self.txn_counter); self.txn_counter += 1;
                            match ty.as_str() {
                                "i8" => {
                                    let tr = format!("%tr{}", self.txn_counter); self.txn_counter += 1;
                                    writeln!(out, "{}{} = trunc i64 {} to i8", indent, tr, val).ok();
                                    writeln!(out, "{}{} = insertvalue %State {}, i8 {}, {}", indent, new_reg, ssa_reg, tr, idx).ok();
                                }
                                "float" => {
                                    let fl = self.native_float_or_box(out, indent, &val.to_string());
                                    writeln!(out, "{}{} = insertvalue %State {}, float {}, {}", indent, new_reg, ssa_reg, fl, idx).ok();
                                }
                                "i8*" => {
                                    let p = format!("%fp{}", self.txn_counter); self.txn_counter += 1;
                                    writeln!(out, "{}{} = inttoptr i64 {} to i8*", indent, p, val).ok();
                                    writeln!(out, "{}{} = insertvalue %State {}, i8* {}, {}", indent, new_reg, ssa_reg, p, idx).ok();
                                }
                                _ => {
                                    writeln!(out, "{}{} = insertvalue %State {}, i64 {}, {}", indent, new_reg, ssa_reg, val, idx).ok();
                                }
                            }
                            self.ssa_state_reg = Some(new_reg);
                            return;
                        }
                    }
                }
                if let Some(&addr) = self.mmio_fields.get(&fname) {
                    let p = format!("%mio{}", self.txn_counter); self.txn_counter += 1;
                    writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, p, addr).ok();
                    writeln!(out, "{}store volatile i64 {}, i64* {}, align 1", indent, val, p).ok();
                    return;
                }
                if let Some(&idx) = self.field_index_map.get(&fname) {
                    let ty = self.field_types[idx].clone();
                    let p = format!("%ap{}", self.txn_counter); self.txn_counter += 1;
                    writeln!(out, "{}{} = getelementptr inbounds %State, %State* %state, i32 0, i32 {}", indent, p, idx).ok();
                    let vol_str = if is_volatile { " volatile" } else { "" };
                    match ty.as_str() {
                        "i8" => {
                            let tr = format!("%tr{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = trunc i64 {} to i8", indent, tr, val).ok();
                            writeln!(out, "{}store{} i8 {}, i8* {}, align {}", indent, vol_str, tr, p, self.align_of(&ty)).ok();
                        }
                        "float" => {
                            let fl = self.native_float_or_box(out, indent, &val.to_string());
                            writeln!(out, "{}store{} float {}, float* {}, align {}", indent, vol_str, fl, p, self.align_of(&ty)).ok();
                        }
                        _ => {
                            writeln!(out, "{}store{} {} {}, {}* {}, align {}", indent, vol_str, ty, val, ty, p, self.align_of(&ty)).ok();
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

                // Guard→select if single assignment (not in SSA mode — branch-based path handles insertvalue)
                if statements.len() == 1 && self.ssa_state_reg.is_none() {
                    if let Statement::Assignment { lhs, expr, modifiers, .. } = &statements[0] {
                        if let Expr::Identifier(n) | Expr::OwnedRef(n) = lhs {
                            if let Some(&idx) = self.field_index_map.get(n) {
                                let g_is_volatile = modifiers.iter().any(|h| h.name == "volatile");
                                let gvol = if g_is_volatile { " volatile" } else { "" };
                                let p = format!("%gp{}", self.txn_counter); self.txn_counter += 1;
                                let av = self.emit_expr(out, expr, indent);
                                let ty = self.field_types[idx].clone();
                                writeln!(out, "{}{} = getelementptr inbounds %State, %State* %state, i32 0, i32 {}", indent, p, idx).ok();
                                let se = format!("%gs{}", self.txn_counter); self.txn_counter += 1;
                                match ty.as_str() {
                                    "i8" => {
                                        let ld = format!("%gl{}", self.txn_counter); self.txn_counter += 1;
                                        writeln!(out, "{}{} = load i8, i8* {}, align {}", indent, ld, p, self.align_of(&ty)).ok();
                                        let av_tr = format!("%gatr{}", self.txn_counter); self.txn_counter += 1;
                                        writeln!(out, "{}{} = trunc i64 {} to i8", indent, av_tr, av).ok();
                                        writeln!(out, "{}{} = select i1 {}, i8 {}, i8 {}", indent, se, i1, av_tr, ld).ok();
                                        writeln!(out, "{}store{} i8 {}, i8* {}, align {}", indent, gvol, se, p, self.align_of(&ty)).ok();
                                    }
                                    "float" => {
                                        let ld = format!("%gl{}", self.txn_counter); self.txn_counter += 1;
                                        writeln!(out, "{}{} = load float, float* {}, align {}", indent, ld, p, self.align_of(&ty)).ok();
                                        let av_fl = self.native_float_or_box(out, indent, &av.to_string());
                                        writeln!(out, "{}{} = select i1 {}, float {}, float {}", indent, se, i1, av_fl, ld).ok();
                                        writeln!(out, "{}store{} float {}, float* {}, align {}", indent, gvol, se, p, self.align_of(&ty)).ok();
                                    }
                                    _ => {
                                        let ld = format!("%gl{}", self.txn_counter); self.txn_counter += 1;
                                        writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, ld, p).ok();
                                        writeln!(out, "{}{} = select i1 {}, i64 {}, i64 {}", indent, se, i1, av, ld).ok();
                                        writeln!(out, "{}store{} i64 {}, i64* {}, align {}", indent, gvol, se, p, self.align_of(&ty)).ok();
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
                let guard_id = format!("guard_{}", self.pgo_guard_idx);
                self.pgo_guard_idx += 1;
                if let Some(ref profile) = self.pgo_profile {
                    if let Some(prof) = crate::analysis::pgo::emit_branch_weights(profile, &guard_id) {
                        writeln!(out, "{}br i1 {}, label %{}, label %{}, {}", indent, i1, then_l, end_l, prof).ok();
                    } else {
                        writeln!(out, "{}br i1 {}, label %{}, label %{}", indent, i1, then_l, end_l).ok();
                    }
                } else {
                    writeln!(out, "{}br i1 {}, label %{}, label %{}", indent, i1, then_l, end_l).ok();
                }
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
                let target = self.variant_disc.get(name.as_str())
                    .map(|(_, d, _)| *d)
                    .unwrap_or(if name == "None" || name == "Err" { 0 } else { 1 });
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
    fn emit_expr(&mut self, out: &mut String, expr: &Expr, indent: &str) -> TypedRegister {
        let expr = if self.optimize_budget > 0 {
            crate::analysis::equality_saturation::simplify(expr)
        } else {
            expr.clone()
        };
        let v = format!("%t{}", self.txn_counter);
        self.txn_counter += 1;
        match &expr {
            Expr::Integer(n) => { writeln!(out, "{}{} = add i64 0, {}", indent, v, n).ok(); return TypedRegister { name: v, ty: Type::Int }; }
            Expr::Bool(b) => { writeln!(out, "{}{} = add i64 0, {}", indent, v, if *b { 1 } else { 0 }).ok(); return TypedRegister { name: v, ty: Type::Bool }; }
            Expr::Float(f) => {
                let bits = float_to_llvm_hex(*f);
                let fl = format!("%ff{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = bitcast i32 {} to float", indent, fl, bits).ok();
                let i32 = format!("%fi{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = bitcast float {} to i32", indent, i32, fl).ok();
                writeln!(out, "{}{} = zext i32 {} to i64", indent, v, i32).ok();
                self.reg_float_cache.insert(v.clone(), fl.clone());
                return TypedRegister { name: v, ty: Type::Float };
            }
            Expr::String(s) => {
                let si = self.string_constants.iter().position(|x| x == s).unwrap_or(0);
                let g = format!("@str.{}", si);
                let p = format!("%sp{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr inbounds [{} x i8], [{} x i8]* {}, i64 0, i64 0", indent, p, s.len() + 1, s.len() + 1, g).ok();
                writeln!(out, "{}{} = ptrtoint i8* {} to i64", indent, v, p).ok();
                return TypedRegister { name: v, ty: Type::String };
            }
            Expr::Char(c) => {
                let ci = format!("%cc{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = add i32 0, {}", indent, ci, *c as i32).ok();
                writeln!(out, "{}{} = zext i32 {} to i64", indent, v, ci).ok();
                return TypedRegister { name: v, ty: Type::Char };
            }
            Expr::Term => { writeln!(out, "{}{} = add i64 0, 0", indent, v).ok(); return TypedRegister { name: v, ty: Type::Int }; }
            Expr::Identifier(name) => {
                // SSA body mode: prefer pre-extracted old-value register
                // for int fields so all body ops are independent.
                if let Some(old_reg) = self.ssa_old_int_regs.get(name) {
                    writeln!(out, "{}{} = add i64 0, {}", indent, v, old_reg).ok();
                    return TypedRegister { name: v, ty: Type::Int };
                }
                // SSA body mode: prefer pre-extracted old-value register
                // for float fields so all body ops are independent.
                if let Some(old_reg) = self.ssa_old_float_regs.get(name) {
                    let i = format!("%if{}", self.txn_counter); self.txn_counter += 1;
                    writeln!(out, "{}{} = bitcast float {} to i32", indent, i, old_reg).ok();
                    writeln!(out, "{}{} = zext i32 {} to i64", indent, v, i).ok();
                    self.reg_float_cache.insert(v.clone(), old_reg.clone());
                    return TypedRegister { name: v, ty: Type::Float };
                }
                if let Some(ref ssa_reg) = self.ssa_state_reg.clone() {
                if let Some(&addr) = self.mmio_fields.get(name) {
                    let p = format!("%gep_exit_{}", self.txn_counter);
                    self.txn_counter += 1;
                    writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, p, addr).ok();
                    writeln!(out, "{}{} = load volatile i64, i64* {}, align 1", indent, v, p).ok();
                } else if let Some(&idx) = self.field_index_map.get(name) {
                        let ll_ty = &self.field_types[idx];
                        let ev = format!("%ev{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = extractvalue %State {}, {}", indent, ev, ssa_reg, idx).ok();
                        let field_ty = match ll_ty.as_str() {
                            "i8" => {
                                let z = format!("%iz{}", self.txn_counter); self.txn_counter += 1;
                                writeln!(out, "{}{} = zext i8 {} to i64", indent, z, ev).ok();
                                writeln!(out, "{}{} = add i64 0, {}", indent, v, z).ok();
                                Type::Bool
                            }
                            "float" => {
                                let fc = self.txn_counter; self.txn_counter += 1;
                                let float_reg = format!("%flt_{}_{}", name, fc);
                                writeln!(out, "{}{} = extractvalue %State {}, {}", indent, float_reg, ssa_reg, idx).ok();
                                let i = format!("%if{}", self.txn_counter); self.txn_counter += 1;
                                writeln!(out, "{}{} = bitcast float {} to i32", indent, i, float_reg).ok();
                                writeln!(out, "{}{} = zext i32 {} to i64", indent, v, i).ok();
                                self.reg_float_cache.insert(v.clone(), float_reg);
                                Type::Float
                            }
                            "i8*" => {
                                writeln!(out, "{}{} = ptrtoint i8* {} to i64", indent, v, ev).ok();
                                Type::String
                            }
                            _ => {
                                writeln!(out, "{}{} = add i64 0, {}", indent, v, ev).ok();
                                Type::Int
                            }
                        };
                        return TypedRegister { name: v, ty: field_ty };
                    }
                }
                if let Some(reg) = self.let_bindings.get(name) {
                    writeln!(out, "{}{} = add i64 0, {}", indent, v, reg).ok();
                    if let Some(ty) = self.let_binding_types.get(name) {
                        return TypedRegister { name: v, ty: ty.clone() };
                    }
                }
                if self.trigger_names.contains(name) {
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
                } else if let Some((ty, expr)) = self.constants.get(name) {
                    // Inline literal integer/bool constants as immediates
                    // instead of loading from global RAM.
                    match (ty, expr) {
                        (Type::Int | Type::UInt, Expr::Integer(n)) => {
                            writeln!(out, "{}{} = add i64 0, {}", indent, v, n).ok();
                            return TypedRegister { name: v, ty: Type::Int };
                        }
                        (Type::Bool, Expr::Bool(b)) => {
                            writeln!(out, "{}{} = add i64 0, {}", indent, v, if *b { 1 } else { 0 }).ok();
                            return TypedRegister { name: v, ty: Type::Bool };
                        }
                        _ => {
                            let ll_ty = match ty {
                                Type::Float => "float",
                                Type::Int | Type::UInt => "i64",
                                Type::Bool => "i8",
                                _ => "i64",
                            };
                            let ld = format!("%il{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = load {}, {}* @{}, align {}", indent, ld, ll_ty, ll_ty, name, self.align_of(ll_ty)).ok();
                            let ret_ty = match ty {
                                Type::Float => {
                                    let i = format!("%if{}", self.txn_counter); self.txn_counter += 1;
                                    writeln!(out, "{}{} = bitcast float {} to i32", indent, i, ld).ok();
                                    writeln!(out, "{}{} = zext i32 {} to i64", indent, v, i).ok();
                                    Type::Float
                                }
                                Type::Bool => {
                                    let z = format!("%iz{}", self.txn_counter); self.txn_counter += 1;
                                    writeln!(out, "{}{} = zext i8 {} to i64", indent, z, ld).ok();
                                    writeln!(out, "{}{} = add i64 0, {}", indent, v, z).ok();
                                    Type::Bool
                                }
                                _ => {
                                    writeln!(out, "{}{} = add i64 0, {}", indent, v, ld).ok();
                                    ty.clone()
                                }
                            };
                            return TypedRegister { name: v, ty: ret_ty };
                        }
                    }
                } else if let Some(&addr) = self.mmio_fields.get(name) {
                    let p = format!("%mio{}", self.txn_counter); self.txn_counter += 1;
                    writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, p, addr).ok();
                    let ld = format!("%mil{}", self.txn_counter); self.txn_counter += 1;
                    writeln!(out, "{}{} = load volatile i64, i64* {}, align 1", indent, ld, p).ok();
                    writeln!(out, "{}{} = add i64 0, {}", indent, v, ld).ok();
                } else if let Some(&idx) = self.field_index_map.get(name) {
                    let ty = &self.field_types[idx];
                    let p = format!("%fdp{}", self.txn_counter); self.txn_counter += 1;
                    writeln!(out, "{}{} = getelementptr inbounds %State, %State* %state, i32 0, i32 {}", indent, p, idx).ok();
                    let ld = format!("%il{}", self.txn_counter); self.txn_counter += 1;
                    let rng = self.field_to_meta_idx.get(name).map(|m| format!(", !range !{}", m)).unwrap_or_default();
                    writeln!(out, "{}{} = load {}, {}* {}, align {}{}", indent, ld, ty, ty, p, self.align_of(&ty), rng).ok();
                    match ty {
                        s if s == "i8" => { let z = format!("%iz{}", self.txn_counter); self.txn_counter += 1; writeln!(out, "{}{} = zext i8 {} to i64", indent, z, ld).ok(); writeln!(out, "{}{} = add i64 0, {}", indent, v, z).ok(); }
                        s if s == "float" => { let i = format!("%if{}", self.txn_counter); self.txn_counter += 1; writeln!(out, "{}{} = bitcast float {} to i32", indent, i, ld).ok(); writeln!(out, "{}{} = zext i32 {} to i64", indent, v, i).ok(); self.reg_float_cache.insert(v.clone(), ld.clone()); }
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
            Expr::Add(l, r) => { let ty = self.emit_binop(out, indent, &v, l, r, "add", "fadd"); return TypedRegister { name: v, ty }; }
            Expr::Sub(l, r) => { let ty = self.emit_binop(out, indent, &v, l, r, "sub", "fsub"); return TypedRegister { name: v, ty }; }
            Expr::Mul(l, r) => { let ty = self.emit_binop(out, indent, &v, l, r, "mul", "fmul"); return TypedRegister { name: v, ty }; }
            Expr::Div(l, r) => { let ty = self.emit_binop(out, indent, &v, l, r, "sdiv", "fdiv"); return TypedRegister { name: v, ty }; }
            Expr::Mod(l, r) => { let (a, b) = (self.emit_expr(out, l, indent), self.emit_expr(out, r, indent)); writeln!(out, "{}{} = srem i64 {}, {}", indent, v, a, b).ok(); }
            // Comparisons
            Expr::Eq(l, r) => { self.emit_fcmp(out, indent, &v, l, r, "oeq"); return TypedRegister { name: v, ty: Type::Bool }; }
            Expr::Ne(l, r) => { self.emit_fcmp(out, indent, &v, l, r, "one"); return TypedRegister { name: v, ty: Type::Bool }; }
            Expr::Lt(l, r) => { self.emit_fcmp(out, indent, &v, l, r, "olt"); return TypedRegister { name: v, ty: Type::Bool }; }
            Expr::Le(l, r) => { self.emit_fcmp(out, indent, &v, l, r, "ole"); return TypedRegister { name: v, ty: Type::Bool }; }
            Expr::Gt(l, r) => { self.emit_fcmp(out, indent, &v, l, r, "ogt"); return TypedRegister { name: v, ty: Type::Bool }; }
            Expr::Ge(l, r) => { self.emit_fcmp(out, indent, &v, l, r, "oge"); return TypedRegister { name: v, ty: Type::Bool }; }
            // Logical
            Expr::And(l, r) => { let (a, b) = (self.emit_expr(out, l, indent), self.emit_expr(out, r, indent)); writeln!(out, "{}{} = and i64 {}, {}", indent, v, a, b).ok(); }
            Expr::Or(l, r) => { let (a, b) = (self.emit_expr(out, l, indent), self.emit_expr(out, r, indent)); writeln!(out, "{}{} = or i64 {}, {}", indent, v, a, b).ok(); }
            Expr::Not(e) => { let inner = self.emit_expr(out, e, indent); writeln!(out, "{}{} = xor i64 {}, 1", indent, v, inner).ok(); }
            Expr::Neg(e) => {
                let inner = self.emit_expr(out, e, indent);
                if inner.ty == Type::Float {
                    let fl = self.native_float_or_box(out, indent, &inner.to_string());
                    let fs = format!("%nfs{}", self.txn_counter); self.txn_counter += 1;
                    let fi = format!("%nfi{}", self.txn_counter); self.txn_counter += 1;
                    writeln!(out, "{}{} = fsub fast float -0.0, {}", indent, fs, fl).ok();
                    writeln!(out, "{}{} = bitcast float {} to i32", indent, fi, fs).ok();
                    writeln!(out, "{}{} = zext i32 {} to i64", indent, v, fi).ok();
                    self.reg_float_cache.insert(v.clone(), fs.clone());
                    return TypedRegister { name: v, ty: Type::Float };
                } else {
                    writeln!(out, "{}{} = sub i64 0, {}", indent, v, inner.name).ok();
                    return TypedRegister { name: v, ty: Type::Int };
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
                let frgn_sig: Option<(Vec<(String, Type)>, crate::ast::ResultType)> = self.frgn_map.get(name).map(|s| (s.inputs.clone(), s.result_type.clone()));
                if let Some((inputs, ret_type)) = frgn_sig {
                    let mut marshaled: Vec<String> = Vec::new();
                    for (i, (_, arg_ty)) in inputs.iter().enumerate() {
                        if i < args.len() {
                            let raw = self.emit_expr(out, &args[i], indent);
                            match arg_ty {
                                Type::Int | Type::UInt => marshaled.push(format!("i64 {}", raw)),
                                Type::Bool => { let z = format!("%fz{}", self.txn_counter); self.txn_counter += 1; writeln!(out, "{}{} = trunc i64 {} to i32", indent, z, raw).ok(); marshaled.push(format!("i32 {}", z)); }
                                Type::Char => { let z = format!("%fz{}", self.txn_counter); self.txn_counter += 1; writeln!(out, "{}{} = trunc i64 {} to i32", indent, z, raw).ok(); marshaled.push(format!("i32 {}", z)); }
                                Type::Float => {
                                    let fl = self.native_float_or_box(out, indent, &raw.to_string());
                                    marshaled.push(format!("float {}", fl));
                                }
                                Type::String | Type::Data => { let p = format!("%fp{}", self.txn_counter); self.txn_counter += 1; writeln!(out, "{}{} = inttoptr i64 {} to i8*", indent, p, raw).ok(); marshaled.push(format!("i8* {}", p)); }
                                _ => marshaled.push(format!("i64 {}", raw)),
                            }
                        }
                    }
                    // Generic FFI call — no special-case magic
                    let is_float_ret = match &ret_type {
                        crate::ast::ResultType::Projection(ts) => ts.iter().any(|t| matches!(t, Type::Float)),
                        _ => false,
                    };
                    let call_ret = if is_float_ret { "float" } else { "i64" };
                    let args_str = marshaled.join(", ");
                    writeln!(out, "{}{} = call {} @{}({})", indent, v, call_ret, name, args_str).ok();
                    if is_float_ret {
                        let bi = format!("%fbi{}", self.txn_counter); self.txn_counter += 1;
                        let ze = format!("%fze{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = bitcast float {} to i32", indent, bi, v).ok();
                        writeln!(out, "{}{} = zext i32 {} to i64", indent, ze, bi).ok();
                        self.reg_float_cache.insert(ze.clone(), v.clone());
                        return TypedRegister { name: ze, ty: Type::Float };
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
                        let disc_val = self.variant_disc.get(name)
                            .map(|(_, d, _)| *d)
                            .unwrap_or_else(|| if name == "None" || name == "Err" { 0u64 } else { 1u64 });
                        let n_slots = a_strs.len() + 1;
                        let p = format!("%cop{}", self.txn_counter); self.txn_counter += 1;
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
            // Lists — 2-slot header layout: [data_ptr, length, elem0, elem1, ...]
            // The ptrtoint returns a pointer to data_ptr (slot 0).
            // ListIndex reads data_ptr from slot 0, then GEPs into elements.
            // ListLen reads length from slot 1.
            Expr::ListLiteral(elems) => {
                let n = elems.len();
                let n_slots = n + 2;
                let p = format!("%llp{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = alloca i64, i64 {}", indent, p, n_slots).ok();
                // Slot 0: pointer to first data element (slot 2)
                let dp = format!("%ldp{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 2", indent, dp, p).ok();
                let di = format!("%ldi{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = ptrtoint i64* {} to i64", indent, di, dp).ok();
                writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, di, p).ok();
                // Slot 1: length
                let lp = format!("%llp{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, lp, p).ok();
                writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, n, lp).ok();
                // Slots 2..n+1: elements
                for (ei, e) in elems.iter().enumerate() {
                    let ev = self.emit_expr(out, e, indent);
                    let ep = format!("%lep{}", self.txn_counter); self.txn_counter += 1;
                    writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, ep, p, ei + 2).ok();
                    writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, ev, ep).ok();
                }
                writeln!(out, "{}{} = ptrtoint i64* {} to i64", indent, v, p).ok();
            }
            Expr::ListIndex(list, idx) => {
                let l = self.emit_expr(out, list, indent);
                let i = self.emit_expr(out, idx, indent);
                // Check if the source is a Ptr<T> — if so, emit direct GEP instead of header load
                if self.is_ptr_expr(list) {
                    let dp = format!("%pdp{}", self.txn_counter); self.txn_counter += 1;
                    writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, dp, l).ok();
                    writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, v, dp, i).ok();
                } else {
                    let hp = format!("%lhp{}", self.txn_counter); self.txn_counter += 1;
                    writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, hp, l).ok();
                    // Load data pointer from slot 0
                    let dp = format!("%ldp{}", self.txn_counter); self.txn_counter += 1;
                    writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, dp, hp).ok();
                    let de = format!("%lde{}", self.txn_counter); self.txn_counter += 1;
                    writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, de, dp).ok();
                    writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, v, de, i).ok();
                }
            }
            Expr::Projection { source, target } => {
                let l = self.emit_expr(out, source, indent);
                match target {
                    ProjectionTarget::Size => {
                        // Load length from slot 1 of the 2-slot header
                        let hp = format!("%lhp{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, hp, l).ok();
                        let lp = format!("%llp{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, lp, hp).ok();
                        writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, v, lp).ok();
                    }
                    ProjectionTarget::Bytes => {
                        // Compile-time type size — default to 8 for all types
                        writeln!(out, "{}{} = add i64 0, 8", indent, v).ok();
                    }
                    ProjectionTarget::Ptr => {
                        // Load data pointer from slot 0 of the 2-slot header
                        let hp = format!("%php{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, hp, l).ok();
                        writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, v, hp).ok();
                    }
                    ProjectionTarget::Alignment => {
                        // Default alignment is 8 bytes
                        writeln!(out, "{}{} = add i64 0, 8", indent, v).ok();
                    }
                    ProjectionTarget::Range => {
                        // Range returns (min, max) — for LLVM, just return i64 range
                        writeln!(out, "{}{} = add i64 0, {}", indent, v, i64::MIN).ok();
                    }
                    ProjectionTarget::Popcount => {
                        writeln!(out, "{}{} = call i64 @llvm.ctpop.i64(i64 {})", indent, v, l).ok();
                    }
                    ProjectionTarget::LeadingZeros => {
                        writeln!(out, "{}{} = call i64 @llvm.ctlz.i64(i64 {}, i1 false)", indent, v, l).ok();
                    }
                    ProjectionTarget::TrailingZeros => {
                        writeln!(out, "{}{} = call i64 @llvm.cttz.i64(i64 {}, i1 false)", indent, v, l).ok();
                    }
                    ProjectionTarget::Absolute => {
                        writeln!(out, "{}{} = call i64 @llvm.abs.i64(i64 {}, i1 false)", indent, v, l).ok();
                    }
                    ProjectionTarget::BitReverse => {
                        writeln!(out, "{}{} = call i64 @llvm.bitreverse.i64(i64 {})", indent, v, l).ok();
                    }
                    ProjectionTarget::Type => {
                        // Type projection — compile-time constant, 0 for runtime
                        writeln!(out, "{}{} = add i64 0, 0 ; type", indent, v).ok();
                    }
                    ProjectionTarget::PtrBang => {
                        // Raw pointer — same as Ptr, load data pointer from slot 0
                        let hp = format!("%ppb{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, hp, l).ok();
                        writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, v, hp).ok();
                    }
                    ProjectionTarget::Match(pattern) => {
                        // Emit DFA table as constant global
                        match crate::analysis::dfa::compile_to_dfa(pattern) {
                            Ok(dfa) => {
                                // DFA state loop: iterate over input characters
                                // Load string source: data pointer from slot 0, length from slot 1
                                let hp = format!("%mhp{}", self.txn_counter); self.txn_counter += 1;
                                writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, hp, l).ok();
                                let dp = format!("%mdp{}", self.txn_counter); self.txn_counter += 1;
                                writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, dp, hp).ok();
                                let sp = format!("%msp{}", self.txn_counter); self.txn_counter += 1;
                                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, sp, hp).ok();
                                let slen = format!("%mslen{}", self.txn_counter); self.txn_counter += 1;
                                writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, slen, sp).ok();
                                let dp2 = format!("%mdp2{}", self.txn_counter); self.txn_counter += 1;
                                writeln!(out, "{}{} = inttoptr i64 {} to i8*", indent, dp2, dp).ok();

                                // State = 0, i = 0
                                let st = format!("%mst{}", self.txn_counter); self.txn_counter += 1;
                                writeln!(out, "{}{} = add i64 0, 0 ; state = 0", indent, st).ok();
                                let idx = format!("%midx{}", self.txn_counter); self.txn_counter += 1;
                                writeln!(out, "{}{} = add i64 0, 0 ; i = 0", indent, idx).ok();

                                // Loop: while i < len
                                let loop_label = format!("match_loop_{}", self.txn_counter);
                                let end_label = format!("match_end_{}", self.txn_counter);
                                let ok_label = format!("match_ok_{}", self.txn_counter);
                                self.txn_counter += 1;
                                writeln!(out, "{}br label %{}", indent, loop_label).ok();
                                writeln!(out, "{}:", loop_label).ok();

                                // Check i < len
                                let cond = format!("%mcond{}", self.txn_counter); self.txn_counter += 1;
                                writeln!(out, "{}{} = icmp slt i64 {}, {}", indent, cond, idx, slen).ok();
                                writeln!(out, "{}br i1 {}, label %{}, label %{}", indent, cond, ok_label, end_label).ok();

                                // Load char at i
                                let char_ptr = format!("%mcp{}", self.txn_counter); self.txn_counter += 1;
                                writeln!(out, "{}{} = getelementptr i8, i8* {}, i64 {}", indent, char_ptr, dp2, idx).ok();
                                let ch = format!("%mch{}", self.txn_counter); self.txn_counter += 1;
                                writeln!(out, "{}{} = load i8, i8* {}, align 1", indent, ch, char_ptr).ok();
                                let ch_ext = format!("%mche{}", self.txn_counter); self.txn_counter += 1;
                                writeln!(out, "{}{} = zext i8 {} to i64", indent, ch_ext, ch).ok();

                                // DFA transition: state = table[state][char]
                                // Emit a basic switch-like chain or compute 2D index
                                let table_size = dfa.dfa_table.len();
                                let table_elem = format!("%mte{}", self.txn_counter); self.txn_counter += 1;
                                writeln!(out, "{}{} = mul i64 {}, {}", indent, table_elem, st, 256i64).ok();
                                let table_idx = format!("%mti{}", self.txn_counter); self.txn_counter += 1;
                                writeln!(out, "{}{} = add i64 {}, {}", indent, table_idx, table_elem, ch_ext).ok();
                                // For now: state stays at 0 (stub)
                                writeln!(out, "{}{} = add i64 0, 0 ; state = state_next (stub)", indent, st).ok();

                                // i++
                                let idx_next = format!("%min{}", self.txn_counter); self.txn_counter += 1;
                                writeln!(out, "{}{} = add i64 {}, 1", indent, idx_next, idx).ok();
                                writeln!(out, "{}{} = add i64 {}, 0", indent, idx, idx_next).ok();
                                writeln!(out, "{}br label %{}", indent, loop_label).ok();

                                // OK: return 1 (match)
                                writeln!(out, "{}:", ok_label).ok();
                                writeln!(out, "{}{} = add i64 0, 1 ; match found", indent, v).ok();
                                writeln!(out, "{}br label %{}", indent, end_label).ok();

                                // End: return 0 (no match)
                                writeln!(out, "{}:", end_label).ok();
                                // v is already set for match-found; need phi for no-match
                                // For now: if we reach end without match, return 0
                                writeln!(out, "{}{} = phi i64 [ 1, %{} ], [ 0, %{} ]", indent, v, ok_label, loop_label).ok();
                            }
                            Err(_) => {
                                // Invalid regex — return 0 at runtime
                                writeln!(out, "{}{} = add i64 0, 0 ; invalid regex", indent, v).ok();
                            }
                        }
                    }
                }
            }
            Expr::Slice { value, start, end, stride, .. } => {
                let l = self.emit_expr(out, value, indent);
                // Load header: data_ptr from slot 0, len from slot 1
                let hp = format!("%shp{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, hp, l).ok();
                let dp = format!("%sdp{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, dp, hp).ok();
                let sp = format!("%ssp{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, sp, hp).ok();
                let slen_reg = format!("%sslen{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, slen_reg, sp).ok();
                // Compute actual start/end/stride
                let s_val = if let Some(s) = start {
                    let sv = self.emit_expr(out, s, indent);
                    sv.to_string()
                } else { "i64 0".to_string() };
                let e_val = if let Some(e) = end {
                    let ev = self.emit_expr(out, e, indent);
                    ev.to_string()
                } else { slen_reg.clone() };
                let stride_val = if let Some(st) = stride {
                    let sv = self.emit_expr(out, st, indent);
                    sv.to_string()
                } else { "i64 1".to_string() };
                // Compute result length: (end - start + stride - 1) / stride
                let rspan = format!("%srn{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = sub i64 {}, {}", indent, rspan, e_val, s_val).ok();
                let rsp2 = format!("%srs{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = add i64 {}, {}", indent, rsp2, rspan, stride_val).ok();
                let rsp3 = format!("%srt{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = sub i64 {}, 1", indent, rsp3, rsp2).ok();
                let rlen = format!("%srl{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = sdiv i64 {}, {}", indent, rlen, rsp3, stride_val).ok();
                // Allocate result list: n_slots = rlen + 2
                let rp = format!("%srp{}", self.txn_counter); self.txn_counter += 1;
                let rn_slots = format!("%srn2{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = add i64 {}, 2", indent, rn_slots, rlen).ok();
                // Stack allocate using constant bound when stride is compile-time known
                // Fall back to runtime alloca for variable stride
                writeln!(out, "{}{} = alloca i64, i64 {}", indent, rp, rn_slots).ok();
                // Store header: data_ptr at slot 0
                let rdp = format!("%srd{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 2", indent, rdp, rp).ok();
                let rdi = format!("%sri{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = ptrtoint i64* {} to i64", indent, rdi, rdp).ok();
                writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, rdi, rp).ok();
                // Store header: length at slot 1
                let rlp = format!("%srlp{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, rlp, rp).ok();
                writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, rlen, rlp).ok();
                // Copy loop: for i in 0..rlen, result[2+i] = source[start + i*stride]
                let loop_entry = format!("sc_e_{}", self.txn_counter); self.txn_counter += 1;
                let copy_hdr = format!("sc_h_{}", self.txn_counter); self.txn_counter += 1;
                let copy_body = format!("sc_b_{}", self.txn_counter); self.txn_counter += 1;
                let copy_end = format!("sc_d_{}", self.txn_counter); self.txn_counter += 1;
                // Data pointer for source
                let src_ep = format!("%sde{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, src_ep, dp).ok();
                // Jump to entry block (this terminates the current basic block)
                writeln!(out, "{}br label %{}", indent, loop_entry).ok();
                // Entry block: branches immediately to header
                writeln!(out, "{}:", loop_entry).ok();
                writeln!(out, "{}  br label %{}", indent, copy_hdr).ok();
                // Header: phi + condition
                let cnext = format!("%scn{}", self.txn_counter); self.txn_counter += 1;
                let ci = format!("%sci{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}:", copy_hdr).ok();
                writeln!(out, "{}  {} = phi i64 [ 0, %{} ], [ {}, %{} ]", indent, ci, loop_entry, cnext, copy_body).ok();
                let loop_cond = format!("%sclc{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}  {} = icmp slt i64 {}, {}", indent, loop_cond, ci, rlen).ok();
                writeln!(out, "{}  br i1 {}, label %{}, label %{}", indent, loop_cond, copy_body, copy_end).ok();
                // Body
                let cnext = format!("%scn{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}:", copy_body).ok();
                let src_idx = format!("%scs{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}  {} = mul i64 {}, {}", indent, src_idx, ci, stride_val).ok();
                let src_off = format!("%sco{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}  {} = add i64 {}, {}", indent, src_off, src_idx, s_val).ok();
                let src_gep = format!("%scg{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}  {} = getelementptr i64, i64* {}, i64 {}", indent, src_gep, src_ep, src_off).ok();
                let sv = format!("%scv{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}  {} = load i64, i64* {}, align 8", indent, sv, src_gep).ok();
                let dst_idx = format!("%scd{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}  {} = add i64 {}, 2", indent, dst_idx, ci).ok();
                let dst_gep = format!("%scdp{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}  {} = getelementptr i64, i64* {}, i64 {}", indent, dst_gep, rp, dst_idx).ok();
                writeln!(out, "{}  store i64 {}, i64* {}, align 8", indent, sv, dst_gep).ok();
                writeln!(out, "{}  {} = add i64 {}, 1", indent, cnext, ci).ok();
                writeln!(out, "{}  br label %{}", indent, copy_hdr).ok();
                // End
                writeln!(out, "{}:", copy_end).ok();
                writeln!(out, "{}{} = ptrtoint i64* {} to i64", indent, v, rp).ok();
            }
            Expr::MultiSlice { value, coordinates, .. } => {
                let l = self.emit_expr(out, value, indent);
                // Delegate to Slice or Index per coordinate, matching the interpreter
                // at interpreter.rs:1848. For a single coordinate, pass through.
                // For multiple, emit as a series of slices.
                if coordinates.len() == 1 {
                    match &coordinates[0] {
                        crate::ast::SliceCoordinate::Index(idx) => {
                            // Delegate to ListIndex: reuse the ListIndex logic
                            let hp = format!("%mhp{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, hp, l).ok();
                            let dp = format!("%mdp{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, dp, hp).ok();
                            let de = format!("%mde{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, de, dp).ok();
                            let idx_val = self.emit_expr(out, idx, indent);
                            writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, v, de, idx_val).ok();
                        }
                        crate::ast::SliceCoordinate::Range { start, end, .. } => {
                            let sv = if let Some(s) = start {
                                let r = self.emit_expr(out, s, indent); r.to_string()
                            } else { "i64 0".to_string() };
                            let hp = format!("%mhp{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, hp, l).ok();
                            let dp = format!("%mdp{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, dp, hp).ok();
                            let de = format!("%mde{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, de, dp).ok();
                            writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, v, de, sv).ok();
                        }
                        crate::ast::SliceCoordinate::Named { coord, .. } => {
                            match coord.as_ref() {
                                crate::ast::SliceCoordinate::Index(idx) => {
                                    let hp = format!("%mhp{}", self.txn_counter); self.txn_counter += 1;
                                    writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, hp, l).ok();
                                    let dp = format!("%mdp{}", self.txn_counter); self.txn_counter += 1;
                                    writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, dp, hp).ok();
                                    let de = format!("%mde{}", self.txn_counter); self.txn_counter += 1;
                                    writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, de, dp).ok();
                                    let idx_val = self.emit_expr(out, idx, indent);
                                    writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, v, de, idx_val).ok();
                                }
                                _ => { writeln!(out, "{}{} = add i64 0, {} ; multi-slice", indent, v, l).ok(); }
                            }
                        }
                        crate::ast::SliceCoordinate::AtDimension { coord, .. } => {
                            // Delegate to inner coordinate
                            match coord.as_ref() {
                                crate::ast::SliceCoordinate::Index(idx) => {
                                    let hp = format!("%mhp{}", self.txn_counter); self.txn_counter += 1;
                                    writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, hp, l).ok();
                                    let dp = format!("%mdp{}", self.txn_counter); self.txn_counter += 1;
                                    writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, dp, hp).ok();
                                    let de = format!("%mde{}", self.txn_counter); self.txn_counter += 1;
                                    writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, de, dp).ok();
                                    let idx_val = self.emit_expr(out, idx, indent);
                                    writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, v, de, idx_val).ok();
                                }
                                _ => { writeln!(out, "{}{} = add i64 0, {} ; multi-slice", indent, v, l).ok(); }
                            }
                        }
                        crate::ast::SliceCoordinate::Ellipsis => {
                            writeln!(out, "{}{} = add i64 0, {} ; multi-slice", indent, v, l).ok();
                        }
                    }
                } else {
                    writeln!(out, "{}{} = add i64 0, {} ; multi-slice", indent, v, l).ok();
                }
            }
            Expr::Tuple(elems) => {
                let n = elems.len();
                let n_slots = n + 2;
                let p = format!("%tp{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = alloca i64, i64 {}", indent, p, n_slots).ok();
                let dp = format!("%tpd{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 2", indent, dp, p).ok();
                let di = format!("%tpi{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = ptrtoint i64* {} to i64", indent, di, dp).ok();
                writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, di, p).ok();
                let lp = format!("%tpl{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, lp, p).ok();
                writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, n, lp).ok();
                for (ei, e) in elems.iter().enumerate() {
                    let ev = self.emit_expr(out, e, indent);
                    let ep = format!("%tpe{}", self.txn_counter); self.txn_counter += 1;
                    writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, ep, p, ei + 2).ok();
                    writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, ev, ep).ok();
                }
                writeln!(out, "{}{} = ptrtoint i64* {} to i64", indent, v, p).ok();
            }
            Expr::TupleDestructure(names, expr) => {
                let inner = self.emit_expr(out, expr, indent);
                let hp = format!("%tdh{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, hp, inner).ok();
                let dp = format!("%tdd{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, dp, hp).ok();
                for (ei, name) in names.iter().enumerate() {
                    let de = format!("%tde{}", self.txn_counter); self.txn_counter += 1;
                    writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, de, dp).ok();
                    let ep = format!("%tdg{}", self.txn_counter); self.txn_counter += 1;
                    writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, ep, de, ei).ok();
                    let ld = format!("%tdl{}", self.txn_counter); self.txn_counter += 1;
                    writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, ld, ep).ok();
                    let reg = format!("%tdr{}", self.txn_counter); self.txn_counter += 1;
                    writeln!(out, "{}{} = add i64 0, {}", indent, reg, ld).ok();
                    self.let_bindings.insert(name.clone(), reg);
                }
                writeln!(out, "{}{} = add i64 0, {} ; destructure", indent, v, inner).ok();
            }
            Expr::StructInstance(typename, fields) => {
                let n = fields.len();
                let p = format!("%sp{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = alloca i64, i64 {}", indent, p, n).ok();
                let mut fvs: Vec<String> = Vec::new();
                for (_, expr) in fields.iter() {
                    let ev = self.emit_expr(out, expr, indent);
                    fvs.push(ev.name);
                }
                for (fi, fv) in fvs.iter().enumerate() {
                    let ep = format!("%sep{}", self.txn_counter); self.txn_counter += 1;
                    writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, ep, p, fi).ok();
                    writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, fv, ep).ok();
                }
                writeln!(out, "{}{} = ptrtoint i64* {} to i64", indent, v, p).ok();
                let ret_ty = Type::Custom(typename.clone());
                return TypedRegister { name: v, ty: ret_ty };
            }
            Expr::ObjectLiteral(fields) => {
                let n = fields.len();
                let p = format!("%op{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = alloca i64, i64 {}", indent, p, n).ok();
                let mut fvs: Vec<String> = Vec::new();
                for (_, expr) in fields.iter() {
                    let ev = self.emit_expr(out, expr, indent);
                    fvs.push(ev.name);
                }
                for (fi, fv) in fvs.iter().enumerate() {
                    let ep = format!("%oep{}", self.txn_counter); self.txn_counter += 1;
                    writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, ep, p, fi).ok();
                    writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, fv, ep).ok();
                }
                writeln!(out, "{}{} = ptrtoint i64* {} to i64", indent, v, p).ok();
            }
            Expr::FieldAccess(obj, field_name) => {
                let obj_reg = self.emit_expr(out, obj, indent);
                let f_ptr = format!("%fap{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, f_ptr, obj_reg).ok();
                if let Type::Custom(out_ty) = &obj_reg.ty {
                    if let Some(struct_fields) = self.struct_types.get(out_ty) {
                        if let Some(fi) = struct_fields.iter().position(|(n, _)| n == field_name) {
                            let gep = format!("%fag{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, gep, f_ptr, fi).ok();
                            writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, v, gep).ok();
                            return TypedRegister { name: v, ty: struct_fields[fi].1.clone() };
                        }
                    }
                }
                writeln!(out, "{}{} = add i64 0, 0 ; field", indent, v).ok();
            }
            // Cast
            Expr::Cast(inner, target_ty) => {
                let src_reg = self.emit_expr(out, inner, indent);
                let src_ty = Some(src_reg.ty.clone());
                self.emit_cast_convert(out, indent, &v, &src_reg.name, src_ty, target_ty);
                return TypedRegister { name: v, ty: target_ty.clone() };
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
                let mut disc_to_vi: HashMap<u64, String> = HashMap::new();
                for arm in arms {
                    if let MatchPattern::Variant { name: vname, .. } = &arm.pattern {
                        let d = self.variant_disc.get(vname).map(|(_, d, _)| *d).unwrap_or(vi);
                        let label = format!("%ma{}_{}", mid, vi);
                        disc_to_vi.insert(d, label.clone());
                        writeln!(out, "{}  i64 {}, label {}", indent, d, label).ok();
                        vi += 1;
                    }
                }
                writeln!(out, "{}]", indent).ok();
                let mut phi_v: Vec<String> = Vec::new();
                let mut phi_l: Vec<String> = Vec::new();
                vi = 0;
                for arm in arms {
                    if let MatchPattern::Variant { name: vname, fields } = &arm.pattern {
                        let d = self.variant_disc.get(vname).map(|(_, d, _)| *d).unwrap_or(vi);
                        let label = disc_to_vi.get(&d).cloned().unwrap_or_else(|| format!("%ma{}_{}", mid, vi));
                        writeln!(out, "{}:", label).ok();
                        // Bind variant fields: GEP into payload slots and register as let bindings
                        let inner_ptr = format!("%mei{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, inner_ptr, inner).ok();
                        for (fi, fname) in fields.iter().enumerate() {
                            let gep = format!("%mfg{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, gep, inner_ptr, fi + 1).ok();
                            let ld = format!("%mfl{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, ld, gep).ok();
                            let reg = format!("%mfr{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = add i64 0, {}", indent, reg, ld).ok();
                            self.let_bindings.insert(fname.clone(), reg);
                        }
                        let av = self.emit_expr(out, &arm.body, indent);
                        phi_v.push(av.name); phi_l.push(format!("%%ma{}_{}", mid, vi));
                        writeln!(out, "{}br label %{}", indent, merge).ok();
                        vi += 1;
                    }
                }
                if has_wc {
                    if let Some(wc) = arms.iter().find(|a| a.pattern == MatchPattern::Wildcard) {
                        writeln!(out, "{}:", def_l).ok();
                        let wv = self.emit_expr(out, &wc.body, indent);
                        phi_v.push(wv.name); phi_l.push(format!("%%{}", def_l));
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
                let target = self.variant_disc.get(variant.as_str())
                    .map(|(_, d, _)| *d)
                    .unwrap_or(if variant == "None" || variant == "Err" { 0 } else { 1 });
                let cmp = format!("%pc{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = icmp eq i64 {}, {}", indent, cmp, disc, target).ok();
                writeln!(out, "{}{} = zext i1 {} to i64", indent, v, cmp).ok();
            }
            // Arrow mutation — uses 2-slot list header [data_ptr, length, elem0, elem1, ...]
            // List pointer is a ptrtoint i64* → i64. Elements start at slot 2.
            Expr::ArrowMut { dir, target, index, value } => {
                let list = self.emit_expr(out, target, indent);
                let is_full_range = matches!(index.as_ref(), Expr::Term);
                let hp = format!("%ahp{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, hp, list).ok();
                let lp = format!("%alp{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, lp, hp).ok();
                let len = format!("%alen{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, len, lp).ok();
                match dir {
                    ArrowDir::Push => {
                        let val = self.emit_expr(out, value.as_ref().unwrap(), indent);
                        let pos_name = if is_full_range {
                            // Append at end: position = len (0-indexed), element slot = len + 2
                            let p = format!("%apos{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = add i64 {}, 2", indent, p, len).ok();
                            p
                        } else {
                            // Insert at index: index value
                            let idx = self.emit_expr(out, index, indent);
                            idx.name
                        };
                        let ep = format!("%aep{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, ep, hp, pos_name).ok();
                        writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, val.name, ep).ok();
                        let new_len = format!("%anl{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = add i64 {}, 1", indent, new_len, len).ok();
                        writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, new_len, lp).ok();
                        writeln!(out, "{}{} = ptrtoint i64* {} to i64", indent, v, hp).ok();
                    }
                    ArrowDir::Pop => {
                        let pos_name = if is_full_range {
                            // Pop from end: last element slot = len + 1
                            let p = format!("%apos{}", self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "{}{} = add i64 {}, 1", indent, p, len).ok();
                            p
                        } else {
                            let idx = self.emit_expr(out, index, indent);
                            idx.name
                        };
                        let ep = format!("%aep{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 {}", indent, ep, hp, pos_name).ok();
                        writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, v, ep).ok();
                        let new_len = format!("%anl{}", self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "{}{} = sub i64 {}, 1", indent, new_len, len).ok();
                        writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, new_len, lp).ok();
                    }
                }
            }
            Expr::ArrowDiscard { target, index } => {
                let list = self.emit_expr(out, target, indent);
                let hp = format!("%dhp{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = inttoptr i64 {} to i64*", indent, hp, list).ok();
                let lp = format!("%dlp{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = getelementptr i64, i64* {}, i64 1", indent, lp, hp).ok();
                let len = format!("%dlen{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, len, lp).ok();
                let new_len = format!("%dnl{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "{}{} = sub i64 {}, 1", indent, new_len, len).ok();
                writeln!(out, "{}store i64 {}, i64* {}, align 8", indent, new_len, lp).ok();
                writeln!(out, "{}{} = add i64 0, 0 ; discard", indent, v).ok();
            }
            Expr::Ellipsis => {
                writeln!(out, "{}{} = add i64 0, 0 ; ellipsis", indent, v).ok();
            }
            // Fallback
            _ => { writeln!(out, "{}{} = add i64 0, 0 ; expr", indent, v).ok(); }
        }
        // Default: treat as Int. Float operations are handled explicitly
        // by emit_binop/emit_fcmp which return Type::Float/Bool respectively.
        TypedRegister { name: v, ty: Type::Int }
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

        writeln!(out, "define void @reactor_tick(%State* noalias nocapture %state) local_unnamed_addr #2 {{").ok();
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
        } else if fusable.is_empty()
            && dispatch.len() >= 2
            && crate::analysis::transition_graph::is_uniform_body_group(txns)
        {
            // Uniform dispatch: all bodies are structurally identical.
            // Skip precondition evaluation entirely — the body is the same
            // regardless of which txn fires. Emit the first body directly.
            // The main loop's exit check handles termination.
            writeln!(out, "  call void @{}(%State* %state)", dispatch[0]).ok();
            writeln!(out, "  ret void").ok();
        } else {
            // Phase 1: Evaluate ALL preconditions in the entry block against the
            // pre-tick state. This prevents the cascade bug where txn N+1's
            // precondition reads state mutated by txn N's body.
            // Phase 3a: After switch-dispatch detection (transition_graph.rs),
            // this serial precondition evaluation may be replaced by a switch.
            let mut pre_regs: Vec<String> = Vec::with_capacity(dispatch.len());
            for (i, txn_name) in dispatch.iter().enumerate() {
                let has_pre = self.dispatch_has_pre(txns, txn_name);
                if has_pre {
                    let reg = format!("%pr{}", i);
                    let txn = self.resolve_dispatch_first_txn(txn_name);
                    writeln!(out, "  {} = call i1 @pre_{}(%State* %state)", reg, txn).ok();
                    pre_regs.push(reg);
                } else {
                    pre_regs.push("true".to_string());
                }
            }

            // Phase 2: Chain through body execution using saved precondition results.
            // Each body fires iff its precondition was true on the pre-tick state.
            for (i, txn_name) in dispatch.iter().enumerate() {
                let b = format!("b{}", i);
                let c = format!("ck{}", i);
                let pr = &pre_regs[i];
                writeln!(out, "  br i1 {}, label %{}, label %{}", pr, b, c).ok();
                writeln!(out, "{}:", b).ok();
                writeln!(out, "  call void @{}(%State* %state)", txn_name).ok();
                writeln!(out, "  br label %{}", c).ok();
                writeln!(out, "{}:", c).ok();
            }
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

        writeln!(out, "define void @reactor_tick(%State* noalias nocapture %state) local_unnamed_addr #2 {{").ok();
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
                    writeln!(out, "  %pr{} = call i1 @pre_{}(%State* %state)", i, first_txn).ok();
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
                writeln!(out, "  call void @{}(%State* %state)", txn_name).ok();
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
                    && !self.trigger_names.contains(name)
                {
                    errors.push(format!(
                        "error: #!exit references unknown variable '{}'\n  note: '{}' is not a state field, constant, or trigger",
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
        // Leaf expressions: delegate integer/bool to emit_expr for constant
        // inlining. Keep Identifier/OwnedRef local because exit conditions
        // Access %state pointer (passed as parameter or via alloca in main)
        match expr {
            Expr::Integer(_) | Expr::Bool(_) | Expr::Float(_) | Expr::Neg(_) => {
                return self.emit_expr(out, expr, indent).name;
            }
            _ => {}
        }
        let v = format!("%t{}", self.txn_counter);
        self.txn_counter += 1;
        match expr {
            Expr::Identifier(name) => {
                if let Some(&idx) = self.field_index_map.get(name) {
                    let p = format!("%gep_exit_{}", self.txn_counter);
                    self.txn_counter += 1;
                    writeln!(out, "{}{} = getelementptr inbounds %State, %State* %state, i32 0, i32 {}", indent, p, idx).ok();
                    writeln!(out, "{}{} = load i64, i64* {}, align 8", indent, v, p).ok();
                } else if self.constants.contains_key(name) {
                    writeln!(out, "{}{} = load i64, i64* @{}, align 8", indent, v, name).ok();
                } else if self.trigger_names.contains(name) {
                    if let Some(t) = self.triggers.get(name).cloned() {
                        let addr_str = match &t.address {
                            crate::ast::LinkRef::Explicit(a) => a.to_string(),
                            crate::ast::LinkRef::Linked(s) => format!("@{}", s),
                        };
                        let addr_is_ptr = matches!(t.address, crate::ast::LinkRef::Linked(_));
                        self.emit_trg_load(out, indent, &v, &addr_str, addr_is_ptr, &t.ty);
                    } else {
                        writeln!(out, "{}{} = add i64 0, 0", indent, v).ok();
                    }
                } else {
                    writeln!(out, "{}{} = add i64 0, 0 ; unknown id '{}'", indent, v, name).ok();
                }
                v
            }
            Expr::OwnedRef(name) => {
                return self.emit_exit_expr(out, &Expr::Identifier(name.clone()), indent);
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
        writeln!(out, "define i32 @main() local_unnamed_addr {} {{", self.slp_attr("main", "#3")).ok();
        writeln!(out, "  entry:").ok();
        writeln!(out, "  %state = alloca %State, align 8").ok();
        writeln!(out, "  call void @init_state(%State* noalias nocapture %state)").ok();
        if has_wake_triggers {
            writeln!(out, "  call void @__rt_init()").ok();
            writeln!(out, "  call void @__rt_poll()").ok();
        }
        if self.has_async_txns && !self.is_lightweight_async {
            let count = self.async_txn_names.len() as i32;
            writeln!(out, "  %tp_fn_ptr = bitcast [{} x void (%State*)*]* @thread_pool_fns to i8**", self.async_txn_names.len()).ok();
            writeln!(out, "  call void @brief_thread_pool_init(i32 {}, i8** %tp_fn_ptr)", count).ok();
        }
        writeln!(out, "  br label %tick").ok();
        writeln!(out, "  tick:").ok();
        if self.has_async_txns && !self.is_lightweight_async {
            self.emit_async_phase(out);
        } else {
            writeln!(out, "  call void @reactor_tick(%State* noalias nocapture %state)").ok();
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

    /// Pre-extract all float fields from the current SSA state register
    /// into named old-value registers. Body statements that read float
    /// fields will use these old-value registers, making all float
    /// operations within the iteration independent — LLVM's scheduler can
    /// then fill all CPU float execution ports simultaneously.
    fn pre_extract_float_fields(&mut self, out: &mut String) {
        let ssa_reg = match self.ssa_state_reg.clone() {
            Some(r) => r,
            None => return,
        };
        self.ssa_old_float_regs.clear();
        for (field_name, &field_idx) in &self.field_index_map {
            if self.field_types[field_idx] == "float" {
                let old_reg = format!("%{}_old_{}", field_name, self.txn_counter);
                self.txn_counter += 1;
                writeln!(out, "  {} = extractvalue %State {}, {}", old_reg, ssa_reg, field_idx).ok();
                self.ssa_old_float_regs.insert(field_name.clone(), old_reg);
            }
        }
    }

    /// Pre-extract all non-Float state fields into SSA registers before the body.
    /// Mirrors `pre_extract_float_fields` for Int fields. This eliminates the
    /// per-reference extractvalue-from-insertvalue-chain pattern that inflates
    /// the SSA body by ~5× for Int-heavy benchmarks.
    fn pre_extract_int_fields(&mut self, out: &mut String) {
        let ssa_reg = match self.ssa_state_reg.clone() {
            Some(r) => r,
            None => return,
        };
        self.ssa_old_int_regs.clear();
        for (field_name, &field_idx) in &self.field_index_map {
            if self.field_types[field_idx] != "float" {
                let old_reg = format!("%{}_old_{}", field_name, self.txn_counter);
                self.txn_counter += 1;
                writeln!(out, "  {} = extractvalue %State {}, {}", old_reg, ssa_reg, field_idx).ok();
                self.ssa_old_int_regs.insert(field_name.clone(), old_reg);
            }
        }
    }

    /// Emit the folded while-loop body (without `@init_state()` or the enclosing
    /// `define` / `ret`).  Used by both `emit_folded_main` and the enum dispatch path.
    ///
    /// When `use_phi = true`, the counter lives in an SSA phi node (register)
    /// instead of being loaded/stored through %state every iteration.
    /// Only valid when the txn body is pure (just counter++).
    ///
    /// When `use_phi = false` and `body = Some(stmts)`, the txn body is emitted
    /// inline with struct-SSA (load `%State` once, insertvalue chains, store once).
    /// When `use_phi = false` and `body = None`, calls the txn function as before.
    fn emit_folded_loop(
        &mut self,
        out: &mut String,
        txn_name: &str,
        counter_idx: usize,
        total_idx: Option<usize>,
        total_const_name: Option<&str>,
        label_prefix: &str,
        use_phi: bool,
        body: Option<&[Statement]>,
        unroll_factor: usize,
        is_decreasing: bool,
        bound_literal: Option<i64>,
    ) {
        let c0 = self.txn_counter;
        if use_phi {
            let entry_label = format!("{}_phi_entry", label_prefix);
            let hdr_label = format!("{}_hdr", label_prefix);
            let body_label = format!("{}_body", label_prefix);
            let done_label = format!("{}_done", label_prefix);
            writeln!(out, "{}:", entry_label).ok();
            // Load bound once
            if let Some(ti) = total_idx {
                writeln!(out, "  %gt_{}_{} = getelementptr inbounds %State, %State* %state, i32 0, i32 {}", label_prefix, c0, ti).ok();
                writeln!(out, "  %lt_{}_{} = load i64, i64* %gt_{}_{}, align 8", label_prefix, c0, label_prefix, c0).ok();
            } else if let Some(cn) = total_const_name {
                writeln!(out, "  %lt_{}_{} = load i64, i64* @{}, align 8", label_prefix, c0, cn).ok();
            } else {
                writeln!(out, "  %lt_{}_{} = add i64 0, 0", label_prefix, c0).ok();
            }
            // Load counter once, precompute remaining iterations
            writeln!(out, "  %gcnt_{}_{} = getelementptr inbounds %State, %State* %state, i32 0, i32 {}", label_prefix, c0, counter_idx).ok();
            writeln!(out, "  %init_{}_{} = load i64, i64* %gcnt_{}_{}, align 8", label_prefix, c0, label_prefix, c0).ok();
            // Counted-down loop: remaining = bound - initial, count down to 0.
            // This eliminates the cmp instruction (sub sets ZF for jne) and
            // matches what clang emits for C for-loops.
            writeln!(out, "  %rem_{}_{} = sub i64 %lt_{}_{}, %init_{}_{}", label_prefix, c0 + 1, label_prefix, c0, label_prefix, c0).ok();
            writeln!(out, "  br label %{}", hdr_label).ok();
            writeln!(out, "{}:", hdr_label).ok();
            writeln!(out, "  %i_{}_{} = phi i64 [ %rem_{}_{}, %{} ], [ %dec_{}_{}, %{} ]", label_prefix, c0 + 2, label_prefix, c0 + 1, entry_label, label_prefix, c0 + 2, body_label).ok();
            writeln!(out, "  %cp_{}_{} = icmp sgt i64 %i_{}_{}, 0", label_prefix, c0 + 3, label_prefix, c0 + 2).ok();
            writeln!(out, "  br i1 %cp_{}_{}, label %{}, label %{}", label_prefix, c0 + 3, body_label, done_label).ok();
            writeln!(out, "{}:", body_label).ok();
            writeln!(out, "  %dec_{}_{} = sub i64 %i_{}_{}, 1", label_prefix, c0 + 2, label_prefix, c0 + 2).ok();
            writeln!(out, "  br label %{}", hdr_label).ok();
            writeln!(out, "{}:", done_label).ok();
            // Final counter value is always the bound after counted-down loop
            writeln!(out, "  store i64 %lt_{}_{}, i64* %gcnt_{}_{}, align 8", label_prefix, c0, label_prefix, c0).ok();
        } else if let Some(stmts) = body {
            // SSA mode: load once, phi in header, inline unrolled body with extract/insert, store once
            if let Some(bl) = bound_literal {
                writeln!(out, "  %lt{}_{} = add i64 0, {}", label_prefix, c0, bl).ok();
            } else if let Some(ti) = total_idx {
                writeln!(out, "  %gt{}_{} = getelementptr inbounds %State, %State* %state, i32 0, i32 {}", label_prefix, c0, ti).ok();
                writeln!(out, "  %lt{}_{} = load i64, i64* %gt{}_{}, align 8", label_prefix, c0, label_prefix, c0).ok();
            } else if let Some(cn) = total_const_name {
                writeln!(out, "  %lt{}_{} = load i64, i64* @{}, align 8", label_prefix, c0, cn).ok();
            } else {
                writeln!(out, "  %lt{}_{} = add i64 0, 0", label_prefix, c0).ok();
            }
            let phi_reg = format!("%ssa_phi_{}", label_prefix);
            let unroll = unroll_factor.max(1);
            let unroll_minus_1 = unroll - 1;

            // --- body4: unrolled loop body ---
            let mut body4_buf = String::new();
            if unroll > 1 {
                writeln!(body4_buf, "{}_body4:", label_prefix).ok();
                let mut cur = phi_reg.clone();
                for _ in 0..unroll {
                    self.let_bindings.clear(); self.let_binding_types.clear(); self.reg_float_cache.clear(); self.reg_type_cache.clear();
                    self.terminated = false;
                    self.returns_i64 = false;
                    self.ssa_state_reg = Some(cur);
                    // Pre-extract all float fields from the entering state
                    // so body field reads use old values — all float ops
                    // become independent, filling all CPU execution ports.
                    self.pre_extract_float_fields(&mut body4_buf);
                    self.pre_extract_int_fields(&mut body4_buf);
                    for stmt in stmts.iter().filter(|s| !matches!(s, Statement::Term { .. } | Statement::TermBang { .. })) {
                        self.emit_stmt(&mut body4_buf, stmt, "  ");
                    }
                    self.ssa_old_float_regs.clear();
                    self.ssa_old_int_regs.clear();
                    cur = self.ssa_state_reg.take().unwrap_or(phi_reg.clone());
                }
                let backedge4 = cur;
                writeln!(body4_buf, "  store %State {}, %State* %slot_{}, align 8", backedge4, label_prefix).ok();
                writeln!(body4_buf, "  br label %{}_hdr", label_prefix).ok();
            }

            // --- body1: remainder loop (single iteration) ---
            let mut body1_buf = String::new();
            writeln!(body1_buf, "{}_body1:", label_prefix).ok();
            self.let_bindings.clear(); self.let_binding_types.clear(); self.reg_float_cache.clear(); self.reg_type_cache.clear();
            self.terminated = false;
            self.returns_i64 = false;
            self.ssa_state_reg = Some(phi_reg.clone());
            self.pre_extract_float_fields(&mut body1_buf);
            self.pre_extract_int_fields(&mut body1_buf);
            for stmt in stmts.iter().filter(|s| !matches!(s, Statement::Term { .. } | Statement::TermBang { .. })) {
                self.emit_stmt(&mut body1_buf, stmt, "  ");
            }
            let backedge_val = self.ssa_state_reg.take().unwrap_or(phi_reg.clone());
            writeln!(body1_buf, "  store %State {}, %State* %slot_{}, align 8", backedge_val, label_prefix).ok();
            writeln!(body1_buf, "  br label %{}_hdr", label_prefix).ok();

            // Build initial %State from known constants
            writeln!(out, "  br label %{}_pre", label_prefix).ok();
            writeln!(out, "{}_pre:", label_prefix).ok();
            let mut cur_init = "zeroinitializer".to_string();
            let mut fields: Vec<(String, usize, String)> = self.field_index_map.iter()
                .map(|(name, &idx)| (name.clone(), idx, self.field_types[idx].clone()))
                .collect();
            fields.sort_by_key(|&(_, idx, _)| idx);
            for (name, idx, ty) in &fields {
                let init = self.field_initializers.get(name).and_then(|e| e.as_ref());
                match init {
                    Some(Expr::Float(f)) => {
                        let h = float_to_llvm_hex(*f);
                        let bc = format!("%fbc{}_{}", label_prefix, self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "  {} = bitcast i32 {} to float", bc, h).ok();
                        let iv = format!("%fiv{}_{}", label_prefix, self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "  {} = insertvalue %State {}, float {}, {}", iv, cur_init, bc, idx).ok();
                        cur_init = iv;
                    }
                    Some(Expr::Integer(n)) => {
                        let iv = format!("%iiv{}_{}", label_prefix, self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "  {} = insertvalue %State {}, i64 {}, {}", iv, cur_init, n, idx).ok();
                        cur_init = iv;
                    }
                    Some(Expr::Bool(b)) => {
                        let v = if *b { 1 } else { 0 };
                        let iv = format!("%biv{}_{}", label_prefix, self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "  {} = insertvalue %State {}, i8 {}, {}", iv, cur_init, v, idx).ok();
                        cur_init = iv;
                    }
                    Some(Expr::Neg(inner)) => {
                        let s = match inner.as_ref() {
                            Expr::Float(f) => float_to_llvm_hex(-*f),
                            Expr::Integer(n) => format!("-{}", n),
                            _ => "0".to_string(),
                        };
                        if ty == "float" {
                            let bc = format!("%nbc{}_{}", label_prefix, self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "  {} = bitcast i32 {} to float", bc, s).ok();
                            let iv = format!("%niv{}_{}", label_prefix, self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "  {} = insertvalue %State {}, float {}, {}", iv, cur_init, bc, idx).ok();
                            cur_init = iv;
                        } else {
                            let iv = format!("%niv{}_{}", label_prefix, self.txn_counter); self.txn_counter += 1;
                            writeln!(out, "  {} = insertvalue %State {}, i64 {}, {}", iv, cur_init, s, idx).ok();
                            cur_init = iv;
                        }
                    }
                    Some(Expr::String(_)) => {
                        let iv = format!("%siv{}_{}", label_prefix, self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "  {} = insertvalue %State {}, i8* null, {}", iv, cur_init, idx).ok();
                        cur_init = iv;
                    }
                    Some(Expr::Char(c)) => {
                        let v = *c as i32;
                        let iv = format!("%civ{}_{}", label_prefix, self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "  {} = insertvalue %State {}, i32 {}, {}", iv, cur_init, v, idx).ok();
                        cur_init = iv;
                    }
                    _ => {
                        let gep = format!("%gep{}_{}", label_prefix, self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "  {} = getelementptr inbounds %State, %State* %state, i32 0, i32 {}", gep, idx).ok();
                        let ld = format!("%ld{}_{}", label_prefix, self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "  {} = load {}, {}* {}, align {}", ld, ty, ty, gep, self.align_of(&ty)).ok();
                        let iv = format!("%liv{}_{}", label_prefix, self.txn_counter); self.txn_counter += 1;
                        writeln!(out, "  {} = insertvalue %State {}, {} {}, {}", iv, cur_init, ty, ld, idx).ok();
                        cur_init = iv;
                    }
                }
            }
            let slot = format!("%slot_{}", label_prefix);
            writeln!(out, "  {} = alloca %State, align 8", slot).ok();
            writeln!(out, "  store %State {}, %State* {}, align 8", cur_init, slot).ok();
            writeln!(out, "  br label %{}_hdr", label_prefix).ok();

            // Header: extract counter, compare with adjusted/un-adjusted bounds
            writeln!(out, "{}_hdr:", label_prefix).ok();
            writeln!(out, "  {} = load %State, %State* {}, align 8", phi_reg, slot).ok();
            writeln!(out, "  %ex{}_{} = extractvalue %State {}, {}", label_prefix, self.txn_counter, phi_reg, counter_idx).ok();
            let ex_reg = format!("%ex{}_{}", label_prefix, self.txn_counter); self.txn_counter += 1;

            if unroll > 1 {
                let adj = format!("%adj{}_{}", label_prefix, self.txn_counter); self.txn_counter += 1;
                if is_decreasing {
                    writeln!(out, "  {} = add i64 %lt{}_{}, {}", adj, label_prefix, c0, unroll_minus_1).ok();
                } else {
                    writeln!(out, "  {} = add i64 %lt{}_{}, -{}", adj, label_prefix, c0, unroll_minus_1).ok();
                }
                let cp4 = format!("%cp{}_{}", label_prefix, self.txn_counter); self.txn_counter += 1;
                if is_decreasing {
                    writeln!(out, "  {} = icmp sgt i64 {}, {}", cp4, ex_reg, adj).ok();
                } else {
                    writeln!(out, "  {} = icmp slt i64 {}, {}", cp4, ex_reg, adj).ok();
                }
                writeln!(out, "  br i1 {}, label %{}_body4, label %{}_rem", cp4, label_prefix, label_prefix).ok();
                writeln!(out, "{}_rem:", label_prefix).ok();
            }
            let cp1 = format!("%cp{}_{}", label_prefix, self.txn_counter); self.txn_counter += 1;
            if is_decreasing {
                writeln!(out, "  {} = icmp sgt i64 {}, %lt{}_{}", cp1, ex_reg, label_prefix, c0).ok();
            } else {
                writeln!(out, "  {} = icmp slt i64 {}, %lt{}_{}", cp1, ex_reg, label_prefix, c0).ok();
            }
            writeln!(out, "  br i1 {}, label %{}_body1, label %{}_done", cp1, label_prefix, label_prefix).ok();

            if unroll > 1 {
                out.push_str(&body4_buf);
            }
            out.push_str(&body1_buf);

            let final_reg = format!("%final_{}", label_prefix);
            writeln!(out, "{}_done:", label_prefix).ok();
            writeln!(out, "  {} = load %State, %State* %slot_{}, align 8", final_reg, label_prefix).ok();
            writeln!(out, "  store %State {}, %State* %state, align 8", final_reg).ok();
        } else {
            if let Some(bl) = bound_literal {
                writeln!(out, "  %lt{}_{} = add i64 0, {}", label_prefix, c0, bl).ok();
            } else if let Some(ti) = total_idx {
                writeln!(out, "  %gt{}_{} = getelementptr inbounds %State, %State* %state, i32 0, i32 {}", label_prefix, c0, ti).ok();
                writeln!(out, "  %lt{}_{} = load i64, i64* %gt{}_{}, align 8", label_prefix, c0, label_prefix, c0).ok();
            } else if let Some(cn) = total_const_name {
                writeln!(out, "  %lt{}_{} = load i64, i64* @{}, align 8", label_prefix, c0, cn).ok();
            } else {
                writeln!(out, "  %lt{}_{} = add i64 0, 0", label_prefix, c0).ok();
            }
            writeln!(out, "  br label %{}_hdr", label_prefix).ok();
            writeln!(out, "{}_hdr:", label_prefix).ok();
            writeln!(out, "  %gp{}_{} = getelementptr inbounds %State, %State* %state, i32 0, i32 {}", label_prefix, c0 + 1, counter_idx).ok();
            writeln!(out, "  %lp{}_{} = load i64, i64* %gp{}_{}, align 8", label_prefix, c0 + 1, label_prefix, c0 + 1).ok();
            let cmp_reg = format!("%cp{}_{}", label_prefix, c0 + 2);
            if is_decreasing {
                writeln!(out, "  {} = icmp sgt i64 %lp{}_{}, %lt{}_{}", cmp_reg, label_prefix, c0 + 1, label_prefix, c0).ok();
            } else {
                writeln!(out, "  {} = icmp slt i64 %lp{}_{}, %lt{}_{}", cmp_reg, label_prefix, c0 + 1, label_prefix, c0).ok();
            }
            writeln!(out, "  br i1 {}, label %{}_body, label %{}_done", cmp_reg, label_prefix, label_prefix).ok();
            writeln!(out, "{}_body:", label_prefix).ok();
            writeln!(out, "  call void @{}(%State* %state)", txn_name).ok();
            writeln!(out, "  br label %{}_hdr", label_prefix).ok();
            writeln!(out, "{}_done:", label_prefix).ok();
        }
    }

    fn emit_folded_main(
        &mut self,
        out: &mut String,
        txn_name: &str,
        counter_idx: usize,
        total_idx: Option<usize>,
        total_const_name: Option<&str>,
        use_phi: bool,
        body: Option<&[Statement]>,
    ) {
        writeln!(out, "define i32 @main() local_unnamed_addr {} {{", self.slp_attr("main", "#0")).ok();
        writeln!(out, "  entry:").ok();
        writeln!(out, "  %state = alloca %State, align 8").ok();
        writeln!(out, "  call void @init_state(%State* noalias nocapture %state)").ok();
        // Legacy phi-mode: uses
        if use_phi {
            writeln!(out, "  br label %case_phi_entry").ok();
        }
        let uf = if !use_phi && body.is_some() { 4 } else { 1 };
        self.emit_folded_loop(out, txn_name, counter_idx, total_idx, total_const_name, "case", use_phi, body, uf, false, None);
        writeln!(out, "  ret i32 0").ok();
        writeln!(out, "}}").ok();
        writeln!(out).ok();
    }

    /// Emit a `main()` that uses struct-SSA for all-convergent programs.
    /// Loads %State once per tick, runs each reactive txn's precondition check
    /// and body inline with extractvalue/insertvalue, stores %State once.
    /// For multi-txn programs where ALL reactive txns have bounded_pre + increments
    /// but are NOT foldable/precomputable/enum/async-pipeline (e.g. precompute_sum_runtime).
    fn emit_ssa_main(
        &mut self,
        out: &mut String,
        txns: &[(String, &crate::ast::Transaction)],
    ) {
        writeln!(out, "define i32 @main() local_unnamed_addr {} {{", self.slp_attr("main", "#0")).ok();
        writeln!(out, "  entry:").ok();
        writeln!(out, "  %state = alloca %State, align 8").ok();
        writeln!(out, "  call void @init_state(%State* noalias nocapture %state)").ok();
        writeln!(out, "  br label %tick").ok();
        writeln!(out, "  tick:").ok();
        let ss0 = format!("%ss{}", self.txn_counter); self.txn_counter += 1; // line 3277 in ssa_main
        writeln!(out, "  {} = load %State, %State* %state, align 8", ss0).ok();
        self.ssa_state_reg = Some(ss0.clone());
        for (name, txn) in txns.iter().filter(|(_, t)| t.is_reactive) {
            let pre = &txn.contract.pre_condition;
            if !matches!(pre, Expr::Bool(true)) {
                let pre_ssa = self.ssa_state_reg.clone().unwrap_or_else(|| ss0.clone());
                let cond = self.emit_expr(out, pre, "  ");
                let i1 = format!("%pi{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "  {} = icmp ne i64 {}, 0", i1, cond).ok();
                let body_l = format!("b_{}", name);
                let skip_l = format!("s_{}", name);
                writeln!(out, "  br i1 {}, label %{}, label %{}", i1, body_l, skip_l).ok();
                writeln!(out, "  {}:", body_l).ok();
                self.let_bindings.clear(); self.let_binding_types.clear(); self.reg_float_cache.clear(); self.reg_type_cache.clear();
                self.terminated = false;
                self.returns_i64 = false;
                self.pre_extract_float_fields(out);
                self.pre_extract_int_fields(out);
                for s in txn.body.iter().filter(|s| !matches!(s, Statement::Term { .. } | Statement::TermBang { .. })) { self.emit_stmt(out, s, "  "); }
                self.ssa_old_float_regs.clear();
                self.ssa_old_int_regs.clear();
                let after_body = self.ssa_state_reg.clone().unwrap_or_else(|| pre_ssa.clone());
                writeln!(out, "  br label %{}", skip_l).ok();
                writeln!(out, "  {}:", skip_l).ok();
                let merge = format!("%me{}", self.txn_counter); self.txn_counter += 1;
                writeln!(out, "  {} = phi %State [ {}, %{} ], [ {}, %{} ]",
                    merge, after_body, body_l, pre_ssa, skip_l).ok();
                self.ssa_state_reg = Some(merge);
            } else {
                self.let_bindings.clear(); self.let_binding_types.clear(); self.reg_float_cache.clear(); self.reg_type_cache.clear();
                self.terminated = false;
                self.returns_i64 = false;
                self.pre_extract_float_fields(out);
                self.pre_extract_int_fields(out);
                for s in txn.body.iter().filter(|s| !matches!(s, Statement::Term { .. } | Statement::TermBang { .. })) { self.emit_stmt(out, s, "  "); }
                self.ssa_old_float_regs.clear();
                self.ssa_old_int_regs.clear();
            }
        }
        let final_reg = self.ssa_state_reg.take().unwrap_or(ss0);
        writeln!(out, "  store %State {}, %State* %state, align 8", final_reg).ok();
        if let Some(ref cond) = self.exit_condition.clone() {
            let val = self.emit_exit_expr(out, cond, "  ");
            let tr = format!("%t{}", self.txn_counter); self.txn_counter += 1;
            writeln!(out, "  {} = trunc i64 {} to i1", tr, val).ok();
            writeln!(out, "  br i1 {}, label %done, label %tick", tr).ok();
            writeln!(out, "  done:").ok();
        } else {
            writeln!(out, "  br label %tick").ok();
        }
        writeln!(out, "  ret i32 0").ok();
        writeln!(out, "}}").ok();
        writeln!(out).ok();
    }

    /// Emit a `main()` that folds ALL reactive transactions into a single
    /// register-pipeline loop.  Each txn gets an SSA phi node for its counter;
    /// the loop terminates when all counters reach their bounds.
    /// Assumes all txns are pure/effectively-pure with bounded_pre + increments.
    /// After the entry setup, performs enum trigger dispatch and switch-based
    /// execution (merged from the original emit_enum_main design).
    fn emit_folded_multi_main(
        &mut self,
        out: &mut String,
        txns: &[(String, &crate::ast::Transaction)],
        enum_sizes: &[(String, Option<u64>)],
        enum_keys: &HashMap<String, Vec<i64>>,
        fold_params: &HashMap<String, FoldParam>,
        fold_pure: &HashMap<String, (bool, Option<i64>)>,
        counter_idx: usize,
        total_idx: Option<usize>,
        total_const_name: Option<&str>,
        composed_fn: Option<&str>,
        composed_trig_map: Option<&HashMap<String, Vec<(i64, String)>>>,
        all_internal_map: Option<&HashMap<String, (usize, i64)>>,
        has_wake: bool,
    ) {
        let c0 = self.txn_counter;
        // Deduplicate by counter index: multiple txns may share the same counter
        let mut uniq: Vec<(usize, String)> = Vec::new();
        let mut seen_idxs: std::collections::HashSet<usize> = std::collections::HashSet::new();
        let mut first_tidx: Option<usize> = None;
        for (_, fp) in fold_params.iter() {
            if seen_idxs.insert(fp.counter_idx) {
                uniq.push((fp.counter_idx, format!("c{}", fp.counter_idx)));
                if first_tidx.is_none() {
                    first_tidx = fp.bound_field_idx;
                }
            }
        }
        let main_attr = self.slp_attr("main", if has_wake { "#3" } else { "#0" });
        writeln!(out, "define i32 @main() local_unnamed_addr {} {{", main_attr).ok();
        writeln!(out, "  entry:").ok();
        writeln!(out, "  %state = alloca %State, align 8").ok();
        writeln!(out, "  call void @init_state(%State* noalias nocapture %state)").ok();
        if has_wake {
            writeln!(out, "  call void @__rt_init()").ok();
            writeln!(out, "  call void @__rt_poll()").ok();
        }
        writeln!(out, "  br label %tick").ok();
        writeln!(out, "tick:").ok();

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
            else if self.has_async_txns && !self.is_lightweight_async { "async_phase" }
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
                for (ptxn_name, fp) in fold_params.iter() {
                    let sub_prefix = format!("{}_{}", prefix, ptxn_name);
                    if let Some(&(pure, tv)) = fold_pure.get(ptxn_name) {
                        if pure {
                            if let Some(tv) = tv {
                                writeln!(out, "  %pc_{} = getelementptr inbounds %State, %State* %state, i32 0, i32 {}", sub_prefix, fp.counter_idx).ok();
                                writeln!(out, "  store i64 {}, i64* %pc_{}, align 8", tv, sub_prefix).ok();
                                continue;
                            } else {
                                let ptcn_ref = fp.bound_const_name.as_deref();
                                this.emit_folded_loop(out, ptxn_name, fp.counter_idx, fp.bound_field_idx, ptcn_ref, &sub_prefix, true, None, 1, fp.is_decreasing, fp.bound_literal);
                                continue;
                            }
                        }
                    }
                    let ptcn_ref = fp.bound_const_name.as_deref();
                    let body = txns.iter().find(|(n, _)| n == ptxn_name).map(|(_, t)| t.body.as_slice());
                    this.emit_folded_loop(out, ptxn_name, fp.counter_idx, fp.bound_field_idx, ptcn_ref, &sub_prefix, false, body, 4, fp.is_decreasing, fp.bound_literal);
                }
            } else {
                let body = txns.iter().find(|(n, _)| n == fn_name).map(|(_, t)| t.body.as_slice());
                this.emit_folded_loop(out, fn_name, ci, ti, tcn, prefix, false, body, 4, false, None);
            }
        };

        if total_combos == 1 && enum_sizes.len() == 1 {
            // Single-value trigger: just fall through to the loop
            let fn_name = trig_to_fn.get(&0).map(|s| s.as_str()).unwrap_or(txn_name);
            if let Some((ci, tv)) = all_internal_lookup(fn_name) {
                writeln!(out, "  %pc_sc = getelementptr inbounds %State, %State* %state, i32 0, i32 {}", ci).ok();
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
            // Use extracted keys when available, otherwise fall back to dense 0..n
            let keys: Vec<i64> = enum_keys.get(tn).cloned().unwrap_or_else(|| (0..n as i64).collect());

            // Check if all case arms produce identical code (uniform-body skip).
            // When trig_to_fn maps all keys to the same function and all have
            // the same all-internal status, the switch dispatch is redundant.
            let uniform_body = keys.len() > 1 && {
                let first_fn = trig_to_fn.get(&keys[0]).map(|s| s.as_str()).unwrap_or(&native_name);
                let first_ai = all_internal_lookup(first_fn);
                keys[1..].iter().all(|k| {
                    let fn_name = trig_to_fn.get(k).map(|s| s.as_str()).unwrap_or(&native_name);
                    fn_name == first_fn && all_internal_lookup(fn_name) == first_ai
                })
            };

            if uniform_body {
                // All case arms identical — skip the switch, emit one body
                let fn_name = trig_to_fn.get(&keys[0]).map(|s| s.as_str()).unwrap_or(&native_name);
                if let Some((ci, tv)) = all_internal_lookup(fn_name) {
                    writeln!(out, "  %pc_uni = getelementptr inbounds %State, %State* %state, i32 0, i32 {}", ci).ok();
                    writeln!(out, "  store i64 {}, i64* %pc_uni, align 8", tv).ok();
                } else {
                    emit_case_folded_loops(self, out, "uni", fn_name, counter_idx, total_idx, total_const_name);
                }
                if has_wake {
                    writeln!(out, "  br label %{}", done_label).ok();
                } else {
                    writeln!(out, "  ret i32 0").ok();
                }
                // Residual label for safety (unreachable for fully-covered enums)
                writeln!(out, "{}_residual:", tn).ok();
                writeln!(out, "  call void @reactor_tick(%State* noalias nocapture %state)").ok();
                if has_wake {
                    writeln!(out, "  br label %{}", done_label).ok();
                } else {
                    writeln!(out, "  br label %{}_residual_loop", tn).ok();
                    writeln!(out, "{}_residual_loop:", tn).ok();
                    writeln!(out, "  call void @reactor_tick(%State* noalias nocapture %state)").ok();
                    writeln!(out, "  br label %{}_residual_loop", tn).ok();
                }
            } else {
            let key_count = keys.len();
            // Try perfect hashing for sparse key sets (gap ratio > 4).
            let (use_hash, multiplier, hash_shift): (bool, u64, u32) =
                if sparsity_ratio(&keys) > 4.0 {
                    if let Some((m, s)) = find_perfect_hash(&keys) {
                        (true, m, s)
                    } else { (false, 0, 0) }
                } else { (false, 0, 0) };
            let dispatch_val = if use_hash {
                // Emit perfect hash: h(k) = (k * M) >> S
                writeln!(out, "  %hm_{} = mul i64 %sz_{}, {}", c0, tn, multiplier).ok();
                writeln!(out, "  %hs_{} = lshr i64 %hm_{}, {}", c0, c0, hash_shift).ok();
                format!("%hs_{}", c0)
            } else {
                format!("%sz_{}", tn)
            };
            writeln!(out, "  switch i64 {}, label %{}_residual [", dispatch_val, tn).ok();
            for (idx, _key) in keys.iter().enumerate() {
                let label = format!("{}_{}", tn, idx);
                writeln!(out, "    i64 {}, label %{}_case_{}", idx, tn, idx).ok();
            }
            writeln!(out, "  ]").ok();
            for (idx, key) in keys.iter().enumerate() {
                let prefix = format!("{}_{}", tn, idx);
                writeln!(out, "{}_case_{}:", tn, idx).ok();
                // For hashed dispatch, verify the original key matches (safety guard)
                if use_hash {
                    writeln!(out, "  %vg_{}_{} = icmp eq i64 %sz_{}, {}", c0, idx, tn, key).ok();
                    writeln!(out, "  br i1 %vg_{}_{}, label %{}_safe_{}, label %{}_residual", c0, idx, tn, idx, tn).ok();
                    writeln!(out, "{}_safe_{}:", tn, idx).ok();
                }
                let fn_name = trig_to_fn.get(key).map(|s| s.as_str()).unwrap_or(&native_name);
                if let Some((ci, tv)) = all_internal_lookup(fn_name) {
                    writeln!(out, "  %pc_{} = getelementptr inbounds %State, %State* %state, i32 0, i32 {}", prefix, ci).ok();
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
            writeln!(out, "  call void @reactor_tick(%State* noalias nocapture %state)").ok();
            if has_wake {
                writeln!(out, "  br label %{}", done_label).ok();
            } else {
                writeln!(out, "  br label %{}_residual_loop", tn).ok();
                writeln!(out, "{}_residual_loop:", tn).ok();
                writeln!(out, "  call void @reactor_tick(%State* noalias nocapture %state)").ok();
                writeln!(out, "  br label %{}_residual_loop", tn).ok();
            }
            }
        } else {
            // Multi-trigger case: just fall through to standard reactor
            if has_wake {
                writeln!(out, "  call void @reactor_tick(%State* noalias nocapture %state)").ok();
                writeln!(out, "  br label %{}", done_label).ok();
            } else {
                writeln!(out, "  br label %residual_entry").ok();
                writeln!(out, "residual_entry:").ok();
                writeln!(out, "  call void @init_state(%State* noalias nocapture %state)").ok();
                writeln!(out, "  br label %residual_loop").ok();
                writeln!(out, "residual_loop:").ok();
                writeln!(out, "  call void @reactor_tick(%State* noalias nocapture %state)").ok();
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
                if self.has_async_txns && !self.is_lightweight_async {
                    writeln!(out, "  br i1 {}, label %done, label %async_phase", tr).ok();
                } else {
                    writeln!(out, "  br i1 {}, label %done, label %do_wait", tr).ok();
                }
            }
            if self.has_async_txns && !self.is_lightweight_async {
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
        writeln!(out, "define i32 @main() local_unnamed_addr {} {{", self.slp_attr("main", "#0")).ok();
        writeln!(out, "  entry:").ok();
        writeln!(out, "  %state = alloca %State, align 8").ok();
        writeln!(out, "  call void @init_state(%State* noalias nocapture %state)").ok();
        writeln!(out, "  %gp = getelementptr inbounds %State, %State* %state, i32 0, i32 {}", counter_idx).ok();
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
        writeln!(out, "define i32 @main() local_unnamed_addr {} {{", self.slp_attr("main", "#0")).ok();
        writeln!(out, "  entry:").ok();
        writeln!(out, "  %state = alloca %State, align 8").ok();
        writeln!(out, "  call void @init_state(%State* noalias nocapture %state)").ok();
        let mut seen: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for (_, bindings) in final_values {
            for (var, val) in bindings {
                if !seen.insert(var) { continue; }
                if let Some(&idx) = self.field_index_map.get(var) {
                    let ty = &self.field_types[idx];
                    writeln!(out, "  %gp_{} = getelementptr inbounds %State, %State* %state, i32 0, i32 {}", var, idx).ok();
                    match ty.as_str() {
                        "float" => {
                            let bits = *val as i32 as u32;
                            writeln!(out, "  store float bitcast (i32 {} to float), float* %gp_{}, align 4", bits, var).ok();
                        }
                        "i8" => {
                            writeln!(out, "  store i8 {}, i8* %gp_{}, align 1", val, var).ok();
                        }
                        _ => {
                            writeln!(out, "  store i64 {}, i64* %gp_{}, align 8", val, var).ok();
                        }
                    }
                } else if let Some(&addr) = self.mmio_fields.get(var) {
                    writeln!(out, "  %gp_{} = inttoptr i64 {} to i64*", var, addr).ok();
                    writeln!(out, "  store volatile i64 {}, i64* %gp_{}, align 1", val, var).ok();
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
        if !self.has_async_txns || self.is_lightweight_async { return; }
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
        if !self.has_async_txns || self.is_lightweight_async { return; }
        writeln!(out, "  call void @brief_barrier_release()").ok();
        // Sequential reactor runs in main thread concurrently with workers
        writeln!(out, "  call void @reactor_tick(%State* noalias nocapture %state)").ok();
        writeln!(out, "  call void @brief_barrier_wait()").ok();
    }

    // ── FUSABLE PAIRS ────────────────────────────────────────
    fn resolve_fusable_pairs(&self, txns: &[(String, &crate::ast::Transaction)]) -> Vec<(String, String)> {
        let prg = crate::ast::Program {
            items: txns.iter().map(|(_, t)| crate::ast::TopLevel::Transaction((*t).clone())).collect(),
            comments: vec![], reactor_speed: None, attrs: vec![], ffi: None, strict_mode: crate::ast::StrictMode::Off, dispatch_mode: crate::ast::DispatchMode::Sequential, exit_condition: None, out_pragmas: vec![], default_sig_modifier: None,
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
                let _ = writeln!(out, "{}{} = fcmp fast une float {}, 0.0", indent, ci, fl);
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

    fn i64_to_float_reg(&mut self, out: &mut String, reg: &str, indent: &str) -> String {
        // Check cache first: these are actual float registers from SSA extraction
        // or float literal caching. Do NOT check register_types here — that map
        // tracks Brief-level float semantics, not LLVM type (boxed as i64).
        if let Some(cached) = self.reg_float_cache.get(reg) {
            return cached.clone();
        }
        let tr = format!("%ftr{}", self.txn_counter); self.txn_counter += 1;
        let fl = format!("%ffl{}", self.txn_counter); self.txn_counter += 1;
        writeln!(out, "{}{} = trunc i64 {} to i32", indent, tr, reg).ok();
        writeln!(out, "{}{} = bitcast i32 {} to float", indent, fl, tr).ok();
        fl
    }

    fn emit_binop(&mut self, out: &mut String, indent: &str, v: &str, l: &Expr, r: &Expr, int_op: &str, float_op: &str) -> Type {
        // Peephole: constant-fold integer binops at compile time
        if let (Expr::Integer(li), Expr::Integer(ri)) = (l, r) {
            let result = match int_op {
                "add" => Some(li.wrapping_add(*ri)),
                "sub" => Some(li.wrapping_sub(*ri)),
                "mul" => Some(li.wrapping_mul(*ri)),
                "sdiv" if *ri != 0 => Some(li / ri),
                "and" => Some(li & ri),
                "or"  => Some(li | ri),
                "xor" => Some(li ^ ri),
                "shl" => Some(li.wrapping_shl(*ri as u32)),
                "lshr" => Some((*li as u64).wrapping_shr(*ri as u32) as i64),
                _ => None,
            };
            if let Some(folded) = result {
                writeln!(out, "{}{} = add i64 0, {}", indent, v, folded).ok();
                return Type::Int;
            }
        }
        let (a, b) = (self.emit_expr(out, l, indent), self.emit_expr(out, r, indent));
        if a.ty == Type::Float || b.ty == Type::Float {
            let fa = self.i64_to_float_reg(out, &a.name, indent);
            let fb = self.i64_to_float_reg(out, &b.name, indent);
            let fr = format!("%bfr{}", self.txn_counter); self.txn_counter += 1;
            writeln!(out, "{}{} = {} fast float {}, {}", indent, fr, float_op, fa, fb).ok();
            let fi = format!("%bfi{}", self.txn_counter); self.txn_counter += 1;
            writeln!(out, "{}{} = bitcast float {} to i32", indent, fi, fr).ok();
            writeln!(out, "{}{} = zext i32 {} to i64", indent, v, fi).ok();
            self.reg_float_cache.insert(v.to_string(), fr.clone());
            Type::Float
        } else {
            writeln!(out, "{}{} = {} i64 {}, {}", indent, v, int_op, a.name, b.name).ok();
            Type::Int
        }
    }

    fn emit_fcmp(&mut self, out: &mut String, indent: &str, v: &str, l: &Expr, r: &Expr, cond: &str) -> Type {
        // Peephole: constant-fold integer comparisons at compile time
        if let (Expr::Integer(li), Expr::Integer(ri)) = (l, r) {
            let result = match cond {
                "oeq" => li == ri,
                "one" => li != ri,
                "olt" => li < ri,
                "ole" => li <= ri,
                "ogt" => li > ri,
                "oge" => li >= ri,
                _ => false,
            };
            writeln!(out, "{}{} = add i64 0, {}", indent, v, if result { 1 } else { 0 }).ok();
            return Type::Bool;
        }
        let (a, b) = (self.emit_expr(out, l, indent), self.emit_expr(out, r, indent));
        let c = format!("%c{}", self.txn_counter); self.txn_counter += 1;
        if a.ty == Type::Float || b.ty == Type::Float {
            let fa = self.i64_to_float_reg(out, &a.name, indent);
            let fb = self.i64_to_float_reg(out, &b.name, indent);
            writeln!(out, "{}{} = fcmp fast {} float {}, {}", indent, c, cond, fa, fb).ok();
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
            writeln!(out, "{}{} = icmp {} i64 {}, {}", indent, c, icmp_cond, a.name, b.name).ok();
        }
        writeln!(out, "{}{} = zext i1 {} to i64", indent, v, c).ok();
        Type::Bool
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
        out_pragmas: vec![],
        default_sig_modifier: None,
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
        out_pragmas: vec![],
        default_sig_modifier: None,
        };
        let output = backend.generate(&program);
        assert!(output.contains("%State"));
        assert!(output.contains("i64"));
        assert!(output.contains("%state"));
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
                        Statement::Term { values: vec![], modifiers: vec![], swan_song: None },
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
        out_pragmas: vec![],
        default_sig_modifier: None,
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
                    body: vec![Statement::Term { values: vec![], modifiers: vec![], swan_song: None }],
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
        out_pragmas: vec![],
        default_sig_modifier: None,
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
                    body: vec![Statement::Term { values: vec![], modifiers: vec![], swan_song: None }],
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
                    body: vec![Statement::Term { values: vec![], modifiers: vec![], swan_song: None }],
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
        out_pragmas: vec![],
        default_sig_modifier: None,
        };
        let output = backend.generate(&program);

        // @ link trigger emits external global
        assert!(output.contains("external global"), "Should declare external globals for @ link");
        assert!(output.contains("__io_pending"), "Should contain trigger global name");

        // Fall-through dispatch: body blocks don't end with ret void
        assert!(output.contains("reactor_tick"), "Should have reactor_tick");
        assert!(output.contains("%state"), "Should reference state pointer");
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
                        Statement::Term { values: vec![], modifiers: vec![], swan_song: None },
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
        out_pragmas: vec![],
        default_sig_modifier: None,
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
                        Statement::Term { values: vec![], modifiers: vec![], swan_song: None },
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
        out_pragmas: vec![],
        default_sig_modifier: None,
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
                        Statement::Term { values: vec![], modifiers: vec![], swan_song: None },
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
        out_pragmas: vec![],
        default_sig_modifier: None,
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
                        Statement::Term { values: vec![], modifiers: vec![], swan_song: None },
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
        out_pragmas: vec![],
        default_sig_modifier: None,
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
        // With uniform-body detection: identical case arms skip the switch dispatch.
        let program = make_wake_trg_program("sig", "__sigint_flag", Type::Bool, true);
        let output = LlvmBackend::new().generate(&program);
        assert!(output.contains("call void @__rt_wait()"),
            "Wake triggers get __rt_wait between ticks");
        assert!(output.contains("call void @__rt_init()"),
            "Wake triggers get __rt_init at startup");
        assert!(!output.contains("switch i64"),
            "Uniform enum bodies skip the switch dispatch");
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
                    body: vec![Statement::Term { values: vec![], modifiers: vec![], swan_song: None }],
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
        out_pragmas: vec![],
        default_sig_modifier: None,
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
                        Statement::Term { values: vec![], modifiers: vec![], swan_song: None },
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
        out_pragmas: vec![],
        default_sig_modifier: None,
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
                        Statement::Term { values: vec![], modifiers: vec![], swan_song: None },
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
        out_pragmas: vec![],
        default_sig_modifier: None,
        };
        let output = backend.generate(&program);
        assert!(output.contains("fadd fast float"),
            "Float binary add should emit fadd fast float");
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
        assert!(output.contains("define void @reactor_tick(%State* noalias nocapture %state) local_unnamed_addr #2"),
            "reactor_tick() should use non-willreturn attribute #2");
        assert!(output.contains("attributes #0"),
            "attributes #0 should still be present for terminating functions");
        assert!(output.contains("define void @init_state(%State* noalias nocapture %state) local_unnamed_addr #0"),
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
        out_pragmas: vec![],
        default_sig_modifier: None,
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
        assert!(output.contains("call void @init_state(%State* noalias nocapture %state)"),
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
        assert!(output.contains("load %State, %State* %state"),
            "All-convergent program should use struct-SSA main");
        assert!(!output.contains("@reactor_tick"),
            "All-convergent program should not emit reactor_tick");
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
        // With dead-field elimination, the float state x is never observed
        // (no exit condition references it, no other txn reads it).
        // The txn becomes effectively pure — only count = count + 1 survives.
        assert!(output.contains("store i64 50000000"),
            "Effectively-pure body should emit O(1) store i64 total, not a while-loop");
        assert!(output.contains("ret i32 0"),
            "Should return after store");
        // The while-loop body (process) is still emitted but main is O(1).
        // Verify main is the pure counter form by checking main is between
        // the store and the return.
        let main_idx = output.find("define i32 @main()").unwrap_or(0);
        let store_in_main = output[main_idx..].contains("store i64 50000000");
        assert!(store_in_main, "store must be in main, not in process");
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
                        Statement::Term { values: vec![], modifiers: vec![], swan_song: None },
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
                        Statement::Term { values: vec![], modifiers: vec![], swan_song: None },
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
        out_pragmas: vec![],
        default_sig_modifier: None,
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
                Statement::Term { values: vec![], modifiers: vec![], swan_song: None },
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
        out_pragmas: vec![],
        default_sig_modifier: None,
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
        // Non-foldable wake program without #!exit: no exit check, no natural death
        let program = make_wake_trg_program("io", "__io_pending", Type::Bool, true);
        let output = LlvmBackend::new().generate(&program);
        // Exit check pattern: `trunc` then `br i1 ..., label %done, ...`
        assert!(!output.contains("label %done"),
            "No branch-to-done without exit condition or natural death");
    }

    #[test]
    fn test_exit_in_enum_main() {
        // Bool trigger → enum dispatch path, no wake → one-shot.
        // Uniform-body detection skips the switch when all case arms are identical.
        let exit_cond = Expr::Eq(
            Box::new(Expr::Identifier("ops".to_string())),
            Box::new(Expr::Identifier("N".to_string())),
        );
        let program = make_exit_program(Some(exit_cond), Type::Bool, false);
        let output = LlvmBackend::new().with_optimize_budget(256).generate(&program);
        // One-shot enum dispatch: no tick loop, no exit check needed
        assert!(!output.contains("switch i64"),
            "Uniform enum bodies skip the switch dispatch");
        assert!(output.contains("ret i32 0"),
            "One-shot path returns 0 at each case arm");
        assert!(!output.contains("exit_check:"),
            "No exit check label in one-shot path (no tick loop)");
    }

    #[test]
    fn test_exit_in_enum_hybrid_wake() {
        // Bool trigger with is_wake → hybrid path (enum + wake).
        // Uniform-body detection skips the switch when all case arms are identical.
        let exit_cond = Expr::Eq(
            Box::new(Expr::Identifier("ops".to_string())),
            Box::new(Expr::Identifier("N".to_string())),
        );
        let program = make_exit_program(Some(exit_cond), Type::Bool, true);
        let output = LlvmBackend::new().with_optimize_budget(256).generate(&program);
        assert!(!output.contains("switch i64"),
            "Uniform enum bodies skip the switch dispatch");
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
        // Wake program without #!exit and without foldable txns should warn.
        // Non-foldable reactive txns cannot converge, so natural death won't help.
        let program = make_wake_trg_program("io", "__io_pending", Type::Bool, true);
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

    // ── Natural death tests ───────────────────────────────────

    #[test]
    fn test_natural_death_exits_foldable_program() {
        // Wake program with foldable txn but no #!exit → natural death emits exit check
        let program = make_exit_program(None, Type::Int, true);
        let mut backend = LlvmBackend::new();
        let _output = backend.generate(&program);
        // Natural death should have set has_natural_exit
        assert!(backend.has_natural_exit,
            "Foldable wake program should have natural exit");
        // Exit check should be emitted (trunc + branch to done)
        assert!(_output.contains("label %done"),
            "Natural death should emit exit check (branch to done)");
        // No warning about missing exit path — natural death handles it
        let has_warning = backend.warnings().iter().any(|w| {
            w.contains("has wake triggers but no exit path")
        });
        assert!(!has_warning,
            "No no-exit-path warning when natural death handles it");
    }

    #[test]
    fn test_natural_death_skipped_for_persistent_txn() {
        // Wake program with non-foldable txn → natural death should NOT apply
        let program = make_wake_trg_program("io", "__io_pending", Type::Bool, true);
        let mut backend = LlvmBackend::new();
        let _output = backend.generate(&program);
        assert!(!backend.has_natural_exit,
            "Program with persistent txn should NOT have natural exit");
        // Warning about missing exit path should fire
        let has_warning = backend.warnings().iter().any(|w| {
            w.contains("has wake triggers but no exit path")
        });
        assert!(has_warning,
            "Persistent wake program without #!exit should warn");
        // No exit check emitted
        assert!(!_output.contains("label %done"),
            "No exit check for persistent program");
    }

    #[test]
    fn test_natural_death_skipped_for_non_wake() {
        // Non-wake program with foldable txn → natural death not needed (one-shot)
        let program = make_exit_program(None, Type::Int, false);
        let mut backend = LlvmBackend::new();
        let _output = backend.generate(&program);
        assert!(!backend.has_natural_exit,
            "Non-wake program should NOT use natural death");
    }

    // ── SLP Hazard Detection Tests ────────────────────────────

    fn make_slp_float_program(n_floats: usize, cross_body: Vec<Statement>, precondition: Option<Expr>) -> Program {
        let mut items: Vec<TopLevel> = Vec::new();
        // Add n float fields: f0..f{n-1} = 0.0
        for i in 0..n_floats {
            items.push(TopLevel::StateDecl(StateDecl {
                name: format!("f{}", i),
                ty: Type::Float,
                expr: Some(Expr::Float(0.0)),
                address: None,
                bit_range: None,
                is_override: false,
                os_mode: false,
                span: None,
                attrs: vec![],
            }));
        }
        // Add counter field so bounded_pre can work
        items.push(TopLevel::StateDecl(StateDecl {
            name: "count".to_string(),
            ty: Type::Int,
            expr: Some(Expr::Integer(0)),
            address: None,
            bit_range: None,
            is_override: false,
            os_mode: false,
            span: None,
            attrs: vec![],
        }));
        items.push(TopLevel::StateDecl(StateDecl {
            name: "total".to_string(),
            ty: Type::Int,
            expr: Some(Expr::Integer(100)),
            address: None,
            bit_range: None,
            is_override: false,
            os_mode: false,
            span: None,
            attrs: vec![],
        }));
        items.push(TopLevel::Transaction(Transaction {
            name: "tick".to_string(),
            is_async: false,
            is_reactive: true,
            parameters: vec![],
            contract: Contract {
                pre_condition: precondition.unwrap_or(Expr::Bool(true)),
                post_condition: Expr::Identifier("count".to_string()),
                watchdog: None,
                span: None,
            },
            body: cross_body,
            reactor_speed: None,
            span: None,
            is_lambda: false,
            dependencies: vec![],
            attrs: vec![],
            modifiers: vec![],
            variant_bodies: vec![],
        }));
        Program {
            items,
            comments: vec![],
            reactor_speed: None,
            attrs: vec![],
            ffi: None,
            strict_mode: StrictMode::Off,
            dispatch_mode: DispatchMode::Sequential,
            exit_condition: None,
        out_pragmas: vec![],
        default_sig_modifier: None,
        }
    }

    fn make_cross_float_body(n_floats: usize, cross_count: usize) -> Vec<Statement> {
        let mut stmts: Vec<Statement> = Vec::new();
        // Assignment: f0 = f1 * f2; f1 = f2 * f3; etc.
        for i in 0..cross_count {
            let a = (i * 3) % n_floats;
            let b = ((i * 3) + 1) % n_floats;
            let c = ((i * 3) + 2) % n_floats;
            stmts.push(Statement::Assignment {
                lhs: Expr::Identifier(format!("f{}", a)),
                expr: Expr::Mul(
                    Box::new(Expr::Identifier(format!("f{}", b))),
                    Box::new(Expr::Identifier(format!("f{}", c))),
                ),
                timeout: None,
                modifiers: vec![],
            });
        }
        // Increment counter so bounded_pre can fire
        stmts.push(Statement::Assignment {
            lhs: Expr::Identifier("count".to_string()),
            expr: Expr::Add(
                Box::new(Expr::Identifier("count".to_string())),
                Box::new(Expr::Integer(1)),
            ),
            timeout: None,
            modifiers: vec![],
        });
        stmts
    }

    #[test]
    fn test_slp_hazard_no_floats() {
        // No float fields → no SLP hazard
        let program = make_slp_float_program(0, make_cross_float_body(0, 0), None);
        let mut backend = LlvmBackend::new();
        let output = backend.generate(&program);
        assert!(!output.contains("disable-slp-vectorize"),
            "No float fields should produce no SLP-disabled attributes");
    }

    #[test]
    fn test_slp_hazard_small_field_count() {
        // 4 float fields, 6 float ops → 6/4=1.5 ops/field ≥ threshold, SLP is safe
        let body = make_cross_float_body(4, 6);
        let program = make_slp_float_program(4, body, None);
        let mut backend = LlvmBackend::new();
        let output = backend.generate(&program);
        assert!(!output.contains("disable-slp-vectorize"),
            "4 float fields with 6 ops should not trigger SLP disable");
    }

    #[test]
    fn test_slp_hazard_large_field_count() {
        // 20 float fields + many cross-ops → SLP hazard on SSE (peak ≥ 16)
        // Formula: ceil(20/4)=5 packed, min(10,20)=10 shuffles, 0 temps, 0 consts, +2 = 17
        let body = make_cross_float_body(20, 40);
        let program = make_slp_float_program(20, body, None);
        let mut backend = LlvmBackend::new();
        let output = backend.generate(&program);
        assert!(output.contains("disable-slp-vectorize"),
            "20 float fields with cross-ops should disable SLP on SSE");
    }

    #[test]
    fn test_slp_hazard_independent_channels() {
        // 12 float fields with ZERO cross-ops → no shuffles needed, SLP is safe
        let mut body: Vec<Statement> = Vec::new();
        for i in 0..12 {
            body.push(Statement::Assignment {
                lhs: Expr::Identifier(format!("f{}", i)),
                expr: Expr::Add(
                    Box::new(Expr::Identifier(format!("f{}", i))),
                    Box::new(Expr::Float(1.0)),
                ),
                timeout: None,
                modifiers: vec![],
            });
        }
        body.push(Statement::Assignment {
            lhs: Expr::Identifier("count".to_string()),
            expr: Expr::Add(
                Box::new(Expr::Identifier("count".to_string())),
                Box::new(Expr::Integer(1)),
            ),
            timeout: None,
            modifiers: vec![],
        });
        let program = make_slp_float_program(12, body, None);
        let mut backend = LlvmBackend::new();
        let output = backend.generate(&program);
        // Independent channels: packed_phis=3, shuffle_regs=0, temps=0, margin=2 → peak=5 < 16
        assert!(!output.contains("disable-slp-vectorize"),
            "12 independent float fields should NOT disable SLP");
    }

    #[test]
    fn test_slp_hazard_with_target_spec() {
        // AArch64 (R=32, W=4), 12 fields, 18 float ops → 18/12=1.5 ops/field, SLP safe
        let body = make_cross_float_body(12, 18);
        let program = make_slp_float_program(12, body, None);
        let mut backend = LlvmBackend::new();
        let spec = crate::target_spec::TargetSpec {
            target: Some(crate::target_spec::TargetSection {
                name: "aarch64-unknown-linux-gnu".to_string(),
                backend: "llvm".to_string(),
                capabilities: vec!["neon".to_string()],
                import_ffi: None,
            }),
            ffi: None,
            codegen: None,
            memory: None,
            bottlenecks: None,
        };
        backend = backend.with_spec(spec);
        let output = backend.generate(&program);
        assert!(!output.contains("disable-slp-vectorize"),
            "AArch64 with 32 registers and ASR 2.4 > 1.5 should allow SLP for 12 fields");
    }

    #[test]
    fn test_slp_hazard_avx_target() {
        // With AVX2 (R=16, W=8) → 12 fields, 32 cross-ops
        // shuffle_pressure=min(32,24)=24, peak=2+24+0+0+2=28 >= 16 → SLP disabled
        let body = make_cross_float_body(12, 32);
        let program = make_slp_float_program(12, body, None);
        let mut backend = LlvmBackend::new();
let spec = crate::target_spec::TargetSpec {
                target: Some(crate::target_spec::TargetSection {
                    name: "x86_64-unknown-linux-gnu".to_string(),
                    backend: "llvm".to_string(),
                    capabilities: vec!["avx2".to_string()],
                    import_ffi: None,
                }),
                ffi: None,
                codegen: None,
                memory: None,
                bottlenecks: None,
        };
        backend = backend.with_spec(spec);
        let output = backend.generate(&program);
        // 32 cross-ops on 12 fields → peak 28 ≥ 16 → spills on AVX2 → disable
        assert!(output.contains("disable-slp-vectorize"),
            "AVX2: 12 fields with 32 cross-ops should disable SLP (peak=28 ≥ 16)");
    }

    #[test]
    fn test_dbvs_import_aliases_loaded() {
        let mut aliases: HashMap<String, crate::dbrief::DbriefType> = HashMap::new();
        aliases.insert("uart_debug".to_string(), crate::dbrief::DbriefType::Data);
        let mut backend = LlvmBackend::new().with_schema_aliases(aliases);
        assert_eq!(backend.schema_aliases.len(), 1);
        assert!(backend.schema_aliases.contains_key("uart_debug"));
        let output = backend.generate(&empty_program());
        assert!(output.contains("ModuleID"));
    }

    #[test]
    fn test_schema_type_unsigned_warning() {
        let mut aliases: HashMap<String, crate::dbrief::DbriefType> = HashMap::new();
        aliases.insert("count".to_string(), crate::dbrief::DbriefType::UInt(64));
        let mut backend = LlvmBackend::new().with_schema_aliases(aliases);
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
                    attrs: Vec::new(),
                }),
            ],
            ..empty_program()
        };
        let _output = backend.generate(&program);
        let warnings = backend.warnings();
        let has_unsigned_warning = warnings.iter().any(|w| w.contains("unsigned") && w.contains("count"));
        assert!(has_unsigned_warning,
            "UInt(64) schema type with Int Brief type should produce unsigned warning, got: {:?}", warnings);
    }

    #[test]
    fn test_schema_vector_rejected() {
        let mut aliases: HashMap<String, crate::dbrief::DbriefType> = HashMap::new();
        aliases.insert("buf".to_string(), crate::dbrief::DbriefType::Vector(
            Box::new(crate::dbrief::DbriefType::UInt(8)), Some(256)));
        let mut backend = LlvmBackend::new().with_schema_aliases(aliases);
        let program = Program {
            items: vec![
                TopLevel::StateDecl(StateDecl {
                    name: "buf".to_string(),
                    ty: Type::Int,
                    expr: Some(Expr::Integer(0)),
                    address: None,
                    bit_range: None,
                    is_override: false,
                    os_mode: false,
                    span: None,
                    attrs: Vec::new(),
                }),
            ],
            ..empty_program()
        };
        let _output = backend.generate(&program);
        let warnings = backend.warnings();
        let has_vector_warning = warnings.iter().any(|w| w.contains("Vector") && w.contains("buf"));
        assert!(has_vector_warning,
            "Vector schema type should produce incompatibility warning, got: {:?}", warnings);
    }

    #[test]
    fn test_no_schema_import_no_validation() {
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
                    attrs: Vec::new(),
                }),
            ],
            ..empty_program()
        };
        let _output = backend.generate(&program);
        assert!(backend.warnings().is_empty(),
            "No schema import should produce no warnings");
    }

    #[test]
    fn test_multiple_schema_imports_merged() {
        let mut aliases: HashMap<String, crate::dbrief::DbriefType> = HashMap::new();
        aliases.insert("gpio0".to_string(), crate::dbrief::DbriefType::UInt(32));
        aliases.insert("gpio1".to_string(), crate::dbrief::DbriefType::UInt(32));
        let mut backend = LlvmBackend::new().with_schema_aliases(aliases);
        assert_eq!(backend.schema_aliases.len(), 2);
        let output = backend.generate(&empty_program());
        assert!(output.contains("ModuleID"));
    }

    #[test]
    fn test_imported_alias_is_mmio() {
        let mut aliases: HashMap<String, crate::dbrief::DbriefType> = HashMap::new();
        aliases.insert("led_0".to_string(), crate::dbrief::DbriefType::UInt(32));
        let mut mmio: HashMap<String, u64> = HashMap::new();
        mmio.insert("led_0".to_string(), 0x40000000);
        let mut backend = LlvmBackend::new()
            .with_schema_aliases(aliases)
            .with_mmio_addresses(mmio);
        let program = Program {
            items: vec![
                TopLevel::StateDecl(StateDecl {
                    name: "led_0".to_string(),
                    ty: Type::Int,
                    expr: Some(Expr::Integer(0)),
                    address: None,
                    bit_range: None,
                    is_override: false,
                    os_mode: false,
                    span: None,
                    attrs: Vec::new(),
                }),
            ],
            ..empty_program()
        };
        let output = backend.generate(&program);
        assert!(output.contains("inttoptr i64 1073741824"),
            "led_0 with schema import should be MMIO (inttoptr). Got: {}", output);
        assert!(output.contains("store volatile i64"),
            "led_0 with schema import should use volatile store. Got: {}", output);
    }

    #[test]
    fn test_unimported_alias_not_mmio() {
        let mut aliases: HashMap<String, crate::dbrief::DbriefType> = HashMap::new();
        aliases.insert("uart_debug".to_string(), crate::dbrief::DbriefType::Data);
        let mut mmio: HashMap<String, u64> = HashMap::new();
        mmio.insert("led_0".to_string(), 0x40000000);
        mmio.insert("uart_debug".to_string(), 0xFF010000);
        let mut backend = LlvmBackend::new()
            .with_schema_aliases(aliases)
            .with_mmio_addresses(mmio);
        let program = Program {
            items: vec![
                TopLevel::StateDecl(StateDecl {
                    name: "led_0".to_string(),
                    ty: Type::Int,
                    expr: Some(Expr::Integer(0)),
                    address: None,
                    bit_range: None,
                    is_override: false,
                    os_mode: false,
                    span: None,
                    attrs: Vec::new(),
                }),
            ],
            ..empty_program()
        };
        let output = backend.generate(&program);
        assert!(!output.contains("inttoptr i64 1073741824"),
            "led_0 NOT in schema should NOT be MMIO (no inttoptr for 0x40000000). Got: {}", output);
        assert!(output.contains("getelementptr inbounds %State"),
            "led_0 NOT in schema should use struct GEP. Got: {}", output);
    }

    // ── Struct codegen tests ───────────────────────────────────

    #[test]
    fn test_struct_type_registered() {
        let mut backend = LlvmBackend::new();
        let program = Program {
            items: vec![
                TopLevel::Struct(StructDefinition {
                    name: "Point".to_string(),
                    type_params: vec![],
                    fields: vec![
                        StructField { name: "x".to_string(), ty: Type::Int, default: None },
                        StructField { name: "y".to_string(), ty: Type::Int, default: None },
                    ],
                    transactions: vec![],
                    view_html: None,
                    span: None,
                    modifiers: vec![],
                    variants: vec![],
                }),
            ],
            ..empty_program()
        };
        let output = backend.generate(&program);
        assert!(output.contains("ModuleID"), "Output should be valid IR");
        assert!(backend.struct_types.contains_key("Point"),
            "Struct 'Point' should be registered");
        assert_eq!(backend.struct_types["Point"].len(), 2);
    }

    fn make_point_program(body: Vec<Statement>) -> Program {
        Program {
            items: vec![
                TopLevel::Struct(StructDefinition {
                    name: "Point".to_string(),
                    type_params: vec![],
                    fields: vec![
                        StructField { name: "x".to_string(), ty: Type::Int, default: None },
                        StructField { name: "y".to_string(), ty: Type::Int, default: None },
                    ],
                    transactions: vec![],
                    view_html: None,
                    span: None,
                    modifiers: vec![],
                    variants: vec![],
                }),
                TopLevel::StateDecl(StateDecl {
                    name: "pt".to_string(),
                    ty: Type::Int,
                    expr: Some(Expr::Integer(0)),
                    address: None, bit_range: None, is_override: false,
                    os_mode: false, span: None, attrs: vec![],
                }),
                TopLevel::Transaction(Transaction {
                    name: "main".to_string(),
                    is_reactive: false,
                    parameters: vec![],
                    contract: Contract {
                        pre_condition: Expr::Bool(true),
                        post_condition: Expr::Bool(true),
                        watchdog: None, span: None,
                    },
                    body,
                    reactor_speed: None, span: None, is_lambda: false,
                    dependencies: vec![],
                    is_async: false,
                    attrs: vec![], modifiers: vec![], variant_bodies: vec![],
                }),
            ],
            ..empty_program()
        }
    }

    #[test]
    fn test_struct_instance_emits_alloca_store_ptrtoint() {
        let mut backend = LlvmBackend::new();
        let body = vec![
            Statement::Let {
                name: "p".to_string(),
                ty: Some(Type::Custom("Point".to_string())),
                expr: Some(Expr::StructInstance("Point".to_string(), vec![
                    ("x".to_string(), Expr::Integer(10)),
                    ("y".to_string(), Expr::Integer(20)),
                ])),
                address: None, address_expr: None, bit_range: None,
                is_override: false, modifiers: vec![],
            },
        ];
        let output = backend.generate(&make_point_program(body));
        assert!(output.contains("alloca i64, i64 2"),
            "StructInstance should alloca for 2 fields. Got: {}", output);
        assert!(output.contains("add i64 0, 10"),
            "StructInstance should load field value 10. Got: {}", output);
        assert!(output.contains("add i64 0, 20"),
            "StructInstance should load field value 20. Got: {}", output);
        assert!(output.contains("ptrtoint i64*"),
            "StructInstance should return ptrtoint. Got: {}", output);
    }

    #[test]
    fn test_field_access_resolves_correct_offset() {
        let mut backend = LlvmBackend::new();
        let body = vec![
            Statement::Let {
                name: "p".to_string(),
                ty: Some(Type::Custom("Point".to_string())),
                expr: Some(Expr::StructInstance("Point".to_string(), vec![
                    ("x".to_string(), Expr::Integer(10)),
                    ("y".to_string(), Expr::Integer(20)),
                ])),
                address: None, address_expr: None, bit_range: None,
                is_override: false, modifiers: vec![],
            },
            Statement::Assignment {
                lhs: Expr::Identifier("pt".to_string()),
                expr: Expr::FieldAccess(
                    Box::new(Expr::Identifier("p".to_string())),
                    "y".to_string(),
                ),
                timeout: None, modifiers: vec![],
            },
        ];
        let output = backend.generate(&make_point_program(body));
        assert!(output.contains("getelementptr i64, i64*"),
            "FieldAccess should emit GEP. Got: {}", output);
    }

    #[test]
    fn test_field_access_unknown_struct_falls_back() {
        let mut backend = LlvmBackend::new();
        fn empty_contract() -> Contract {
            Contract { pre_condition: Expr::Bool(true), post_condition: Expr::Bool(true), watchdog: None, span: None }
        }
        let program = Program {
            items: vec![
                TopLevel::StateDecl(StateDecl {
                    name: "raw".to_string(),
                    ty: Type::Int,
                    expr: Some(Expr::Integer(0)),
                    address: None, bit_range: None, is_override: false,
                    os_mode: false, span: None, attrs: vec![],
                }),
                TopLevel::Transaction(Transaction {
                    name: "bad".to_string(),
                    is_reactive: false, parameters: vec![],
                    contract: empty_contract(),
                    body: vec![
                        Statement::Assignment {
                            lhs: Expr::Identifier("raw".to_string()),
                            expr: Expr::FieldAccess(
                                Box::new(Expr::Identifier("raw".to_string())),
                                "nonexistent".to_string(),
                            ),
                            timeout: None, modifiers: vec![],
                        },
                    ],
                    reactor_speed: None, span: None, is_lambda: false,
                    dependencies: vec![],
                    is_async: false,
                    attrs: vec![], modifiers: vec![], variant_bodies: vec![],
                }),
            ],
            ..empty_program()
        };
        let output = backend.generate(&program);
        assert!(output.contains("add i64 0, 0 ; field"),
            "Unknown struct FieldAccess should emit fallback. Got: {}", output);
    }

    #[test]
    fn test_object_literal_emits_alloca_store_ptrtoint() {
        let mut backend = LlvmBackend::new();
        fn empty_contract() -> Contract {
            Contract { pre_condition: Expr::Bool(true), post_condition: Expr::Bool(true), watchdog: None, span: None }
        }
        let program = Program {
            items: vec![
                TopLevel::StateDecl(StateDecl {
                    name: "obj".to_string(),
                    ty: Type::Int,
                    expr: Some(Expr::Integer(0)),
                    address: None, bit_range: None, is_override: false,
                    os_mode: false, span: None, attrs: vec![],
                }),
                TopLevel::Transaction(Transaction {
                    name: "make_obj".to_string(),
                    is_reactive: false, parameters: vec![],
                    contract: empty_contract(),
                    body: vec![
                        Statement::Let {
                            name: "o".to_string(),
                            ty: None,
                            expr: Some(Expr::ObjectLiteral(vec![
                                ("name".to_string(), Expr::String("test".to_string())),
                                ("value".to_string(), Expr::Integer(42)),
                            ])),
                            address: None, address_expr: None, bit_range: None,
                            is_override: false, modifiers: vec![],
                        },
                    ],
                    reactor_speed: None, span: None, is_lambda: false,
                    dependencies: vec![],
                    is_async: false,
                    attrs: vec![], modifiers: vec![], variant_bodies: vec![],
                }),
            ],
            ..empty_program()
        };
        let output = backend.generate(&program);
        assert!(output.contains("alloca i64, i64 2"),
            "ObjectLiteral should alloca for fields. Got: {}", output);
        assert!(output.contains("ptrtoint i64*"),
            "ObjectLiteral should return ptrtoint. Got: {}", output);
    }

    // ── Enum codegen tests ────────────────────────────────────

    #[test]
    fn test_enum_type_registered_and_variant_disc() {
        let mut backend = LlvmBackend::new();
        let program = Program {
            items: vec![
                TopLevel::Enum(EnumDefinition {
                    name: "Option".to_string(),
                    type_params: vec![],
                    variants: vec![
                        EnumVariant::Unit("None".to_string()),
                        EnumVariant::Tuple("Some".to_string(), vec![Type::Int]),
                    ],
                    span: None,
                }),
            ],
            ..empty_program()
        };
        let _ = backend.generate(&program);
        assert!(backend.enum_types.contains_key("Option"));
        assert!(backend.variant_disc.contains_key("None"));
        assert!(backend.variant_disc.contains_key("Some"));
        assert_eq!(backend.variant_disc.get("None").map(|(_, d, _)| *d), Some(0));
        assert_eq!(backend.variant_disc.get("Some").map(|(_, d, _)| *d), Some(1));
        assert_eq!(backend.variant_disc.get("Some").map(|(_, _, f)| *f), Some(1));
    }

    #[test]
    fn test_enum_constructor_uses_registered_discriminant() {
        let mut backend = LlvmBackend::new();
        let program = Program {
            items: vec![
                TopLevel::Enum(EnumDefinition {
                    name: "Result".to_string(),
                    type_params: vec![],
                    variants: vec![
                        EnumVariant::Unit("Err".to_string()),
                        EnumVariant::Tuple("Ok".to_string(), vec![Type::Int]),
                    ],
                    span: None,
                }),
                TopLevel::StateDecl(StateDecl {
                    name: "r".to_string(), ty: Type::Int,
                    expr: Some(Expr::Integer(0)),
                    address: None, bit_range: None, is_override: false,
                    os_mode: false, span: None, attrs: vec![],
                }),
                TopLevel::Transaction(Transaction {
                    name: "wrap".to_string(), is_reactive: false,
                    parameters: vec![],
                    contract: Contract {
                        pre_condition: Expr::Bool(true),
                        post_condition: Expr::Bool(true),
                        watchdog: None, span: None,
                    },
                    body: vec![
                        Statement::Let {
                            name: "x".to_string(), ty: None,
                            expr: Some(Expr::Call("Ok".to_string(), vec![Expr::Integer(42)])),
                            address: None, address_expr: None, bit_range: None,
                            is_override: false, modifiers: vec![],
                        },
                    ],
                    reactor_speed: None, span: None, is_lambda: false,
                    dependencies: vec![],
                    is_async: false,
                    attrs: vec![], modifiers: vec![], variant_bodies: vec![],
                }),
            ],
            ..empty_program()
        };
        let output = backend.generate(&program);
        assert!(output.contains("store i64 1"), "Ok should have disc 1. Got: {}", output);
        assert!(output.contains("store i64 %t"), "Ok should store payload register. Got: {}", output);
    }

    #[test]
    fn test_pattern_match_uses_registered_discriminant() {
        let mut backend = LlvmBackend::new();
        let program = Program {
            items: vec![
                TopLevel::Enum(EnumDefinition {
                    name: "Status".to_string(),
                    type_params: vec![],
                    variants: vec![
                        EnumVariant::Unit("Off".to_string()),
                        EnumVariant::Unit("On".to_string()),
                        EnumVariant::Unit("Error".to_string()),
                    ],
                    span: None,
                }),
                TopLevel::StateDecl(StateDecl {
                    name: "check".to_string(), ty: Type::Int,
                    expr: Some(Expr::Integer(0)),
                    address: None, bit_range: None, is_override: false,
                    os_mode: false, span: None, attrs: vec![],
                }),
                TopLevel::Transaction(Transaction {
                    name: "test".to_string(), is_reactive: false,
                    parameters: vec![],
                    contract: Contract {
                        pre_condition: Expr::Bool(true),
                        post_condition: Expr::Bool(true),
                        watchdog: None, span: None,
                    },
                    body: vec![
                        Statement::Let {
                            name: "s".to_string(), ty: None,
                            expr: Some(Expr::Call("Error".to_string(), vec![])),
                            address: None, address_expr: None, bit_range: None,
                            is_override: false, modifiers: vec![],
                        },
                        Statement::Let {
                            name: "matched".to_string(), ty: None,
                            expr: Some(Expr::PatternMatch {
                                value: Box::new(Expr::Identifier("s".to_string())),
                                variant: "Error".to_string(),
                                fields: vec![],
                            }),
                            address: None, address_expr: None, bit_range: None,
                            is_override: false, modifiers: vec![],
                        },
                    ],
                    reactor_speed: None, span: None, is_lambda: false,
                    dependencies: vec![],
                    is_async: false,
                    attrs: vec![], modifiers: vec![], variant_bodies: vec![],
                }),
            ],
            ..empty_program()
        };
        let output = backend.generate(&program);
        assert!(output.contains("icmp eq i64"), "PatternMatch should compare discriminant. Got: {}", output);
    }

    #[test]
    fn test_match_arm_field_binding() {
        let mut backend = LlvmBackend::new();
        let program = Program {
            items: vec![
                TopLevel::Enum(EnumDefinition {
                    name: "Option".to_string(),
                    type_params: vec![],
                    variants: vec![
                        EnumVariant::Unit("None".to_string()),
                        EnumVariant::Tuple("Some".to_string(), vec![Type::Int]),
                    ],
                    span: None,
                }),
                TopLevel::StateDecl(StateDecl {
                    name: "inner".to_string(), ty: Type::Int,
                    expr: Some(Expr::Integer(0)),
                    address: None, bit_range: None, is_override: false,
                    os_mode: false, span: None, attrs: vec![],
                }),
                TopLevel::Transaction(Transaction {
                    name: "unwrap".to_string(), is_reactive: false,
                    parameters: vec![],
                    contract: Contract {
                        pre_condition: Expr::Bool(true),
                        post_condition: Expr::Bool(true),
                        watchdog: None, span: None,
                    },
                    body: vec![
                        Statement::Assignment {
                            lhs: Expr::Identifier("inner".to_string()),
                            expr: Expr::Match {
                                value: Box::new(Expr::Call("Some".to_string(), vec![Expr::Integer(7)])),
                                arms: vec![
                                    MatchArm {
                                        pattern: MatchPattern::Variant {
                                            name: "Some".to_string(),
                                            fields: vec!["val".to_string()],
                                        },
                                        guard: None,
                                        body: Box::new(Expr::Identifier("val".to_string())),
                                    },
                                    MatchArm {
                                        pattern: MatchPattern::Wildcard,
                                        guard: None,
                                        body: Box::new(Expr::Integer(-1)),
                                    },
                                ],
                            },
                            timeout: None, modifiers: vec![],
                        },
                    ],
                    reactor_speed: None, span: None, is_lambda: false,
                    dependencies: vec![],
                    is_async: false,
                    attrs: vec![], modifiers: vec![], variant_bodies: vec![],
                }),
            ],
            ..empty_program()
        };
        let output = backend.generate(&program);
        assert!(output.contains("switch i64"), "Match should emit switch. Got: {}", output);
        assert!(output.contains("getelementptr i64, i64*"), "Field binding should GEP. Got: {}", output);
    }

    #[test]
    fn test_enum_multi_variant_discriminants() {
        let mut backend = LlvmBackend::new();
        let program = Program {
            items: vec![
                TopLevel::Enum(EnumDefinition {
                    name: "Tree".to_string(),
                    type_params: vec![],
                    variants: vec![
                        EnumVariant::Unit("Leaf".to_string()),
                        EnumVariant::Tuple("Node".to_string(), vec![Type::Int, Type::Int]),
                    ],
                    span: None,
                }),
            ],
            ..empty_program()
        };
        let _ = backend.generate(&program);
        assert_eq!(backend.variant_disc.get("Leaf").map(|(_, d, _)| *d), Some(1));
        assert_eq!(backend.variant_disc.get("Node").map(|(_, d, _)| *d), Some(2));
        assert_eq!(backend.variant_disc.get("Node").map(|(_, _, f)| *f), Some(2));
    }

    // ── Collection (list) tests ────────────────────────────────────

    #[test]
    fn test_list_literal_2slot_header() {
        let mut backend = LlvmBackend::new();
        let program = Program {
            items: vec![
                TopLevel::StateDecl(StateDecl {
                    name: "lst".to_string(), ty: Type::Int, expr: Some(Expr::Integer(0)),
                    address: None, bit_range: None, is_override: false,
                    os_mode: false, span: None, attrs: vec![],
                }),
                TopLevel::Transaction(Transaction {
                    name: "mklist".to_string(), is_reactive: false, parameters: vec![],
                    contract: Contract { pre_condition: Expr::Bool(true), post_condition: Expr::Bool(true), watchdog: None, span: None },
                    body: vec![
                        Statement::Assignment {
                            lhs: Expr::Identifier("lst".to_string()),
                            expr: Expr::ListLiteral(vec![Expr::Integer(10), Expr::Integer(20)]),
                            timeout: None, modifiers: vec![],
                        },
                    ],
                    reactor_speed: None, span: None, is_lambda: false,
                    dependencies: vec![], is_async: false,
                    attrs: vec![], modifiers: vec![], variant_bodies: vec![],
                }),
            ],
            ..empty_program()
        };
        let output = backend.generate(&program);
        // 2-slot header means 4 slots: [data_ptr, len, elem0, elem1]
        assert!(output.contains("alloca i64, i64 4"), "2-elem list = 4 slots. Got: {}", output);
        assert!(output.contains("store i64 2, i64*"), "Length should be 2. Got: {}", output);
        assert!(output.contains("ptrtoint i64*"), "Should emit ptrtoint for data_ptr. Got: {}", output);
    }

    #[test]
    fn test_list_index_uses_2slot_header() {
        let mut backend = LlvmBackend::new();
        let program = Program {
            items: vec![
                TopLevel::StateDecl(StateDecl {
                    name: "elem".to_string(), ty: Type::Int, expr: Some(Expr::Integer(0)),
                    address: None, bit_range: None, is_override: false,
                    os_mode: false, span: None, attrs: vec![],
                }),
                TopLevel::Transaction(Transaction {
                    name: "idx".to_string(), is_reactive: false, parameters: vec![],
                    contract: Contract { pre_condition: Expr::Bool(true), post_condition: Expr::Bool(true), watchdog: None, span: None },
                    body: vec![
                        Statement::Assignment {
                            lhs: Expr::Identifier("elem".to_string()),
                            expr: Expr::ListIndex(
                                Box::new(Expr::ListLiteral(vec![Expr::Integer(99)])),
                                Box::new(Expr::Integer(0)),
                            ),
                            timeout: None, modifiers: vec![],
                        },
                    ],
                    reactor_speed: None, span: None, is_lambda: false,
                    dependencies: vec![], is_async: false,
                    attrs: vec![], modifiers: vec![], variant_bodies: vec![],
                }),
            ],
            ..empty_program()
        };
        let output = backend.generate(&program);
        // ListIndex must load data_ptr from slot 0 before GEP
        assert!(output.contains("load i64, i64*"), "Should load data_ptr. Got: {}", output);
        assert!(output.contains("getelementptr i64, i64*"), "Should GEP from data. Got: {}", output);
    }

    #[test]
    fn test_list_len_loads_length() {
        let mut backend = LlvmBackend::new();
        let program = Program {
            items: vec![
                TopLevel::StateDecl(StateDecl {
                    name: "len".to_string(), ty: Type::Int, expr: Some(Expr::Integer(0)),
                    address: None, bit_range: None, is_override: false,
                    os_mode: false, span: None, attrs: vec![],
                }),
                TopLevel::Transaction(Transaction {
                    name: "chk_len".to_string(), is_reactive: false, parameters: vec![],
                    contract: Contract { pre_condition: Expr::Bool(true), post_condition: Expr::Bool(true), watchdog: None, span: None },
                    body: vec![
                        Statement::Assignment {
                            lhs: Expr::Identifier("len".to_string()),
                            expr: Expr::Projection { source: Box::new(Expr::ListLiteral(vec![Expr::Integer(1), Expr::Integer(2)])), target: ProjectionTarget::Size },
                            timeout: None, modifiers: vec![],
                        },
                    ],
                    reactor_speed: None, span: None, is_lambda: false,
                    dependencies: vec![], is_async: false,
                    attrs: vec![], modifiers: vec![], variant_bodies: vec![],
                }),
            ],
            ..empty_program()
        };
        let output = backend.generate(&program);
        // Size projection must load length from slot 1, NOT return constant 0
        assert!(output.contains("load i64, i64*"), "Size projection should load from memory. Got: {}", output);
    }

    #[test]
    fn test_slice_emits_copy_loop() {
        let mut backend = LlvmBackend::new();
        let program = Program {
            items: vec![
                TopLevel::StateDecl(StateDecl {
                    name: "sliced".to_string(), ty: Type::Int, expr: Some(Expr::Integer(0)),
                    address: None, bit_range: None, is_override: false,
                    os_mode: false, span: None, attrs: vec![],
                }),
                TopLevel::Transaction(Transaction {
                    name: "slice_op".to_string(), is_reactive: false, parameters: vec![],
                    contract: Contract { pre_condition: Expr::Bool(true), post_condition: Expr::Bool(true), watchdog: None, span: None },
                    body: vec![
                        Statement::Assignment {
                            lhs: Expr::Identifier("sliced".to_string()),
                            expr: Expr::Slice {
                                value: Box::new(Expr::ListLiteral(vec![Expr::Integer(10), Expr::Integer(20), Expr::Integer(30)])),
                                start: Some(Box::new(Expr::Integer(1))),
                                end: Some(Box::new(Expr::Integer(3))),
                                stride: None,
                                mask: None,
                            },
                            timeout: None, modifiers: vec![],
                        },
                    ],
                    reactor_speed: None, span: None, is_lambda: false,
                    dependencies: vec![], is_async: false,
                    attrs: vec![], modifiers: vec![], variant_bodies: vec![],
                }),
            ],
            ..empty_program()
        };
        let output = backend.generate(&program);
        // Slice should emit a counted loop (phi + icmp + br)
        assert!(output.contains("phi i64"), "Slice should emit a phi. Got: {}", output);
        assert!(output.contains("icmp slt"), "Slice should have loop condition. Got: {}", output);
    }

    #[test]
    fn test_multislice_index_delegates() {
        let mut backend = LlvmBackend::new();
        let mkv: Vec<Expr> = (0..5).map(|i| Expr::Integer(i)).collect();
        let program = Program {
            items: vec![
                TopLevel::StateDecl(StateDecl {
                    name: "v".to_string(), ty: Type::Int, expr: Some(Expr::Integer(0)),
                    address: None, bit_range: None, is_override: false,
                    os_mode: false, span: None, attrs: vec![],
                }),
                TopLevel::Transaction(Transaction {
                    name: "m".to_string(), is_reactive: false, parameters: vec![],
                    contract: Contract { pre_condition: Expr::Bool(true), post_condition: Expr::Bool(true), watchdog: None, span: None },
                    body: vec![
                        Statement::Assignment {
                            lhs: Expr::Identifier("v".to_string()),
                            expr: Expr::MultiSlice {
                                value: Box::new(Expr::ListLiteral(mkv)),
                                coordinates: vec![SliceCoordinate::Index(Box::new(Expr::Integer(2)))],
                                mask: None,
                            },
                            timeout: None, modifiers: vec![],
                        },
                    ],
                    reactor_speed: None, span: None, is_lambda: false,
                    dependencies: vec![], is_async: false,
                    attrs: vec![], modifiers: vec![], variant_bodies: vec![],
                }),
            ],
            ..empty_program()
        };
        let output = backend.generate(&program);
        // MultiSlice with single Index should load data_ptr and GEP
        assert!(output.contains("getelementptr i64, i64*"), "Should GEP. Got: {}", output);
    }

    // ── Tuple tests ────────────────────────────────────────────

    #[test]
    fn test_tuple_emits_2slot_header() {
        let mut backend = LlvmBackend::new();
        let program = Program {
            items: vec![
                TopLevel::StateDecl(StateDecl {
                    name: "t".to_string(), ty: Type::Int, expr: Some(Expr::Integer(0)),
                    address: None, bit_range: None, is_override: false,
                    os_mode: false, span: None, attrs: vec![],
                }),
                TopLevel::Transaction(Transaction {
                    name: "mktup".to_string(), is_reactive: false, parameters: vec![],
                    contract: Contract { pre_condition: Expr::Bool(true), post_condition: Expr::Bool(true), watchdog: None, span: None },
                    body: vec![
                        Statement::Assignment {
                            lhs: Expr::Identifier("t".to_string()),
                            expr: Expr::Tuple(vec![Expr::Integer(1), Expr::Integer(2), Expr::Integer(3)]),
                            timeout: None, modifiers: vec![],
                        },
                    ],
                    reactor_speed: None, span: None, is_lambda: false,
                    dependencies: vec![], is_async: false,
                    attrs: vec![], modifiers: vec![], variant_bodies: vec![],
                }),
            ],
            ..empty_program()
        };
        let output = backend.generate(&program);
        assert!(output.contains("alloca i64, i64 5"), "3-elem tuple = 5 slots. Got: {}", output);
        assert!(output.contains("store i64 3, i64*"), "Length should be 3. Got: {}", output);
    }

    #[test]
    fn test_tuple_destructure_binds_variables() {
        let mut backend = LlvmBackend::new();
        let program = Program {
            items: vec![
                TopLevel::StateDecl(StateDecl {
                    name: "val".to_string(), ty: Type::Int, expr: Some(Expr::Integer(0)),
                    address: None, bit_range: None, is_override: false,
                    os_mode: false, span: None, attrs: vec![],
                }),
                TopLevel::Transaction(Transaction {
                    name: "destr".to_string(), is_reactive: false, parameters: vec![],
                    contract: Contract { pre_condition: Expr::Bool(true), post_condition: Expr::Bool(true), watchdog: None, span: None },
                    body: vec![
                        Statement::Let {
                            name: "$a_b".to_string(), ty: None,
                            expr: Some(Expr::TupleDestructure(
                                vec!["a".to_string(), "b".to_string()],
                                Box::new(Expr::Tuple(vec![Expr::Integer(5), Expr::Integer(6)])),
                            )),
                            address: None, address_expr: None, bit_range: None,
                            is_override: false, modifiers: vec![],
                        },
                        Statement::Assignment {
                            lhs: Expr::Identifier("val".to_string()),
                            expr: Expr::Identifier("b".to_string()),
                            timeout: None, modifiers: vec![],
                        },
                    ],
                    reactor_speed: None, span: None, is_lambda: false,
                    dependencies: vec![], is_async: false,
                    attrs: vec![], modifiers: vec![], variant_bodies: vec![],
                }),
            ],
            ..empty_program()
        };
        let output = backend.generate(&program);
        assert!(output.contains("add i64 0, %tdr"), "Should bind destructured vars. Got: {}", output);
    }

    #[test]
    fn test_list_index_assign_non_ssa() {
        let mut backend = LlvmBackend::new();
        let program = Program {
            items: vec![
                TopLevel::StateDecl(StateDecl {
                    name: "xs".to_string(), ty: Type::Int,
                    expr: Some(Expr::ListLiteral(vec![Expr::Integer(10), Expr::Integer(20), Expr::Integer(30)])),
                    address: None, bit_range: None, is_override: false,
                    os_mode: false, span: None, attrs: vec![],
                }),
                TopLevel::Transaction(Transaction {
                    name: "update".to_string(), is_reactive: false, parameters: vec![],
                    contract: Contract { pre_condition: Expr::Bool(true), post_condition: Expr::Bool(true), watchdog: None, span: None },
                    body: vec![
                        Statement::Assignment {
                            lhs: Expr::ListIndex(Box::new(Expr::Identifier("xs".to_string())), Box::new(Expr::Integer(1))),
                            expr: Expr::Integer(99),
                            timeout: None, modifiers: vec![],
                        },
                    ],
                    reactor_speed: None, span: None, is_lambda: false,
                    dependencies: vec![], is_async: false,
                    attrs: vec![], modifiers: vec![], variant_bodies: vec![],
                }),
            ],
            ..empty_program()
        };
        let output = backend.generate(&program);
        assert!(output.contains("inttoptr i64"), "Should inttoptr list ptr. Output:\n{}", output);
        // store into list element: store i64 %t..., i64* %lep...
        assert!(output.contains("%lep") && output.contains("store i64"), "Should store at list element ptr. Output:\n{}", output);
    }
}