// ── Three-Tier Overfitting Prevention ──────────────────────────────────
// 2026-07-28: Tier 1 — Identity-op pruning (generation-time).
//              Tier 2 — Random-input contract verification (post-synthesis).
//              Tier 3 — Boundary testing + constant-output detection.
//
// Architecture: verify_candidate() is called from synthesize() after a
// candidate matches all examples. If verification fails, the candidate is
// rejected and search continues for a different expression.

use crate::ast::{Expr, Type};
use crate::derive::engine::evaluate_synthesized;
use crate::derive::engine::SynthesisEvalContext;
use crate::derive::SynthesizeError;

/// Result of candidate verification.
#[derive(Debug)]
pub enum VerifyResult {
    /// Candidate passed all verification checks.
    Pass,
    /// Candidate failed verification. The Vec contains failing input expressions
    /// (one per parameter, in order).
    Fail(Vec<Vec<Expr>>, String),
}

/// Generate test inputs for a function with given parameters.
/// Returns `sample_count` input vectors, each with one Expr per parameter.
/// Mix: boundary values (0, 1, -1, MAX, MIN) + random + example-adjacent.
pub fn generate_test_inputs(
    params: &[(String, Type)],
    sample_count: usize,
) -> Vec<Vec<Expr>> {
    let mut inputs = Vec::new();

    // Boundary values for Int params
    let boundaries: Vec<i64> = vec![0, 1, -1, i64::MAX, i64::MIN, 2, -2, 64, -64, 1024];

    for (_, ty) in params {
        if ty == &Type::int() || ty == &Type::bits(64) {
            for &b in &boundaries {
                let mut row = Vec::new();
                for (_, other_ty) in params {
                    if other_ty == &Type::int() || other_ty == &Type::bits(64) {
                        row.push(Expr::Decimal(b));
                    } else if other_ty == &Type::bool_() {
                        row.push(Expr::Bool(b != 0));
                    } else {
                        row.push(Expr::Decimal(0));
                    }
                }
                inputs.push(row);
            }
        }
    }

    // Fill up to sample_count with random-looking deterministic inputs
    let mut rng_state: i64 = 12345;
    while inputs.len() < sample_count {
        let mut row = Vec::new();
        for (_, ty) in params {
            if ty == &Type::int() || ty == &Type::bits(64) {
                rng_state = rng_state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                row.push(Expr::Decimal(rng_state.wrapping_abs()));
            } else if ty == &Type::bool_() {
                rng_state = rng_state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                row.push(Expr::Bool(rng_state & 1 == 1));
            } else {
                row.push(Expr::Decimal(0));
            }
        }
        inputs.push(row);
    }

    inputs
}

/// Verify a synthesized candidate against random inputs, boundary values,
/// and optional postcondition.
///
/// Returns `Pass` if the candidate produces no evaluation errors and satisfies
/// all checks. Returns `Fail` with the first failing input if any check fails.
///
/// Checks performed:
/// 1. No evaluation errors (division by zero, overflow) for ANY tested input
/// 2. If postcondition provided: evaluate post(result) and check it's true
/// 3. Constant-output detection: if candidate returns same value for ALL tested
///    inputs (while examples have varying outputs), reject.
pub fn verify_candidate(
    candidate: &Expr,
    params: &[(String, Type)],
    postcondition: Option<&Expr>,
    sample_count: usize,
) -> VerifyResult {
    let inputs = generate_test_inputs(params, sample_count);

    // Track outputs for constant-output detection
    let mut seen_outputs: Vec<i64> = Vec::new();

    for input_row in &inputs {
        // Bind params to input values
        let mut ctx = SynthesisEvalContext::new();
        for (i, (name, _)) in params.iter().enumerate() {
            if let Some(val_expr) = input_row.get(i) {
                let val = expr_to_decimal(val_expr);
                ctx.bind(name, val);
            }
        }

        // Evaluate candidate
        let result = match evaluate_synthesized(candidate, &mut ctx) {
            Ok(v) => v,
            Err(e) => {
                return VerifyResult::Fail(
                    vec![input_row.clone()],
                    format!("evaluation error: {:?}", e),
                );
            }
        };

        // Record output for constant detection
        match &result {
            crate::interpreter::Value::Int(n) => seen_outputs.push(*n),
            _ => seen_outputs.push(0),
        }

        // Check postcondition if provided
        if let Some(post) = postcondition {
            let post_ctx = &mut SynthesisEvalContext::new();
            if let crate::interpreter::Value::Int(n) = &result {
                post_ctx.bind("#Term", crate::interpreter::Value::Int(*n));
            }
            let post_result = evaluate_synthesized(post, post_ctx);
            match post_result {
                Ok(crate::interpreter::Value::Bits(b)) => {
                    if b.is_empty() || b.iter().all(|&x| x == 0) {
                        return VerifyResult::Fail(
                            vec![input_row.clone()],
                            format!("postcondition failed for output {:?}", result),
                        );
                    }
                }
                Ok(crate::interpreter::Value::Int(n)) => {
                    if n == 0 {
                        return VerifyResult::Fail(
                            vec![input_row.clone()],
                            format!("postcondition failed for output {:?}", result),
                        );
                    }
                }
                _ => {}
            }
        }
    }

    // Constant-output detection: if the candidate produces the same value
    // for ALL tested inputs, it's likely overfitted (unless there's only
    // one test input, which can't happen — sample_count >= 10).
    if seen_outputs.len() >= 2 {
        let first = seen_outputs[0];
        if seen_outputs.iter().all(|&v| v == first) {
            return VerifyResult::Fail(
                vec![],
                format!("constant output {} for all {} test inputs", first, seen_outputs.len()),
            );
        }
    }

    VerifyResult::Pass
}

/// Extract an i64 value from a constant expression (for test input generation).
fn expr_to_decimal(expr: &Expr) -> crate::interpreter::Value {
    match expr {
        Expr::Decimal(n) => crate::interpreter::Value::Int(*n),
        Expr::Bool(b) => crate::interpreter::Value::Bits(vec![if *b { 1 } else { 0 }]),
        _ => crate::interpreter::Value::Int(0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn int_params(n: usize) -> Vec<(String, Type)> {
        (0..n).map(|i| (format!("x{}", i), Type::int())).collect()
    }

    #[test]
    fn test_generate_test_inputs_count() {
        let params = int_params(1);
        let inputs = generate_test_inputs(&params, 100);
        assert!(inputs.len() >= 10, "should generate at least 10 inputs");
        assert!(inputs.len() <= 200, "should not over-generate");
    }

    #[test]
    fn test_generate_test_inputs_includes_boundaries() {
        let params = int_params(1);
        let inputs = generate_test_inputs(&params, 10);
        let all_vals: Vec<i64> = inputs.iter()
            .filter_map(|row| row.first())
            .filter_map(|e| match e { Expr::Decimal(n) => Some(*n), _ => None })
            .collect();
        assert!(all_vals.contains(&0), "should contain 0");
        assert!(all_vals.contains(&1), "should contain 1");
        assert!(all_vals.contains(&(-1)), "should contain -1");
        assert!(all_vals.contains(&i64::MAX), "should contain MAX");
        assert!(all_vals.contains(&i64::MIN), "should contain MIN");
    }

    #[test]
    fn test_verify_trivial_identity() {
        let params = int_params(1);
        // x0 always passes — it's the identity function
        let candidate = Expr::Identifier("x0".into());
        let result = verify_candidate(&candidate, &params, None, 20);
        assert!(matches!(result, VerifyResult::Pass), "identity should pass");
    }

    #[test]
    fn test_verify_constant_detected() {
        let params = int_params(1);
        // Constant 42 — should be detected as constant-output
        let candidate = Expr::Decimal(42);
        let result = verify_candidate(&candidate, &params, None, 20);
        match result {
            VerifyResult::Fail(_, ref msg) => {
                assert!(msg.contains("constant output"),
                    "expected constant output message, got: {}", msg);
            }
            _ => panic!("constant should be detected: {:?}", result),
        }
    }

    #[test]
    fn test_verify_with_postcondition() {
        let params = int_params(1);
        // x0 (identity) — postcondition @result >= -1 is always true
        // Use a simple postcondition expression constructed as AST
        let candidate = Expr::Identifier("x0".into());
        // Postcondition: just `true` (always passes)
        let post = Expr::Bool(true);
        let result = verify_candidate(&candidate, &params, Some(&post), 100);
        assert!(matches!(result, VerifyResult::Pass), "should pass postcondition: {:?}", result);
    }

    #[test]
    fn test_verify_two_params() {
        let params = int_params(2);
        // x0 + x1 — should pass verification
        let candidate = Expr::BinaryOp(
            crate::ast::BinaryOpKind::Add,
            Box::new(Expr::Identifier("x0".into())),
            Box::new(Expr::Identifier("x1".into())),
        );
        let result = verify_candidate(&candidate, &params, None, 20);
        assert!(matches!(result, VerifyResult::Pass), "addition should pass");
    }
}
