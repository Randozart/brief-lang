// ── LLVM ABI Marshaling ───────────────────────────────────────────────
// 2026-07-12: Phase 2.7/4 — Marshaling between C calling convention
// and Brief internal types (Bool zext/trunc, String ptr extract, etc.).

use crate::ast_new::Type;

/// Marshal a C parameter value into a Brief internal value.
/// Bool: trunc i8 to i1
pub fn marshal_param_to_brief(param_ty: &Type, param_reg: &str) -> String {
    if is_bool_type(param_ty) {
        format!("trunc i8 %{} to i1", param_reg)
    } else {
        format!("%{}", param_reg)
    }
}

/// Marshal a Brief return value to C calling convention.
/// Bool: zext i1 to i8
/// String: extract .ptr field as ptr
pub fn marshal_brief_to_return(ret_ty: &Type, ret_reg: &str) -> String {
    if is_bool_type(ret_ty) {
        format!("zext i1 %{} to i8", ret_reg)
    } else {
        format!("%{}", ret_reg)
    }
}

/// Generate the LLVM IR for an export wrapper function.
/// The wrapper handles type marshaling between C ABI and Brief internal types.
pub fn marshal_export_wrapper(
    defn_name: &str,
    export_name: &str,
    param_tys: &[Type],
    ret_ty: &Option<Type>,
) -> String {
    let mut out = String::new();
    // Declare the wrapper with C calling convention
    let ret_llvm = ret_ty
        .as_ref()
        .map(|t| super::types::lower_type(t))
        .unwrap_or_else(|| "void".into());
    let params_llvm: Vec<String> = param_tys
        .iter()
        .map(|t| super::types::lower_type(t))
        .collect();
    let param_decls: Vec<String> = param_tys
        .iter()
        .enumerate()
        .map(|(i, t)| format!("{} %p{}", super::types::lower_type(t), i))
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
            .map(|(i, t)| marshal_param_to_brief(&param_tys[i], &format!("p{}", i)))
            .collect::<Vec<_>>()
            .join(", ")
    )
    .ok();
    writeln!(out, "  ret {} %result", ret_llvm).ok();
    writeln!(out, "}}").ok();

    out
}

/// Check if a type is Bool (i1 in LLVM, i8 in C ABI).
fn is_bool_type(ty: &Type) -> bool {
    match ty {
        Type::Custom(name) => name == "Bool",
        Type::Applied(name, _) => name == "Bool",
        _ => false,
    }
}
