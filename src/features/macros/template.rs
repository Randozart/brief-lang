use crate::ast::{Expr, Statement};
use crate::features::macros::context::{MacroContext, MacroDef, TemplateDef};

/// Expand a template by substituting @-interpolation markers, then executing
/// the body in a sandboxed interpreter.  This evaluates [guard] conditions,
/// evaluates @{expr} computed interpolations, and runs control flow.
pub fn expand_template(
    ctx: &mut MacroContext,
    interpreter: &mut crate::interpreter::Interpreter,
    def: &TemplateDef,
    args: &[Expr],
    block: Option<crate::ast::Block>,
) -> Result<crate::interpreter::Value, String> {
    // Build bindings from @-interpolation markers to argument AST
    let mut bindings: Vec<(String, Expr)> = Vec::new();
    for (i, (param_name, _)) in def.params.iter().enumerate() {
        if i < args.len() {
            bindings.push((param_name.clone(), args[i].clone()));
        } else {
            return Err(format!("Template '{}': missing argument for parameter '{}'",
                def.name, param_name));
        }
    }
    if let Some(b) = block {
        bindings.push(("__block".to_string(), Expr::QuoteBlock {
            statements: b.statements,
            trailing_expr: b.trailing_expr,
        }));
    }

    // Substitute @-markers in the body, bind args as state variables
    let substituted: Vec<Statement> = def.body.iter().map(|s| substitute_in_stmt(s, &bindings)).collect();

    // Execute each statement through the interpreter (evaluates guards, control flow)
    for stmt in &substituted {
        interpreter.exec_stmt(stmt)
            .map_err(|e| format!("template '{}' body error: {:?}", def.name, e))?;
        if let Some(val) = interpreter.return_value.take() {
            return Ok(val);
        }
    }

    // No term statement — return the substituted body as Block (backwards-compat)
    Ok(crate::interpreter::Value::Block(substituted))
}

/// Expand a macro by executing its body in a sandboxed interpreter.
pub fn expand_macro(
    ctx: &mut MacroContext,
    interpreter: &mut crate::interpreter::Interpreter,
    def: &MacroDef,
    args: &[Expr],
    _block: Option<crate::ast::Block>,
) -> Result<crate::interpreter::Value, String> {
    // Evaluate arguments and bind to parameter names
    for (i, (param_name, _)) in def.params.iter().enumerate() {
        if i < args.len() {
            let value = interpreter.eval_expr(&args[i])
                .map_err(|e| format!("macro '{}': error evaluating arg '{}': {:?}", def.name, param_name, e))?;
            interpreter.state.insert(param_name.clone(), value);
        } else {
            return Err(format!("macro '{}': missing argument for parameter '{}'", def.name, param_name));
        }
    }

    // Execute macro body statements, checking for return_value after each
    for stmt in &def.body {
        interpreter.exec_stmt(stmt)
            .map_err(|e| format!("macro '{}' body error: {:?}", def.name, e))?;
        if let Some(val) = interpreter.return_value.take() {
            return Ok(val);
        }
    }

    Err(format!("macro '{}' body did not return a value via `term`", def.name))
}

/// Convert a Value to a Vec of AST Statements for injection into the program.
pub fn value_to_statements(value: &crate::interpreter::Value) -> Vec<Statement> {
    match value {
        crate::interpreter::Value::Block(stmts) => stmts.clone(),
        crate::interpreter::Value::Stmt(stmt) => vec![*stmt.clone()],
        crate::interpreter::Value::Expr(expr) => {
            vec![Statement::Expression(*expr.clone())]
        }
        other => {
            // Wrap non-AST values as expression statements
            vec![Statement::Expression(expr_from_value(other))]
        }
    }
}

pub(crate) fn expr_from_value(value: &crate::interpreter::Value) -> Expr {
    match value {
        crate::interpreter::Value::Int(n) => Expr::Integer(*n),
        crate::interpreter::Value::Float(f) => Expr::Float(*f),
        crate::interpreter::Value::String(s) => Expr::String(s.clone()),
        crate::interpreter::Value::Bool(b) => Expr::Bool(*b),
        crate::interpreter::Value::Char(c) => Expr::Char(*c),
        _ => Expr::Term,
    }
}

fn substitute_in_stmt(stmt: &Statement, bindings: &[(String, Expr)]) -> Statement {
    match stmt {
        Statement::Expression(expr) => {
            Statement::Expression(substitute_in_expr(expr, bindings))
        }
        Statement::Let { name, ty, expr, address, address_expr, bit_range, constraint, is_override, modifiers } => {
            Statement::Let {
                name: name.clone(),
                ty: ty.clone(),
                expr: expr.as_ref().map(|e| substitute_in_expr(e, bindings)),
                address: *address,
                address_expr: address_expr.as_ref().map(|e| Box::new(substitute_in_expr(e, bindings))),
                bit_range: bit_range.clone(),
                constraint: constraint.clone(),
                is_override: *is_override,
                modifiers: modifiers.clone(),
            }
        }
        Statement::Guarded { condition, statements } => {
            Statement::Guarded {
                condition: substitute_in_expr(condition, bindings),
                statements: statements.iter().map(|s| substitute_in_stmt(s, bindings)).collect(),
            }
        }
        Statement::Term { values, swan_song, modifiers } => {
            Statement::Term {
                values: values.iter().map(|v| v.as_ref().map(|e| substitute_in_expr(e, bindings))).collect(),
                swan_song: swan_song.as_ref().map(|ss| Box::new(substitute_in_stmt(ss, bindings))),
                modifiers: modifiers.clone(),
            }
        }
        Statement::TermBang { values, swan_song, modifiers } => {
            Statement::TermBang {
                values: values.iter().map(|v| v.as_ref().map(|e| substitute_in_expr(e, bindings))).collect(),
                swan_song: swan_song.as_ref().map(|ss| Box::new(substitute_in_stmt(ss, bindings))),
                modifiers: modifiers.clone(),
            }
        }
        Statement::Foreach { item, list, body, modifiers } => {
            Statement::Foreach {
                item: item.clone(),
                list: Box::new(substitute_in_expr(list, bindings)),
                body: body.iter().map(|s| substitute_in_stmt(s, bindings)).collect(),
                modifiers: modifiers.clone(),
            }
        }
        Statement::Assignment { lhs, expr, timeout, modifiers } => {
            Statement::Assignment {
                lhs: substitute_in_expr(lhs, bindings),
                expr: substitute_in_expr(expr, bindings),
                timeout: timeout.clone(),
                modifiers: modifiers.clone(),
            }
        }
        other => other.clone(),
    }
}

fn substitute_in_expr(expr: &Expr, bindings: &[(String, Expr)]) -> Expr {
    match expr {
        Expr::Interpolate(name) => {
            if let Some((_, arg)) = bindings.iter().find(|(n, _)| n == name) {
                arg.clone()
            } else {
                expr.clone()
            }
        }
        Expr::InterpolateExpr(inner) => {
            substitute_in_expr(inner, bindings)
        }
        // Recurse into compound expressions
        Expr::Add(a, b) | Expr::Sub(a, b) | Expr::Mul(a, b) | Expr::Div(a, b)
        | Expr::Mod(a, b) | Expr::Eq(a, b) | Expr::Ne(a, b) | Expr::Lt(a, b)
        | Expr::Le(a, b) | Expr::Gt(a, b) | Expr::Ge(a, b) | Expr::Or(a, b)
        | Expr::And(a, b) | Expr::BitAnd(a, b) | Expr::BitOr(a, b)
        | Expr::BitXor(a, b) | Expr::Shl(a, b) | Expr::Shr(a, b)
        | Expr::Concat(a, b) => {
            let l = substitute_in_expr(a, bindings);
            let r = substitute_in_expr(b, bindings);
            match expr {
                Expr::Add(..) => Expr::Add(Box::new(l), Box::new(r)),
                Expr::Sub(..) => Expr::Sub(Box::new(l), Box::new(r)),
                Expr::Mul(..) => Expr::Mul(Box::new(l), Box::new(r)),
                Expr::Div(..) => Expr::Div(Box::new(l), Box::new(r)),
                Expr::Mod(..) => Expr::Mod(Box::new(l), Box::new(r)),
                Expr::Eq(..) => Expr::Eq(Box::new(l), Box::new(r)),
                Expr::Ne(..) => Expr::Ne(Box::new(l), Box::new(r)),
                Expr::Lt(..) => Expr::Lt(Box::new(l), Box::new(r)),
                Expr::Le(..) => Expr::Le(Box::new(l), Box::new(r)),
                Expr::Gt(..) => Expr::Gt(Box::new(l), Box::new(r)),
                Expr::Ge(..) => Expr::Ge(Box::new(l), Box::new(r)),
                Expr::Or(..) => Expr::Or(Box::new(l), Box::new(r)),
                Expr::And(..) => Expr::And(Box::new(l), Box::new(r)),
                Expr::BitAnd(..) => Expr::BitAnd(Box::new(l), Box::new(r)),
                Expr::BitOr(..) => Expr::BitOr(Box::new(l), Box::new(r)),
                Expr::BitXor(..) => Expr::BitXor(Box::new(l), Box::new(r)),
                Expr::Shl(..) => Expr::Shl(Box::new(l), Box::new(r)),
                Expr::Shr(..) => Expr::Shr(Box::new(l), Box::new(r)),
                Expr::Concat(..) => Expr::Concat(Box::new(l), Box::new(r)),
                _ => unreachable!(),
            }
        }
        Expr::Not(a) | Expr::Neg(a) | Expr::BitNot(a) => {
            Expr::Not(Box::new(substitute_in_expr(a, bindings)))
        }
        _ => expr.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_substitute_interpolation() {
        let body = vec![
            Statement::Expression(Expr::Interpolate("x".to_string())),
        ];
        let bindings = vec![
            ("x".to_string(), Expr::Integer(42)),
        ];
        let result: Vec<Statement> = body.iter().map(|s| substitute_in_stmt(s, &bindings)).collect();
        assert_eq!(result.len(), 1);
        if let Statement::Expression(Expr::Integer(42)) = &result[0] {
            // OK
        } else {
            panic!("Expected Integer(42), got {:?}", result[0]);
        }
    }
}
