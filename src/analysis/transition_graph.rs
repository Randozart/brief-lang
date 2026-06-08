use crate::ast::{ArrowDir, BracketOp, Expr, Hashtag, Program, ProjectionTarget, SliceCoordinate, Statement, TopLevel};
use std::collections::HashSet;

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

        for item in &program.items {
            match item {
                TopLevel::Transaction(txn) => {
                    let simplified_body = simplify_body(&txn.body);
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
                    let is_pure = is_pure_body(&simplified_body, &state_field_names, &increments);
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
            compute_effectively_pure(node, &live_fields);
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
fn extract_valid_bounded_pre(pre: &Expr, inc: &Option<IncrementInfo>) -> Option<BoundedPre> {
    let bp = extract_bounded_pre(pre)?;
    let is_mutated = inc.as_ref().map_or(false, |i| i.var == bp.var);
    if is_mutated { Some(bp) } else { None }
}

/// Recursively simplify an expression using algebraic cancellation rules.
/// Applied bottom-up with fixpoint iteration (max 5 passes) to handle
/// chains like `((x + R) - R) + 1` → `x + 1`.
fn simplify_expr(expr: &Expr) -> Expr {
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
        Statement::Let { name, ty, expr, address, address_expr, bit_range, is_override, modifiers } => Statement::Let {
            name: name.clone(),
            ty: ty.clone(),
            expr: expr.as_ref().map(|e| simplify_expr(e)),
            address: *address,
            address_expr: address_expr.clone(),
            bit_range: bit_range.clone(),
            is_override: *is_override,
            modifiers: modifiers.clone(),
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
    let mut current = body.to_vec();
    for _ in 0..5 {
        let next: Vec<Statement> = current.iter().map(|s| simplify_stmt(s)).collect();
        if next == current {
            break;
        }
        current = next;
    }
    current
}

fn detect_increments(body: &[Statement]) -> Option<IncrementInfo> {
    for stmt in body {
        if let Statement::Assignment { lhs, expr, .. } = stmt {
            let name = match lhs {
                Expr::Identifier(n) | Expr::OwnedRef(n) => n.clone(),
                _ => continue,
            };
            if let Expr::Add(a, b) = expr {
                if let (Expr::Identifier(var), Expr::Integer(delta)) = (a.as_ref(), b.as_ref()) {
                    if *var == name && *delta > 0 {
                        return Some(IncrementInfo { var: name.clone(), delta: *delta });
                    }
                }
                if let (Expr::Identifier(var), Expr::Integer(delta)) = (b.as_ref(), a.as_ref()) {
                    if *var == name && *delta > 0 {
                        return Some(IncrementInfo { var: name.clone(), delta: *delta });
                    }
                }
            }
            // Decreasing counter: count = count - delta or count = count - 1
            if let Expr::Sub(a, b) = expr {
                if let (Expr::Identifier(var), Expr::Integer(delta)) = (a.as_ref(), b.as_ref()) {
                    if *var == name && *delta > 0 {
                        return Some(IncrementInfo { var: name.clone(), delta: *delta });
                    }
                }
            }
            // Interval bounds: (x + R1) - R2 where net step R1 - R2 ≥ 1
            if let Expr::Sub(inner, rhs) = expr {
                if let Expr::Add(lhs, rhs2) = inner.as_ref() {
                    let is_self_add = matches!(lhs.as_ref(), Expr::Identifier(v) if *v == name);
                    if is_self_add {
                        // Try r1 - r2 with both as constants
                        let r1 = if let Expr::Integer(n) = rhs2.as_ref() { Some(*n) } else { None };
                        let r2 = if let Expr::Integer(n) = rhs.as_ref() { Some(*n) } else { None };
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
                    if references_triggers_or_ffi(e) {
                        return false;
                    }
                }
            }
            Statement::Assignment { expr, .. } => {
                if references_triggers_or_ffi(expr) {
                    return false;
                }
            }
            Statement::Expression(e) => {
                if references_triggers_or_ffi(e) {
                    return false;
                }
            }
            Statement::Term { swan_song, .. } | Statement::TermBang { swan_song, .. } => {
                if let Some(swan) = swan_song {
                    if statement_contains_ffi(swan) { return false; }
                }
            }
            Statement::Escape(_) => return false,
            Statement::OnExit { .. } => return false,
            Statement::Guarded { condition, statements } => {
                if references_triggers_or_ffi(condition) {
                    return false;
                }
                if statements.iter().any(|s| statement_contains_ffi(s)) {
                    return false;
                }
            }
            _ => {}
        }
    }
    true
}

fn references_triggers_or_ffi(expr: &Expr) -> bool {
    match expr {
        Expr::Call(_, _) => true,
        Expr::Identifier(_) | Expr::Integer(_) | Expr::Float(_) | Expr::Bool(_) | Expr::String(_) | Expr::Char(_) => false,
        Expr::Add(a, b) | Expr::Sub(a, b) | Expr::Mul(a, b) | Expr::Div(a, b) | Expr::Mod(a, b)
        | Expr::Eq(a, b) | Expr::Ne(a, b) | Expr::Lt(a, b) | Expr::Le(a, b) | Expr::Gt(a, b)
        | Expr::Ge(a, b) | Expr::And(a, b) | Expr::Or(a, b) | Expr::BitAnd(a, b)
        | Expr::BitOr(a, b) | Expr::BitXor(a, b) | Expr::Shl(a, b) | Expr::Shr(a, b) => {
            references_triggers_or_ffi(a) || references_triggers_or_ffi(b)
        }
        Expr::Not(a) | Expr::Neg(a) | Expr::BitNot(a) => references_triggers_or_ffi(a),
        Expr::Cast(a, _) => references_triggers_or_ffi(a),
        Expr::Block(_, last) | Expr::TupleDestructure(_, last) => references_triggers_or_ffi(last),
        Expr::ListLiteral(elems) => elems.iter().any(|e| references_triggers_or_ffi(e)),
        Expr::ListIndex(list, idx) => references_triggers_or_ffi(list) || references_triggers_or_ffi(idx),
        Expr::Projection { source: inner, .. } => references_triggers_or_ffi(inner),
        Expr::Tuple(elems) => elems.iter().any(|e| references_triggers_or_ffi(e)),
        Expr::FieldAccess(obj, _) => references_triggers_or_ffi(obj),
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

fn compute_effectively_pure(node: &mut ReactorNode, live_fields: &HashSet<String>) {
    // FFI calls have side effects — cannot fold to pure counter
    if node.body.iter().any(|s| statement_contains_ffi(s)) {
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

fn statement_contains_ffi(stmt: &Statement) -> bool {
    match stmt {
        Statement::Assignment { expr, .. } => references_triggers_or_ffi(expr),
        Statement::Let { expr, .. } => expr.as_ref().map_or(false, |e| references_triggers_or_ffi(e)),
        Statement::Expression(e) => references_triggers_or_ffi(e),
        Statement::Term { values, swan_song, .. } => {
            values.iter().any(|v| v.as_ref().map_or(false, |e| references_triggers_or_ffi(e)))
                || swan_song.as_ref().map_or(false, |s| statement_contains_ffi(s))
        }
        Statement::TermBang { values, swan_song, .. } => {
            values.iter().any(|v| v.as_ref().map_or(false, |e| references_triggers_or_ffi(e)))
                || swan_song.as_ref().map_or(false, |s| statement_contains_ffi(s))
        }
        Statement::Guarded { condition, statements } => {
            references_triggers_or_ffi(condition)
                || statements.iter().any(|s| statement_contains_ffi(s))
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
        Expr::Integer(_) | Expr::Float(_) | Expr::String(_) | Expr::Char(_) | Expr::Bool(_) | Expr::Term => {}
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
        Expr::Not(a) | Expr::Neg(a) | Expr::BitNot(a) | Expr::Projection { source: a, .. } => {
            collect_identifiers(a, out);
        }
        Expr::Cast(a, _) => collect_identifiers(a, out),
        Expr::Call(_, args) => {
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
            Expr::SubtypeProjection { source, .. } => {
                collect_identifiers(source, out);
            }
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
        assert!(is_pure_body(&body, &fields, &inc));
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
        assert!(!is_pure_body(&body, &fields, &inc));
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
            attrs: vec![],
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
            attrs: vec![],
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
            attrs: vec![],
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
                    attrs: vec![],
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
        };
        let graph = ReactorTransitionGraph::build(&program);
        assert_eq!(graph.nodes.len(), 1);
        assert!(!graph.has_triggers);
        let node = &graph.nodes[0];
        assert!(node.bounded_pre.is_some());
        assert!(node.increments.is_some());
        assert!(node.is_pure_body);
    }
}
