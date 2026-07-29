// ── SPIR-V Normalizer — AST Annotation Pass ──────────────────────────
// 2026-07-15: Resolves types via TypeUniverse, flags kernels, validates
// against SPIR-V supported ops.
//
// 2026-07-20: Simplified for hashword protocol. ALU metadata removed.
// Op validation uses hardcoded standard ops instead of TOML config.

use crate::ast::*;
use crate::backend::normalizer;
use crate::type_universe::TypeUniverse;
use std::collections::HashSet;

/// 2026-07-15: Normalize AST for SPIR-V backend emission.
/// 2026-07-20: No TOML config — hashword protocol replaces op dispatch.
pub fn normalize(items: &mut Vec<TopLevel>, universe: &mut TypeUniverse, _int_bits: u64) -> Result<(), String> {
    // 2026-07-15: Flag kernel transactions ([idx < N] pattern)
    for item in items.iter_mut() {
        if let TopLevel::Transaction(txn) = item {
            if is_kernel_txn(txn) {
                txn.metadata.insert("is_kernel".into(), PropertyValue::Bool(true));
            }
        }
    }

    // 2026-07-15: Validate operators against standard op set
    let supported = build_supported_ops();
    let errors = normalizer::validate_intrinsics(items, &supported);
    if !errors.is_empty() {
        return Err(format!("SPIR-V normalizer:\n  {}", errors.join("\n  ")));
    }

    // Strip irrelevant metadata
    let keep: HashSet<String> = ["is_kernel", "disamb"].iter().map(|s| s.to_string()).collect();
    for rt in universe.types.values_mut() {
        rt.properties.retain(|k, _| keep.contains(k) || k.starts_with("op."));
    }

    Ok(())
}

fn is_kernel_txn(txn: &crate::ast::top::Transaction) -> bool {
    // 2026-07-15: Kernel if contract has an index-sized precondition
    let pre = &txn.contract.pre_condition;
    let s = format!("{}", pre);
    s.contains("idx") || s.contains("Index")
}

/// Build the set of supported intrinsic names.
fn build_supported_ops() -> HashSet<String> {
    let mut set = HashSet::new();
    for name in &["Add#", "Sub#", "Mul#", "Div#", "Eq#", "Lt#", "Gt#",
                   "BitAnd#", "BitOr#", "BitXor#", "Shl#", "Shr#",
                   "Malloc#", "Free#", "Print#"] {
        set.insert(name.to_string());
    }
    set
}
