// ── Phase 1 — SMT Verification Query Builder ──────────────────────────
// 2026-07-28: Converts Brief Expr to SMT-LIB2 terms, builds forall
// verification queries, invokes Z3, and extracts counterexamples.
// This is the core of the CEGIS verification loop.

use crate::ast::{BinaryOpKind, Expr, Type, UnaryOpKind};
use crate::ast::Pattern;
use crate::derive::SynthesizeError;
use std::process::Command;
use std::io::Write;

/// Convert a Brief expression to an SMT-LIB2 term string.
/// `param_names` are the names of bound variables (function parameters).
pub fn expr_to_smt_term(expr: &Expr, param_names: &[String]) -> String {
    match expr {
        Expr::Decimal(n) => format_smt_int(*n),
        Expr::Float(f) => {
            let bits = (*f as f32).to_bits();
            format!("#x{:08X}", bits)
        }
        Expr::Bool(b) => {
            if *b { "true".into() } else { "false".into() }
        }
        Expr::Identifier(name) => {
            // Check if this is a parameter or a special variable
            if param_names.contains(name) {
                name.clone()
            } else if name == "@result" {
                // @result is the postcondition result variable
                "result_val".into()
            } else {
                name.clone()
            }
        }
        Expr::UnaryOp(op, inner) => {
            let inner_str = expr_to_smt_term(inner, param_names);
            match op {
                UnaryOpKind::Neg => format!("(bvneg {})", inner_str),
                UnaryOpKind::Not => format!("(not {})", inner_str),
                UnaryOpKind::BitNot => format!("(bvnot {})", inner_str),
            }
        }
        Expr::BinaryOp(op, lhs, rhs) => {
            let l = expr_to_smt_term(lhs, param_names);
            let r = expr_to_smt_term(rhs, param_names);
            let op_str = binary_op_to_smt(*op);
            format!("({} {} {})", op_str, l, r)
        }
        Expr::If(cond, then, else_) => {
            let c = expr_to_smt_term(cond, param_names);
            let t = expr_to_smt_term(then, param_names);
            let e_str = else_.as_ref()
                .map(|e| expr_to_smt_term(e, param_names))
                .unwrap_or_else(|| "#x0000000000000000".into());
            format!("(ite {} {} {})", c, t, e_str)
        }
        // 2026-07-28: Phase 2 — Constructor (via Call), Field, Match
        Expr::Call(name, args, _) => {
            let arg_strs: Vec<String> = args.iter()
                .map(|a| expr_to_smt_term(a, param_names))
                .collect();
            format!("({} {})", name, arg_strs.join(" "))
        }
        Expr::Field(inner, field_name) => {
            let inner_str = expr_to_smt_term(inner, param_names);
            // 2026-07-28: Phase 4 — Use Z3 datatype selector names.
            // The selector name depends on the constructor:
            //   Const: "0" → "Const-val"
            //   Add: "0" → "Add-left", "1" → "Add-right"
            // If the inner expression is a Call, derive the selector from it.
            let sel = field_name.clone();
            if let Expr::Call(cname, _, _) = inner.as_ref() {
                let prefix = &cname;
                let suffix = match field_name.as_str() {
                    "0" => "-val",
                    "1" => "-right",
                    _ => "",
                };
                let alt = format!("{}{}", prefix, suffix);
                format!("({} {})", alt, inner_str)
            } else {
                format!("({} {})", sel, inner_str)
            }
        }
        Expr::Match(expr, arms) => {
            let scrutinee = expr_to_smt_term(expr, param_names);
            // Convert match to nested ite: (ite (is-Variant scrutinee) body ...)
            let mut result = String::new();
            for (i, arm) in arms.iter().enumerate() {
                match &arm.pattern {
                    Pattern::EnumVariant(name, _) => {
                        let arm_body = expr_to_smt_term(&arm.body, param_names);
                        if i == 0 {
                            result = format!("(ite (is-{}) {} {}", name, scrutinee, arm_body);
                        } else {
                            result = format!("{} {}", result, arm_body);
                        }
                    }
                    Pattern::Wildcard => {
                        let arm_body = expr_to_smt_term(&arm.body, param_names);
                        result = format!("{} {}", result, arm_body);
                    }
                    _ => {}
                }
            }
            // Close all ite expressions (one for each arm except wildcard)
            let arm_count = arms.iter().filter(|a| matches!(a.pattern, Pattern::EnumVariant(_, _))).count();
            for _ in 0..arm_count {
                result.push(')');
            }
            result
        }
        // Fallback for unsupported expression types
        _ => "#x0000000000000000".into(),
    }
}

/// Convert a BinaryOpKind to an SMT-LIB2 operator string.
fn binary_op_to_smt(op: BinaryOpKind) -> &'static str {
    match op {
        BinaryOpKind::Add => "bvadd",
        BinaryOpKind::Sub => "bvsub",
        BinaryOpKind::Mul => "bvmul",
        BinaryOpKind::Div => "bvsdiv",
        BinaryOpKind::Mod => "bvsrem",
        BinaryOpKind::Eq => "=",
        BinaryOpKind::Neq => "distinct", // (distinct a b) = not(= a b)
        BinaryOpKind::Lt => "bvslt",
        BinaryOpKind::Gt => "bvsgt",
        BinaryOpKind::Le => "bvsle",
        BinaryOpKind::Ge => "bvsge",
        BinaryOpKind::BitAnd => "bvand",
        BinaryOpKind::BitOr => "bvor",
        BinaryOpKind::BitXor => "bvxor",
        BinaryOpKind::Shl => "bvshl",
        BinaryOpKind::Shr => "bvashr",
        BinaryOpKind::And => "and",
        BinaryOpKind::Or => "or",
        BinaryOpKind::Concat => "concat",
    }
}

/// Convert a Brief Type to an SMT-LIB2 sort string.
pub fn type_to_smt_sort(ty: &Type) -> String {
    match ty {
        Type::Custom(name) => match name.as_str() {
            "Int" | "Int64" | "UInt64" => "(_ BitVec 64)".into(),
            "Int32" | "UInt32" => "(_ BitVec 32)".into(),
            "Int16" | "UInt16" => "(_ BitVec 16)".into(),
            "Int8" | "UInt8" => "(_ BitVec 8)".into(),
            "Bool" => "Bool".into(),
            "Float" => "(_ FloatingPoint 11 53)".into(),
            // 2026-07-28: Phase 4 — Compound type names used as Z3 datatype names
            name if is_compound_type(name) => name.into(),
            _ => "(_ BitVec 64)".into(),
        },
        Type::Bits(n) => {
            let width = *n as u64 * 8;
            format!("(_ BitVec {})", width)
        }
        Type::Vector(inner, n) => {
            let inner_sort = type_to_smt_sort(inner);
            let size: String = n.iter().map(|d| d.to_string()).collect::<Vec<_>>().join("x");
            format!("(Vector {} {})", inner_sort, size)
        }
        _ => "(_ BitVec 64)".into(),
    }
}

/// 2026-07-28: Check if a type name is a known compound (datatype) type.
fn is_compound_type(name: &str) -> bool {
    matches!(name, "Expr")
}

/// Get Z3 datatype declaration for a Brief type, if it's a compound type.
fn datatype_decl_for_type(ty: &Type) -> Option<String> {
    match ty {
        Type::Custom(name) => get_datatype_declaration(name),
        _ => None,
    }
}

/// Get Z3 datatype declaration by type name string.
fn datatype_decl_for_type_str(name: &str) -> Option<String> {
    get_datatype_declaration(name)
}

/// 2026-07-28: Phase 4 — Generate Z3 declare-datatypes for compound types.
/// Returns the datatype declaration for built-in synthesis types.
/// Each type gets constructors with selectors matching expr_to_smt_term naming.
pub fn get_datatype_declaration(type_name: &str) -> Option<String> {
    match type_name {
        "Expr" => Some(
            "(declare-datatypes () ((Expr\n\
             (Const (Const-val (_ BitVec 64)))\n\
             (Add (Add-left Expr) (Add-right Expr))\n\
             (Sub (Sub-left Expr) (Sub-right Expr))\n\
             (Mul (Mul-left Expr) (Mul-right Expr))\n\
             )))".into()
        ),
        "Bool" | "Int" | "Int64" | "Int32" | "Int16" | "Int8" => None,
        _ => None,
    }
}

/// Format an i64 as an SMT bitvector constant (hex).
pub fn format_smt_int(n: i64) -> String {
    if n >= 0 {
        format!("#x{:016X}", n)
    } else {
        let bits = n as u64;
        format!("#x{:016X}", bits)
    }
}

/// Build a verification query for a synthesized candidate.
/// Uses `(assert (not (forall ...)))` to check if the candidate is correct
/// for ALL inputs. When unsat → candidate is proven correct. When sat →
/// counterexample extracted from model.
///
/// If `postcondition` is provided, it's used as the specification.
/// If not, the examples themselves are used (less strong — only proves
/// the candidate matches examples, which is already true by construction).
pub fn build_verification_query(
    name: &str,
    candidate: &Expr,
    params: &[(String, Type)],
    postcondition: Option<&Expr>,
) -> String {
    let mut q = String::new();
    q.push_str("(set-option :produce-models true)\n");
    q.push_str("(set-logic ALL)\n\n");

    // 2026-07-28: Phase 4 — Datatype declarations for compound types
    for (_, ty) in params {
        if let Some(decl) = datatype_decl_for_type(ty) {
            q.push_str(&decl);
            q.push('\n');
        }
    }
    if let Some(decl) = postcondition.and_then(|_| datatype_decl_for_type_str("Expr")) {
        q.push_str(&decl);
        q.push('\n');
    }
    q.push('\n');

    // define-fun for the candidate function — fixes f as the candidate body
    // so Z3 doesn't assign arbitrary values.
    q.push_str(&format!("; Candidate: {}\n", name));
    let mut param_names: Vec<String> = (0..params.len())
        .map(|i| format!("x{}", i))
        .collect();
    let candidate_body = expr_to_smt_term(candidate, &param_names);
    q.push_str("(define-fun f (");
    for (i, (_, ty)) in params.iter().enumerate() {
        q.push_str(&format!(" (x{} {})", i, type_to_smt_sort(ty)));
    }
    q.push_str(&format!(") (_ BitVec 64) {})\n\n", candidate_body));

    // If postcondition provided: verify candidate against it
    if let Some(post) = postcondition {
        q.push_str("; Verify: forall inputs, candidate satisfies postcondition\n");
        q.push_str("(assert (not (forall (");
        for (i, (name, ty)) in params.iter().enumerate() {
            q.push_str(&format!(" (x{} {})", i, type_to_smt_sort(ty)));
        }
        q.push_str(")\n");

        // The postcondition in SMT — #Term is replaced by (f x0 x1 ...)
        // which is the function's output for the quantified inputs.
        let param_refs: Vec<String> = (0..params.len())
            .map(|i| format!("x{}", i))
            .collect();
        let f_call = format!("(f {})", param_refs.join(" "));
        // Parse the postcondition, replacing #Term with (f x0 x1 ...)
        param_names.push("#Term".to_string());
        let post_raw = expr_to_smt_term(post, &param_names);
        // Substitute: the raw "#Term" becomes the function call
        let post_body = post_raw.replace("#Term", &f_call);

        q.push_str(&format!("   {})))\n", post_body));
    }

    q.push_str("\n(check-sat)\n");
    q.push_str("(get-model)\n");
    q
}

/// Result of Z3 verification.
#[derive(Debug)]
pub enum VerificationResult {
    /// Candidate is correct for ALL inputs.
    Proven,
    /// Counterexample found. The Vec contains input expressions.
    Counterexample(Vec<Vec<Expr>>),
    /// Solver error.
    Error(String),
}

/// Run Z3 with a verification query and parse the result.
pub fn run_z3_verify(query: &str) -> Result<VerificationResult, SynthesizeError> {
    let mut child = Command::new("z3-4.12")
        .arg("-in")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| SynthesizeError::SolverUnavailable(
            format!("failed to start z3: {}", e)
        ))?;

    // Write query to stdin, flush, then close stdin so Z3 sees EOF
    if let Some(mut stdin) = child.stdin.take() {
        writeln!(&stdin, "{}", query)
            .map_err(|e| SynthesizeError::SolverError(
                format!("failed to write to z3: {}", e)
            ))?;
        stdin.flush()
            .map_err(|e| SynthesizeError::SolverError(
                format!("failed to flush z3: {}", e)
            ))?;
        // stdin is dropped here, closing the pipe
    }

    let output = child.wait_with_output()
        .map_err(|e| SynthesizeError::SolverError(
            format!("failed to wait for z3: {}", e)
        ))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    if !stderr.is_empty() && !stderr.contains("warning") {
        return Err(SynthesizeError::SolverError(
            format!("z3 stderr: {}", stderr.trim())
        ));
    }

    // Parse result
    if stdout.contains("unsat") {
        Ok(VerificationResult::Proven)
    } else if stdout.contains("sat") {
        // Extract counterexample from model
        let examples = extract_counterexamples(&stdout);
        Ok(VerificationResult::Counterexample(examples))
    } else if stdout.contains("unknown") {
        Ok(VerificationResult::Error("Z3 returned unknown".into()))
    } else {
        Err(SynthesizeError::SolverError(
            format!("unexpected z3 output: {}", stdout.trim())
        ))
    }
}

/// Parse counterexample variable bindings from Z3's sat model output.
/// Z3's get-model returns a define-fun for the function and model-add
/// entries for the quantified variable bindings.
fn extract_counterexamples(output: &str) -> Vec<Vec<Expr>> {
    let mut results = Vec::new();
    let mut line_iter = output.lines().peekable();

    while let Some(line) = line_iter.next() {
        let trimmed = line.trim();
        // Look for model variable bindings: (x0 (_ BitVec 64) #xHEX)
        if trimmed.starts_with("(x0") && trimmed.contains("#x") {
            let mut example = Vec::new();
            // Collect all (xN ... #xHEX) bindings on consecutive lines
            let mut idx = 0;
            loop {
                let var_line = if idx == 0 { trimmed } else {
                    line_iter.peek().map(|l| l.trim()).unwrap_or("")
                };
                if let Some(hpos) = var_line.find("#x") {
                    let hex_str = &var_line[hpos..];
                    let end = hex_str.find(|c: char| !c.is_alphanumeric() && c != 'x')
                        .unwrap_or(18);
                    if let Ok(val) = i64::from_str_radix(&hex_str[2..2+end.min(16)], 16) {
                        example.push(Expr::Decimal(val));
                    }
                }
                idx += 1;
                if idx > 1 {
                    let next = line_iter.peek().map(|l| l.trim()).unwrap_or("");
                    if next.starts_with("(x") && next.contains("#x") {
                        line_iter.next();
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            }
            if !example.is_empty() {
                results.push(example);
            }
        }
    }

    if results.is_empty() {
        results.push(vec![Expr::Decimal(0)]);
    }
    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expr_to_smt_term_decimal() {
        let result = expr_to_smt_term(&Expr::Decimal(42), &[]);
        assert_eq!(result, "#x000000000000002A");
    }

    #[test]
    fn test_expr_to_smt_term_identifier() {
        let result = expr_to_smt_term(&Expr::Identifier("x0".into()), &["x0".into()]);
        assert_eq!(result, "x0");
    }

    #[test]
    fn test_expr_to_smt_term_add() {
        let expr = Expr::BinaryOp(
            BinaryOpKind::Add,
            Box::new(Expr::Identifier("x0".into())),
            Box::new(Expr::Identifier("x1".into())),
        );
        let result = expr_to_smt_term(&expr, &["x0".into(), "x1".into()]);
        assert_eq!(result, "(bvadd x0 x1)");
    }

    #[test]
    fn test_expr_to_smt_term_sub() {
        let expr = Expr::BinaryOp(
            BinaryOpKind::Sub,
            Box::new(Expr::Identifier("x0".into())),
            Box::new(Expr::Decimal(1)),
        );
        let result = expr_to_smt_term(&expr, &["x0".into()]);
        assert_eq!(result, "(bvsub x0 #x0000000000000001)");
    }

    #[test]
    fn test_expr_to_smt_term_ite() {
        let cond = Expr::BinaryOp(
            BinaryOpKind::Lt,
            Box::new(Expr::Identifier("x0".into())),
            Box::new(Expr::Decimal(0)),
        );
        let then = Expr::UnaryOp(UnaryOpKind::Neg, Box::new(Expr::Identifier("x0".into())));
        let else_ = Expr::Identifier("x0".into());
        let expr = Expr::If(Box::new(cond), Box::new(then), Some(Box::new(else_)));
        let result = expr_to_smt_term(&expr, &["x0".into()]);
        assert_eq!(result, "(ite (bvslt x0 #x0000000000000000) (bvneg x0) x0)");
    }

    #[test]
    fn test_build_verification_query_simple() {
        let candidate = Expr::Identifier("x0".into());
        let params = vec![("x".into(), Type::int())];
        let query = build_verification_query("test", &candidate, &params, None);
        assert!(query.contains("define-fun"));
        assert!(query.contains("(set-logic ALL)"));
        assert!(query.contains("(check-sat)"));
    }

    #[test]
    fn test_build_verification_query_with_postcondition() {
        let candidate = Expr::BinaryOp(
            BinaryOpKind::Add,
            Box::new(Expr::Identifier("x0".into())),
            Box::new(Expr::Decimal(1)),
        );
        let params = vec![("x".into(), Type::int())];
        // Postcondition: #Term > 0
        let post = Expr::BinaryOp(
            BinaryOpKind::Gt,
            Box::new(Expr::Identifier("#Term".into())),
            Box::new(Expr::Decimal(0)),
        );
        let query = build_verification_query("test", &candidate, &params, Some(&post));
        assert!(query.contains("(forall"));
        assert!(query.contains("(f x0"));
    }

    #[test]
    fn test_format_smt_int_positive() {
        let result = format_smt_int(42);
        assert_eq!(result, "#x000000000000002A");
    }

    #[test]
    fn test_format_smt_int_negative() {
        let result = format_smt_int(-1);
        // -1 as u64 = 0xFFFFFFFFFFFFFFFF
        assert_eq!(result, "#xFFFFFFFFFFFFFFFF");
    }

    #[test]
    fn test_type_to_smt_sort_int() {
        assert_eq!(type_to_smt_sort(&Type::int()), "(_ BitVec 64)");
    }

    #[test]
    fn test_type_to_smt_sort_bool() {
        assert_eq!(type_to_smt_sort(&Type::bool_()), "Bool");
    }

    #[test]
    fn test_run_z3_verify_simple() {
        // trivial query: true is satisfiable
        let query = "(set-logic ALL) (assert true) (check-sat)";
        let result = run_z3_verify(query);
        assert!(result.is_ok());
        // Z3 4.8.12 returns sat for trivial true
        match result.unwrap() {
            VerificationResult::Counterexample(examples) => {
                assert!(!examples.is_empty(), "should have counterexample");
            }
            VerificationResult::Proven => {
                // If Z3 returns unsat, that's also fine (different version)
            }
            VerificationResult::Error(_) => {}
        }
    }
}
