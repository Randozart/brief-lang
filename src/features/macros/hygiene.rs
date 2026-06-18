use crate::ast::{Expr, Statement};
use std::collections::HashMap;

/// Rename local `let` bindings with `__gensym_N` to prevent capture in the
/// caller's scope. `state`/`fn`/`txn`/`struct`/`enum` names are preserved.
/// References to the renamed binding are updated throughout the block.
pub fn apply_hygiene(stmts: &mut [Statement], gensym: &mut impl FnMut() -> String) {
    let mut sym_map: HashMap<String, String> = HashMap::new();
    for stmt in stmts.iter_mut() {
        apply_to_stmt(stmt, &mut sym_map, gensym);
    }
}

/// Walk a statement: rename `let` bindings to gensym, collect mappings,
/// then rename all identifier references using the accumulated map.
fn apply_to_stmt(
    stmt: &mut Statement,
    sym_map: &mut HashMap<String, String>,
    gensym: &mut impl FnMut() -> String,
) {
    match stmt {
        Statement::Let { name, expr, .. } => {
            if !sym_map.contains_key(name) && !name.starts_with("__gensym_") {
                let new_name = gensym();
                let old = std::mem::replace(name, new_name);
                sym_map.insert(old, name.clone());
                // Rename references inside the initializer
                if let Some(e) = expr.as_mut() {
                    rename_expr(e, sym_map);
                }
            }
        }
        Statement::Guarded { condition, statements } => {
            for s in statements.iter_mut() {
                apply_to_stmt(s, sym_map, gensym);
            }
            // Guard condition gets renamed after nested statements
        }
        Statement::Term { swan_song, .. } | Statement::TermBang { swan_song, .. } => {
            if let Some(ss) = swan_song.as_mut() {
                apply_to_stmt(ss, sym_map, gensym);
            }
        }
        Statement::Foreach { body, .. } => {
            for s in body.iter_mut() {
                apply_to_stmt(s, sym_map, gensym);
            }
        }
        Statement::SyncBlock { body } => {
            for s in body.iter_mut() {
                apply_to_stmt(s, sym_map, gensym);
            }
        }
        Statement::Oracle { handler, body, .. } => {
            for s in handler.iter_mut() {
                apply_to_stmt(s, sym_map, gensym);
            }
            for s in body.iter_mut() {
                apply_to_stmt(s, sym_map, gensym);
            }
        }
        _ => {}
    }
    // Rename all identifiers in this statement using the accumulated map
    rename_stmt(stmt, sym_map);
}

/// Rename all identifiers in a statement + its sub-statements using the map.
fn rename_stmt(stmt: &mut Statement, sym_map: &HashMap<String, String>) {
    match stmt {
        Statement::Let { expr, .. } => {
            if let Some(e) = expr.as_mut() {
                rename_expr(e, sym_map);
            }
        }
        Statement::Expression(expr) => rename_expr(expr, sym_map),
        Statement::Assignment { lhs, expr, .. } => {
            rename_expr(lhs, sym_map);
            rename_expr(expr, sym_map);
        }
        Statement::Term { values, swan_song, .. }
        | Statement::TermBang { values, swan_song, .. } => {
            for v in values.iter_mut().flatten() {
                rename_expr(v, sym_map);
            }
            if let Some(ss) = swan_song.as_mut() {
                rename_stmt(ss, sym_map);
            }
        }
        Statement::Guarded { condition, statements } => {
            rename_expr(condition, sym_map);
            for s in statements.iter_mut() {
                rename_stmt(s, sym_map);
            }
        }
        Statement::Foreach { list, body, .. } => {
            rename_expr(list, sym_map);
            for s in body.iter_mut() {
                rename_stmt(s, sym_map);
            }
        }
        Statement::SyncBlock { body } => {
            for s in body.iter_mut() {
                rename_stmt(s, sym_map);
            }
        }
        Statement::Oracle { handler, body, .. } => {
            for s in handler.iter_mut() {
                rename_stmt(s, sym_map);
            }
            for s in body.iter_mut() {
                rename_stmt(s, sym_map);
            }
        }
        _ => {}
    }
}

/// Rename all identifiers in an expression tree using the map.
fn rename_expr(expr: &mut Expr, sym_map: &HashMap<String, String>) {
    match expr {
        Expr::Identifier(name) => {
            if let Some(new_name) = sym_map.get(name) {
                *name = new_name.clone();
            }
        }
        Expr::Add(a, b) | Expr::Sub(a, b) | Expr::Mul(a, b) | Expr::Div(a, b)
        | Expr::Mod(a, b) | Expr::Eq(a, b) | Expr::Ne(a, b) | Expr::Lt(a, b)
        | Expr::Le(a, b) | Expr::Gt(a, b) | Expr::Ge(a, b) | Expr::Or(a, b)
        | Expr::And(a, b) | Expr::BitAnd(a, b) | Expr::BitOr(a, b)
        | Expr::BitXor(a, b) | Expr::Shl(a, b) | Expr::Shr(a, b)
        | Expr::Concat(a, b) => {
            rename_expr(a, sym_map);
            rename_expr(b, sym_map);
        }
        Expr::Not(a) | Expr::Neg(a) | Expr::BitNot(a) => {
            rename_expr(a, sym_map);
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hygiene_renames_let_binding() {
        let mut stmts = vec![Statement::Let {
            name: "temp".to_string(), ty: None, expr: Some(Expr::Integer(42)),
            address: None, address_expr: None, bit_range: None,
            range_constraint: None, is_override: false, modifiers: Vec::new(),
        }];
        let mut counter = 0u64;
        let mut gensym = move || { let n = counter; counter += 1; format!("__gensym_{}", n) };
        apply_hygiene(&mut stmts, &mut gensym);
        let name = match &stmts[0] {
            Statement::Let { name, .. } => name.clone(),
            _ => panic!("Expected Let"),
        };
        assert!(name.starts_with("__gensym_"), "Expected __gensym_, got {}", name);
    }

    #[test]
    fn test_hygiene_renames_references_in_following_stmt() {
        let mut stmts = vec![
            Statement::Let {
                name: "temp".to_string(), ty: None, expr: Some(Expr::Integer(42)),
                address: None, address_expr: None, bit_range: None,
                range_constraint: None, is_override: false, modifiers: Vec::new(),
            },
            Statement::Expression(Expr::Identifier("temp".to_string())),
        ];
        let mut counter = 0u64;
        let mut gensym = move || { let n = counter; counter += 1; format!("__gensym_{}", n) };
        apply_hygiene(&mut stmts, &mut gensym);

        let let_name = match &stmts[0] {
            Statement::Let { name, .. } => name.clone(),
            _ => panic!("Expected Let"),
        };
        assert!(let_name.starts_with("__gensym_"), "let should be renamed: got {}", let_name);

        match &stmts[1] {
            Statement::Expression(Expr::Identifier(ref_name)) => {
                assert_eq!(ref_name, &let_name,
                    "reference should match renamed let, got {} expected {}",
                    ref_name, let_name);
            }
            _ => panic!("Expected Expression(Identifier)"),
        }
    }

    #[test]
    fn test_hygiene_preserves_state_names() {
        let mut stmts = vec![
            Statement::Let {
                name: "temp".to_string(), ty: None, expr: Some(Expr::Integer(1)),
                address: None, address_expr: None, bit_range: None,
                range_constraint: None, is_override: false, modifiers: Vec::new(),
            },
            Statement::Expression(Expr::Identifier("state_val".to_string())),
            Statement::Expression(Expr::Identifier("temp".to_string())),
        ];
        let mut counter = 0u64;
        let mut gensym = move || { let n = counter; counter += 1; format!("__gensym_{}", n) };
        apply_hygiene(&mut stmts, &mut gensym);

        // state_val should NOT be renamed
        match &stmts[1] {
            Statement::Expression(Expr::Identifier(name)) => {
                assert_eq!(name, "state_val", "state names should be preserved");
            }
            _ => panic!("Expected Expression"),
        }
        // temp reference should be renamed to __gensym_0
        let let_name = match &stmts[0] {
            Statement::Let { name, .. } => name.clone(),
            _ => panic!("Expected Let"),
        };
        match &stmts[2] {
            Statement::Expression(Expr::Identifier(name)) => {
                assert_eq!(name, &let_name, "reference should match renamed let");
            }
            _ => panic!("Expected Expression"),
        }
    }
}
