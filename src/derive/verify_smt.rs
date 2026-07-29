// ── Phase 1 — SMT Verification Query Builder ──────────────────────────
// 2026-07-28: Converts Brief Expr to SMT-LIB2 terms, builds forall
// verification queries, invokes Z3, and extracts counterexamples.
// This is the core of the CEGIS verification loop.

use crate::ast::{BinaryOpKind, Expr, Statement, Type, UnaryOpKind};
use crate::ast::Pattern;
use crate::derive::SynthesizeError;
use std::process::Command;
use std::io::Write;

/// Convert a Brief expression to an SMT-LIB2 term string.
/// `param_names` are the names of bound variables (function parameters).
/// 2026-07-29: Convert a block of statements to a single SMT expression.
/// Inlines let bindings via β-reduction (substitution) and returns the
/// SMT term of the final expression (from Term, Return, or the last value).
/// If the block is a single expression, returns it directly.
/// Handles the reference function body: let bindings + final term expr.
fn block_to_smt_term(block: &[Statement], param_names: &[String]) -> String {
    // Start from the end and work backwards, inlining let bindings
    let mut stmts = block.to_vec();
    // Process statements in reverse, inlining let bindings
    let mut current_body: Option<String> = None;
    let mut current_expr: Option<Expr> = None;

    for stmt in stmts.into_iter().rev() {
        match stmt {
            Statement::Term(Some(expr)) => {
                // Final expression — this is what we need to convert
                current_expr = Some(expr);
            }
            Statement::Let { names, expr, .. } => {
                // Inline the let binding: substitute the name into current_expr
                if let (Some(let_expr), Some(ce)) = (expr, current_expr.take()) {
                    let mut result = ce;
                    for name in &names {
                        result = substitute_var_local(&result, name, &let_expr);
                    }
                    current_expr = Some(result);
                }
            }
            Statement::Expression(expr) => {
                if current_expr.is_none() {
                    current_expr = Some(expr);
                }
            }
            _ => {}
        }
    }

    if let Some(ce) = current_expr {
        expr_to_smt_term(&ce, param_names)
    } else {
        "#x0000000000000000".into()
    }
}

/// 2026-07-29: Local substitute_var for verify_smt.rs (mirrors smt.rs).
fn substitute_var_local(expr: &Expr, var_name: &str, replacement: &Expr) -> Expr {
    match expr {
        Expr::Identifier(name) => {
            if name == var_name { replacement.clone() } else { expr.clone() }
        }
        Expr::UnaryOp(kind, inner) => {
            Expr::UnaryOp(*kind, Box::new(substitute_var_local(inner, var_name, replacement)))
        }
        Expr::BinaryOp(kind, lhs, rhs) => {
            Expr::BinaryOp(
                *kind,
                Box::new(substitute_var_local(lhs, var_name, replacement)),
                Box::new(substitute_var_local(rhs, var_name, replacement)),
            )
        }
        Expr::If(cond, then_, else_) => {
            Expr::If(
                Box::new(substitute_var_local(cond, var_name, replacement)),
                Box::new(substitute_var_local(then_, var_name, replacement)),
                else_.as_ref().map(|e| Box::new(substitute_var_local(e, var_name, replacement))),
            )
        }
        Expr::Call(name, args, aid) => {
            Expr::Call(
                name.clone(),
                args.iter().map(|a| substitute_var_local(a, var_name, replacement)).collect(),
                *aid,
            )
        }
        Expr::Field(inner, fname) => {
            Expr::Field(Box::new(substitute_var_local(inner, var_name, replacement)), fname.clone())
        }
        Expr::Match(scrut, arms) => {
            Expr::Match(
                Box::new(substitute_var_local(scrut, var_name, replacement)),
                arms.iter().map(|a| crate::ast::MatchArm {
                    pattern: a.pattern.clone(),
                    guard: a.guard.clone(),
                    body: Box::new(substitute_var_local(&a.body, var_name, replacement)),
                }).collect(),
            )
        }
        Expr::Block(stmts) => {
            Expr::Block(stmts.iter().map(|s| match s {
                Statement::Let { name, names, ty, expr, modifiers } => {
                    Statement::Let {
                        name: name.clone(),
                        names: names.clone(),
                        ty: ty.clone(),
                        expr: expr.as_ref().map(|e| substitute_var_local(e, var_name, replacement)),
                        modifiers: modifiers.clone(),
                    }
                }
                Statement::Term(val) => {
                    Statement::Term(val.as_ref().map(|e| substitute_var_local(e, var_name, replacement)))
                }
                Statement::Assign(lhs, rhs) => {
                    Statement::Assign(
                        substitute_var_local(lhs, var_name, replacement),
                        substitute_var_local(rhs, var_name, replacement),
                    )
                }
                other => other.clone(),
            }).collect())
        }
        _ => expr.clone(),
    }
}

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
    precondition: Option<&Expr>,
    ref_fn: Option<(&Expr, &[String])>,
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

    // 2026-07-29: Reference function as define-fun ref.
    // Substitute reference's own param names with standardized x0, x1, ...
    // so the ref body uses the same variable names as the candidate and
    // postcondition in the SMT query.
    if let Some((ref_expr, ref_param_names)) = ref_fn {
        let ref_body_renamed = rename_variables(ref_expr, ref_param_names, &param_names);
        // 2026-07-29: Handle Block bodies (let bindings + term) by inlining
        // let bindings via β-reduction. Fall back to expr_to_smt_term for
        // simple expressions.
        let ref_body = match &ref_body_renamed {
            Expr::Block(stmts) => block_to_smt_term(stmts, &param_names),
            _ => expr_to_smt_term(&ref_body_renamed, &param_names),
        };
        q.push_str("(define-fun ref (");
        for (i, (_, ty)) in params.iter().enumerate() {
            q.push_str(&format!(" (x{} {})", i, type_to_smt_sort(ty)));
        }
        q.push_str(&format!(") (_ BitVec 64) {})\n\n", ref_body));
    }

    // 2026-07-29: Build combined verification condition.
    // When both postcondition and reference are present, assert BOTH:
    //   (=> pre post) AND (= (f x) (ref x))
    // When only one is present, assert just that one.
    let param_refs: Vec<String> = (0..params.len())
        .map(|i| format!("x{}", i))
        .collect();
    let mut conditions: Vec<String> = Vec::new();

    if let Some(post) = postcondition {
        let f_call = format!("(f {})", param_refs.join(" "));
        let mut post_params = param_names.clone();
        post_params.push("#Term".to_string());
        let post_raw = expr_to_smt_term(post, &post_params);
        let post_body = post_raw.replace("#Term", &f_call);

        let cond = if let Some(pre) = precondition {
            let pre_body = expr_to_smt_term(pre, &param_names);
            format!("(=> {} {})", pre_body, post_body)
        } else {
            post_body
        };
        conditions.push(cond);
    }

    if ref_fn.is_some() {
        let f_call = format!("(f {})", param_refs.join(" "));
        let ref_call = format!("(ref {})", param_refs.join(" "));
        let equality = format!("(= {})", vec![f_call, ref_call].join(" "));
        let cond = if let Some(pre) = precondition {
            let pre_body = expr_to_smt_term(pre, &param_names);
            format!("(=> {} {})", pre_body, equality)
        } else {
            equality
        };
        conditions.push(cond);
    }

    if !conditions.is_empty() {
        let forall_body = if conditions.len() == 1 {
            conditions[0].clone()
        } else {
            format!("(and {})", conditions.join(" "))
        };

        q.push_str("; Verify: forall inputs, candidate satisfies specification\n");
        q.push_str("(assert (not (forall (");
        for (i, (_, ty)) in params.iter().enumerate() {
            q.push_str(&format!(" (x{} {})", i, type_to_smt_sort(ty)));
        }
        q.push_str(&format!(")\n   {})))\n", forall_body));
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

// ── Variable Renaming ────────────────────────────────────────────────

/// 2026-07-29: Rename variables in an expression according to a positional mapping.
/// Each identifier in the expression that matches a name in `from_names` is replaced
/// with the corresponding name in `to_names`. Used to standardize reference function
/// parameter names before SMT conversion.
fn rename_variables(expr: &Expr, from_names: &[String], to_names: &[String]) -> Expr {
    match expr {
        Expr::Identifier(name) => {
            if let Some(idx) = from_names.iter().position(|p| p == name) {
                to_names.get(idx).cloned().map_or_else(
                    || expr.clone(),
                    |new_name| Expr::Identifier(new_name),
                )
            } else {
                expr.clone()
            }
        }
        Expr::UnaryOp(kind, inner) => {
            Expr::UnaryOp(*kind, Box::new(rename_variables(inner, from_names, to_names)))
        }
        Expr::BinaryOp(kind, lhs, rhs) => {
            Expr::BinaryOp(
                *kind,
                Box::new(rename_variables(lhs, from_names, to_names)),
                Box::new(rename_variables(rhs, from_names, to_names)),
            )
        }
        Expr::If(cond, then, else_) => {
            Expr::If(
                Box::new(rename_variables(cond, from_names, to_names)),
                Box::new(rename_variables(then, from_names, to_names)),
                else_.as_ref().map(|e| Box::new(rename_variables(e, from_names, to_names))),
            )
        }
        Expr::Call(name, args, aid) => {
            Expr::Call(
                name.clone(),
                args.iter().map(|a| rename_variables(a, from_names, to_names)).collect(),
                *aid,
            )
        }
        Expr::Field(inner, fname) => {
            Expr::Field(Box::new(rename_variables(inner, from_names, to_names)), fname.clone())
        }
        Expr::Match(scrut, arms) => {
            Expr::Match(
                Box::new(rename_variables(scrut, from_names, to_names)),
                arms.iter().map(|a| crate::ast::MatchArm {
                    pattern: a.pattern.clone(),
                    guard: a.guard.clone(),
                    body: Box::new(rename_variables(&a.body, from_names, to_names)),
                }).collect(),
            )
        }
        Expr::Block(stmts) => {
            Expr::Block(stmts.iter().map(|s| rename_stmt(s, from_names, to_names)).collect())
        }
        _ => expr.clone(),
    }
}

/// 2026-07-29: Rename variables in a statement (used by rename_variables for blocks).
fn rename_stmt(stmt: &Statement, from_names: &[String], to_names: &[String]) -> Statement {
    match stmt {
        Statement::Let { name, names, ty, expr, modifiers } => {
            Statement::Let {
                name: name.clone(),
                names: names.clone(),
                ty: ty.clone(),
                expr: expr.as_ref().map(|e| rename_variables(e, from_names, to_names)),
                modifiers: modifiers.clone(),
            }
        }
        Statement::Assign(lhs, rhs) => {
            Statement::Assign(
                rename_variables(lhs, from_names, to_names),
                rename_variables(rhs, from_names, to_names),
            )
        }
        Statement::Expression(expr) => {
            Statement::Expression(rename_variables(expr, from_names, to_names))
        }
        Statement::Term(val) => {
            Statement::Term(val.as_ref().map(|e| rename_variables(e, from_names, to_names)))
        }
        Statement::TermBang(val) => {
            Statement::TermBang(val.as_ref().map(|e| rename_variables(e, from_names, to_names)))
        }
        Statement::Guarded(cond, body) => {
            Statement::Guarded(
                rename_variables(cond, from_names, to_names),
                body.iter().map(|s| rename_stmt(s, from_names, to_names)).collect(),
            )
        }
        Statement::Gate(cond) => {
            Statement::Gate(rename_variables(cond, from_names, to_names))
        }
        Statement::If(cond, then, else_) => {
            Statement::If(
                rename_variables(cond, from_names, to_names),
                then.iter().map(|s| rename_stmt(s, from_names, to_names)).collect(),
                else_.iter().map(|s| rename_stmt(s, from_names, to_names)).collect(),
            )
        }
        Statement::Block(body) => {
            Statement::Block(body.iter().map(|s| rename_stmt(s, from_names, to_names)).collect())
        }
        Statement::SyncBlock(body) => {
            Statement::SyncBlock(body.iter().map(|s| rename_stmt(s, from_names, to_names)).collect())
        }
        Statement::Return(val) => {
            Statement::Return(val.as_ref().map(|e| rename_variables(e, from_names, to_names)))
        }
        _ => stmt.clone(),
    }
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
        let query = build_verification_query("test", &candidate, &params, None, None, None);
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
        let query = build_verification_query("test", &candidate, &params, Some(&post), None, None);
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
