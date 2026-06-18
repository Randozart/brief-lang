pub mod directive;
pub mod dispatch;
pub mod emit_expr;
pub mod emit_stmt;
pub mod emit_toplevel;
pub mod gpu;
pub mod hazard;
pub mod loop_engine;
pub mod optimizer;
pub mod reorder;

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
/// Detect terminating guard at end of body and hoist it.
/// Returns (body_without_guard, vec_of_(field_name, intrinsic_name)).
pub(crate) fn hoist_terminating_guard(
    body: &[Statement],
    field_index_map: &std::collections::HashMap<String, usize>,
) -> (Vec<Statement>, Vec<(String, String)>) {
    let mut stmts: Vec<&Statement> = body.iter()
        .filter(|s| !matches!(s, Statement::Term { .. } | Statement::TermBang { .. }))
        .collect();
    let mut hoist: Vec<(String, String)> = Vec::new();
    let mut let_to_field: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for stmt in body {
        if let Statement::Assignment { lhs: Expr::OwnedRef(fname), expr, .. } = stmt {
            if field_index_map.contains_key(fname) {
                let s = format!("{:?}", expr);
                if let Some(let_name) = s.strip_prefix("Identifier(\"").and_then(|s| s.split('"').next()) {
                    let_to_field.insert(let_name.to_string(), fname.clone());
                }
            }
        }
    }
    while let Some(last_idx) = stmts.len().checked_sub(1) {
        if let Statement::Guarded { statements, .. } = &stmts[last_idx] {
            let is_terminating = statements.iter().any(|s| matches!(s, Statement::TermBang { .. }));
            if !is_terminating { break; }
            for s in statements {
                if let Statement::Expression(Expr::IntrinsicCall { intrinsic, args }) = s {
                    let intrinsic_name = intrinsic.name();
                    if let Some(Expr::Identifier(fname)) = args.first() {
                        if field_index_map.contains_key(fname) {
                            hoist.push((fname.clone(), intrinsic_name.to_string()));
                        }
                    }
                }
                if let Statement::TermBang { values, swan_song, .. } = s {
                    for v in values {
                        if let Some(Expr::IntrinsicCall { intrinsic, args }) = v {
                            let intrinsic_name = intrinsic.name();
                            if let Some(Expr::Identifier(fname)) = args.first() {
                                if field_index_map.contains_key(fname) {
                                    hoist.push((fname.clone(), intrinsic_name.to_string()));
                                }
                            }
                        }
                    }
                    if let Some(ss) = swan_song {
                        if let Statement::Expression(Expr::IntrinsicCall { intrinsic, args }) = ss.as_ref() {
                            let intrinsic_name = intrinsic.name();
                            if let Some(Expr::Identifier(fname)) = args.first() {
                                if field_index_map.contains_key(fname) {
                                    hoist.push((fname.clone(), intrinsic_name.to_string()));
                                } else if let Some(mapped_field) = let_to_field.get(fname) {
                                    hoist.push((mapped_field.clone(), intrinsic_name.to_string()));
                                }
                            }
                        }
                    }
                }
            }
            if !hoist.is_empty() {
                stmts.pop();
            }
            break;
        } else { break; }
    }
    let body_vec: Vec<Statement> = stmts.into_iter().cloned().collect();
    (body_vec, hoist)
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

/// Map Brief Type to native LLVM type string.
/// This is the single source of truth — eliminates i64 boxing for strings, chars, bools.
impl TypedRegister {
    pub fn llvm(&self) -> &'static str {
        match self.ty {
            Type::Bool => "i1",
            Type::Char => "i32",
            Type::Int | Type::UInt => "i64",
            Type::Float => "float",
            Type::String | Type::Data => "i8*",
            _ => "i64",
        }
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
        Statement::Foreach { list, body, .. } => {
            collect_strings_expr(list, seen, out);
            for s in body { collect_strings_stmt(s, seen, out); }
        }
        Statement::Oracle { body, handler, .. } => {
            for s in body { collect_strings_stmt(s, seen, out); }
            for s in handler { collect_strings_stmt(s, seen, out); }
        }
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
        Expr::String(s) | Expr::RegexLiteral(s) => {
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
        Expr::IntrinsicCall { intrinsic: _, args } => { for a in args { collect_strings_expr(a, seen, out); } }
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
        // Type check expressions
        Expr::IsType(e, _) => { collect_strings_expr(e, seen, out); }
        Expr::FromCheck(e, _) => { collect_strings_expr(e, seen, out); }
        Expr::Like(l, r) => { collect_strings_expr(l, seen, out); collect_strings_expr(r, seen, out); }
        // Terminals
        Expr::Integer(_) | Expr::Float(_) | Expr::Bool(_) | Expr::Char(_) | Expr::Term | Expr::Identifier(_)
        | Expr::Ellipsis | Expr::TypeRef(_) | Expr::OwnedRef(_) | Expr::PriorState(_) => {}
        // Macro/template nodes — should be expanded before reaching backends
        Expr::TemplateCall { .. } | Expr::MacroCall { .. } | Expr::Interpolate(..) | Expr::InterpolateExpr(..) | Expr::QuoteBlock { .. } => {
            unreachable!("macro/template should have been expanded")
        }
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
pub(super) fn trg_llvm_storage_ty(ty: &Type) -> &str {
    match ty {
        Type::Bool => "i8",
        Type::Int | Type::UInt => "i64",
        Type::Float => "float",
        Type::Char => "i32",
        Type::String | Type::Data => "i8*",
        _ => "i8", // fallback for unsupported types
    }
}

/// Map a field's LLVM storage type string to its TBAA metadata node index.
/// Returns the !N index into the TBAA tree emitted at end of module.
pub(super) fn tbaa_node(ty_str: &str) -> i32 {
    match ty_str {
        "i64" => 1,  // Int / UInt / boxed list/counter
        "i8"  => 2,  // Bool
        "i32" => 3,  // Char
        "i8*" | "ptr" => 4,  // String / Data
        "float" => 5, // Float
        _ => 1,  // fallback: Int
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
    // ── Target & Spec ──────────────────────────────────────
    spec: Option<crate::target_spec::TargetSpec>,
    explain: bool,

    // ── State Fields ───────────────────────────────────────
    field_index_map: HashMap<String, usize>,
    field_types: Vec<String>,
    pub(crate) field_initializers: HashMap<String, Option<Expr>>,
    range_bounds: HashMap<String, (i64, i64)>,
    field_to_meta_idx: HashMap<String, usize>,
    exit_condition: Option<Box<Expr>>,
    has_natural_exit: bool,

    // ── MMIO & Schema ──────────────────────────────────────
    mmio_fields: HashMap<String, u64>,
    mmio_initializers: HashMap<String, Option<Expr>>,
    mmio_prepopulated: bool,
    schema_aliases: HashMap<String, crate::dbrief::DbriefType>,

    // ── Codegen State (per-function) ───────────────────────
    pub(crate) txn_counter: usize,
    pub(crate) metadata_counter: usize,  // for !llvm.loop metadata nodes
    pub(crate) dep_graph: crate::analysis::dependency_graph::DependencyGraph, // trg dependency graph
    pending_cleanup: Vec<Statement>,
    pub(crate) let_bindings: HashMap<String, String>,
    pub(crate) let_binding_types: HashMap<String, Type>,
    /// 2026-06-17: Original type before boxing (e.g. String→Int).
    /// Used by is_string_chain to detect string parameters stored as Type::Int.
    pub(crate) let_original_types: HashMap<String, Type>,
    terminated: bool,
    returns_i64: bool,
    fn_ret_ty: String,
    callable_txn_result: Option<String>,
    callable_txn_post_label: Option<String>,
    in_callable_txn: bool,
    loop_exit_label: Option<String>,
    phi_induction_reg: Option<(String, String, String)>, // (counter_field, phi_reg, next_reg)
    pending_post_hoist: Vec<(String, String)>, // post-loop prints saved for emission after canonical loop exit
    param_slots: HashMap<String, String>,
    state_reg_name: String,

    // ── SSA State ──────────────────────────────────────────
    ssa_state_reg: Option<String>,
    ssa_old_float_regs: HashMap<String, String>,
    ssa_old_int_regs: HashMap<String, String>,
    pub(crate) reg_float_cache: HashMap<String, String>,
    reg_type_cache: HashMap<String, Type>,

    // ── Optimization ───────────────────────────────────────
    pub(crate) optimize_budget: u64,
    optimize_report: bool,
    optimize_size: Option<u64>,
    pgo_profile: Option<crate::analysis::pgo::PgoProfile>,
    pgo_guard_idx: usize,
    has_cycles: bool,
    slp_hazard_fns: HashSet<String>,

    // ── Async / Thread Pool ────────────────────────────────
    pub(crate) has_async_txns: bool,
    pub(crate) async_txn_names: Vec<String>,
    pub(crate) async_thread_pool_size: u32,
    pub(crate) is_lightweight_async: bool,

    // ── FFI Registry ───────────────────────────────────────
    pub(crate) triggers: HashMap<String, crate::ast::TriggerDeclaration>,
    pub(crate) trigger_names: Vec<String>,
    program_txns: Vec<String>,
    frgn_map: HashMap<String, ForeignSignature>,
    defn_params: HashMap<String, Vec<Type>>,
    defn_return_types: HashMap<String, Vec<Type>>,
    fused_to_first: HashMap<String, String>,
    sampled_triggers: HashMap<String, String>,
    txn_write_masks: HashMap<String, u64>,

    // ── Constants & Strings ────────────────────────────────
    pub(crate) string_constants: Vec<String>,
    pub(crate) constants: HashMap<String, (Type, Expr)>,
    struct_types: HashMap<String, Vec<(String, Type)>>,
    enum_types: HashMap<String, crate::ast::EnumDefinition>,
    variant_disc: HashMap<String, (String, u64, usize)>,

    // ── Reporting ──────────────────────────────────────────
    report_lines: Vec<String>,
    warnings: Vec<String>,
    llvm_extra_flags: Vec<String>,
    dead_info_disabled: bool,

    // ── Optimization Remarks ───────────────────────────────
    pub(crate) remarks: Vec<crate::backend::llvm::directive::OptimizationRemark>,
    emit_remarks: bool,

    // ── GPU Offloading ─────────────────────────────────────
    gpu_offload: bool,
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
            metadata_counter: 100,
            dep_graph: crate::analysis::dependency_graph::DependencyGraph {
                topo_order: Vec::new(),
                bit_index: std::collections::HashMap::new(),
                dependencies: std::collections::HashMap::new(),
                dependents: std::collections::HashMap::new(),
                is_trg: std::collections::HashSet::new(),
                all_vars: std::collections::HashSet::new(),
            },  // start above likely conflict range
            has_cycles: false,
            pending_cleanup: Vec::new(),
            let_bindings: HashMap::new(),
            let_binding_types: HashMap::new(),
            let_original_types: HashMap::new(),
            terminated: false,
            returns_i64: false,
            fn_ret_ty: "void".to_string(),
            callable_txn_result: None,
            callable_txn_post_label: None,
            in_callable_txn: false,
            loop_exit_label: None,
            phi_induction_reg: None,
            pending_post_hoist: Vec::new(),
            param_slots: HashMap::new(),
            range_bounds: HashMap::new(),
            field_to_meta_idx: HashMap::new(),
            triggers: HashMap::new(),
            trigger_names: Vec::new(),
            program_txns: Vec::new(),
            frgn_map: HashMap::new(),
            defn_params: HashMap::new(),
            defn_return_types: HashMap::new(),
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
            remarks: Vec::new(),
            emit_remarks: false,
            gpu_offload: false,
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

    pub fn with_emit_remarks(mut self, emit: bool) -> Self {
        self.emit_remarks = emit;
        self
    }

    pub fn with_gpu_offload(mut self, offload: bool) -> Self {
        self.gpu_offload = offload;
        self
    }

    pub(crate) fn push_remark(&mut self, remark: crate::backend::llvm::directive::OptimizationRemark) {
        if self.emit_remarks {
            self.remarks.push(remark);
        }
    }

    pub fn remarks(&self) -> &[crate::backend::llvm::directive::OptimizationRemark] {
        &self.remarks
    }

    pub fn generate(&mut self, program: &Program) -> String {
        let mut analysis = crate::backend::analyze_program(program, false);
        self.dep_graph = analysis.dependency_graph.clone();

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
        // Inject synthetic __trg_epfd field if program has built-in triggers
        let has_builtin_trg = program.items.iter().any(|item| {
            if let TopLevel::Trigger(t) = item {
                matches!(t.address, crate::ast::LinkRef::Stdin | crate::ast::LinkRef::Timer(_) | crate::ast::LinkRef::Signal(_))
            } else { false }
        });
        if has_builtin_trg && !self.field_index_map.contains_key("__trg_epfd") {
            let idx = self.field_index_map.len();
            self.field_index_map.insert("__trg_epfd".to_string(), idx);
            self.field_types.push("i32".to_string());
            self.field_initializers.insert("__trg_epfd".to_string(), None);
        }
        self.validate_schema_types();
        self.triggers.clear();
        self.trigger_names.clear();
        self.program_txns.clear();
        self.defn_params.clear();
        self.defn_return_types.clear();
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
                        self.defn_return_types.insert(t.name.clone(), t.outputs.clone());
                    }
                }
                TopLevel::Trigger(t) => {
                    self.triggers.insert(t.name.clone(), t.clone());
                    self.trigger_names.push(t.name.clone());
                }
                TopLevel::Definition(d) => {
                    let tys: Vec<Type> = d.parameters.iter().map(|(_, t)| t.clone()).collect();
                    self.defn_params.insert(d.name.clone(), tys);
                    self.defn_return_types.insert(d.name.clone(), d.outputs.clone());
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
        // Skip names that are also linked triggers — they'll be emitted as global variables below.
        let trigger_linked_symbols: std::collections::HashSet<&str> = self.triggers.iter()
            .filter_map(|(_, t)| match &t.address {
                crate::ast::LinkRef::Linked(sym) => Some(sym.as_str()),
                _ => None,
            })
            .collect();
        for (name, sig) in &self.frgn_map {
            if trigger_linked_symbols.contains(name.as_str()) { continue; }
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

        // Declare memory/string helpers used by inline concat and FFI marshaling
        // malloc/strlen declared by brief_rt.c via #include <stdlib.h> + <string.h>
        writeln!(out, "declare i64 @strlen(i8*) #1").ok();

        // Declare epoll + libc functions for the trg reactive event loop
        writeln!(out, "declare i32 @epoll_create1(i32) #1").ok();
        writeln!(out, "declare i32 @epoll_ctl(i32, i32, i32, i8*) #1").ok();
        writeln!(out, "declare i32 @epoll_wait(i32, i8*, i32, i32) #1").ok();
        writeln!(out, "declare i64 @read(i32, i8*, i64) #1").ok();
        writeln!(out, "declare i32 @fcntl(i32, i32, i32) #1").ok();
        writeln!(out, "declare i32 @timerfd_create(i32, i32) #1").ok();
        writeln!(out, "declare i32 @timerfd_settime(i32, i32, i8*, i8*) #1").ok();
        writeln!(out, "declare i32 @signalfd(i32, i8*, i32) #1").ok();
        writeln!(out, "declare i32 @sigemptyset(i8*) #1").ok();
        writeln!(out, "declare i32 @sigaddset(i8*, i32) #1").ok();
        writeln!(out, "declare i32 @sigprocmask(i32, i8*, i8*) #1").ok();
        // The step() function is defined in the same module — no declare needed.
        // writeln!(out, "declare void @step(%State*, i64) #1").ok();

        // Declare cast helper functions
        writeln!(out, "declare i8* @__chr_to_str(i32) #1").ok();
        writeln!(out, "declare i64 @__int_to_str(i64) #1").ok();
        writeln!(out, "declare i64 @__str_to_int(i8*) #1").ok();

        // Format string constants for benchmark intrinsics (print_int#, print_float#)
        writeln!(out, "@FMT_INT = private unnamed_addr constant [5 x i8] c\"%ld\\0A\\00\"").ok();
        writeln!(out, "@FMT_FLOAT = private unnamed_addr constant [6 x i8] c\"%.9f\\0A\\00\"").ok();
        writeln!(out, "@FMT_STR = private unnamed_addr constant [4 x i8] c\"%s\\0A\\00\"").ok();
        // Error message for read_file# — returned as Err's String payload
        writeln!(out, "@STR_READFILE_ERR = private unnamed_addr constant [15 x i8] c\"file not found\\00\"").ok();
        // Declare libc functions used by direct-libc intrinsics
        writeln!(out, "@stdout = external global ptr").ok();
        writeln!(out, "declare i32 @fprintf(ptr, ptr, ...) #1").ok();
        writeln!(out, "declare i32 @fputc(i32, ptr) #1").ok();
        writeln!(out, "declare i32 @fflush(ptr) #1").ok();
        writeln!(out, "declare ptr @getenv(ptr) #1").ok();
        writeln!(out, "declare i64 @atol(ptr) #1").ok();
        writeln!(out, "declare void @exit(i32) #1").ok();
        writeln!(out, "declare i32 @setvbuf(ptr, ptr, i32, i64) #1").ok();
        writeln!(out, "declare i32 @sleep(i32) #1").ok();
        writeln!(out, "declare i32 @nanosleep(ptr, ptr) #1").ok();
        writeln!(out, "declare ptr @fopen(ptr, ptr) #1").ok();
        writeln!(out, "declare i64 @fwrite(ptr, i64, i64, ptr) #1").ok();
        writeln!(out, "declare i32 @fclose(ptr) #1").ok();

        // Emit external global declarations for linked triggers (fixes bug 4B)
        for (name, trg) in &self.triggers {
            if let crate::ast::LinkRef::Linked(sym) = &trg.address {
                let store_ty = trg_llvm_storage_ty(&trg.ty);
                let align = if store_ty == "i64" { 8 } else if store_ty == "i32" { 4 } else { 1 };
                writeln!(out, "@{} = external global {}, align {}", sym, store_ty, align).ok();
                // Warn if a linked trigger symbol is also declared as a frgn function
                if self.frgn_map.contains_key(sym.as_str()) {
                    eprintln!("warning: '{}' is declared as a frgn function but used as a @ link trigger. \
                               Use a volatile C variable for triggers, or built-in sources like @stdin#.", sym);
                }
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

        // Emit string constants as global Brief headers
        // Each is a 2-slot header: { data_ptr (ptrtoint of slot 2), length, [chars] }
        // This makes ALL string values in the IR uniform — same format as heap-allocated strings.
        for (si, s) in self.string_constants.iter().enumerate() {
            let escaped = escape_llvm_string(s);
            let len = s.len();
            writeln!(out, "@str.{} = private unnamed_addr constant <{{ i64, i64, [{} x i8] }}> <{{", si, len + 1).ok();
            writeln!(out, "  i64 ptrtoint (i8* getelementptr inbounds (<{{ i64, i64, [{} x i8] }}>, <{{ i64, i64, [{} x i8] }}>* @str.{}, i64 0, i32 2) to i64),", len + 1, len + 1, si).ok();
            writeln!(out, "  i64 {},", len).ok();
            writeln!(out, "  [{} x i8] c\"{}\\00\"", len + 1, escaped).ok();
            writeln!(out, "}}>, align 8").ok();
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
                            let raw_body = &txns[0].1.body;
                            let (body_stmts, post_hoist) = hoist_terminating_guard(raw_body, &self.field_index_map);
                            self.pending_post_hoist = post_hoist;
                            let has_guards = body_stmts.iter().any(|s| matches!(s, crate::ast::Statement::Guarded { .. } | crate::ast::Statement::Escape(_) | crate::ast::Statement::SyncBlock { .. }));
                            if has_guards && !crate::proof_engine::prove_linear(&body_stmts) {
                                // A005b: non-linear body with branching guards → memory path (no phi)
                                self.warnings.push(format!("info: txn '{}' dispatched via folded (memory, no phi — not provably linear)", &node.name));
                                self.emit_folded_memory_main(&mut out, &node.name, counter_idx, total_idx, total_const_name, &body_stmts);
                            } else {
                                // A005a: straight-line or provably linear body → SSA insertvalue path
                                self.warnings.push(format!("info: txn '{}' dispatched via folded SSA (inline, {})", &node.name,
                                    if has_guards { "proven linear" } else { "straight-line" }));
                                self.emit_folded_main(&mut out, &node.name, counter_idx, total_idx, total_const_name, false, Some(&body_stmts));
                            }
                            true
                        }
                    } else { false }
                } else { false }
            } else { false }
        } else { false };

        // Emit the trg step() function if the program has trigger declarations.
        // The step() function recomputes dependent variables in topological order
        // when trigger inputs change. It is called from the event loop.
        if !self.trigger_names.is_empty() {
            let trg_names = self.trigger_names.clone();
            self.emit_trg_step(&mut out, &analysis.dependency_graph, &trg_names);
        }

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
                    self.emit_ssa_main(&mut out, &txns, false);
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
            } else if !txns.is_empty()
                && self.async_txn_names.is_empty()
                && self.mmio_fields.is_empty()
            {
                // A006: Direct phi-based loop — no async, no MMIO.
                // Inline all txn bodies directly in main() instead of reactor_tick.
                // Triggers are sampled inline via lazy emit_trg_load, wake path uses
                // __rt_wait between ticks. LLVM promotes %State fields to phi nodes.
                if has_wake_triggers {
                    writeln!(out, "declare void @__rt_wait() local_unnamed_addr").ok();
                }
                self.warnings.push(
                    "info: program dispatched via direct SSA loop".into()
                );
                self.emit_ssa_main(&mut out, &txns, has_wake_triggers);
            } else if !txns.is_empty() {
                // reactor loop fallback — only reached for async dispatch or MMIO
                // (all other programs go through A006 direct SSA loop above)
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

        // TBAA metadata tree for type-based alias analysis
        // Each TBAA node defines a type in the Brief type hierarchy.
        // LLVM uses this to disambiguate loads/stores: accesses tagged
        // with different type trees are assumed to never alias.
        writeln!(out).ok();
        writeln!(out, "!0 = !{{!\"Brief\"}}").ok();
        writeln!(out, "!1 = !{{!\"Int\", !0}}").ok();
        writeln!(out, "!2 = !{{!\"Bool\", !0}}").ok();
        writeln!(out, "!3 = !{{!\"Char\", !0}}").ok();
        writeln!(out, "!4 = !{{!\"String\", !0}}").ok();
        writeln!(out, "!5 = !{{!\"Float\", !0}}").ok();

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
            } else if let TopLevel::Trigger(t) = item {
                // Triggers get a slot in the state struct so the event loop
                // can store their values and emit_expr can load them.
                self.field_index_map
                    .insert(t.name.clone(), self.field_types.len());
                self.field_types.push(self.llvm_type(&t.ty).to_string());
                self.field_initializers.insert(t.name.clone(), None);
            }
        }
    }

}

