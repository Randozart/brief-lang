// ── Webstack Normalizer — AST Annotation Pass ─────────────────────────
// 2026-07-14: Attaches js_type for JS/TS codegen.
// Keeps primitive, strips hardware-specific metadata.

use std::collections::HashSet;
use crate::ast::*;
use crate::backend::normalizer;
use crate::type_universe::TypeUniverse;

/// 2026-07-14: Normalize the AST for Webstack (WASM + JS) backend.
/// Attaches js_type annotation based on primitive metadata.
pub fn normalize(items: &mut Vec<TopLevel>, universe: &mut TypeUniverse) -> Result<(), String> {
    // 2026-07-20: Derive js_type from llvm_type (hashword protocol replaces CTD)
    for rt in universe.types.values_mut() {
        let llvm_ty = rt.properties.get("llvm_type").and_then(|pv| match pv {
            PropertyValue::String(s) => Some(s.as_str()),
            _ => None,
        });
        let js_type = match llvm_ty {
            Some("i64" | "i32" | "i16" | "i8") => "number",
            Some("float" | "double") => "number",
            Some("i1" | "i8") => "boolean",
            _ => match rt.base.as_str() {
                "String" | "Bits" if rt.fields.len() >= 2 => "string",
                _ => "object",
            },
        };
        rt.properties.insert("js_type".into(), PropertyValue::String(js_type.into()));
    }

    // Reject hardware-specific intrinsics
    let supported = build_supported_ops();
    let errors = normalizer::validate_intrinsics(items, &supported);
    if !errors.is_empty() {
        return Err(format!("Webstack normalizer:\n  {}", errors.join("\n  ")));
    }

    // Strip hardware-specific metadata
    let keep: HashSet<String> = ["js_type", "llvm_type", "disamb"]
        .iter().map(|s| s.to_string()).collect();
    for rt in universe.types.values_mut() {
        rt.properties.retain(|k, _| keep.contains(k));
    }

    Ok(())
}

/// Webstack supported intrinsics.
fn build_supported_ops() -> HashSet<String> {
    let mut set = HashSet::new();
    for op in &[
        "Add#", "Sub#", "Mul#", "Div#", "Rem#",
        "Eq#", "Neq#", "Lt#", "Gt#", "Le#", "Ge#",
        "Neg#", "Abs#",
        "Sqrt#", "Sin#", "Cos#", "Fabs#", "Ceil#", "Floor#",
        "Print#",
        "Concat#", "Length#", "ToInt#", "ToFloat#", "ToString#",
        "Malloc#", "Free#", "Memcpy#", "Memset#",
        "Get#", "Insert#",
    ] {
        set.insert(op.to_string());
    }
    set
}
