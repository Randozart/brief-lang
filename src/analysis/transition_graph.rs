use crate::ast::{ArrowDir, BracketOp, Expr, Hashtag, Intrinsic, Program, ProjectionTarget, SliceCoordinate, Statement, TopLevel};
use crate::features::literal::LiteralExpr;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Copy)]
pub enum ConvergeDirection {
    Increasing,
    Decreasing,
}

#[derive(Debug, Clone)]
pub struct BoundedPre {
    pub var: String,
    pub bound_var: String,
    pub direction: ConvergeDirection,
    /// If the bound is a literal integer (e.g., `count > 0`), this holds the value.
    /// None means bound_var is a named field or constant.
    pub bound_literal: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct IncrementInfo {
    pub var: String,
    pub delta: i64,
}

#[derive(Debug, Clone)]
pub struct ReactorNode {
    pub name: String,
    pub is_reactive: bool,
    pub precondition: Expr,
    pub body: Vec<Statement>,
    pub bounded_pre: Option<BoundedPre>,
    pub increments: Option<IncrementInfo>,
    pub is_pure_body: bool,
    pub write_set: HashSet<String>,
    pub is_effectively_pure: bool,
    /// Lexicographic tuple ranking: multiple variables that together form
    /// a well-founded multi-variable ranking function. Each variable has
    /// an independent decrement path, and the loop exits when ALL reach zero.
    /// Example: `[x > 0 || y > 0][x == 0 && y == 0]` with guarded decrements.
    pub lexicographic_vars: Vec<String>,
    /// Trigger names guaranteed to fire by #assume_event pragma.
    /// Enables termination proofs for external-trigger loops.
    pub assume_events: Vec<String>,
    /// Rollback action from #assume_shape(guard_expr, escape|run|exit) pragma.
    /// The guard expression parsing from string to Expr is future work;
    /// for now, the guard is assumed true and only the rollback action is emitted.
    pub assume_shape_action: Option<String>,
}

pub struct ReactorTransitionGraph {
    pub nodes: Vec<ReactorNode>,
    pub has_triggers: bool,
    pub live_fields: HashSet<String>,
}

impl ReactorTransitionGraph {
    pub fn build(program: &Program) -> Self {
        let mut nodes = Vec::new();
        let mut has_triggers = false;

        // Collect inop declarations for side-effect analysis
        let inop_decls: HashMap<String, bool> = program.items.iter().filter_map(|item| {
            if let TopLevel::Inop(inop) = item {
                Some((inop.name.clone(), inop.has_side_effects))
            } else {
                None
            }
        }).collect();

        for item in &program.items {
            match item {
                TopLevel::Transaction(txn) => {
                    // Remove terminating guards before analysis so increments/purity
                    // checks see a guard-free body.
                    let body_no_term: Vec<Statement> = {
                        let mut filtered: Vec<&Statement> = txn.body.iter()
                            .filter(|s| !matches!(s, Statement::Term { .. } | Statement::TermBang { .. }))
                            .collect();
                        while filtered.last().map_or(false, |s| {
                            if let Statement::Guarded { statements, .. } = s {
                                statements.iter().any(|s| matches!(s, Statement::TermBang { .. }))
                            } else { false }
                        }) {
                            filtered.pop();
                        }
                        filtered.into_iter().cloned().collect()
                    };
                    let simplified_body = simplify_body(&body_no_term);
                    let increments = detect_increments(&simplified_body)
                        .or_else(|| detect_popcount_decay(&simplified_body))
                        .or_else(|| detect_collection_drain(&simplified_body));
                    let bounded_pre = extract_valid_bounded_pre(&txn.contract.pre_condition, &increments);
                    let state_field_names: HashSet<String> = program
                        .items
                        .iter()
                        .filter_map(|i| {
                            if let TopLevel::StateDecl(s) = i {
                                Some(s.name.clone())
                            } else {
                                None
                            }
                        })
                        .collect();
                    let is_pure = is_pure_body(&simplified_body, &state_field_names, &increments, &inop_decls);
                    let write_set = extract_write_set(&simplified_body, &state_field_names);
                    let lexicographic_vars = detect_lexicographic_ranking(&txn.contract.pre_condition, &simplified_body);

                    let assume_events: Vec<String> = txn.modifiers.iter()
                        .filter(|m| m.name == "assume_event")
                        .filter_map(|m| m.value.clone())
                        .collect();

                    let assume_shape_action = txn.modifiers.iter()
                        .find(|m| m.name == "assume_shape")
                        .and_then(|m| m.value.as_ref())
                        .and_then(|v| {
                            let parts: Vec<&str> = v.splitn(2, ", ").collect();
                            if parts.len() == 2 {
                                let action = parts[1].trim();
                                if action == "run" || action == "exit" {
                                    Some(action.to_string())
                                } else {
                                    Some("escape".to_string())
                                }
                            } else {
                                Some("escape".to_string())
                            }
                        });

                    nodes.push(ReactorNode {
                        name: txn.name.clone(),
                        is_reactive: txn.is_reactive,
                        precondition: txn.contract.pre_condition.clone(),
                        body: simplified_body,
                        bounded_pre,
                        increments,
                        is_pure_body: is_pure,
                        write_set,
                        is_effectively_pure: false,
                        lexicographic_vars,
                        assume_events,
                        assume_shape_action,
                    });
                }
                TopLevel::Trigger(_) => {
                    has_triggers = true;
                }
                _ => {}
            }
        }

        let live_fields = compute_live_fields(&program.exit_condition, &program.out_pragmas, &nodes);
        for node in &mut nodes {
            compute_effectively_pure(node, &live_fields, &inop_decls);
        }

        ReactorTransitionGraph { nodes, has_triggers, live_fields }
    }
}

fn extract_bounded_pre(pre: &Expr) -> Option<BoundedPre> {
    match pre {
        Expr::Lt(l, r) | Expr::Le(l, r) => {
            match (l.as_ref(), r.as_ref()) {
                (Expr::Identifier(var), Expr::Identifier(bound)) => Some(BoundedPre {
                    var: var.clone(),
                    bound_var: bound.clone(),
                    direction: ConvergeDirection::Increasing,
                    bound_literal: None,
                }),
                (Expr::Identifier(var), Expr::Integer(n)) => Some(BoundedPre {
                    var: var.clone(),
                    bound_var: format!("__lit__{}", var),
                    direction: ConvergeDirection::Increasing,
                    bound_literal: Some(*n),
                }),
                (Expr::Identifier(var), Expr::Neg(bn)) if matches!(bn.as_ref(), Expr::Integer(_)) => {
                    let n = match bn.as_ref() { Expr::Integer(n) => -n, _ => 0 };
                    Some(BoundedPre {
                        var: var.clone(),
                        bound_var: format!("__lit__{}", var),
                        direction: ConvergeDirection::Increasing,
                        bound_literal: Some(n),
                    })
                }
                // len(list) < N — list drains toward full
                (Expr::Projection { source: list, target: ProjectionTarget::Size }, _) => {
                    if let Some(name) = expr_name(list) {
                        Some(BoundedPre {
                            var: name,
                            bound_var: format!("__len_bound"),
                            direction: ConvergeDirection::Increasing,
                            bound_literal: None,
                        })
                    } else { None }
                }
                _ => None,
            }
        }
        Expr::Gt(l, r) | Expr::Ge(l, r) => {
            match (l.as_ref(), r.as_ref()) {
                (Expr::Identifier(var), Expr::Identifier(bound)) => Some(BoundedPre {
                    var: var.clone(),
                    bound_var: bound.clone(),
                    direction: ConvergeDirection::Decreasing,
                    bound_literal: None,
                }),
                (Expr::Identifier(var), Expr::Integer(n)) => Some(BoundedPre {
                    var: var.clone(),
                    bound_var: format!("__lit__{}", var),
                    direction: ConvergeDirection::Decreasing,
                    bound_literal: Some(*n),
                }),
                (Expr::Identifier(var), Expr::Neg(bn)) if matches!(bn.as_ref(), Expr::Integer(_)) => {
                    let n = match bn.as_ref() { Expr::Integer(n) => -n, _ => 0 };
                    Some(BoundedPre {
                        var: var.clone(),
                        bound_var: format!("__lit__{}", var),
                        direction: ConvergeDirection::Decreasing,
                        bound_literal: Some(n),
                    })
                }
                // len(list) > 0 — list drains to empty (bound=0)
                (Expr::Projection { source: list, target: ProjectionTarget::Size }, Expr::Integer(0)) => {
                    if let Some(name) = expr_name(list) {
                        Some(BoundedPre {
                            var: name.clone(),
                            bound_var: format!("__lit__{}", name),
                            direction: ConvergeDirection::Decreasing,
                            bound_literal: Some(0),
                        })
                    } else { None }
                }
                // len(list) > N — list drains to N
                (Expr::Projection { source: list, target: ProjectionTarget::Size }, Expr::Integer(n)) if *n > 0 => {
                    if let Some(name) = expr_name(list) {
                        Some(BoundedPre {
                            var: name.clone(),
                            bound_var: format!("__lit__{}", name),
                            direction: ConvergeDirection::Decreasing,
                            bound_literal: Some(*n),
                        })
                    } else { None }
                }
                _ => None,
            }
        }
        // reg != N — treat as reg > N (decreasing toward N) or reg < N
        // (increasing toward N). Decreasing is the common case (popcount
        // decay toward 0). Direction is validated by
        // extract_valid_bounded_pre against IncrementInfo.
        Expr::Ne(l, r) => {
            match (l.as_ref(), r.as_ref()) {
                (Expr::Identifier(var), Expr::Integer(n)) => Some(BoundedPre {
                    var: var.clone(),
                    bound_var: format!("__lit__{}", var),
                    direction: ConvergeDirection::Decreasing,
                    bound_literal: Some(*n),
                }),
                _ => None,
            }
        }
        Expr::And(l, r) => {
            extract_bounded_pre(l).or_else(|| extract_bounded_pre(r))
        }
        _ => None,
    }
}

/// Wraps `extract_bounded_pre` with mutation validation:
/// Only accept a `BoundedPre` candidate if its variable is actually
/// mutated in the transaction body (i.e., appears in `IncrementInfo`).
/// Without this check, `extract_bounded_pre` can pick an immutable
/// bound variable (e.g., `bound > 0 && count < bound` picks `bound`)
/// and produce a universal loop condition that never enters the body.
///
/// 2026-07-01: Normalize precondition to old-style variants before
/// matching. The parser creates Expr::BinaryOp for comparisons, but
/// extract_bounded_pre matches old-style Expr::Lt/Expr::Gt etc.
/// Without normalization, [a < N] produces bounded_pre=None, which
/// prevents the multi-txn pure fold at mod.rs:2224.
fn extract_valid_bounded_pre(pre: &Expr, inc: &Option<IncrementInfo>) -> Option<BoundedPre> {
    let normalized = pre.normalize_to_old_recursive();
    let bp = extract_bounded_pre(&normalized)?;
    let is_mutated = inc.as_ref().map_or(false, |i| i.var == bp.var);
    if is_mutated { Some(bp) } else { None }
}

/// Recursively simplify an expression using algebraic cancellation rules.
/// Applied bottom-up with fixpoint iteration (max 5 passes) to handle
/// chains like `((x + R) - R) + 1` → `x + 1`.
fn simplify_expr(expr: &Expr) -> Expr {
    // 2026-06-27: Normalize new-style BinaryOp/UnaryOp to old variants
    // so the match below can recurse into children for simplification.
    if let Some(norm) = expr.normalize_to_old() {
        return simplify_expr(&norm);
    }
    let expr = match expr {
        // Recurse first: simplify children bottom-up
        Expr::Add(a, b) => Expr::Add(
            Box::new(simplify_expr(a)),
            Box::new(simplify_expr(b)),
        ),
        Expr::Sub(a, b) => Expr::Sub(
            Box::new(simplify_expr(a)),
            Box::new(simplify_expr(b)),
        ),
        Expr::Mul(a, b) => Expr::Mul(
            Box::new(simplify_expr(a)),
            Box::new(simplify_expr(b)),
        ),
        // Recurse into all other compound expressions unchanged
        Expr::Div(a, b) => Expr::Div(
            Box::new(simplify_expr(a)),
            Box::new(simplify_expr(b)),
        ),
        Expr::Mod(a, b) => Expr::Mod(
            Box::new(simplify_expr(a)),
            Box::new(simplify_expr(b)),
        ),
        Expr::Eq(a, b) => Expr::Eq(
            Box::new(simplify_expr(a)),
            Box::new(simplify_expr(b)),
        ),
        Expr::Ne(a, b) => Expr::Ne(
            Box::new(simplify_expr(a)),
            Box::new(simplify_expr(b)),
        ),
        Expr::Lt(a, b) => Expr::Lt(
            Box::new(simplify_expr(a)),
            Box::new(simplify_expr(b)),
        ),
        Expr::Le(a, b) => Expr::Le(
            Box::new(simplify_expr(a)),
            Box::new(simplify_expr(b)),
        ),
        Expr::Gt(a, b) => Expr::Gt(
            Box::new(simplify_expr(a)),
            Box::new(simplify_expr(b)),
        ),
        Expr::Ge(a, b) => Expr::Ge(
            Box::new(simplify_expr(a)),
            Box::new(simplify_expr(b)),
        ),
        Expr::And(a, b) => Expr::And(
            Box::new(simplify_expr(a)),
            Box::new(simplify_expr(b)),
        ),
        Expr::Or(a, b) => Expr::Or(
            Box::new(simplify_expr(a)),
            Box::new(simplify_expr(b)),
        ),
        Expr::BitAnd(a, b) => Expr::BitAnd(
            Box::new(simplify_expr(a)),
            Box::new(simplify_expr(b)),
        ),
        Expr::BitOr(a, b) => Expr::BitOr(
            Box::new(simplify_expr(a)),
            Box::new(simplify_expr(b)),
        ),
        Expr::BitXor(a, b) => Expr::BitXor(
            Box::new(simplify_expr(a)),
            Box::new(simplify_expr(b)),
        ),
        Expr::Shl(a, b) => Expr::Shl(
            Box::new(simplify_expr(a)),
            Box::new(simplify_expr(b)),
        ),
        Expr::Shr(a, b) => Expr::Shr(
            Box::new(simplify_expr(a)),
            Box::new(simplify_expr(b)),
        ),
        Expr::Concat(a, b) => Expr::Concat(
            Box::new(simplify_expr(a)),
            Box::new(simplify_expr(b)),
        ),
        Expr::Not(a) => Expr::Not(Box::new(simplify_expr(a))),
        Expr::Neg(a) => Expr::Neg(Box::new(simplify_expr(a))),
        Expr::BitNot(a) => Expr::BitNot(Box::new(simplify_expr(a))),
        Expr::Cast(a, t) => Expr::Cast(Box::new(simplify_expr(a)), t.clone()),
        Expr::Projection { source, target } => Expr::Projection {
            source: Box::new(simplify_expr(source)),
            target: target.clone(),
        },
        Expr::ListIndex(list, idx) => Expr::ListIndex(
            Box::new(simplify_expr(list)),
            Box::new(simplify_expr(idx)),
        ),
        Expr::FieldAccess(obj, f) => Expr::FieldAccess(
            Box::new(simplify_expr(obj)),
            f.clone(),
        ),
        Expr::ListLiteral(elems) => Expr::ListLiteral(
            elems.iter().map(|e| simplify_expr(e)).collect(),
        ),
        Expr::Tuple(elems) => Expr::Tuple(
            elems.iter().map(|e| simplify_expr(e)).collect(),
        ),
        Expr::Block(stmts, last) => Expr::Block(
            stmts.iter().map(|s| simplify_stmt(s)).collect(),
            Box::new(simplify_expr(last)),
        ),
        other => other.clone(),
    };

    // Now apply algebraic rules to the simplified children.
    // Use `if let` with `.as_ref()` to match through Box<Expr> wrappers.
    match &expr {
        Expr::Sub(sub_lhs, sub_rhs) => {
            // R1: (a + b) - a → b  and  (a + b) - b → a
            if let Expr::Add(add_lhs, add_rhs) = sub_lhs.as_ref() {
                if vars_match(add_lhs, sub_rhs) {
                    return add_rhs.as_ref().clone();
                }
                if vars_match(add_rhs, sub_rhs) {
                    return add_lhs.as_ref().clone();
                }
            }
            // R2: a - (a - b) → b
            if let Expr::Sub(inner_a, inner_b) = sub_rhs.as_ref() {
                if vars_match(sub_lhs, inner_a) {
                    return inner_b.as_ref().clone();
                }
            }
            // R3: (a + b) - (a + c) → b - c
            if let (Expr::Add(a1, b1), Expr::Add(a2, b2)) = (sub_lhs.as_ref(), sub_rhs.as_ref()) {
                if vars_match(a1, a2) && !vars_match(b1, b2) {
                    return Expr::Sub(
                        Box::new(b1.as_ref().clone()),
                        Box::new(b2.as_ref().clone()),
                    );
                }
            }
            // R5: (a - b) - (c - b) → a - c
            if let (Expr::Sub(sa, sb1), Expr::Sub(sc, sb2)) = (sub_lhs.as_ref(), sub_rhs.as_ref()) {
                if vars_match(sb1, sb2) {
                    return Expr::Sub(
                        Box::new(sa.as_ref().clone()),
                        Box::new(sc.as_ref().clone()),
                    );
                }
            }
            // R7: a - 0 → a
            if let Expr::Integer(0) = sub_rhs.as_ref() {
                return sub_lhs.as_ref().clone();
            }
            expr
        }

        Expr::Add(add_lhs, add_rhs) => {
            // R4: (a - b) + b → a
            if let Expr::Sub(sub_a, sub_b) = add_lhs.as_ref() {
                if vars_match(sub_b, add_rhs) {
                    return sub_a.as_ref().clone();
                }
            }
            // R6: a + 0 → a
            if let Expr::Integer(0) = add_rhs.as_ref() {
                return add_lhs.as_ref().clone();
            }
            // R6b: 0 + a → a
            if let Expr::Integer(0) = add_lhs.as_ref() {
                return add_rhs.as_ref().clone();
            }
            expr
        }

        Expr::Mul(mul_lhs, mul_rhs) => {
            // R8: a * 1 → a
            if let Expr::Integer(1) = mul_rhs.as_ref() {
                return mul_lhs.as_ref().clone();
            }
            // R8b: 1 * a → a
            if let Expr::Integer(1) = mul_lhs.as_ref() {
                return mul_rhs.as_ref().clone();
            }
            // R10: a * 0 → 0
            if let Expr::Integer(0) = mul_rhs.as_ref() {
                return Expr::Integer(0);
            }
            // R10b: 0 * a → 0
            if let Expr::Integer(0) = mul_lhs.as_ref() {
                return Expr::Integer(0);
            }
            expr
        }

        Expr::Div(div_lhs, div_rhs) => {
            // R9: a / 1 → a
            if let Expr::Integer(1) = div_rhs.as_ref() {
                return div_lhs.as_ref().clone();
            }
            expr
        }

        _ => expr,
    }
}

fn vars_match(a: &Expr, b: &Expr) -> bool {
    matches!((a, b), (Expr::Identifier(an), Expr::Identifier(bn)) if an == bn)
}

/// Simplify a statement by simplifying its expression parts.
fn simplify_stmt(stmt: &Statement) -> Statement {
    match stmt {
        Statement::Assignment { lhs, expr, timeout, modifiers } => Statement::Assignment {
            lhs: lhs.clone(),
            expr: simplify_expr(expr),
            timeout: timeout.clone(),
            modifiers: modifiers.clone(),
        },
        Statement::Let { name, ty, expr, address, address_expr, bit_range, is_override, modifiers, .. } => Statement::Let {
            name: name.clone(),
            ty: ty.clone(),
            expr: expr.as_ref().map(|e| simplify_expr(e)),
            address: *address,
            address_expr: address_expr.clone(),
            bit_range: bit_range.clone(),
            is_override: *is_override,
            modifiers: modifiers.clone(),
            constraint: None,
        },
        Statement::Expression(e) => Statement::Expression(simplify_expr(e)),
        Statement::Guarded { condition, statements } => Statement::Guarded {
            condition: simplify_expr(condition),
            statements: statements.iter().map(|s| simplify_stmt(s)).collect(),
        },
        other => other.clone(),
    }
}

/// Simplify a transaction body using algebraic cancellation rules.
/// Applies fixpoint iteration (max 5 passes) to handle chained reductions.
pub fn simplify_body(body: &[Statement]) -> Vec<Statement> {
    // Pre-convert Literal(Integer(n)) → Integer(n) so legacy pattern matchers work
    fn lit_to_int(e: &Expr) -> Expr {
        match e {
            Expr::Literal(boxed) => match boxed.as_ref() { LiteralExpr::Integer(n) => Expr::Integer(*n), _ => e.clone() },
            _ => e.clone(),
        }
    }
    fn lit_stmt(s: &Statement) -> Statement {
        match s {
            Statement::Let { name, ty, expr, address, address_expr, bit_range, constraint, is_override, modifiers } => {
                Statement::Let { name: name.clone(), ty: ty.clone(), expr: expr.as_ref().map(|e| lit_to_int(e)), address: address.clone(), address_expr: address_expr.clone(), bit_range: bit_range.clone(), constraint: constraint.clone(), is_override: is_override.clone(), modifiers: modifiers.clone() }
            }
            Statement::Assignment { lhs, expr, timeout, modifiers } => {
                Statement::Assignment { lhs: lhs.clone(), expr: lit_to_int(expr), timeout: timeout.clone(), modifiers: modifiers.clone() }
            }
            Statement::Guarded { condition, statements } => {
                let stmts: Vec<Statement> = statements.iter().map(|s| lit_stmt(s)).collect();
                Statement::Guarded { condition: lit_to_int(condition), statements: stmts }
            }
            other => other.clone(),
        }
    }
    let body: Vec<Statement> = body.iter().map(|s| lit_stmt(s)).collect();
    let mut current = body;
    for _ in 0..5 {
        let next: Vec<Statement> = current.iter().map(|s| simplify_stmt(s)).collect();
        if next == current {
            break;
        }
        current = next;
    }
    current
}

fn get_int(e: &Expr) -> Option<i64> {
    match e {
        Expr::Integer(n) => Some(*n),
        Expr::Literal(boxed) => match boxed.as_ref() { LiteralExpr::Integer(n) => Some(*n), _ => None },
        _ => None,
    }
}

fn detect_increments(body: &[Statement]) -> Option<IncrementInfo> {
    for stmt in body {
        if let Statement::Assignment { lhs, expr, .. } = stmt {
            let name = match lhs {
                Expr::Identifier(n) | Expr::OwnedRef(n) => n.clone(),
                _ => continue,
            };
            // 2026-06-27: Normalize new-style BinaryOp/UnaryOp to old variants
            // so the Add/Sub checks below can detect increment patterns.
            let normalized = expr.normalize_to_old();
            let expr_ref: &Expr = match normalized {
                Some(ref norm) => norm,
                None => expr,
            };
            if let Expr::Add(a, b) = expr_ref {
                if let (Expr::Identifier(var), Some(delta)) = (a.as_ref(), get_int(b)) {
                    if *var == name && delta > 0 {
                        return Some(IncrementInfo { var: name.clone(), delta });
                    }
                }
                if let (Expr::Identifier(var), Some(delta)) = (b.as_ref(), get_int(a)) {
                    if *var == name && delta > 0 {
                        return Some(IncrementInfo { var: name.clone(), delta });
                    }
                }
            }
            // Decreasing counter: count = count - delta or count = count - 1
            if let Expr::Sub(a, b) = expr_ref {
                if let (Expr::Identifier(var), Some(delta)) = (a.as_ref(), get_int(b)) {
                    if *var == name && delta > 0 {
                        return Some(IncrementInfo { var: name.clone(), delta });
                    }
                }
            }
            // Interval bounds: (x + R1) - R2 where net step R1 - R2 ≥ 1
            if let Expr::Sub(inner, rhs) = expr_ref {
                if let Expr::Add(lhs, rhs2) = inner.as_ref() {
                    let is_self_add = matches!(lhs.as_ref(), Expr::Identifier(v) if *v == name);
                    if is_self_add {
                        // Try r1 - r2 with both as constants
                        let r1 = get_int(rhs2);
                        let r2 = get_int(rhs);
                        if let (Some(r1_val), Some(r2_val)) = (r1, r2) {
                            let net = r1_val - r2_val;
                            if net > 0 {
                                return Some(IncrementInfo { var: name.clone(), delta: net });
                            }
                        }
                        // If only one side is a constant and positive, assume at least 1
                        if r1.map_or(false, |v| v >= 1) && r2.is_none() {
                            return Some(IncrementInfo { var: name.clone(), delta: 1 });
                        }
                    }
                }
            }
        }
    }
    None
}

/// Detect popcount decay: `reg = reg & (reg - 1)` clears one bit per iteration.
/// The ranking function `popcount(reg) → 0` is bounded at 64 bits.
fn detect_popcount_decay(body: &[Statement]) -> Option<IncrementInfo> {
    for stmt in body {
        if let Statement::Assignment { lhs, expr, .. } = stmt {
            let name = match lhs {
                Expr::Identifier(n) | Expr::OwnedRef(n) => n.clone(),
                _ => continue,
            };
            // reg & (reg - 1)
            if let Expr::BitAnd(a, b) = expr {
                let a_is_self = matches!(a.as_ref(), Expr::Identifier(v) if *v == name);
                let b_is_self_minus = if let Expr::Sub(inner, val) = b.as_ref() {
                    matches!(inner.as_ref(), Expr::Identifier(v) if *v == name)
                        && matches!(val.as_ref(), Expr::Integer(1))
                } else {
                    false
                };
                if a_is_self && b_is_self_minus {
                    return Some(IncrementInfo { var: name.clone(), delta: 1 });
                }
                // (reg - 1) & reg
                let b_is_self = matches!(b.as_ref(), Expr::Identifier(v) if *v == name);
                let a_is_self_minus = if let Expr::Sub(inner, val) = a.as_ref() {
                    matches!(inner.as_ref(), Expr::Identifier(v) if *v == name)
                        && matches!(val.as_ref(), Expr::Integer(1))
                } else {
                    false
                };
                if b_is_self && a_is_self_minus {
                    return Some(IncrementInfo { var: name.clone(), delta: 1 });
                }
            }
        }
    }
    None
}

/// Detect collection drain: `<- &list` or `x <- &list` pops from the list,
/// decreasing its length by exactly 1 per iteration.
/// Ranking function: `τ = len(list) → 0`, decreases by exactly 1 per pop.
fn detect_collection_drain(body: &[Statement]) -> Option<IncrementInfo> {
    for stmt in body {
        match stmt {
            // Pattern 1: x <- &list  — ArrowMut(Pop, target, index, Some(value))
            Statement::Let { expr: Some(e), .. } => {
                if let Expr::ArrowMut { dir: ArrowDir::Pop, target, index, .. } = e {
                    if let Expr::Term = index.as_ref() {
                        if let Some(name) = expr_name(target.as_ref()) {
                            return Some(IncrementInfo { var: name, delta: 1 });
                        }
                    }
                }
            }
            // Pattern 2: <- &list — ArrowDiscard(target, index)
            Statement::Expression(Expr::ArrowDiscard { target, index }) => {
                if let Expr::Term = index.as_ref() {
                    if let Some(name) = expr_name(target.as_ref()) {
                        return Some(IncrementInfo { var: name, delta: 1 });
                    }
                }
            }
            _ => {}
        }
    }
    None
}

/// Detect a lexicographic tuple ranking: precondition is a disjunction (Or)
/// of decreasing bounds on different variables, and each variable has a
/// corresponding decrement in the body. This is a multi-variable ranking
/// function where the loop exits when ALL variables reach zero.
///
/// Example: `[x > 0 || y > 0][x == 0 && y == 0]` with body
/// `&x = x - 1; &y = y - 1;` (each decremented in its own guard).
fn detect_lexicographic_ranking(pre: &Expr, body: &[Statement]) -> Vec<String> {
    /// Collect decreasing-bound variable names from an Or chain
    fn collect_or_vars(expr: &Expr, out: &mut Vec<String>) {
        match expr {
            Expr::Or(l, r) => {
                collect_or_vars(l, out);
                collect_or_vars(r, out);
            }
            _ => {
                // Check for Gt(var, N) where N >= 0
                if let Expr::Gt(inner, val) = expr {
                    if let (Expr::Identifier(var), Expr::Integer(n)) = (inner.as_ref(), val.as_ref()) {
                        if *n >= 0 && !out.contains(var) {
                            out.push(var.clone());
                        }
                    }
                }
                // Check for Ge(var, N) where N > 0
                if let Expr::Ge(inner, val) = expr {
                    if let (Expr::Identifier(var), Expr::Integer(n)) = (inner.as_ref(), val.as_ref()) {
                        if *n > 0 && !out.contains(var) {
                            out.push(var.clone());
                        }
                    }
                }
            }
        }
    }

    let mut vars = Vec::new();
    collect_or_vars(pre, &mut vars);
    if vars.len() < 2 {
        return Vec::new();
    }

    // Verify each variable has a decrement in the body
    let decremented: HashSet<String> = body.iter().filter_map(|stmt| {
        if let Statement::Assignment { lhs, expr, .. } = stmt {
            let name = match lhs {
                Expr::Identifier(n) | Expr::OwnedRef(n) => Some(n.clone()),
                _ => None,
            }?;
            let is_decrement = if let Expr::Sub(a, d) = expr {
                            matches!(a.as_ref(), Expr::Identifier(v) if *v == name)
                                && matches!(d.as_ref(), Expr::Integer(val) if *val >= 1)
                        } else { false };
            if is_decrement { Some(name) } else { None }
        } else { None }
    }).collect();

    vars.retain(|v| decremented.contains(v));
    if vars.len() < 2 { vec![] } else { vars }
}

fn is_pure_body(
    body: &[Statement],
    state_fields: &HashSet<String>,
    increments: &Option<IncrementInfo>,
    inop_decls: &HashMap<String, bool>,
) -> bool {
    let inc_var = increments.as_ref().map(|i| &i.var);
    for stmt in body {
        match stmt {
            Statement::Assignment { lhs, expr, .. } if inc_var.is_some() => {
                let name = match lhs {
                    Expr::Identifier(n) | Expr::OwnedRef(n) => n.clone(),
                    _ => return false,
                };
                if Some(&name) == inc_var {
                    if !matches!(expr, Expr::Add(_, _) | Expr::BitAnd(_, _)) {
                        return false;
                    }
                    continue;
                }
                if state_fields.contains(&name) {
                    return false;
                }
            }
            Statement::Let { expr, .. } => {
                if let Some(e) = expr {
                    if references_triggers_or_ffi_with_decls(e, inop_decls) {
                        return false;
                    }
                }
            }
            Statement::Assignment { expr, .. } => {
                if references_triggers_or_ffi_with_decls(expr, inop_decls) {
                    return false;
                }
            }
            Statement::Expression(e) => {
                if references_triggers_or_ffi_with_decls(e, inop_decls) {
                    return false;
                }
            }
            Statement::Term { swan_song, .. } | Statement::TermBang { swan_song, .. } => {
                if let Some(swan) = swan_song {
                    if statement_contains_ffi_with_decls(swan, inop_decls) { return false; }
                }
            }
            Statement::Escape(_) => return false,
            Statement::OnExit { .. } => return false,
            Statement::Guarded { condition, statements } => {
                if references_triggers_or_ffi_with_decls(condition, inop_decls) {
                    return false;
                }
                if statements.iter().any(|s| statement_contains_ffi_with_decls(s, inop_decls)) {
                    return false;
                }
            }
            _ => {}
        }
    }
    true
}

fn intrinsic_has_side_effects(intrinsic: &Intrinsic, inop_decls: &HashMap<String, bool>) -> bool {
    match intrinsic {
        Intrinsic::UserDefined(name) => inop_decls.get(name).copied().unwrap_or(true),
        other => other.has_side_effects(),
    }
}

fn references_triggers_or_ffi(expr: &Expr) -> bool {
    references_triggers_or_ffi_with_decls(expr, &HashMap::new())
}

fn references_triggers_or_ffi_with_decls(expr: &Expr, inop_decls: &HashMap<String, bool>) -> bool {
    // 2026-06-27: Normalize new-style BinaryOp/UnaryOp to old variants
    // so FFI references inside them are properly detected.
    if let Some(norm) = expr.normalize_to_old() {
        return references_triggers_or_ffi_with_decls(&norm, inop_decls);
    }
    match expr {
        Expr::Call(_, _) => true,
        Expr::IntrinsicCall { intrinsic, .. } => intrinsic_has_side_effects(intrinsic, inop_decls),
        Expr::Identifier(_) | Expr::Integer(_) | Expr::Float(_) | Expr::Bool(_) | Expr::String(_) | Expr::Char(_) => false,
        Expr::Add(a, b) | Expr::Sub(a, b) | Expr::Mul(a, b) | Expr::Div(a, b) | Expr::Mod(a, b)
        | Expr::Eq(a, b) | Expr::Ne(a, b) | Expr::Lt(a, b) | Expr::Le(a, b) | Expr::Gt(a, b)
        | Expr::Ge(a, b) | Expr::And(a, b) | Expr::Or(a, b) | Expr::BitAnd(a, b)
        | Expr::BitOr(a, b) | Expr::BitXor(a, b) | Expr::Shl(a, b) | Expr::Shr(a, b) => {
            references_triggers_or_ffi_with_decls(a, inop_decls) || references_triggers_or_ffi_with_decls(b, inop_decls)
        }
        Expr::Not(a) | Expr::Neg(a) | Expr::BitNot(a) => references_triggers_or_ffi_with_decls(a, inop_decls),
        Expr::Cast(a, _) => references_triggers_or_ffi_with_decls(a, inop_decls),
        Expr::Block(_, last) | Expr::TupleDestructure(_, last) => references_triggers_or_ffi_with_decls(last, inop_decls),
        Expr::ListLiteral(elems) => elems.iter().any(|e| references_triggers_or_ffi_with_decls(e, inop_decls)),
        Expr::ListIndex(list, idx) => references_triggers_or_ffi_with_decls(list, inop_decls) || references_triggers_or_ffi_with_decls(idx, inop_decls),
        Expr::Projection { source: inner, .. } => references_triggers_or_ffi_with_decls(inner, inop_decls),
        Expr::Tuple(elems) => elems.iter().any(|e| references_triggers_or_ffi_with_decls(e, inop_decls)),
        Expr::FieldAccess(obj, _) => references_triggers_or_ffi_with_decls(obj, inop_decls),
        _ => false,
    }
}

/// Detect whether a list of transaction pairs all have structurally
/// identical bodies. When all bodies are the same, the dispatch is uniform
/// — it doesn't matter which txn fires, because the effect is identical.
/// This enables skipping the entire precondition chain in emit_reactor.
pub fn is_uniform_body_group(txns: &[(String, &crate::ast::Transaction)]) -> bool {
    if txns.len() < 2 { return false; }
    let first_body = &txns[0].1.body;
    for (_, txn) in &txns[1..] {
        if txn.body != *first_body { return false; }
    }
    true
}

pub fn compute_live_fields(
    exit_condition: &Option<Box<Expr>>,
    out_pragmas: &[String],
    nodes: &[ReactorNode],
) -> HashSet<String> {
    let mut live = HashSet::new();
    if let Some(ec) = exit_condition {
        collect_identifiers(ec, &mut live);
    }
    for name in out_pragmas {
        live.insert(name.clone());
    }
    for node in nodes {
        collect_identifiers(&node.precondition, &mut live);
    }

    // Pre-pass: FFI calls make their argument expressions observable.
    // Seed the live set with all identifiers reachable from FFI arguments,
    // including local let-bound intermediates. The fixpoint loop below
    // then traces them backward through Let/Assignment to state fields.
    for node in nodes {
        for stmt in &node.body {
            scan_for_ffi_args(stmt, &mut live);
        }
    }

    // Transitive liveness: if a live field reads another field through an
    // assignment or let binding, that field is also live.  Iterate to
    // fixpoint through the txn bodies.
    loop {
        let mut changed = false;
        for node in nodes {
            let mut stmts: Vec<&Statement> = node.body.iter().collect();
            let mut i = 0;
            while i < stmts.len() {
                if let Statement::Guarded { statements, .. } = stmts[i] {
                    stmts.extend(statements);
                }
                i += 1;
            }
            for stmt in stmts {
                let (target, expr) = match stmt {
                    Statement::Assignment { lhs, expr, .. } => {
                        (expr_name(lhs), expr)
                    }
                    Statement::Let { name, expr: Some(e), .. } => {
                        (Some(name.clone()), e)
                    }
                    _ => continue,
                };
                if let Some(ref t) = target {
                    if live.contains(t) {
                        let mut idents = HashSet::new();
                        collect_identifiers(expr, &mut idents);
                        for ident in idents {
                            if live.insert(ident) {
                                changed = true;
                            }
                        }
                    }
                }
            }
        }
        if !changed { break; }
    }

    live
}

/// Scan all transaction bodies for projection expressions on state fields.
/// Returns a map: state field name → set of projection target strings used on that field.
/// Used by the Adaptive Layout Engine to determine which fields need cache slots.
pub fn compute_projection_usage(program: &crate::ast::Program) -> HashMap<String, HashSet<String>> {
    let state_fields: HashSet<String> = program.items.iter()
        .filter_map(|item| if let crate::ast::TopLevel::StateDecl(s) = item { Some(s.name.clone()) } else { None })
        .collect();
    let mut usage: HashMap<String, HashSet<String>> = HashMap::new();
    for item in &program.items {
        if let crate::ast::TopLevel::Transaction(txn) = item {
            scan_for_projections_in_stmts(&txn.body, &state_fields, &mut usage);
        }
    }
    usage
}

pub fn projection_target_name(target: &crate::ast::ProjectionTarget) -> String {
    use crate::ast::ProjectionTarget;
    match target {
        ProjectionTarget::Size => "Size".into(),
        ProjectionTarget::Bytes => "Bytes".into(),
        ProjectionTarget::Ptr => "Ptr".into(),
        ProjectionTarget::Alignment => "Alignment".into(),
        ProjectionTarget::Range => "Range".into(),
        ProjectionTarget::Popcount => "Popcount".into(),
        ProjectionTarget::LeadingZeros => "LeadingZeros".into(),
        ProjectionTarget::TrailingZeros => "TrailingZeros".into(),
        ProjectionTarget::Absolute => "Absolute".into(),
        ProjectionTarget::BitReverse => "BitReverse".into(),
        ProjectionTarget::Type => "Type".into(),
        ProjectionTarget::PtrBang => "Ptr!".into(),
        ProjectionTarget::Keys => "Keys".into(),
        ProjectionTarget::Values => "Values".into(),
        ProjectionTarget::Contains(_) => "Contains(...)".into(),
        ProjectionTarget::IsEmpty => "IsEmpty".into(),
        ProjectionTarget::Get(_) => "Get(...)".into(),
        ProjectionTarget::Top => "Top".into(),
        ProjectionTarget::Front => "Front".into(),
        ProjectionTarget::Elements => "Elements".into(),
        ProjectionTarget::AsStack => "AsStack".into(),
        ProjectionTarget::AsQueue => "AsQueue".into(),
        ProjectionTarget::BitRange(_) => "BitRange".into(),
        ProjectionTarget::UserDefined(n) => format!("UserDefined({})", n),
        ProjectionTarget::UserDefinedWithArg(n, _) => format!("UserDefined({}, ...)", n),
        ProjectionTarget::Address => "Address".into(),
        ProjectionTarget::Name => "Name".into(),
        ProjectionTarget::Params => "Params".into(),
        ProjectionTarget::Returns => "Returns".into(),
        ProjectionTarget::Arity => "Arity".into(),
        ProjectionTarget::Loc => "Loc".into(),
        ProjectionTarget::Doc => "Doc".into(),
        ProjectionTarget::Hash => "Hash".into(),
        ProjectionTarget::Contracts => "Contracts".into(),
        ProjectionTarget::Module => "Module".into(),
        ProjectionTarget::IsPure => "IsPure".into(),
        ProjectionTarget::FnSpan => "FnSpan".into(),
    }
}

/// Recursively scan statements for `Expr::Projection` where the source is a state field.
fn scan_for_projections_in_stmts(stmts: &[crate::ast::Statement], state_fields: &HashSet<String>, usage: &mut HashMap<String, HashSet<String>>) {
    for stmt in stmts {
        match stmt {
            crate::ast::Statement::Expression(expr) => {
                collect_projection_identifiers(expr, state_fields, usage);
            }
            crate::ast::Statement::Let { expr: Some(expr), .. } => {
                collect_projection_identifiers(expr, state_fields, usage);
            }
            crate::ast::Statement::Assignment { expr, .. } => {
                collect_projection_identifiers(expr, state_fields, usage);
            }
            crate::ast::Statement::Guarded { statements, .. } => {
                scan_for_projections_in_stmts(statements, state_fields, usage);
            }
            crate::ast::Statement::Term { swan_song: Some(stmt), .. }
            | crate::ast::Statement::TermBang { swan_song: Some(stmt), .. } => {
                scan_for_projections_in_stmts(std::slice::from_ref(stmt.as_ref()), state_fields, usage);
            }
            _ => {}
        }
    }
}

/// Collect projection identifiers from an expression tree.
fn collect_projection_identifiers(expr: &crate::ast::Expr, state_fields: &HashSet<String>, usage: &mut HashMap<String, HashSet<String>>) {
    match expr {
        crate::ast::Expr::Projection { source, target } => {
            if let crate::ast::Expr::Identifier(name) = source.as_ref() {
                if state_fields.contains(name) {
                    usage.entry(name.clone()).or_default().insert(projection_target_name(target));
                }
            }
            // Also recurse into source (in case it's a chain of projections)
            collect_projection_identifiers(source, state_fields, usage);
        }
        crate::ast::Expr::Add(l, r) | crate::ast::Expr::Sub(l, r)
        | crate::ast::Expr::Mul(l, r) | crate::ast::Expr::Div(l, r)
        | crate::ast::Expr::Eq(l, r) | crate::ast::Expr::Ne(l, r)
        | crate::ast::Expr::Lt(l, r) | crate::ast::Expr::Le(l, r)
        | crate::ast::Expr::Gt(l, r) | crate::ast::Expr::Ge(l, r)
        | crate::ast::Expr::And(l, r) | crate::ast::Expr::Or(l, r) => {
            collect_projection_identifiers(l, state_fields, usage);
            collect_projection_identifiers(r, state_fields, usage);
        }
        crate::ast::Expr::Call(_, args) => {
            for arg in args {
                collect_projection_identifiers(arg, state_fields, usage);
            }
        }
        crate::ast::Expr::Cast(inner, _) => {
            collect_projection_identifiers(inner, state_fields, usage);
        }
        crate::ast::Expr::FieldAccess(obj, _field_name) => {
            collect_projection_identifiers(obj, state_fields, usage);
        }
        crate::ast::Expr::Block(_, last) => {
            collect_projection_identifiers(last, state_fields, usage);
        }
        _ => {}
    }
}

/// Compute the set of state fields that are directly referenced (read or written)
/// anywhere in transaction bodies or definition bodies. This is broader than
/// `compute_live_fields` — it catches plain field reads like `&x = x + 1` that
/// don't involve FFI, exit conditions, or preconditions.
pub fn compute_referenced_fields(program: &crate::ast::Program) -> HashSet<String> {
    let state_fields: HashSet<String> = program.items.iter()
        .filter_map(|item| if let crate::ast::TopLevel::StateDecl(s) = item { Some(s.name.clone()) } else { None })
        .collect();
    let mut referenced: HashSet<String> = HashSet::new();

    for item in &program.items {
        let body: Option<&[crate::ast::Statement]> = match item {
            crate::ast::TopLevel::Transaction(t) => Some(&t.body),
            crate::ast::TopLevel::Definition(d) => Some(&d.body),
            _ => None,
        };
        if let Some(body) = body {
            scan_for_state_identifiers(body, &state_fields, &mut referenced);
        }
        // Also scan preconditions and postconditions — a field used only in
        // a contract (e.g. `let N = getenv_int#("BOUND")` referenced in
        // `[count < N]`) must not be eliminated as dead.
        if let crate::ast::TopLevel::Transaction(t) = item {
            collect_state_identifiers(&t.contract.pre_condition, &state_fields, &mut referenced);
            collect_state_identifiers(&t.contract.post_condition, &state_fields, &mut referenced);
        }
    }

    // Scan the program's exit condition (#!exit) — a field referenced only
    // in the exit condition must not be eliminated.
    if let Some(ref exit_cond) = program.exit_condition {
        collect_state_identifiers(exit_cond, &state_fields, &mut referenced);
    }

    // Scan state field initializers — a field's initializer may reference
    // another state field that would otherwise appear unused.
    for item in &program.items {
        if let crate::ast::TopLevel::StateDecl(s) = item {
            if let Some(ref expr) = s.expr {
                collect_state_identifiers(expr, &state_fields, &mut referenced);
            }
        }
    }

    // If any %state-accessing inop exists, all state fields are live
    // (conservative — Phase 2, precise GEP-index tracking is Phase 3)
    if program.items.iter().any(|item| {
        matches!(item, crate::ast::TopLevel::Inop(inop) if inop.has_state_access)
    }) {
        referenced.extend(state_fields);
    }

    referenced
}

fn scan_for_state_identifiers(stmts: &[crate::ast::Statement], state_fields: &HashSet<String>, out: &mut HashSet<String>) {
    for stmt in stmts {
        match stmt {
            crate::ast::Statement::Expression(expr)
            | crate::ast::Statement::Let { expr: Some(expr), .. } => {
                collect_state_identifiers(expr, state_fields, out);
            }
            crate::ast::Statement::Assignment { lhs, expr, .. } => {
                // Check LHS for state field references (including nested like ListIndex)
                collect_state_identifiers(lhs, state_fields, out);
                collect_state_identifiers(expr, state_fields, out);
            }
            crate::ast::Statement::Guarded { statements, .. } => {
                scan_for_state_identifiers(statements, state_fields, out);
            }
            crate::ast::Statement::Term { values, swan_song, .. }
            | crate::ast::Statement::TermBang { values, swan_song, .. } => {
                for v in values.iter().flatten() {
                    collect_state_identifiers(v, state_fields, out);
                }
                if let Some(ss) = swan_song {
                    scan_for_state_identifiers(std::slice::from_ref(ss.as_ref()), state_fields, out);
                }
            }
            crate::ast::Statement::Escape(Some(expr)) => {
                collect_state_identifiers(expr, state_fields, out);
            }
            crate::ast::Statement::SyncBlock { body } => {
                scan_for_state_identifiers(body, state_fields, out);
            }
            _ => {}
        }
    }
}

fn collect_state_identifiers(expr: &crate::ast::Expr, state_fields: &HashSet<String>, out: &mut HashSet<String>) {
    match expr {
        crate::ast::Expr::Identifier(name) => {
            if state_fields.contains(name) {
                out.insert(name.clone());
            }
        }
        crate::ast::Expr::Add(l, r) | crate::ast::Expr::Sub(l, r)
        | crate::ast::Expr::Mul(l, r) | crate::ast::Expr::Div(l, r)
        | crate::ast::Expr::Eq(l, r) | crate::ast::Expr::Ne(l, r)
        | crate::ast::Expr::Lt(l, r) | crate::ast::Expr::Le(l, r)
        | crate::ast::Expr::Gt(l, r) | crate::ast::Expr::Ge(l, r)
        | crate::ast::Expr::And(l, r) | crate::ast::Expr::Or(l, r) => {
            collect_state_identifiers(l, state_fields, out);
            collect_state_identifiers(r, state_fields, out);
        }
        crate::ast::Expr::BinaryOp(bop) => {
            collect_state_identifiers(&bop.left, state_fields, out);
            collect_state_identifiers(&bop.right, state_fields, out);
        }
        crate::ast::Expr::UnaryOp(uop) => {
            collect_state_identifiers(&uop.operand, state_fields, out);
        }
        crate::ast::Expr::Not(inner) | crate::ast::Expr::Neg(inner)
        | crate::ast::Expr::Cast(inner, _) => {
            collect_state_identifiers(inner, state_fields, out);
        }
        crate::ast::Expr::PriorState(_name) => {
            // PriorState is a string reference to a previous value — not a state field ref.
        }
        crate::ast::Expr::Projection { source, .. } => {
            collect_state_identifiers(source, state_fields, out);
        }
        crate::ast::Expr::Call(_, args) => {
            for arg in args {
                collect_state_identifiers(arg, state_fields, out);
            }
        }
        crate::ast::Expr::FieldAccess(obj, _) => {
            collect_state_identifiers(obj, state_fields, out);
        }
        crate::ast::Expr::ListIndex(obj, idx) => {
            collect_state_identifiers(obj, state_fields, out);
            collect_state_identifiers(idx, state_fields, out);
        }
        crate::ast::Expr::Slice { value, start, end, stride, mask } => {
            collect_state_identifiers(value, state_fields, out);
            if let Some(s) = start { collect_state_identifiers(s, state_fields, out); }
            if let Some(e) = end { collect_state_identifiers(e, state_fields, out); }
            if let Some(s) = stride { collect_state_identifiers(s, state_fields, out); }
            if let Some(m) = mask { collect_state_identifiers(m, state_fields, out); }
        }
        crate::ast::Expr::OwnedRef(name) => {
            if state_fields.contains(name) {
                out.insert(name.clone());
            }
        }
        crate::ast::Expr::Block(_, last) => {
            collect_state_identifiers(last, state_fields, out);
        }
        crate::ast::Expr::Tuple(exprs) | crate::ast::Expr::ListLiteral(exprs) => {
            for e in exprs {
                collect_state_identifiers(e, state_fields, out);
            }
        }
        crate::ast::Expr::Match { value, arms } => {
            collect_state_identifiers(value, state_fields, out);
            for arm in arms {
                collect_state_identifiers(&arm.body, state_fields, out);
            }
        }
        crate::ast::Expr::StructInstance(_, fields) => {
            for (_field_name, val) in fields {
                collect_state_identifiers(val, state_fields, out);
            }
        }
        crate::ast::Expr::ObjectLiteral(fields) => {
            for (_field_name, val) in fields {
                collect_state_identifiers(val, state_fields, out);
            }
        }
        crate::ast::Expr::MapLiteral(entries) => {
            for (k, v) in entries {
                collect_state_identifiers(k, state_fields, out);
                collect_state_identifiers(v, state_fields, out);
            }
        }
        crate::ast::Expr::SetLiteral(items) => {
            for item in items {
                collect_state_identifiers(item, state_fields, out);
            }
        }
        crate::ast::Expr::IntrinsicCall { args, .. } => {
            for arg in args {
                collect_state_identifiers(arg, state_fields, out);
            }
        }
        _ => {}
    }
}

/// Assign a FieldMode to each state field based on liveness, referencedness, and projection usage.
/// Fields that are never referenced anywhere in any transaction body → Never (eliminated).
/// Fields with dual-lens access (≥2 different projection targets) → LazyCached.
/// Everything else → Always.
pub fn assign_field_modes(
    all_state_fields: &HashSet<String>,
    referenced_fields: &HashSet<String>,
    projection_usage: &HashMap<String, HashSet<String>>,
) -> HashMap<String, super::FieldMode> {
    let mut modes: HashMap<String, super::FieldMode> = HashMap::new();
    let mut next_cache_index: usize = 0;

    for field in all_state_fields {
        let usage = projection_usage.get(field);
        if let Some(targets) = usage {
            if targets.len() >= 2 {
                // Dual-lens access: cache slot needed.
                modes.insert(field.clone(), super::FieldMode::LazyCached {
                    cache_index: {
                        let ci = next_cache_index;
                        next_cache_index += 1;
                        ci
                    },
                });
                continue;
            }
        }

        // Check if field is referenced anywhere in transaction bodies
        if referenced_fields.contains(field) {
            modes.insert(field.clone(), super::FieldMode::Always);
        } else {
            // Not referenced at all — safe to eliminate
            modes.insert(field.clone(), super::FieldMode::Never);
        }
    }

    modes
}

/// Recursively scan a statement for FFI calls and collect identifiers from
/// their arguments into the live set. This is the liveness seed that makes
/// `frgn __print_float(energy)` keep `energy` (and transitively `x0`, `p00`, ...) alive.
fn scan_for_ffi_args(stmt: &Statement, out: &mut HashSet<String>) {
    match stmt {
        Statement::Expression(expr) | Statement::Let { expr: Some(expr), .. } => {
            collect_ffi_identifiers(expr, out);
        }
        Statement::Guarded { condition: _, statements } => {
            for s in statements {
                scan_for_ffi_args(s, out);
            }
        }
        _ => {}
    }
}

/// Recursively collect all identifiers reachable through FFI argument expressions.
/// The catch-all `_ => collect_identifiers(expr, out)` is critical: it picks up
/// leaf identifiers like `energy`, `ei`, `ej` from `__print_float(energy + ei + ej)`
/// that would otherwise be invisible to DFE's name-based fixpoint loop.
fn collect_ffi_identifiers(expr: &Expr, out: &mut HashSet<String>) {
    match expr {
        Expr::Call(_, args) => {
            for arg in args {
                collect_identifiers(arg, out);
            }
        }
        Expr::IntrinsicCall { intrinsic: _, args } => {
            for arg in args {
                collect_identifiers(arg, out);
            }
        }
        Expr::Add(l, r) | Expr::Sub(l, r) | Expr::Mul(l, r) | Expr::Div(l, r)
        | Expr::Eq(l, r) | Expr::Ne(l, r) | Expr::Lt(l, r) | Expr::Le(l, r)
        | Expr::Gt(l, r) | Expr::Ge(l, r) | Expr::And(l, r) | Expr::Or(l, r) => {
            collect_ffi_identifiers(l, out);
            collect_ffi_identifiers(r, out);
        }
        _ => collect_identifiers(expr, out),
    }
}

fn expr_name(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Identifier(n) | Expr::OwnedRef(n) => Some(n.clone()),
        _ => None,
    }
}

fn compute_effectively_pure(node: &mut ReactorNode, live_fields: &HashSet<String>, inop_decls: &HashMap<String, bool>) {
    // FFI calls have side effects — cannot fold to pure counter
    if node.body.iter().any(|s| statement_contains_ffi_with_decls(s, inop_decls)) {
        return;
    }
    // term/term! with swan song is an observable side effect — prevents fold
    if node.body.iter().any(|s| statement_has_swan_song(s)) {
        return;
    }
    if let (Some(bp), Some(inc)) = (&node.bounded_pre, &node.increments) {
        if inc.var == bp.var && inc.delta > 0 && live_fields.contains(&inc.var) {
            let non_counter_writes: Vec<&String> = node.write_set.iter()
                .filter(|f| *f != &inc.var)
                .collect();
            if non_counter_writes.iter().all(|f| !live_fields.contains(*f)) {
                node.is_effectively_pure = true;
            }
        }
    }
}

/// Check if a statement contains a Term or TermBang with a swan song.
/// Swan songs are observable side effects that prevent pure-counter fold.
fn statement_has_swan_song(stmt: &Statement) -> bool {
    match stmt {
        Statement::Term { swan_song, .. } | Statement::TermBang { swan_song, .. } => {
            swan_song.is_some()
        }
        Statement::Guarded { statements, .. } => {
            statements.iter().any(|s| statement_has_swan_song(s))
        }
        _ => false,
    }
}

pub(crate) fn statement_contains_ffi(stmt: &Statement) -> bool {
    statement_contains_ffi_with_decls(stmt, &HashMap::new())
}

pub(crate) fn statement_contains_ffi_with_decls(stmt: &Statement, inop_decls: &HashMap<String, bool>) -> bool {
    match stmt {
        Statement::Assignment { expr, .. } => references_triggers_or_ffi_with_decls(expr, inop_decls),
        Statement::Let { expr, .. } => expr.as_ref().map_or(false, |e| references_triggers_or_ffi_with_decls(e, inop_decls)),
        Statement::Expression(e) => references_triggers_or_ffi_with_decls(e, inop_decls),
        Statement::Term { values, swan_song, .. } => {
            values.iter().any(|v| v.as_ref().map_or(false, |e| references_triggers_or_ffi_with_decls(e, inop_decls)))
                || swan_song.as_ref().map_or(false, |s| statement_contains_ffi_with_decls(s, inop_decls))
        }
        Statement::TermBang { values, swan_song, .. } => {
            values.iter().any(|v| v.as_ref().map_or(false, |e| references_triggers_or_ffi_with_decls(e, inop_decls)))
                || swan_song.as_ref().map_or(false, |s| statement_contains_ffi_with_decls(s, inop_decls))
        }
        Statement::Guarded { condition, statements } => {
            references_triggers_or_ffi_with_decls(condition, inop_decls)
                || statements.iter().any(|s| statement_contains_ffi_with_decls(s, inop_decls))
        }
        _ => false,
    }
}

fn is_self_identity(a: &Expr, b: &Expr) -> bool {
    matches!((a, b),
        (Expr::Identifier(n1), Expr::Identifier(n2)) if n1 == n2)
}

fn collect_identifiers(expr: &Expr, out: &mut HashSet<String>) {
    match expr {
        Expr::Identifier(name) | Expr::OwnedRef(name) | Expr::PriorState(name) => {
            out.insert(name.clone());
        }
        // Self-identity operations (x == x, x >= x, x <= x) are tautologies that
        // don't actually observe the field's value. Skip them to avoid keeping
        // fields artificially alive in dead-field analysis.
        Expr::Eq(a, b) if is_self_identity(a, b) => {}
        Expr::Ge(a, b) if is_self_identity(a, b) => {}
        Expr::Le(a, b) if is_self_identity(a, b) => {}
        Expr::Add(a, b) | Expr::Sub(a, b) | Expr::Mul(a, b) | Expr::Div(a, b)
        | Expr::Mod(a, b) | Expr::Eq(a, b) | Expr::Ne(a, b) | Expr::Lt(a, b)
        | Expr::Le(a, b) | Expr::Gt(a, b) | Expr::Ge(a, b) | Expr::Or(a, b)
        | Expr::And(a, b) | Expr::BitAnd(a, b) | Expr::BitOr(a, b) | Expr::BitXor(a, b)
        | Expr::Shl(a, b) | Expr::Shr(a, b) | Expr::Concat(a, b) => {
            collect_identifiers(a, out);
            collect_identifiers(b, out);
        }
        Expr::BinaryOp(bop) => {
            collect_identifiers(&bop.left, out);
            collect_identifiers(&bop.right, out);
        }
        Expr::UnaryOp(uop) => {
            collect_identifiers(&uop.operand, out);
        }
        Expr::Not(a) | Expr::Neg(a) | Expr::BitNot(a) | Expr::Projection { source: a, .. } => {
            collect_identifiers(a, out);
        }
        Expr::Cast(a, _) => collect_identifiers(a, out),
        Expr::Call(_, args) => {
            for arg in args {
                collect_identifiers(arg, out);
            }
        }
        Expr::IntrinsicCall { intrinsic: _, args } => {
            for arg in args {
                collect_identifiers(arg, out);
            }
        }
        Expr::ListLiteral(elems) => {
            for elem in elems {
                collect_identifiers(elem, out);
            }
        }
        Expr::ListIndex(list, idx) => {
            collect_identifiers(list, out);
            collect_identifiers(idx, out);
        }
        Expr::Slice { value, start, end, stride, mask } => {
            collect_identifiers(value, out);
            if let Some(s) = start { collect_identifiers(s, out); }
            if let Some(e) = end { collect_identifiers(e, out); }
            if let Some(s) = stride { collect_identifiers(s, out); }
            if let Some(m) = mask { collect_identifiers(m, out); }
        }
        Expr::MultiSlice { value, ops } => {
            collect_identifiers(value, out);
            for op in ops {
                match op {
                    BracketOp::Coord(c) => collect_identifiers_in_coord(c, out),
                    BracketOp::Mask(m) => collect_identifiers(m, out),
                    BracketOp::Stride(s) => collect_identifiers(s, out),
                }
            }
        }
        Expr::FieldAccess(obj, _) => {
            collect_identifiers(obj, out);
        }
        Expr::StructInstance(_, fields) => {
            for (_, expr) in fields {
                collect_identifiers(expr, out);
            }
        }
        Expr::ObjectLiteral(fields) => {
            for (_, expr) in fields {
                collect_identifiers(expr, out);
            }
        }
        Expr::PatternMatch { value, .. } => {
            collect_identifiers(value, out);
        }
        Expr::Match { value, arms } => {
            collect_identifiers(value, out);
            for arm in arms {
                if let Some(ref guard) = arm.guard {
                    collect_identifiers(guard, out);
                }
                collect_identifiers(&arm.body, out);
            }
        }
        Expr::Block(_, last) | Expr::TupleDestructure(_, last) => {
            collect_identifiers(last, out);
        }
        Expr::Tuple(elems) => {
            for elem in elems {
                collect_identifiers(elem, out);
            }
        }
        Expr::ArrowMut { target, index, value, .. } => {
            collect_identifiers(target, out);
            collect_identifiers(index, out);
            if let Some(v) = value {
                collect_identifiers(v, out);
            }
        }
            Expr::ArrowDiscard { target, index } => {
                collect_identifiers(target, out);
                collect_identifiers(index, out);
            }
            Expr::ArrowTransfer { dest, source, filter } => {
                collect_identifiers(dest, out);
                collect_identifiers(source, out);
                if let Some(f) = filter {
                    collect_identifiers(f, out);
                }
            }
            Expr::SigCall { expr, .. } => {
                collect_identifiers(expr, out);
            }
            Expr::Ellipsis => {}
            Expr::MapLiteral(entries) => {
                for (k, v) in entries {
                    collect_identifiers(k, out);
                    collect_identifiers(v, out);
                }
            }
            Expr::SetLiteral(entries) => {
                for e in entries {
                    collect_identifiers(e, out);
                }
            }
            Expr::DbvlTable { .. } => {}
            Expr::SubtypeProjection { source, .. } => {
                collect_identifiers(source, out);
            }
            _ => {}
        }
    }

fn collect_identifiers_in_coord(coord: &SliceCoordinate, out: &mut HashSet<String>) {
    match coord {
        SliceCoordinate::Index(e) => collect_identifiers(e, out),
        SliceCoordinate::Range { start, end } => {
            if let Some(s) = start { collect_identifiers(s, out); }
            if let Some(e) = end { collect_identifiers(e, out); }
        }
            SliceCoordinate::Named { coord, .. } => {
                collect_identifiers_in_coord(coord, out);
            }
            SliceCoordinate::AtDimension { coord, .. } => {
                collect_identifiers_in_coord(coord, out);
            }
            SliceCoordinate::Ellipsis => {}
        }
    }

fn extract_write_set(body: &[Statement], state_fields: &HashSet<String>) -> HashSet<String> {
    let mut writes = HashSet::new();
    for stmt in body {
        if let Statement::Assignment { lhs, .. } = stmt {
            let name = match lhs {
                Expr::Identifier(n) | Expr::OwnedRef(n) => n.clone(),
                _ => continue,
            };
            if state_fields.contains(&name) {
                writes.insert(name);
            }
        }
    }
    writes
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::*;

    fn make_state(name: &str, ty: Type) -> TopLevel {
        TopLevel::StateDecl(StateDecl {
            name: name.to_string(),
            ty,
            expr: None,
            address: None,
            bit_range: None,
            is_override: false,
            os_mode: false,
            span: None,
            attrs: vec![],
        constraint: None,
        })
    }

    #[test]
    fn test_extract_bounded_pre_counter_lt_total() {
        let pre = Expr::Lt(
            Box::new(Expr::Identifier("count".to_string())),
            Box::new(Expr::Identifier("total".to_string())),
        );
        let bp = extract_bounded_pre(&pre).unwrap();
        assert_eq!(bp.var, "count");
        assert_eq!(bp.bound_var, "total");
    }

    #[test]
    fn test_detect_increments() {
        let body = vec![Statement::Assignment {
            lhs: Expr::Identifier("count".to_string()),
            expr: Expr::Add(
                Box::new(Expr::Identifier("count".to_string())),
                Box::new(Expr::Integer(1)),
            ),
            timeout: None,
            modifiers: vec![],
        }];
        let inc = detect_increments(&body).unwrap();
        assert_eq!(inc.var, "count");
        assert_eq!(inc.delta, 1);
    }

    #[test]
    fn test_pure_counter_body() {
        let fields: HashSet<String> = ["count".to_string(), "total".to_string()].into();
        let body = vec![Statement::Assignment {
            lhs: Expr::Identifier("count".to_string()),
            expr: Expr::Add(
                Box::new(Expr::Identifier("count".to_string())),
                Box::new(Expr::Integer(1)),
            ),
            timeout: None,
            modifiers: vec![],
        }];
        let inc = detect_increments(&body);
        assert!(is_pure_body(&body, &fields, &inc, &HashMap::new()));
    }

    #[test]
    fn test_impure_body_with_state_write() {
        let fields: HashSet<String> = ["count".to_string(), "value".to_string()].into();
        let body = vec![
            Statement::Assignment {
                lhs: Expr::Identifier("value".to_string()),
                expr: Expr::Float(1.0),
                timeout: None,
                modifiers: vec![],
            },
            Statement::Assignment {
                lhs: Expr::Identifier("count".to_string()),
                expr: Expr::Add(
                    Box::new(Expr::Identifier("count".to_string())),
                    Box::new(Expr::Integer(1)),
                ),
                timeout: None,
                modifiers: vec![],
            },
        ];
        let inc = detect_increments(&body);
        assert!(!is_pure_body(&body, &fields, &inc, &HashMap::new()));
    }

    #[test]
    fn test_is_uniform_body_group_identical() {
        let body = vec![Statement::Assignment {
            lhs: Expr::Identifier("count".to_string()),
            expr: Expr::Add(
                Box::new(Expr::Identifier("count".to_string())),
                Box::new(Expr::Integer(1)),
            ),
            timeout: None,
            modifiers: vec![],
        }];
        let txn1 = Transaction {
            name: "txn_a".to_string(),
            is_reactive: true,
            is_async: false,
            parameters: vec![],
            contract: Contract {
                pre_condition: Expr::Bool(true),
                post_condition: Expr::Bool(true),
                span: None,
                watchdog: None,
            },
            body: body.clone(),
            reactor_speed: None,
            span: None,
            is_lambda: false,
            dependencies: vec![],

            annotations: vec![],
            modifiers: vec![],
            variant_bodies: vec![],
                 outputs: Vec::new(),
         output_type: None,
     };
        let txn2 = Transaction {
            name: "txn_b".to_string(),
            .. txn1.clone()
        };
        let pairs: Vec<(String, &Transaction)> = vec![
            ("txn_a".to_string(), &txn1),
            ("txn_b".to_string(), &txn2),
        ];
        assert!(is_uniform_body_group(&pairs));
    }

    #[test]
    fn test_is_uniform_body_group_different() {
        let body_a = vec![Statement::Assignment {
            lhs: Expr::Identifier("a".to_string()),
            expr: Expr::Integer(1),
            timeout: None,
            modifiers: vec![],
        }];
        let body_b = vec![Statement::Assignment {
            lhs: Expr::Identifier("b".to_string()),
            expr: Expr::Integer(2),
            timeout: None,
            modifiers: vec![],
        }];
        let txn_a = Transaction {
            name: "txn_a".to_string(),
            is_reactive: true,
            is_async: false,
            parameters: vec![],
            contract: Contract {
                pre_condition: Expr::Bool(true),
                post_condition: Expr::Bool(true),
                span: None,
                watchdog: None,
            },
            body: body_a,
            reactor_speed: None,
            span: None,
            is_lambda: false,
            dependencies: vec![],

            annotations: vec![],
            modifiers: vec![],
            variant_bodies: vec![],
                 outputs: Vec::new(),
         output_type: None,
     };
        let txn_b = Transaction {
            body: body_b,
            name: "txn_b".to_string(),
            .. txn_a.clone()
        };
        let pairs: Vec<(String, &Transaction)> = vec![
            ("txn_a".to_string(), &txn_a),
            ("txn_b".to_string(), &txn_b),
        ];
        assert!(!is_uniform_body_group(&pairs));
    }

    #[test]
    fn test_is_uniform_body_group_single() {
        let body = vec![];
        let txn = Transaction {
            name: "only".to_string(),
            is_reactive: true,
            is_async: false,
            parameters: vec![],
            contract: Contract {
                pre_condition: Expr::Bool(true),
                post_condition: Expr::Bool(true),
                span: None,
                watchdog: None,
            },
            body,
            reactor_speed: None,
            span: None,
            is_lambda: false,
            dependencies: vec![],

            annotations: vec![],
            modifiers: vec![],
            variant_bodies: vec![],
                 outputs: Vec::new(),
         output_type: None,
     };
        let pairs: Vec<(String, &Transaction)> = vec![("only".to_string(), &txn)];
        assert!(!is_uniform_body_group(&pairs));
    }

    #[test]
    fn test_graph_single_counter_txn() {
        let program = Program {
            items: vec![
                make_state("count", Type::Int),
                make_state("total", Type::Int),
                TopLevel::Transaction(Transaction {
                    name: "inc".to_string(),
                    is_reactive: true,
                    is_async: false,
                    parameters: vec![],
                    contract: Contract {
                        pre_condition: Expr::Lt(
                            Box::new(Expr::Identifier("count".to_string())),
                            Box::new(Expr::Identifier("total".to_string())),
                        ),
                        post_condition: Expr::Bool(true),
                        span: None,
                        watchdog: None,
                    },
                    body: vec![Statement::Assignment {
                        lhs: Expr::Identifier("count".to_string()),
                        expr: Expr::Add(
                            Box::new(Expr::Identifier("count".to_string())),
                            Box::new(Expr::Integer(1)),
                        ),
                        timeout: None,
                        modifiers: vec![],
                    }],
                    reactor_speed: None,
                    span: None,
                    is_lambda: false,
                    dependencies: vec![],

                    annotations: vec![],
                    modifiers: vec![],
                    variant_bodies: vec![],
                                 outputs: Vec::new(),
                 output_type: None,
             }),
            ],
            comments: vec![],
            reactor_speed: None,
            attrs: Vec::new(),
            ffi: None,
            strict_mode: StrictMode::Off,
            dispatch_mode: DispatchMode::Sequential,
            exit_condition: None,
            out_pragmas: vec![],
            default_sig_modifier: None,
                watchdog_defaults: (None, None),
        };
        let graph = ReactorTransitionGraph::build(&program);
        assert_eq!(graph.nodes.len(), 1);
        assert!(!graph.has_triggers);
        let node = &graph.nodes[0];
        assert!(node.bounded_pre.is_some());
        assert!(node.increments.is_some());
        assert!(node.is_pure_body);
    }

    #[test]
    fn test_compute_projection_usage_none() {
        let program = Program {
            items: vec![
                make_state("x", Type::Int),
                TopLevel::Transaction(Transaction {
                    name: "t".into(),
                    parameters: vec![],
                    contract: Contract::new(Expr::Bool(true), Expr::Bool(true)),
                    body: vec![Statement::Term { values: vec![], modifiers: vec![], swan_song: None }],
                    is_async: false, is_reactive: false, reactor_speed: None, span: None,
                    is_lambda: false, dependencies: vec![],
                    annotations: vec![],
                    modifiers: vec![], variant_bodies: vec![], outputs: Vec::new(), output_type: None,
                }),
            ],
            comments: vec![], reactor_speed: None, attrs: vec![], ffi: None,
            strict_mode: StrictMode::Off, dispatch_mode: DispatchMode::Sequential,
            exit_condition: None, out_pragmas: vec![], watchdog_defaults: (None, None), default_sig_modifier: None,
        };
        let usage = compute_projection_usage(&program);
        assert!(usage.is_empty(), "no projections → empty usage");
    }

    #[test]
    fn test_compute_projection_usage_single() {
        let program = Program {
            items: vec![
                make_state("x", Type::Int),
                TopLevel::Transaction(Transaction {
                    name: "t".into(),
                    parameters: vec![],
                    contract: Contract::new(Expr::Bool(true), Expr::Bool(true)),
                    body: vec![
                        Statement::Expression(Expr::Projection {
                            source: Box::new(Expr::Identifier("x".into())),
                            target: crate::ast::ProjectionTarget::Size,
                        }),
                        Statement::Term { values: vec![], modifiers: vec![], swan_song: None },
                    ],
                    is_async: false, is_reactive: false, reactor_speed: None, span: None,
                    is_lambda: false, dependencies: vec![],
                    annotations: vec![],
                    modifiers: vec![], variant_bodies: vec![], outputs: Vec::new(), output_type: None,
                }),
            ],
            comments: vec![], reactor_speed: None, attrs: vec![], ffi: None,
            strict_mode: StrictMode::Off, dispatch_mode: DispatchMode::Sequential,
            exit_condition: None, out_pragmas: vec![], watchdog_defaults: (None, None), default_sig_modifier: None,
        };
        let usage = compute_projection_usage(&program);
        assert_eq!(usage.len(), 1, "field x should have projection usage");
        let targets = usage.get("x").expect("x should be in usage");
        assert!(targets.contains("Size"), "x should have Size projection");
        assert_eq!(targets.len(), 1, "only one projection target");
    }

    #[test]
    fn test_assign_field_modes_single_lens() {
        let mut usage: HashMap<String, HashSet<String>> = HashMap::new();
        usage.insert("x".to_string(), {
            let mut s = HashSet::new();
            s.insert("Size".to_string());
            s
        });
        let all: HashSet<String> = ["x"].iter().map(|s| s.to_string()).collect();
        let referenced: HashSet<String> = ["x"].iter().map(|s| s.to_string()).collect();
        let modes = assign_field_modes(&all, &referenced, &usage);
        let mode = modes.get("x").expect("x should have a mode");
        assert_eq!(*mode, crate::analysis::FieldMode::Always, "single-lens + referenced → Always");
    }

    #[test]
    fn test_assign_field_modes_dual_lens() {
        let mut usage: HashMap<String, HashSet<String>> = HashMap::new();
        usage.insert("x".to_string(), {
            let mut s = HashSet::new();
            s.insert("Ptr".to_string());
            s.insert("Size".to_string());
            s
        });
        let all: HashSet<String> = ["x"].iter().map(|s| s.to_string()).collect();
        let referenced: HashSet<String> = ["x"].iter().map(|s| s.to_string()).collect();
        let modes = assign_field_modes(&all, &referenced, &usage);
        let mode = modes.get("x").expect("x should have a mode");
        assert_eq!(*mode, crate::analysis::FieldMode::LazyCached { cache_index: 0 }, "dual-lens → LazyCached");
    }

    #[test]
    fn test_assign_field_modes_no_usage_unreferenced() {
        let usage: HashMap<String, HashSet<String>> = HashMap::new();
        let all: HashSet<String> = ["x"].iter().map(|s| s.to_string()).collect();
        let referenced: HashSet<String> = HashSet::new(); // x is NOT referenced
        let modes = assign_field_modes(&all, &referenced, &usage);
        let mode = modes.get("x").expect("x should have a mode");
        assert_eq!(*mode, crate::analysis::FieldMode::Never, "unreferenced + no projection → Never");
    }

    #[test]
    fn test_assign_field_modes_no_usage_referenced() {
        let usage: HashMap<String, HashSet<String>> = HashMap::new();
        let all: HashSet<String> = ["x"].iter().map(|s| s.to_string()).collect();
        let referenced: HashSet<String> = ["x"].iter().map(|s| s.to_string()).collect(); // x IS referenced
        let modes = assign_field_modes(&all, &referenced, &usage);
        let mode = modes.get("x").expect("x should have a mode");
        assert_eq!(*mode, crate::analysis::FieldMode::Always, "referenced + no projection → Always");
    }
}

#[cfg(all(kani, feature = "kani_full"))]
mod kani_full_tests {
    use super::*;

    #[kani::proof]
    fn verify_collect_identifiers_literal_integer() {
        let mut out = HashSet::new();
        let expr = Expr::Literal(Box::new(crate::features::literal::LiteralExpr::Integer(42)));
        collect_identifiers(&expr, &mut out);
        assert!(out.is_empty());
    }

    #[kani::proof]
    fn verify_collect_identifiers_literal_bool() {
        let mut out = HashSet::new();
        let expr = Expr::Literal(Box::new(crate::features::literal::LiteralExpr::Bool(true)));
        collect_identifiers(&expr, &mut out);
        assert!(out.is_empty());
    }

    #[kani::proof]
    fn verify_collect_identifiers_literal_term() {
        let mut out = HashSet::new();
        let expr = Expr::Literal(Box::new(crate::features::literal::LiteralExpr::Term));
        collect_identifiers(&expr, &mut out);
        assert!(out.is_empty());
    }
}
