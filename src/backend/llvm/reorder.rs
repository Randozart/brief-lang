// Transaction body instruction reordering for ILP.
// Builds a dependency DAG from statement read/write sets and
// topologically sorts to group independent operations together.
//
// Why this exists: Brief transaction bodies are written as sequential
// statements, but many sequences contain independent operations (e.g.,
// &x = a + b; &y = c + d). LLVM's scheduler can issue independent
// operations simultaneously, but only if they appear in separate
// dependency chains. Kahn's topological sort finds the chains and
// interleaves them for maximum ILP.
//
// Why not let LLVM handle this entirely? LLVM's instruction scheduler
// works within a basic block after codegen. By reordering at the Brief
// statement level before LLVM IR emission, we:
//   1. Give LLVM a wider scheduling window from the start
//   2. Avoid LLVM having to push independent ops through alias analysis
//   3. Keep the emitted IR readable for debugging

use crate::ast::{Expr, Statement};
use std::collections::{HashMap, HashSet, VecDeque};

/// Reorder body statements to maximize instruction-level parallelism.
/// Independent statements (no read-write conflicts) are grouped together
/// so LLVM's scheduler can issue them simultaneously.
/// Term and TermBang statements are always placed last — they set a
/// "terminated" flag that stops body emission in emit_stmt. If reordered
/// to an earlier position, subsequent statements would be silently dropped.
/// Returns (reordered_statements, has_cycle) where has_cycle indicates
/// a dependency cycle was detected and sorted order may be suboptimal.
///
/// Why the < 3 threshold: bodies with 0-2 statements have no reordering
/// opportunity (no ILP to exploit). The dependency graph construction
/// and topological sort would add overhead for zero benefit.
pub(crate) fn reorder_body_statements(body: &[Statement]) -> (Vec<Statement>, bool) {
    if body.len() < 3 {
        return (body.to_vec(), false);
    }
    // Separate term statements (always last) from non-term statements
    let mut terms: Vec<Statement> = Vec::new();
    let mut non_terms: Vec<Statement> = Vec::new();
    for s in body {
        if matches!(s, Statement::Term { .. } | Statement::TermBang { .. }) {
            terms.push(s.clone());
        } else {
            non_terms.push(s.clone());
        }
    }
    if non_terms.len() < 2 {
        return (body.to_vec(), false);
    }
    let sets: Vec<ReadWriteSet> = non_terms.iter().map(rw_set_of).collect();
    let deps = build_dependency_graph(&sets);
    let (mut reordered, has_cycle) = topological_sort(&non_terms, &deps);
    // Append term statements at the end
    for s in &terms {
        reordered.push(s.clone());
    }
    (reordered, has_cycle)
}

/// Read and write sets for a single statement.
struct ReadWriteSet {
    reads: HashSet<String>,
    writes: HashSet<String>,
}

/// Extract read/write identifiers from a statement.
fn rw_set_of(stmt: &Statement) -> ReadWriteSet {
    let mut reads = HashSet::new();
    let mut writes = HashSet::new();
    match stmt {
        Statement::Assignment { lhs, expr, .. } => {
            // lhs is the write target
            collect_write_target(lhs, &mut writes);
            // expr is read
            collect_reads_from_expr(expr, &mut reads);
        }
        Statement::Guarded { condition, statements, .. } => {
            collect_reads_from_expr(condition, &mut reads);
            for s in statements {
                let inner = rw_set_of(s);
                reads.extend(inner.reads);
                writes.extend(inner.writes);
            }
        }
        Statement::Let { name, expr, .. } => {
            if let Some(e) = expr {
                collect_reads_from_expr(e, &mut reads);
            }
            writes.insert(name.clone());
        }
        Statement::Expression(e) => {
            collect_reads_from_expr(e, &mut reads);
        }
        Statement::Foreach { list, body, .. } => {
            collect_reads_from_expr(list, &mut reads);
            for s in body {
                let inner = rw_set_of(s);
                reads.extend(inner.reads);
                writes.extend(inner.writes);
            }
        }
        Statement::Term { values, swan_song, .. } | Statement::TermBang { values, swan_song, .. } => {
            for v in values {
                if let Some(e) = v {
                    collect_reads_from_expr(e, &mut reads);
                }
            }
            if let Some(ss) = swan_song {
                let inner = rw_set_of(ss);
                reads.extend(inner.reads);
                writes.extend(inner.writes);
            }
        }
        Statement::SyncBlock { body } => {
            for s in body {
                let inner = rw_set_of(s);
                reads.extend(inner.reads);
                writes.extend(inner.writes);
            }
        }
        Statement::Oracle { body, handler, .. } => {
            for s in body {
                let inner = rw_set_of(s);
                reads.extend(inner.reads);
                writes.extend(inner.writes);
            }
            for s in handler {
                let inner = rw_set_of(s);
                reads.extend(inner.reads);
                writes.extend(inner.writes);
            }
        }
        _ => {}
    }
    ReadWriteSet { reads, writes }
}

/// Collect write target variable from an LHS expression.
fn collect_write_target(expr: &Expr, writes: &mut HashSet<String>) {
    match expr {
        Expr::Identifier(name) => { writes.insert(name.clone()); }
        Expr::ListIndex(target, _) => collect_write_target(target, writes),
        Expr::Projection { source, .. } => collect_write_target(source, writes),
        _ => {}
    }
}

/// Collect all variable reads from an expression.
fn collect_reads_from_expr(expr: &Expr, reads: &mut HashSet<String>) {
    match expr {
        Expr::Identifier(name) => { reads.insert(name.clone()); }
        Expr::Add(l, r) | Expr::Sub(l, r) | Expr::Mul(l, r) | Expr::Div(l, r)
        | Expr::Eq(l, r) | Expr::Ne(l, r) | Expr::Lt(l, r) | Expr::Le(l, r)
        | Expr::Gt(l, r) | Expr::Ge(l, r) | Expr::And(l, r) | Expr::Or(l, r) => {
            collect_reads_from_expr(l, reads);
            collect_reads_from_expr(r, reads);
        }
        Expr::Not(e) | Expr::Neg(e) => collect_reads_from_expr(e, reads),
        Expr::PriorState(_) => {},
        Expr::BinaryOp(bop) => {
            collect_reads_from_expr(&bop.left, reads);
            collect_reads_from_expr(&bop.right, reads);
        }
        Expr::UnaryOp(op) => collect_reads_from_expr(&op.operand, reads),
        Expr::Call(_, args) => {
            for a in args { collect_reads_from_expr(a, reads); }
        }
        Expr::IntrinsicCall { args, .. } => {
            for a in args { collect_reads_from_expr(a, reads); }
        }
        Expr::Projection { source, .. } => collect_reads_from_expr(source, reads),
        Expr::ListIndex(list, idx) => {
            collect_reads_from_expr(list, reads);
            collect_reads_from_expr(idx, reads);
        }
        Expr::ListLiteral(items) => {
            for item in items { collect_reads_from_expr(item, reads); }
        }
        Expr::Cast(inner, _) => collect_reads_from_expr(inner, reads),
        Expr::FieldAccess(obj, _) => collect_reads_from_expr(obj, reads),
        Expr::Block(_, body) => collect_reads_from_expr(body, reads),
        Expr::MapLiteral(entries) => {
            for (k, v) in entries {
                collect_reads_from_expr(k, reads);
                collect_reads_from_expr(v, reads);
            }
        }
        Expr::Match { value, arms } => {
            collect_reads_from_expr(value, reads);
            for arm in arms { collect_reads_from_expr(&arm.body, reads); }
        }
        Expr::Slice { value, start, end, stride, mask } => {
            collect_reads_from_expr(value, reads);
            if let Some(s) = start { collect_reads_from_expr(s, reads); }
            if let Some(e) = end { collect_reads_from_expr(e, reads); }
            if let Some(s) = stride { collect_reads_from_expr(s, reads); }
            if let Some(m) = mask { collect_reads_from_expr(m, reads); }
        }
        Expr::Tuple(items) => {
            for item in items { collect_reads_from_expr(item, reads); }
        }
        Expr::StructInstance(_, fields) => {
            for (_name, val) in fields { collect_reads_from_expr(val, reads); }
        }
        Expr::ObjectLiteral(fields) => {
            for (_name, val) in fields { collect_reads_from_expr(val, reads); }
        }
        Expr::SetLiteral(items) => {
            for item in items { collect_reads_from_expr(item, reads); }
        }
        _ => {}
    }
}

/// Build a dependency graph: stmt i must come before stmt j if j reads
/// what i writes, or j writes what i writes (WAW), or j writes what i reads (WAR).
///
/// Why only forward edges (i < j): edges are only added from earlier
/// statements to later ones. This guarantees the graph is acyclic by
/// construction — Kahn's algorithm in topological_sort can never detect
/// a cycle from reorder_body_statements input. Cycles can still arise
/// from manually constructed graphs in tests or if this function is
/// called on unsorted input.
///
/// Why three edge types:
///   RAW (i writes, j reads)  — classic true dependency, must preserve
///   WAW (both write)         — output dependency, must preserve for
///                              deterministic final value
///   WAR (i reads, j writes)  — anti-dependency, must preserve to avoid
///                              reading a pre-emptively updated value
fn build_dependency_graph(sets: &[ReadWriteSet]) -> HashMap<usize, HashSet<usize>> {
    let mut deps: HashMap<usize, HashSet<usize>> = HashMap::new();
    for i in 0..sets.len() {
        deps.entry(i).or_default();
        for j in i + 1..sets.len() {
            // Statement i writes; statement j reads → RAW dependency (i before j)
            if sets[i].writes.intersection(&sets[j].reads).next().is_some() {
                deps.entry(i).or_default().insert(j);
            }
            // Statement i writes; statement j writes → WAW dependency (i before j)
            if sets[i].writes.intersection(&sets[j].writes).next().is_some() {
                deps.entry(i).or_default().insert(j);
            }
            // Statement i reads; statement j writes → WAR dependency (i before j)
            if sets[i].reads.intersection(&sets[j].writes).next().is_some() {
                deps.entry(i).or_default().insert(j);
            }
        }
    }
    deps
}

/// Kahn's topological sort — emits independent statements grouped together
/// for maximum ILP. Returns (sorted_statements, has_cycle).
///
/// Why Kahn's over DFS-based topological sort: Kahn's emits statements
/// as soon as all their dependencies are satisfied, which naturally groups
/// independent statements together (they all become ready at the same time
/// and are popped consecutively). DFS post-order produces a reverse
/// topo-sort that doesn't have this grouping property.
///
/// Cycle fallback: if Kahn's leaves some statements unscheduled (should
/// not happen from build_dependency_graph but can from manual input),
/// the unscheduled statements are appended in original order. This is a
/// correctness fallback — the schedule is valid (all known dependencies
/// are satisfied) but may miss some transitive edges.
fn topological_sort(body: &[Statement], deps: &HashMap<usize, HashSet<usize>>) -> (Vec<Statement>, bool) {
    let n = body.len();
    let mut in_degree = vec![0usize; n];
    for (_, successors) in deps {
        for &succ in successors {
            in_degree[succ] += 1;
        }
    }
    let mut ready: VecDeque<usize> = (0..n).filter(|&i| in_degree[i] == 0).collect();
    let mut result = Vec::with_capacity(n);
    let mut scheduled = HashSet::new();
    while let Some(idx) = ready.pop_front() {
        scheduled.insert(idx);
        result.push(body[idx].clone());
        if let Some(successors) = deps.get(&idx) {
            for &succ in successors {
                in_degree[succ] -= 1;
                if in_degree[succ] == 0 {
                    ready.push_back(succ);
                }
            }
        }
    }
    let has_cycle = scheduled.len() < n;
    // If cycle detected (some statements not scheduled), append unscheduled
    // in original order as fallback.
    if has_cycle {
        for (i, s) in body.iter().enumerate() {
            if !scheduled.contains(&i) {
                result.push(s.clone());
            }
        }
    }
    (result, has_cycle)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::*;

    #[test]
    fn test_reorder_independent_assignments() {
        // x = a + b; y = c + d; — independent, order preserved
        let body = vec![
            Statement::Assignment {
                lhs: Expr::AddrOf(Box::new(Expr::Identifier("x".into()))),
                expr: Expr::Add(Box::new(Expr::Identifier("a".into())), Box::new(Expr::Identifier("b".into()))),
                timeout: None, modifiers: vec![],
            },
            Statement::Assignment {
                lhs: Expr::AddrOf(Box::new(Expr::Identifier("y".into()))),
                expr: Expr::Add(Box::new(Expr::Identifier("c".into())), Box::new(Expr::Identifier("d".into()))),
                timeout: None, modifiers: vec![],
            },
        ];
        let (reordered, has_cycle) = reorder_body_statements(&body);
        assert_eq!(reordered.len(), 2);
        assert!(!has_cycle);
    }

    #[test]
    fn test_reorder_dependent_assignments() {
        // x = a + b; y = x + 1; — y depends on x, must come after
        let body = vec![
            Statement::Assignment {
                lhs: Expr::AddrOf(Box::new(Expr::Identifier("x".into()))),
                expr: Expr::Add(Box::new(Expr::Identifier("a".into())), Box::new(Expr::Identifier("b".into()))),
                timeout: None, modifiers: vec![],
            },
            Statement::Assignment {
                lhs: Expr::AddrOf(Box::new(Expr::Identifier("y".into()))),
                expr: Expr::Add(Box::new(Expr::Identifier("x".into())), Box::new(Expr::Decimal(1))),
                timeout: None, modifiers: vec![],
            },
        ];
        let (reordered, has_cycle) = reorder_body_statements(&body);
        assert_eq!(reordered.len(), 2);
        // y must come after x
        let x_pos = reordered.iter().position(|s| matches!(s, Statement::Assignment { lhs: Expr::AddrOf(inner), .. } if inner.as_var_name() == Some("x")));
        let y_pos = reordered.iter().position(|s| matches!(s, Statement::Assignment { lhs: Expr::AddrOf(inner), .. } if inner.as_var_name() == Some("y")));
        assert!(x_pos < y_pos, "dependent statement must come after");
    }

    #[test]
    fn test_reorder_chain() {
        // a = 1; b = a + 1; c = b + 1; — chain, must preserve order
        let body = vec![
            Statement::Assignment { lhs: Expr::AddrOf(Box::new(Expr::Identifier("a".into()))), expr: Expr::Decimal(1), timeout: None, modifiers: vec![] },
            Statement::Assignment { lhs: Expr::AddrOf(Box::new(Expr::Identifier("b".into()))), expr: Expr::Add(Box::new(Expr::Identifier("a".into())), Box::new(Expr::Decimal(1))), timeout: None, modifiers: vec![] },
            Statement::Assignment { lhs: Expr::AddrOf(Box::new(Expr::Identifier("c".into()))), expr: Expr::Add(Box::new(Expr::Identifier("b".into())), Box::new(Expr::Decimal(1))), timeout: None, modifiers: vec![] },
        ];
        let (reordered, has_cycle) = reorder_body_statements(&body);
        assert_eq!(reordered.len(), 3);
        assert!(!has_cycle);
        let a_pos = reordered.iter().position(|s| matches!(s, Statement::Assignment { lhs: Expr::AddrOf(inner), .. } if inner.as_var_name() == Some("a")));
        let b_pos = reordered.iter().position(|s| matches!(s, Statement::Assignment { lhs: Expr::AddrOf(inner), .. } if inner.as_var_name() == Some("b")));
        let c_pos = reordered.iter().position(|s| matches!(s, Statement::Assignment { lhs: Expr::AddrOf(inner), .. } if inner.as_var_name() == Some("c")));
        assert!(a_pos < b_pos && b_pos < c_pos, "chain order must be preserved");
    }

    #[test]
    fn test_topological_sort_cycle_detected() {
        // 2026-06-19: Verify cycle detection in the internal topological_sort.
        // reorder_body_statements cannot produce cycles (edges are always forward),
        // so test topological_sort directly with a manually constructed cycle graph.
        let body = vec![
            Statement::Assignment { lhs: Expr::AddrOf(Box::new(Expr::Identifier("x".into()))), expr: Expr::Decimal(1), timeout: None, modifiers: vec![] },
            Statement::Assignment { lhs: Expr::AddrOf(Box::new(Expr::Identifier("y".into()))), expr: Expr::Decimal(2), timeout: None, modifiers: vec![] },
        ];
        let mut deps: HashMap<usize, HashSet<usize>> = HashMap::new();
        deps.insert(0, HashSet::from([1]));
        deps.insert(1, HashSet::from([0]));
        let (reordered, has_cycle) = topological_sort(&body, &deps);
        assert!(has_cycle, "Cycle in dependency graph should be detected");
        // Fallback: all statements appear in original order
        assert_eq!(reordered.len(), 2, "All statements must appear in fallback order");
    }

    #[test]
    fn test_reorder_short_body_no_reordering() {
        // 2026-06-19: Bodies with < 3 statements are returned as-is.
        let body = vec![
            Statement::Assignment { lhs: Expr::AddrOf(Box::new(Expr::Identifier("x".into()))), expr: Expr::Decimal(1), timeout: None, modifiers: vec![] },
        ];
        let (reordered, has_cycle) = reorder_body_statements(&body);
        assert_eq!(reordered.len(), 1);
        assert!(!has_cycle);
    }
}
