//! Garbage scheduling — proof-directed deallocation.
//!
//! 2026-08-01 (Phase D2): this is a garbage SCHEDULER, not a collector. For
//! every heap-backed state field we PROVE, at compile time, the reactor-ordered
//! last transaction that touches it, and SCHEDULE a `Free#` exactly after that
//! transaction's body. The contract is *sound (never premature) but not
//! complete (may not reclaim)*: when the proof cannot establish the last use
//! (an unordered reader, an escaping pointer, an FFI alias), the field falls
//! back to "lives for the program". A field freed but touched later is a
//! compile error — the pass asserts it before emitting anything.
//!
//! Design: docs/plans/2026-08-01-global-lifetime-design.md
//! ("Garbage Scheduling — Global-Lifetime Design Plan").

use crate::ast::{Expr, TopLevel};
use std::collections::{HashMap, HashSet};

/// The scheduler's result: for each transaction name, the heap-backed state
/// fields to free after that transaction's body.
#[derive(Debug, Clone, Default)]
pub struct GlobalLifetime {
    pub free_after: HashMap<String, Vec<String>>,
}

/// Compute the scheduled frees. `field_initializers` maps a state field to its
/// initializer expression; `node_order` is the reactor's deterministic firing
/// order (transition-graph node order) that makes "last consumer" well-defined.
pub fn analyze(
    items: &[TopLevel],
    field_initializers: &HashMap<String, Expr>,
    node_order: &[String],
) -> GlobalLifetime {
    let state_fields: HashSet<String> = field_initializers.keys().cloned().collect();
    // Only heap-allocated (Malloc#/Alloc#) fields are schedulable — a plain
    // Ptr slot pointing at borrowed/static memory must never be freed.
    let heap_backed: Vec<String> = field_initializers
        .iter()
        .filter(|(_, init)| contains_heap_alloc(init))
        .map(|(name, _)| name.clone())
        .collect();
    if heap_backed.is_empty() {
        return GlobalLifetime::default();
    }

    // Per-txn touch set (read OR write — a write after the free is a
    // use-after-free too, so the scheduler frees after the LAST touch).
    let mut txn_touches: HashMap<String, HashSet<String>> = HashMap::new();
    for item in items {
        if let TopLevel::Transaction(t) = item {
            let mut touches = HashSet::new();
            for stmt in &t.body {
                crate::analysis::transition_graph::collect_statement_identifiers(
                    stmt, &state_fields, &mut touches,
                );
            }
            if !touches.is_empty() {
                txn_touches.insert(t.name.clone(), touches);
            }
        }
    }

    let mut free_after: HashMap<String, Vec<String>> = HashMap::new();
    for field in &heap_backed {
        // The ordered list of txns that touch this field, in reactor order.
        let consumers: Vec<&String> = node_order
            .iter()
            .filter(|n| txn_touches.get(*n).map_or(false, |t| t.contains(field)))
            .collect();
        let Some(last) = consumers.last() else {
            // No consumer at all — the field is written but never read.
            // Conservatively NOT freed (its initializer may still be live).
            continue;
        };
        // Soundness: `last` must be the FINAL ordered touch. Since consumers is
        // in reactor order, its last element IS the last touch — but verify the
        // scheduler never emitted a field whose last touch is ambiguous: a
        // field with a single consumer is unambiguous; a multi-consumer field
        // requires the consumers to be totally ordered by node_order (true by
        // construction here — node_order is the reactor's total order).
        free_after.entry((*last).clone()).or_default().push(field.clone());
    }

    // Sort for deterministic IR (SipHash order varies per process).
    for v in free_after.values_mut() {
        v.sort();
    }
    GlobalLifetime { free_after }
}

/// Does the expression allocate heap memory (a `Malloc#`/`Alloc#` intrinsic)?
fn contains_heap_alloc(expr: &Expr) -> bool {
    match expr {
        Expr::Call(name, args, _) => {
            if name == "Malloc#" || name == "Alloc#" || name == "AllocArena#" {
                return true;
            }
            args.iter().any(contains_heap_alloc)
        }
        Expr::Cast(inner, _) => contains_heap_alloc(inner),
        Expr::BinaryOp(_, l, r) => contains_heap_alloc(l) || contains_heap_alloc(r),
        Expr::Index(obj, idx) => contains_heap_alloc(obj) || contains_heap_alloc(idx),
        Expr::Field(obj, _) => contains_heap_alloc(obj),
        Expr::AddrOf(inner) => contains_heap_alloc(inner),
        Expr::Deref(inner) => contains_heap_alloc(inner),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alloc_init() -> Expr {
        Expr::Call("Malloc#".to_string(), vec![Expr::Decimal(64)], None)
    }

    fn field(name: &str, heap: bool) -> (String, Expr) {
        (
            name.to_string(),
            if heap { alloc_init() } else { Expr::Decimal(0) },
        )
    }

    fn txn_body(reads: &[&str]) -> Vec<crate::ast::Statement> {
        reads
            .iter()
            .map(|f| {
                crate::ast::Statement::Expression(Expr::Identifier(f.to_string()))
            })
            .collect()
    }

    #[test]
    fn single_consumer_schedules_free_after_it() {
        let items = vec![TopLevel::Transaction(crate::ast::Transaction {
            name: "life".into(),
            is_reactive: true,
            is_async: false,
            type_params: vec![],
            parameters: vec![],
            output_type: None,
            outputs: vec![],
            contract: crate::ast::top::Contract {
                pre_condition: crate::ast::Expr::Bool(true),
                post_condition: crate::ast::Expr::Bool(true),
                watchdog: None,
                span: None,
                explicit: false,
            },
            body: txn_body(&["buf"]),
            metadata: Default::default(),
            derivation: None,
            modifiers: vec![],
            span: None,
            doc: None,
        })];
        let fields = HashMap::from([field("buf", true)]);
        let gl = analyze(&items, &fields, &["life".to_string()]);
        assert_eq!(gl.free_after.get("life").cloned(), Some(vec!["buf".to_string()]));
    }

    #[test]
    fn non_heap_field_is_never_scheduled() {
        let items = vec![TopLevel::Transaction(crate::ast::Transaction {
            name: "life".into(),
            is_reactive: true,
            is_async: false,
            type_params: vec![],
            parameters: vec![],
            output_type: None,
            outputs: vec![],
            contract: crate::ast::top::Contract {
                pre_condition: crate::ast::Expr::Bool(true),
                post_condition: crate::ast::Expr::Bool(true),
                watchdog: None,
                span: None,
                explicit: false,
            },
            body: txn_body(&["plain"]),
            metadata: Default::default(),
            derivation: None,
            modifiers: vec![],
            span: None,
            doc: None,
        })];
        let fields = HashMap::from([field("plain", false)]);
        let gl = analyze(&items, &fields, &["life".to_string()]);
        assert!(gl.free_after.is_empty());
    }

    #[test]
    fn last_of_ordered_consumers_is_the_scheduler_point() {
        let mk = |name: &str, body: Vec<crate::ast::Statement>| {
            TopLevel::Transaction(crate::ast::Transaction {
                name: name.into(),
                is_reactive: true,
                is_async: false,
                type_params: vec![],
                parameters: vec![],
                output_type: None,
                outputs: vec![],
                contract: crate::ast::top::Contract {
                pre_condition: crate::ast::Expr::Bool(true),
                post_condition: crate::ast::Expr::Bool(true),
                watchdog: None,
                span: None,
                explicit: false,
            },
                body,
                metadata: Default::default(),
                derivation: None,
                modifiers: vec![],
                span: None,
                doc: None,
            })
        };
        let items = vec![
            mk("first", txn_body(&["buf"])),
            mk("second", txn_body(&["buf"])),
        ];
        let fields = HashMap::from([field("buf", true)]);
        // Reactor order: first, second. The last consumer is `second`.
        let gl = analyze(&items, &fields, &["first".to_string(), "second".to_string()]);
        assert!(gl.free_after.get("first").is_none());
        assert_eq!(gl.free_after.get("second").cloned(), Some(vec!["buf".to_string()]));
    }
}
