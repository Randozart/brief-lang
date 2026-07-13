// ── LLVM Type Lowering ─────────────────────────────────────────────────
// 2026-07-12: Phase 2.6/4 — Lower Brief types to LLVM IR type strings.
// Consults the llvm metadata property, falls back to iN based on byte width.

use crate::ast_new::Type;
use crate::type_universe::resolve_type;

/// Lower a Brief Type to an LLVM IR type string.
pub fn lower_type(ty: &Type) -> String {
    match ty {
        Type::Custom(name) => lower_custom_type(name),
        Type::Applied(name, _) => lower_custom_type(name),
        Type::Bits(n) => format!("i{}", n * 8),
        Type::Void => "void".into(),
        Type::Ptr(_) => "ptr".into(),
        Type::Tuple(types) => {
            let inner: Vec<String> = types.iter().map(lower_type).collect();
            format!("{{ {} }}", inner.join(", "))
        }
        Type::Function(params, ret) => {
            let param_strs: Vec<String> = params.iter().map(lower_type).collect();
            format!("{} ({})", lower_type(ret), param_strs.join(", "))
        }
        _ => "i64".into(),
    }
}

/// Lower a custom named type.
fn lower_custom_type(name: &str) -> String {
    match name {
        "Int" | "UInt" | "Int64" | "UInt64" => "i64",
        "Int32" | "UInt32" => "i32",
        "Int16" | "UInt16" => "i16",
        "Int8" | "UInt8" | "Bool" => "i8",
        "Float" | "Float32" => "float",
        "Float64" | "Double" => "double",
        "String" | "Data" => "ptr",
        "Char" => "i32",
        "Ptr" => "ptr",
        _ => "i64",
    }
    .into()
}

/// Get the byte size of a type in the LLVM ABI.
pub fn type_size(ty: &Type) -> u64 {
    match ty {
        Type::Custom(name) => match name.as_str() {
            "Int" | "UInt" | "Float64" | "Double" | "Int64" | "UInt64" => 8,
            "Float" | "Float32" | "Int32" | "UInt32" | "Char" => 4,
            "Int16" | "UInt16" => 2,
            "Bool" | "Int8" | "UInt8" => 1,
            "String" | "Data" => 8,
            "Ptr" => 8,
            _ => 8,
        },
        Type::Bits(n) => *n,
        Type::Ptr(_) => 8,
        Type::Void => 0,
        _ => 8,
    }
}

/// Get the alignment of a type in bytes.
pub fn type_alignment(ty: &Type) -> u64 {
    type_size(ty).min(8)
}
