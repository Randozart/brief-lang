use crate::ast::{Expr, Program, Statement, TopLevel};
use crate::features::macros::context::{MacroContext, MacroDef, TemplateDef};
use crate::features::macros::template;

/// Phase 1a: Expand all template calls in the program.
pub fn expand_templates(program: &mut Program, ctx: &mut MacroContext) -> Result<(), String> {
    collect_template_defs(program, ctx);
    expand_template_calls_in_items(&mut program.items, ctx)
}

/// Phase 1b: Expand all macro calls in the program.
/// Re-runs Phase 1a on macro output since macros can emit template calls.
pub fn expand_macros(program: &mut Program, ctx: &mut MacroContext) -> Result<(), String> {
    collect_macro_defs(program, ctx);
    expand_macro_calls_in_items(&mut program.items, ctx)?;
    // Re-run Phase 1a: macros may emit template calls
    expand_template_calls_in_items(&mut program.items, ctx)
}

// ── Collection ────────────────────────────────────────────────

fn collect_template_defs(program: &mut Program, ctx: &mut MacroContext) {
    let mut i = 0;
    while i < program.items.len() {
        if let TopLevel::TemplateDef { name, params, return_type, body } = &program.items[i] {
            let def = TemplateDef { name: name.clone(), params: params.clone(), return_type: return_type.clone(), body: body.clone() };
            ctx.templates.insert(name.clone(), def);
            program.items.remove(i);
        } else { i += 1; }
    }
}

fn collect_macro_defs(program: &mut Program, ctx: &mut MacroContext) {
    let mut i = 0;
    while i < program.items.len() {
        if let TopLevel::MacroDef { name, params, return_type, body } = &program.items[i] {
            let def = MacroDef { name: name.clone(), params: params.clone(), return_type: return_type.clone(), body: body.clone() };
            ctx.macros.insert(name.clone(), def);
            program.items.remove(i);
        } else { i += 1; }
    }
}

// ── Template expansion (Phase 1a) ─────────────────────────────

fn expand_template_calls_in_items(
    items: &mut Vec<TopLevel>,
    ctx: &mut MacroContext,
) -> Result<(), String> {
    let mut i = 0;
    while i < items.len() {
        let expanded = match &items[i] {
            TopLevel::Statement(stmt) => {
                expand_template_in_stmt(stmt, ctx)?
            }
            _ => None,
        };
        if let Some(new_stmts) = expanded {
            items.remove(i);
            for (j, s) in new_stmts.into_iter().enumerate() {
                items.insert(i + j, TopLevel::Statement(Box::new(s)));
            }
            i += 1;
        } else {
            // Recurse into definitions/transactions for sub-expression templates
            match &mut items[i] {
                TopLevel::Definition(def) => {
                    expand_template_in_stmts(&mut def.body, ctx)?;
                }
                TopLevel::Transaction(txn) => {
                    expand_template_in_stmts(&mut txn.body, ctx)?;
                }
                _ => {}
            }
            i += 1;
        }
    }
    Ok(())
}

/// Try to expand a TemplateCall at the statement level.
/// Returns Some(replacement_statements) if the statement was a template call
/// that returns Stmt or Block, or None if unchanged.
fn expand_template_in_stmt(
    stmt: &Statement,
    ctx: &mut MacroContext,
) -> Result<Option<Vec<Statement>>, String> {
    if let Statement::Expression(Expr::TemplateCall { name, args, block, span }) = stmt {
        let def = match ctx.templates.get(name) {
            Some(d) => d.clone(),
            None => return Err(format!("undefined template '{}'", name)),
        };
        ctx.call_site_span = span.clone();
        let mut interpreter = crate::interpreter::Interpreter::new();
        let value = template::expand_template(ctx, &mut interpreter, &def, args, block.clone());
        ctx.call_site_span = None;
        let value = value?;
        let mut stmts = match &value {
            crate::interpreter::Value::Block(s) => s.clone(),
            crate::interpreter::Value::Stmt(s) => vec![*s.clone()],
            crate::interpreter::Value::Expr(e) => vec![Statement::Expression(*e.clone())],
            other => vec![Statement::Expression(template::expr_from_value(other))],
        };
        // Apply hygiene: rename local let bindings with __gensym_N to prevent capture
        let mut gensym = || ctx.next_gensym();
        crate::features::macros::hygiene::apply_hygiene(&mut stmts, &mut gensym);
        return Ok(Some(stmts));
    }
    // Recurse into nested statements for sub-expression template calls
    let mut changed = false;
    let mut result = stmt.clone();
    expand_template_in_stmts_inner(&mut result, ctx, &mut changed)?;
    if changed { Ok(Some(vec![result])) } else { Ok(None) }
}

fn expand_template_in_stmts(
    stmts: &mut Vec<Statement>,
    ctx: &mut MacroContext,
) -> Result<(), String> {
    let mut i = 0;
    while i < stmts.len() {
        let expanded = expand_template_in_stmt(&stmts[i], ctx)?;
        if let Some(new_stmts) = expanded {
            stmts.splice(i..=i, new_stmts);
            i += 1;
        } else {
            i += 1;
        }
    }
    Ok(())
}

/// Walk a single statement and expand any TemplateCall in sub-expression positions.
fn expand_template_in_stmts_inner(
    stmt: &mut Statement,
    ctx: &mut MacroContext,
    changed: &mut bool,
) -> Result<(), String> {
    match stmt {
        Statement::Expression(expr) => {
            if let Some(e) = expand_template_in_expr(expr, ctx)? {
                *expr = e;
                *changed = true;
            }
        }
        Statement::Let { expr, .. } => {
            if let Some(e) = expr.as_mut() {
                if let Some(expanded) = expand_template_in_expr(e, ctx)? {
                    *e = expanded;
                    *changed = true;
                }
            }
        }
        Statement::Guarded { condition, statements } => {
            if let Some(e) = expand_template_in_expr(condition, ctx)? {
                *condition = e;
                *changed = true;
            }
            for s in statements.iter_mut() {
                expand_template_in_stmts_inner(s, ctx, changed)?;
            }
        }
        Statement::Term { values, swan_song, .. } => {
            for v in values.iter_mut().flatten() {
                if let Some(e) = expand_template_in_expr(v, ctx)? {
                    *v = e;
                    *changed = true;
                }
            }
            if let Some(ss) = swan_song.as_mut() {
                expand_template_in_stmts_inner(ss, ctx, changed)?;
            }
        }
        Statement::TermBang { values, swan_song, .. } => {
            for v in values.iter_mut().flatten() {
                if let Some(e) = expand_template_in_expr(v, ctx)? {
                    *v = e;
                    *changed = true;
                }
            }
            if let Some(ss) = swan_song.as_mut() {
                expand_template_in_stmts_inner(ss, ctx, changed)?;
            }
        }
        Statement::Assignment { lhs, expr, .. } => {
            if let Some(e) = expand_template_in_expr(lhs, ctx)? {
                *lhs = e;
                *changed = true;
            }
            if let Some(e) = expand_template_in_expr(expr, ctx)? {
                *expr = e;
                *changed = true;
            }
        }
        Statement::Foreach { list, body, .. } => {
            if let Some(e) = expand_template_in_expr(list, ctx)? {
                *list = Box::new(e);
                *changed = true;
            }
            for s in body.iter_mut() {
                expand_template_in_stmts_inner(s, ctx, changed)?;
            }
        }
        Statement::SyncBlock { body } => {
            for s in body.iter_mut() {
                expand_template_in_stmts_inner(s, ctx, changed)?;
            }
        }
        Statement::Oracle { handler, body, .. } => {
            for s in handler.iter_mut() {
                expand_template_in_stmts_inner(s, ctx, changed)?;
            }
            for s in body.iter_mut() {
                expand_template_in_stmts_inner(s, ctx, changed)?;
            }
        }
        _ => {}
    }
    Ok(())
}

/// Expand a TemplateCall in expression position — only valid for templates returning Expr.
fn expand_template_in_expr(
    expr: &mut Expr,
    ctx: &mut MacroContext,
) -> Result<Option<Expr>, String> {
    let (name, args, block, span) = match expr {
        Expr::TemplateCall { name, args, block, span } => {
            (name.clone(), args.clone(), block.clone(), span.clone())
        }
        _ => return Ok(None),
    };
    if let Some(def) = ctx.templates.get(&name).cloned() {
        let mut interpreter = crate::interpreter::Interpreter::new();
        ctx.call_site_span = span;
        let value = template::expand_template(ctx, &mut interpreter, &def, &args, block);
        ctx.call_site_span = None;
        let value = value?;
        match &value {
            crate::interpreter::Value::Expr(e) => Ok(Some(*e.clone())),
            other => Err(format!(
                "template '{}' returned {:?} but expression context requires Expr", name, other
            )),
        }
    } else {
        Ok(None)
    }
}

// ── Macro expansion (Phase 1b) ────────────────────────────────

fn expand_macro_calls_in_items(
    items: &mut Vec<TopLevel>,
    ctx: &mut MacroContext,
) -> Result<(), String> {
    let mut i = 0;
    while i < items.len() {
        match &items[i] {
            TopLevel::Statement(stmt) => {
                let expanded = expand_macro_in_stmt(stmt, ctx)?;
                if let Some(new_stmts) = expanded {
                    items.remove(i);
                    for (j, s) in new_stmts.into_iter().enumerate() {
                        items.insert(i + j, TopLevel::Statement(Box::new(s)));
                    }
                    i += 1;
                } else {
                    i += 1;
                }
            }
            _ => i += 1,
        }
    }
    Ok(())
}

/// Try to expand a MacroCall at the statement level, including nested positions.
fn expand_macro_in_stmt(
    stmt: &Statement,
    ctx: &mut MacroContext,
) -> Result<Option<Vec<Statement>>, String> {
    if let Statement::Expression(Expr::MacroCall { name, args, block, span }) = stmt {
        let def = ctx.macros.get(name)
            .ok_or_else(|| format!("undefined macro '{}'", name))?
            .clone();
        ctx.call_site_span = span.clone();
        let mut interpreter = crate::interpreter::Interpreter::new();
        let value = template::expand_macro(ctx, &mut interpreter, &def, args, block.clone());
        ctx.call_site_span = None;
        let value = value?;
        return Ok(Some(template::value_to_statements(&value)));
    }
    // Recurse into nested statements
    let mut changed = false;
    let mut result = stmt.clone();
    expand_macro_in_stmts_inner(&mut result, ctx, &mut changed)?;
    if changed { Ok(Some(vec![result])) } else { Ok(None) }
}

fn expand_macro_in_stmts_inner(
    stmt: &mut Statement,
    ctx: &mut MacroContext,
    changed: &mut bool,
) -> Result<(), String> {
    match stmt {
        Statement::Expression(expr) => {
            if let Some(e) = expand_macro_in_expr(expr, ctx)? {
                *expr = e;
                *changed = true;
            }
        }
        Statement::Let { expr, .. } => {
            if let Some(e) = expr.as_mut() {
                if let Some(expanded) = expand_macro_in_expr(e, ctx)? {
                    *e = expanded;
                    *changed = true;
                }
            }
        }
        Statement::Guarded { condition, statements } => {
            if let Some(e) = expand_macro_in_expr(condition, ctx)? {
                *condition = e;
                *changed = true;
            }
            for s in statements.iter_mut() {
                expand_macro_in_stmts_inner(s, ctx, changed)?;
            }
        }
        Statement::Term { values, swan_song, .. } => {
            for v in values.iter_mut().flatten() {
                if let Some(e) = expand_macro_in_expr(v, ctx)? {
                    *v = e;
                    *changed = true;
                }
            }
            if let Some(ss) = swan_song.as_mut() {
                expand_macro_in_stmts_inner(ss, ctx, changed)?;
            }
        }
        Statement::TermBang { values, swan_song, .. } => {
            for v in values.iter_mut().flatten() {
                if let Some(e) = expand_macro_in_expr(v, ctx)? {
                    *v = e;
                    *changed = true;
                }
            }
            if let Some(ss) = swan_song.as_mut() {
                expand_macro_in_stmts_inner(ss, ctx, changed)?;
            }
        }
        Statement::Assignment { lhs, expr, .. } => {
            if let Some(e) = expand_macro_in_expr(lhs, ctx)? {
                *lhs = e;
                *changed = true;
            }
            if let Some(e) = expand_macro_in_expr(expr, ctx)? {
                *expr = e;
                *changed = true;
            }
        }
        Statement::Foreach { list, body, .. } => {
            if let Some(e) = expand_macro_in_expr(list, ctx)? {
                *list = Box::new(e);
                *changed = true;
            }
            for s in body.iter_mut() {
                expand_macro_in_stmts_inner(s, ctx, changed)?;
            }
        }
        Statement::SyncBlock { body } => {
            for s in body.iter_mut() {
                expand_macro_in_stmts_inner(s, ctx, changed)?;
            }
        }
        Statement::Oracle { handler, body, .. } => {
            for s in handler.iter_mut() {
                expand_macro_in_stmts_inner(s, ctx, changed)?;
            }
            for s in body.iter_mut() {
                expand_macro_in_stmts_inner(s, ctx, changed)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn expand_macro_in_expr(
    expr: &mut Expr,
    ctx: &mut MacroContext,
) -> Result<Option<Expr>, String> {
    let (name, args, block, span) = match expr {
        Expr::MacroCall { name, args, block, span } => {
            (name.clone(), args.clone(), block.clone(), span.clone())
        }
        _ => return Ok(None),
    };
    if let Some(def) = ctx.macros.get(&name).cloned() {
        let mut interpreter = crate::interpreter::Interpreter::new();
        ctx.call_site_span = span;
        let value = template::expand_macro(ctx, &mut interpreter, &def, &args, block);
        ctx.call_site_span = None;
        let value = value?;
        // In expression context we can only return a single Expr
        match &value {
            crate::interpreter::Value::Expr(e) => Ok(Some(*e.clone())),
            other => Err(format!(
                "macro '{}' returned {:?} but expression context requires Expr", name, other
            )),
        }
    } else {
        Ok(None)
    }
}

// ── Validation ────────────────────────────────────────────────

/// Validate that no compile-time-only intrinsics remain after macro expansion.
pub fn validate_no_compile_time_intrinsics(program: &Program) -> Result<(), String> {
    for item in &program.items {
        if let TopLevel::Statement(stmt) = item {
            check_stmt_for_intrinsics(stmt)?;
        }
    }
    Ok(())
}

fn check_stmt_for_intrinsics(stmt: &Statement) -> Result<(), String> {
    match stmt {
        Statement::Expression(expr) => check_expr_for_intrinsics(expr),
        Statement::Let { expr, .. } => {
            if let Some(e) = expr { check_expr_for_intrinsics(e) } else { Ok(()) }
        }
        Statement::Guarded { condition, statements } => {
            check_expr_for_intrinsics(condition)?;
            for s in statements { check_stmt_for_intrinsics(s)?; }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn check_expr_for_intrinsics(expr: &Expr) -> Result<(), String> {
    if let Expr::IntrinsicCall { intrinsic, .. } = expr {
        if intrinsic.is_compile_time_only() {
            return Err(format!(
                "compile-time-only intrinsic {}/# survived expansion — compiler bug",
                intrinsic.name()
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_templates_collects_defs() {
        let mut program = Program {
            items: vec![
                TopLevel::TemplateDef { name: "foo".to_string(), params: vec![], return_type: None, body: vec![] },
                TopLevel::TemplateDef { name: "bar".to_string(), params: vec![], return_type: None, body: vec![] },
            ],
            comments: vec![], reactor_speed: None, attrs: vec![], ffi: None,
            strict_mode: crate::ast::StrictMode::Off, dispatch_mode: crate::ast::DispatchMode::Sequential,
            exit_condition: None, out_pragmas: vec![], default_sig_modifier: None,
        };
        let mut ctx = MacroContext::new();
        assert!(expand_templates(&mut program, &mut ctx).is_ok());
        assert!(program.items.is_empty());
    }

    #[test]
    fn test_expand_expr_template() {
        // Template that returns Expr: should work in expression position
        let mut ctx = MacroContext::new();
        ctx.templates.insert("double".to_string(), TemplateDef {
            name: "double".to_string(),
            params: vec![("x".to_string(), crate::ast::MacroArgType::Expr)],
            return_type: Some(crate::ast::MacroArgType::Expr),
            body: vec![Statement::Term {
                values: vec![Some(Expr::Add(
                    Box::new(Expr::Interpolate("x".to_string())),
                    Box::new(Expr::Interpolate("x".to_string())),
                ))],
                swan_song: None, modifiers: vec![],
            }],
        });
        let mut stmt = Statement::Expression(Expr::TemplateCall {
            name: "double".to_string(), args: vec![Expr::Integer(5)], block: None, span: None,
        });
        let result = expand_template_in_stmt(&stmt, &mut ctx);
        assert!(result.is_ok(), "Expected Ok, got {:?}", result.err());
        // Statement-level should return Some with the expanded statement
        let expanded = result.unwrap();
        assert!(expanded.is_some(), "Expected Some expanded statements");
    }

    #[test]
    fn test_call_site_span_propagated() {
        let mut ctx = MacroContext::new();
        let span = crate::errors::Span::new(10, 20, 5, 3);
        ctx.macros.insert("foo".to_string(), MacroDef {
            name: "foo".to_string(), params: vec![], return_type: None,
            body: vec![Statement::Term { values: vec![Some(Expr::Integer(42))], swan_song: None, modifiers: vec![] }],
        });
        let stmt = Statement::Expression(Expr::MacroCall { name: "foo".to_string(), args: vec![], block: None, span: Some(span) });
        let result = expand_macro_in_stmt(&stmt, &mut ctx);
        assert!(result.is_ok(), "Expected Ok: {:?}", result.err());
    }

    #[test]
    fn test_validate_no_compile_time_intrinsics_ok() {
        let program = Program {
            items: vec![TopLevel::Statement(Box::new(Statement::Expression(Expr::Integer(42))))],
            comments: vec![], reactor_speed: None, attrs: vec![], ffi: None,
            strict_mode: crate::ast::StrictMode::Off, dispatch_mode: crate::ast::DispatchMode::Sequential,
            exit_condition: None, out_pragmas: vec![], default_sig_modifier: None,
        };
        assert!(validate_no_compile_time_intrinsics(&program).is_ok());
    }

    #[test]
    fn test_integration_template_expand_expr_expr() {
        // Template that returns an Expr: $double(5) → 5 + 5 = 10
        let mut ctx = MacroContext::new();
        ctx.templates.insert("double".to_string(), TemplateDef {
            name: "double".to_string(),
            params: vec![("x".to_string(), crate::ast::MacroArgType::Expr)],
            return_type: Some(crate::ast::MacroArgType::Expr),
            body: vec![Statement::Term {
                values: vec![Some(Expr::Interpolate("x".to_string()))],
                swan_song: None,
                modifiers: vec![],
            }],
        });
        let mut stmt = Statement::Expression(Expr::TemplateCall {
            name: "double".to_string(),
            args: vec![Expr::Integer(5)],
            block: None,
            span: None,
        });
        let result = expand_template_in_stmt(&stmt, &mut ctx);
        assert!(result.is_ok(), "Expected Ok: {:?}", result.err());
        let expanded = result.unwrap();
        assert!(expanded.is_some(), "Expected Some expanded statements");
        let stmts = expanded.unwrap();
        assert_eq!(stmts.len(), 1, "Expected 1 statement");
    }

    #[test]
    fn test_integration_macro_expands_to_term_stmt() {
        // Macro that returns a simple Term statement
        let mut ctx = MacroContext::new();
        ctx.macros.insert("gen".to_string(), MacroDef {
            name: "gen".to_string(),
            params: vec![],
            return_type: None,
            body: vec![Statement::Term {
                values: vec![Some(Expr::Integer(42))],
                swan_song: None,
                modifiers: vec![],
            }],
        });
        let stmt = Statement::Expression(Expr::MacroCall {
            name: "gen".to_string(),
            args: vec![],
            block: None,
            span: None,
        });
        let result = expand_macro_in_stmt(&stmt, &mut ctx);
        assert!(result.is_ok(), "Expected Ok: {:?}", result.err());
        let expanded = result.unwrap();
        assert!(expanded.is_some(), "Expected Some expanded statements");
        let stmts = expanded.unwrap();
        assert_eq!(stmts.len(), 1, "Expected 1 expanded statement (Term)");
    }

    #[test]
    fn test_integration_expand_templates_full_pipeline() {
        let mut program = Program {
            items: vec![
                TopLevel::TemplateDef {
                    name: "hey".to_string(),
                    params: vec![],
                    return_type: None,
                    body: vec![Statement::Term {
                        values: vec![Some(Expr::Integer(7))],
                        swan_song: None, modifiers: vec![],
                    }],
                },
                TopLevel::Statement(Box::new(Statement::Expression(Expr::TemplateCall {
                    name: "hey".to_string(), args: vec![], block: None, span: None,
                }))),
            ],
            comments: vec![], reactor_speed: None, attrs: vec![], ffi: None,
            strict_mode: crate::ast::StrictMode::Off,
            dispatch_mode: crate::ast::DispatchMode::Sequential,
            exit_condition: None, out_pragmas: vec![],
            default_sig_modifier: None,
        };
        let mut ctx = MacroContext::new();
        let result = expand_templates(&mut program, &mut ctx);
        assert!(result.is_ok(), "expand_templates failed: {:?}", result.err());
        assert_eq!(program.items.len(), 1, "Expected 1 item after expansion");
        // The expanded call should be present — no panic on TemplateDef removal
        assert!(matches!(&program.items[0], TopLevel::Statement(_)));
    }

    #[test]
    fn test_integration_no_undefined_template_error() {
        let mut ctx = MacroContext::new();
        let stmt = Statement::Expression(Expr::TemplateCall {
            name: "nonexistent".to_string(),
            args: vec![],
            block: None,
            span: None,
        });
        let result = expand_template_in_stmt(&stmt, &mut ctx);
        assert!(result.is_err(), "Expected undefined template error");
        assert!(result.unwrap_err().contains("undefined template"));
    }

    #[test]
    fn test_collect_macro_defs() {
        let mut program = Program {
            items: vec![TopLevel::MacroDef { name: "m".to_string(), params: vec![], return_type: None,
                body: vec![Statement::Term { values: vec![Some(Expr::Integer(42))], swan_song: None, modifiers: vec![] }],
            }],
            comments: vec![], reactor_speed: None, attrs: vec![], ffi: None,
            strict_mode: crate::ast::StrictMode::Off, dispatch_mode: crate::ast::DispatchMode::Sequential,
            exit_condition: None, out_pragmas: vec![], default_sig_modifier: None,
        };
        let mut ctx = MacroContext::new();
        collect_macro_defs(&mut program, &mut ctx);
        assert!(program.items.is_empty());
        assert!(ctx.macros.contains_key("m"));
    }
}
