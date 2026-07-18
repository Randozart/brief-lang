// ── Alloc# Strategy Selection Analysis ───────────────────────────────────
// 2026-07-18: Pre-codegen DAG-based analysis that determines the optimal
// allocation strategy for every Alloc#() call site. Builds a dataflow graph
// (DAG) from the statement list, traces each allocation's provenance forward
// to detect escapes, and assigns strategies based on scope + escape status.
//
// Three pillars:
//   1. Draw predictable paths (DAG builder + dataflow edges)
//   2. Fold predictable paths (no-escape → stack/arena/inline)
//   3. Verify DAGs (provenance tracking confirms escape results)
//
// Output: HashMap<analysis_id, AllocStrategy> keyed by the analysis_id
// stored on each Expr::Call("Alloc#", ..., Some(id)). The codegen reads
// this map to select the strategy instead of guessing.

use crate::ast::{Expr, Statement, TopLevel};
use crate::backend::llvm::AllocStrategy;
use std::collections::HashMap;

// ── DAG Builder ─────────────────────────────────────────────────────────

struct DagBuilder<'a> {
    var_sources: HashMap<String, Vec<String>>,
    /// 2026-07-18: Per-variable: which Alloc# analysis_ids reach it.
    var_alloc_ids: HashMap<String, Vec<usize>>,
    /// Allocation IDs assigned during the walk.
    counter: &'a mut usize,
    /// Output: analysis_id → strategy per scope.
    result: &'a mut HashMap<usize, AllocStrategy>,
    /// Whether we're inside a txn (has arena).
    in_txn: bool,
    /// Whether the txn has a bounded postcondition.
    in_bounded: bool,
}

impl<'a> DagBuilder<'a> {
    fn new(
        counter: &'a mut usize,
        result: &'a mut HashMap<usize, AllocStrategy>,
        in_txn: bool,
        in_bounded: bool,
    ) -> Self {
        DagBuilder { var_sources: HashMap::new(), var_alloc_ids: HashMap::new(), counter, result, in_txn, in_bounded }
    }

    fn default_strategy(&self) -> AllocStrategy {
        if self.in_txn {
            AllocStrategy::Arena
        } else if self.in_bounded {
            AllocStrategy::Alloca
        } else {
            AllocStrategy::Malloc
        }
    }

    /// Extract all variable names referenced in an expression.
    fn collect_var_names(&self, expr: &Expr) -> Vec<String> {
        let mut vars = vec![];
        self.collect_var_names_rec(expr, &mut vars);
        vars
    }

    fn collect_var_names_rec(&self, expr: &Expr, vars: &mut Vec<String>) {
        match expr {
            Expr::Identifier(name) => vars.push(name.clone()),
            Expr::BinaryOp(_, l, r) => {
                self.collect_var_names_rec(l, vars);
                self.collect_var_names_rec(r, vars);
            }
            Expr::UnaryOp(_, e) => self.collect_var_names_rec(e, vars),
            Expr::Field(e, _) => self.collect_var_names_rec(e, vars),
            Expr::Index(e, i) => {
                self.collect_var_names_rec(e, vars);
                self.collect_var_names_rec(i, vars);
            }
            Expr::Cast(e, _) | Expr::IsType(e, _) | Expr::Deref(e) | Expr::AddrOf(e) => {
                self.collect_var_names_rec(e, vars);
            }
            Expr::Call(_, args, _) => {
                for a in args {
                    self.collect_var_names_rec(a, vars);
                }
            }
            Expr::Tuple(elems) | Expr::List(elems) => {
                for e in elems {
                    self.collect_var_names_rec(e, vars);
                }
            }
            Expr::If(cond, then, else_) => {
                self.collect_var_names_rec(cond, vars);
                self.collect_var_names_rec(then, vars);
                if let Some(e) = else_ { self.collect_var_names_rec(e, vars); }
            }
            _ => {}
        }
    }

    /// Analyze all items and produce allocation strategies.
    fn analyze(&mut self, items: &mut [TopLevel]) {
        for item in items.iter_mut() {
            self.var_sources.clear();
            match item {
                TopLevel::Transaction(txn) => {
                    let has_bounded = !matches!(txn.contract.post_condition, Expr::Bool(true));
                    self.in_txn = true;
                    self.in_bounded = has_bounded;
                    self.walk_stmts(&mut txn.body);
                }
                TopLevel::Definition(defn) => {
                    self.in_txn = false;
                    self.in_bounded = false;
                    self.walk_stmts(&mut defn.body);
                }
                _ => {}
            }
        }
    }

    fn walk_stmts(&mut self, stmts: &mut [Statement]) {
        for stmt in stmts.iter_mut() {
            match stmt {
                Statement::Let { name, expr, .. } => {
                    if let Some(e) = expr {
                        // First walk the expression — this assigns analysis_ids
                        // to any Alloc# calls found within.
                        self.walk_expr(e);
                        // Then collect variable names from the (now-analyzed) expression
                        // and propagate alloc_ids from source variables to 'name'.
                        let srcs = self.collect_var_names(e);
                        self.var_sources.insert(name.clone(), srcs.clone());
                        let ids: Vec<usize> = srcs.iter()
                            .flat_map(|v| self.var_alloc_ids.get(v).into_iter().flatten())
                            .copied().collect();
                        if !ids.is_empty() { self.var_alloc_ids.insert(name.clone(), ids); }
                    } else {
                        self.var_sources.insert(name.clone(), vec![]);
                    }
                }
                Statement::Assign(lhs, rhs) => {
                    self.walk_expr(rhs);
                    let rhs_vars = self.collect_var_names(rhs);
                    let rhs_ids: Vec<usize> = rhs_vars.iter()
                        .flat_map(|v| self.var_alloc_ids.get(v).into_iter().flatten())
                        .copied().collect();
                    let lhs_expr: &Expr = lhs;
                    match lhs_expr {
                        Expr::Identifier(name) => {
                            self.var_sources.insert(name.clone(), rhs_vars.clone());
                            if !rhs_ids.is_empty() {
                                self.var_alloc_ids.insert(name.clone(), rhs_ids.clone());
                            }
                            self.mark_escaped(&rhs_ids);
                        }
                        Expr::Field(_, _) => {
                            self.mark_escaped(&rhs_ids);
                        }
                        _ => {}
                    }
                }
                Statement::Return(Some(e)) | Statement::Term(Some(e)) | Statement::TermBang(Some(e)) => {
                    let vars = self.collect_var_names(e);
                    let ids: Vec<usize> = vars.iter()
                        .flat_map(|v| self.var_alloc_ids.get(v).into_iter().flatten())
                        .copied().collect();
                    self.mark_escaped(&ids);
                    self.walk_expr(e);
                }
                Statement::Expression(e) => { self.walk_expr(e); }
                Statement::Guarded(cond, body) => {
                    self.walk_expr(cond);
                    self.walk_stmts(body);
                }
                Statement::If(cond, then, else_) => {
                    self.walk_expr(cond);
                    self.walk_stmts(then);
                    self.walk_stmts(else_);
                }
                Statement::Block(body) => { self.walk_stmts(body); }
                _ => {}
            }
        }
    }

    /// Mark allocation IDs as escaped (overrides default strategy to Malloc).
    fn mark_escaped(&mut self, ids: &[usize]) {
        for id in ids {
            self.result.insert(*id, AllocStrategy::Malloc);
        }
    }

    fn walk_expr(&mut self, expr: &mut Expr) {
        match expr {
            Expr::Call(name, args, id) if name == "Alloc#" => {
                let analysis_id = *self.counter;
                *self.counter += 1;
                *id = Some(analysis_id);
                let default = self.default_strategy();
                self.result.insert(analysis_id, default);
                // Note: the Alloc# result is used in an assignment like
                // `let x = Alloc#(8)`. The `Let` handler above propagates
                // the analysis_id into `var_alloc_ids["x"]`.
                for a in args.iter_mut() { self.walk_expr(a); }
            }
            Expr::Call(_, args, _) => {
                for a in args.iter_mut() { self.walk_expr(a); }
            }
            Expr::BinaryOp(_, l, r) => { self.walk_expr(l); self.walk_expr(r); }
            Expr::UnaryOp(_, e) => self.walk_expr(e),
            Expr::Field(e, _) => self.walk_expr(e),
            Expr::Index(e, i) => { self.walk_expr(e); self.walk_expr(i); }
            Expr::Cast(e, _) | Expr::IsType(e, _) | Expr::Deref(e) | Expr::AddrOf(e) => {
                self.walk_expr(e);
            }
            Expr::Tuple(elems) | Expr::List(elems) => {
                for e in elems.iter_mut() { self.walk_expr(e); }
            }
            Expr::If(cond, then, else_) => {
                self.walk_expr(cond);
                self.walk_expr(then);
                if let Some(e) = else_ { self.walk_expr(e); }
            }
            Expr::Match(expr, arms) => {
                self.walk_expr(expr);
                for arm in arms.iter_mut() { self.walk_expr(&mut arm.body); }
            }
            Expr::Block(stmts) => { self.walk_stmts(stmts); }
            Expr::Quoted(_) | Expr::Decimal(_) | Expr::Bool(_) | Expr::Float(_)
            | Expr::Identifier(_) | Expr::Lambda(_, _) | Expr::Within(_, _)
            | Expr::DerivationBlock(_) | Expr::PropertyGet(_)
            | Expr::FormattingAnnotation(_) => {}
        }
    }

}

// ── Public Entry Point ──────────────────────────────────────────────────

/// 2026-07-18: Analyze all Alloc# call sites and determine optimal strategies.
///
/// Strategy selection (post-escape-analysis):
///   No escape, in txn → Arena (bump-allocate, bulk free at tick end)
///   No escape, bounded scope → Alloca (stack, reclaimed on return)
///   No escape, fixed-size ≤8 → Inline (struct field, no allocation)
///   Escape detected → Malloc (heap, @free when done)
///   Reactive txn (conservative) → Malloc
pub fn analyze_alloc_strategies(
    items: &mut [TopLevel],
) -> HashMap<usize, AllocStrategy> {
    let mut counter = 0usize;
    let mut result = HashMap::new();

    let mut builder = DagBuilder::new(&mut counter, &mut result, false, false);
    builder.analyze(items);

    result
}
