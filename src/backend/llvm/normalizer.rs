// ── LLVM Normalizer — AST Annotation Pass ─────────────────────────────
// 2026-07-14: Walks the AST and attaches llvm_type to every type reference.
// Backend never reads config files or matches on primitive/bytes.

use crate::ast::*;
use crate::backend::normalizer;
use crate::config::{derive_llvm_type, OpConfig, TypeConfig};
use crate::type_universe::TypeUniverse;

/// 2026-07-14: Normalize the AST for LLVM backend emission.
/// Attaches llvm_type property to every ResolvedType in the universe.
pub fn normalize(items: &mut Vec<TopLevel>, universe: &mut TypeUniverse) -> Result<(), String> {
    let prim_config = TypeConfig::load();

    // Attach llvm_type to every type
    for rt in universe.types.values_mut() {
        let prim = rt.primitive();
        let llvm_ty = derive_llvm_type(prim, rt.bytes, &prim_config);
        rt.properties.insert("llvm_type".into(), PropertyValue::String(llvm_ty));
    }

    // Validate intrinsics against supported set
    let op_config = OpConfig::load();
    let supported = build_supported_ops(&op_config);
    let errors = normalizer::validate_intrinsics(items, &supported);
    if !errors.is_empty() {
        return Err(format!("LLVM normalizer:\n  {}", errors.join("\n  ")));
    }

    // Strip metadata LLVM doesn't use
    let keep: HashSet<String> = ["primitive", "llvm_type", "encoding"]
        .iter().map(|s| s.to_string()).collect();
    for rt in universe.types.values_mut() {
        rt.properties.retain(|k, _| keep.contains(k));
    }

    Ok(())
}

use std::collections::HashSet;

/// Build the set of supported intrinsic names from the op config.
fn build_supported_ops(config: &OpConfig) -> HashSet<String> {
    let mut set = HashSet::new();
    // Generic operations (from llvm-ops.toml section keys "op.Add" etc.)
    for op_name in STANDARD_OPS {
        set.insert(format!("{}#", op_name));
    }
    // Also add some well-known intrinsics that don't appear as operations
    for name in &["GetEnv#", "GetGlobalId#", "GetGlobalSize#", "GetLocalId#",
                   "ToInt#", "ToFloat#", "ToString#", "Concat#", "Length#"] {
        set.insert(name.to_string());
    }
    set
}

/// Standard generic operation names (without the # suffix).
const STANDARD_OPS: &[&str] = &[
    "Add", "Sub", "Mul", "Div", "Rem",
    "Eq", "Neq", "Lt", "Gt", "Le", "Ge",
    "Neg", "Abs",
    "Sqrt", "Sin", "Cos", "Fabs", "Ceil", "Floor", "Pow",
    "Print",
    "Malloc", "Free", "Memcpy", "Memset",
];
