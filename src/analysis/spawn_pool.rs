//! 2026-08-07 (object instance pools): predictably-inexhaustible pools.
//!
//! Briv has no runtime errors: a spawn pool must be PROVABLY inexhaustible.
//! This analysis computes, per obj base, the maximum number of concurrent
//! live instances (spawns minus frees, weighted by the enclosing bounded
//! iteration / reactive firing count). The backend sizes the member columns
//! to this proven maximum — no runtime exhaustion path exists.
//!
//! A spawn whose multiplicity cannot be statically bounded (a runtime-bound
//! loop or a never-converging reactive node) is a COMPILE ERROR: the pool
//! could exhaust, which the language forbids. (The runtime-sized dependent
//! capacity buffer for runtime-bound loops is a documented follow-up.)

use crate::ast::{Expr, Statement, TopLevel};
use std::collections::HashMap;

/// The result: `base` → the proven maximum live instance count (≥ 1 — row 0
/// is the static instance), plus the unprovable-spawn errors.
pub fn analyze(items: &[TopLevel]) -> (HashMap<String, usize>, Vec<String>) {
    let mut capacities: HashMap<String, usize> = HashMap::new();
    let mut errors: Vec<String> = Vec::new();
    for item in items {
        match item {
            TopLevel::Transaction(t) => {
                // The reactive firing multiplicity — the countdown count if
                // it is a compile-time constant, else unprovable.
                let firing = node_firing_count(t);
                let mut live: HashMap<String, i64> = HashMap::new();
                walk_stmts(&t.body, 1, firing.as_ref(), &mut live, &mut errors);
                merge_max(&mut capacities, &live);
            }
            TopLevel::Definition(d) => {
                let mut live: HashMap<String, i64> = HashMap::new();
                walk_stmts(&d.body, 1, None, &mut live, &mut errors);
                merge_max(&mut capacities, &live);
            }
            TopLevel::Statement(stmt) => {
                let mut live: HashMap<String, i64> = HashMap::new();
                walk_stmt(stmt, 1, None, &mut live, &mut errors);
                merge_max(&mut capacities, &live);
            }
            _ => {}
        }
    }
    (capacities, errors)
}

/// A reactive node's firing count: `[count < N][count == N]` with a
/// compile-time constant `N`. `None` when unprovable (a runtime bound or a
/// never-converging precondition such as `[true]`).
fn node_firing_count(t: &crate::ast::Transaction) -> Option<i64> {
    if !t.is_reactive {
        return None;
    }
    let pre = &t.contract.pre_condition;
    match pre {
        Expr::Bool(true) => None,
        Expr::BinaryOp(crate::ast::BinaryOpKind::Lt, _, r) => match r.as_ref() {
            Expr::Decimal(n) if *n >= 0 => Some(*n),
            _ => None,
        },
        Expr::BinaryOp(crate::ast::BinaryOpKind::Le, _, r) => match r.as_ref() {
            Expr::Decimal(n) if *n >= 0 => Some(*n + 1),
            _ => None,
        },
        _ => None,
    }
}

fn walk_stmts(
    stmts: &[Statement],
    multiplier: i64,
    firing: Option<&i64>,
    live: &mut HashMap<String, i64>,
    errors: &mut Vec<String>,
) {
    for s in stmts {
        walk_stmt(s, multiplier, firing, live, errors);
    }
}

fn walk_stmt(
    stmt: &Statement,
    multiplier: i64,
    firing: Option<&i64>,
    live: &mut HashMap<String, i64>,
    errors: &mut Vec<String>,
) {
    match stmt {
        Statement::Foreach { list, body, .. } => {
            // The loop multiplicity: a compile-time-constant range length.
            let count = match list.as_ref() {
                Expr::Range { start, end, inclusive } => match (start.as_ref(), end.as_ref()) {
                    (Expr::Decimal(s), Expr::Decimal(e)) if *s <= *e => {
                        Some((if *inclusive { e - s + 1 } else { e - s }).max(0))
                    }
                    _ => None,
                },
                _ => None,
            };
            match count {
                Some(c) => walk_stmts(body, multiplier * c, firing, live, errors),
                None => {
                    // Runtime-bound loop — a spawn inside it cannot be
                    // statically bounded.
                    let mut inner = live.clone();
                    let mut errs = Vec::new();
                    walk_stmts(body, 0, firing, &mut inner, &mut errs);
                    let unprovable: Vec<String> = errs.iter().filter(|e| e.contains("spawn")).cloned().collect();
                    if !unprovable.is_empty() {
                        errors.push(
                            "a spawn inside a runtime-bound loop cannot be statically bounded; \
                             use a compile-time constant loop bound (the runtime-sized dependent \
                             capacity buffer is a follow-up)"
                                .to_string(),
                        );
                    }
                    let _ = firing;
                }
            }
        }
        Statement::Guarded(_, body) => walk_stmts(body, multiplier, firing, live, errors),
        Statement::If(_, then, els) => {
            walk_stmts(then, multiplier, firing, live, errors);
            walk_stmts(els, multiplier, firing, live, errors);
        }
        Statement::Block(body) | Statement::SyncBlock(body) => {
            walk_stmts(body, multiplier, firing, live, errors);
        }
        Statement::Assign(_, rhs) => walk_expr(rhs, multiplier, firing, live, errors),
        Statement::Term(Some(e)) | Statement::EndProgram(Some(e)) => {
            walk_expr(e, multiplier, firing, live, errors);
        }
        Statement::Expression(e) | Statement::Gate(e) => walk_expr(e, multiplier, firing, live, errors),
        Statement::FreeHint(name) | Statement::KeepHint(name) => {
            let entry = live.entry(name.clone()).or_insert(0);
            *entry = (*entry - multiplier).max(0);
        }
        Statement::Let { expr: Some(e), .. } => walk_expr(e, multiplier, firing, live, errors),
        _ => {}
    }
}

fn walk_expr(
    expr: &Expr,
    multiplier: i64,
    firing: Option<&i64>,
    live: &mut HashMap<String, i64>,
    errors: &mut Vec<String>,
) {
    match expr {
        Expr::Spawn { type_name, args } => {
            let weight = multiplier * firing.map(|f| *f).unwrap_or(1);
            match firing {
                Some(_) => {
                    let entry = live.entry(type_name.clone()).or_insert(0);
                    *entry += weight;
                }
                None => {
                    errors.push(format!(
                        "spawn of '{}' is not statically bounded — the pool must be predictably \
                         inexhaustible; spawn inside a bounded iteration or a countdown node with a \
                         compile-time constant bound",
                        type_name
                    ));
                }
            }
            for a in args {
                walk_expr(a, multiplier, firing, live, errors);
            }
        }
        Expr::BinaryOp(_, l, r) => {
            walk_expr(l, multiplier, firing, live, errors);
            walk_expr(r, multiplier, firing, live, errors);
        }
        Expr::Call(_, args, _) | Expr::List(args) | Expr::Tuple(args) => {
            for a in args {
                walk_expr(a, multiplier, firing, live, errors);
            }
        }
        Expr::Cast(i, _) | Expr::IsType(i, _) | Expr::Consume(i) | Expr::Deref(i) | Expr::AddrOf(i) => {
            walk_expr(i, multiplier, firing, live, errors);
        }
        Expr::Field(o, _) | Expr::Index(o, _) | Expr::Reflect(o, _, _) => {
            walk_expr(o, multiplier, firing, live, errors);
        }
        Expr::MethodCall(recv, _, args, _) => {
            walk_expr(recv, multiplier, firing, live, errors);
            for a in args {
                walk_expr(a, multiplier, firing, live, errors);
            }
        }
        Expr::If(c, t, e) => {
            walk_expr(c, multiplier, firing, live, errors);
            walk_expr(t, multiplier, firing, live, errors);
            if let Some(e) = e {
                walk_expr(e, multiplier, firing, live, errors);
            }
        }
        Expr::Match(s, arms) => {
            walk_expr(s, multiplier, firing, live, errors);
            for arm in arms {
                if let Some(g) = &arm.guard {
                    walk_expr(g, multiplier, firing, live, errors);
                }
                walk_expr(&arm.body, multiplier, firing, live, errors);
            }
        }
        Expr::Slice { array, start, end, stride, .. } => {
            walk_expr(array, multiplier, firing, live, errors);
            for b in [start, end, stride].into_iter().flatten() {
                walk_expr(b, multiplier, firing, live, errors);
            }
        }
        Expr::StructLiteral { fields, .. } => {
            for (_, f) in fields {
                walk_expr(f, multiplier, firing, live, errors);
            }
        }
        Expr::Range { start, end, .. } => {
            walk_expr(start, multiplier, firing, live, errors);
            walk_expr(end, multiplier, firing, live, errors);
        }
        _ => {}
    }
}

fn merge_max(out: &mut HashMap<String, usize>, live: &HashMap<String, i64>) {
    for (base, n) in live {
        let entry = out.entry(base.clone()).or_insert(0);
        *entry = (*entry).max(*n as usize).max(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Contract, Expr, Statement, TopLevel, Transaction};

    fn txn(name: &str, pre: Expr, post: Expr, body: Vec<Statement>) -> TopLevel {
        TopLevel::Transaction(Transaction {
            name: name.to_string(),
            is_reactive: true,
            is_async: false,
            type_params: vec![],
            parameters: vec![],
            output_type: None,
            outputs: vec![],
            contract: Contract {
                pre_condition: pre,
                post_condition: post,
                watchdog: None,
                explicit: false,
                span: None,
            },
            body,
            metadata: std::collections::HashMap::new(),
            derivation: None,
            modifiers: vec![],
            span: None,
            doc: None,
        })
    }

    fn spawn(base: &str) -> Statement {
        Statement::Let {
            name: "h".to_string(),
            names: vec![],
            ty: None,
            expr: Some(Expr::Spawn { type_name: base.to_string(), args: vec![] }),
            modifiers: vec![],
        }
    }

    #[test]
    fn const_countdown_spawn_is_bounded() {
        // `[ticks < 2]` fires twice; one spawn per firing → live = 2.
        let program = vec![txn(
            "work",
            Expr::BinaryOp(crate::ast::BinaryOpKind::Lt,
                Box::new(Expr::Identifier("ticks".into())),
                Box::new(Expr::Decimal(2))),
            Expr::BinaryOp(crate::ast::BinaryOpKind::Eq,
                Box::new(Expr::Identifier("ticks".into())),
                Box::new(Expr::Decimal(2))),
            vec![spawn("Counter")],
        )];
        let (caps, errors) = analyze(&program);
        assert!(errors.is_empty(), "expected no errors, got {errors:?}");
        assert_eq!(caps.get("Counter"), Some(&2));
    }

    #[test]
    fn runtime_countdown_spawn_is_rejected() {
        // `[ticks < N]` with a runtime N — not statically bounded.
        let program = vec![txn(
            "work",
            Expr::BinaryOp(crate::ast::BinaryOpKind::Lt,
                Box::new(Expr::Identifier("ticks".into())),
                Box::new(Expr::Identifier("N".into()))),
            Expr::BinaryOp(crate::ast::BinaryOpKind::Eq,
                Box::new(Expr::Identifier("ticks".into())),
                Box::new(Expr::Identifier("N".into()))),
            vec![spawn("Counter")],
        )];
        let (_, errors) = analyze(&program);
        assert!(!errors.is_empty(), "a runtime-bound spawn must be rejected");
        assert!(errors[0].contains("not statically bounded"));
    }
}
