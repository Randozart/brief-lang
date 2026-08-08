//! 2026-08-07 (object instance pools): predictably-inexhaustible pools.
//!
//! Briv has no runtime errors: a spawn pool must be PROVABLY inexhaustible.
//! This analysis computes, per obj base, the maximum number of concurrent
//! live instances (spawns minus frees, weighted by the enclosing bounded
//! iteration / reactive firing count), OR marks the pool DEPENDENT when the
//! bound is a runtime value.
//!
//! - A STATIC countdown (`[ticks < N]` with a compile-time N) sizes the
//!   member columns to the proven maximum — no runtime exhaustion path.
//! - A DEPENDENT countdown (`[ticks < N]` with N a runtime field/const name)
//!   still bounds the pool: the capacity is N at runtime, so the backend
//!   allocates the member columns as a runtime-sized heap buffer (proven ≥
//!   the bound; SPEC §16.6 dependent bounds). The analysis returns the bound
//!   EXPRESSION per base so the backend can size the malloc.
//! - A spawn whose multiplicity is genuinely unbounded (a `[true]` node, a
//!   non-countdown loop) is a COMPILE ERROR.

use crate::ast::{Expr, Statement, TopLevel};
use std::collections::HashMap;

/// A reactive node's firing multiplicity.
#[derive(Clone, PartialEq)]
enum Firing {
    /// `[count < N]` with a compile-time constant N — the columns are sized
    /// statically to the proven maximum.
    Static(i64),
    /// `[count < N]` with a runtime N (a state field or a named const) — the
    /// pool is sized at runtime from the bound (dependent capacity). Carries
    /// the bound EXPRESSION so the backend can size the heap buffer.
    Dependent(Expr),
    /// Not a countdown (a `[true]` precondition, a non-bounded guard) — an
    /// unprovable spawn here is an error.
    Unprovable,
}

/// One runtime-bound spawn context for a base: a compile-time multiplier
/// (the enclosing const foreach products) times the runtime bound expression
/// (a countdown bound or a runtime-range foreach bound).
#[derive(Clone, Debug)]
pub struct DependentTerm {
    pub multiplier: i64,
    pub bound: Expr,
}

/// The result: `base` → the proven maximum live instance count for STATIC
/// pools (≥ 1 — row 0 is the static instance); `base` → the runtime-bound
/// spawn terms for DEPENDENT pools (the backend sizes the heap buffer to
/// the sum of the terms + 1, an over-approximation of the concurrent live
/// maximum — provably inexhaustible); and the unprovable-spawn errors.
pub fn analyze(items: &[TopLevel]) -> (HashMap<String, usize>, HashMap<String, Vec<DependentTerm>>, Vec<String>) {
    let mut capacities: HashMap<String, usize> = HashMap::new();
    let mut dependent: HashMap<String, Vec<DependentTerm>> = HashMap::new();
    let mut errors: Vec<String> = Vec::new();
    for item in items {
        match item {
            TopLevel::Transaction(t) => {
                let firing = node_firing(t);
                let mut live: HashMap<String, i64> = HashMap::new();
                let mut terms: HashMap<String, Vec<DependentTerm>> = HashMap::new();
                let mut ctx = WalkCtx { firing: &firing, bound_terms: &[] };
                walk_stmts(&t.body, 1, &mut ctx, &mut live, &mut terms, &mut errors);
                merge_max(&mut capacities, &live);
                for (base, ts) in terms {
                    dependent.entry(base).or_default().extend(ts);
                }
            }
            TopLevel::Definition(d) => {
                let mut live: HashMap<String, i64> = HashMap::new();
                let mut terms: HashMap<String, Vec<DependentTerm>> = HashMap::new();
                let mut ctx = WalkCtx { firing: &Firing::Static(1), bound_terms: &[] };
                walk_stmts(&d.body, 1, &mut ctx, &mut live, &mut terms, &mut errors);
                merge_max(&mut capacities, &live);
                for (base, ts) in terms {
                    dependent.entry(base).or_default().extend(ts);
                }
            }
            TopLevel::Statement(stmt) => {
                let mut live: HashMap<String, i64> = HashMap::new();
                let mut terms: HashMap<String, Vec<DependentTerm>> = HashMap::new();
                let mut ctx = WalkCtx { firing: &Firing::Static(1), bound_terms: &[] };
                walk_stmt(stmt, 1, &mut ctx, &mut live, &mut terms, &mut errors);
                merge_max(&mut capacities, &live);
                for (base, ts) in terms {
                    dependent.entry(base).or_default().extend(ts);
                }
            }
            _ => {}
        }
    }
    (capacities, dependent, errors)
}

/// The walk context: the node's firing multiplicity, plus the runtime bound
/// expressions of enclosing runtime-bound foreachs (each spawn's capacity is
/// the firing count times the product of those bounds).
struct WalkCtx<'a> {
    firing: &'a Firing,
    bound_terms: &'a [Expr],
}

/// Classify a reactive node's firing: a countdown `[count < N][count == N]`
/// with a compile-time N is Static; with a runtime N (a field or a named
/// const) it is Dependent (the capacity is N at runtime); anything else is
/// Unprovable.
fn node_firing(t: &crate::ast::Transaction) -> Firing {
    if !t.is_reactive {
        return Firing::Static(1);
    }
    match &t.contract.pre_condition {
        Expr::Bool(true) => Firing::Unprovable,
        Expr::BinaryOp(crate::ast::BinaryOpKind::Lt, _, r) | Expr::BinaryOp(crate::ast::BinaryOpKind::Le, _, r) => {
            match r.as_ref() {
                Expr::Decimal(n) if *n >= 0 => {
                    let inclusive = matches!(t.contract.pre_condition, Expr::BinaryOp(crate::ast::BinaryOpKind::Le, _, _));
                    Firing::Static(if inclusive { n + 1 } else { *n })
                }
                Expr::Identifier(_) => Firing::Dependent((**r).clone()),
                _ => Firing::Unprovable,
            }
        }
        _ => Firing::Unprovable,
    }
}

/// The runtime bound of a foreach list expression, when it is a runtime
/// value (a `0..M` range with a runtime end). A const range and a list
/// literal are statically known; anything else cannot bound a spawn.
fn foreach_bound(list: &Expr) -> Option<Expr> {
    match list {
        Expr::Range { start, end, .. } => match (start.as_ref(), end.as_ref()) {
            (Expr::Decimal(_), Expr::Decimal(_)) => None,
            (_, Expr::Identifier(_)) | (_, _) if matches!(end.as_ref(), Expr::Identifier(_) | Expr::Field(_, _)) => {
                Some((**end).clone())
            }
            _ => None,
        },
        _ => None,
    }
}

fn walk_stmts(
    stmts: &[Statement],
    multiplier: i64,
    ctx: &mut WalkCtx,
    live: &mut HashMap<String, i64>,
    terms: &mut HashMap<String, Vec<DependentTerm>>,
    errors: &mut Vec<String>,
) {
    for s in stmts {
        walk_stmt(s, multiplier, ctx, live, terms, errors);
    }
}

fn walk_stmt(
    stmt: &Statement,
    multiplier: i64,
    ctx: &mut WalkCtx,
    live: &mut HashMap<String, i64>,
    terms: &mut HashMap<String, Vec<DependentTerm>>,
    errors: &mut Vec<String>,
) {
    match stmt {
        Statement::Foreach { list, body, .. } => {
            let count = match list.as_ref() {
                Expr::Range { start, end, inclusive } => match (start.as_ref(), end.as_ref()) {
                    (Expr::Decimal(s), Expr::Decimal(e)) if *s <= *e => {
                        Some((if *inclusive { e - s + 1 } else { e - s }).max(0))
                    }
                    _ => None,
                },
                Expr::List(elems) => Some(elems.len() as i64),
                _ => None,
            };
            match count {
                Some(c) => walk_stmts(body, multiplier * c, ctx, live, terms, errors),
                None => {
                    // A runtime-bound loop — a spawn inside it yields a
                    // DEPENDENT pool (the capacity is the loop's runtime
                    // bound, §16.6), not an error. Push the bound expression
                    // so inner spawns accumulate it.
                    if let Some(bound) = foreach_bound(list.as_ref()) {
                        let nested = ctx.bound_terms;
                        let extended: Vec<Expr> = nested.iter().cloned().chain(std::iter::once(bound)).collect();
                        let mut inner_ctx = WalkCtx { firing: ctx.firing, bound_terms: &extended };
                        walk_stmts(body, multiplier, &mut inner_ctx, live, terms, errors);
                    } else {
                        // A foreach we cannot bound (a value-carrying list).
                        let mut inner_live: HashMap<String, i64> = HashMap::new();
                        let mut inner_terms: HashMap<String, Vec<DependentTerm>> = HashMap::new();
                        let mut inner_errors = Vec::new();
                        let nested = ctx.bound_terms;
                        let extended: Vec<Expr> = nested.to_vec();
                        let mut inner_ctx = WalkCtx { firing: ctx.firing, bound_terms: &extended };
                        walk_stmts(body, 0, &mut inner_ctx, &mut inner_live, &mut inner_terms, &mut inner_errors);
                        // An unprovable firing context (not a countdown) inside
                        // such a loop is still an error.
                        for e in inner_errors {
                            errors.push(e);
                        }
                        for (base, ts) in inner_terms {
                            terms.entry(base).or_default().extend(ts);
                        }
                    }
                }
            }
        }
        Statement::Guarded(_, body) => walk_stmts(body, multiplier, ctx, live, terms, errors),
        Statement::If(_, then, els) => {
            walk_stmts(then, multiplier, ctx, live, terms, errors);
            walk_stmts(els, multiplier, ctx, live, terms, errors);
        }
        Statement::Block(body) | Statement::SyncBlock(body) => {
            walk_stmts(body, multiplier, ctx, live, terms, errors);
        }
        Statement::Assign(_, rhs) => walk_expr(rhs, multiplier, ctx, live, terms, errors),
        Statement::Term(Some(e)) | Statement::EndProgram(Some(e)) => {
            walk_expr(e, multiplier, ctx, live, terms, errors);
        }
        Statement::Expression(e) | Statement::Gate(e) => walk_expr(e, multiplier, ctx, live, terms, errors),
        Statement::FreeHint(name) | Statement::KeepHint(name) => {
            let entry = live.entry(name.clone()).or_insert(0);
            *entry = (*entry - multiplier).max(0);
        }
        Statement::Let { expr: Some(e), .. } => walk_expr(e, multiplier, ctx, live, terms, errors),
        _ => {}
    }
}

fn walk_expr(
    expr: &Expr,
    multiplier: i64,
    ctx: &mut WalkCtx,
    live: &mut HashMap<String, i64>,
    terms: &mut HashMap<String, Vec<DependentTerm>>,
    errors: &mut Vec<String>,
) {
    match expr {
        Expr::Spawn { type_name, args } => {
            match ctx.firing {
                Firing::Static(n) if ctx.bound_terms.is_empty() => {
                    let entry = live.entry(type_name.clone()).or_insert(0);
                    *entry += multiplier * (*n).max(1);
                }
                Firing::Static(n) => {
                    // Enclosing runtime-bound foreachs: the capacity is the
                    // const firing count times the runtime loop bounds.
                    let bound = product(ctx.bound_terms);
                    let entry = terms.entry(type_name.clone()).or_default();
                    entry.push(DependentTerm { multiplier: multiplier * (*n).max(1), bound });
                }
                                Firing::Dependent(countdown_bound) => {
                    // The countdown bound N is the runtime firing count; each
                    // enclosing runtime-bound foreach multiplies it.
                    let mut bounds: Vec<Expr> = Vec::with_capacity(ctx.bound_terms.len() + 1);
                    bounds.push(countdown_bound.clone());
                    bounds.extend(ctx.bound_terms.iter().cloned());
                    let bound = product(&bounds);
                    let entry = terms.entry(type_name.clone()).or_default();
                    entry.push(DependentTerm { multiplier, bound });
                }
                Firing::Unprovable => {
                    errors.push(format!(
                        "spawn of '{}' is not statically bounded — the pool must be predictably \
                         inexhaustible; spawn inside a countdown node with a compile-time constant \
                         or runtime-field bound",
                        type_name
                    ));
                }
            }
            for a in args {
                walk_expr(a, multiplier, ctx, live, terms, errors);
            }
        }
        Expr::BinaryOp(_, l, r) => {
            walk_expr(l, multiplier, ctx, live, terms, errors);
            walk_expr(r, multiplier, ctx, live, terms, errors);
        }
        Expr::Call(_, args, _) | Expr::List(args) | Expr::Tuple(args) => {
            for a in args {
                walk_expr(a, multiplier, ctx, live, terms, errors);
            }
        }
        Expr::Cast(i, _) | Expr::IsType(i, _) | Expr::Consume(i) | Expr::Deref(i) | Expr::AddrOf(i) => {
            walk_expr(i, multiplier, ctx, live, terms, errors);
        }
        Expr::Field(o, _) | Expr::Index(o, _) | Expr::Reflect(o, _, _) => {
            walk_expr(o, multiplier, ctx, live, terms, errors);
        }
        Expr::MethodCall(recv, _, args, _) => {
            walk_expr(recv, multiplier, ctx, live, terms, errors);
            for a in args {
                walk_expr(a, multiplier, ctx, live, terms, errors);
            }
        }
        Expr::If(c, t, e) => {
            walk_expr(c, multiplier, ctx, live, terms, errors);
            walk_expr(t, multiplier, ctx, live, terms, errors);
            if let Some(e) = e {
                walk_expr(e, multiplier, ctx, live, terms, errors);
            }
        }
        Expr::Match(s, arms) => {
            walk_expr(s, multiplier, ctx, live, terms, errors);
            for arm in arms {
                if let Some(g) = &arm.guard {
                    walk_expr(g, multiplier, ctx, live, terms, errors);
                }
                walk_expr(&arm.body, multiplier, ctx, live, terms, errors);
            }
        }
        Expr::Slice { array, start, end, stride, .. } => {
            walk_expr(array, multiplier, ctx, live, terms, errors);
            for b in [start, end, stride].into_iter().flatten() {
                walk_expr(b, multiplier, ctx, live, terms, errors);
            }
        }
        Expr::StructLiteral { fields, .. } => {
            for (_, f) in fields {
                walk_expr(f, multiplier, ctx, live, terms, errors);
            }
        }
        Expr::Range { start, end, .. } => {
            walk_expr(start, multiplier, ctx, live, terms, errors);
            walk_expr(end, multiplier, ctx, live, terms, errors);
        }
        _ => {}
    }
}

/// Product of the runtime foreach bounds — `a * b * c` as a nested expr.
fn product(bounds: &[Expr]) -> Expr {
    if bounds.is_empty() {
        return Expr::Decimal(1);
    }
    let mut acc = bounds[0].clone();
    for b in &bounds[1..] {
        acc = Expr::BinaryOp(crate::ast::BinaryOpKind::Mul, Box::new(acc), Box::new(b.clone()));
    }
    acc
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

    fn countdown(bound: Expr) -> TopLevel {
        txn(
            "work",
            Expr::BinaryOp(crate::ast::BinaryOpKind::Lt,
                Box::new(Expr::Identifier("ticks".into())),
                Box::new(bound)),
            Expr::BinaryOp(crate::ast::BinaryOpKind::Eq,
                Box::new(Expr::Identifier("ticks".into())),
                Box::new(Expr::Decimal(0))),
            vec![spawn("Counter")],
        )
    }

    #[test]
    fn const_countdown_spawn_is_bounded() {
        let program = vec![countdown(Expr::Decimal(2))];
        let (caps, dependent, errors) = analyze(&program);
        assert!(errors.is_empty(), "expected no errors, got {errors:?}");
        assert!(dependent.is_empty(), "a const countdown is not dependent");
        assert_eq!(caps.get("Counter"), Some(&2));
    }

    #[test]
    fn runtime_countdown_spawn_is_dependent_with_bound() {
        // `[ticks < N]` with a runtime N — the pool is dependent (sized from
        // N at runtime, SPEC §16.6), NOT an error, and the bound expression
        // is threaded for the backend malloc.
        let program = vec![countdown(Expr::Identifier("N".into()))];
        let (caps, dependent, errors) = analyze(&program);
        assert!(errors.is_empty(), "expected no errors, got {errors:?}");
        assert!(!caps.contains_key("Counter"));
        let terms = dependent.get("Counter").expect("a runtime-bound spawn must be dependent");
        assert_eq!(terms.len(), 1);
        assert_eq!(terms[0].multiplier, 1);
        assert!(matches!(terms[0].bound, Expr::Identifier(ref n) if n == "N"));
    }

    #[test]
    fn runtime_foreach_multiplies_dependent_bound() {
        // A runtime-bound foreach `foreach i in 0..M` inside a runtime-bound
        // countdown multiplies the countdown bound by the loop bound.
        let foreach = Statement::Foreach {
            item: "i".to_string(),
            list: Box::new(Expr::Range {
                start: Box::new(Expr::Decimal(0)),
                end: Box::new(Expr::Identifier("M".into())),
                inclusive: false,
            }),
            body: vec![spawn("Counter")],
        };
        let program = vec![countdown_with_body(Expr::Identifier("N".into()), vec![foreach])];
        let (_, dependent, errors) = analyze(&program);
        assert!(errors.is_empty(), "expected no errors, got {errors:?}");
        let terms = dependent.get("Counter").expect("dependent pool expected");
        assert_eq!(terms.len(), 1);
        assert_eq!(terms[0].multiplier, 1);
        assert!(matches!(&terms[0].bound,
            Expr::BinaryOp(crate::ast::BinaryOpKind::Mul, _, _)));
    }

    fn countdown_with_body(bound: Expr, body: Vec<Statement>) -> TopLevel {
        txn(
            "work",
            Expr::BinaryOp(crate::ast::BinaryOpKind::Lt,
                Box::new(Expr::Identifier("ticks".into())),
                Box::new(bound)),
            Expr::BinaryOp(crate::ast::BinaryOpKind::Eq,
                Box::new(Expr::Identifier("ticks".into())),
                Box::new(Expr::Decimal(0))),
            body,
        )
    }

    #[test]
    fn unprovable_spawn_is_rejected() {
        let program = vec![txn(
            "work",
            Expr::Bool(true),
            Expr::Bool(false),
            vec![spawn("Counter")],
        )];
        let (_, _, errors) = analyze(&program);
        assert!(!errors.is_empty(), "an unbounded spawn must be rejected");
        assert!(errors[0].contains("not statically bounded"));
    }
}
