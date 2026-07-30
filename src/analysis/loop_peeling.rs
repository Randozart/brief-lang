// ── Loop Peeling Analysis ──────────────────────────────────────
//
// 2026-07-29: Detects infrequent side-effecting guards in loop bodies
// and reorders the body so pure compute statements come first,
// followed by any hoistable guards (periodic prints, termination checks).
//
// This enables LLVM's if-conversion to handle the compute block
// independently, without opaque function calls blocking the transform.
//
// See docs/plans/2026-07-29-loop-peeling-automatic.md
//
// A guard is hoistable if its body contains a function call (PrintLn,
// PrintInt, etc.) that prevents LLVM's if-conversion.

use crate::ast::{BinaryOpKind, Expr, Statement};
use std::collections::{HashMap, HashSet};

/// Reorder a loop body so that pure compute statements come first
/// and hoistable guards (containing function calls) come last.
///
/// Returns (reordered_body) where the first N statements are pure
/// compute (no function calls) and the remaining are guards.
pub fn reorder_body(body: &[Statement]) -> Vec<Statement> {
    let mut pure: Vec<Statement> = Vec::new();
    let mut guards: Vec<Statement> = Vec::new();

    for stmt in body {
        if is_hoistable_guard(stmt) {
            guards.push(stmt.clone());
        } else {
            pure.push(stmt.clone());
        }
    }

    // Pure compute first, then guards
    pure.extend(guards);
    pure
}

/// Check if a statement is a guard that should be hoisted to the end.
/// A guard is hoistable if it contains a function call.
pub(crate) fn is_hoistable_guard(stmt: &Statement) -> bool {
    contains_function_call(stmt)
}

/// Extract the batch size from a `when count % N == 0` guard condition.
pub fn extract_batch_size(cond: &Expr, count_var: &str) -> Option<usize> {
    let (l, r) = match cond {
        Expr::BinaryOp(BinaryOpKind::Eq, l, r) => (l, r),
        _ => return None,
    };
    let (mod_expr, zero_expr) = pick_mod_vs_zero(l, r, count_var)?;
    if !is_zero(zero_expr) {
        return None;
    }
    let right: &Expr = match mod_expr {
        Expr::BinaryOp(BinaryOpKind::Mod, _, r) => r,
        _ => return None,
    };
    match right {
        Expr::Decimal(n) if *n > 0 && *n <= 1_000_000_000 => Some(*n as usize),
        _ => None,
    }
}

fn pick_mod_vs_zero<'a>(l: &'a Expr, r: &'a Expr, count_var: &str) -> Option<(&'a Expr, &'a Expr)> {
    if is_mod_with(l, count_var) {
        Some((l, r))
    } else if is_mod_with(r, count_var) {
        Some((r, l))
    } else {
        None
    }
}

fn is_zero(expr: &Expr) -> bool {
    matches!(expr, Expr::Decimal(0) | Expr::Float(0.0))
}

/// Split the body into pure compute statements and hoistable guards.
/// Returns only the hoistable guards that are SAFE to hoist — i.e., the
/// guard body only references state fields and identifiers defined within
/// the guard itself. Guards referencing non-state let-bindings from the
/// main body (like dx01, energy) are NOT hoistable because those let-
/// bindings have no %State representation and their computed registers
/// are invalid in the outer guard block.
/// `let_to_field` maps let-binding names to state field names (e.g.,
/// energy → last_energy), used to accept identifiers that can be remapped.
pub fn split_hoistable(
    body: &[Statement],
    field_index_map: &HashSet<String>,
    let_to_field: &HashMap<String, String>,
) -> Vec<Statement> {
    body.iter().filter(|s| is_safe_to_hoist(s, field_index_map, let_to_field)).cloned().collect()
}

/// A guard is safe to hoist iff every identifier referenced in its body
/// is either a state field (in field_index_map), an identifier remappable
/// through `let_to_field` (e.g., energy → last_energy), or defined within
/// the guard body itself.
fn is_safe_to_hoist(
    stmt: &Statement, field_index_map: &HashSet<String>,
    let_to_field: &HashMap<String, String>,
) -> bool {
    match stmt {
        Statement::Guarded(_, body) => {
            let mut defined_here: HashSet<String> = HashSet::new();
            for s in body {
                if let Statement::Let { name, .. } = s {
                    defined_here.insert(name.clone());
                }
            }
            body.iter().all(|s| guard_stmt_uses_only_safe_refs(s, field_index_map, let_to_field, &defined_here))
        }
        _ => false,
    }
}

fn guard_stmt_uses_only_safe_refs(
    stmt: &Statement, field_index_map: &HashSet<String>,
    let_to_field: &HashMap<String, String>,
    defined_here: &HashSet<String>,
) -> bool {
    match stmt {
        Statement::Let { expr: Some(e), .. } => {
            expr_uses_only_safe_refs(e, field_index_map, let_to_field, defined_here)
        }
        Statement::Expression(e) => {
            expr_uses_only_safe_refs(e, field_index_map, let_to_field, defined_here)
        }
        _ => false,
    }
}

fn expr_uses_only_safe_refs(
    expr: &Expr, field_index_map: &HashSet<String>,
    let_to_field: &HashMap<String, String>,
    defined_here: &HashSet<String>,
) -> bool {
    match expr {
        Expr::Identifier(name) => {
            // State fields are safe (loaded from %State via phi register).
            // Identifiers remappable through let_to_field are safe
            // (e.g., energy → last_energy, which IS a state field).
            // Identifiers defined within the guard body are safe.
            field_index_map.contains(name.as_str())
                || let_to_field.contains_key(name.as_str())
                || defined_here.contains(name.as_str())
        }
        Expr::Call(_, args, _) => {
            args.iter().all(|a| expr_uses_only_safe_refs(a, field_index_map, let_to_field, defined_here))
        }
        Expr::BinaryOp(_, l, r) => {
            expr_uses_only_safe_refs(l, field_index_map, let_to_field, defined_here)
                && expr_uses_only_safe_refs(r, field_index_map, let_to_field, defined_here)
        }
        Expr::UnaryOp(_, e) => expr_uses_only_safe_refs(e, field_index_map, let_to_field, defined_here),
        Expr::Cast(e, _) => expr_uses_only_safe_refs(e, field_index_map, let_to_field, defined_here),
        Expr::Block(stmts) => stmts.iter().all(|s| {
            match s {
                Statement::Expression(e) => expr_uses_only_safe_refs(e, field_index_map, let_to_field, defined_here),
                Statement::Let { expr: Some(e), .. } => expr_uses_only_safe_refs(e, field_index_map, let_to_field, defined_here),
                _ => false,
            }
        }),
        Expr::Field(obj, _) => expr_uses_only_safe_refs(obj, field_index_map, let_to_field, defined_here),
        Expr::Index(arr, idx) => {
            expr_uses_only_safe_refs(arr, field_index_map, let_to_field, defined_here)
                && expr_uses_only_safe_refs(idx, field_index_map, let_to_field, defined_here)
        }
        Expr::List(items) => items.iter().all(|i| expr_uses_only_safe_refs(i, field_index_map, let_to_field, defined_here)),
        Expr::Tuple(items) => items.iter().all(|i| expr_uses_only_safe_refs(i, field_index_map, let_to_field, defined_here)),
        Expr::Quoted(_) | Expr::Decimal(_) | Expr::Float(_) | Expr::Bool(_) => true,
        Expr::PluginIntercept { args, .. } => {
            args.iter().all(|a| expr_uses_only_safe_refs(a, field_index_map, let_to_field, defined_here))
        }
        _ => false,
    }
}

/// Extract the batch size from a list of guard statements.
pub fn extract_batch_size_from_guards(guards: &[Statement], count_var: &str) -> Option<usize> {
    for guard in guards {
        let (cond, _body) = match guard {
            Statement::Guarded(c, b) => (c, b),
            _ => continue,
        };
        match extract_batch_size(cond, count_var) {
            Some(bs) => return Some(bs),
            None => continue,
        }
    }
    None
}

fn is_identifier(expr: &Expr, name: &str) -> bool {
    matches!(expr, Expr::Identifier(n) if n == name)
}

fn is_mod_with(expr: &Expr, var: &str) -> bool {
    matches!(expr, Expr::BinaryOp(BinaryOpKind::Mod, l, _) if is_identifier(l, var))
}

/// Check if a statement contains a function call (directly or in sub-expressions).
fn contains_function_call(stmt: &Statement) -> bool {
    match stmt {
        Statement::Guarded(_, body) => {
            body.iter().any(|s| contains_function_call(s))
        }
        Statement::Term(Some(e)) | Statement::Expression(e) => has_call_expr(e),
        Statement::TermBang(Some(e)) => has_call_expr(e),
        // 2026-07-30: Let bindings with PluginIntercept (PrintLn!, Print!) or
        // Block expressions (post-resolution prints) are hoistable.
        Statement::Let { expr: Some(e), .. } => has_call_expr(e),
        Statement::Assign(_, rhs) => has_call_expr(rhs),
        _ => false,
    }
}

fn has_call_expr(expr: &Expr) -> bool {
    match expr {
        Expr::Call(_, _, _) => true,
        Expr::BinaryOp(_, l, r) => has_call_expr(l) || has_call_expr(r),
        Expr::UnaryOp(_, e) => has_call_expr(e),
        Expr::Cast(e, _) => has_call_expr(e),
        Expr::Field(obj, _) => has_call_expr(obj),
        Expr::Index(arr, idx) => has_call_expr(arr) || has_call_expr(idx),
        Expr::List(items) => items.iter().any(|i| has_call_expr(i)),
        Expr::Tuple(items) => items.iter().any(|i| has_call_expr(i)),
        Expr::Block(stmts) => stmts.iter().any(|s| contains_function_call(s)),
        Expr::PluginIntercept { .. } => true,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
use crate::ast::{BinaryOpKind, Expr, Statement};

    #[test]
    fn test_guard_with_println_is_hoistable() {
        let stmt = Statement::Guarded(
            Expr::Decimal(1),
            vec![
                Statement::Expression(
                    Expr::Call("PrintLn".to_string(), vec![], None),
                ),
            ],
        );
        assert!(is_hoistable_guard(&stmt));
    }

    #[test]
    fn test_pure_assign_not_hoistable() {
        let stmt = Statement::Assign(
            Expr::Identifier("x".to_string()),
            Expr::Decimal(42),
        );
        assert!(!is_hoistable_guard(&stmt));
    }

    #[test]
    fn test_reorder_moves_guards_to_end() {
        let body = vec![
            Statement::Assign(
                Expr::Identifier("x".to_string()),
                Expr::Decimal(1),
            ),
            Statement::Guarded(
                Expr::Decimal(1),
                vec![
                    Statement::Expression(
                        Expr::Call("PrintLn".to_string(), vec![], None),
                    ),
                ],
            ),
            Statement::Assign(
                Expr::Identifier("y".to_string()),
                Expr::Decimal(2),
            ),
        ];
        let reordered = reorder_body(&body);
        assert_eq!(reordered.len(), 3);
        // First two should be the pure assigns
        match &reordered[0] {
            Statement::Assign(Expr::Identifier(n), _) => assert_eq!(n, "x"),
            _ => panic!("expected assign to x"),
        }
        match &reordered[1] {
            Statement::Assign(Expr::Identifier(n), _) => assert_eq!(n, "y"),
            _ => panic!("expected assign to y"),
        }
        // Last should be the guard
        match &reordered[2] {
            Statement::Guarded(_, _) => {} // expected
            _ => panic!("expected guard"),
        }
    }

    #[test]
    fn test_no_guards_returns_same_order() {
        let body = vec![
            Statement::Assign(
                Expr::Identifier("x".to_string()),
                Expr::Decimal(1),
            ),
            Statement::Term(None),
        ];
        let reordered = reorder_body(&body);
        assert_eq!(reordered.len(), 2);
        match &reordered[0] {
            Statement::Assign(Expr::Identifier(n), _) => assert_eq!(n, "x"),
            _ => panic!("expected assign to x"),
        }
    }

    #[test]
    fn test_term_with_call_is_hoistable() {
        let stmt = Statement::Term(Some(
            Expr::Call("PrintInt#".to_string(), vec![Expr::Decimal(1)], None),
        ));
        assert!(is_hoistable_guard(&stmt));
    }
}
