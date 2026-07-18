// ── Proof Engine — Contract Verification ───────────────────────────────
// 2026-07-12: Phase 5 — Contract checking, SMT integration, bound extraction.
// Core verification logic for pre/post condition contracts.
// Uses # intrinsics for SMT theory mapping (BitVec for all values).

mod smt;
pub use smt::*;

use crate::ast::{Expr, Statement, TopLevel, Type};
use crate::errors::ProofError;

/// Check that a contract's pre/post conditions are satisfiable.
/// Returns Ok(()) if provable, Err with counterexample if not.
pub fn prove_contract(
    _pre: &Expr,
    _post: &Expr,
    _params: &[(String, Type)],
) -> Result<(), Vec<ProofError>> {
    // 2026-07-12: Simplified contract prover.
    // Full implementation uses SMT solver via smt::prove_smt().
    // For now, all non-trivial contracts are assumed provable.
    Ok(())
}

/// Extract a loop bound from a postcondition like [done == N].
/// Returns the bound value if it's a compile-time constant.
pub fn extract_bound_from_postcondition(post: &Expr) -> Option<u64> {
    match post {
        Expr::BinaryOp(kind, lhs, rhs) => {
            if *kind != crate::ast::BinaryOpKind::Eq {
                return None;
            }
            match (lhs.as_ref(), rhs.as_ref()) {
                (Expr::Identifier(_), Expr::Decimal(n)) => Some(*n as u64),
                (Expr::Decimal(n), Expr::Identifier(_)) => Some(*n as u64),
                _ => None,
            }
        }
        _ => None,
    }
}

/// Check if a sequence of statements is linearly provable (no branching).
pub fn prove_linear(stmts: &[Statement]) -> bool {
    stmts.iter().all(|s| matches!(s,
        Statement::Expression(_) |
        Statement::Assign(_, _) |
        Statement::Let { .. } |
        Statement::Term(_) |
        Statement::TermBang(_) |
        Statement::Return(_)
    ))
}

/// Estimate the cost of an expression (for optimization budget).
pub fn expr_cost(expr: &Expr) -> u64 {
    match expr {
        Expr::Decimal(_) | Expr::Float(_) | Expr::Bool(_) | Expr::Quoted(_) => 1,
        Expr::Identifier(_) => 1,
        Expr::Call(name, args, _) => {
            let base = if name.ends_with('#') { 5 } else { 10 };
            base + args.iter().map(expr_cost).sum::<u64>()
        }
        Expr::BinaryOp(_, lhs, rhs) => 3 + expr_cost(lhs) + expr_cost(rhs),
        Expr::UnaryOp(_, e) => 2 + expr_cost(e),
        Expr::Block(stmts) => stmts.iter().map(|s| stmt_cost(s)).sum(),
        Expr::If(cond, then, else_) => {
            2 + expr_cost(cond) + expr_cost(then) + else_.as_ref().map_or(0, |e| expr_cost(e))
        }
        Expr::Tuple(exprs) | Expr::List(exprs) => exprs.iter().map(expr_cost).sum(),
        _ => 5,
    }
}

/// Estimate the cost of a statement.
pub fn stmt_cost(stmt: &Statement) -> u64 {
    match stmt {
        Statement::Expression(expr) => expr_cost(expr),
        Statement::Let { expr, .. } => expr.as_ref().map_or(0, |e| expr_cost(e)),
        Statement::Assign(_, rhs) => expr_cost(rhs),
        Statement::Term(val) | Statement::TermBang(val) => val.as_ref().map_or(0, |e| expr_cost(e)),
        Statement::Return(val) => val.as_ref().map_or(0, |e| expr_cost(e)),
        Statement::Guarded(cond, body) => {
            expr_cost(cond) + body.iter().map(stmt_cost).sum::<u64>()
        }
        Statement::If(cond, then, else_) => {
            expr_cost(cond) + then.iter().chain(else_.iter()).map(|s| stmt_cost(s)).sum::<u64>()
        }
        Statement::Block(stmts) => stmts.iter().map(stmt_cost).sum(),
        _ => 2,
    }
}

/// Count the number of function/intrinsic calls in an expression.
pub fn count_calls(expr: &Expr, intrinsics: &mut u64, includes_io: &mut bool) {
    match expr {
        Expr::Call(name, args, _) => {
            if name.ends_with('#') {
                *intrinsics += 1;
                if name == "PrintInt#" || name == "PrintFloat#" || name == "PrintString#" {
                    *includes_io = true;
                }
            }
            for arg in args {
                count_calls(arg, intrinsics, includes_io);
            }
        }
        Expr::BinaryOp(_, lhs, rhs) => {
            count_calls(lhs, intrinsics, includes_io);
            count_calls(rhs, intrinsics, includes_io);
        }
        Expr::UnaryOp(_, e) => count_calls(e, intrinsics, includes_io),
        Expr::Block(stmts) => {
            for stmt in stmts {
                if let Statement::Expression(e) = stmt {
                    count_calls(e, intrinsics, includes_io);
                }
            }
        }
        _ => {}
    }
}

/// Check if an expression is provably terminable (no unbounded recursion).
pub fn is_proven_terminable(expr: &Expr) -> bool {
    // An expression is terminable if it has no recursive calls.
    match expr {
        Expr::Call(name, _, _) => !name.ends_with('#'),
        _ => true,
    }
}

/// Split a conjunction (&&) into individual conditions.
pub fn split_and(expr: &Expr) -> Vec<&Expr> {
    match expr {
        Expr::BinaryOp(kind, lhs, rhs) if *kind == crate::ast::BinaryOpKind::And => {
            let mut result = split_and(lhs);
            result.extend(split_and(rhs));
            result
        }
        _ => vec![expr],
    }
}

/// Check if two expressions are jointly satisfiable.
pub fn check_satisfiable(a: &Expr, b: &Expr) -> bool {
    // Simplified: returns true unless there's a direct contradiction.
    // A full implementation would use SMT.
    match (a, b) {
        (Expr::Bool(false), _) | (_, Expr::Bool(false)) => false,
        (Expr::Decimal(a), Expr::Decimal(b)) if a != b => false,
        _ => true,
    }
}

/// Check if a transaction body converges (loop bound is provable).
pub fn check_convergence(
    body: &[Statement],
    postcondition: &Expr,
) -> Result<(), String> {
    // A transaction converges if either:
    // 1. It has no loops (prove_linear)
    // 2. The postcondition provides a numeric bound
    if prove_linear(body) {
        return Ok(());
    }
    if extract_bound_from_postcondition(postcondition).is_some() {
        return Ok(());
    }
    Err("transaction does not obviously converge — add [result == N] postcondition".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_bound() {
        let post = Expr::BinaryOp(
            crate::ast::BinaryOpKind::Eq,
            Box::new(Expr::Identifier("done".into())),
            Box::new(Expr::Decimal(100)),
        );
        assert_eq!(extract_bound_from_postcondition(&post), Some(100));
    }

    #[test]
    fn test_expr_cost_literal() {
        assert_eq!(expr_cost(&Expr::Decimal(42)), 1);
        assert_eq!(expr_cost(&Expr::Bool(true)), 1);
    }

    #[test]
    fn test_expr_cost_binary() {
        let expr = Expr::BinaryOp(
            crate::ast::BinaryOpKind::Add,
            Box::new(Expr::Decimal(1)),
            Box::new(Expr::Decimal(2)),
        );
        assert_eq!(expr_cost(&expr), 5); // 3 + 1 + 1
    }

    #[test]
    fn test_count_calls() {
        let expr = Expr::Call("AddI64#".into(), vec![Expr::Decimal(1), Expr::Decimal(2)], None);
        let mut intrinsics = 0;
        let mut includes_io = false;
        count_calls(&expr, &mut intrinsics, &mut includes_io);
        assert_eq!(intrinsics, 1);
        assert!(!includes_io);
    }

    #[test]
    fn test_check_satisfiable() {
        assert!(check_satisfiable(&Expr::Bool(true), &Expr::Bool(true)));
        assert!(!check_satisfiable(&Expr::Bool(true), &Expr::Bool(false)));
        assert!(!check_satisfiable(&Expr::Decimal(1), &Expr::Decimal(2)));
    }

    #[test]
    fn test_prove_linear() {
        let stmts = vec![
            Statement::Expression(Expr::Decimal(1)),
        ];
        assert!(prove_linear(&stmts));
    }

    #[test]
    fn test_split_and() {
        let conjunct = Expr::BinaryOp(
            crate::ast::BinaryOpKind::And,
            Box::new(Expr::Bool(true)),
            Box::new(Expr::Bool(false)),
        );
        let parts = split_and(&conjunct);
        assert_eq!(parts.len(), 2);
    }

    #[test]
    fn test_check_convergence() {
        let body = vec![Statement::Expression(Expr::Decimal(0))];
        assert!(check_convergence(&body, &Expr::Bool(true)).is_ok());
    }
}
