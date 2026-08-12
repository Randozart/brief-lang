use std::fmt::Write;
// ── LLVM ABI Marshaling ───────────────────────────────────────────────
// 2026-07-12: Phase 2.7/4 — Marshaling between C calling convention
// and Briev internal types (Bool zext/trunc, String ptr extract, etc.).

use crate::ast::Type;
use crate::type_universe::TypeUniverse;

/// Marshal a C parameter value into a Briev internal value.
/// Bool: trunc i8 to i1
///
/// 2026-07-31: Phase 3 (§8.4-D9) — Bool detection via the `Cast.#Bool`
/// protocol property (universe) instead of the type name.
pub fn marshal_param_to_briev(
    param_ty: &Type,
    param_reg: &str,
    universe: &Option<TypeUniverse>,
) -> String {
    if is_bool_type(param_ty, universe) {
        format!("trunc i8 %{} to i1", param_reg)
    } else {
        format!("%{}", param_reg)
    }
}

/// Marshal a Briev return value to C calling convention.
/// Bool: zext i1 to i8
/// String: extract .ptr field as ptr
pub fn marshal_briev_to_return(
    ret_ty: &Type,
    ret_reg: &str,
    universe: &Option<TypeUniverse>,
) -> String {
    if is_bool_type(ret_ty, universe) {
        format!("zext i1 %{} to i8", ret_reg)
    } else {
        format!("%{}", ret_reg)
    }
}

/// Generate the LLVM IR for an export wrapper function.
/// The wrapper handles type marshaling between C ABI and Briev internal types.
pub fn marshal_export_wrapper(
    defn_name: &str,
    export_name: &str,
    param_tys: &[Type],
    ret_ty: &Option<Type>,
    universe: &Option<TypeUniverse>,
) -> String {
    let mut out = String::new();
    // Declare the wrapper with C calling convention
    let ret_llvm = ret_ty
        .as_ref()
        .map(|t| super::types::lower_type(t, None))
        .unwrap_or_else(|| "void".into());
    let params_llvm: Vec<String> = param_tys
        .iter()
        .map(|t| super::types::lower_type(t, None))
        .collect();
    let param_decls: Vec<String> = param_tys
        .iter()
        .enumerate()
        .map(|(i, t)| format!("{} %p{}", super::types::lower_type(t, None), i))
        .collect();

    writeln!(
        out,
        "define {} @{}({}) #0 {{",
        ret_llvm,
        export_name,
        param_decls.join(", ")
    )
    .ok();
    writeln!(
        out,
        "  %result = call {} @{}({})",
        ret_llvm,
        defn_name,
        params_llvm
            .iter()
            .enumerate()
            .map(|(i, t)| marshal_param_to_briev(&param_tys[i], &format!("p{}", i), universe))
            .collect::<Vec<_>>()
            .join(", ")
    )
    .ok();
    writeln!(out, "  ret {} %result", ret_llvm).ok();
    writeln!(out, "}}").ok();

    out
}

/// Check if a type is Bool (i1 in LLVM, i8 in C ABI).
///
/// 2026-07-31: Phase 3 (§8.4-D9) — protocol membership via the `Cast.#Bool`
/// property instead of matching the type name. Mirrors the casting graph's
/// category resolution (Bool primordial seeds Cast.#Bool).
fn is_bool_type(ty: &Type, universe: &Option<TypeUniverse>) -> bool {
    ty.universe_key()
        .and_then(|k| universe.as_ref().and_then(|u| u.get(k)))
        .map_or(false, |rt| rt.properties.contains_key("Cast.#Bool"))
}
