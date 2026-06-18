use crate::ast::{Expr, Statement};

pub fn apply_hygiene(stmts: &mut [Statement], gensym: &mut impl FnMut() -> String) {
    let mut sym_map: std::collections::HashMap<String, String> = std::collections::HashMap::new();
    for stmt in stmts.iter_mut() {
        apply_hygiene_to_stmt(stmt, &mut sym_map, gensym);
    }
}

fn apply_hygiene_to_stmt(
    stmt: &mut Statement,
    sym_map: &mut std::collections::HashMap<String, String>,
    gensym: &mut impl FnMut() -> String,
) {
    match stmt {
        Statement::Let { name, expr, .. } => {
            if !sym_map.contains_key(name) {
                let new_name = gensym();
                // Don't insert into sym_map here — let bindings are independent
                let old = std::mem::replace(name, new_name);
                if let Some(e) = expr {
                    rename_idents_in_expr(e, &old, name);
                }
            }
        }
        _ => {}
    }
    // Walk sub-statements
    walk_substmts(stmt, sym_map, gensym);
}

fn walk_substmts(
    stmt: &mut Statement,
    sym_map: &mut std::collections::HashMap<String, String>,
    gensym: &mut impl FnMut() -> String,
) {
    match stmt {
        Statement::Guarded { statements, .. } => {
            for s in statements.iter_mut() {
                apply_hygiene_to_stmt(s, sym_map, gensym);
            }
        }
        Statement::Foreach { body, .. } => {
            for s in body.iter_mut() {
                apply_hygiene_to_stmt(s, sym_map, gensym);
            }
        }
        Statement::SyncBlock { body } => {
            for s in body.iter_mut() {
                apply_hygiene_to_stmt(s, sym_map, gensym);
            }
        }
        Statement::Oracle { handler, body, .. } => {
            for s in handler.iter_mut() {
                apply_hygiene_to_stmt(s, sym_map, gensym);
            }
            for s in body.iter_mut() {
                apply_hygiene_to_stmt(s, sym_map, gensym);
            }
        }
        _ => {}
    }
}

fn rename_idents_in_expr(expr: &mut Expr, old: &str, new: &str) {
    match expr {
        Expr::Identifier(name) if name == old => {
            *name = new.to_string();
        }
        Expr::Add(a, b)
        | Expr::Sub(a, b)
        | Expr::Mul(a, b)
        | Expr::Div(a, b)
        | Expr::Mod(a, b)
        | Expr::Eq(a, b)
        | Expr::Ne(a, b)
        | Expr::Lt(a, b)
        | Expr::Le(a, b)
        | Expr::Gt(a, b)
        | Expr::Ge(a, b)
        | Expr::Or(a, b)
        | Expr::And(a, b)
        | Expr::BitAnd(a, b)
        | Expr::BitOr(a, b)
        | Expr::BitXor(a, b)
        | Expr::Shl(a, b)
        | Expr::Shr(a, b)
        | Expr::Concat(a, b) => {
            rename_idents_in_expr(a, old, new);
            rename_idents_in_expr(b, old, new);
        }
        Expr::Not(a)
        | Expr::Neg(a)
        | Expr::BitNot(a) => rename_idents_in_expr(a, old, new),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hygiene_renames_let_binding() {
        let mut stmts = vec![
            Statement::Let {
                name: "temp".to_string(),
                ty: None,
                expr: Some(Expr::Integer(42)),
                address: None,
                address_expr: None,
                bit_range: None,
                range_constraint: None,
                is_override: false,
                modifiers: Vec::new(),
            },
        ];
        let mut counter = 0u64;
        let mut gensym = move || { let n = counter; counter += 1; format!("__gensym_{}", n) };
        apply_hygiene(&mut stmts, &mut gensym);
        if let Statement::Let { name, .. } = &stmts[0] {
            assert!(name.starts_with("__gensym_"), "Expected gensym prefix, got {}", name);
        } else {
            panic!("Expected Let");
        }
    }
}
