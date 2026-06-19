// ── TopLevel::TypeDef — Type Derivation System ──────────────────────
//
// Phase 1.5: Introduces `Type Name <: Base { ... }` syntax with a
// primitive kernel of ~10 compiler-known properties (Bytes, Alignment,
// Endian, Volatile, Atomic, ElementType, FixedSize, InsertAt,
// ExtractFrom, AllowIndex, AllowSlice, AllowArrow, Codec).
//
// DESIGN SKETCH (REFACTOR_PLAN.md §Phase 1.5):
//   The Pass 1 Type-Universe passes over all TypeDef declarations and
//   builds a frozen map of resolved type metadata. Pass 2 uses this map
//   for type checking, literal encoding, and access gate validation.
//
// DEFERRED ITEMS (marked DEFERRED in code):
//   - Full codec signature validation (D-2)
//   - InsertAt/ExtractFrom strategy synthesis (heap, circular buffer) (D-3)
//   - Synthetic constraint guards at runtime (D-7)
//   - Volatile/Atomic as pragmas (D-6)
//   - Expression type parameters for generic ordering (D-1)
//
// Current implementation: stubs for all 5 trait impls. TypeDefs are
// collected and resolved by type_universe.rs in Pass 1, then become
// read-only. The interpreter skips them at runtime.

use crate::ast::{Expr, Type, TypeDef, TypeDefBody, TypeProperty};
use crate::errors::TypeError;
use crate::features::traits::*;
use crate::interpreter::{Interpreter, RuntimeError, Value};
use crate::parser::Parser;
use crate::typechecker::TypeChecker;

// ── Stub trait implementations ──────────────────────────────────────
// These are placeholders. Full implementation (resolving metadata,
// validating constraints, synthesizing access gates) lives in
// type_universe.rs and the TypeDef router arms.

impl ExprTypecheck for TypeDef {
    fn typecheck(
        &self,
        ctx: &mut TypeChecker,
        dispatch: &ExprDispatch,
    ) -> Result<Type, TypeError> {
        // DEFERRED (D-1, D-2, D-7): Full type-level validation
        // Current: skip — TypeDefs are validated in Pass 1 (type_universe.rs)
        Ok(Type::Void)
    }
}

impl ExprEval for TypeDef {
    fn evaluate(
        &self,
        ctx: &mut Interpreter,
        dispatch: &ExprDispatch,
    ) -> Result<Value, RuntimeError> {
        // TypeDefs are compile-time only — no runtime representation.
        Err(RuntimeError::TypeMismatch("TypeDef cannot be evaluated at runtime".into()))
    }
}

impl ExprCodegenLLVM for TypeDef {
    fn emit_llvm(
        &self,
        ctx: &mut crate::backend::llvm::LlvmBackend,
        out: &mut String,
        dispatch: &ExprDispatch,
    ) -> crate::backend::llvm::TypedRegister {
        // DEFERRED: Emit layout/alloca from Bytes, Alignment, Endian
        crate::backend::llvm::TypedRegister {
            name: "%void".into(),
            ty: Type::Void,
        }
    }
}


impl ExprCodegenWebstack for TypeDef {
    fn emit_js(
        &self,
        ctx: &crate::backend::webstack::WebstackGenerator,
        dispatch: &ExprDispatch,
    ) -> String {
        // TypeDefs are compile-time only in all backends
        "JsValue::UNDEFINED".into()
    }
}

// ── TypeProperty helper ─────────────────────────────────────────────
// Resolves a TypeProperty to its i64 value if it's a simple integer
// literal. Used by type_universe.rs to extract Bytes, Alignment, etc.
pub fn resolve_property_as_i64(prop: &TypeProperty) -> Option<i64> {
    match prop {
        TypeProperty::Bytes(e)
        | TypeProperty::Alignment(e)
        | TypeProperty::Endian(e)
        | TypeProperty::Volatile(e)
        | TypeProperty::Atomic(e)
        | TypeProperty::ElementType(e)
        | TypeProperty::FixedSize(e)
        | TypeProperty::InsertAt(e)
        | TypeProperty::ExtractFrom(e)
        | TypeProperty::AllowIndex(e)
        | TypeProperty::AllowSlice(e)
        | TypeProperty::AllowArrow(e) => match e.as_ref() {
            Expr::Integer(n) => Some(*n),
            _ => None,
        },
        TypeProperty::Codec(_) => None,
    }
}

// ── Tests ───────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_type_property_bytes_roundtrip() {
        let prop = TypeProperty::Bytes(Box::new(Expr::Integer(8)));
        assert_eq!(resolve_property_as_i64(&prop), Some(8));
    }

    #[test]
    fn test_type_property_alignment_roundtrip() {
        let prop = TypeProperty::Alignment(Box::new(Expr::Integer(4)));
        assert_eq!(resolve_property_as_i64(&prop), Some(4));
    }

    #[test]
    fn test_type_property_endian_expr_only() {
        let prop = TypeProperty::Endian(Box::new(Expr::Identifier("Little".into())));
        // resolve_property_as_i64 returns None for non-integer exprs
        assert_eq!(resolve_property_as_i64(&prop), None);
    }

    #[test]
    fn test_type_property_codec_is_string() {
        let prop = TypeProperty::Codec("Utf8".into());
        match prop {
            TypeProperty::Codec(name) => assert_eq!(name, "Utf8"),
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_type_def_body_construct() {
        let body = TypeDefBody {
            properties: vec![
                TypeProperty::Bytes(Box::new(Expr::Integer(8))),
                TypeProperty::Alignment(Box::new(Expr::Integer(8))),
            ],
            constraints: vec![],
            span: None,
        };
        assert_eq!(body.properties.len(), 2);
        let bytes = resolve_property_as_i64(&body.properties[0]);
        assert_eq!(bytes, Some(8));
    }

    #[test]
    fn test_type_def_struct_construct() {
        let def = TypeDef {
            name: "U64".into(),
            type_params: vec![],
            base: Box::new(Expr::TypeRef("Bits".into())),
            body: TypeDefBody {
                properties: vec![
                    TypeProperty::Bytes(Box::new(Expr::Integer(8))),
                    TypeProperty::Alignment(Box::new(Expr::Integer(8))),
                ],
                constraints: vec![],
                span: None,
            },
            span: None,
        };
        assert_eq!(def.name, "U64");
    }
}

// ── Kani harnesses ─────────────────────────────────────────────────
// Fast group (kani): no Box::new, no Vec, no String, no formatting, no heap allocation.
// These harnesses use pure match dispatch only.
#[cfg(kani)]
mod kani_tests {
    use super::*;

    // DEFERRED (D-1): TypeProperty dispatch uses Box<Expr> which requires heap
    // allocation. Move to kani_full until TypeProperty is refactored to use
    // a non-heap representation for the primitive kernel.
}

#[cfg(all(kani, feature = "kani_full"))]
mod kani_full_tests {
    use super::*;

    #[kani::proof]
    fn verify_type_property_bytes_dispatch() {
        let prop = TypeProperty::Bytes(Box::new(Expr::Integer(8)));
        match prop {
            TypeProperty::Bytes(ref e) => match e.as_ref() {
                Expr::Integer(n) => assert_eq!(*n, 8),
                _ => panic!("wrong expr variant"),
            },
            _ => panic!("wrong property variant"),
        }
    }

    #[kani::proof]
    fn verify_type_property_alignment_dispatch() {
        let prop = TypeProperty::Alignment(Box::new(Expr::Integer(4)));
        match prop {
            TypeProperty::Alignment(ref e) => match e.as_ref() {
                Expr::Integer(n) => assert_eq!(*n, 4),
                _ => panic!("wrong expr variant"),
            },
            _ => panic!("wrong property variant"),
        }
    }

    #[kani::proof]
    fn verify_type_property_codec_dispatch() {
        let prop = TypeProperty::Codec("Utf8".into());
        match prop {
            TypeProperty::Codec(ref name) => assert_eq!(name, "Utf8"),
            _ => panic!("wrong property variant"),
        }
    }

    #[kani::proof]
    fn verify_type_def_body_construct() {
        let body = TypeDefBody {
            properties: vec![TypeProperty::Bytes(Box::new(Expr::Integer(8)))],
            constraints: vec![],
            span: None,
        };
        assert_eq!(body.properties.len(), 1);
    }
}
