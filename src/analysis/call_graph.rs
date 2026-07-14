use crate::ast::{BinaryOpKind, Expr, Statement, TopLevel, UnaryOpKind};
use std::collections::{HashMap, HashSet};

/// Call graph for transaction-to-transaction calls.
///
/// Maps each transaction name to the set of transaction names it calls.
/// This is a directed graph where edges represent "calls during execution."
///
/// Backends query `has_cycle()` to decide codegen strategy:
/// - Acyclic: can use static dispatch, no recursion guards
/// - Cyclic: must use dynamic dispatch, recursion guards, or bounded execution
#[derive(Clone)]
pub struct CallGraph {
    graph: HashMap<String, Vec<String>>,
    txn_names: HashSet<String>,
    cycles: Vec<Vec<String>>,
}

impl CallGraph {
    pub fn new() -> Self {
        CallGraph {
            graph: HashMap::new(),
            txn_names: HashSet::new(),
            cycles: Vec::new(),
        }
    }

    /// Build call graph from top-level items, analyzing all transactions
    pub fn build_from_program(&mut self, items: &[TopLevel]) {
        self.graph.clear();
        self.txn_names.clear();
        self.cycles.clear();

        for item in items {
            if let TopLevel::Transaction(txn) = item {
                self.txn_names.insert(txn.name.clone());
                let called = extract_called_transactions(&txn.body);
                self.graph.entry(txn.name.clone()).or_default().extend(called);
            }
        }
    }

    /// Returns true if the call graph contains any cycles
    pub fn has_cycle(&self) -> bool {
        if !self.cycles.is_empty() {
            return true;
        }
        for txn_name in &self.txn_names {
            let mut visited = HashSet::new();
            let mut path = Vec::new();
            if detect_cycle(txn_name, &self.graph, &mut visited, &mut path) {
                return true;
            }
        }
        false
    }

    /// Detect and collect all cycles in the call graph
    pub fn find_all_cycles(&mut self) -> &[Vec<String>] {
        self.cycles.clear();
        for txn_name in &self.txn_names {
            let mut visited = HashSet::new();
            let mut path = Vec::new();
            if detect_cycle(txn_name, &self.graph, &mut visited, &mut path) {
                self.cycles.push(path.clone());
            }
        }
        &self.cycles
    }

    /// Get the adjacency list for a given transaction
    pub fn edges_from(&self, txn_name: &str) -> Option<&Vec<String>> {
        self.graph.get(txn_name)
    }

    /// Total number of transactions in the graph
    pub fn node_count(&self) -> usize {
        self.txn_names.len()
    }

    /// Total number of edges (calls between transactions)
    pub fn edge_count(&self) -> usize {
        self.graph.values().map(|v| v.len()).sum()
    }
}

/// Extract all transaction names called from a list of statements
pub fn extract_called_transactions(body: &[Statement]) -> Vec<String> {
    let mut called = Vec::new();
    for stmt in body {
        match stmt {
            Statement::Assign(_, expr) => {
                collect_call_names(expr, &mut called);
            }
            Statement::Let { expr, .. } => {
                if let Some(e) = expr {
                    collect_call_names(e, &mut called);
                }
            }
            Statement::Expression(e) => {
                collect_call_names(e, &mut called);
            }
            Statement::Guarded(_, statements) => {
                called.extend(extract_called_transactions(statements));
            }
            _ => {}
        }
    }
    called
}

/// Recursively collect function/transaction call names from expressions
pub fn collect_call_names(expr: &Expr, called: &mut Vec<String>) {
    match expr {
        Expr::Call(name, args) => {
            called.push(name.clone());
            for arg in args {
                collect_call_names(arg, called);
            }
        }
        Expr::BinaryOp(BinaryOpKind::Add, l, r)
        | Expr::BinaryOp(BinaryOpKind::Sub, l, r)
        | Expr::BinaryOp(BinaryOpKind::Mul, l, r)
        | Expr::BinaryOp(BinaryOpKind::Div, l, r)
        | Expr::BinaryOp(BinaryOpKind::Mod, l, r) => {
            collect_call_names(l, called);
            collect_call_names(r, called);
        }
        Expr::BinaryOp(BinaryOpKind::Eq, l, r)
        | Expr::BinaryOp(BinaryOpKind::Neq, l, r)
        | Expr::BinaryOp(BinaryOpKind::Lt, l, r)
        | Expr::BinaryOp(BinaryOpKind::Le, l, r)
        | Expr::BinaryOp(BinaryOpKind::Gt, l, r)
        | Expr::BinaryOp(BinaryOpKind::Ge, l, r) => {
            collect_call_names(l, called);
            collect_call_names(r, called);
        }
        Expr::BinaryOp(BinaryOpKind::And, l, r) | Expr::BinaryOp(BinaryOpKind::Or, l, r) => {
            collect_call_names(l, called);
            collect_call_names(r, called);
        }
        Expr::UnaryOp(UnaryOpKind::Not, e) | Expr::UnaryOp(UnaryOpKind::Neg, e) | Expr::UnaryOp(UnaryOpKind::BitNot, e) => {
            collect_call_names(e, called);
        }
        Expr::Field(e, _) => {
            collect_call_names(e, called);
        }
        Expr::List(elems) => {
            for elem in elems {
                collect_call_names(elem, called);
            }
        }
        _ => {}
    }
}

/// DFS-based cycle detection in a directed graph
///
/// Returns true if a cycle is reachable from `node`.
/// `visited` tracks already-explored nodes (no need to re-visit).
/// `path` tracks the current traversal path for cycle reporting.
pub fn detect_cycle(
    node: &str,
    graph: &HashMap<String, Vec<String>>,
    visited: &mut HashSet<String>,
    path: &mut Vec<String>,
) -> bool {
    let node_str = node.to_string();
    if path.iter().any(|n| *n == node_str) {
        if let Some(pos) = path.iter().position(|n| *n == node_str) {
            let cycle_start = pos;
            path.push(node_str.clone());
            for i in cycle_start..path.len() {
                if path[i] == node_str && i > cycle_start {
                    return true;
                }
            }
            path.pop();
        }
        return true;
    }
    if visited.contains(node) {
        return false;
    }
    visited.insert(node.to_string());
    path.push(node.to_string());
    if let Some(edges) = graph.get(node) {
        for next in edges {
            if detect_cycle(next, graph, visited, path) {
                return true;
            }
        }
    }
    path.pop();
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::*;

    fn make_txn(name: &str, body: Vec<Statement>) -> TopLevel {
        TopLevel::Transaction(Transaction {
            name: name.to_string(),
            body,
            contract: Contract {
                pre_condition: Expr::Bool(true),
                post_condition: Expr::Bool(true),
                is_entry: false,
                watchdog: None,
                span: None,
            },
            is_async: false,
            is_reactive: false,
            type_params: vec![],
            parameters: vec![],
            outputs: vec![],
            output_type: None,
            span: None,
            metadata: HashMap::new(),
            modifiers: vec![],
            derivation: None,
        })
    }

    fn make_call(name: &str) -> Expr {
        Expr::Call(name.to_string(), vec![])
    }

    fn make_guarded(stmts: Vec<Statement>) -> Statement {
        Statement::Guarded(Expr::Bool(true), stmts)
    }

    #[test]
    fn test_empty_graph_is_acyclic() {
        let items: Vec<TopLevel> = vec![];
        let mut cg = CallGraph::new();
        cg.build_from_program(&items);
        assert!(!cg.has_cycle());
    }

    #[test]
    fn test_single_txn_no_calls_is_acyclic() {
        let items = vec![make_txn("a", vec![])];
        let mut cg = CallGraph::new();
        cg.build_from_program(&items);
        assert!(!cg.has_cycle());
    }

    #[test]
    fn test_direct_cycle() {
        let items = vec![make_txn("a", vec![Statement::Expression(make_call("a"))])];
        let mut cg = CallGraph::new();
        cg.build_from_program(&items);
        assert!(cg.has_cycle());
    }

    #[test]
    fn test_indirect_cycle() {
        let items = vec![
            make_txn("a", vec![Statement::Expression(make_call("b"))]),
            make_txn("b", vec![Statement::Expression(make_call("c"))]),
            make_txn("c", vec![Statement::Expression(make_call("a"))]),
        ];
        let mut cg = CallGraph::new();
        cg.build_from_program(&items);
        assert!(cg.has_cycle());
    }

    #[test]
    fn test_acyclic_chain() {
        let items = vec![
            make_txn("a", vec![Statement::Expression(make_call("b"))]),
            make_txn("b", vec![Statement::Expression(make_call("c"))]),
            make_txn("c", vec![]),
        ];
        let mut cg = CallGraph::new();
        cg.build_from_program(&items);
        assert!(!cg.has_cycle());
    }

    #[test]
    fn test_cycle_within_guard() {
        let items = vec![
            make_txn("a", vec![make_guarded(vec![Statement::Expression(make_call("a"))])]),
        ];
        let mut cg = CallGraph::new();
        cg.build_from_program(&items);
        assert!(cg.has_cycle());
    }

    #[test]
    fn test_node_count_and_edges() {
        let items = vec![
            make_txn("a", vec![Statement::Expression(make_call("b"))]),
            make_txn("b", vec![Statement::Expression(make_call("c"))]),
            make_txn("c", vec![]),
        ];
        let mut cg = CallGraph::new();
        cg.build_from_program(&items);
        assert_eq!(cg.node_count(), 3);
        assert_eq!(cg.edge_count(), 2);
    }
}