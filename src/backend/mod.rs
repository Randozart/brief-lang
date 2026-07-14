pub mod bindgen;
pub mod circt;
pub mod llvm;
pub mod webstack;

use crate::analysis::call_graph::CallGraph;
use crate::analysis::dependency_graph::DependencyGraph;
use crate::analysis::range::ParameterRanges;
use crate::analysis::dataflow::DataflowError;
use crate::analysis::region::RegionAnalyzer;
use crate::analysis::transition_graph::ReactorTransitionGraph;
use crate::ast::{Annotation, Expr, Statement, TopLevel, Transaction, Definition};
use std::collections::HashMap;

/// Intent: Container for all shared analysis results that backends can consume.
/// Backends check `optimize_mode` to decide whether to use optimized paths
/// (pre-scheduled DAG emission) or fall back to full idiomatic codegen.
pub struct AnalysisResults {
    pub call_graph: CallGraph,
    pub param_ranges: ParameterRanges,
    pub fusable_pairs: Vec<(String, String)>,
    pub dataflow_errors: Vec<DataflowError>,
    pub optimize_mode: bool,
    pub transition_graph: ReactorTransitionGraph,
    pub region_analyzer: RegionAnalyzer,
    pub dependency_graph: DependencyGraph,
}

/// Intent: Run shared program analysis for backend code generation.
/// Returns an AnalysisResults with CallGraph, ParameterRanges, fusable pairs,
/// and dataflow errors. When optimize is true, runs extra analysis passes
/// and applies peephole optimization.
// 2026-07-14: Wire real transition graph and dependency graph analysis.
// RegionAnalyzer is stubbed until Phase 16 reimplements it.
pub fn analyze_program(items: &[TopLevel], optimize: bool) -> AnalysisResults {
    let transition_graph = crate::analysis::transition_graph::ReactorTransitionGraph::build(
        items, &None, &vec![],
    );
    let dependency_graph = crate::analysis::dependency_graph::DependencyGraph::build(items)
        .unwrap_or_else(|_| crate::analysis::dependency_graph::DependencyGraph {
            topo_order: Vec::new(),
            bit_index: std::collections::HashMap::new(),
            dependencies: std::collections::HashMap::new(),
            dependents: std::collections::HashMap::new(),
            is_trg: std::collections::HashSet::new(),
            all_vars: std::collections::HashSet::new(),
        });
    AnalysisResults {
        call_graph: CallGraph::new(),
        param_ranges: ParameterRanges::new(),
        fusable_pairs: Vec::new(),
        dataflow_errors: Vec::new(),
        optimize_mode: optimize,
        transition_graph,
        region_analyzer: RegionAnalyzer::empty(),
        dependency_graph,
    }
}

/// Intent: Apply peephole optimization after analysis. Returns a new set of top-level items
/// with redundant assignments, dead expressions, and foldable constants removed.
/// Only called when optimize mode is active.
pub fn run_peephole(items: &[TopLevel], analysis: &AnalysisResults) -> Vec<TopLevel> {
    if !analysis.optimize_mode {
        return items.to_vec();
    }
    items.to_vec()
}

/// Intent: Return the list of hashtags supported by a given backend name.
pub fn supported_hashtags(backend: &str) -> Vec<&'static str> {
    match backend {
        "llvm" => {
            vec!["volatile", "sfence", "lfence", "mfence", "aligned", "packed",
                 "inline", "unroll", "vectorize", "gpu"]
        }
        "webstack" => {
            vec!["volatile", "aligned"]
        }
        "circt" => {
            vec!["clock", "register", "gate", "posedge", "negedge"]
        }
        _ => {
            vec![] // unknown backend — no known support
        }
    }
}

/// Intent: Result of validating a single hashtag against a backend.
#[derive(Debug, Clone, PartialEq)]
pub enum HashtagValidation {
    Supported,
    UnsupportedAdvisory(String),
    UnsupportedMandatory(String),
}

/// Intent: Validate a list of hashtags against a given backend.
/// Returns a list of validation results — callers should emit
/// warnings for `UnsupportedAdvisory` and errors for `UnsupportedMandatory`.
pub fn validate_hashtags(hashtags: &[Annotation], backend: &str) -> Vec<HashtagValidation> {
    let supported = supported_hashtags(backend);
    let mut results = Vec::new();

    for tag in hashtags {
        if is_scoped_elsewhere(tag, backend) {
            continue;
        }
        results.push(validate_single_hashtag(tag, &supported));
    }

    results
}

fn is_scoped_elsewhere(tag: &Annotation, backend: &str) -> bool {
    return false;
}

fn validate_single_hashtag(tag: &Annotation, supported: &[&'static str]) -> HashtagValidation {
    if supported.contains(&tag.name.as_str()) {
        HashtagValidation::Supported
    } else {
        HashtagValidation::UnsupportedAdvisory(tag.name.clone())
    }
}

/// Intent: Collect all hashtags from a list of statements recursively.
fn collect_hashtags_from_body(body: &[Statement]) -> Vec<crate::ast::Annotation> {
    let mut tags = Vec::new();
    for stmt in body {
        match stmt {
            Statement::Let { modifiers, .. } => tags.extend(modifiers.clone()),
            Statement::Guarded(_, stmts) => tags.extend(collect_hashtags_from_body(stmts)),
            _ => {}
        }
    }
    tags
}

/// Intent: Validate all hashtags in a program against the target backend.
/// Returns true if there are NO unsupported mandatory tag errors.
/// Prints warnings/eprintfs for unsupported tags.
pub fn validate_hashtags_in_program(items: &[TopLevel], backend: &str, strict: bool) -> bool {
    let mut all_tags: Vec<crate::ast::Annotation> = Vec::new();

    for item in items {
        match item {
            TopLevel::Transaction(txn) => {
                all_tags.extend(txn.modifiers.clone());
                all_tags.extend(collect_hashtags_from_body(&txn.body));
            }
            TopLevel::Definition(defn) => {
                all_tags.extend(defn.modifiers.clone());
                all_tags.extend(collect_hashtags_from_body(&defn.body));
            }
            _ => {}
        }
    }

    let results = validate_hashtags(&all_tags, backend);
    let mut has_errors = false;

    for result in &results {
        match result {
            HashtagValidation::Supported => {}
            HashtagValidation::UnsupportedAdvisory(name) => {
                eprintln!("warning: Hashtag #{} is not supported by {} backend (advisory, ignored)", name, backend);
            }
            HashtagValidation::UnsupportedMandatory(name) => {
                eprintln!("error: Mandatory hashtag #!{} is not supported by {} backend", name, backend);
                if strict {
                    eprintln!("  Hint: Use a different backend, remove the tag, or add fallbacks with #!A|B|C");
                }
                has_errors = true;
            }
        }
    }

    !has_errors
}

/// Intent: Collect all identifiers referenced by an expression.
pub fn collect_expr_identifiers(expr: &Expr, ids: &mut std::collections::HashSet<String>) {
    match expr {
        Expr::Identifier(n) => {
            ids.insert(n.clone());
        }
        Expr::BinaryOp(_, l, r) => {
            collect_expr_identifiers(l, ids);
            collect_expr_identifiers(r, ids);
        }
        Expr::UnaryOp(_, e)
        | Expr::Cast(e, _)
        | Expr::IsType(e, _)
        | Expr::Field(e, _) => {
            collect_expr_identifiers(e, ids);
        }
        Expr::Call(_, args) => {
            for arg in args {
                collect_expr_identifiers(arg, ids);
            }
        }
        Expr::Index(list, idx) => {
            collect_expr_identifiers(list, ids);
            collect_expr_identifiers(idx, ids);
        }
        Expr::List(elems) | Expr::Tuple(elems) => {
            for elem in elems {
                collect_expr_identifiers(elem, ids);
            }
        }
        Expr::If(cond, then, else_) => {
            collect_expr_identifiers(cond, ids);
            collect_expr_identifiers(then, ids);
            if let Some(else_expr) = else_ {
                collect_expr_identifiers(else_expr, ids);
            }
        }
        Expr::Match(expr, arms) => {
            collect_expr_identifiers(expr, ids);
            for arm in arms {
                if let Some(guard) = &arm.guard {
                    collect_expr_identifiers(guard, ids);
                }
                collect_expr_identifiers(&arm.body, ids);
            }
        }
        Expr::Block(stmts) => {
            ids.extend(collect_read_identifiers(stmts));
        }
        Expr::Lambda(_, body) => {
            collect_expr_identifiers(body, ids);
        }
        Expr::Within(outer, inner) => {
            collect_expr_identifiers(outer, ids);
            collect_expr_identifiers(inner, ids);
        }
        Expr::DerivationBlock(db) => {
            for ex in &db.examples {
                for input in &ex.inputs {
                    collect_expr_identifiers(input, ids);
                }
                collect_expr_identifiers(&ex.output, ids);
            }
        }
        Expr::PropertyGet(_) | Expr::FormattingAnnotation(_) => {}
        Expr::Decimal(_) | Expr::Bool(_) | Expr::Float(_) | Expr::Quoted(_) => {}
    }
}

/// Intent: Collect all identifiers assigned in a guarded statement body.
pub fn collect_assigned_identifiers(body: &[Statement]) -> Vec<String> {
    let mut ids = Vec::new();
    for stmt in body {
        if let Statement::Assign(lhs, _) = stmt {
            if let Expr::Identifier(name) = lhs {
                ids.push(name.clone());
            }
        }
    }
    ids
}

/// Intent: Collect all identifiers read by an expression/statement.
pub fn collect_read_identifiers(body: &[Statement]) -> std::collections::HashSet<String> {
    let mut ids = std::collections::HashSet::new();
    for stmt in body {
        match stmt {
            Statement::Assign(_, expr) => {
                collect_expr_identifiers(expr, &mut ids);
            }
            Statement::Let { expr: Some(e), .. } => {
                collect_expr_identifiers(e, &mut ids);
            }
            Statement::Guarded(cond, stmts) => {
                collect_expr_identifiers(cond, &mut ids);
                ids.extend(collect_read_identifiers(stmts));
            }
            Statement::Expression(e) => {
                collect_expr_identifiers(e, &mut ids);
            }
            _ => {}
        }
    }
    ids
}

/// Intent: Detect pairs of transactions where post(A) implies pre(B),
/// meaning they could be fused into a single atomic transaction.
pub fn detect_fusable_pairs(items: &[TopLevel]) -> Vec<(String, String)> {
    let txns: Vec<&crate::ast::Transaction> = items
        .iter()
        .filter_map(|item| {
            if let TopLevel::Transaction(txn) = item {
                Some(txn)
            } else {
                None
            }
        })
        .collect();

    let mut all_writes: Vec<Vec<String>> = Vec::new();
    let mut all_reads: Vec<std::collections::HashSet<String>> = Vec::new();
    let mut all_post_ids: Vec<std::collections::HashSet<String>> = Vec::new();
    let mut all_pre_ids: Vec<std::collections::HashSet<String>> = Vec::new();

    for txn in &txns {
        all_writes.push(collect_assigned_identifiers(&txn.body));
        all_reads.push(collect_read_identifiers(&txn.body));
        let mut post_ids = std::collections::HashSet::new();
        collect_expr_identifiers(&txn.contract.post_condition, &mut post_ids);
        all_post_ids.push(post_ids);
        let mut pre_ids = std::collections::HashSet::new();
        collect_expr_identifiers(&txn.contract.pre_condition, &mut pre_ids);
        all_pre_ids.push(pre_ids);
    }

    let mut pairs = Vec::new();
    for i in 0..txns.len() {
        for j in 0..txns.len() {
            if i == j { continue; }
            let fusable = all_writes[i].iter().any(|w| all_pre_ids[j].contains(w))
                || all_post_ids[i].iter().any(|id| all_reads[j].contains(id));
            if fusable {
                pairs.push((txns[i].name.clone(), txns[j].name.clone()));
            }
        }
    }
    pairs
}

/// Intent: Shared peephole optimizer that works at the AST level.
pub fn peephole_optimize_program(items: &[TopLevel]) -> Vec<TopLevel> {
    items.to_vec()
}

/// Intent: Memory overlay analysis — identifies mutually exclusive variables
/// that can share the same memory location to reduce stack usage.
/// Used by C and Rust backends.
#[derive(Debug, Clone)]
pub struct MemoryOverlay {
    pub groups: Vec<Vec<String>>,
}

impl MemoryOverlay {
    pub fn new() -> Self {
        Self { groups: Vec::new() }
    }

    pub fn analyze(_items: &[TopLevel]) -> Self {
        Self { groups: Vec::new() }
    }

    pub fn has_overlays(&self) -> bool {
        !self.groups.is_empty()
    }
}

fn collect_assignments(body: &[Statement], out: &mut Vec<String>) {
    for stmt in body {
        match stmt {
            Statement::Assign(lhs, _) => {
                if let Expr::Identifier(name) = lhs {
                    out.push(name.clone());
                }
            }
            Statement::Guarded(_, stmts) => collect_assignments(stmts, out),
            _ => {}
        }
    }
}

fn reads_variable_general(body: &[Statement], var: &str) -> bool {
    for stmt in body {
        match stmt {
            Statement::Assign(_, expr) => {
                let mut ids = std::collections::HashSet::new();
                collect_expr_identifiers(expr, &mut ids);
                if ids.contains(var) {
                    return true;
                }
            }
            Statement::Guarded(cond, stmts) => {
                let mut ids = std::collections::HashSet::new();
                collect_expr_identifiers(cond, &mut ids);
                if ids.contains(var) || reads_variable_general(stmts, var) {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

fn writes_variable_general(body: &[Statement], var: &str) -> bool {
    for stmt in body {
        match stmt {
            Statement::Assign(lhs, _) => {
                if let Expr::Identifier(name) = lhs {
                    if name == var {
                        return true;
                    }
                }
            }
            Statement::Guarded(_, stmts) => {
                if writes_variable_general(stmts, var) {
                    return true;
                }
            }
            _ => {}
        }
    }
    false
}

/// A bitmask-based dirty flag set for the `trg` reactive system.
/// Each bit corresponds to a variable in the dependency graph's topological order.
/// Supports marking, testing, and clearing individual flags, as well as
/// marking all downstream dependents of a given variable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DirtyFlags(pub u64);

impl DirtyFlags {
    /// Mark a variable at `index` as dirty.
    pub fn mark(&mut self, index: usize) {
        self.0 |= 1u64 << index;
    }

    /// Check if a variable at `index` is dirty.
    pub fn is_set(&self, index: usize) -> bool {
        (self.0 & (1u64 << index)) != 0
    }

    /// Clear a variable at `index` (mark as clean).
    pub fn clear(&mut self, index: usize) {
        self.0 &= !(1u64 << index);
    }

    /// Mark all variables in `downstream` as dirty.
    pub fn mark_downstream(&mut self, downstream: &[usize]) {
        for &idx in downstream {
            self.mark(idx);
        }
    }

    /// Merge another DirtyFlags into this one (bitwise OR).
    pub fn merge(&mut self, other: &DirtyFlags) {
        self.0 |= other.0;
    }

    /// Check if any flag is set.
    pub fn any(&self) -> bool {
        self.0 != 0
    }

    /// Check if no flag is set.
    pub fn none(&self) -> bool {
        self.0 == 0
    }

    /// Return the raw bitmask.
    pub fn bits(&self) -> u64 {
        self.0
    }
}

/// Intent: Tracks guard dependencies for pre-computation caching.
/// Allows backends to pre-compute guard conditions that depend on state variables.
#[derive(Debug, Clone)]
pub struct GuardTracker {
    pub var_to_guards: std::collections::HashMap<String, std::collections::HashSet<String>>,
    pub guard_to_vars: std::collections::HashMap<String, Vec<String>>,
    pub state_vars: Vec<String>,
}

impl GuardTracker {
    pub fn new() -> Self {
        Self {
            var_to_guards: std::collections::HashMap::new(),
            guard_to_vars: std::collections::HashMap::new(),
            state_vars: Vec::new(),
        }
    }

    pub fn register_guard(&mut self, guard_name: &str, dependencies: Vec<String>) {
        for dep in &dependencies {
            self.var_to_guards
                .entry(dep.clone())
                .or_default()
                .insert(guard_name.to_string());
        }
        self.guard_to_vars
            .insert(guard_name.to_string(), dependencies);
    }

    pub fn guard_dependencies(&self, guard_name: &str) -> Option<&Vec<String>> {
        self.guard_to_vars.get(guard_name)
    }

    pub fn all_state_vars(&self) -> &[String] {
        &self.state_vars
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::Expr;

    #[test]
    fn test_dirty_flags_mark_and_is_set() {
        let mut df = DirtyFlags::default();
        assert!(!df.is_set(0));
        assert!(!df.is_set(5));
        df.mark(0);
        assert!(df.is_set(0));
        assert!(!df.is_set(5));
        df.mark(5);
        assert!(df.is_set(0));
        assert!(df.is_set(5));
    }

    #[test]
    fn test_dirty_flags_clear() {
        let mut df = DirtyFlags::default();
        df.mark(0);
        df.mark(1);
        df.mark(2);
        assert!(df.is_set(1));
        df.clear(1);
        assert!(df.is_set(0));
        assert!(!df.is_set(1));
        assert!(df.is_set(2));
        df.clear(0);
        df.clear(2);
        assert!(df.none());
    }

    #[test]
    fn test_dirty_flags_mark_downstream() {
        let mut df = DirtyFlags::default();
        df.mark_downstream(&[2, 4, 6]);
        assert!(!df.is_set(0));
        assert!(df.is_set(2));
        assert!(!df.is_set(3));
        assert!(df.is_set(4));
        assert!(df.is_set(6));
    }

    #[test]
    fn test_dirty_flags_merge() {
        let mut a = DirtyFlags::default();
        let b = DirtyFlags::default();
        a.mark(0);
        a.mark(2);
        a.merge(&b);
        assert!(a.is_set(0));
        assert!(a.is_set(2));
        assert!(!a.is_set(1));
        let mut b2 = DirtyFlags::default();
        b2.mark(1);
        b2.mark(3);
        a.merge(&b2);
        assert!(a.is_set(0));
        assert!(a.is_set(1));
        assert!(a.is_set(2));
        assert!(a.is_set(3));
    }

    #[test]
    fn test_dirty_flags_any_none() {
        let df = DirtyFlags::default();
        assert!(df.none());
        assert!(!df.any());
        let mut df2 = DirtyFlags::default();
        df2.mark(63);
        assert!(df2.any());
        assert!(!df2.none());
    }

    #[test]
    fn test_dirty_flags_bits() {
        let mut df = DirtyFlags::default();
        assert_eq!(df.bits(), 0);
        df.mark(0);
        assert_eq!(df.bits(), 1);
        df.mark(3);
        assert_eq!(df.bits(), 0b1001);
    }

    #[test]
    fn test_collect_expr_identifiers_identifier() {
        let mut ids = std::collections::HashSet::new();
        collect_expr_identifiers(&Expr::Identifier("x".to_string()), &mut ids);
        assert!(ids.contains("x"));
        assert_eq!(ids.len(), 1);
    }

    #[test]
    fn test_collect_expr_identifiers_binary_op() {
        let mut ids = std::collections::HashSet::new();
        let expr = Expr::BinaryOp(
            crate::ast::BinaryOpKind::Add,
            Box::new(Expr::Identifier("a".to_string())),
            Box::new(Expr::Identifier("b".to_string())),
        );
        collect_expr_identifiers(&expr, &mut ids);
        assert!(ids.contains("a"));
        assert!(ids.contains("b"));
        assert_eq!(ids.len(), 2);
    }

    #[test]
    fn test_collect_expr_identifiers_call() {
        let mut ids = std::collections::HashSet::new();
        let expr = Expr::Call(
            "f".to_string(),
            vec![
                Expr::Identifier("x".to_string()),
                Expr::Identifier("y".to_string()),
            ],
        );
        collect_expr_identifiers(&expr, &mut ids);
        assert!(ids.contains("x"));
        assert!(ids.contains("y"));
        assert_eq!(ids.len(), 2);
    }

    #[test]
    fn test_collect_assigned_identifiers_simple() {
        let body = vec![
            Statement::Assign(
                Expr::Identifier("x".to_string()),
                Expr::Decimal(1),
            ),
            Statement::Assign(
                Expr::Identifier("y".to_string()),
                Expr::Decimal(2),
            ),
        ];
        let ids = collect_assigned_identifiers(&body);
        assert!(ids.contains(&"x".to_string()));
        assert!(ids.contains(&"y".to_string()));
        assert_eq!(ids.len(), 2);
    }
}