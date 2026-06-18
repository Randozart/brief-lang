use crate::ast::{Expr, Program, TopLevel};
use crate::features::macros::context::{MacroContext, MacroDef, TemplateDef};
use crate::features::macros::template;

/// Phase 1a: Expand all template calls in the program.
/// Collects template definitions, removes them from the program,
/// then walks the AST expanding TemplateCall nodes.
pub fn expand_templates(program: &mut Program, ctx: &mut MacroContext) -> Result<(), String> {
    // Collect TemplateDef from the program
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

    // Walk the AST and expand TemplateCall nodes
    expand_template_calls(&mut program.items, ctx)
}

/// Phase 1b: Expand all macro calls in the program.
/// Collects macro definitions, removes them from the program,
/// then walks the AST expanding MacroCall nodes.
pub fn expand_macros(program: &mut Program, ctx: &mut MacroContext) -> Result<(), String> {
    // Collect MacroDef from the program
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

    // TODO: Phase 1b — walk AST and expand MacroCall nodes
    Ok(())
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

fn expand_template_call_in_stmt(
    stmt: &mut crate::ast::Statement,
    ctx: &mut MacroContext,
) -> Result<(), String> {
    match stmt {
        crate::ast::Statement::Expression(expr) => {
            let expanded = expand_template_call_in_expr(expr, ctx)?;
            if let Some(e) = expanded {
                *expr = e;
            }
            Ok(())
        }
        crate::ast::Statement::Guarded { condition, statements } => {
            let expanded = expand_template_call_in_expr(condition, ctx)?;
            if let Some(e) = expanded {
                *condition = e;
            }
            for s in statements.iter_mut() {
                expand_template_call_in_stmt(s, ctx)?;
            }
            Ok(())
        }
        crate::ast::Statement::Let { expr, .. } => {
            if let Some(e) = expr.as_mut() {
                let expanded = expand_template_call_in_expr(e, ctx)?;
                if let Some(ee) = expanded {
                    *e = ee;
                }
            }
            Ok(())
        }
        _ => Ok(()),
    }
}

fn expand_template_call_in_expr(
    expr: &mut Expr,
    _ctx: &mut MacroContext,
) -> Result<Option<Expr>, String> {
    // Check if this is a TemplateCall that needs expansion
    if let Expr::TemplateCall { name, args, block } = expr {
        // Look up the template
        if let Some(def) = _ctx.templates.get(name) {
            // For now, this is a stub — actual expansion requires interpreter evaluation
            // which is implemented in M3
            let _ = (def, args, block);
            return Ok(Some(Expr::Term)); // placeholder: replace with expanded result
        }
    }
    Ok(None)
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
}
