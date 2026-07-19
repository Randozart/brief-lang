// ── Print Plugin — Front + Mid Stage ───────────────────────────────────
// 2026-07-19: Resolves !Print(x) and !PrintLn(x) to typed stdlib calls
// (print_int, print_str, print_float).
//
// Front stage: collect variable type annotations from let declarations.
// Mid stage: resolve PluginIntercept using collected types + literal inference.

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
        vec![StageKind::Front, StageKind::Mid]
    }

    fn on_ast(
        &self,
        program: &mut Vec<TopLevel>,
        _universe: &mut TypeUniverse,
    ) -> Result<(), String> {
        let mut known_types: HashMap<String, Type> = HashMap::new();

        // Front pass: collect let binding type annotations
        for item in program.iter() {
            collect_binding_types(item, &mut known_types);
        }

        // Mid pass: resolve PluginIntercept using known types
        let ctx = TypeEnv { known_types: &known_types };
        for item in program.iter_mut() {
            walk_item(item, &ctx);
        }
        Ok(())
    }
}

struct TypeEnv<'a> {
    known_types: &'a HashMap<String, Type>,
}

fn collect_binding_types(item: &TopLevel, map: &mut HashMap<String, Type>) {
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

fn walk_item(item: &mut TopLevel, ctx: &TypeEnv) {
    match item {
        TopLevel::Definition(d) => walk_stmts(&mut d.body, ctx),
        TopLevel::Transaction(t) => walk_stmts(&mut t.body, ctx),
        TopLevel::Constant(c) => walk_expr(&mut c.expr, ctx),
        TopLevel::Statement(stmt) => walk_stmt(stmt, ctx),
        _ => {}
    }
}

fn walk_stmts(stmts: &mut [crate::ast::Statement], ctx: &TypeEnv) {
    for stmt in stmts.iter_mut() {
        walk_stmt(stmt, ctx);
    }
}

fn walk_stmt(stmt: &mut crate::ast::Statement, ctx: &TypeEnv) {
    match stmt {
        crate::ast::Statement::Assign(_, expr)
        | crate::ast::Statement::Let { expr: Some(expr), .. }
        | crate::ast::Statement::Expression(expr)
        | crate::ast::Statement::Term(Some(expr))
        | crate::ast::Statement::TermBang(Some(expr)) => {
            walk_expr(expr, ctx);
        }
        crate::ast::Statement::Guarded(_, body) => walk_stmts(body, ctx),
        _ => {}
    }
}

fn walk_expr(expr: &mut Expr, ctx: &TypeEnv) {
    match expr {
        Expr::PluginIntercept { name, args, type_args: _ } => {
            if let Some(replacement) = resolve_print(name, args, ctx) {
                *expr = replacement;
            }
        }
        Expr::BinaryOp(_, lhs, rhs) => { walk_expr(lhs, ctx); walk_expr(rhs, ctx); }
        Expr::UnaryOp(_, inner) => walk_expr(inner, ctx),
        Expr::Call(_, args, _) => { for a in args { walk_expr(a, ctx); } }
        Expr::If(cond, then, else_) => {
            walk_expr(cond, ctx); walk_expr(then, ctx);
            if let Some(el) = else_ { walk_expr(el, ctx); }
        }
        Expr::Match(_, arms) => { for arm in arms { walk_expr(&mut arm.body, ctx); } }
        Expr::Block(stmts) => walk_stmts(stmts, ctx),
        Expr::Tuple(elems) | Expr::List(elems) => { for e in elems { walk_expr(e, ctx); } }
        Expr::Field(obj, _) | Expr::Index(obj, _) => walk_expr(obj, ctx),
        Expr::Cast(inner, _) | Expr::IsType(inner, _) | Expr::Deref(inner)
        | Expr::AddrOf(inner) => walk_expr(inner, ctx),
        Expr::Within(body, _) => walk_expr(body, ctx),
        Expr::Lambda(_, body) => walk_expr(body, ctx),
        Expr::DerivationBlock(db) => {
            for ex in &mut db.examples {
                for inp in &mut ex.inputs { walk_expr(inp, ctx); }
                walk_expr(&mut ex.output, ctx);
            }
        }
        _ => {}
    }
}

/// Determine the type of an expression for dispatch.
fn expr_type(expr: &Expr, ctx: &TypeEnv) -> Option<&'static str> {
    match expr {
        Expr::Quoted(_) => Some("String"),
        Expr::Decimal(_) => Some("Int"),
        Expr::Float(_) => Some("Float"),
        Expr::Identifier(name) => {
            ctx.known_types.get(name).map(|t| {
                if *t == Type::int() || *t == Type::bits(8) { "Int" }
                else if *t == Type::string() { "String" }
                else if *t == Type::float() || *t == Type::float64() { "Float" }
                else { "Int" } // default fallback
            })
        }
        // For complex expressions, try the first argument of calls
        Expr::Call(_, args, _) => args.first().and_then(|a| expr_type(a, ctx)),
        _ => None,
    }
}

/// Resolve !Print(x) or !PrintLn(x) to a typed stdlib call.
fn resolve_print(name: &str, args: &[Expr], ctx: &TypeEnv) -> Option<Expr> {
    if args.is_empty() {
        return None;
    }
    let value = &args[0];

    let print_fn = expr_type(value, ctx).and_then(|t| match t {
        "String" => Some("__print_str"),
        "Float" => Some("__print_float"),
        _ => Some("__print_int"),
    }).unwrap_or("__print_int");

    let print_call = Expr::Call(print_fn.to_string(), vec![value.clone()], None);

    if name == "PrintLn" {
        let newline_call = Expr::Call("__print_char".to_string(), vec![Expr::Decimal(10)], None);
        Some(Expr::Block(vec![
            crate::ast::Statement::Expression(print_call),
            crate::ast::Statement::Expression(newline_call),
        ]))
    } else {
        Some(print_call)
    }
}
