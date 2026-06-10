pub mod emit_expr;
pub mod emit_stmt;
pub mod folded_loop;
pub mod optimizer;

#[cfg(test)]
mod tests;

#[cfg(all(kani, feature = "kani_full"))]
mod kani;

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

pub(crate) fn float_to_llvm_hex(f: f64) -> String {
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
        Expr::Literal(lit) => {
            if let crate::features::literal::LiteralExpr::Float(f) = lit.as_ref() {
                Some(*f)
            } else {
                None
            }
        }
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
    ArrowDir, BracketOp, DispatchMode, Expr, ForeignSignature, MatchArm, MatchPattern, Pattern, Program, ProjectionTarget, SliceCoordinate, Statement, TopLevel, Type,
};
use crate::features::traits::{ExprCodegenLLVM, ExprDispatch};

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
        Statement::Escape(Some(e)) => { collect_strings_expr(e, seen, out); }
        Statement::Escape(None) => {}
        Statement::LocalTrigger { expr, .. } => { if let Some(e) = expr { collect_strings_expr(e, seen, out); } }
        Statement::SyncBlock { body } => { for s in body { collect_strings_stmt(s, seen, out); } }
        Statement::Alka { .. } | Statement::OnExit { .. } | Statement::InlineAsm { .. } => {}
    }
}

fn collect_strings_from_subtype_ops(ops: &[crate::ast::SubtypeOp], seen: &mut std::collections::HashSet<String>, out: &mut Vec<String>) {
    for op in ops {
        match op {
            crate::ast::SubtypeOp::Filter(e) | crate::ast::SubtypeOp::Map(e) | crate::ast::SubtypeOp::Sort(e)
            | crate::ast::SubtypeOp::Group(e) | crate::ast::SubtypeOp::Sum(e) | crate::ast::SubtypeOp::Avg(e)
            | crate::ast::SubtypeOp::Min(e) | crate::ast::SubtypeOp::Max(e) | crate::ast::SubtypeOp::Match(e) => {
                collect_strings_expr(e, seen, out);
            }
            crate::ast::SubtypeOp::Join(a, b) => {
                collect_strings_expr(a, seen, out);
                collect_strings_expr(b, seen, out);
            }
            crate::ast::SubtypeOp::Limit(_) | crate::ast::SubtypeOp::Skip(_) | crate::ast::SubtypeOp::Unique
            | crate::ast::SubtypeOp::Count => {}
        }
    }
}

fn collect_strings_from_bracket_ops(ops: &[crate::ast::BracketOp], seen: &mut std::collections::HashSet<String>, out: &mut Vec<String>) {
    for op in ops {
        match op {
            crate::ast::BracketOp::Coord(_) => {}
            crate::ast::BracketOp::Mask(e) | crate::ast::BracketOp::Stride(e) => {
                collect_strings_expr(e, seen, out);
            }
        }
    }
}

fn collect_strings_expr(expr: &Expr, seen: &mut std::collections::HashSet<String>, out: &mut Vec<String>) {
    match expr {
        Expr::String(s) => {
            if !seen.contains(s) {
                seen.insert(s.clone());
                out.push(s.clone());
            }
        }
        Expr::Literal(lit) => {
            if let crate::features::literal::LiteralExpr::String(s) = lit.as_ref() {
                if !seen.contains(s) {
                    seen.insert(s.clone());
                    out.push(s.clone());
                }
            }
        }
        // Binary/unary ops
        Expr::Add(l, r) | Expr::Sub(l, r) | Expr::Mul(l, r) | Expr::Div(l, r) | Expr::Mod(l, r)
        | Expr::Eq(l, r) | Expr::Ne(l, r) | Expr::Lt(l, r) | Expr::Le(l, r) | Expr::Gt(l, r) | Expr::Ge(l, r)
        | Expr::And(l, r) | Expr::Or(l, r) | Expr::BitAnd(l, r) | Expr::BitOr(l, r) | Expr::BitXor(l, r)
        | Expr::Shl(l, r) | Expr::Shr(l, r) | Expr::Concat(l, r) => {
            collect_strings_expr(l, seen, out);
            collect_strings_expr(r, seen, out);
        }
        Expr::Not(e) | Expr::Neg(e) | Expr::BitNot(e) | Expr::Cast(e, _) => {
            collect_strings_expr(e, seen, out);
        }
        Expr::OwnedRef(_) | Expr::PriorState(_) => {}
        // Collections
        Expr::ListLiteral(elems) => { for e in elems { collect_strings_expr(e, seen, out); } }
        Expr::MapLiteral(pairs) => { for (k, v) in pairs { collect_strings_expr(k, seen, out); collect_strings_expr(v, seen, out); } }
        Expr::SetLiteral(elems) => { for e in elems { collect_strings_expr(e, seen, out); } }
        Expr::ListIndex(l, i) => { collect_strings_expr(l, seen, out); collect_strings_expr(i, seen, out); }
        // Slice/MultiSlice
        Expr::Slice { value, start, end, stride, mask } => {
            collect_strings_expr(value, seen, out);
            for opt in [start, end, stride, mask].into_iter().flatten() { collect_strings_expr(opt, seen, out); }
        }
        Expr::MultiSlice { value, ops } => {
            collect_strings_expr(value, seen, out);
            collect_strings_from_bracket_ops(ops, seen, out);
        }
        // Tuple
        Expr::Tuple(elems) => { for e in elems { collect_strings_expr(e, seen, out); } }
        Expr::TupleDestructure(_, e) => { collect_strings_expr(e, seen, out); }
        // Arrow ops
        Expr::ArrowMut { target, index, value, .. } => {
            collect_strings_expr(target, seen, out);
            collect_strings_expr(index, seen, out);
            if let Some(v) = value { collect_strings_expr(v, seen, out); }
        }
        Expr::ArrowDiscard { target, index } => {
            collect_strings_expr(target, seen, out);
            collect_strings_expr(index, seen, out);
        }
        Expr::ArrowTransfer { dest, source, filter } => {
            collect_strings_expr(dest, seen, out);
            collect_strings_expr(source, seen, out);
            if let Some(f) = filter { collect_strings_expr(f, seen, out); }
        }
        // Field/object
        Expr::FieldAccess(o, _) => { collect_strings_expr(o, seen, out); }
        Expr::StructInstance(_, fields) => { for (_, e) in fields { collect_strings_expr(e, seen, out); } }
        Expr::ObjectLiteral(fields) => { for (_, e) in fields { collect_strings_expr(e, seen, out); } }
        // Call, Match, Pattern
        Expr::Call(_, args) => { for a in args { collect_strings_expr(a, seen, out); } }
        Expr::Match { value, arms } => { collect_strings_expr(value, seen, out); for arm in arms { collect_strings_expr(&arm.body, seen, out); } }
        Expr::PatternMatch { value, .. } => { collect_strings_expr(value, seen, out); }
        // Block
        Expr::Block(stmts, last) => {
            for s in stmts { collect_strings_stmt(s, seen, out); }
            collect_strings_expr(last, seen, out);
        }
        // Projection/Sig/Subtype
        Expr::Projection { source, .. } => { collect_strings_expr(source, seen, out); }
        Expr::SubtypeProjection { source, ops } => {
            collect_strings_expr(source, seen, out);
            collect_strings_from_subtype_ops(ops, seen, out);
        }
        Expr::SigCall { expr, .. } => { collect_strings_expr(expr, seen, out); }
        // Pattern B packed variants
        Expr::ArrowMutExpr(e) => {
            collect_strings_expr(e.target.as_ref(), seen, out);
            collect_strings_expr(e.index.as_ref(), seen, out);
            if let Some(v) = e.value.as_ref() { collect_strings_expr(v, seen, out); }
        }
        Expr::ArrowDiscardExpr(e) => {
            collect_strings_expr(e.target.as_ref(), seen, out);
            collect_strings_expr(e.index.as_ref(), seen, out);
        }
        Expr::ArrowTransferExpr(e) => {
            collect_strings_expr(e.dest.as_ref(), seen, out);
            collect_strings_expr(e.source.as_ref(), seen, out);
            if let Some(f) = e.filter.as_ref() { collect_strings_expr(f, seen, out); }
        }
        Expr::ListLiteralExpr(e) => { for el in &e.elements { collect_strings_expr(el, seen, out); } }
        Expr::MapLiteralExpr(e) => { for (k, v) in &e.entries { collect_strings_expr(k, seen, out); collect_strings_expr(v, seen, out); } }
        Expr::SetLiteralExpr(e) => { for el in &e.entries { collect_strings_expr(el, seen, out); } }
        Expr::MultiSliceExpr(e) => { collect_strings_expr(e.value.as_ref(), seen, out); collect_strings_from_bracket_ops(&e.ops, seen, out); }
        Expr::FieldAccessExpr(e) => { collect_strings_expr(e.obj.as_ref(), seen, out); }
        Expr::ObjectLiteralExpr(e) => { for (_, v) in &e.fields { collect_strings_expr(v, seen, out); } }
        Expr::SubtypeProjectionExpr(e) => {
            collect_strings_expr(e.source.as_ref(), seen, out);
            collect_strings_from_subtype_ops(&e.ops, seen, out);
        }
        Expr::BinaryOp(e) => { collect_strings_expr(e.left.as_ref(), seen, out); collect_strings_expr(e.right.as_ref(), seen, out); }
        Expr::UnaryOp(e) => { collect_strings_expr(e.operand.as_ref(), seen, out); }
        Expr::CallExpr(e) => { for a in &e.args { collect_strings_expr(a, seen, out); } }
        Expr::ProjectionExpr(e) => { collect_strings_expr(e.source.as_ref(), seen, out); }
        Expr::BlockExpr(e) => { for s in &e.stmts { collect_strings_stmt(s, seen, out); } collect_strings_expr(e.last.as_ref(), seen, out); }
        Expr::MatchExpr(e) => { collect_strings_expr(e.value.as_ref(), seen, out); for arm in &e.arms { collect_strings_expr(&arm.body, seen, out); } }
        Expr::PatternMatchExpr(e) => { collect_strings_expr(e.value.as_ref(), seen, out); }
        Expr::TupleDestructureExpr(e) => { collect_strings_expr(e.expr.as_ref(), seen, out); }
        Expr::TupleExpr(e) => { for el in &e.exprs { collect_strings_expr(el, seen, out); } }
        Expr::SigCallExpr(e) => { collect_strings_expr(e.expr.as_ref(), seen, out); }
        Expr::SliceExpr(e) => {
            collect_strings_expr(e.value.as_ref(), seen, out);
            for opt in [&e.start, &e.end, &e.stride, &e.mask].into_iter().flatten() { collect_strings_expr(opt, seen, out); }
        }
        Expr::StructInstanceExpr(e) => { for (_, v) in &e.fields { collect_strings_expr(v, seen, out); } }
        Expr::EllipsisExpr(_) | Expr::DbvlTable { .. } | Expr::DbvlTableExpr(_) => {}
        // Terminals
        Expr::Integer(_) | Expr::Float(_) | Expr::Bool(_) | Expr::Char(_) | Expr::Term | Expr::Identifier(_)
        | Expr::Ellipsis | Expr::TypeRef(_) => {}
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
    pub(crate) field_initializers: HashMap<String, Option<Expr>>,
    mmio_fields: HashMap<String, u64>,
    mmio_initializers: HashMap<String, Option<Expr>>,
    mmio_prepopulated: bool,
    schema_aliases: HashMap<String, crate::dbrief::DbriefType>,
    pgo_profile: Option<crate::analysis::pgo::PgoProfile>,
    pgo_guard_idx: usize,
    pub(crate) txn_counter: usize,
    has_cycles: bool,
    pending_cleanup: Vec<Statement>,
    let_bindings: HashMap<String, String>,
    /// Types of let-bound expressions — needed so FieldAccess can GEP into
    /// struct instances held in local variables.
    let_binding_types: HashMap<String, Type>,
    terminated: bool,
    returns_i64: bool,
    /// Return type of the enclosing function for term/termbang/escape emission.
    /// "void" by default; set to "i32" inside emit_folded_main / emit_ssa_main / etc.
    fn_ret_ty: String,
    /// When set, Term/TermBang store values to this alloca and branch to
    /// callable_txn_post_label instead of emitting `ret`. Used by callable txns.
    callable_txn_result: Option<String>,
    callable_txn_post_label: Option<String>,
    in_callable_txn: bool,
    /// Maps parameter name → alloca slot name for callable txns.
    /// Used by Statement::Assignment to store updated values to mutable param slots.
    param_slots: HashMap<String, String>,
    range_bounds: HashMap<String, (i64, i64)>,
    field_to_meta_idx: HashMap<String, usize>,
    pub(crate) triggers: HashMap<String, crate::ast::TriggerDeclaration>,
    pub(crate) trigger_names: Vec<String>,
    program_txns: Vec<String>,
    frgn_map: HashMap<String, ForeignSignature>,
    defn_params: HashMap<String, Vec<Type>>,
    pub(crate) string_constants: Vec<String>,
    pub(crate) constants: HashMap<String, (Type, Expr)>,
    fused_to_first: HashMap<String, String>,
    sampled_triggers: HashMap<String, String>,
    txn_write_masks: HashMap<String, u64>,
    pub(crate) optimize_budget: u64,
    optimize_report: bool,
    optimize_size: Option<u64>,
    report_lines: Vec<String>,
    pub(crate) has_async_txns: bool,
    pub(crate) async_txn_names: Vec<String>,
    pub(crate) async_thread_pool_size: u32,
    pub(crate) is_lightweight_async: bool,
    exit_condition: Option<Box<Expr>>,
    has_natural_exit: bool,
    dead_info_disabled: bool,
    warnings: Vec<String>,
    ssa_state_reg: Option<String>,
    llvm_extra_flags: Vec<String>,
    slp_hazard_fns: HashSet<String>,
    pub(crate) reg_float_cache: HashMap<String, String>,
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
            fn_ret_ty: "void".to_string(),
            callable_txn_result: None,
            callable_txn_post_label: None,
            in_callable_txn: false,
            param_slots: HashMap::new(),
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
        } else if !analysis.region_analyzer.composed_chains.is_empty() {
            // A002 already covers empty-chains case: no precomputable program.
            // Only emit A001 when chains exist but budget/FFI prevents evaluation.
            let has_ffi = analysis.region_analyzer.composed_chains.iter().any(|cc|
                crate::analysis::region::has_ffi_or_trigger_stmt_in_chain(&cc.composed_body));
            if has_ffi {
                self.warnings.push("info: program not fully precomputed — FFI calls in transaction body prevent compile-time evaluation".into());
            } else {
                self.warnings.push(format!(
                    "info: program not fully precomputed — budget {} exceeded by composed chain product. Emitting runtime loop.",
                    self.optimize_budget));
            }
            None
        } else {
            None
        };

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
                    // Register callable txn param types for Expr::Call marshaling
                    let has_output = t.output_type.is_some() || !t.outputs.is_empty();
                    if !t.is_reactive && (!t.parameters.is_empty() || has_output) {
                        let tys: Vec<Type> = t.parameters.iter().map(|(_, ty)| ty.clone()).collect();
                        self.defn_params.insert(t.name.clone(), tys);
                    }
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
            let mut next_disc: u64 = 0;
            for v in &edef.variants {
                let (vname, field_count) = match v {
                    crate::ast::EnumVariant::Unit(n) => (n.clone(), 0),
                    crate::ast::EnumVariant::Tuple(n, fields) => (n.clone(), fields.len()),
                    crate::ast::EnumVariant::Struct(n, fields) => (n.clone(), fields.len()),
                };
                let disc = next_disc;
                next_disc += 1;
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

        // Select optimization strategy via extracted decision tree
        let strategy = self.select_optimization_strategy(program, &analysis, &txns);
        let dispatch_mode = strategy.dispatch_mode;
        let has_wake_triggers = strategy.has_wake_triggers;
        let enumerable = strategy.enumerable;
        let enum_keys = strategy.enum_keys;
        let enum_txn_names = strategy.enum_txn_names;

        let mut out = String::new();
        self.emit_header(&mut out);
self.emit_declares(&mut out);

        // Emit foreign declares inline (frgn_map is populated from the scan above)
        for (name, sig) in &self.frgn_map {
            let ret_ty = match sig.result_type {
                crate::ast::ResultType::VoidType | crate::ast::ResultType::TrueAssertion => "void",
                crate::ast::ResultType::Projection(ref ts) => {
                    if ts.is_empty() || ts.iter().any(|t| matches!(t, Type::Void)) { "void" }
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
                Expr::Literal(lit) => match lit.as_ref() {
                    crate::features::literal::LiteralExpr::Float(f) => format!("{}:bitcast(i32 {} to float)", llvm_ty, float_to_llvm_hex(*f)),
                    crate::features::literal::LiteralExpr::Integer(n) => format!("{}:{}", llvm_ty, n),
                    crate::features::literal::LiteralExpr::Bool(b) => format!("{}:{}", llvm_ty, if *b { "true" } else { "false" }),
                    crate::features::literal::LiteralExpr::String(_) => format!("{}:null", llvm_ty),
                    crate::features::literal::LiteralExpr::Char(_) | crate::features::literal::LiteralExpr::Term => format!("{}:{}", llvm_ty, name),
                },
                Expr::Integer(n) => format!("{}:{}", llvm_ty, n),
                Expr::Bool(b) => format!("{}:{}", llvm_ty, if *b { "true" } else { "false" }),
                Expr::Neg(inner) => match inner.as_ref() {
                    Expr::Float(f) => format!("{}:bitcast(i32 {} to float)", llvm_ty, float_to_llvm_hex(-*f)),
                    Expr::Literal(lit) => {
                        if let crate::features::literal::LiteralExpr::Float(f) = lit.as_ref() {
                            format!("{}:bitcast(i32 {} to float)", llvm_ty, float_to_llvm_hex(-*f))
                        } else {
                            format!("{}:neg:{}", llvm_ty, name)
                        }
                    }
                    Expr::Integer(n) => format!("{}:-{}", llvm_ty, n),
                    _ => format!("{}:neg:{}", llvm_ty, name),
                },
                Expr::String(_) => format!("{}:null", llvm_ty),
                _ => format!("{}:unresolved:{}", llvm_ty, name),
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
                Expr::Literal(lit) => match lit.as_ref() {
                    crate::features::literal::LiteralExpr::Float(f) => format!("bitcast (i32 {} to float)", float_to_llvm_hex(*f)),
                    crate::features::literal::LiteralExpr::Integer(n) => n.to_string(),
                    crate::features::literal::LiteralExpr::Bool(b) => (if *b { "true" } else { "false" }).to_string(),
                    crate::features::literal::LiteralExpr::String(_) => "null".to_string(),
                    crate::features::literal::LiteralExpr::Char(c) => format!("{}", *c as i64),
                    crate::features::literal::LiteralExpr::Term => "0".to_string(),
                },
                Expr::Integer(n) => n.to_string(),
                Expr::Bool(b) => (if *b { "true" } else { "false" }).to_string(),
                Expr::Neg(inner) => match inner.as_ref() {
                    Expr::Float(f) => format!("bitcast (i32 {} to float)", float_to_llvm_hex(-*f)),
                    Expr::Literal(lit) => {
                        if let crate::features::literal::LiteralExpr::Float(f) = lit.as_ref() {
                            format!("bitcast (i32 {} to float)", float_to_llvm_hex(-*f))
                        } else {
                            if *ty == Type::Float { "0.0".to_string() } else { "0".to_string() }
                        }
                    }
                    Expr::Integer(n) => format!("-{}", n),
                    _ => if *ty == Type::Float { "0.0".to_string() } else { "0".to_string() },
                },
                Expr::String(_) => "null".to_string(),
                _ => {
                    if *ty == Type::Float {
                        "0.0".to_string()
                    } else {
                        "0".to_string()
                    }
                },
            };
            writeln!(out, "@{} = constant {} {}", name, llvm_ty, val_str).ok();
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
        // Precondition functions (skip callable txns — no %State*)
        for (name, txn) in &txns {
            let has_output = txn.output_type.is_some() || !txn.outputs.is_empty();
            if !txn.is_reactive && (!txn.parameters.is_empty() || has_output) { continue; }
            self.emit_pre_function(&mut out, txn, name);
        }
        // Async body functions — simple pre→fire wrapper for worker threads
        for (name, txn) in &txns {
            if self.async_txn_names.iter().any(|n| n.as_str() == name.as_str()) && !self.is_lightweight_async {
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
                                // A005: pure counter fold
                                self.warnings.push(format!("info: txn '{}' dispatched via pure counter fold ({} iterations, O(1) store)", node.name, tv));
                                // Compile-time constant total — emit O(1) store
                                self.emit_folded_pure_counter(&mut out, counter_idx, tv);
                                true
                            } else {
                                // A005: folded SSA (phi pipeline)
                                self.warnings.push(format!("info: txn '{}' dispatched via folded SSA (runtime-variable bound)", node.name));
                                // Pure body + runtime-variable bound → phi-node register pipeline
                                self.emit_folded_main(&mut out, &node.name, counter_idx, total_idx, total_const_name, true, None);
                                true
                            }
                        } else {
                            // A005: folded SSA (non-pure body)
                            self.warnings.push(format!("info: txn '{}' dispatched via folded SSA (non-pure body, inline)", node.name));
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
                // A000: fully precomputed — no runtime loop emitted
                self.warnings.push("info: program fully precomputed — no runtime loop emitted. If this is unexpected, increase --optimize-budget or add frgn calls for observability.".into());
                self.emit_precomputed_main(&mut out, final_values);
                true
            } else { false };

            if !precomputed {
                // A004: warn when a runtime loop has zero observability
                if !txns.is_empty() {
                    let any_has_ffi = txns.iter().any(|(_, t)|
                        t.body.iter().any(|s| crate::analysis::transition_graph::statement_contains_ffi(s)));
                    if !any_has_ffi {
                        self.warnings.push(
                            "warning: emitted runtime loop has no observable side effects — \
                             LLVM may eliminate it entirely. Add frgn calls for output, \
                             or this program may run without producing results.".into());
                    }
                }
                // Multi-txn all-pure folding: when NO triggers exist and ALL
                // reactive async txns have bounded_pre + increments with pure
                // bodies, fold them into a single register-pipeline main loop.
                let multi_foldable = enumerable.is_none()
                    && !has_wake_triggers
                    && !self.async_txn_names.is_empty()
                    && self.async_txn_names.iter().all(|name| {
                        graph.nodes.iter().find(|n| n.name == *name).map_or(false, |node| {
                            (node.is_pure_body || node.is_effectively_pure)
                            && node.bounded_pre.is_some()
                            && node.increments.is_some()
                        })
                    });
                let mut multi_fold_params: HashMap<String, FoldParam> = HashMap::new();
                if multi_foldable {
                    for txn_name in &self.async_txn_names {
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
                    // A005: multi-txn pure fold
                    let txn_list: Vec<&str> = multi_fold_params.keys().map(|s| s.as_str()).collect();
                    self.warnings.push(format!("info: txns [{}] dispatched via multi-txn pure fold (all-internal, async)", txn_list.join(", ")));
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
                    // A005: SSA register pipeline
                    self.warnings.push("info: program dispatched via SSA register pipeline (sequential, bounded txns)".into());
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
                // Declare __rt_wait for wake-triggered programs.
                // trg owns its runtime — the compiler emits the declare implicitly.
                // A005: enum dispatch
                self.warnings.push(format!("info: program dispatched via enum trigger dispatch ({} trigger keys)", enum_keys.len()));
                if has_wake_triggers {
                    writeln!(out, "declare void @__rt_wait() local_unnamed_addr").ok();
                }
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
                // A005: reactor loop (fallback)
                self.warnings.push(format!("info: program dispatched via reactor loop ({})", match dispatch_mode {
                    DispatchMode::Parallel => "parallel thread pool",
                    DispatchMode::Sequential => "sequential tick loop",
                }));
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
                if has_wake_triggers {
                    writeln!(out, "declare void @__rt_wait() local_unnamed_addr").ok();
                }
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
        writeln!(out, "declare void @brief_barrier_release()").ok();
        writeln!(out, "declare void @brief_barrier_wait()").ok();
        writeln!(out, "declare void @brief_thread_pool_init(i32, i8**)").ok();
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
                    match inner.as_ref() {
                        Expr::Float(f) => {
                            let h = float_to_llvm_hex(-*f);
                            let bits_reg = format!("%ip{}b", reg - 1);
                            writeln!(out, "  {} = bitcast i32 {} to float", bits_reg, h).ok();
                            writeln!(out, "  store float {}, float* {}, align {}", bits_reg, p, self.align_of("float")).ok();
                        }
                        Expr::Literal(lit) => {
                            if let crate::features::literal::LiteralExpr::Float(f) = lit.as_ref() {
                                let h = float_to_llvm_hex(-*f);
                                let bits_reg = format!("%ip{}b", reg - 1);
                                writeln!(out, "  {} = bitcast i32 {} to float", bits_reg, h).ok();
                                writeln!(out, "  store float {}, float* {}, align {}", bits_reg, p, self.align_of("float")).ok();
                            } else {
                                writeln!(out, "  store i64 0, i64* {}, align {}", p, self.align_of("i64")).ok();
                            }
                        }
                        Expr::Integer(n) => {
                            writeln!(out, "  store i64 -{}, i64* {}, align {}", n, p, self.align_of("i64")).ok();
                        }
                        _ => {
                            writeln!(out, "  store i64 0, i64* {}, align {}", p, self.align_of("i64")).ok();
                        }
                    }
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
            let reg: String;
            if matches!(t, Type::Bool | Type::Char | Type::String | Type::Data | Type::Float) {
                // These need a conversion from the native ABI type to i64
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
                reg = conv;
            } else {
                // Int, UInt, List, HashMap, etc. — already i64, no conversion needed
                reg = raw;
            }
            self.let_bindings.insert(n.clone(), reg);
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
        // Callable transactions: emit as standalone functions with convergence loop.
        // Only route if the txn has parameters (for argument passing) or a return type
        // (for value return). Plain non-reactive txns without either still use the %State* path.
        let has_output = txn.output_type.is_some() || !txn.outputs.is_empty();
        if !txn.is_reactive && (!txn.parameters.is_empty() || has_output) {
            self.emit_callable_txn(out, txn, name);
            return;
        }
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

    // ── CALLABLE TRANSACTION ──────────────────────────────────
    fn emit_callable_txn(&mut self, out: &mut String, txn: &crate::ast::Transaction, name: &str) {
        self.pending_cleanup.clear();
        self.let_bindings.clear();
        self.let_binding_types.clear();
        self.reg_float_cache.clear();
        self.reg_type_cache.clear();
        self.param_slots.clear();

        // Determine if the txn returns a value.
        // Parse bug: single outputs like `-> Float` store type in outputs Vec
        // but output_type is None (parser line 2758). Both must be checked.
        let has_return = if let Some(ref ot) = txn.output_type {
            match ot {
                crate::ast::OutputType::Single(ty) => !matches!(ty, Type::Void),
                crate::ast::OutputType::Tuple(ts) => !ts.is_empty(),
                _ => false,
            }
        } else {
            !txn.outputs.is_empty() && !matches!(txn.outputs.first(), Some(Type::Void))
        };
        let ret_llvm = if has_return { "i64" } else { "void" };

        // Function signature (same pattern as emit_definition)
        write!(out, "define {} @{}(", ret_llvm, name).ok();
        for (i, (n, t)) in txn.parameters.iter().enumerate() {
            if i > 0 { write!(out, ", ").ok(); }
            write!(out, "{} %arg{}", self.llvm_type(t), i).ok();
        }
        writeln!(out, ") local_unnamed_addr #0 {{").ok();
        writeln!(out, "  entry:").ok();

        // Result slot for term values
        writeln!(out, "  %result = alloca i64, align 8").ok();
        writeln!(out, "  store i64 0, i64* %result, align 8").ok();

        // Parameter storage allocas (for mutability across iterations)
        for (i, (n, t)) in txn.parameters.iter().enumerate() {
            let raw = format!("%arg{}", i);
            // Convert param to i64 (same as emit_definition)
            let conv: String;
            if matches!(t, Type::Bool | Type::Char | Type::String | Type::Data | Type::Float) {
                let ac = format!("%ac{}", i);
                match t {
                    Type::Bool => { writeln!(out, "  {} = zext i8 {} to i64", ac, raw).ok(); }
                    Type::Char => { writeln!(out, "  {} = zext i32 {} to i64", ac, raw).ok(); }
                    Type::String | Type::Data => { writeln!(out, "  {} = ptrtoint i8* {} to i64", ac, raw).ok(); }
                    Type::Float => {
                        let m = format!("%ai{}", i);
                        writeln!(out, "  {} = bitcast float {} to i32", m, raw).ok();
                        writeln!(out, "  {} = zext i32 {} to i64", ac, m).ok();
                    }
                    _ => {}
                }
                conv = ac;
            } else {
                conv = raw;
            }
            // Alloca mutable slot + store initial value
            let slot = format!("%p{}_s", i);
            writeln!(out, "  {} = alloca i64, align 8", slot).ok();
            writeln!(out, "  store i64 {}, i64* {}, align 8", conv, slot).ok();
            self.param_slots.insert(n.clone(), slot);
        }

        // Branch to convergence loop header
        writeln!(out, "  br label %loop").ok();
        writeln!(out, "loop:").ok();

        // Reload params from mutable slots into fresh SSA registers
        for (i, (n, t)) in txn.parameters.iter().enumerate() {
            let slot = format!("%p{}_s", i);
            let loaded = format!("%p{}_l{}", i, self.txn_counter);
            self.txn_counter += 1;
            writeln!(out, "  {} = load i64, i64* {}, align 8", loaded, slot).ok();
            self.let_bindings.insert(n.clone(), loaded);
            self.let_binding_types.insert(n.clone(), t.clone());
        }

        // Set up callable txn state for Term/TermBang/Escape intercept
        self.callable_txn_result = Some("%result".to_string());
        self.callable_txn_post_label = Some("post".to_string());
        self.in_callable_txn = true;
        self.txn_counter = 0;
        self.terminated = false;
        self.returns_i64 = has_return;

        // Precondition check
        if !matches!(txn.contract.pre_condition, Expr::Bool(true)) {
            let cond = self.emit_expr(out, &txn.contract.pre_condition, "  ");
            let i1 = format!("%pc{}", self.txn_counter); self.txn_counter += 1;
            writeln!(out, "  {} = icmp ne i64 {}, 0", i1, cond).ok();
            writeln!(out, "  br i1 {}, label %body, label %done", i1).ok();
        } else {
            // No precondition → body fires once, then loop sees false
            // (since the postcondition becoming true is the convergence signal)
            writeln!(out, "  br label %body").ok();
        }

        // Body label
        writeln!(out, "body:").ok();

        // Emit body statements (Term/TermBang handled by intercept in emit_stmt)
        for s in &txn.body {
            self.emit_stmt(out, s, "  ");
            if self.terminated {
                // A non-intercepted Term/TermBang set terminated —
                // this happens if in_callable_txn is false somehow.
                // Guard against this in case of bugs.
                self.terminated = false;
            }
        }

        // Post-loop: go back to check precondition
        writeln!(out, "post:").ok();
        writeln!(out, "  br label %loop").ok();

        // Done: return the recorded result
        writeln!(out, "done:").ok();
        if has_return {
            let ret = format!("%ret{}", self.txn_counter); self.txn_counter += 1;
            writeln!(out, "  {} = load i64, i64* %result, align 8", ret).ok();
            writeln!(out, "  ret i64 {}", ret).ok();
        } else {
            writeln!(out, "  ret void").ok();
        }
        writeln!(out, "}}").ok();

        // Clean up callable txn state
        self.callable_txn_result = None;
        self.callable_txn_post_label = None;
        self.in_callable_txn = false;
        self.param_slots.clear();
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
}

