use crate::ast::{BinaryOpKind, Expr, Statement, Type, UnaryOpKind};
use std::collections::{HashMap, HashSet};

/// A group of isomorphic statements that can be vectorized.
#[derive(Debug, Clone)]
pub struct SlpIsomorphicGroup {
    /// Index of the first statement in the group within the txn body.
    pub base_index: usize,
    /// Number of repetitions (vector width).
    pub width: usize,
    /// Variable mapping for each lane beyond the template.
    /// lane_mappings[0] = {bx0→bx0, ...} (template identity)
    /// lane_mappings[1] = {bx0→bx1, ...} (lane 1 substitutions)
    pub lane_mappings: Vec<HashMap<String, String>>,
    /// The LHS variable names for each lane.
    pub lhs_names: Vec<String>,
    /// The body index for each lane. For contiguous groups this is
    /// base_index..base_index+width. For merged groups it's concatenated
    /// from source groups — lanes may come from non-contiguous positions.
    pub lane_positions: Vec<usize>,
    /// The vector element type.
    pub element_type: Type,
}

impl SlpIsomorphicGroup {
    pub fn is_viable(&self) -> bool {
        // Need at least 2 lanes for vectorization to be worthwhile
        self.width >= 2
    }
}

/// Result of analyzing a single transaction body for SLP opportunities.
#[derive(Debug, Clone, Default)]
pub struct SlpAnalysisResult {
    /// Groups of isomorphic statements found.
    pub groups: Vec<SlpIsomorphicGroup>,
    /// Whether any viable groups were found.
    pub has_slp_opportunities: bool,
}

/// Check if two expressions are structurally isomorphic under a variable mapping.
/// The mapping maps variable names from expression `a` to expression `b`.
/// Returns true if the expressions have the same structure (same BinaryOp kinds,
/// same literals) up to the variable renaming defined by `mapping`.
pub fn exprs_isomorphic(
    a: &Expr,
    b: &Expr,
    mapping: &HashMap<String, String>,
) -> bool {
    match (a, b) {
        (Expr::BinaryOp(k1, l1, r1), Expr::BinaryOp(k2, l2, r2)) => {
            k1 == k2
                && exprs_isomorphic(l1, l2, mapping)
                && exprs_isomorphic(r1, r2, mapping)
        }
        (Expr::UnaryOp(k1, e1), Expr::UnaryOp(k2, e2)) => {
            k1 == k2 && exprs_isomorphic(e1, e2, mapping)
        }
        (Expr::Decimal(n1), Expr::Decimal(n2)) => n1 == n2,
        (Expr::Float(n1), Expr::Float(n2)) => (n1 - n2).abs() < 1e-10,
        (Expr::Bool(b1), Expr::Bool(b2)) => b1 == b2,
        (Expr::Identifier(n1), Expr::Identifier(n2)) => {
            match mapping.get(n1) {
                Some(mapped) => mapped == n2,
                None => n1 == n2, // not in mapping, must be identical
            }
        }
        (Expr::Cast(e1, t1), Expr::Cast(e2, t2)) => {
            t1 == t2 && exprs_isomorphic(e1, e2, mapping)
        }
        (Expr::Field(obj1, f1), Expr::Field(obj2, f2)) => {
            f1 == f2 && exprs_isomorphic(obj1, obj2, mapping)
        }
        _ => false,
    }
}

/// Build a variable mapping from two expressions by finding the first identifier
/// difference and using it to create the mapping.
pub fn build_mapping(
    a: &Expr,
    b: &Expr,
) -> Option<HashMap<String, String>> {
    let mut mapping = HashMap::new();
    if try_build_mapping(a, b, &mut mapping) {
        Some(mapping)
    } else {
        None
    }
}

fn try_build_mapping(
    a: &Expr,
    b: &Expr,
    mapping: &mut HashMap<String, String>,
) -> bool {
    match (a, b) {
        (Expr::BinaryOp(k1, l1, r1), Expr::BinaryOp(k2, l2, r2)) => {
            k1 == k2
                && try_build_mapping(l1, l2, mapping)
                && try_build_mapping(r1, r2, mapping)
        }
        (Expr::UnaryOp(k1, e1), Expr::UnaryOp(k2, e2)) => {
            k1 == k2 && try_build_mapping(e1, e2, mapping)
        }
        (Expr::Decimal(n1), Expr::Decimal(n2)) => n1 == n2,
        (Expr::Float(n1), Expr::Float(n2)) => (n1 - n2).abs() < 1e-10,
        (Expr::Bool(b1), Expr::Bool(b2)) => b1 == b2,
        (Expr::Identifier(n1), Expr::Identifier(n2)) => {
            if n1 == n2 {
                return true; // same identifier — no mapping needed
            }
            match mapping.get(n1) {
                Some(mapped) => mapped == n2, // already mapped, must match
                None => {
                    // Check n2 isn't already mapped to something else
                    if mapping.values().any(|v| v == n2) {
                        return false;
                    }
                    mapping.insert(n1.clone(), n2.clone());
                    true
                }
            }
        }
        (Expr::Cast(e1, t1), Expr::Cast(e2, t2)) => {
            t1 == t2 && try_build_mapping(e1, e2, mapping)
        }
        (Expr::Field(obj1, f1), Expr::Field(obj2, f2)) => {
            f1 == f2 && try_build_mapping(obj1, obj2, mapping)
        }
        _ => false,
    }
}

/// Check if two statements are structurally isomorphic.
pub fn statements_isomorphic(
    a: &Statement,
    b: &Statement,
) -> Option<HashMap<String, String>> {
    match (a, b) {
        (Statement::Let { name: n1, ty: t1, expr: e1, .. },
         Statement::Let { name: n2, ty: t2, expr: e2, .. }) => {
            if t1 != t2 { return None; }
            let mapping = build_mapping(e1.as_ref()?, e2.as_ref()?)?;
            Some(mapping)
        }
        (Statement::Assign(l1, e1), Statement::Assign(l2, e2)) => {
            // Build mapping from LHS identifiers
            let mut mapping = HashMap::new();
            if !try_build_mapping_lhs(l1, l2, &mut mapping) {
                return None;
            }
            // Verify RHS expressions are isomorphic under this mapping
            if exprs_isomorphic(e1, e2, &mapping) {
                Some(mapping)
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Build mapping from LHS of two assignments.
fn try_build_mapping_lhs(
    a: &Expr,
    b: &Expr,
    mapping: &mut HashMap<String, String>,
) -> bool {
    match (a, b) {
        (Expr::Identifier(n1), Expr::Identifier(n2)) => {
            if n1 == n2 { return true; }
            if mapping.contains_key(n1) {
                mapping[n1] == *n2
            } else {
                mapping.insert(n1.clone(), n2.clone());
                true
            }
        }
        (Expr::Field(obj1, f1), Expr::Field(obj2, f2)) => {
            f1 == f2 && try_build_mapping_lhs(obj1, obj2, mapping)
        }
        _ => a == b,
    }
}

/// Find groups of isomorphic statements in a sequence.
/// Segments consecutive Let/Assign statements and groups those with
/// structural similarity.
fn find_isomorphic_groups(
    body: &[Statement],
    start_idx: usize,
) -> Vec<SlpIsomorphicGroup> {
    let mut groups = Vec::new();
    if start_idx >= body.len() {
        return groups;
    }

    let template = &body[start_idx];
    let mut mappings: Vec<HashMap<String, String>> = Vec::new();
    let mut lhs_names: Vec<String> = Vec::new();

    // Extract LHS name for the template
    let template_lhs = match template {
        Statement::Let { name, .. } => Some(name.clone()),
        Statement::Assign(Expr::Identifier(n), _) => Some(n.clone()),
        _ => None,
    };
    let template_lhs = match template_lhs {
        Some(n) => n,
        None => return groups,
    };
    lhs_names.push(template_lhs.clone());
    mappings.push(HashMap::new()); // template identity mapping

    // Compare subsequent statements to the template
    for (offset, stmt) in body.iter().enumerate().skip(start_idx + 1) {
        if let Some(mapping) = statements_isomorphic(template, stmt) {
            let lhs = match stmt {
                Statement::Let { name, .. } => name.clone(),
                Statement::Assign(Expr::Identifier(n), _) => n.clone(),
                _ => break,
            };
            mappings.push(mapping);
            lhs_names.push(lhs);
        } else {
            break; // non-isomorphic statement ends the group
        }
    }

    if mappings.len() >= 2 {
        let width = mappings.len();
        groups.push(SlpIsomorphicGroup {
            base_index: start_idx,
            width,
            lane_mappings: mappings,
            lhs_names,
            lane_positions: (start_idx..start_idx + width).collect(),
            element_type: Type::int(), // will be refined later
        });
    }

    groups
}

/// Compute a template signature for an expression. The signature captures the
/// structure (BinaryOp kinds, tree depth, literal values) but ignores variable
/// names — two expressions with the same signature are isomorphic.
fn expr_signature(expr: &Expr) -> Vec<u8> {
    let mut sig = Vec::new();
    expr_signature_inner(expr, &mut sig);
    sig
}

fn expr_signature_inner(expr: &Expr, sig: &mut Vec<u8>) {
    match expr {
        Expr::BinaryOp(kind, l, r) => {
            sig.push(1); // tag: BinaryOp
            sig.push(match kind {
                BinaryOpKind::Add => 1, BinaryOpKind::Sub => 2,
                BinaryOpKind::Mul => 3, BinaryOpKind::Div => 4,
                BinaryOpKind::Eq => 5, BinaryOpKind::Neq => 6,
                BinaryOpKind::Lt => 7, BinaryOpKind::Gt => 8,
                BinaryOpKind::Le => 9, BinaryOpKind::Ge => 10,
                _ => 0,
            });
            expr_signature_inner(l, sig);
            expr_signature_inner(r, sig);
        }
        Expr::UnaryOp(kind, e) => {
            sig.push(2); // tag: UnaryOp
            sig.push(match kind {
                UnaryOpKind::Neg => 1, UnaryOpKind::Not => 2,
                _ => 0,
            });
            expr_signature_inner(e, sig);
        }
        Expr::Identifier(_) => {
            sig.push(3); // tag: any identifier (variable name ignored)
        }
        Expr::Decimal(n) => {
            sig.push(4); // tag: decimal literal
            for &b in &n.to_le_bytes() { sig.push(b); }
        }
        Expr::Float(f) => {
            sig.push(5); // tag: float literal
            let bits = f.to_bits();
            for &b in &bits.to_le_bytes() { sig.push(b); }
        }
        Expr::Bool(b) => {
            sig.push(6); // tag: bool literal
            sig.push(if *b { 1 } else { 0 });
        }
        _ => {
            sig.push(0); // tag: other/non-vectorizable
        }
    }
}

/// Get the template signature for a group's first statement.
fn group_template_signature(body: &[Statement], group: &SlpIsomorphicGroup) -> Option<Vec<u8>> {
    let stmt = body.get(group.base_index)?;
    let expr = match stmt {
        Statement::Let { expr: Some(e), .. } => &*e,
        Statement::Assign(_, e) => &*e,
        _ => return None,
    };
    Some(expr_signature(expr))
}

/// Build a map from let-binding variable name to its definition position in the body.
fn build_def_sites(body: &[Statement]) -> HashMap<String, usize> {
    let mut defs = HashMap::new();
    for (i, stmt) in body.iter().enumerate() {
        match stmt {
            Statement::Let { name, .. } => { defs.insert(name.clone(), i); }
            Statement::Assign(lhs, _) => {
                if let Expr::Identifier(n) = &*lhs { defs.insert(n.clone(), i); }
            }
            _ => {}
        }
    }
    defs
}

/// Collect all identifier names referenced by an expression.
fn collect_expr_vars(expr: &Expr, out: &mut Vec<String>) {
    match expr {
        Expr::Identifier(n) => out.push(n.clone()),
        Expr::BinaryOp(_, l, r) => { collect_expr_vars(l, out); collect_expr_vars(r, out); }
        Expr::UnaryOp(_, e) => collect_expr_vars(e, out),
        Expr::Field(obj, _) => collect_expr_vars(obj, out),
        Expr::Cast(e, _) => collect_expr_vars(e, out),
        _ => {}
    }
}

/// Check that all variables referenced by a group's lanes are available
/// at the template's body position. A variable is available if:
///   a) It is NOT a let-binding (state field or constant) — always available
///   b) It IS a let-binding defined at body index < template_base_index
/// Returns false if any lane references a let-binding defined at or after
/// the template position (the emitter won't have its value yet).
fn all_deps_available(
    body: &[Statement],
    group: &SlpIsomorphicGroup,
    template_base_index: usize,
) -> bool {
    let def_sites = build_def_sites(body);

    for lane_idx in 0..group.width {
        let stmt = match body.get(group.base_index + lane_idx) {
            Some(s) => s,
            None => return false,
        };
        let rhs = match stmt {
            Statement::Let { expr: Some(e), .. } => &*e,
            Statement::Assign(_, e) => &*e,
            _ => continue,
        };

        let mut vars = Vec::new();
        collect_expr_vars(rhs, &mut vars);
        for var in &vars {
            match def_sites.get(var) {
                Some(&pos) if pos < template_base_index => continue,
                Some(_) => return false, // defined at or after template — not available
                None => continue, // state field or constant — always available
            }
        }
    }
    true
}

/// Merge groups with the same template signature into wider cross-pair groups.
/// Groups must have the same width to be merged. The resulting group contains
/// lanes from all merged groups, with composed lane mappings.
fn merge_groups(body: &[Statement], groups: Vec<SlpIsomorphicGroup>) -> Vec<SlpIsomorphicGroup> {
    if groups.len() < 2 {
        return groups;
    }

    // Compute signatures for each group
    let mut sig_buckets: HashMap<Vec<u8>, Vec<SlpIsomorphicGroup>> = HashMap::new();
    for g in &groups {
        if let Some(sig) = group_template_signature(body, g) {
            sig_buckets.entry(sig).or_default().push(g.clone());
        }
    }

    let mut merged = Vec::new();

    for (_, bucket) in sig_buckets {
        if bucket.len() < 2 {
            merged.extend(bucket);
            continue;
        }

        // Group by width within this signature bucket
        let mut by_width: HashMap<usize, Vec<SlpIsomorphicGroup>> = HashMap::new();
        for g in bucket {
            by_width.entry(g.width).or_default().push(g);
        }

        for (_, same_width_groups) in by_width {
            if same_width_groups.len() < 2 {
                merged.extend(same_width_groups);
                continue;
            }

            let mut sorted = same_width_groups;
            sorted.sort_by_key(|g| g.base_index);
            // 2026-07-21: Only merge groups where all lane dependencies are
            // available at the template position. Checks that every variable
            // referenced by a lane's RHS is either a state field (always available)
            // or a let-binding defined before the template.
            let template_base = sorted[0].base_index;
            sorted.retain(|g| g.base_index == template_base || all_deps_available(body, g, template_base));
            if sorted.len() < 2 {
                merged.extend(sorted);
                continue;
            }
            let template_group = sorted[0].clone();

            let mut merged_lane_mappings = template_group.lane_mappings.clone();
            let mut merged_lhs_names = template_group.lhs_names.clone();

            for src in sorted.iter().skip(1) {
                for lane_idx in 0..src.width {
                    let src_lhs = &src.lhs_names[lane_idx];
                    let src_mapping = &src.lane_mappings[lane_idx];
                    let mut composed = HashMap::new();
                    for (k, v) in src_mapping {
                        composed.insert(k.clone(), v.clone());
                    }
                    if lane_idx < template_group.lhs_names.len() {
                        let t_lhs = &template_group.lhs_names[lane_idx];
                        if t_lhs != src_lhs {
                            composed.insert(t_lhs.clone(), src_lhs.clone());
                        }
                    }
                    merged_lane_mappings.push(composed);
                    merged_lhs_names.push(src_lhs.clone());
                }
            }

            let merged_width = template_group.width * sorted.len();
            let mut merged_lane_positions = template_group.lane_positions.clone();
            for src in sorted.iter().skip(1) {
                for lane_idx in 0..src.width {
                    merged_lane_positions.push(src.lane_positions[lane_idx]);
                }
            }
            merged.push(SlpIsomorphicGroup {
                base_index: template_group.base_index,
                width: merged_width,
                lane_mappings: merged_lane_mappings,
                lhs_names: merged_lhs_names,
                lane_positions: merged_lane_positions,
                element_type: template_group.element_type.clone(),
            });
        }
    }

    merged
}

/// Analyze a transaction body for SLP vectorization opportunities.
pub fn analyze_body(body: &[Statement]) -> SlpAnalysisResult {
    let mut result = SlpAnalysisResult::default();
    let mut i = 0;
    while i < body.len() {
        match &body[i] {
            Statement::Let { .. } | Statement::Assign(Expr::Identifier(_), _) => {
                let groups = find_isomorphic_groups(body, i);
                for group in groups {
                    if group.is_viable() {
                        result.groups.push(group);
                    }
                }
                i += 1;
            }
            _ => {
                i += 1;
            }
        }
    }

    // 2026-07-21: Cross-pair merge — groups with the same template signature
    // across non-consecutive positions (e.g., all Newton-step-2 from pairs
    // 01-10) are merged into wider groups with independent lanes.
    if result.groups.len() >= 10 {
        let pre = result.groups.len();
        let old_groups = std::mem::take(&mut result.groups);
        let merged = merge_groups(body, old_groups);
        result.groups = merged;
    }

    result.has_slp_opportunities = !result.groups.is_empty();
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{BinaryOpKind, Expr, Statement};

    fn float_expr(val: f64) -> Expr {
        Expr::Float(val)
    }

    fn ident(n: &str) -> Expr {
        Expr::Identifier(n.to_string())
    }

    fn add_expr(a: Expr, b: Expr) -> Expr {
        Expr::BinaryOp(BinaryOpKind::Add, Box::new(a), Box::new(b))
    }

    fn sub_expr(a: Expr, b: Expr) -> Expr {
        Expr::BinaryOp(BinaryOpKind::Sub, Box::new(a), Box::new(b))
    }

    fn mul_expr(a: Expr, b: Expr) -> Expr {
        Expr::BinaryOp(BinaryOpKind::Mul, Box::new(a), Box::new(b))
    }

    #[test]
    fn test_isomorphic_simple_add() {
        let a = add_expr(ident("x"), float_expr(1.0));
        let b = add_expr(ident("y"), float_expr(1.0));
        let mapping = build_mapping(&a, &b);
        assert!(mapping.is_some(), "x+1 vs y+1 should be isomorphic");
        assert_eq!(mapping.unwrap().get("x"), Some(&"y".to_string()));
    }

    #[test]
    fn test_isomorphic_chain() {
        let a = mul_expr(add_expr(ident("x"), ident("y")), float_expr(0.5));
        let b = mul_expr(add_expr(ident("a"), ident("b")), float_expr(0.5));
        let mapping = build_mapping(&a, &b);
        assert!(mapping.is_some(), "0.5*(x+y) vs 0.5*(a+b) should be isomorphic");
    }

    #[test]
    fn test_not_isomorphic_different_op() {
        let a = add_expr(ident("x"), float_expr(1.0));
        let b = sub_expr(ident("x"), float_expr(1.0));
        assert!(!exprs_isomorphic(&a, &b, &HashMap::new()));
    }

    #[test]
    fn test_statement_isomorphic_let() {
        let a = Statement::Let { names: vec![], 
            name: "dx01".to_string(),
            ty: Some(Type::float()),
            expr: Some(sub_expr(ident("bx0"), ident("bx1"))),
            modifiers: vec![],
        };
        let b = Statement::Let { names: vec![], 
            name: "dx02".to_string(),
            ty: Some(Type::float()),
            expr: Some(sub_expr(ident("bx0"), ident("bx2"))),
            modifiers: vec![],
        };
        let mapping = statements_isomorphic(&a, &b);
        assert!(mapping.is_some(), "dx01 vs dx02 should be isomorphic");
    }

    #[test]
    fn test_nbody_distance_pattern() {
        // Simulate nbody's dx01/dy01 pattern
        let body = vec![
            Statement::Let { names: vec![], 
                name: "dx01".to_string(),
                ty: Some(Type::float()),
                expr: Some(sub_expr(ident("bx0"), ident("bx1"))),
                modifiers: vec![],
            },
            Statement::Let { names: vec![], 
                name: "dy01".to_string(),
                ty: Some(Type::float()),
                expr: Some(sub_expr(ident("by0"), ident("by1"))),
                modifiers: vec![],
            },
            Statement::Let { names: vec![], 
                name: "dz01".to_string(),
                ty: Some(Type::float()),
                expr: Some(sub_expr(ident("bz0"), ident("bz1"))),
                modifiers: vec![],
            },
        ];
        let result = analyze_body(&body);
        assert!(result.has_slp_opportunities,
            "dx01/dy01/dz01 should form an SLP group");
        assert_eq!(result.groups[0].width, 3,
            "Should have 3 lanes (dx, dy, dz)");
    }
}
