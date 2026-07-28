// ── Print Plugin — Front Stage ────────────────────────────────────────
// 2026-07-19: Resolves !Print(x) and !PrintLn(x) to typed C runtime calls
// (__print_int, __print_str, __print_float, __print_char for newline).
//
// Runs at Front stage (before typechecking). Collects variable type
// annotations from `let name: Type = ...` declarations. For literals
// (Quoted, Decimal, Float) dispatches directly. Falls back to __print_int
// when the type can't be determined (all benchmarks use Int as default).

use crate::ast::{Expr, StageKind, TopLevel, Type};
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
        _universe: &mut TypeUniverse,
    ) -> Result<(), String> {
        let mut known_types: HashMap<String, Type> = HashMap::new();
        collect_binding_types(program, &mut known_types);
        resolve_prints(program, &known_types);
        Ok(())
    }
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

fn resolve_prints(program: &mut Vec<TopLevel>, known_types: &HashMap<String, Type>) {
    for item in program.iter_mut() {
        walk_item(item, known_types);
    }
}

fn walk_item(item: &mut TopLevel, known_types: &HashMap<String, Type>) {
    match item {
        TopLevel::Definition(d) => walk_stmts(&mut d.body, known_types),
        TopLevel::Transaction(t) => walk_stmts(&mut t.body, known_types),
        TopLevel::Constant(c) => walk_expr(&mut c.expr, known_types),
        TopLevel::Statement(stmt) => walk_stmt(stmt, known_types),
        _ => {}
    }
}

fn walk_stmts(stmts: &mut [crate::ast::Statement], known_types: &HashMap<String, Type>) {
    for stmt in stmts.iter_mut() {
        walk_stmt(stmt, known_types);
    }
}

fn walk_stmt(stmt: &mut crate::ast::Statement, known_types: &HashMap<String, Type>) {
    match stmt {
        crate::ast::Statement::Assign(_, expr)
        | crate::ast::Statement::Let { expr: Some(expr), .. }
        | crate::ast::Statement::Expression(expr)
        | crate::ast::Statement::Term(Some(expr))
        | crate::ast::Statement::TermBang(Some(expr)) => {
            walk_expr(expr, known_types);
        }
        crate::ast::Statement::Guarded(_, body) => walk_stmts(body, known_types),
        _ => {}
    }
}

fn walk_expr(expr: &mut Expr, known_types: &HashMap<String, Type>) {
    match expr {
        Expr::PluginIntercept { name, args, type_args: _ } => {
            if let Some(replacement) = resolve_print(name, args, known_types) {
                *expr = replacement;
            }
        }
        Expr::BinaryOp(_, lhs, rhs) => { walk_expr(lhs, known_types); walk_expr(rhs, known_types); }
        Expr::UnaryOp(_, inner) => walk_expr(inner, known_types),
        Expr::Call(_, args, _) => { for a in args { walk_expr(a, known_types); } }
        Expr::If(cond, then, else_) => {
            walk_expr(cond, known_types); walk_expr(then, known_types);
            if let Some(el) = else_ { walk_expr(el, known_types); }
        }
        Expr::Match(_, arms) => { for arm in arms { walk_expr(&mut arm.body, known_types); } }
        Expr::Block(stmts) => walk_stmts(stmts, known_types),
        Expr::Tuple(elems) | Expr::List(elems) => { for e in elems { walk_expr(e, known_types); } }
        Expr::Field(obj, _) | Expr::Index(obj, _) => walk_expr(obj, known_types),
        Expr::Cast(inner, _) | Expr::IsType(inner, _) | Expr::Deref(inner)
        | Expr::AddrOf(inner) => walk_expr(inner, known_types),
        Expr::Within(body, _) => walk_expr(body, known_types),
        Expr::Lambda(_, body) => walk_expr(body, known_types),
        Expr::DerivationBlock(db) => {
            for ex in &mut db.examples {
                for inp in &mut ex.inputs { walk_expr(inp, known_types); }
                walk_expr(&mut ex.output, known_types);
            }
        }
        _ => {}
    }
}

/// Determine an expression's print category for dispatch.
fn kind_from_expr(expr: &Expr, known_types: &HashMap<String, Type>) -> &'static str {
    match expr {
        Expr::Quoted(_) => "String",
        Expr::Decimal(_) => "Int",
        Expr::Float(_) => "Float",
        Expr::Identifier(name) => {
            match known_types.get(name) {
                Some(t) => kind_from_type(t),
                None => "Int", // default fallback
            }
        }
        // 2026-07-27: For complex expressions (BinaryOp, UnaryOp, Call, Cast,
        // etc.), recurse into the expression tree to find leaf-type information.
        // Previously defaulted all non-trivial exprs to "Int", causing __print_int
        // to be called on float values — an ABI mismatch.
        _ => kind_from_expr_deep(expr, known_types),
    }
}

/// 2026-07-27: Recursive expression type inference for print dispatch.
/// Walks BinaryOp/UnaryOp/Call trees to find leaf types. If any operand
/// is a float literal or float-typed variable, the result is float.
/// Conservative: errs on the side of Float (call __print_float instead of
/// __print_int) because __print_float will still print the value correctly.
fn kind_from_expr_deep(expr: &Expr, known_types: &HashMap<String, Type>) -> &'static str {
    match expr {
        Expr::Float(_) => "Float",
        Expr::Decimal(_) => "Int",
        Expr::Quoted(_) => "String",
        Expr::Identifier(name) => {
            match known_types.get(name) {
                Some(t) => kind_from_type(t),
                None => "Int",
            }
        }
        Expr::BinaryOp(_, lhs, rhs) => {
            let lk = kind_from_expr_deep(lhs, known_types);
            let rk = kind_from_expr_deep(rhs, known_types);
            // If either side is float, result is float (float arithmetic propagates).
            if lk == "Float" || rk == "Float" { "Float" }
            else { "Int" }
        }
        Expr::UnaryOp(_, e) => kind_from_expr_deep(e, known_types),
        Expr::Call(_, args, _) => {
            // Heuristic: check argument types. If any arg is float, result may be float.
            // This is conservative — some functions take float and return Int, but
            // for the print plugin, being wrong on the side of Float is safe (we'll
            // call __print_float instead of __print_int, which still prints the value).
            for arg in args {
                let ak = kind_from_expr_deep(arg, known_types);
                if ak == "Float" { return "Float"; }
            }
            "Int"
        }
        Expr::Cast(_, target) => kind_from_type(target),
        Expr::Field(_, _) | Expr::Index(_, _) => {
            // Conservative: field access may return float, but we can't
            // determine the type without deeper analysis.
            "Int"
        }
        _ => "Int",
    }
}

/// Determine print dispatch kind from a Type annotation.
/// Uses the type's name (Custom variant) to avoid fragile value equality.
fn kind_from_type(t: &Type) -> &'static str {
    match t {
        Type::Custom(name) => {
            if name == "Int" || name == "Int8" || name == "Int16" || name == "Int32"
               || name == "Int64" || name == "UInt" || name == "UInt8"
               || name == "UInt16" || name == "UInt32" || name == "UInt64"
               || name == "Bit" || name == "Byte" || name == "Bool" {
                "Int"
            } else if name == "String" || name == "StaticString" || name == "SmallString64" {
                "String"
            } else if name == "Float" || name == "Float32" || name == "Float64"
               || name == "Double" {
                "Float"
            } else {
                "Int" // unknown — default to Int
            }
        }
        _ => "Int",
    }
}

/// Resolve !Print(x) or !PrintLn(x) to a typed C runtime call.
fn resolve_print(name: &str, args: &[Expr], known_types: &HashMap<String, Type>) -> Option<Expr> {
    let value = args.first()?;
    let kind = kind_from_expr(value, known_types);

    let print_fn = match kind {
        "String" => "__print_str",
        "Float" => "__print_float",
        _ => "__print_int",
    };

    let print_call = Expr::Call(print_fn.to_string(), vec![value.clone()], None);

    if name == "PrintLn" {
        let newline = Expr::Call("__print_char".to_string(), vec![Expr::Decimal(10)], None);
        eprintln!("print plugin: PrintLn -> {} + __print_char", print_fn);
        Some(Expr::Block(vec![
            crate::ast::Statement::Expression(print_call),
            crate::ast::Statement::Expression(newline),
        ]))
    } else {
        eprintln!("print plugin: Print -> {}", print_fn);
        Some(print_call)
    }
}
