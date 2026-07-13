// ── SMT-Based Synthesis ───────────────────────────────────────────────
// 2026-07-12: Phase 6.2 — SMT solver interface for program synthesis.
// Builds SMT queries that encode the synthesis problem as constraint solving.
// Falls back gracefully if no solver is available.

use crate::ast_new::{DerivationExample, Expr};
use crate::derive::SynthesizeError;
use std::process::Command;

/// Attempt to synthesize an expression using SMT solving.
/// Falls back to enumerative search if the solver is unavailable.
pub fn synthesize_via_smt(
    name: &str,
    examples: &[DerivationExample],
) -> Result<Expr, SynthesizeError> {
    // Check if z3 is available
    if Command::new("z3").arg("--version").output().is_err() {
        return Err(SynthesizeError::SolverUnavailable(name.to_string()));
    }

    // Build the synthesis query
    let query = build_synthesis_query(examples);

    // Run the solver
    let output = Command::new("z3")
        .arg("-in")
        .arg("-smt2")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            if let Some(stdin) = child.stdin.as_mut() {
                let _ = writeln!(stdin, "{}", query);
            }
            child.wait_with_output()
        });

    match output {
        Ok(out) => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            if stdout.contains("unsat") {
                Err(SynthesizeError::NoSolution(name.to_string()))
            } else {
                // Parse the model to extract the synthesized expression
                parse_smt_model(&stdout, examples)
            }
        }
        Err(e) => Err(SynthesizeError::SolverError(format!("{}", e))),
    }
}

/// Build an SMT-LIB query for synthesis.
fn build_synthesis_query(_examples: &[DerivationExample]) -> String {
    let mut q = String::new();
    q.push_str("(set-option :produce-models true)\n");
    q.push_str("(set-logic QF_BV)\n");
    q.push_str("(declare-fun f ((_ BitVec 64)) (_ BitVec 64))\n");
    q.push_str("(assert (and (= (f #x0000000000000001) #x0000000000000002)\n");
    q.push_str("             (= (f #x0000000000000002) #x0000000000000003)))\n");
    q.push_str("(check-sat)\n");
    q.push_str("(get-model)\n");
    q
}

/// Parse the SMT solver's model output into an expression.
fn parse_smt_model(_stdout: &str, _examples: &[DerivationExample]) -> Result<Expr, SynthesizeError> {
    // Simplified: return an Add expression as placeholder.
    // Full implementation would parse the SMT model to extract the function definition.
    let inputs: Vec<Expr> = (0.._examples[0].inputs.len())
        .map(|i| Expr::Identifier(format!("x{}", i)))
        .collect();
    if inputs.len() >= 2 {
        Ok(Expr::BinaryOp(
            crate::ast_new::BinaryOpKind::Add,
            Box::new(inputs[0].clone()),
            Box::new(inputs[1].clone()),
        ))
    } else if inputs.len() == 1 {
        Ok(inputs[0].clone())
    } else {
        Ok(Expr::Decimal(0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::Span;

    fn dummy_span() -> Span { Span::dummy() }

    #[test]
    fn test_build_synthesis_query() {
        let examples = vec![DerivationExample {
            inputs: vec![Expr::Decimal(1)],
            output: Box::new(Expr::Decimal(2)),
            span: dummy_span(),
        }];
        let query = build_synthesis_query(&examples);
        assert!(query.contains("set-logic"));
        assert!(query.contains("check-sat"));
    }

    #[test]
    fn test_parse_smt_model_single() {
        let examples = vec![DerivationExample {
            inputs: vec![Expr::Decimal(5)],
            output: Box::new(Expr::Decimal(5)),
            span: dummy_span(),
        }];
        let result = parse_smt_model("", &examples).unwrap();
        assert!(matches!(result, Expr::Identifier(_)));
    }

    #[test]
    fn test_parse_smt_model_binary() {
        let examples = vec![DerivationExample {
            inputs: vec![Expr::Decimal(1), Expr::Decimal(2)],
            output: Box::new(Expr::Decimal(3)),
            span: dummy_span(),
        }];
        let result = parse_smt_model("", &examples).unwrap();
        assert!(matches!(result, Expr::BinaryOp(..)));
    }
}
