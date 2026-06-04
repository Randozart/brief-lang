use crate::ast::{Expr, Hashtag, Program, SliceCoordinate, Statement, TopLevel};
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
                    let bounded_pre = extract_bounded_pre(&txn.contract.pre_condition);
                    let increments = detect_increments(&txn.body);
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
                    let is_pure = is_pure_body(&txn.body, &state_field_names, &increments);
                    let write_set = extract_write_set(&txn.body, &state_field_names);

                    nodes.push(ReactorNode {
                        name: txn.name.clone(),
                        is_reactive: txn.is_reactive,
                        precondition: txn.contract.pre_condition.clone(),
                        body: txn.body.clone(),
                        bounded_pre,
                        increments,
                        is_pure_body: is_pure,
                        write_set,
                        is_effectively_pure: false,
                    });
                }
                TopLevel::Trigger(_) => {
                    has_triggers = true;
                }
                _ => {}
            }
        }

        let live_fields = compute_live_fields(&program.exit_condition, &nodes);
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
                _ => None,
            }
        }
        Expr::And(l, r) => {
            extract_bounded_pre(l).or_else(|| extract_bounded_pre(r))
        }
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
        }
    }
    None
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
                    if !matches!(expr, Expr::Add(_, _)) {
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
            Statement::Term { .. } => {}
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
        Expr::ListLen(inner) => references_triggers_or_ffi(inner),
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
    nodes: &[ReactorNode],
) -> HashSet<String> {
    let mut live = HashSet::new();
    if let Some(ec) = exit_condition {
        collect_identifiers(ec, &mut live);
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
        Statement::Expression(expr) => {
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

fn statement_contains_ffi(stmt: &Statement) -> bool {
    match stmt {
        Statement::Assignment { expr, .. } => references_triggers_or_ffi(expr),
        Statement::Let { expr, .. } => expr.as_ref().map_or(false, |e| references_triggers_or_ffi(e)),
        Statement::Expression(e) => references_triggers_or_ffi(e),
        Statement::Term { values, .. } => values.iter().any(|v| v.as_ref().map_or(false, |e| references_triggers_or_ffi(e))),
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
        Expr::Not(a) | Expr::Neg(a) | Expr::BitNot(a) | Expr::ListLen(a) => {
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
        Expr::MultiSlice { value, coordinates, mask } => {
            collect_identifiers(value, out);
            for coord in coordinates {
                collect_identifiers_in_coord(coord, out);
            }
            if let Some(m) = mask { collect_identifiers(m, out); }
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
        Expr::ForAll { expr, .. } | Expr::Exists { expr, .. } => {
            collect_identifiers(expr, out);
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
            Expr::Ellipsis => {}
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
                }),
            ],
            comments: vec![],
            reactor_speed: None,
            attrs: Vec::new(),
            ffi: None,
            strict_mode: StrictMode::Off,
            dispatch_mode: DispatchMode::Sequential,
            exit_condition: None,
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
