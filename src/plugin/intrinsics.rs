// ── $ Intrinsic Dispatch ──────────────────────────────────────────────
//
// 2026-07-15: Phase 3 — Compile-time $ intrinsics callable from $(Stage)
// block bodies. Each intrinsic is a Rust function that operates on the
// compilation state (program AST, type universe).
//
// The dispatch function is called by StageBlockPlugin::evaluate_body()
// which iterates the block's Statement vec and dispatches each $ call.
//
// Architecture:
//   dispatch_intrinsic(name, args, program, universe) -> Result<(), String>
//   ├── InsertRegistryImport$(path)  — pushes Import::registry(path)
//   ├── EmitWarning$(msg)            — eprintln warning
//   ├── EmitError$(msg)              — returns Err(msg)
//   ├── Collect$(pattern)            — stub: returns empty set
//   └── MatchIR$(pattern, replace)   — stub: returns false
//
// Each intrinsic receives pre-evaluated argument expressions. For Phase 3
// only literal args are supported (string, integer, boolean). Phase 6 will
// add compile-time expression evaluation for full metaprogramming.

use crate::ast::{Expr, Import, Statement, TopLevel};
use crate::bvir;
use crate::type_universe::TypeUniverse;

/// Result of evaluating a single $ intrinsic call.
pub enum IntrinsicResult {
    /// Continue compilation normally.
    Continue,
    /// Abort compilation with an error message.
    Abort(String),
}

/// Dispatch a `$` intrinsic call.
///
/// `name` is the identifier (e.g. `"InsertRegistryImport$"`).
/// `args` are the pre-evaluated argument expressions (literals only for now).
/// `program` and `universe` are the current compilation state and may be
/// mutated by the intrinsic.
///
/// 2026-07-15: Phase 3 — Flat dispatch: match name, extract args, call handler.
pub fn dispatch_intrinsic(
    name: &str,
    args: &[Expr],
    program: &mut Vec<TopLevel>,
    universe: &mut TypeUniverse,
) -> Result<(), String> {
    match name {
        "InsertLiteralImport$" => intrinsic_insert_literal_import(args, program),
        "InsertRegistryImport$" => intrinsic_insert_registry_import(args, program),
        "EmitWarning$" => intrinsic_emit_warning(args),
        "EmitError$" => intrinsic_emit_error(args),
        "Collect$" => intrinsic_collect(args, program),
        "MatchIR$" => intrinsic_match_ir(args, program, universe),
        _ => Err(format!(
            "unknown $ intrinsic '{}'. Available: InsertRegistryImport$, \
             EmitWarning$, EmitError$, Collect$, MatchIR$",
            name
        )),
    }
}

// ── Argument Helpers ──────────────────────────────────────────────────

/// Extract a string literal from an expression at the given argument index.
/// 2026-07-15: Phase 3 — literal-only. Phase 6 will add expression eval.
fn expect_string_arg(args: &[Expr], idx: usize, intrinsic: &str) -> Result<String, String> {
    let arg = args.get(idx).ok_or_else(|| {
        format!("{}: missing argument {}", intrinsic, idx)
    })?;
    match arg {
        Expr::Quoted(bytes) => {
            // Quoted bytes may contain non-UTF-8 data; for imports the path
            // must be valid UTF-8.
            String::from_utf8(bytes.clone()).map_err(|_| {
                format!("{}: argument {} is not a valid UTF-8 string", intrinsic, idx)
            })
        }
        _ => Err(format!(
            "{}: argument {} must be a string literal, got {:?}",
            intrinsic, idx, arg
        )),
    }
}

// ── Intrinsic Implementations ─────────────────────────────────────────

/// `InsertLiteralImport$(path)` — Inject a literal (filesystem) import.
///
/// Pushes `TopLevel::Import(Import::literal(path, []))` into the program AST.
/// The path is resolved against the project's search paths (same as
/// `import "path"` in source code). Used for prelude injection and other
/// filesystem-based imports.
///
/// 2026-07-15: Phase 4 — Replaces the hardcoded prelude injection in
/// import_resolver.rs. Used by plugins/front/prelude.bv.
fn intrinsic_insert_literal_import(
    args: &[Expr],
    program: &mut Vec<TopLevel>,
) -> Result<(), String> {
    let path = expect_string_arg(args, 0, "InsertLiteralImport$")?;
    program.push(TopLevel::Import(Import::literal(path, vec![])));
    Ok(())
}

/// `InsertRegistryImport$(name)` — Inject a config-registry import.
///
/// Pushes `TopLevel::Import(Import::registry(name, []))` into the program AST.
/// The name is resolved against the compiler's module registry config
/// (`config/module-registry.toml`). When the registry config doesn't exist,
/// falls back to literal filesystem resolution.
///
/// 2026-07-15: Phase 3 — Currently creates Import::registry (same resolution
/// as literal until the config registry is implemented in a later phase).
fn intrinsic_insert_registry_import(
    args: &[Expr],
    program: &mut Vec<TopLevel>,
) -> Result<(), String> {
    let path = expect_string_arg(args, 0, "InsertRegistryImport$")?;
    program.push(TopLevel::Import(Import::registry(path, vec![])));
    Ok(())
}

/// `EmitWarning$(msg)` — Emit a compiler warning.
///
/// Prints a warning message to stderr and continues compilation.
/// 2026-07-15: Phase 3 — Simple stderr output; Phase 6 may add structured
/// diagnostic infrastructure.
fn intrinsic_emit_warning(args: &[Expr]) -> Result<(), String> {
    let msg = expect_string_arg(args, 0, "EmitWarning$")?;
    eprintln!("warning: {}", msg);
    Ok(())
}

/// `EmitError$(msg)` — Emit a compiler error and abort.
///
/// Returns Err with the message, which aborts compilation.
/// 2026-07-15: Phase 3 — Direct abort; Phase 6 may add structured errors.
fn intrinsic_emit_error(args: &[Expr]) -> Result<(), String> {
    let msg = expect_string_arg(args, 0, "EmitError$")?;
    Err(msg)
}

/// `Collect$(pattern)` — Collect AST nodes matching a pattern.
///
/// 2026-07-15: Phase 6 — Serializes the program AST to BVIR, then matches
/// the pattern against all sub-expressions. Logs match count and first match.
/// Future: return collected nodes as a compile-time value.
fn intrinsic_collect(args: &[Expr], program: &[TopLevel]) -> Result<(), String> {
    let pattern_str = expect_string_arg(args, 0, "Collect$")?;
    let pattern = bvir::pattern::parse_pattern(&pattern_str)
        .map_err(|e| format!("Collect$: invalid pattern '{}': {}", pattern_str, e))?;

    // Serialize program to BVIR for pattern matching
    let universe = TypeUniverse::new();
    let bvir_text = bvir::serialize::to_bvir(program, &universe);
    let tokens = bvir::sexpr::tokenize(&bvir_text)
        .map_err(|e| format!("Collect$: tokenize error: {}", e))?;
    let root = bvir::sexpr::parse(&tokens)
        .map_err(|e| format!("Collect$: parse error: {}", e))?;

    let matches = bvir::pattern::collect_matches(&pattern, &root);
    let count = matches.len();

    if count == 0 {
        eprintln!("Collect$: no matches for pattern '{}'", pattern_str);
    } else {
        eprintln!("Collect$: found {} match(es) for pattern '{}'", count, pattern_str);
        for (i, m) in matches.iter().enumerate().take(3) {
            let s = bvir::sexpr::to_string(m);
            let truncated = if s.len() > 80 { format!("{}...", &s[..77]) } else { s };
            eprintln!("  match[{}]: {}", i, truncated);
        }
    }
    Ok(())
}

/// `MatchIR$(pattern, replacement)` — Match and rewrite IR patterns.
///
/// 2026-07-15: Phase 6 — Serializes the program AST to BVIR, applies the
/// pattern-match replacement, then deserializes back into the AST.
/// The program is modified in place. Returns error if pattern matches nothing.
fn intrinsic_match_ir(
    args: &[Expr],
    program: &mut Vec<TopLevel>,
    universe: &mut TypeUniverse,
) -> Result<(), String> {
    let pattern_str = expect_string_arg(args, 0, "MatchIR$")?;
    let replacement_str = expect_string_arg(args, 1, "MatchIR$")?;

    let pattern = bvir::pattern::parse_pattern(&pattern_str)
        .map_err(|e| format!("MatchIR$: invalid pattern '{}': {}", pattern_str, e))?;
    let replacement = bvir::pattern::parse_pattern(&replacement_str)
        .map_err(|e| format!("MatchIR$: invalid replacement '{}': {}", replacement_str, e))?;

    // Serialize program + universe to BVIR
    let bvir_text = bvir::serialize::to_bvir(program, universe);
    let tokens = bvir::sexpr::tokenize(&bvir_text)
        .map_err(|e| format!("MatchIR$: tokenize error: {}", e))?;
    let root = bvir::sexpr::parse(&tokens)
        .map_err(|e| format!("MatchIR$: parse error: {}", e))?;

    // Apply replacement
    let (new_root, count) = bvir::pattern::replace_all(&pattern, &replacement, &root);

    if count == 0 {
        return Err(format!(
            "MatchIR$: pattern '{}' did not match any AST node",
            pattern_str
        ));
    }

    // Deserialize back into AST
    let new_bvir = bvir::sexpr::to_string(&new_root);
    let (new_items, new_universe) = bvir::deserialize::from_bvir(&new_bvir)
        .map_err(|e| format!("MatchIR$: deserialize error after replacement: {}", e))?;

    *program = new_items;
    *universe = new_universe;

    eprintln!("MatchIR$: applied {} replacement(s)", count);
    Ok(())
}

// ── Evaluate a Statement for $ intrinsic calls ────────────────────────

/// Evaluate a single Statement, dispatching any $ intrinsic calls found.
/// Non-$ statements are silently skipped (may include let bindings, blocks,
/// etc. for future use).
///
/// 2026-07-15: Phase 3 — Currently only handles Statement::Expression with
/// a Call to a $ identifier. Other statement types are no-ops.
pub fn evaluate_statement(
    stmt: &Statement,
    program: &mut Vec<TopLevel>,
    universe: &mut TypeUniverse,
) -> Result<(), String> {
    match stmt {
        Statement::Expression(expr) => {
            evaluate_expression_for_intrinsic(expr, program, universe)
        }
        Statement::Block(statements) => {
            for s in statements {
                evaluate_statement(s, program, universe)?;
            }
            Ok(())
        }
        // Other statement types are silently skipped for now.
        _ => Ok(()),
    }
}

/// Examine an expression for $ intrinsic calls and dispatch them.
/// Recurses into sub-expressions to find nested calls.
///
/// 2026-07-15: Phase 3 — Looks for Expr::Call where name ends with '$'.
fn evaluate_expression_for_intrinsic(
    expr: &Expr,
    program: &mut Vec<TopLevel>,
    universe: &mut TypeUniverse,
) -> Result<(), String> {
    match expr {
        Expr::Call(name, args) if name.ends_with('$') => {
            dispatch_intrinsic(name, args, program, universe)
        }
        Expr::Call(_, args) => {
            // Non-$ calls: check arguments for nested $ intrinsics.
            for arg in args {
                evaluate_expression_for_intrinsic(arg, program, universe)?;
            }
            Ok(())
        }
        // Recurse into sub-expressions
        Expr::Block(statements) => {
            for s in statements {
                evaluate_statement(s, program, universe)?;
            }
            Ok(())
        }
        Expr::If(cond, then, else_) => {
            evaluate_expression_for_intrinsic(cond, program, universe)?;
            evaluate_expression_for_intrinsic(then, program, universe)?;
            if let Some(else_expr) = else_ {
                evaluate_expression_for_intrinsic(else_expr, program, universe)?;
            }
            Ok(())
        }
        Expr::Field(inner, _) => evaluate_expression_for_intrinsic(inner, program, universe),
        Expr::Index(lhs, rhs) => {
            evaluate_expression_for_intrinsic(lhs, program, universe)?;
            evaluate_expression_for_intrinsic(rhs, program, universe)
        }
        Expr::UnaryOp(_, inner) => evaluate_expression_for_intrinsic(inner, program, universe),
        Expr::BinaryOp(_, lhs, rhs) => {
            evaluate_expression_for_intrinsic(lhs, program, universe)?;
            evaluate_expression_for_intrinsic(rhs, program, universe)
        }
        Expr::Cast(inner, _) => evaluate_expression_for_intrinsic(inner, program, universe),
        Expr::Tuple(items) | Expr::List(items) => {
            for item in items {
                evaluate_expression_for_intrinsic(item, program, universe)?;
            }
            Ok(())
        }
        Expr::Lambda(_, body) => evaluate_expression_for_intrinsic(body, program, universe),
        Expr::Match(scrutinee, arms) => {
            evaluate_expression_for_intrinsic(scrutinee, program, universe)?;
            for arm in arms {
                evaluate_expression_for_intrinsic(&arm.body, program, universe)?;
            }
            Ok(())
        }
        // Literals and identifiers have no sub-expressions.
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Expr, ImportKind, Statement, TopLevel};

    // ── Helper: create a $ call expression ────────────────────────────

    fn make_call(name: &str, args: Vec<Expr>) -> Expr {
        Expr::Call(name.to_string(), args)
    }

    fn make_string(s: &str) -> Expr {
        Expr::Quoted(s.as_bytes().to_vec())
    }

    fn make_int(n: i64) -> Expr {
        Expr::Decimal(n)
    }

    fn make_expr_stmt(expr: Expr) -> Statement {
        Statement::Expression(expr)
    }

    // ── InsertRegistryImport$ ────────────────────────────────────────

    #[test]
    fn test_insert_registry_import_adds_import() {
        let mut program = vec![];
        let mut universe = TypeUniverse::new();
        let expr = make_call("InsertRegistryImport$", vec![make_string("std/prelude.bv")]);
        let result = evaluate_expression_for_intrinsic(&expr, &mut program, &mut universe);
        assert!(result.is_ok());
        assert_eq!(program.len(), 1);
        match &program[0] {
            TopLevel::Import(imp) => {
                assert_eq!(imp.path(), "std/prelude.bv");
            }
            other => panic!("expected Import, got {:?}", other),
        }
    }

    #[test]
    fn test_insert_registry_import_via_statement() {
        let mut program = vec![];
        let mut universe = TypeUniverse::new();
        let stmt = make_expr_stmt(make_call(
            "InsertRegistryImport$",
            vec![make_string("std/hardware.bv")],
        ));
        let result = evaluate_statement(&stmt, &mut program, &mut universe);
        assert!(result.is_ok());
        assert_eq!(program.len(), 1);
        match &program[0] {
            TopLevel::Import(imp) => {
                assert_eq!(imp.path(), "std/hardware.bv");
            }
            other => panic!("expected Import, got {:?}", other),
        }
    }

    #[test]
    fn test_insert_registry_import_missing_arg() {
        let mut program = vec![];
        let mut universe = TypeUniverse::new();
        let expr = make_call("InsertRegistryImport$", vec![]);
        let result = evaluate_expression_for_intrinsic(&expr, &mut program, &mut universe);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("missing argument"));
    }

    #[test]
    fn test_insert_registry_import_wrong_arg_type() {
        let mut program = vec![];
        let mut universe = TypeUniverse::new();
        let expr = make_call("InsertRegistryImport$", vec![make_int(42)]);
        let result = evaluate_expression_for_intrinsic(&expr, &mut program, &mut universe);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("must be a string literal"));
    }

    // ── EmitWarning$ ─────────────────────────────────────────────────

    #[test]
    fn test_emit_warning_ok() {
        let mut program = vec![];
        let mut universe = TypeUniverse::new();
        let expr = make_call("EmitWarning$", vec![make_string("test warning")]);
        // Warning just prints to stderr — should not error
        let result = evaluate_expression_for_intrinsic(&expr, &mut program, &mut universe);
        assert!(result.is_ok());
    }

    #[test]
    fn test_emit_warning_missing_arg() {
        let mut program = vec![];
        let mut universe = TypeUniverse::new();
        let expr = make_call("EmitWarning$", vec![]);
        let result = evaluate_expression_for_intrinsic(&expr, &mut program, &mut universe);
        assert!(result.is_err());
    }

    // ── EmitError$ ───────────────────────────────────────────────────

    #[test]
    fn test_emit_error_aborts() {
        let mut program = vec![];
        let mut universe = TypeUniverse::new();
        let expr = make_call("EmitError$", vec![make_string("fatal")]);
        let result = evaluate_expression_for_intrinsic(&expr, &mut program, &mut universe);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "fatal");
    }

    // ── Unknown intrinsic ────────────────────────────────────────────

    #[test]
    fn test_unknown_intrinsic() {
        let mut program = vec![];
        let mut universe = TypeUniverse::new();
        let expr = make_call("NoSuchIntrinsic$", vec![]);
        let result = evaluate_expression_for_intrinsic(&expr, &mut program, &mut universe);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("unknown $ intrinsic"));
    }

    // ── Non-$ identifiers are not dispatched ─────────────────────────

    #[test]
    fn test_non_dollar_identifier_not_dispatched() {
        let mut program = vec![];
        let mut universe = TypeUniverse::new();
        // A regular function call (no $) should not be dispatched
        let expr = make_call("printf", vec![make_string("hello")]);
        let result = evaluate_expression_for_intrinsic(&expr, &mut program, &mut universe);
        assert!(result.is_ok());
        assert!(program.is_empty());
    }

    // ── Statement::Block recurses ────────────────────────────────────

    #[test]
    fn test_block_recursion() {
        let mut program = vec![];
        let mut universe = TypeUniverse::new();
        let inner = make_expr_stmt(make_call(
            "InsertRegistryImport$",
            vec![make_string("std/test.bv")],
        ));
        let stmt = Statement::Block(vec![inner]);
        let result = evaluate_statement(&stmt, &mut program, &mut universe);
        assert!(result.is_ok());
        assert_eq!(program.len(), 1);
    }

    // ── Multiple intrinsic calls in sequence ─────────────────────────

    #[test]
    fn test_multiple_intrinsics() {
        let mut program = vec![];
        let mut universe = TypeUniverse::new();
        let stmts = vec![
            make_expr_stmt(make_call(
                "InsertRegistryImport$",
                vec![make_string("std/a.bv")],
            )),
            make_expr_stmt(make_call(
                "InsertRegistryImport$",
                vec![make_string("std/b.bv")],
            )),
            make_expr_stmt(make_call(
                "EmitWarning$",
                vec![make_string("imported a and b")],
            )),
        ];
        for stmt in &stmts {
            evaluate_statement(stmt, &mut program, &mut universe).unwrap();
        }
        assert_eq!(program.len(), 2);
    }

    // ── Let statements are silently skipped ──────────────────────────

    #[test]
    fn test_let_is_skipped() {
        let mut program = vec![];
        let mut universe = TypeUniverse::new();
        let stmt = Statement::Let {
            name: "x".to_string(),
            ty: None,
            expr: Some(Expr::Decimal(42)),
            modifiers: vec![],
        };
        let result = evaluate_statement(&stmt, &mut program, &mut universe);
        assert!(result.is_ok());
        assert!(program.is_empty());
    }
}
