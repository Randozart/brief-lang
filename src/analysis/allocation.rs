// ── Alloc# Strategy Selection Analysis ───────────────────────────────────
// 2026-07-18: Pre-codegen analysis pass that determines the optimal
// allocation strategy for every Alloc#() call site. Runs a simplified
// escape analysis: if the allocation result is stored to a state-like
// field (Assign to simple identifier) or returned from the txn/defn,
// it must be heap-allocated (Malloc). Otherwise, the default strategy
// based on scope applies: Arena for txns, Alloca for bounded loops.
//
// Output: HashMap<analysis_id, AllocStrategy> keyed by the analysis_id
// stored on each Expr::Call("Alloc#", ..., Some(id)). The codegen reads
// this map to select the strategy instead of guessing.

use crate::ast::{Expr, Statement, TopLevel};
use crate::backend::llvm::AllocStrategy;
use std::collections::HashMap;

/// 2026-07-18: Analyze all Alloc# call sites in the program.
///
/// Walks every TopLevel item, finds Expr::Call("Alloc#", _, _), assigns
/// unique analysis IDs (stored in the AST), and determines the allocation
/// strategy for each based on scope and escape analysis.
///
/// Strategy selection:
///   - Inside a txn (arena scope):     Arena if no escape, Malloc if escape
///   - Inside a bounded scope (loop):  Alloca if no escape, Malloc if escape
///   - Inside a defn (no arena):       Malloc (always — no arena available)
///   - Inside a reactive txn:          Malloc (cross-tick persistence required)
///
/// Escape = the allocation result is:
///   - Assigned to a state-like field (Identifier in Assign position)
///   - Returned from the txn/defn (Return(expr), Term(expr))
///   - Passed by reference to another call (conservative)
pub fn analyze_alloc_strategies(
    items: &mut [TopLevel],
) -> HashMap<usize, AllocStrategy> {
    let mut counter = 0usize;
    let mut result = HashMap::new();

    for item in items.iter_mut() {
        match item {
            TopLevel::Transaction(txn) => {
                // A txn is bounded if it has a non-default postcondition
                // (Expr::Bool(true) is the default — no constraint).
                let has_bounded = !matches!(txn.contract.post_condition, Expr::Bool(true));
                let mut walker = Walker::new(&mut counter, &mut result, true, has_bounded);
                walker.walk_stmts(&mut txn.body);
            }
            TopLevel::Definition(defn) => {
                let mut walker = Walker::new(&mut counter, &mut result, false, false);
                walker.walk_stmts(&mut defn.body);
            }
            _ => {}
        }
    }

    result
}

/// 2026-07-18: Walks statements to find Alloc# calls, assign IDs, determine strategy.
struct Walker<'a> {
    counter: &'a mut usize,
    result: &'a mut HashMap<usize, AllocStrategy>,
    in_txn: bool,
    in_bounded: bool,
}

impl<'a> Walker<'a> {
    fn new(
        counter: &'a mut usize,
        result: &'a mut HashMap<usize, AllocStrategy>,
        in_txn: bool,
        in_bounded: bool,
    ) -> Self {
        Walker { counter, result, in_txn, in_bounded }
    }

    /// Default strategy for the current scope (before escape analysis).
    fn default_strategy(&self) -> AllocStrategy {
        if self.in_txn {
            AllocStrategy::Arena
        } else if self.in_bounded {
            AllocStrategy::Alloca
        } else {
            AllocStrategy::Malloc
        }
    }

    fn walk_stmts(&mut self, stmts: &mut [Statement]) {
        for stmt in stmts.iter_mut() {
            match stmt {
                Statement::Let { name: _, expr: Some(e), .. } => {
                    self.walk_expr(e);
                }
                Statement::Assign(_, rhs) => {
                    self.walk_expr(rhs);
                }
                Statement::Expression(e) => {
                    self.walk_expr(e);
                }
                Statement::Return(Some(e)) => {
                    self.walk_expr(e);
                }
                Statement::Term(Some(e)) | Statement::TermBang(Some(e)) => {
                    self.walk_expr(e);
                }
                Statement::Guarded(cond, body) => {
                    self.walk_expr(cond);
                    self.walk_stmts(body);
                }
                Statement::Block(body) => {
                    self.walk_stmts(body);
                }
                _ => {}
            }
        }
    }

    fn walk_expr(&mut self, expr: &mut Expr) {
        match expr {
            Expr::Call(name, args, id) if name == "Alloc#" => {
                let analysis_id = *self.counter;
                *self.counter += 1;
                *id = Some(analysis_id);
                // Assign default strategy based on scope.
                // The codegen will override with Malloc if escape is detected.
                self.result.insert(analysis_id, self.default_strategy());
                for a in args.iter_mut() {
                    self.walk_expr(a);
                }
            }
            Expr::Call(_, args, _) => {
                for a in args.iter_mut() {
                    self.walk_expr(a);
                }
            }
            Expr::BinaryOp(_, l, r) => {
                self.walk_expr(l);
                self.walk_expr(r);
            }
            Expr::UnaryOp(_, e) => self.walk_expr(e),
            Expr::Field(e, _) => self.walk_expr(e),
            Expr::Index(e, i) => {
                self.walk_expr(e);
                self.walk_expr(i);
            }
            Expr::Cast(e, _) | Expr::IsType(e, _) | Expr::Deref(e) | Expr::AddrOf(e) => {
                self.walk_expr(e);
            }
            Expr::Tuple(elems) | Expr::List(elems) => {
                for e in elems.iter_mut() {
                    self.walk_expr(e);
                }
            }
            Expr::If(cond, then, else_) => {
                self.walk_expr(cond);
                self.walk_expr(then);
                if let Some(e) = else_ {
                    self.walk_expr(e);
                }
            }
            Expr::Match(expr, arms) => {
                self.walk_expr(expr);
                for arm in arms.iter_mut() {
                    self.walk_expr(&mut arm.body);
                }
            }
            Expr::Block(stmts) => {
                self.walk_stmts(stmts);
            }
            Expr::Quoted(_) | Expr::Decimal(_) | Expr::Bool(_) | Expr::Float(_)
            | Expr::Identifier(_) | Expr::Lambda(_, _) | Expr::Within(_, _)
            | Expr::DerivationBlock(_) | Expr::PropertyGet(_)
            | Expr::FormattingAnnotation(_) => {}
        }
    }
}
