// ── Type Resolution ────────────────────────────────────────────────────
// 2026-07-12: Phase 2.1 — Resolve type definitions to Bits(N).
// Follows the derivation chain until reaching Bits or a known base type.

use crate::ast::{PropertyValue, Type};
use crate::type_universe::{ResolvedType, TypeUniverse};

/// Resolve a Type to its ResolvedType metadata.
/// For built-in types (Custom("Int"), etc.), returns a synthetic ResolvedType.
pub fn resolve_type(universe: &TypeUniverse, ty: &Type) -> Option<ResolvedType> {
    match ty {
        Type::Custom(name) => {
            if let Some(rt) = universe.get(name) {
                return Some(rt.clone());
            }
            // Built-in type fallbacks
            builtin_resolved(name)
        }
        Type::Bits(n) => Some(ResolvedType {
            name: format!("Bits({})", n),
            base: "Bits".into(),
            bytes: *n,
            alignment: (*n).min(8),
            llvm_type: format!("i{}", n * 8),
            properties: std::collections::HashMap::new(),
        }),
        Type::Void => Some(ResolvedType {
            name: "Void".into(),
            base: "Bits".into(),
            bytes: 0,
            alignment: 1,
            llvm_type: "void".into(),
            properties: std::collections::HashMap::new(),
        }),
        Type::Ptr(inner) => {
            let inner_rt = resolve_type(universe, inner).unwrap_or_else(|| ResolvedType {
                name: "unknown".into(),
                base: "Bits".into(),
                bytes: 8,
                alignment: 8,
                llvm_type: "i8".into(),
                properties: std::collections::HashMap::new(),
            });
            Some(ResolvedType {
                name: format!("Ptr<{}>", inner_rt.name),
                base: "Ptr".into(),
                bytes: 8,
                alignment: 8,
                llvm_type: "ptr".into(),
                properties: std::collections::HashMap::new(),
            })
        }
        _ => None,
    }
}

/// Return synthetic ResolvedType for built-in types.
fn builtin_resolved(name: &str) -> Option<ResolvedType> {
    let (bytes, llvm_type) = match name {
        "Int" => (8, "i64"),
        "UInt" => (8, "i64"),
        "Int8" | "i8" => (1, "i8"),
        "Int16" | "i16" => (2, "i16"),
        "Int32" | "i32" => (4, "i32"),
        "Int64" | "i64" => (8, "i64"),
        "UInt8" | "u8" => (1, "i8"),
        "UInt16" | "u16" => (2, "i16"),
        "UInt32" | "u32" => (4, "i32"),
        "UInt64" | "u64" => (8, "i64"),
        "Float" | "Float32" | "f32" | "F32" => (4, "float"),
        "Float64" | "f64" | "F64" | "Double" => (8, "double"),
        "Bool" => (1, "i1"),
        "Char" => (4, "i32"),
        "String" => (24, "%String"),
        "Data" => (8, "i8*"),
        _ => return None,
    };
    Some(ResolvedType {
        name: name.to_string(),
        base: "Bits".to_string(),
        bytes,
        alignment: bytes.min(8),
        llvm_type: llvm_type.to_string(),
        properties: std::collections::HashMap::new(),
    })
}

/// Apply generic type parameters: substitute TypeVar references.
pub fn apply_type_params(params: &[String], args: &[Type], target: &Type) -> Type {
    // Simple substitution: replace TypeVar instances with the corresponding arg.
    // In a full implementation this would traverse the full type tree.
    match target {
        Type::TypeVar(name) => params
            .iter()
            .position(|p| p == name)
            .and_then(|i| args.get(i).cloned())
            .unwrap_or(target.clone()),
        Type::Applied(name, inner) => {
            let inner: Vec<Type> = inner
                .iter()
                .map(|t| apply_type_params(params, args, t))
                .collect();
            Type::Applied(name.clone(), inner)
        }
        Type::Ptr(inner) => Type::ptr(apply_type_params(params, args, inner)),
        _ => target.clone(),
    }
}
