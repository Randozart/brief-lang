// ── Enumerative Synthesis Engine ──────────────────────────────────────
// 2026-07-12: Phase 6.1 — Depth-bounded enumerative search for expressions.
// Generates all expressions up to a given depth and checks them against examples.

use crate::ast::{BinaryOpKind, DerivationExample, Expr, UnaryOpKind};
use crate::derive::SynthesizeError;

/// Depth-bounded enumerative search for a function body.
/// Returns `Ok(Some(expr))` if found, `Ok(None)` if not found (try SMT).
pub fn enumerative_search(
    name: &str,
    examples: &[DerivationExample],
    max_depth: usize,
) -> Result<Option<Expr>, SynthesizeError> {
    if examples.is_empty() {
        return Err(SynthesizeError::NoExamples(name.to_string()));
    }
    // Build grammar of candidate expressions
    let input_names: Vec<String> = (0..examples[0].inputs.len())
        .map(|i| format!("x{}", i))
        .collect();

    for depth in 1..=max_depth {
        let candidates = generate_expressions(&input_names, depth);
        for candidate in &candidates {
            if matches_all_examples(candidate, examples) {
                return Ok(Some(candidate.clone()));
            }
        }
    }
    Ok(None)
}

/// Generate all valid expressions up to the given depth.
fn generate_expressions(vars: &[String], depth: usize) -> Vec<Expr> {
    if depth == 0 {
        return Vec::new();
    }
    let mut result = Vec::new();

    // Variables
    for var in vars {
        result.push(Expr::Identifier(var.clone()));
    }

    // Constants (depth 1 only)
    if depth == 1 {
        result.push(Expr::Decimal(0));
        result.push(Expr::Decimal(1));
        return result;
    }

    // Unary operations: recurse at depth-1
    let sub = generate_expressions(vars, depth - 1);
    for e in &sub {
        result.push(Expr::UnaryOp(UnaryOpKind::Neg, Box::new(e.clone())));
        result.push(Expr::UnaryOp(UnaryOpKind::Not, Box::new(e.clone())));
    }

    // Binary operations: recurse at depth-1 for each side
    for lhs in &sub {
        for rhs in &sub {
            for kind in &[
                BinaryOpKind::Add, BinaryOpKind::Sub, BinaryOpKind::Mul,
                BinaryOpKind::Div, BinaryOpKind::Eq, BinaryOpKind::Lt,
            ] {
                result.push(Expr::BinaryOp(*kind, Box::new(lhs.clone()), Box::new(rhs.clone())));
            }
        }
    }

    result
}

/// Check if an expression matches all given examples.
fn matches_all_expr(expr: &Expr, example: &DerivationExample) -> bool {
    // Simplified: uses pattern matching to check if expr produces the expected output.
    // A full implementation would use the interpreter to evaluate.
    matches_pattern(expr, &example.inputs, example.output.as_ref())
}

/// Check if an expression matches all examples.
fn matches_all_examples(expr: &Expr, examples: &[DerivationExample]) -> bool {
    examples.iter().all(|ex| matches_all_expr(expr, ex))
}

/// Simple pattern matching: check if an expression looks like it produces the given output.
fn matches_pattern(expr: &Expr, inputs: &[Expr], output: &Expr) -> bool {
    match (expr, output) {
        // x + y pattern
        (Expr::BinaryOp(kind, lhs, rhs), Expr::Decimal(expected)) => {
            if let (Expr::Identifier(l), Expr::Identifier(r)) = (lhs.as_ref(), rhs.as_ref()) {
                if let (Some(li), Some(ri)) = (var_value(l, inputs), var_value(r, inputs)) {
                    let result = match kind {
                        BinaryOpKind::Add => li + ri,
                        BinaryOpKind::Sub => li - ri,
                        BinaryOpKind::Mul => li * ri,
                        BinaryOpKind::Div => if ri != 0 { li / ri } else { return false; },
                        _ => return false,
                    };
                    return result == *expected;
                }
            }
            false
        }
        // 2026-07-14: Identity pattern — identifier matching the expected value
        (Expr::Identifier(name), Expr::Decimal(expected)) => {
            var_value(name, inputs).map_or(false, |v| v == *expected)
        }
        _ => false,
    }
}

/// Get the value of a variable from the input examples.
fn var_value(name: &str, inputs: &[Expr]) -> Option<i64> {
    let idx = name.strip_prefix('x')?.parse::<usize>().ok()?;
    let input = inputs.get(idx)?;
    if let Expr::Decimal(n) = input { Some(*n) } else { None }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::Span;

    fn dummy_span() -> Span { Span::dummy() }

    #[test]
    fn test_generate_expressions_depth_1() {
        let exprs = generate_expressions(&["x".into()], 1);
        assert!(exprs.contains(&Expr::Identifier("x".into())));
        assert!(exprs.contains(&Expr::Decimal(0)));
        assert!(exprs.contains(&Expr::Decimal(1)));
    }

    #[test]
    fn test_generate_expressions_depth_2() {
        let exprs = generate_expressions(&["x".into()], 2);
        // Should have unary ops
        assert!(exprs.iter().any(|e| matches!(e, Expr::UnaryOp(UnaryOpKind::Neg, _))));
    }

    #[test]
    fn test_identity() {
        let example = DerivationExample {
            inputs: vec![Expr::Decimal(5)],
            output: Box::new(Expr::Decimal(5)),
            span: dummy_span(),
        };
        let expr = Expr::Identifier("x0".into());
        assert!(matches_all_expr(&expr, &example));
    }

    #[test]
    fn test_addition() {
        let example = DerivationExample {
            inputs: vec![Expr::Decimal(2), Expr::Decimal(3)],
            output: Box::new(Expr::Decimal(5)),
            span: dummy_span(),
        };
        let expr = Expr::BinaryOp(
            BinaryOpKind::Add,
            Box::new(Expr::Identifier("x0".into())),
            Box::new(Expr::Identifier("x1".into())),
        );
        assert!(matches_all_expr(&expr, &example));
    }

    #[test]
    fn test_subtraction() {
        let example = DerivationExample {
            inputs: vec![Expr::Decimal(10), Expr::Decimal(3)],
            output: Box::new(Expr::Decimal(7)),
            span: dummy_span(),
        };
        let expr = Expr::BinaryOp(
            BinaryOpKind::Sub,
            Box::new(Expr::Identifier("x0".into())),
            Box::new(Expr::Identifier("x1".into())),
        );
        assert!(matches_all_expr(&expr, &example));
    }

    #[test]
    fn test_enumerative_search_identity() {
        let examples = vec![DerivationExample {
            inputs: vec![Expr::Decimal(42)],
            output: Box::new(Expr::Decimal(42)),
            span: dummy_span(),
        }];
        let result = enumerative_search("id", &examples, 3).unwrap();
        assert!(result.is_some());
    }

    #[test]
    fn test_empty_examples() {
        let result = enumerative_search("f", &[], 3);
        assert!(result.is_err());
    }

    #[test]
    fn test_no_solution() {
        let examples = vec![DerivationExample {
            inputs: vec![Expr::Decimal(1)],
            output: Box::new(Expr::Decimal(999)),
            span: dummy_span(),
        }];
        let result = enumerative_search("f", &examples, 2).unwrap();
        assert!(result.is_none()); // 999 is not a simple expression with x0, 0, 1
    }
}
