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

use crate::ast::{Expr, Type, TypeBinding, TypeDef, TypeDefBody};
use crate::errors::TypeError;
use crate::features::traits::*;
use crate::interpreter::{Interpreter, RuntimeError, Value};
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

// ── Tests ───────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    fn make_binding(name: &str, value: Expr) -> TypeBinding {
        TypeBinding {
            name: name.into(),
            params: vec![],
            value: Box::new(value),
            span: None,
        }
    }

    #[test]
    fn test_type_binding_bytes() {
        let binding = make_binding("Bytes", Expr::Integer(8));
        assert_eq!(binding.name, "Bytes");
        assert_eq!(binding.value.as_integer(), Some(8));
    }

    #[test]
    fn test_type_binding_alignment() {
        let binding = make_binding("Alignment", Expr::Integer(4));
        assert_eq!(binding.name, "Alignment");
        assert_eq!(binding.value.as_integer(), Some(4));
    }

    #[test]
    fn test_type_binding_codec() {
        let binding = make_binding("Codec", Expr::String("Utf8".into()));
        assert_eq!(binding.name, "Codec");
        assert_eq!(binding.value.as_string(), Some("Utf8"));
    }

    #[test]
    fn test_type_binding_endian() {
        let binding = make_binding("Endian", Expr::Identifier("Big".into()));
        assert_eq!(binding.name, "Endian");
    }

    #[test]
    fn test_type_def_body_with_bindings() {
        let body = TypeDefBody {
            bindings: vec![
                make_binding("Bytes", Expr::Integer(8)),
                make_binding("Alignment", Expr::Integer(8)),
            ],
            constraints: vec![],
            span: None,
        };
        assert_eq!(body.bindings.len(), 2);
        assert_eq!(body.bindings[0].value.as_integer(), Some(8));
    }

    #[test]
    fn test_type_def_struct_with_bindings() {
        let def = TypeDef {
            name: "U64".into(),
            type_params: vec![],
            base: Box::new(Expr::TypeRef("Bits".into())),
            bit_range: None,
            body: TypeDefBody {
                bindings: vec![
                    make_binding("Bytes", Expr::Integer(8)),
                    make_binding("Alignment", Expr::Integer(8)),
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

    // DEFERRED (D-1): TypeBinding uses Box<Expr> which requires heap
    // allocation. Move to kani_full until TypeBinding is refactored to use
    // a non-heap representation for the primitive kernel.
}

#[cfg(all(kani, feature = "kani_full"))]
mod kani_full_tests {
    use super::*;

    #[kani::proof]
    fn verify_type_binding_bytes_dispatch() {
        let binding = TypeBinding {
            name: "Bytes".into(),
            params: vec![],
            value: Box::new(Expr::Integer(8)),
            span: None,
        };
        match binding.value.as_ref() {
            Expr::Integer(n) => assert_eq!(*n, 8),
            _ => panic!("wrong expr variant"),
        }
    }

    #[kani::proof]
    fn verify_type_binding_alignment_dispatch() {
        let binding = TypeBinding {
            name: "Alignment".into(),
            params: vec![],
            value: Box::new(Expr::Integer(4)),
            span: None,
        };
        match binding.value.as_ref() {
            Expr::Integer(n) => assert_eq!(*n, 4),
            _ => panic!("wrong expr variant"),
        }
    }

    #[kani::proof]
    fn verify_type_binding_codec_dispatch() {
        let binding = TypeBinding {
            name: "Codec".into(),
            params: vec![],
            value: Box::new(Expr::String("Utf8".into())),
            span: None,
        };
        match binding.value.as_ref() {
            Expr::String(s) => assert_eq!(s, "Utf8"),
            _ => panic!("wrong expr variant"),
        }
    }

    #[kani::proof]
    fn verify_type_def_body_with_bindings() {
        let body = TypeDefBody {
            bindings: vec![TypeBinding {
                name: "Bytes".into(),
                params: vec![],
                value: Box::new(Expr::Integer(8)),
                span: None,
            }],
            constraints: vec![],
            span: None,
        };
        assert_eq!(body.bindings.len(), 1);
    }
}
