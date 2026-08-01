// ── Print Plugin — Front Stage ────────────────────────────────────────
// 2026-07-19: Resolves !Print(x) and !PrintLn(x) to typed C runtime calls
// (__print_int, __print_str, __print_float, __print_char for newline).
//
// Runs at Front stage (before typechecking). Collects variable type
// annotations from `let name: Type = ...` declarations. For literals
// (Quoted, Decimal, Float) dispatches directly. Falls back to __print_int
// when the type can't be determined (all benchmarks use Int as default).
//
// 2026-08-01: Phase 1 of the plugin-macro rework — the entry points are now
// the lowercase macros `print!` / `println!`. Two forms are supported:
//   - print!("literal {0} {1}", a, b) — Rust-style format string with
//     `{{`/`}}` escapes, `{}` sequential placeholders, and `{n}` positional
//     placeholders. Out-of-range `{n}` is a compile error; surplus trailing
//     arguments produce a warning (Rust-compatible).
//   - print!(value) / println!(value) — the legacy single-value form,
//     dispatch derived from the value's protocol category.
// `println!()` with no arguments prints a newline alone.
// The old PascalCase `Print!`/`PrintLn!` names are rejected by the
// typechecker with a rename hint (see src/typechecker/mod.rs).

use crate::ast::{Expr, StageKind, Statement, TopLevel, Type};
use crate::plugin::Plugin;
use crate::type_universe::TypeUniverse;
use std::collections::HashMap;

#[derive(Debug)]
pub struct PrintPlugin;

impl Plugin for PrintPlugin {
    fn name(&self) -> &str {
        "print"
    }

    fn stages(&self) -> Vec<StageKind> {
        vec![StageKind::Parsed, StageKind::Typed]
    }

    fn on_ast(
        &self,
        program: &mut Vec<TopLevel>,
        universe: &mut TypeUniverse,
    ) -> Result<(), String> {
        let mut known_types: HashMap<String, Type> = HashMap::new();
        collect_binding_types(program, &mut known_types);
        resolve_prints(program, &known_types, universe)
    }
}

/// 2026-08-01: One segment of a parsed format string.
/// `Literal` is raw bytes ({{/}} escapes already unfolded);
/// `Next` is a sequential `{}` placeholder;
/// `Position(n)` is a positional `{n}` placeholder.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum FmtPart {
    Literal(Vec<u8>),
    Next,
    Position(usize),
}

/// 2026-08-01: Parse a Rust-style format string into segments.
/// `{{` and `}}` are escapes for a literal brace; `{}` is a sequential
/// placeholder; `{n}` is a positional placeholder. Any other placeholder
/// body is a compile error. Shared with the interpreter so the reference
/// evaluator and the codegen path parse identically.
pub(crate) fn parse_format(fmt: &[u8]) -> Result<Vec<FmtPart>, String> {
    let mut parts = Vec::new();
    let mut literal = Vec::new();
    let mut i = 0;
    while i < fmt.len() {
        match fmt[i] {
            b'{' => {
                if i + 1 < fmt.len() && fmt[i + 1] == b'{' {
                    literal.push(b'{');
                    i += 2;
                    continue;
                }
                if !literal.is_empty() {
                    parts.push(FmtPart::Literal(std::mem::take(&mut literal)));
                }
                let mut j = i + 1;
                while j < fmt.len() && fmt[j] != b'}' {
                    j += 1;
                }
                if j >= fmt.len() {
                    return Err("unmatched '{' in format string".to_string());
                }
                let inner = &fmt[i + 1..j];
                i = j + 1;
                if inner.is_empty() {
                    parts.push(FmtPart::Next);
                } else {
                    let spec = std::str::from_utf8(inner).map_err(|_| {
                        format!("invalid placeholder in format string")
                    })?;
                    let idx = spec.parse::<usize>().map_err(|_| {
                        format!("invalid placeholder '{{{}}}' in format string", spec)
                    })?;
                    parts.push(FmtPart::Position(idx));
                }
            }
            b'}' => {
                if i + 1 < fmt.len() && fmt[i + 1] == b'}' {
                    literal.push(b'}');
                    i += 2;
                    continue;
                }
                return Err("unmatched '}' in format string".to_string());
            }
            c => {
                literal.push(c);
                i += 1;
            }
        }
    }
    if !literal.is_empty() {
        parts.push(FmtPart::Literal(literal));
    }
    Ok(parts)
}

fn collect_binding_types(program: &[TopLevel], map: &mut HashMap<String, Type>) {
    for item in program {
        collect_item_types(item, map);
    }
}

fn collect_item_types(item: &TopLevel, map: &mut HashMap<String, Type>) {
    match item {
        TopLevel::Definition(d) => collect_from_stmts(&d.body, map),
        TopLevel::Transaction(t) => collect_from_stmts(&t.body, map),
        TopLevel::Statement(stmt) => collect_from_stmt(stmt, map),
        _ => {}
    }
}

fn collect_from_stmts(stmts: &[crate::ast::Statement], map: &mut HashMap<String, Type>) {
    for stmt in stmts {
        collect_from_stmt(stmt, map);
    }
}

fn collect_from_stmt(stmt: &crate::ast::Statement, map: &mut HashMap<String, Type>) {
    match stmt {
        crate::ast::Statement::Let { name, ty: Some(t), .. } => {
            map.insert(name.clone(), t.clone());
        }
        crate::ast::Statement::Guarded(_, body) => collect_from_stmts(body, map),
        _ => {}
    }
}

fn resolve_prints(
    program: &mut Vec<TopLevel>,
    known_types: &HashMap<String, Type>,
    universe: &TypeUniverse,
) -> Result<(), String> {
    for item in program.iter_mut() {
        walk_item(item, known_types, universe)?;
    }
    Ok(())
}

fn walk_item(
    item: &mut TopLevel,
    known_types: &HashMap<String, Type>,
    universe: &TypeUniverse,
) -> Result<(), String> {
    match item {
        TopLevel::Definition(d) => walk_stmts(&mut d.body, known_types, universe),
        TopLevel::Transaction(t) => walk_stmts(&mut t.body, known_types, universe),
        TopLevel::Constant(c) => walk_expr(&mut c.expr, known_types, universe),
        TopLevel::Statement(stmt) => walk_stmt(stmt, known_types, universe),
        _ => Ok(()),
    }
}

fn walk_stmts(
    stmts: &mut [crate::ast::Statement],
    known_types: &HashMap<String, Type>,
    universe: &TypeUniverse,
) -> Result<(), String> {
    for stmt in stmts.iter_mut() {
        walk_stmt(stmt, known_types, universe)?;
    }
    Ok(())
}

fn walk_stmt(
    stmt: &mut crate::ast::Statement,
    known_types: &HashMap<String, Type>,
    universe: &TypeUniverse,
) -> Result<(), String> {
    match stmt {
        crate::ast::Statement::Assign(_, expr)
        | crate::ast::Statement::Let { expr: Some(expr), .. }
        | crate::ast::Statement::Expression(expr)
        | crate::ast::Statement::Term(Some(expr))
        | crate::ast::Statement::TermBang(Some(expr)) => {
            walk_expr(expr, known_types, universe)
        }
        crate::ast::Statement::Guarded(_, body) => walk_stmts(body, known_types, universe),
        _ => Ok(()),
    }
}

fn walk_expr(
    expr: &mut Expr,
    known_types: &HashMap<String, Type>,
    universe: &TypeUniverse,
) -> Result<(), String> {
    match expr {
        Expr::PluginIntercept { name, args, type_args: _ } => {
            if let Some(replacement) = resolve_print(name, args, known_types, universe)? {
                *expr = replacement;
            }
            Ok(())
        }
        Expr::BinaryOp(_, lhs, rhs) => {
            walk_expr(lhs, known_types, universe)?;
            walk_expr(rhs, known_types, universe)
        }
        Expr::UnaryOp(_, inner) => walk_expr(inner, known_types, universe),
        Expr::Call(_, args, _) => {
            for a in args {
                walk_expr(a, known_types, universe)?;
            }
            Ok(())
        }
        Expr::If(cond, then, else_) => {
            walk_expr(cond, known_types, universe)?;
            walk_expr(then, known_types, universe)?;
            if let Some(el) = else_ {
                walk_expr(el, known_types, universe)?;
            }
            Ok(())
        }
        Expr::Match(_, arms) => {
            for arm in arms {
                walk_expr(&mut arm.body, known_types, universe)?;
            }
            Ok(())
        }
        Expr::Block(stmts) => walk_stmts(stmts, known_types, universe),
        Expr::Tuple(elems) | Expr::List(elems) => {
            for e in elems {
                walk_expr(e, known_types, universe)?;
            }
            Ok(())
        }
        Expr::Field(obj, _) | Expr::Index(obj, _) => walk_expr(obj, known_types, universe),
        Expr::Cast(inner, _) | Expr::IsType(inner, _) | Expr::Deref(inner)
        | Expr::AddrOf(inner) => walk_expr(inner, known_types, universe),
        Expr::Within(body, _) => walk_expr(body, known_types, universe),
        Expr::Lambda(_, body) => walk_expr(body, known_types, universe),
        Expr::DerivationBlock(db) => {
            for ex in &mut db.examples {
                for inp in &mut ex.inputs {
                    walk_expr(inp, known_types, universe)?;
                }
                walk_expr(&mut ex.output, known_types, universe)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

#[derive(Clone, Copy, PartialEq)]
enum PrintKind {
    Int,
    Float,
    Str,
}

/// Determine an expression's print category for dispatch.
fn kind_from_expr(
    expr: &Expr,
    known_types: &HashMap<String, Type>,
    universe: &TypeUniverse,
) -> PrintKind {
    match expr {
        Expr::Quoted(_) => PrintKind::Str,
        Expr::Decimal(_) => PrintKind::Int,
        Expr::Float(_) => PrintKind::Float,
        Expr::Identifier(name) => {
            match known_types.get(name) {
                Some(t) => kind_from_type(t, universe),
                None => PrintKind::Int, // default fallback
            }
        }
        // 2026-07-27: For complex expressions (BinaryOp, UnaryOp, Call, Cast,
        // etc.), recurse into the expression tree to find leaf-type information.
        // Previously defaulted all non-trivial exprs to "Int", causing __print_int
        // to be called on float values — an ABI mismatch.
        _ => kind_from_expr_deep(expr, known_types, universe),
    }
}

/// 2026-07-27: Recursive expression type inference for print dispatch.
/// Walks BinaryOp/UnaryOp/Call trees to find leaf types. If any operand
/// is a float literal or float-typed variable, the result is float.
/// Conservative: errs on the side of Float (call __print_float instead of
/// __print_int) because __print_float will still print the value correctly.
fn kind_from_expr_deep(
    expr: &Expr,
    known_types: &HashMap<String, Type>,
    universe: &TypeUniverse,
) -> PrintKind {
    match expr {
        Expr::Float(_) => PrintKind::Float,
        Expr::Decimal(_) => PrintKind::Int,
        Expr::Quoted(_) => PrintKind::Str,
        Expr::Identifier(name) => {
            match known_types.get(name) {
                Some(t) => kind_from_type(t, universe),
                None => PrintKind::Int,
            }
        }
        Expr::BinaryOp(_, lhs, rhs) => {
            let lk = kind_from_expr_deep(lhs, known_types, universe);
            let rk = kind_from_expr_deep(rhs, known_types, universe);
            // If either side is float, result is float (float arithmetic propagates).
            if lk == PrintKind::Float || rk == PrintKind::Float {
                PrintKind::Float
            } else {
                PrintKind::Int
            }
        }
        Expr::UnaryOp(_, e) => kind_from_expr_deep(e, known_types, universe),
        Expr::Call(_, args, _) => {
            // Heuristic: check argument types. If any arg is float, result may be float.
            // This is conservative — some functions take float and return Int, but
            // for the print plugin, being wrong on the side of Float is safe (we'll
            // call __print_float instead of __print_int, which still prints the value).
            for arg in args {
                let ak = kind_from_expr_deep(arg, known_types, universe);
                if ak == PrintKind::Float {
                    return PrintKind::Float;
                }
            }
            PrintKind::Int
        }
        Expr::Cast(_, target) => kind_from_type(target, universe),
        Expr::Field(_, _) | Expr::Index(_, _) => {
            // Conservative: field access may return float, but we can't
            // determine the type without deeper analysis.
            PrintKind::Int
        }
        _ => PrintKind::Int,
    }
}

/// 2026-08-01: Determine print dispatch kind from a Type annotation.
/// Derived from the type's protocol category (Cast.#String / Cast.#Float)
/// in the TypeUniverse rather than matching type names (rule #18). A type
/// carrying neither protocol category prints as Int (the numerical default).
fn kind_from_type(t: &Type, universe: &TypeUniverse) -> PrintKind {
    let Some(key) = t.universe_key() else { return PrintKind::Int; };
    let Some(rt) = universe.get(key) else { return PrintKind::Int; };
    if rt.properties.contains_key("Cast.#String") {
        PrintKind::Str
    } else if rt.properties.contains_key("Cast.#Float") {
        PrintKind::Float
    } else {
        PrintKind::Int
    }
}

/// Build the runtime call for one value of a given print kind.
fn print_call(kind: PrintKind, value: &Expr) -> Expr {
    let fn_name = match kind {
        PrintKind::Str => "PrintStr#",
        PrintKind::Float => "PrintFloat#",
        PrintKind::Int => "PrintInt#",
    };
    Expr::Call(fn_name.to_string(), vec![value.clone()], None)
}

/// A newline-printing call (Char 10).
fn newline_call() -> Expr {
    Expr::Call("PrintChar#".to_string(), vec![Expr::Decimal(10)], None)
}

/// 2026-08-01: Expand a format string against its value arguments into a
/// sequence of print statements. Returns the statements and how many
/// arguments were consumed (for the surplus-argument warning).
fn expand_format(
    fmt: &[u8],
    args: &[Expr],
    known_types: &HashMap<String, Type>,
    universe: &TypeUniverse,
) -> Result<(Vec<Statement>, usize), String> {
    let parts = parse_format(fmt)?;
    let mut stmts = Vec::new();
    let mut next = 0usize;
    let mut max_used = 0usize;
    for part in &parts {
        match part {
            FmtPart::Literal(seg) => {
                if !seg.is_empty() {
                    stmts.push(Statement::Expression(Expr::Call(
                        "PrintStr#".to_string(),
                        vec![Expr::Quoted(seg.clone())],
                        None,
                    )));
                }
            }
            FmtPart::Next => {
                if next >= args.len() {
                    return Err(format!(
                        "format string references argument {} but only {} were supplied",
                        next,
                        args.len()
                    ));
                }
                stmts.push(Statement::Expression(print_call(
                    kind_from_expr(&args[next], known_types, universe),
                    &args[next],
                )));
                next += 1;
            }
            FmtPart::Position(n) => {
                if *n >= args.len() {
                    return Err(format!(
                        "format string argument {{{}}} is out of range ({} were supplied)",
                        n,
                        args.len()
                    ));
                }
                stmts.push(Statement::Expression(print_call(
                    kind_from_expr(&args[*n], known_types, universe),
                    &args[*n],
                )));
                max_used = max_used.max(*n + 1);
            }
        }
    }
    Ok((stmts, next.max(max_used)))
}

/// Resolve a `print!` or `println!` invocation to typed runtime calls.
/// Returns Ok(None) when `name` is not one of ours (e.g. a PascalCase
/// legacy name — the typechecker rejects those with a rename hint).
fn resolve_print(
    name: &str,
    args: &[Expr],
    known_types: &HashMap<String, Type>,
    universe: &TypeUniverse,
) -> Result<Option<Expr>, String> {
    let is_println = name == "println";
    if !is_println && name != "print" {
        return Ok(None);
    }

    // println!() — newline only.
    if args.is_empty() {
        if is_println {
            return Ok(Some(newline_call()));
        }
        return Err("print! requires a value or a format-string argument".to_string());
    }

    let mut stmts = Vec::new();
    if let Expr::Quoted(fmt) = &args[0] {
        // Rust-style format string: value arguments follow the literal.
        let (mut parts, used) = expand_format(fmt, &args[1..], known_types, universe)?;
        let supplied = args.len() - 1;
        if supplied > used {
            eprintln!(
                "print plugin: warning: {} format-string argument(s) unused in {:?}",
                supplied - used,
                String::from_utf8_lossy(fmt)
            );
        }
        stmts.append(&mut parts);
    } else {
        // Legacy value form: a single non-literal argument printed directly.
        let kind = kind_from_expr(&args[0], known_types, universe);
        stmts.push(Statement::Expression(print_call(kind, &args[0])));
    }

    if is_println {
        stmts.push(Statement::Expression(newline_call()));
    }

    // A lone call stands alone (preserves value/expression semantics);
    // multiple calls form a block.
    if stmts.len() == 1 {
        if let Some(Statement::Expression(e)) = stmts.pop() {
            return Ok(Some(e));
        }
    }
    Ok(Some(Expr::Block(stmts)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_plain_literal() {
        assert_eq!(parse_format(b"hello").unwrap(), vec![FmtPart::Literal(b"hello".to_vec())]);
    }

    #[test]
    fn parse_sequential_placeholder() {
        assert_eq!(
            parse_format(b"x={}").unwrap(),
            vec![FmtPart::Literal(b"x=".to_vec()), FmtPart::Next]
        );
    }

    #[test]
    fn parse_positional_placeholder() {
        assert_eq!(
            parse_format(b"{1}:{0}").unwrap(),
            vec![FmtPart::Position(1), FmtPart::Literal(b":".to_vec()), FmtPart::Position(0)]
        );
    }

    #[test]
    fn parse_escaped_braces() {
        assert_eq!(
            parse_format(b"a{{b}}c").unwrap(),
            vec![FmtPart::Literal(b"a{b}c".to_vec())]
        );
    }

    #[test]
    fn parse_unmatched_brace_errors() {
        assert!(parse_format(b"oops {").is_err());
        assert!(parse_format(b"oops }").is_err());
    }

    #[test]
    fn parse_bad_placeholder_errors() {
        assert!(parse_format(b"{abc}").is_err());
        assert!(parse_format(b"{-1}").is_err());
    }

    #[test]
    fn expand_out_of_range_positional_errors() {
        let universe = TypeUniverse::new();
        let known = HashMap::new();
        let err = expand_format(b"{1}", &[], &known, &universe).unwrap_err();
        assert!(err.contains("out of range"), "err was: {err}");
    }

    #[test]
    fn expand_too_few_sequential_errors() {
        let universe = TypeUniverse::new();
        let known = HashMap::new();
        let err = expand_format(b"{}", &[], &known, &universe).unwrap_err();
        assert!(err.contains("only 0 were supplied"), "err was: {err}");
    }

    #[test]
    fn expand_value_kinds() {
        let universe = TypeUniverse::new();
        let known = HashMap::new();
        let (stmts, used) = expand_format(b"{}, {1}, {0}", &[Expr::Decimal(7), Expr::Float(1.5)], &known, &universe).unwrap();
        // {} -> PrintInt#(7), ", " -> literal, {1} -> PrintFloat#(1.5),
        // ", " -> literal, {0} -> PrintFloat#(1.5).
        assert_eq!(stmts.len(), 5);
        assert_eq!(used, 2);
        match &stmts[0] {
            Statement::Expression(Expr::Call(name, args, _)) => {
                assert_eq!(name, "PrintInt#");
                assert_eq!(args[0], Expr::Decimal(7));
            }
            other => panic!("expected call, got {other:?}"),
        }
        match &stmts[1] {
            Statement::Expression(Expr::Call(name, args, _)) => {
                assert_eq!(name, "PrintStr#");
                assert_eq!(args[0], Expr::Quoted(b", ".to_vec()));
            }
            other => panic!("expected call, got {other:?}"),
        }
        match &stmts[2] {
            Statement::Expression(Expr::Call(name, args, _)) => {
                assert_eq!(name, "PrintFloat#");
                assert_eq!(args[0], Expr::Float(1.5));
            }
            other => panic!("expected call, got {other:?}"),
        }
        match &stmts[4] {
            Statement::Expression(Expr::Call(name, args, _)) => {
                assert_eq!(name, "PrintInt#");
                assert_eq!(args[0], Expr::Decimal(7));
            }
            other => panic!("expected call, got {other:?}"),
        }
    }
}
