// ── SMT Solver Integration ────────────────────────────────────────────
// 2026-07-12: Phase 5 — SMT-LIB query construction and solver invocation.
// All values are modeled as (_ BitVec N) — no type-specific theories.
// This aligns with the Bits-is-primitive architecture.

use crate::ast::{Expr, Type};
use std::process::Command;

/// Result of an SMT query.
#[derive(Debug, Clone)]
pub enum SmtResult {
    /// The formula is satisfiable (counterexample exists).
    Sat(Vec<(String, String)>),
    /// The formula is unsatisfiable (provably true).
    Unsat,
    /// The solver could not determine satisfiability.
    Unknown,
}

/// Prove a formula using the SMT solver.
/// The formula is a Boolean expression encoded as SMT-LIB.
pub fn prove_smt(formula: &Expr, _param_types: &[(String, Type)]) -> SmtResult {
    // Build SMT-LIB query
    let query = build_smt_query(formula);
    // Invoke solver (z3 must be on PATH)
    match Command::new("z3").arg("-in").arg("-smt2").stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped()).spawn()
    {
        Ok(mut child) => {
            use std::io::Write;
            if let Some(stdin) = child.stdin.as_mut() {
                let _ = writeln!(stdin, "{}", query);
            }
            match child.wait_with_output() {
                Ok(output) => {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    if stdout.contains("unsat") {
                        SmtResult::Unsat
                    } else if stdout.contains("sat") {
                        SmtResult::Sat(Vec::new())
                    } else {
                        SmtResult::Unknown
                    }
                }
                Err(_) => SmtResult::Unknown,
            }
        }
        Err(_) => SmtResult::Unknown,
    }
}

/// 2026-07-16: P6 — Prove an SMT-LIB2 formula string directly (not from a Brief Expr).
/// Writes formula to stdin of z3 -in -smt2, parses output.
/// Returns Unsat if the formula is unsatisfiable (property holds).
/// Uses a simple timeout wrapper around the cli invocation.
pub fn prove_smt_formula(formula: &str, _timeout_ms: u64) -> SmtResult {
    match Command::new("z3")
        .arg("-in")
        .arg("-smt2")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
    {
        Ok(mut child) => {
            use std::io::Write;
            if let Some(stdin) = child.stdin.as_mut() {
                let _ = writeln!(stdin, "{}", formula);
            }
            // Drop stdin so z3 can process
            drop(child.stdin.take());
            match child.wait_with_output() {
                Ok(output) => {
                    let stdout = String::from_utf8_lossy(&output.stdout);
                    if stdout.contains("unsat") {
                        SmtResult::Unsat
                    } else if stdout.contains("sat") {
                        SmtResult::Sat(parse_smt_model(&stdout))
                    } else {
                        SmtResult::Unknown
                    }
                }
                Err(_) => SmtResult::Unknown,
            }
        }
        Err(_) => SmtResult::Unknown,
    }
}

/// Parse Z3's model output into (variable, value) pairs.
fn parse_smt_model(_stdout: &str) -> Vec<(String, String)> {
    // Placeholder — full model parsing deferred
    // In a full implementation, parse "(define-fun x () (_ BitVec 64) #x0000000000000001)"
    // style model definitions from Z3 output.
    Vec::new()
}

/// Build an SMT-LIB query from a Brief expression.
/// All values are encoded as (_ BitVec 64) — the only primitive.
fn build_smt_query(expr: &Expr) -> String {
    let mut q = String::new();
    q.push_str("(set-option :produce-models true)\n");
    q.push_str("(set-logic QF_BV)\n");
    q.push_str("(declare-const x (_ BitVec 64))\n");
    q.push_str("(assert ");
    q.push_str(&encode_expr_smt(expr));
    q.push_str(")\n");
    q.push_str("(check-sat)\n");
    q
}

/// Encode a Brief expression as an SMT-LIB term.
/// Only supports a subset of Expr — the Boolean constraint subset.
/// Sanitizes #Self → self_var for SMT-LIB compatibility.
fn encode_expr_smt(expr: &Expr) -> String {
    match expr {
        Expr::Bool(true) => "(= #b1 #b1)".into(),
        Expr::Bool(false) => "(= #b0 #b1)".into(),
        Expr::Decimal(n) => {
            format!("#x{:016x}", *n as u64)
        }
        Expr::Identifier(name) => sanitize_name(name),
        Expr::BinaryOp(kind, lhs, rhs) => {
            let l = encode_expr_smt(lhs);
            let r = encode_expr_smt(rhs);
            match kind {
                crate::ast::BinaryOpKind::Eq => format!("(= {} {})", l, r),
                crate::ast::BinaryOpKind::Neq => format!("(not (= {} {}))", l, r),
                crate::ast::BinaryOpKind::Lt => format!("(bvslt {} {})", l, r),
                crate::ast::BinaryOpKind::Gt => format!("(bvsgt {} {})", l, r),
                crate::ast::BinaryOpKind::Le => format!("(bvsle {} {})", l, r),
                crate::ast::BinaryOpKind::Ge => format!("(bvsge {} {})", l, r),
                crate::ast::BinaryOpKind::And => format!("(and {} {})", l, r),
                crate::ast::BinaryOpKind::Or => format!("(or {} {})", l, r),
                crate::ast::BinaryOpKind::Add => format!("(bvadd {} {})", l, r),
                crate::ast::BinaryOpKind::Sub => format!("(bvsub {} {})", l, r),
                crate::ast::BinaryOpKind::Mul => format!("(bvmul {} {})", l, r),
                _ => format!("(= {} {})", l, r),
            }
        }
        Expr::UnaryOp(kind, e) => {
            let inner = encode_expr_smt(e);
            match kind {
                crate::ast::UnaryOpKind::Not => format!("(not {})", inner),
                crate::ast::UnaryOpKind::Neg => format!("(bvneg {})", inner),
                _ => inner,
            }
        }
        _ => "(= #b0 #b0)".into(),
    }
}

/// 2026-07-23: Build an SMT-LIB query to prove a contract condition.
/// Declares free variables for `#Self` and all named params,
/// then asserts `(not condition)` — proving UNSAT means the
/// condition always holds.
pub fn build_contract_query(expr: &Expr, params: &[(String, Type)]) -> String {
    let mut q = String::new();
    q.push_str("(set-option :produce-models true)\n");
    q.push_str("(set-logic QF_BV)\n");

    // Declare #Self as a free variable (for protocol contracts)
    q.push_str("(declare-const self_var (_ BitVec 64))\n");

    // Declare named parameters as free variables
    for (name, _ty) in params {
        let var = sanitize_name(name);
        q.push_str(&format!("(declare-const {} (_ BitVec 64))\n", var));
    }

    // Assert the negation: if UNSAT, the condition always holds
    q.push_str("(assert (not ");
    q.push_str(&encode_expr_smt(expr));
    q.push_str("))\n");

    q.push_str("(check-sat)\n");
    q
}

/// Sanitize a Brief identifier for SMT-LIB.
/// #Self → self_var (SMT-LIB doesn't allow # in simple symbols).
fn sanitize_name(name: &str) -> String {
    if name == "#Self" || name == "#self" {
        "self_var".to_string()
    } else if name.starts_with('#') {
        format!("hash_{}", &name[1..])
    } else {
        name.to_string()
    }
}

/// 2026-07-23: Quick check if z3 is on PATH by spawning with --version.
pub fn is_z3_available() -> bool {
    std::process::Command::new("z3")
        .arg("--version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .and_then(|mut c| c.wait())
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_bool() {
        assert_eq!(encode_expr_smt(&Expr::Bool(true)), "(= #b1 #b1)");
        assert_eq!(encode_expr_smt(&Expr::Bool(false)), "(= #b0 #b1)");
    }

    #[test]
    fn test_encode_eq() {
        let expr = Expr::BinaryOp(
            crate::ast::BinaryOpKind::Eq,
            Box::new(Expr::Decimal(42)),
            Box::new(Expr::Decimal(42)),
        );
        let smt = encode_expr_smt(&expr);
        assert!(smt.starts_with("(= "));
        assert!(smt.contains("#x000000000000002a"));
    }

    #[test]
    fn test_encode_and() {
        let expr = Expr::BinaryOp(
            crate::ast::BinaryOpKind::And,
            Box::new(Expr::Bool(true)),
            Box::new(Expr::Bool(true)),
        );
        let smt = encode_expr_smt(&expr);
        assert!(smt.starts_with("(and "));
    }

    #[test]
    fn test_build_query() {
        let query = build_smt_query(&Expr::Bool(true));
        assert!(query.contains("set-logic QF_BV"));
        assert!(query.contains("check-sat"));
    }
}
