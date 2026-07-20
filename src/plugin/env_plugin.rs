// ── Environment Plugin — Front Stage ────────────────────────────────────
// 2026-07-19: Resolves !GetEnv(name) and !GetEnvInt(name) to their stdlib
// equivalents: getenv(name) and get_env_int(name). Runs at Front stage so
// replacements are visible to the typechecker.

use crate::ast::{Expr, StageKind, TopLevel};
use crate::plugin::Plugin;
use crate::type_universe::TypeUniverse;

/// Environment plugin: replaces PluginIntercept calls for env var access.
#[derive(Debug)]
pub struct EnvPlugin;

impl Plugin for EnvPlugin {
    fn name(&self) -> &str {
        "env"
    }

    fn stages(&self) -> Vec<StageKind> {
        vec![StageKind::Front]
    }

    fn on_ast(
        &self,
        program: &mut Vec<TopLevel>,
        _universe: &mut TypeUniverse,
    ) -> Result<(), String> {
        // 2026-07-19: First pass — resolve const-level !GetEnv/!GetEnvInt to
        // literal values. This allows `const N = !GetEnvInt("BOUND")` to work
        // even though get_env_int is a regular function (not an intrinsic).
        resolve_const_env_vars(program);
        // Second pass — normal resolution for non-const contexts
        let orig_count = count_intercepts(program);
        for item in program.iter_mut() {
            walk_item(item);
        }
        if orig_count > 0 {
            let _new_count = count_intercepts(program);
        }
        Ok(())
    }
}

/// Evaluate !GetEnv/!GetEnvInt inside const declarations at compile time.
fn resolve_const_env_vars(program: &mut [TopLevel]) {
    for item in program.iter_mut() {
        let TopLevel::Constant(c) = item else { continue };
        let Expr::PluginIntercept { name, args, .. } = &c.expr else { continue };
        let key = extract_string_arg(args).unwrap_or_default();
        let val = std::env::var(&key).unwrap_or_default();
        c.expr = match name.as_str() {
            "GetEnvInt" => Expr::Decimal(val.parse::<i64>().unwrap_or(0)),
            "GetEnv"    => Expr::Quoted(val.as_bytes().to_vec()),
            _           => continue,
        };
    }
}

/// Extract a string literal argument from a PluginIntercept call.
fn extract_string_arg(args: &[Expr]) -> Option<String> {
    if let Some(Expr::Quoted(bytes)) = args.first() {
        String::from_utf8(bytes.clone()).ok()
    } else {
        None
    }
}

fn count_intercepts(program: &[TopLevel]) -> usize {
    let mut count = 0;
    for item in program {
        count_expr_intercepts(item, &mut count);
    }
    count
}

fn count_expr_intercepts(item: &TopLevel, count: &mut usize) {
    match item {
        TopLevel::Definition(d) => { for s in &d.body { count_stmt_intercepts(s, count); } }
        TopLevel::Transaction(t) => { for s in &t.body { count_stmt_intercepts(s, count); } }
        TopLevel::Constant(c) => { count_intercepts_in_expr(&c.expr, count); }
        TopLevel::Statement(stmt) => { count_stmt_intercepts(stmt, count); }
        _ => {}
    }
}

fn count_stmt_intercepts(stmt: &crate::ast::Statement, count: &mut usize) {
    match stmt {
        crate::ast::Statement::Assign(_, expr)
        | crate::ast::Statement::Let { expr: Some(expr), .. }
        | crate::ast::Statement::Expression(expr)
        | crate::ast::Statement::Term(Some(expr))
        | crate::ast::Statement::TermBang(Some(expr)) => {
            count_intercepts_in_expr(expr, count);
        }
        _ => {}
    }
}

fn count_intercepts_in_expr(expr: &Expr, count: &mut usize) {
    match expr {
        Expr::PluginIntercept { .. } => { *count += 1; }
        Expr::BinaryOp(_, l, r) => { count_intercepts_in_expr(l, count); count_intercepts_in_expr(r, count); }
        Expr::UnaryOp(_, e) => count_intercepts_in_expr(e, count),
        Expr::Call(_, args, _) => { for a in args { count_intercepts_in_expr(a, count); } }
        Expr::If(c, t, e) => { count_intercepts_in_expr(c, count); count_intercepts_in_expr(t, count); if let Some(e) = e { count_intercepts_in_expr(e, count); } }
        Expr::Block(stmts) => { for s in stmts { count_stmt_intercepts(s, count); } }
        Expr::Match(_, arms) => { for a in arms { count_intercepts_in_expr(&a.body, count); } }
        Expr::Tuple(elems) | Expr::List(elems) => { for e in elems { count_intercepts_in_expr(e, count); } }
        Expr::Field(obj, _) | Expr::Index(obj, _) => count_intercepts_in_expr(obj, count),
        Expr::Cast(e, _) | Expr::IsType(e, _) | Expr::Deref(e) | Expr::AddrOf(e) => count_intercepts_in_expr(e, count),
        Expr::Within(body, _) => count_intercepts_in_expr(body, count),
        Expr::Lambda(_, body) => count_intercepts_in_expr(body, count),
        Expr::DerivationBlock(db) => {
            for ex in &db.examples {
                for inp in &ex.inputs { count_intercepts_in_expr(inp, count); }
                count_intercepts_in_expr(&ex.output, count);
            }
        }
        _ => {}
    }
}

fn walk_item(item: &mut TopLevel) {
    match item {
        TopLevel::Definition(d) => walk_stmts(&mut d.body),
        TopLevel::Transaction(t) => walk_stmts(&mut t.body),
        TopLevel::Constant(c) => walk_expr(&mut c.expr),
        TopLevel::Statement(stmt) => walk_stmt(stmt),
        TopLevel::StateDecl(_) | TopLevel::Trigger(_) => {}
        TopLevel::ForeignBinding(_) => {}
        _ => {}
    }
}

fn walk_stmts(stmts: &mut [crate::ast::Statement]) {
    for stmt in stmts.iter_mut() {
        walk_stmt(stmt);
    }
}

fn walk_stmt(stmt: &mut crate::ast::Statement) {
    match stmt {
        crate::ast::Statement::Assign(_, expr)
        | crate::ast::Statement::Let { expr: Some(expr), .. } => {
            walk_expr(expr);
        }
        crate::ast::Statement::Expression(expr)
        | crate::ast::Statement::Term(Some(expr))
        | crate::ast::Statement::TermBang(Some(expr)) => {
            walk_expr(expr);
        }
        crate::ast::Statement::Guarded(_, body) => {
            walk_stmts(body);
        }
        _ => {}
    }
}

fn walk_expr(expr: &mut Expr) {
    match expr {
        Expr::PluginIntercept { name, args, type_args: _ } => {
            eprintln!("env plugin: found PluginIntercept '{}'", name);
            if let Some(replacement) = resolve_intercept(name, args) {
                eprintln!("env plugin: replacing '{}' with '{}'", name,
                    match &replacement { Expr::Call(n, _, _) => n, _ => "?" });
                *expr = replacement;
            } else {
                eprintln!("env plugin: no replacement for '{}'", name);
            }
        }
        Expr::BinaryOp(_, lhs, rhs) => {
            walk_expr(lhs);
            walk_expr(rhs);
        }
        Expr::UnaryOp(_, inner) => walk_expr(inner),
        Expr::Call(_, args, _) => {
            for a in args { walk_expr(a); }
        }
        Expr::If(cond, then, else_) => {
            walk_expr(cond);
            walk_expr(then);
            if let Some(el) = else_ { walk_expr(el); }
        }
        Expr::Match(_, arms) => {
            for arm in arms { walk_expr(&mut arm.body); }
        }
        Expr::Block(stmts) => walk_stmts(stmts),
        Expr::Tuple(elems) | Expr::List(elems) => {
            for e in elems { walk_expr(e); }
        }
        Expr::Field(obj, _) | Expr::Index(obj, _) => walk_expr(obj),
        Expr::Cast(inner, _) | Expr::IsType(inner, _) | Expr::Deref(inner)
        | Expr::AddrOf(inner) => walk_expr(inner),
        Expr::Within(body, _) => walk_expr(body),
        Expr::Lambda(_, body) => walk_expr(body),
        Expr::DerivationBlock(db) => {
            for ex in &mut db.examples {
                for inp in &mut ex.inputs { walk_expr(inp); }
                walk_expr(&mut ex.output);
            }
        }
        // Leaves — no sub-expressions
        Expr::Decimal(_) | Expr::TaggedLiteral(_, _) | Expr::Bool(_) | Expr::Float(_) | Expr::Quoted(_)
        | Expr::Identifier(_) | Expr::PropertyGet(_)
        | Expr::FormattingAnnotation(_) => {}
    }
}

/// Resolve a plugin-intercept call to a typed stdlib function call.
fn resolve_intercept(name: &str, args: &[Expr]) -> Option<Expr> {
    let call_args = args.to_vec();
    match name {
        "GetEnv" => Some(Expr::Call("get_env".to_string(), call_args, None)),
        "GetEnvInt" => Some(Expr::Call("get_env_int".to_string(), call_args, None)),
        _ => None,
    }
}
