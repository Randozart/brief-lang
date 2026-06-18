use crate::ast::{Expr, Statement};
use crate::features::macros::context::{MacroContext, MacroDef, TemplateDef};

/// Expand a macro by executing its body in a sandboxed interpreter.
pub fn expand_macro(
    _ctx: &mut MacroContext,
    _interpreter: &mut crate::interpreter::Interpreter,
    def: &MacroDef,
    _args: &[Expr],
    _block: Option<crate::ast::Block>,
) -> Result<crate::interpreter::Value, String> {
    // TODO: implement full macro body execution (M3.2)
    Err(format!("macro '{}' expansion not yet implemented", def.name))
}

fn substitute_in_stmt(stmt: &Statement, bindings: &[(String, Expr)]) -> Statement {
    match stmt {
        Statement::Expression(expr) => {
            Statement::Expression(substitute_in_expr(expr, bindings))
        }
        Statement::Let { name, ty, expr, address, address_expr, bit_range, range_constraint, is_override, modifiers } => {
            Statement::Let {
                name: name.clone(),
                ty: ty.clone(),
                expr: expr.as_ref().map(|e| substitute_in_expr(e, bindings)),
                address: *address,
                address_expr: address_expr.as_ref().map(|e| Box::new(substitute_in_expr(e, bindings))),
                bit_range: bit_range.clone(),
                range_constraint: range_constraint.clone(),
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
        Statement::Foreach { item, list, body } => {
            Statement::Foreach {
                item: item.clone(),
                list: Box::new(substitute_in_expr(list, bindings)),
                body: body.iter().map(|s| substitute_in_stmt(s, bindings)).collect(),
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
