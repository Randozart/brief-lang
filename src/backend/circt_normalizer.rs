// ── CIRCT Normalizer — AST Annotation Pass ────────────────────────────
// 2026-07-14: Strips primitive metadata (CIRCT uses bytes only),
// attaches bit_width, rejects runtime-dependent intrinsics.
// Follows the same pattern as llvm/normalizer.rs.

use std::collections::HashSet;
use crate::ast::*;
use crate::backend::normalizer;
use crate::type_universe::TypeUniverse;

/// 2026-07-14: Normalize the AST for CIRCT hardware backend emission.
/// CIRCT doesn't use primitive — it uses bytes to determine bit width.
/// Rejects intrinsics that require runtime OS support (Print#, Malloc#, etc.).
pub fn normalize(items: &mut Vec<TopLevel>, universe: &mut TypeUniverse) -> Result<(), String> {
    // Attach bit_width to every type
    for rt in universe.types.values_mut() {
        let bits = rt.bytes * 8;
        rt.properties.insert("bit_width".into(), PropertyValue::Int(bits as i64));
    }

    // Reject runtime-dependent intrinsics
    let supported = build_supported_ops();
    let errors = normalizer::validate_intrinsics(items, &supported);
    if !errors.is_empty() {
        return Err(format!("CIRCT normalizer:\n  {}", errors.join("\n  ")));
    }

    // Strip everything except what CIRCT needs
    let keep: HashSet<String> = ["bit_width", "hardware", "alignment"]
        .iter().map(|s| s.to_string()).collect();
    for rt in universe.types.values_mut() {
        rt.properties.retain(|k, _| keep.contains(k));
    }

    Ok(())
}

/// CIRCT supported intrinsics — hardware subset only.
fn build_supported_ops() -> HashSet<String> {
    let mut set = HashSet::new();
    for op in &[
        "Add#", "Sub#", "Mul#", "Div#", "Rem#",
        "Eq#", "Neq#", "Lt#", "Gt#", "Le#", "Ge#",
        "Neg#", "Abs#",
        "GetGlobalId#", "GetGlobalSize#", "GetLocalId#",
    ] {
        set.insert(op.to_string());
    }
    set
}
