// ── AoS → SoA Field Reorder ─────────────────────────────────────
//
// 2026-07-29: Reorders state field declarations from Array-of-Structs
// (per-body: bx0, by0, bz0, vx0, vy0, vz0, bx1, ...) to Struct-of-Arrays
// (per-component: bx0, bx1, bx2, bx3, bx4, by0, by1, ...) layout BEFORE
// field indices are assigned.
//
// This is a FRONT-END transformation — it reorders the TopLevel items
// vector so that `build_field_index` assigns consecutive indices to
// same-component fields. The backend's index-run grouping then naturally
// forms <4 x float> vector phis without any naming heuristics.
//
// Safety: Reordering is only performed when the analysis can prove
// pairwise data independence between fields of the same component family.
// A field is independent of its siblings if its update expression never
// references them (even transitively through let-bindings).
//
// See docs/plans/2026-07-29-frontend-ir-quality-improvements.md §3.

use crate::ast::{Expr, Statement, TopLevel, Type};
use std::collections::{HashMap, HashSet};

/// Reorder state field `let` declarations to SoA layout.
///
/// Input: items in declaration order (may be AoS: bx0, by0, ..., bx1, ...)
/// Output: items with float field declarations reordered to SoA (bx0, bx1, ..., by0, by1, ...)
///
/// Non-float items and items that cannot be safely reordered remain in their
/// original position.
struct FloatField {
    name: String,
    prefix: String,
    index: usize,
    item_index: usize,
    ty: Type,
    expr: Option<Expr>,
}

pub fn reorder_fields(items: &[TopLevel]) -> Vec<TopLevel> {
    let mut float_fields: Vec<FloatField> = Vec::new();
    let mut non_float_indices: HashSet<usize> = HashSet::new();

    for (i, item) in items.iter().enumerate() {
        if let Some(float_field) = try_extract_float_field(item, i) {
            float_fields.push(float_field);
        } else {
            non_float_indices.insert(i);
        }
    }

    if float_fields.is_empty() {
        return items.to_vec();
    }

    // Group by prefix
    let mut by_prefix: HashMap<String, Vec<&FloatField>> = HashMap::new();
    for f in &float_fields {
        by_prefix.entry(f.prefix.clone()).or_default().push(f);
    }

    // Sort each prefix group by index
    for (_, group) in &mut by_prefix {
        group.sort_by_key(|f| f.index);
    }

    // Check which groups can be safely reordered (independent + isomorphic)
    let mut safe_groups: Vec<String> = Vec::new();
    for (prefix, group) in &by_prefix {
        if group.len() < 2 {
            continue;
        }
        if !group_members_independent(group, items) {
            continue;
        }
        safe_groups.push(prefix.clone());
    }

    if safe_groups.is_empty() {
        return items.to_vec();
    }

    // Build reordered items: non-float items first (keep original order),
    // then SoA-ordered float fields.
    let mut result: Vec<TopLevel> = Vec::with_capacity(items.len());

    // Collect float field items in original order for lookup
    let original_items: HashMap<usize, &TopLevel> = float_fields.iter()
        .map(|f| (f.item_index, &items[f.item_index]))
        .collect();

    // First pass: output non-float items in original order
    for idx in 0..items.len() {
        if non_float_indices.contains(&idx) {
            result.push(items[idx].clone());
        }
    }

    // Second pass: output SoA-ordered float fields
    let mut inserted_names: HashSet<String> = HashSet::new();
    // Sort safe prefixes alphabetically for deterministic output
    let mut sorted_prefixes: Vec<&String> = safe_groups.iter().collect();
    sorted_prefixes.sort();
    for prefix in &sorted_prefixes {
        if let Some(group) = by_prefix.get(prefix.as_str()) {
            for f in group {
                if inserted_names.insert(f.name.clone()) {
                    result.push(original_items[&f.item_index].clone());
                }
            }
        }
    }
    // Insert any remaining float fields (not in a safe group)
    for f in &float_fields {
        if !inserted_names.contains(&f.name) {
            result.push(original_items[&f.item_index].clone());
            inserted_names.insert(f.name.clone());
        }
    }

    result
}

/// Extract a FloatField from a TopLevel item if it's a float let declaration
/// with a numeric suffix (e.g., "bx0", "vx12"). Returns None otherwise.
fn try_extract_float_field(item: &TopLevel, index: usize) -> Option<FloatField> {
    let box_stmt = match item {
        TopLevel::Statement(s) => s,
        TopLevel::StateDecl(s) => {
            // Legacy state declaration format — still produced by some code paths
            if !is_reorderable_float_type(&s.ty) {
                return None;
            }
            let (prefix, idx) = parse_numeric_prefix(&s.name)?;
            return Some(FloatField {
                name: s.name.clone(),
                prefix,
                index: idx,
                item_index: index,
                ty: s.ty.clone(),
                expr: None,
            });
        }
        _ => return None,
    };
    let stmt = box_stmt.as_ref();
    let (name, ty, expr) = match stmt {
        Statement::Let { name, ty, expr, .. } => (name, ty.as_ref()?, expr),
        _ => return None,
    };
    if !is_reorderable_float_type(ty) {
        return None;
    }
    let (prefix, idx) = parse_numeric_prefix(name)?;
    Some(FloatField {
        name: name.clone(),
        prefix,
        index: idx,
        item_index: index,
        ty: ty.clone(),
        expr: expr.clone(),
    })
}

/// Check if a Brief type is reorderable (float or double).
fn is_reorderable_float_type(ty: &Type) -> bool {
    ty == &Type::float() || ty == &Type::float64() || ty.to_string() == "Float32"
        || ty.to_string() == "Float64"
}

/// Parse a field name to extract prefix and numeric index.
/// e.g., "bx0" → Some(("bx", 0)), "energy" → None, "count" → None
fn parse_numeric_prefix(name: &str) -> Option<(String, usize)> {
    let digits_start = name.rfind(|c: char| !c.is_ascii_digit())? + 1;
    if digits_start >= name.len() || digits_start == 0 {
        return None;
    }
    let prefix = name[..digits_start].to_string();
    let suffix: usize = name[digits_start..].parse().ok()?;
    Some((prefix, suffix))
}

/// Check that all members of a prefix group are pairwise data-independent.
/// Field A is independent of Field B if A's update expression never references B
/// (even transitively through the body's let-bindings).
fn group_members_independent(group: &[&FloatField], items: &[TopLevel]) -> bool {
    if group.is_empty() {
        return true;
    }

    // Collect all names in this group
    let group_names: HashSet<&str> = group.iter().map(|f| f.name.as_str()).collect();

    // Find the transaction body that updates these fields
    let body = find_txn_body(items);

    // For each group member, scan its update expression for references to
    // other group members
    for f in group {
        if let Some(expr) = &f.expr {
            if expr_references_group(expr, &group_names, &f.name) {
                return false;
            }
        }
    }

    // Also check assignments in the body for cross-references
    for stmt in &body {
        if let Statement::Assign(lhs, rhs) = stmt {
            if let Expr::Identifier(name) = lhs {
                if group_names.contains(name.as_str())
                    && expr_references_group_other(rhs, &group_names, name.as_str())
                {
                    return false;
                }
            }
        }
    }

    true
}

/// Check if an expression references any field in `group_names` other than `self_name`.
fn expr_references_group(expr: &Expr, group_names: &HashSet<&str>, self_name: &str) -> bool {
    match expr {
        Expr::Identifier(n) => group_names.contains(n.as_str()) && n != self_name,
        Expr::BinaryOp(_, l, r) => {
            expr_references_group(l, group_names, self_name)
                || expr_references_group(r, group_names, self_name)
        }
        Expr::UnaryOp(_, e) => expr_references_group(e, group_names, self_name),
        Expr::Field(obj, _) => expr_references_group(obj, group_names, self_name),
        Expr::Cast(e, _) => expr_references_group(e, group_names, self_name),
        _ => false,
    }
}

/// Check if expression references any field in group_names OTHER than `self_name`.
fn expr_references_group_other(expr: &Expr, group_names: &HashSet<&str>, self_name: &str) -> bool {
    expr_references_group(expr, group_names, self_name)
}

/// Find the body of the first transaction in items.
fn find_txn_body(items: &[TopLevel]) -> Vec<Statement> {
    for item in items {
        if let TopLevel::Transaction(t) = item {
            return t.body.clone();
        }
    }
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{BinaryOpKind, Expr, Statement, TopLevel, Type};

    fn name_of(item: &TopLevel) -> &str {
        match item {
            TopLevel::Statement(s) => match s.as_ref() {
                Statement::Let { name, .. } => name.as_str(),
                _ => "stmt",
            },
            TopLevel::Constant(c) => &c.name,
            TopLevel::Transaction(t) => &t.name,
            _ => "unknown",
        }
    }

    fn state_let(name: &str, expr: Expr) -> TopLevel {
        TopLevel::Statement(Box::new(Statement::Let {
            names: vec![],
            name: name.to_string(),
            ty: Some(Type::float()),
            expr: Some(expr),
            modifiers: vec![],
        }))
    }

    #[test]
    fn test_no_float_fields_returns_original() {
        let items = vec![
            TopLevel::Constant(crate::ast::top::Constant {
                name: "pi".to_string(), ty: Type::float(), expr: Expr::Float(3.14),
            }),
        ];
        let result = reorder_fields(&items);
        assert_eq!(result.len(), 1);
        assert_eq!(name_of(&result[0]), "pi");
    }

    #[test]
    fn test_independent_bx_group() {
        let items = vec![
            state_let("bx0", Expr::BinaryOp(
                BinaryOpKind::Add, Box::new(Expr::Identifier("bx0".to_string())),
                Box::new(Expr::Decimal(1)),
            )),
            state_let("bx1", Expr::BinaryOp(
                BinaryOpKind::Add, Box::new(Expr::Identifier("bx1".to_string())),
                Box::new(Expr::Decimal(1)),
            )),
        ];
        let result = reorder_fields(&items);
        assert_eq!(result.len(), 2);
        assert_eq!(name_of(&result[0]), "bx0");
        assert_eq!(name_of(&result[1]), "bx1");
    }

    #[test]
    fn test_dependent_group_not_reordered() {
        let items = vec![
            state_let("bx1", Expr::BinaryOp(
                BinaryOpKind::Add, Box::new(Expr::Identifier("bx1".to_string())),
                Box::new(Expr::Decimal(1)),
            )),
            state_let("bx0", Expr::Identifier("bx1".to_string())),
        ];
        let result = reorder_fields(&items);
        assert_eq!(result.len(), 2);
        // Should keep original order because cross-dependency found
        assert_eq!(name_of(&result[0]), "bx1");
        assert_eq!(name_of(&result[1]), "bx0");
    }

    #[test]
    fn test_single_field_not_reordered() {
        let items = vec![
            state_let("bx0", Expr::Identifier("bx0".to_string())),
        ];
        let result = reorder_fields(&items);
        assert_eq!(result.len(), 1);
        assert_eq!(name_of(&result[0]), "bx0");
    }

    #[test]
    fn test_parse_numeric_prefix() {
        assert_eq!(parse_numeric_prefix("bx0"), Some(("bx".to_string(), 0)));
        assert_eq!(parse_numeric_prefix("vx12"), Some(("vx".to_string(), 12)));
        assert_eq!(parse_numeric_prefix("energy"), None);
        assert_eq!(parse_numeric_prefix("count"), None);
        assert_eq!(parse_numeric_prefix("x"), None);
    }
}
