// ── LLVM Normalizer — AST Annotation Pass ─────────────────────────────
// 2026-07-14: Walks the AST and attaches llvm_type to every type reference.
// Backend never reads config files or matches on primitive/bytes.

use crate::ast::*;
use crate::backend::normalizer;
use crate::config::{derive_llvm_type, OpConfig, TypeConfig};
use crate::type_universe::TypeUniverse;

/// 2026-07-14: Normalize the AST for LLVM backend emission.
/// Attaches llvm_type property to every ResolvedType in the universe.
/// For types with fixed-width layout, parses the pattern and attaches
/// field-level bit offset annotations.
pub fn normalize(items: &mut Vec<TopLevel>, universe: &mut TypeUniverse) -> Result<(), String> {
    let prim_config = TypeConfig::load();

    // Attach llvm_type to every type
    for rt in universe.types.values_mut() {
        let prim = rt.primitive();
        let llvm_ty = derive_llvm_type(prim, rt.bytes, &prim_config);
        rt.properties.insert("llvm_type".into(), PropertyValue::String(llvm_ty));

        // 2026-07-14: Parse layout pattern and attach field annotations
        if let Some(PropertyValue::String(layout_str)) = rt.properties.get("layout") {
            if let Ok(pat) = crate::bvir::layout::parse_layout_pattern(layout_str) {
                attach_layout_fields(rt, &pat);
            }
        }
    }

    // Validate intrinsics against supported set
    let op_config = OpConfig::load();
    let supported = build_supported_ops(&op_config);
    let errors = normalizer::validate_intrinsics(items, &supported);
    if !errors.is_empty() {
        return Err(format!("LLVM normalizer:\n  {}", errors.join("\n  ")));
    }

    // Strip metadata LLVM doesn't use
    let keep: HashSet<String> = ["primitive", "llvm_type", "encoding", "layout"]
        .iter().map(|s| s.to_string()).collect();
    for rt in universe.types.values_mut() {
        rt.properties.retain(|k, _| keep.contains(k));
    }

    Ok(())
}

/// 2026-07-14: Walk a LayoutPattern and attach field-level annotations.
fn attach_layout_fields(rt: &mut crate::type_universe::ResolvedType, pat: &crate::ast::layout::LayoutPattern) {
    if let crate::ast::layout::LayoutPattern::Slice(fields) = pat {
        let mut offset = 0u64;
        for field in fields {
            // Attach offset and width as properties
            rt.properties.insert(
                format!("field.{}.offset", field.name),
                PropertyValue::Int(offset as i64),
            );
            rt.properties.insert(
                format!("field.{}.width", field.name),
                PropertyValue::Int(field.bits as i64),
            );
            if field.mutable {
                rt.properties.insert(
                    format!("field.{}.mutable", field.name),
                    PropertyValue::Bool(true),
                );
            }
            offset += field.bits;
        }
    }
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
