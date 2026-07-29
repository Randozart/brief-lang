// ── Abstraction Discovery — Ephemeral Helper Library ─────────────────────
// 2026-07-29: Abstraction discovery for depth-bounded enumerative synthesis.
// Extracts reusable sub-expressions from the pruned LevelCache and promotes
// them to helper functions (adapted from Koza ADFs [GP'92] and Feser et al.
// lambda abstraction [PLDI'15]). Helpers are ephemeral: discovered after
// depth N, registered into the LevelCache, consumed at depth N+1, and
// garbage-collected if unused.
// Flat code: each function max 2 levels of nesting.

use crate::ast::{BinaryOpKind, Expr, UnaryOpKind};
use crate::derive::engine::{CostModel, LevelCache, is_commutative_op, is_identity_op};
use std::collections::{HashMap, HashSet};

// ── Configuration ────────────────────────────────────────────────────

/// 2026-07-29: Configuration for abstraction discovery.
/// Defaults are conservative: activate at depth 2, require 5% frequency,
/// cap at 20 helpers per type.
#[derive(Debug, Clone)]
pub struct DiscoverConfig {
    /// Minimum depth for discovery (2 = after binary ops exist)
    pub min_depth: u8,
    /// Minimum frequency (0.0-1.0) of a sub-expression to promote
    pub min_frequency: f64,
    /// Maximum helpers per return type (Int, Float, Bool, compound)
    pub max_helpers_per_type: usize,
    /// Maximum cost of a helper body (expressions above this cost are
    /// too expensive to extract as helpers)
    pub max_body_cost: u64,
    /// Fixed overhead for helper declaration (in cost units)
    pub decl_overhead: u64,
}

impl Default for DiscoverConfig {
    fn default() -> Self {
        DiscoverConfig {
            min_depth: 2,
            min_frequency: 0.05,
            max_helpers_per_type: 20,
            max_body_cost: 10,
            decl_overhead: 0,
        }
    }
}

/// 2026-07-29: Global default — used by synthesize_enumerative.
/// decl_overhead = 0 because the search-space compression benefit of
/// helpers (reducing candidate cross product) is not captured by the
/// cost model. The real cost-control is GC: unused helpers are removed
/// after the next depth level.
pub static DISCOVER_CONFIG: DiscoverConfig = DiscoverConfig {
    min_depth: 2,
    min_frequency: 0.05,
    max_helpers_per_type: 20,
    max_body_cost: 10,
    decl_overhead: 0,
};

// ── Helper Type ──────────────────────────────────────────────────────

/// 2026-07-29: A helper function discovered during synthesis.
/// Represents a reusable sub-expression extracted from the LevelCache.
/// Name is auto-generated ("_h0", "_h1", ...). Params are the free
/// variables in the sub-expression. Body is the extracted expression.
/// Adapted from Koza's ADFs [GP'92] §6.3: subroutines discovered during
/// search are promoted to first-class primitives for subsequent depths.
#[derive(Debug, Clone)]
pub struct HelperFunction {
    /// Auto-generated name ("_h0", "_h1", ...)
    pub name: String,
    /// Parameter names (free variables of the extracted sub-expression)
    pub params: Vec<String>,
    /// Parameter types corresponding to params
    pub param_types: Vec<String>,
    /// The extracted expression body
    pub body: Expr,
    /// Return type of the helper (e.g., "Int", "Bool")
    pub ret_type: String,
    /// Body cost (for debugging/provenance)
    pub body_cost: u64,
    /// Cost to CALL the helper (cheaper than body to incentivize reuse)
    pub call_cost: u64,
    /// How many expressions at the next depth reference this helper
    pub use_count: usize,
}

// ── Fingerprinting for Deduplication ─────────────────────────────────

/// 2026-07-29: Structural fingerprint of an expression for deduplication.
/// Handles commutative ops by sorting operands: `x + y` and `y + x`
/// produce the same fingerprint. Used by discover_helpers to avoid
/// registering the same sub-expression twice.
fn expr_fingerprint(expr: &Expr) -> String {
    match expr {
        Expr::Decimal(n) => format!("d:{}", n),
        Expr::Float(f) => {
            let bits = f.to_bits();
            format!("f:{}", bits)
        }
        Expr::Bool(b) => format!("b:{}", b),
        Expr::Identifier(n) => format!("v:{}", n),
        Expr::UnaryOp(kind, inner) => {
            format!("u:{:?}({})", kind, expr_fingerprint(inner))
        }
        Expr::BinaryOp(kind, lhs, rhs) => {
            if is_commutative_op(*kind) {
                let mut fps = vec![expr_fingerprint(lhs), expr_fingerprint(rhs)];
                fps.sort();
                format!("b:{:?}({},{})", kind, fps[0], fps[1])
            } else {
                format!(
                    "b:{:?}({},{})",
                    kind,
                    expr_fingerprint(lhs),
                    expr_fingerprint(rhs)
                )
            }
        }
        Expr::If(cond, then, else_) => {
            let else_fp = else_
                .as_ref()
                .map(|e| expr_fingerprint(e))
                .unwrap_or_default();
            format!(
                "if({},{},{})",
                expr_fingerprint(cond),
                expr_fingerprint(then),
                else_fp
            )
        }
        _ => format!("{:?}", expr),
    }
}

// ── Sub-tree Extraction ──────────────────────────────────────────────

/// 2026-07-29: Collect all extractable sub-expressions from a single
/// expression tree. An extractable sub-expression is any node where:
///   - The subtree cost <= max_body_cost
///   - It involves at least one parameter (variable reference)
///   - It is not an identity operation (e.g., x + 0, x * 1)
/// Uses a recursive walk; each node is checked independently.
/// Flat code: single helper function with early returns.
fn collect_sub_trees(
    expr: &Expr,
    param_names: &[String],
    results: &mut Vec<(String, Expr, Vec<String>)>,
    is_root: bool,
) {
    match expr {
        // Constants and variables: never promote alone (no computational content)
        // unless the root (entire expression) is just a variable (identity function)
        Expr::Decimal(_) | Expr::Float(_) | Expr::Bool(_) => {
            if is_root {
                // Keep the root for absolute simplest cases (identity)
                // but it won't pass the cost_savings check without frequency.
            }
        }
        Expr::Identifier(_) => {
            if is_root {
                // Identity function — too trivial, skip.
            }
        }

        // 2026-07-29: Unary ops are extractable if the argument is a
        // variable or constant (depth-1 sub-expression). Examples: -x, !b.
        Expr::UnaryOp(kind, inner) => {
            let inner_is_leaf = matches!(
                inner.as_ref(),
                Expr::Identifier(_) | Expr::Decimal(_) | Expr::Float(_) | Expr::Bool(_)
            );
            let involves_param = has_variable(expr, param_names);
            if inner_is_leaf && involves_param {
                let cost = CostModel::default().cost_of_expr(expr);
                if cost <= 10 {
                    let free_vars = free_variables(expr, param_names);
                    results.push((expr_fingerprint(expr), expr.clone(), free_vars));
                }
            }
            // Recurse into inner for deeper extraction
            collect_sub_trees(inner, param_names, results, false);
        }

        // 2026-07-29: Binary ops are the primary helper candidates.
        // Extract if children are depth-1 (variables/constants) or if
        // the expression has cost <= max_body_cost. Examples: x + y,
        // x < 0, x & mask.
        Expr::BinaryOp(kind, lhs, rhs) => {
            let involves_param = has_variable(expr, param_names);
            let cost = CostModel::default().cost_of_expr(expr);

            // 2026-07-29: Skip identity operations (x + 0, x * 1, etc.)
            // These inflate the search space and produce no savings.
            if !is_identity_op(*kind, lhs, rhs) && involves_param && cost <= 10 {
                let free_vars = free_variables(expr, param_names);
                results.push((expr_fingerprint(expr), expr.clone(), free_vars));
            }

            // Recurse into children for deeper extraction
            collect_sub_trees(lhs, param_names, results, false);
            collect_sub_trees(rhs, param_names, results, false);
        }

        // 2026-07-29: If expressions are extractable when they form a
        // useful conditional (e.g., min/max/abs patterns). The cost
        // check naturally handles this: abs at cost ~14 may exceed
        // max_body_cost = 10, so it's extracted at depth 3 instead.
        Expr::If(cond, then, else_) => {
            let involves_param = has_variable(expr, param_names);
            let cost = CostModel::default().cost_of_expr(expr);
            if involves_param && cost <= 10 {
                let free_vars = free_variables(expr, param_names);
                results.push((expr_fingerprint(expr), expr.clone(), free_vars));
            }

            // Recurse into children
            collect_sub_trees(cond, param_names, results, false);
            collect_sub_trees(then, param_names, results, false);
            if let Some(e) = else_ {
                collect_sub_trees(e, param_names, results, false);
            }
        }

        // 2026-07-29: Match/Call/Field are compound-type expressions.
        // Extract them if they involve parameters and are small enough.
        Expr::Call(_, args, _) => {
            let involves_param = has_variable(expr, param_names);
            let cost = CostModel::default().cost_of_expr(expr);
            if involves_param && cost <= 10 {
                let free_vars = free_variables(expr, param_names);
                results.push((expr_fingerprint(expr), expr.clone(), free_vars));
            }
            for arg in args {
                collect_sub_trees(arg, param_names, results, false);
            }
        }
        Expr::Field(inner, _) => {
            let involves_param = has_variable(expr, param_names);
            let cost = CostModel::default().cost_of_expr(expr);
            if involves_param && cost <= 10 {
                let free_vars = free_variables(expr, param_names);
                results.push((expr_fingerprint(expr), expr.clone(), free_vars));
            }
            collect_sub_trees(inner, param_names, results, false);
        }
        Expr::Match(scrut, arms) => {
            let involves_param = has_variable(expr, param_names);
            let cost = CostModel::default().cost_of_expr(expr);
            if involves_param && cost <= 10 {
                let free_vars = free_variables(expr, param_names);
                results.push((expr_fingerprint(expr), expr.clone(), free_vars));
            }
            collect_sub_trees(scrut, param_names, results, false);
            for arm in arms {
                collect_sub_trees(&arm.body, param_names, results, false);
            }
        }
        _ => {}
    }
}

/// 2026-07-29: Check if an expression references at least one variable
/// from the given param list. Used to filter out constant-only sub-trees
/// (e.g., 1 + 2) which don't depend on inputs and are thus not reusable.
fn has_variable(expr: &Expr, param_names: &[String]) -> bool {
    match expr {
        Expr::Decimal(_) | Expr::Float(_) | Expr::Bool(_) => false,
        Expr::Identifier(name) => param_names.iter().any(|p| p == name),
        Expr::UnaryOp(_, inner) => has_variable(inner, param_names),
        Expr::BinaryOp(_, lhs, rhs) => {
            has_variable(lhs, param_names) || has_variable(rhs, param_names)
        }
        Expr::If(cond, then, else_) => {
            has_variable(cond, param_names)
                || has_variable(then, param_names)
                || else_
                    .as_ref()
                    .map_or(false, |e| has_variable(e, param_names))
        }
        Expr::Call(_, args, _) => args.iter().any(|a| has_variable(a, param_names)),
        Expr::Field(inner, _) => has_variable(inner, param_names),
        Expr::Match(scrut, arms) => {
            has_variable(scrut, param_names)
                || arms.iter().any(|a| has_variable(&a.body, param_names))
        }
        _ => false,
    }
}

/// 2026-07-29: Collect the set of parameter names referenced in an
/// expression. These become the helper function's parameters.
fn free_variables(expr: &Expr, param_names: &[String]) -> Vec<String> {
    let mut vars = Vec::new();
    collect_free_vars(expr, param_names, &mut vars);
    // Deduplicate while preserving order
    let mut seen = HashSet::new();
    vars.retain(|v| seen.insert(v.clone()));
    vars
}

fn collect_free_vars(expr: &Expr, param_names: &[String], vars: &mut Vec<String>) {
    match expr {
        Expr::Identifier(name) => {
            if param_names.iter().any(|p| p == name) && !vars.contains(name) {
                vars.push(name.clone());
            }
        }
        Expr::UnaryOp(_, inner) => collect_free_vars(inner, param_names, vars),
        Expr::BinaryOp(_, lhs, rhs) => {
            collect_free_vars(lhs, param_names, vars);
            collect_free_vars(rhs, param_names, vars);
        }
        Expr::If(cond, then, else_) => {
            collect_free_vars(cond, param_names, vars);
            collect_free_vars(then, param_names, vars);
            if let Some(e) = else_ {
                collect_free_vars(e, param_names, vars);
            }
        }
        Expr::Call(_, args, _) => {
            for arg in args {
                collect_free_vars(arg, param_names, vars);
            }
        }
        Expr::Field(inner, _) => collect_free_vars(inner, param_names, vars),
        Expr::Match(scrut, arms) => {
            collect_free_vars(scrut, param_names, vars);
            for arm in arms {
                collect_free_vars(&arm.body, param_names, vars);
            }
        }
        _ => {}
    }
}

/// 2026-07-29: Infer the return type of an expression for helper registration.
/// Uses the same logic as expr_type_hint_with_params in engine.rs but
/// works independently.
fn infer_return_type(expr: &Expr) -> String {
    match expr {
        Expr::Decimal(_) => "Int".to_string(),
        Expr::Float(_) => "Float".to_string(),
        Expr::Bool(_) => "Bool".to_string(),
        Expr::Identifier(_) => "Int".to_string(), // default; caller corrects via param_types
        Expr::UnaryOp(UnaryOpKind::Neg, _) => "Int".to_string(),
        Expr::UnaryOp(UnaryOpKind::Not, _) => "Bool".to_string(),
        Expr::UnaryOp(UnaryOpKind::BitNot, _) => "Int".to_string(),
        Expr::BinaryOp(op, _, _) => {
            match op {
                BinaryOpKind::Add | BinaryOpKind::Sub | BinaryOpKind::Mul
                | BinaryOpKind::Div | BinaryOpKind::Mod
                | BinaryOpKind::BitAnd | BinaryOpKind::BitOr | BinaryOpKind::BitXor
                | BinaryOpKind::Shl | BinaryOpKind::Shr => "Int".to_string(),
                BinaryOpKind::Eq | BinaryOpKind::Neq
                | BinaryOpKind::Lt | BinaryOpKind::Gt
                | BinaryOpKind::Le | BinaryOpKind::Ge
                | BinaryOpKind::And | BinaryOpKind::Or => "Bool".to_string(),
                BinaryOpKind::Concat => "Int".to_string(),
            }
        }
        Expr::If(_, _, _) => "Int".to_string(), // corrected by caller if needed
        _ => "Int".to_string(),
    }
}

// ── Discovery ────────────────────────────────────────────────────────

/// 2026-07-29: Discover useful helper functions from the pruned LevelCache.
///
/// Algorithm (adapted from Koza ADFs [GP'92] §6.3 and Schmidt/Lipson
/// Eureqa [SL'09] cost-savings metric):
///
/// 1. For each expression in the LevelCache, extract all extractable
///    sub-expressions (depth-2 sub-trees involving at least one param).
/// 2. Count frequency: how many LevelCache expressions contain each
///    sub-expression (by structural fingerprint).
/// 3. Score by cost savings:
///    savings = body_cost × freq - call_cost × freq - decl_overhead
/// 4. Deduplicate by fingerprint. Return top-k per return type.
///
/// Only expressions with savings > 0 are promoted. This ensures that
/// a helper never makes the search worse.
///
/// The frequency analysis uses the LevelCache (pruned, non-redundant set)
/// rather than raw candidates, because the LevelCache represents the
/// diverse set of useful computations at the current depth.
pub(crate) fn discover_helpers(
    cache: &LevelCache,
    param_names: &[String],
    param_types: &[String],
    config: &DiscoverConfig,
) -> Vec<HelperFunction> {
    // Step 1: Collect all expressions from the LevelCache into a flat list
    let mut all_exprs: Vec<&Expr> = Vec::new();
    for e in &cache.int_exprs {
        all_exprs.push(e);
    }
    for e in &cache.float_exprs {
        all_exprs.push(e);
    }
    for e in &cache.bool_exprs {
        all_exprs.push(e);
    }
    for list in cache.compound_exprs.values() {
        for e in list {
            all_exprs.push(e);
        }
    }

    if all_exprs.len() < 2 {
        return Vec::new();
    }

    // Step 2: Extract all sub-trees from all expressions
    // (fingerprint, expr, free_vars) — raw, before dedup
    let mut raw_sub_trees: Vec<(String, Expr, Vec<String>)> = Vec::new();
    for expr in &all_exprs {
        collect_sub_trees(expr, param_names, &mut raw_sub_trees, true);
    }

    if raw_sub_trees.is_empty() {
        return Vec::new();
    }

    // Step 3: Deduplicate by fingerprint — keep the first occurrence
    let mut seen_fp: HashSet<String> = HashSet::new();
    // (fingerprint, expr, ret_type, free_vars, frequency)
    let mut candidates: Vec<(String, Expr, String, Vec<String>, usize)> = Vec::new();

    for (fp, expr, free_vars) in &raw_sub_trees {
        if seen_fp.insert(fp.clone()) {
            if free_vars.is_empty() {
                continue; // skip constant-only expressions
            }
            // Infer the return type
            let mut ret_type = infer_return_type(expr);
            // Correct identifier types from param_types
            if let Expr::Identifier(name) = expr {
                if let Some(idx) = param_names.iter().position(|n| n == name) {
                    if let Some(t) = param_types.get(idx) {
                        ret_type = t.clone();
                    }
                }
            }
            // Skip Bool-returning binary ops when all operands are params
            // of type Bool? No, keep them — Bool ops are useful as IF conditions.

            candidates.push((fp.clone(), expr.clone(), ret_type, free_vars.clone(), 0));
        }
    }

    // Step 4: Count frequency — how many LevelCache expressions contain
    // each candidate sub-expression. We need to re-scan all_exprs.
    for (fp, _, _, _, freq) in &mut candidates {
        for expr in &all_exprs {
            if contains_subtree(expr, fp, param_names) {
                *freq += 1;
            }
        }
    }

    // Step 5: Score by cost savings and filter
    let n_exprs = all_exprs.len() as f64;
    let cost_model = CostModel::default();
    // (score, expr, ret_type, free_vars, body_cost, call_cost, freq)
    let mut scored: Vec<(f64, Expr, String, Vec<String>, u64, u64, usize)> = Vec::new();

    for (_fp, expr, ret_type, free_vars, freq) in candidates {
        if freq == 0 || (freq as f64 / n_exprs) < config.min_frequency {
            continue;
        }
        let body_cost = cost_model.cost_of_expr(&expr);
        if body_cost > config.max_body_cost {
            continue;
        }
            // call_cost = Expr::Call baseline (3) + number of args
            let call_cost = 3 + free_vars.len() as u64;
            // 2026-07-29: Cost savings = body_cost * freq - call_cost * freq.
            // decl_overhead is the fixed cost of declaring the helper.
            // We require savings >= 0 (not > 0) because the search-space
            // compression benefit of helpers is not captured by the cost model.
            // A helper with body_cost = call_cost (e.g., x + y at cost 5) still
            // reduces the candidate cross product at depth 4+, so we promote it
            // if it appears frequently enough.
            let savings = body_cost as f64 * freq as f64
                - call_cost as f64 * freq as f64
                - config.decl_overhead as f64;
            if savings >= 0.0 {
            scored.push((savings, expr, ret_type, free_vars, body_cost, call_cost, freq));
        }
    }

    // Step 6: Sort by score descending
    scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    // Step 7: Top-k per return type
    let mut by_type: HashMap<String, Vec<HelperFunction>> = HashMap::new();
    let mut name_counter = 0;

    for (_score, expr, ret_type, free_vars, body_cost, call_cost, _freq) in &scored {
        let entry = by_type.entry(ret_type.clone()).or_default();
        if entry.len() >= config.max_helpers_per_type {
            continue;
        }
        // Resolve parameter types
        let param_types_resolved: Vec<String> = free_vars
            .iter()
            .map(|v| {
                param_names
                    .iter()
                    .position(|n| n == v)
                    .and_then(|idx| param_types.get(idx).cloned())
                    .unwrap_or_else(|| "Int".to_string())
            })
            .collect();

        let name = format!("_h{}", name_counter);
        name_counter += 1;

        entry.push(HelperFunction {
            name,
            params: free_vars.clone(),
            param_types: param_types_resolved,
            body: expr.clone(),
            ret_type: ret_type.clone(),
            body_cost: *body_cost,
            call_cost: *call_cost,
            use_count: 0,
        });
    }

    // Flatten and return
    let mut result: Vec<HelperFunction> = Vec::new();
    for mut entry in by_type.into_values() {
        result.append(&mut entry);
    }
    result
}

/// 2026-07-29: Check whether a LevelCache expression contains a subtree
/// matching the given fingerprint. This is a structural membership test:
/// we walk the expression and compare each node's fingerprint to the target.
fn contains_subtree(expr: &Expr, target_fp: &str, param_names: &[String]) -> bool {
    // Fast path: check the root first
    if expr_fingerprint(expr) == target_fp {
        return true;
    }
    // Recurse into children
    match expr {
        Expr::Decimal(_) | Expr::Float(_) | Expr::Bool(_) | Expr::Identifier(_) => false,
        Expr::UnaryOp(_, inner) => contains_subtree(inner, target_fp, param_names),
        Expr::BinaryOp(_, lhs, rhs) => {
            contains_subtree(lhs, target_fp, param_names)
                || contains_subtree(rhs, target_fp, param_names)
        }
        Expr::If(cond, then, else_) => {
            contains_subtree(cond, target_fp, param_names)
                || contains_subtree(then, target_fp, param_names)
                || else_
                    .as_ref()
                    .map_or(false, |e| contains_subtree(e, target_fp, param_names))
        }
        Expr::Call(_, args, _) => args.iter().any(|a| contains_subtree(a, target_fp, param_names)),
        Expr::Field(inner, _) => contains_subtree(inner, target_fp, param_names),
        Expr::Match(scrut, arms) => {
            contains_subtree(scrut, target_fp, param_names)
                || arms.iter().any(|a| contains_subtree(&a.body, target_fp, param_names))
        }
        _ => false,
    }
}

// ── Registration ─────────────────────────────────────────────────────

/// 2026-07-29: Register discovered helpers into the LevelCache.
/// Each helper is injected into the appropriate per-type bucket as an
/// Expr::Call(helper_name, params, None). The helper's name is also
/// added to cache.helper_names for iteration in generate_next_level().
///
/// The Expr::Call costs 3 + N × 1 (variable cost) in the existing
/// CostModel, which is naturally cheaper than the full helper body.
/// No modification to CostModel is needed.
///
/// Registering a helper makes it available as a single-token expression
/// at the next depth level. This is the key scaling improvement: instead
/// of regenerating the helper's entire subtree at depth N+1, the search
/// references it by name.
pub(crate) fn register_helpers(cache: &mut LevelCache, helpers: &[HelperFunction]) {
    for helper in helpers {
        if helper.params.is_empty() {
            // Constant helper — generate once and store
            let call = helper.body.clone();
            match helper.ret_type.as_str() {
                "Int" => cache.int_exprs.push(call),
                "Float" => cache.float_exprs.push(call),
                "Bool" => cache.bool_exprs.push(call),
                _ => {
                    cache
                        .compound_exprs
                        .entry(helper.ret_type.clone())
                        .or_default()
                        .push(call);
                }
            }
        } else {
            let args: Vec<Expr> = helper
                .params
                .iter()
                .map(|p| Expr::Identifier(p.clone()))
                .collect();
            let call = Expr::Call(helper.name.clone(), args, None);
            match helper.ret_type.as_str() {
                "Int" => cache.int_exprs.push(call),
                "Float" => cache.float_exprs.push(call),
                "Bool" => cache.bool_exprs.push(call),
                _ => {
                    cache
                        .compound_exprs
                        .entry(helper.ret_type.clone())
                        .or_default()
                        .push(call);
                }
            }
        }
        // 2026-07-29: Track the helper name for iteration in
        // generate_next_level(). This also enables GC: after depth N+1,
        // we remove helpers with zero references.
        if !cache.helper_names.contains(&helper.name) {
            cache.helper_names.push(helper.name.clone());
        }
    }
}

// ── Garbage Collection ───────────────────────────────────────────────

/// 2026-07-29: Garbage-collect unused helpers after each depth level.
/// A helper with zero references at depth N+1 is removed from the
/// LevelCache and the global pool. This is the ephemeral lifecycle:
/// helpers live only during the depths where they're actively referenced.
/// Unreferenced helpers cannot be referenced at future depths (the search
/// is monotonically widening), so early removal is safe.
pub(crate) fn gc_helpers(
    cache: &mut LevelCache,
    helpers: &mut Vec<HelperFunction>,
) {
    // Build set of active (referenced) helper names
    let active: HashSet<String> = helpers
        .iter()
        .filter(|h| h.use_count > 0)
        .map(|h| h.name.clone())
        .collect();

    // Retain only active helpers
    helpers.retain(|h| h.use_count > 0);

    // Update cache.helper_names to match
    cache
        .helper_names
        .retain(|name| active.contains(name));

    // 2026-07-29: Also remove helper call expressions from per-type buckets.
    // A helper call is Expr::Call(name, args, None). We scan each bucket
    // and remove calls whose name is no longer active.
    for bucket in [&mut cache.int_exprs, &mut cache.float_exprs, &mut cache.bool_exprs] {
        bucket.retain(|e| {
            if let Expr::Call(name, _, _) = e {
                active.contains(name)
            } else {
                true
            }
        });
    }
    for list in cache.compound_exprs.values_mut() {
        list.retain(|e| {
            if let Expr::Call(name, _, _) = e {
                active.contains(name)
            } else {
                true
            }
        });
    }
}

/// 2026-07-29: Increment use counts for helpers that are referenced
/// in a batch of expressions. Called by generate_next_level() after
/// candidate generation to track which helpers are actually consumed.
pub fn count_helper_uses(helpers: &mut Vec<HelperFunction>, candidates: &[Expr], param_names: &[String]) {
    // Reset all use counts
    for helper in helpers.iter_mut() {
        helper.use_count = 0;
    }

    // For each candidate, check if it calls each helper
    for candidate in candidates {
        count_uses_in_expr(candidate, helpers, param_names);
    }
}

fn count_uses_in_expr(expr: &Expr, helpers: &mut [HelperFunction], param_names: &[String]) {
    match expr {
        Expr::Call(name, _, _) => {
            if let Some(helper) = helpers.iter_mut().find(|h| h.name == *name) {
                helper.use_count += 1;
            }
        }
        Expr::UnaryOp(_, inner) => count_uses_in_expr(inner, helpers, param_names),
        Expr::BinaryOp(_, lhs, rhs) => {
            count_uses_in_expr(lhs, helpers, param_names);
            count_uses_in_expr(rhs, helpers, param_names);
        }
        Expr::If(cond, then, else_) => {
            count_uses_in_expr(cond, helpers, param_names);
            count_uses_in_expr(then, helpers, param_names);
            if let Some(e) = else_ {
                count_uses_in_expr(e, helpers, param_names);
            }
        }
        Expr::Call(_, args, _) => {
            for arg in args {
                count_uses_in_expr(arg, helpers, param_names);
            }
        }
        Expr::Field(inner, _) => count_uses_in_expr(inner, helpers, param_names),
        Expr::Match(scrut, arms) => {
            count_uses_in_expr(scrut, helpers, param_names);
            for arm in arms {
                count_uses_in_expr(&arm.body, helpers, param_names);
            }
        }
        _ => {}
    }
}

// ── Tests ────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::derive::engine::LevelCache;

    fn empty_cache() -> LevelCache {
        LevelCache {
            int_exprs: vec![],
            float_exprs: vec![],
            bool_exprs: vec![],
            compound_exprs: std::collections::HashMap::new(),
            helper_names: vec![],
            helper_info: std::collections::HashMap::new(),
        }
    }

    fn int_var(name: &str) -> Expr {
        Expr::Identifier(name.to_string())
    }

    fn int_const(n: i64) -> Expr {
        Expr::Decimal(n)
    }

    fn bool_const(b: bool) -> Expr {
        Expr::Bool(b)
    }

    fn binop(kind: BinaryOpKind, lhs: Expr, rhs: Expr) -> Expr {
        Expr::BinaryOp(kind, Box::new(lhs), Box::new(rhs))
    }

    fn uneg(inner: Expr) -> Expr {
        Expr::UnaryOp(UnaryOpKind::Neg, Box::new(inner))
    }

    // ── Fingerprint Tests ─────────────────────────────────────────

    #[test]
    fn test_fingerprint_commutative_dedup() {
        let expr1 = binop(BinaryOpKind::Add, int_var("x"), int_var("y"));
        let expr2 = binop(BinaryOpKind::Add, int_var("y"), int_var("x"));
        assert_eq!(
            expr_fingerprint(&expr1),
            expr_fingerprint(&expr2),
            "commutative ops should produce same fingerprint"
        );
    }

    #[test]
    fn test_fingerprint_noncommutative_distinct() {
        let expr1 = binop(BinaryOpKind::Sub, int_var("x"), int_var("y"));
        let expr2 = binop(BinaryOpKind::Sub, int_var("y"), int_var("x"));
        // For non-commutative ops, x - y != y - x
        assert_ne!(
            expr_fingerprint(&expr1),
            expr_fingerprint(&expr2),
            "non-commutative reversed should differ"
        );
    }

    #[test]
    fn test_fingerprint_distinct_ops() {
        let add = binop(BinaryOpKind::Add, int_var("x"), int_var("y"));
        let mul = binop(BinaryOpKind::Mul, int_var("x"), int_var("y"));
        assert_ne!(
            expr_fingerprint(&add),
            expr_fingerprint(&mul),
            "Add and Mul should have different fingerprints"
        );
    }

    // ── Free Variables Tests ───────────────────────────────────────

    #[test]
    fn test_free_vars_simple() {
        let params = vec!["x".to_string(), "y".to_string()];
        let expr = binop(BinaryOpKind::Add, int_var("x"), int_var("y"));
        let fv = free_variables(&expr, &params);
        assert_eq!(fv.len(), 2);
        assert!(fv.contains(&"x".to_string()));
        assert!(fv.contains(&"y".to_string()));
    }

    #[test]
    fn test_free_vars_constant_only() {
        let params = vec!["x".to_string()];
        let expr = int_const(42);
        let fv = free_variables(&expr, &params);
        assert!(fv.is_empty(), "constant should have no free vars");
    }

    #[test]
    fn test_free_vars_partial() {
        let params = vec!["x".to_string(), "y".to_string()];
        let expr = binop(BinaryOpKind::Add, int_var("x"), int_const(1));
        let fv = free_variables(&expr, &params);
        assert_eq!(fv.len(), 1);
        assert!(fv.contains(&"x".to_string()));
    }

    // ── has_variable Tests ─────────────────────────────────────────

    #[test]
    fn test_has_variable_true() {
        let params = vec!["x".to_string()];
        assert!(has_variable(&int_var("x"), &params));
    }

    #[test]
    fn test_has_variable_false() {
        let params = vec!["x".to_string()];
        assert!(!has_variable(&int_const(42), &params));
        assert!(!has_variable(&bool_const(true), &params));
    }

    // ── Sub-tree Extraction Tests ──────────────────────────────────

    #[test]
    fn test_collect_sub_trees_binary_op() {
        let params = vec!["x".to_string(), "y".to_string()];
        let expr = binop(BinaryOpKind::Add, int_var("x"), int_var("y"));
        let mut results = Vec::new();
        collect_sub_trees(&expr, &params, &mut results, true);
        // Should contain at least x + y
        let add_fp = expr_fingerprint(&expr);
        assert!(
            results.iter().any(|(fp, _, _)| *fp == add_fp),
            "should extract x + y as a sub-tree"
        );
    }

    #[test]
    fn test_collect_sub_trees_constant_only_skipped() {
        let params = vec!["x".to_string()];
        let expr = binop(BinaryOpKind::Add, int_const(1), int_const(2));
        let mut results = Vec::new();
        collect_sub_trees(&expr, &params, &mut results, true);
        // 1 + 2 has no parameter, should not be extracted
        assert!(
            results.is_empty(),
            "constant-only sub-trees should not be extracted"
        );
    }

    #[test]
    fn test_collect_sub_trees_identity_op_skipped() {
        let params = vec!["x".to_string()];
        // x + 0 is an identity op
        let expr = binop(BinaryOpKind::Add, int_var("x"), int_const(0));
        let mut results = Vec::new();
        collect_sub_trees(&expr, &params, &mut results, true);
        let fp = expr_fingerprint(&expr);
        assert!(
            !results.iter().any(|(f, _, _)| *f == fp),
            "identity op x + 0 should be skipped"
        );
    }

    // ── discover_helpers Tests ─────────────────────────────────────

    #[test]
    fn test_discover_empty_cache() {
        let cache = empty_cache();
        let params = vec!["x".to_string()];
        let param_types = vec!["Int".to_string()];
        let helpers = discover_helpers(&cache, &params, &param_types, &DiscoverConfig::default());
        assert!(helpers.is_empty(), "empty cache should produce no helpers");
    }

    #[test]
    fn test_discover_single_expr_no_reuse() {
        let mut cache = empty_cache();
        cache.int_exprs.push(int_var("x"));
        let params = vec!["x".to_string()];
        let param_types = vec!["Int".to_string()];
        let helpers = discover_helpers(&cache, &params, &param_types, &DiscoverConfig::default());
        // Single expression (just x) — no useful abstraction
        assert!(helpers.is_empty());
    }

    #[test]
    fn test_discover_reusable_add() {
        let mut cache = empty_cache();
        // Build a LevelCache with several expressions containing x + y
        let x_plus_y = binop(BinaryOpKind::Add, int_var("x"), int_var("y"));
        cache.int_exprs.push(x_plus_y.clone());
        cache.int_exprs.push(binop(BinaryOpKind::Sub, int_var("x"), int_var("y")));
        cache.int_exprs.push(binop(BinaryOpKind::Mul, int_var("x"), int_var("y")));

        let params = vec!["x".to_string(), "y".to_string()];
        let param_types = vec!["Int".to_string(), "Int".to_string()];
        let helpers = discover_helpers(&cache, &params, &param_types, &DiscoverConfig::default());
        // x + y appears in 1/3 = 33% of expressions, above 5% threshold
        assert!(!helpers.is_empty(), "should discover at least one helper");
        // At least one helper should be x + y
        let has_add = helpers.iter().any(|h| {
            matches!(&h.body, Expr::BinaryOp(BinaryOpKind::Add, _, _))
        });
        assert!(has_add, "should discover x + y");
    }

    #[test]
    fn test_discover_below_threshold() {
        let mut cache = empty_cache();
        let x_plus_y = binop(BinaryOpKind::Add, int_var("x"), int_var("y"));
        cache.int_exprs.push(x_plus_y.clone());
        // Add many other expressions so x + y appears in < 5%
        for i in 0..100 {
            cache.int_exprs.push(binop(
                BinaryOpKind::Add,
                int_var("x"),
                int_const(i),
            ));
        }

        let params = vec!["x".to_string(), "y".to_string()];
        let param_types = vec!["Int".to_string(), "Int".to_string()];
        let helpers = discover_helpers(&cache, &params, &param_types, &DiscoverConfig::default());
        // x + y appears in 1/101 ≈ 0.99%, below 5% threshold
        let has_add = helpers.iter().any(|h| {
            matches!(&h.body, Expr::BinaryOp(BinaryOpKind::Add, lhs, _) if matches!(lhs.as_ref(), Expr::Identifier(n) if n == "x"))
        });
        assert!(!has_add, "x + y should not be discovered below frequency threshold");
    }

    // ── register_helpers Tests ─────────────────────────────────────

    #[test]
    fn test_register_single_helper() {
        let mut cache = empty_cache();
        let helper = HelperFunction {
            name: "_h0".to_string(),
            params: vec!["x".to_string()],
            param_types: vec!["Int".to_string()],
            body: binop(BinaryOpKind::Add, int_var("x"), int_const(1)),
            ret_type: "Int".to_string(),
            body_cost: 5,
            call_cost: 4,
            use_count: 0,
        };

        register_helpers(&mut cache, &[helper]);

        assert_eq!(cache.int_exprs.len(), 1);
        assert!(cache.helper_names.contains(&"_h0".to_string()));
        match &cache.int_exprs[0] {
            Expr::Call(name, args, None) => {
                assert_eq!(name, "_h0");
                assert_eq!(args.len(), 1);
                assert!(matches!(&args[0], Expr::Identifier(n) if n == "x"));
            }
            _ => panic!("expected Expr::Call"),
        }
    }

    #[test]
    fn test_register_constant_helper() {
        let mut cache = empty_cache();
        let helper = HelperFunction {
            name: "_h0".to_string(),
            params: vec![],  // no params — constant
            param_types: vec![],
            body: int_const(42),
            ret_type: "Int".to_string(),
            body_cost: 1,
            call_cost: 1,
            use_count: 0,
        };

        register_helpers(&mut cache, &[helper]);

        assert_eq!(cache.int_exprs.len(), 1);
        match &cache.int_exprs[0] {
            Expr::Decimal(42) => {} // constant helper stored as raw value
            _ => panic!("expected Expr::Decimal(42)"),
        }
    }

    #[test]
    fn test_register_max_cap() {
        let mut cache = empty_cache();
        let mut helpers = Vec::new();
        for i in 0..25 {
            helpers.push(HelperFunction {
                name: format!("_h{}", i),
                params: vec!["x".to_string()],
                param_types: vec!["Int".to_string()],
                body: binop(BinaryOpKind::Add, int_var("x"), int_const(i)),
                ret_type: "Int".to_string(),
                body_cost: 5,
                call_cost: 4,
                use_count: 0,
            });
        }

        register_helpers(&mut cache, &helpers);

        // All 25 helpers should have been registered (the cap is on
        // discovery, not registration — registration is additive)
        assert_eq!(cache.int_exprs.len(), 25);
        assert_eq!(cache.helper_names.len(), 25);
    }

    // ── GC Tests ───────────────────────────────────────────────────

    #[test]
    fn test_gc_unused_helper() {
        let mut cache = empty_cache();
        let mut helpers = vec![HelperFunction {
            name: "_h0".to_string(),
            params: vec!["x".to_string()],
            param_types: vec!["Int".to_string()],
            body: binop(BinaryOpKind::Add, int_var("x"), int_const(1)),
            ret_type: "Int".to_string(),
            body_cost: 5,
            call_cost: 4,
            use_count: 0, // unused!
        }];
        register_helpers(&mut cache, &helpers);

        gc_helpers(&mut cache, &mut helpers);

        assert!(helpers.is_empty(), "unused helper should be GC'd");
        assert!(!cache.helper_names.contains(&"_h0".to_string()));
        assert!(cache.int_exprs.is_empty());
    }

    #[test]
    fn test_gc_keeps_used() {
        let mut cache = empty_cache();
        let mut helpers = vec![HelperFunction {
            name: "_h0".to_string(),
            params: vec!["x".to_string()],
            param_types: vec!["Int".to_string()],
            body: binop(BinaryOpKind::Add, int_var("x"), int_const(1)),
            ret_type: "Int".to_string(),
            body_cost: 5,
            call_cost: 4,
            use_count: 3, // used!
        }];
        register_helpers(&mut cache, &mut helpers);

        gc_helpers(&mut cache, &mut helpers);

        assert_eq!(helpers.len(), 1, "used helper should survive GC");
        assert!(cache.helper_names.contains(&"_h0".to_string()));
    }

    // ── count_helper_uses Tests ───────────────────────────────────

    #[test]
    fn test_count_single_use() {
        let mut helpers = vec![HelperFunction {
            name: "_h0".to_string(),
            params: vec!["x".to_string()],
            param_types: vec!["Int".to_string()],
            body: binop(BinaryOpKind::Add, int_var("x"), int_const(1)),
            ret_type: "Int".to_string(),
            body_cost: 5,
            call_cost: 4,
            use_count: 0,
        }];

        let candidates = vec![Expr::Call(
            "_h0".to_string(),
            vec![int_var("x")],
            None,
        )];

        let params = vec!["x".to_string()];
        count_helper_uses(&mut helpers, &candidates, &params);

        assert_eq!(helpers[0].use_count, 1);
    }

    #[test]
    fn test_count_no_use() {
        let mut helpers = vec![HelperFunction {
            name: "_h0".to_string(),
            params: vec!["x".to_string()],
            param_types: vec!["Int".to_string()],
            body: binop(BinaryOpKind::Add, int_var("x"), int_const(1)),
            ret_type: "Int".to_string(),
            body_cost: 5,
            call_cost: 4,
            use_count: 0,
        }];

        // Candidate does not call _h0
        let candidates = vec![int_var("x")];
        let params = vec!["x".to_string()];
        count_helper_uses(&mut helpers, &candidates, &params);

        assert_eq!(helpers[0].use_count, 0);
    }

    #[test]
    fn test_count_multiple_uses() {
        let mut helpers = vec![HelperFunction {
            name: "_h0".to_string(),
            params: vec!["x".to_string()],
            param_types: vec!["Int".to_string()],
            body: binop(BinaryOpKind::Add, int_var("x"), int_const(1)),
            ret_type: "Int".to_string(),
            body_cost: 5,
            call_cost: 4,
            use_count: 0,
        }];

        let candidates = vec![
            Expr::Call("_h0".to_string(), vec![int_var("x")], None),
            Expr::BinaryOp(
                BinaryOpKind::Add,
                Box::new(Expr::Call("_h0".to_string(), vec![int_var("x")], None)),
                Box::new(Expr::Call("_h0".to_string(), vec![int_var("y")], None)),
            ),
        ];

        let params = vec!["x".to_string(), "y".to_string()];
        count_helper_uses(&mut helpers, &candidates, &params);

        // _h0 is called 3 times across both candidates
        assert_eq!(helpers[0].use_count, 3);
    }
}
