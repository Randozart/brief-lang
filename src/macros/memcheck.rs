//! `brivc memcheck <file.bv>` — the garbage-scheduler diagnostics subcommand.
//!
//! 2026-08-01 (Phase 5): reports, per heap-backed state field, whether the
//! garbage scheduler proved a last use and scheduled a free (and after which
//! transaction), or fell back to "lives for the program" (a potential leak).
//! Also reports the effect of every `free x;` / `keep x;` hint.

use crate::analysis::global_lifetime::GlobalLifetime;
use std::collections::HashMap;

/// Run the memcheck analysis on a parsed program.
pub fn run_memcheck(items: &[crate::ast::TopLevel]) -> MemcheckReport {
    // Field initializers mirror analyze_program (StateDecl + top-level let).
    let mut field_inits: HashMap<String, crate::ast::Expr> = HashMap::new();
    for item in items {
        if let crate::ast::TopLevel::StateDecl(s) = item {
            field_inits.entry(s.name.clone()).or_insert(crate::ast::Expr::Decimal(0));
        } else if let crate::ast::TopLevel::Statement(stmt) = item {
            if let crate::ast::Statement::Let { name, expr, .. } = stmt.as_ref() {
                if let Some(e) = expr {
                    field_inits.entry(name.clone()).or_insert_with(|| e.clone());
                }
            }
        }
    }
    // The reactor firing order is the transition-graph node order.
    let transition_graph = crate::analysis::transition_graph::ReactorTransitionGraph::build(
        items, &None, &vec![],
    );
    let node_order: Vec<String> = transition_graph.nodes.iter().map(|n| n.name.clone()).collect();
    let foldable: std::collections::HashSet<String> = transition_graph
        .nodes
        .iter()
        .filter(|n| n.bounded_pre.is_some())
        .map(|n| n.name.clone())
        .collect();
    let lifetime = crate::analysis::global_lifetime::analyze(items, &field_inits, &node_order, &foldable);
    // Only heap-backed fields are schedulable; scalars are never freed (a
    // "lives for the program" report on them would be misleading).
    let heap_fields: Vec<String> = field_inits
        .iter()
        .filter(|(_, init)| crate::analysis::global_lifetime::contains_heap_alloc(init))
        .map(|(name, _)| name.clone())
        .collect();
    MemcheckReport {
        lifetime,
        field_names: heap_fields,
    }
}

/// The report: the scheduler's decisions + the hint outcomes.
pub struct MemcheckReport {
    pub lifetime: GlobalLifetime,
    pub field_names: Vec<String>,
}

/// Print the report to stdout.
pub fn print_memcheck(report: &MemcheckReport) {
    let mut fields = report.field_names.clone();
    fields.sort();
    println!("=== memcheck — garbage-scheduler decisions ===");
    if fields.is_empty() {
        println!("  (no state fields)");
    }
    for f in &fields {
        // Scheduled (freed after a txn)?
        let scheduled: Vec<&String> = report
            .lifetime
            .free_after
            .iter()
            .filter_map(|(txn, fields)| {
                if fields.contains(f) {
                    Some(txn)
                } else {
                    None
                }
            })
            .collect();
        if !scheduled.is_empty() {
            let txn_names: Vec<&str> = scheduled.iter().map(|t| t.as_str()).collect();
            println!("  {}: freed after {}", f, txn_names.join(", "));
        } else {
            println!("  {}: lives for the program (unprovable — potential leak; add `free`/`keep` or a refcount)", f);
        }
    }
    if !report.lifetime.redundant_keeps.is_empty() {
        println!("  redundant `keep` hints (the scheduler would not free these anyway):");
        for k in &report.lifetime.redundant_keeps {
            println!("    keep {};", k);
        }
    }
    println!("=== end memcheck ===");
}
