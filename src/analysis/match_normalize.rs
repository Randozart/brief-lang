// ── Match → When Normalization ─────────────────────────────────
//
// 2026-07-31: Normalize statement-level `match` into a sequence of
// `when` guards so the composite-node decomposition pass (§11 of
// docs/plans/2026-07-30-flat-node-decomposition.md) handles only one
// conditional construct.
//
// A statement-level match:
//
//   match x { 0 => { A } 1 => { B } _ => { C } }
//
// normalizes to (Brief's `when` has first-match-wins semantics):
//
//   when x == 0 { A };
//   when x == 1 { B };
//   when !(x == 0 || x == 1) { C };
//
// The fallback (wildcard) becomes the negation of ALL other arm
// predicates — NEVER `when true`, which would be indistinguishable
// from an unconditional block to the static predicate analysis.

use crate::ast::{BinaryOpKind, Expr, Statement, StmtMatchPattern};

/// Replace every statement-level `Statement::Match` in `body` with a
/// sequence of `Statement::Guarded` (`when`) forms.
///
/// Non-match statements pass through unchanged. Nested statements inside
/// other constructs (guards, blocks, if bodies) are NOT normalized here —
/// they are handled by the recursive decomposition in later phases.
pub fn normalize_match_to_when(body: Vec<Statement>) -> Vec<Statement> {
    let mut out: Vec<Statement> = Vec::with_capacity(body.len());
    for stmt in body {
        match stmt {
            Statement::Match { expr, arms } => {
                out.extend(expand_match(&expr, &arms));
            }
            other => out.push(other),
        }
    }
    out
}

/// Expand one statement-level match into a sequence of `when` guards.
fn expand_match(scrutinee: &Expr, arms: &[crate::ast::StmtMatchArm]) -> Vec<Statement> {
    // Build a condition for each non-wildcard arm.
    // 2026-07-31: conditions are compared against the scrutinee at the
    // match site — the split point semantics of the version-DAG apply.
    let mut guards: Vec<Statement> = Vec::new();
    let mut fallback: Option<&Vec<Statement>> = None;
    let mut arm_conditions: Vec<Expr> = Vec::new();

    for arm in arms {
        match &arm.pattern {
            StmtMatchPattern::Wildcard => {
                // Assume the wildcard is the standard trailing fallback.
                fallback = Some(&arm.body);
            }
            pattern => {
                let cond = pattern_condition(scrutinee, pattern);
                arm_conditions.push(cond.clone());
                guards.push(Statement::Guarded(cond, arm.body.clone()));
            }
        }
    }

    // Emit the fallback as the negation of ALL non-wildcard conditions.
    // 2026-07-31: `when !(c1 || c2 || ...)` — precise mutual exclusion.
    // Never `when true`.
    if let Some(fb) = fallback {
        if arm_conditions.is_empty() {
            // Only a wildcard: the match is unconditional.
            guards.push(Statement::Guarded(Expr::Bool(true), fb.clone()));
        } else {
            let mut negated = arm_conditions.into_iter();
            let first = negated.next().expect("non-empty");
            let or: Expr = negated.fold(first, |acc, c| {
                Expr::BinaryOp(BinaryOpKind::Or, Box::new(acc), Box::new(c))
            });
            guards.push(Statement::Guarded(
                Expr::UnaryOp(crate::ast::UnaryOpKind::Not, Box::new(or)),
                fb.clone(),
            ));
        }
    }

    guards
}

/// Build the boolean condition for one non-wildcard match pattern.
fn pattern_condition(scrutinee: &Expr, pattern: &StmtMatchPattern) -> Expr {
    match pattern {
        StmtMatchPattern::Literal(n) => Expr::BinaryOp(
            BinaryOpKind::Eq,
            Box::new(scrutinee.clone()),
            // 2026-07-31: StmtMatchPattern::Literal is i128; Expr::Decimal is i64.
            // The parser produces match literals from integer source that fit i64.
            Box::new(Expr::Decimal((*n).try_into().unwrap_or(i64::MAX))),
        ),
        StmtMatchPattern::String(s) => Expr::BinaryOp(
            BinaryOpKind::Eq,
            Box::new(scrutinee.clone()),
            Box::new(Expr::Quoted(s.as_bytes().to_vec())),
        ),
        StmtMatchPattern::Multi(patterns) => {
            // `0x30 | 0x31 => body` — the arm fires if any sub-pattern matches.
            let mut it = patterns.iter();
            let first = it.next().expect("Multi must be non-empty");
            let first_cond = pattern_condition(scrutinee, first);
            it.fold(first_cond, |acc, p| {
                let c = pattern_condition(scrutinee, p);
                Expr::BinaryOp(BinaryOpKind::Or, Box::new(acc), Box::new(c))
            })
        }
        StmtMatchPattern::Wildcard => Expr::Bool(true),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Expr, Statement, StmtMatchArm, StmtMatchPattern};

    fn arm(pattern: StmtMatchPattern, body: Vec<Statement>) -> StmtMatchArm {
        StmtMatchArm { pattern, body }
    }

    #[test]
    fn test_match_literal_arms() {
        let body = vec![Statement::Match {
            expr: Box::new(Expr::Identifier("x".to_string())),
            arms: vec![
                arm(StmtMatchPattern::Literal(0), vec![]),
                arm(StmtMatchPattern::Literal(1), vec![]),
                arm(StmtMatchPattern::Wildcard, vec![]),
            ],
        }];
        let out = normalize_match_to_when(body);
        assert_eq!(out.len(), 3);
        match &out[0] {
            Statement::Guarded(Expr::BinaryOp(BinaryOpKind::Eq, l, r), _) => {
                assert!(matches!(**l, Expr::Identifier(_)));
                assert!(matches!(**r, Expr::Decimal(0)));
            }
            _ => panic!("expected when x == 0"),
        }
        // Fallback must be a negation (NOT an OR), never a bare `when true`.
        match &out[2] {
            Statement::Guarded(Expr::UnaryOp(crate::ast::UnaryOpKind::Not, inner), _) => {
                assert!(matches!(**inner, Expr::BinaryOp(BinaryOpKind::Or, _, _)));
            }
            _ => panic!("expected when !(x == 0 || x == 1)"),
        }
    }

    #[test]
    fn test_match_single_wildcard() {
        // Only a wildcard — the match is unconditional.
        let body = vec![Statement::Match {
            expr: Box::new(Expr::Identifier("x".to_string())),
            arms: vec![arm(StmtMatchPattern::Wildcard, vec![])],
        }];
        let out = normalize_match_to_when(body);
        assert_eq!(out.len(), 1);
        match &out[0] {
            Statement::Guarded(Expr::Bool(true), _) => {}
            _ => panic!("expected when true for single wildcard"),
        }
    }

    #[test]
    fn test_match_multi_pattern() {
        let body = vec![Statement::Match {
            expr: Box::new(Expr::Identifier("x".to_string())),
            arms: vec![arm(
                StmtMatchPattern::Multi(vec![
                    StmtMatchPattern::Literal(48),
                    StmtMatchPattern::Literal(49),
                ]),
                vec![],
            )],
        }];
        let out = normalize_match_to_when(body);
        assert_eq!(out.len(), 1);
        match &out[0] {
            Statement::Guarded(Expr::BinaryOp(BinaryOpKind::Or, _, _), _) => {}
            _ => panic!("expected when (x == 48 || x == 49)"),
        }
    }

    #[test]
    fn test_non_match_passthrough() {
        let body = vec![Statement::Assign(
            Expr::Identifier("y".to_string()),
            Expr::Decimal(1),
        )];
        let out = normalize_match_to_when(body);
        assert_eq!(out.len(), 1);
        assert!(matches!(&out[0], Statement::Assign(_, _)));
    }
}
