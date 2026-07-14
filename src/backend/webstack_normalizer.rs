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
    // Attach js_type to every type
    for rt in universe.types.values_mut() {
        let js_type = match rt.primitive() {
            Some("Int") | Some("UInt") | Some("Int64") | Some("UInt64") => "number",
            Some("Float") | Some("Float32") => "number",
            Some("Float64") | Some("Double") => "number",
            Some("Bool") => "boolean",
            Some("String") => "string",
            Some("Char") => "number",
            Some("Data") => "Uint8Array",
            _ => "object",
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
    let keep: HashSet<String> = ["primitive", "js_type", "encoding", "bytes"]
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
