use crate::ast::{Expr, Program, Statement, TopLevel};
use crate::features::macros::context::{MacroContext, MacroDef, TemplateDef};
use crate::features::macros::template;

/// Phase 1a: Expand all template calls in the program.
/// Collects template definitions, removes them from the program,
/// then walks the AST expanding TemplateCall nodes.
pub fn expand_templates(program: &mut Program, ctx: &mut MacroContext) -> Result<(), String> {
    collect_template_defs(program, ctx);
    expand_template_calls(&mut program.items, ctx)
}

/// Phase 1b: Expand all macro calls in the program.
/// Collects macro definitions, then walks the AST expanding MacroCall nodes.
/// Re-runs Phase 1a on macro output since macros can emit template calls.
pub fn expand_macros(program: &mut Program, ctx: &mut MacroContext) -> Result<(), String> {
    collect_macro_defs(program, ctx);
    expand_macro_calls(&mut program.items, ctx)?;
    // Re-run Phase 1a: macros may emit template calls
    expand_template_calls(&mut program.items, ctx)
}

fn collect_template_defs(program: &mut Program, ctx: &mut MacroContext) {
    let mut i = 0;
    while i < program.items.len() {
        if let TopLevel::TemplateDef { name, params, return_type, body } = &program.items[i] {
            let def = TemplateDef {
                name: name.clone(),
                params: params.clone(),
                return_type: return_type.clone(),
                body: body.clone(),
            };
            ctx.templates.insert(name.clone(), def);
            program.items.remove(i);
        } else {
            i += 1;
        }
    }
}

fn collect_macro_defs(program: &mut Program, ctx: &mut MacroContext) {
    let mut i = 0;
    while i < program.items.len() {
        if let TopLevel::MacroDef { name, params, return_type, body } = &program.items[i] {
            let def = MacroDef {
                name: name.clone(),
                params: params.clone(),
                return_type: return_type.clone(),
                body: body.clone(),
            };
            ctx.macros.insert(name.clone(), def);
            program.items.remove(i);
        } else {
            i += 1;
        }
    }
}

fn expand_template_calls(
    items: &mut [TopLevel],
    ctx: &mut MacroContext,
) -> Result<(), String> {
    for item in items.iter_mut() {
        match item {
            TopLevel::Statement(stmt) => {
                expand_template_call_in_stmt(stmt, ctx)?;
            }
            TopLevel::Definition(def) => {
                for s in def.body.iter_mut() {
                    expand_template_call_in_stmt(s, ctx)?;
                }
            }
            TopLevel::Transaction(txn) => {
                for s in txn.body.iter_mut() {
                    expand_template_call_in_stmt(s, ctx)?;
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn expand_macro_calls(
    items: &mut Vec<TopLevel>,
    ctx: &mut MacroContext,
) -> Result<(), String> {
    let mut i = 0;
    while i < items.len() {
        match &items[i] {
            TopLevel::Statement(stmt) => {
                if has_macro_call_in_stmt(stmt) {
                    let stmt_owned = match items.remove(i) {
                        TopLevel::Statement(s) => s,
                        _ => unreachable!(),
                    };
                    let expanded = expand_macro_call_in_stmt(&stmt_owned, ctx)?;
                    // Insert expanded statements at current position
                    for (j, new_stmt) in expanded.into_iter().enumerate() {
                        items.insert(i + j, TopLevel::Statement(Box::new(new_stmt)));
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

fn has_macro_call_in_stmt(stmt: &Statement) -> bool {
    match stmt {
        Statement::Expression(Expr::MacroCall { .. }) => true,
        _ => false,
    }
}

fn expand_macro_call_in_stmt(
    stmt: &Statement,
    ctx: &mut MacroContext,
) -> Result<Vec<Statement>, String> {
    if let Statement::Expression(Expr::MacroCall { name, args, block, span }) = stmt {
        let def = ctx.macros.get(name)
            .ok_or_else(|| format!("undefined macro '{}'", name))?
            .clone();
        ctx.call_site_span = span.clone();
        let mut interpreter = crate::interpreter::Interpreter::new();
        let value = template::expand_macro(ctx, &mut interpreter, &def, args, block.clone());
        ctx.call_site_span = None;
        let value = value?;
        Ok(template::value_to_statements(&value))
    } else {
        Ok(vec![stmt.clone()])
    }
}

fn expand_template_call_in_stmt(
    stmt: &mut Statement,
    ctx: &mut MacroContext,
) -> Result<(), String> {
    match stmt {
        Statement::Expression(expr) => {
            if let Some(expanded) = expand_template_call_in_expr(expr, ctx)? {
                *expr = expanded;
            }
            Ok(())
        }
        Statement::Guarded { condition, statements } => {
            expand_template_call_in_expr(condition, ctx)?;
            for s in statements.iter_mut() {
                expand_template_call_in_stmt(s, ctx)?;
            }
            Ok(())
        }
        Statement::Let { expr, .. } => {
            if let Some(e) = expr.as_mut() {
                expand_template_call_in_expr(e, ctx)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn expand_template_call_in_expr(
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
        let value = template::expand_template(ctx, &mut interpreter, &def, &args, block)?;
        match &value {
            crate::interpreter::Value::Expr(e) => return Ok(Some(*e.clone())),
            crate::interpreter::Value::Stmt(_) => {
                return Ok(Some(Expr::Term));
            }
            crate::interpreter::Value::Block(stmts) => {
                return Ok(Some(Expr::QuoteBlock {
                    statements: stmts.clone(),
                    trailing_expr: None,
                }));
            }
            _ => return Ok(Some(Expr::Term)),
        }
    }
    Ok(None)
}

/// Validate that no compile-time-only intrinsics remain after macro expansion.
pub fn validate_no_compile_time_intrinsics(program: &Program) -> Result<(), String> {
    for item in &program.items {
        match item {
            TopLevel::Statement(stmt) => {
                check_stmt_for_intrinsics(stmt)?;
            }
            _ => {}
        }
    }
    Ok(())
}

fn check_stmt_for_intrinsics(stmt: &Statement) -> Result<(), String> {
    match stmt {
        Statement::Expression(expr) => check_expr_for_intrinsics(expr),
        Statement::Let { expr, .. } => {
            if let Some(e) = expr {
                check_expr_for_intrinsics(e)
            } else {
                Ok(())
            }
        }
        Statement::Guarded { condition, statements } => {
            check_expr_for_intrinsics(condition)?;
            for s in statements {
                check_stmt_for_intrinsics(s)?;
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn check_expr_for_intrinsics(expr: &Expr) -> Result<(), String> {
    if let Expr::IntrinsicCall { intrinsic, .. } = expr {
        if intrinsic.is_compile_time_only() {
            return Err(format!(
                "compile-time-only intrinsic {}/# survived expansion — this is a compiler bug",
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
                TopLevel::TemplateDef {
                    name: "foo".to_string(),
                    params: vec![],
                    return_type: None,
                    body: vec![],
                },
                TopLevel::TemplateDef {
                    name: "bar".to_string(),
                    params: vec![],
                    return_type: None,
                    body: vec![],
                },
            ],
            comments: vec![],
            reactor_speed: None,
            attrs: vec![],
            ffi: None,
            strict_mode: crate::ast::StrictMode::Off,
            dispatch_mode: crate::ast::DispatchMode::Sequential,
            exit_condition: None,
            out_pragmas: vec![],
            default_sig_modifier: None,
        };
        let mut ctx = MacroContext::new();
        let result = expand_templates(&mut program, &mut ctx);
        assert!(result.is_ok());
        assert!(program.items.is_empty(), "TemplateDefs should be removed from program");
        assert!(ctx.templates.contains_key("foo"));
        assert!(ctx.templates.contains_key("bar"));
    }

    #[test]
    fn test_call_site_span_propagated() {
        let mut ctx = MacroContext::new();
        let span = crate::errors::Span::new(10, 20, 5, 3);
        let stmt = Statement::Expression(Expr::MacroCall {
            name: "foo".to_string(),
            args: vec![],
            block: None,
            span: Some(span.clone()),
        });
        ctx.macros.insert("foo".to_string(), MacroDef {
            name: "foo".to_string(),
            params: vec![],
            return_type: None,
            body: vec![Statement::Term {
                values: vec![Some(Expr::Integer(42))],
                swan_song: None,
                modifiers: vec![],
            }],
        });
        let result = expand_macro_call_in_stmt(&stmt, &mut ctx);
        assert!(result.is_ok(), "Expected expansion to succeed: {:?}", result.err());
        let _value = result.unwrap();
    }

    #[test]
    fn test_validate_no_compile_time_intrinsics_ok() {
        let program = Program {
            items: vec![
                TopLevel::Statement(Box::new(Statement::Expression(Expr::Integer(42)))),
            ],
            comments: vec![],
            reactor_speed: None,
            attrs: vec![],
            ffi: None,
            strict_mode: crate::ast::StrictMode::Off,
            dispatch_mode: crate::ast::DispatchMode::Sequential,
            exit_condition: None,
            out_pragmas: vec![],
            default_sig_modifier: None,
        };
        assert!(validate_no_compile_time_intrinsics(&program).is_ok());
    }

    #[test]
    fn test_collect_macro_defs() {
        let mut program = Program {
            items: vec![
                TopLevel::MacroDef {
                    name: "m".to_string(),
                    params: vec![],
                    return_type: None,
                    body: vec![
                        Statement::Term {
                            values: vec![Some(Expr::Integer(42))],
                            swan_song: None,
                            modifiers: vec![],
                        },
                    ],
                },
            ],
            comments: vec![],
            reactor_speed: None,
            attrs: vec![],
            ffi: None,
            strict_mode: crate::ast::StrictMode::Off,
            dispatch_mode: crate::ast::DispatchMode::Sequential,
            exit_condition: None,
            out_pragmas: vec![],
            default_sig_modifier: None,
        };
        let mut ctx = MacroContext::new();
        collect_macro_defs(&mut program, &mut ctx);
        assert!(program.items.is_empty());
        assert!(ctx.macros.contains_key("m"));
    }
}
