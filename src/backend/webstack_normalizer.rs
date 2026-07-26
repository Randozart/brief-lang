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

    // 2026-07-26: Reject intrinsics not supported by WASM/WebAssembly backend.
    // See docs/architecture/features/webstack-intrinsics.md for the full policy.
    let supported = build_supported_ops();
    let errors = normalizer::validate_intrinsics(items, &supported);
    if !errors.is_empty() {
        let detail = errors.join("\n  ");
        return Err(format!(
            "Intrinsic is not supported by the webstack/WebAssembly backend:\n  {}\n\
             See docs/architecture/features/webstack-intrinsics.md",
            detail
        ));
    }

    // Strip hardware-specific metadata
    let keep: HashSet<String> = ["js_type", "llvm_type", "disamb"]
        .iter().map(|s| s.to_string()).collect();
    for rt in universe.types.values_mut() {
        rt.properties.retain(|k, _| keep.contains(k));
    }

    Ok(())
}

/// Webstack supported intrinsics — Tiers 1-3 from webstack intrinsics policy.
/// 2026-07-26: Tier 1 = WASM native, Tier 2 = WASM runtime, Tier 3 = browser API.
/// Any intrinsic not in this set produces a compile error:
///   "Intrinsic '<name>' is not supported by the webstack/WebAssembly backend."
/// See docs/architecture/features/webstack-intrinsics.md
fn build_supported_ops() -> HashSet<String> {
    let mut set = HashSet::new();
    // Tier 1: WASM native (arithmetic, comparison, bitwise, float math)
    for op in &[
        "Add#", "Sub#", "Mul#", "Div#", "Rem#", "Neg#", "Abs#",
        "Eq#", "Neq#", "Lt#", "Gt#", "Le#", "Ge#",
        "BitAnd#", "BitOr#", "BitXor#", "Shl#", "Shr#", "BitNot#",
        "Not#",
        "Fabs#", "Ceil#", "Floor#", "Sqrt#", "Sin#", "Cos#", "Pow#",
    ] { set.insert(op.to_string()); }
    // Tier 2: WASM runtime (memory, atomics, pointer, string ops)
    for op in &[
        "Ptr#", "Deref#", "Index#", "Cast#", "AddressOf#",
        "Load#", "Store#", "Malloc#", "Alloc#", "Free#", "Copy#", "Fill#",
        "Memcpy#", "Memset#",
        "Len#", "Length#", "Concat#", "Get#", "Insert#",
        "ToInt#", "ToFloat#", "ToString#",
        "AtomicLoad#", "AtomicStore#", "AtomicCas#", "AtomicXchg#",
        "AtomicAdd#", "Fence#",
    ] { set.insert(op.to_string()); }
    // Tier 3: Browser API (console, time, env queries — JS shim provides)
    for op in &[
        "PrintInt#", "PrintFloat#", "PrintChar#", "Print#",
        "Time#", "CpuCount#", "Hostname#", "PageSize#", "Errno#", "Sleep#",
    ] { set.insert(op.to_string()); }
    set
}
