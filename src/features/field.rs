use crate::ast::{Expr, Type};
use crate::features::traits::*;
use crate::interpreter::{Interpreter, RuntimeError, Value};
use crate::typechecker::TypeChecker;

#[derive(Debug, Clone, PartialEq)]
pub struct FieldAccessExpr {
    pub object: Box<Expr>,
    pub field: String,
}

impl FieldAccessExpr {
    pub fn new(object: Expr, field: String) -> Self {
        FieldAccessExpr { object: Box::new(object), field }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct StructInstanceExpr {
    pub name: String,
    pub fields: Vec<(String, Expr)>,
}

impl StructInstanceExpr {
    pub fn new(name: String, fields: Vec<(String, Expr)>) -> Self {
        StructInstanceExpr { name, fields }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ObjectLiteralExpr {
    pub fields: Vec<(String, Expr)>,
}

impl ObjectLiteralExpr {
    pub fn new(fields: Vec<(String, Expr)>) -> Self {
        ObjectLiteralExpr { fields }
    }
}

macro_rules! stub_impls {
    ($ty:ty) => {
        impl ExprTypecheck for $ty {
            fn typecheck(&self, _ctx: &mut TypeChecker, _dispatch: &ExprDispatch) -> Result<Type, crate::errors::TypeError> {
                Ok(Type::Void)
            }
        }
        impl ExprEval for $ty {
            fn evaluate(&self, _ctx: &mut Interpreter, _dispatch: &ExprDispatch) -> Result<Value, RuntimeError> {
                Err(RuntimeError::TypeMismatch(String::new()))
            }
        }
        impl ExprCodegenLLVM for $ty {
            fn emit_llvm(&self, _ctx: &mut crate::backend::llvm::LlvmBackend, _out: &mut String, _dispatch: &ExprDispatch) -> crate::backend::llvm::TypedRegister {
                crate::backend::llvm::TypedRegister { name: "%fld".to_string(), ty: Type::Void }
            }
        }
        impl ExprCodegenVHDL for $ty {
            fn emit_vhdl(&self, _ctx: &crate::backend::vhdl::VhdlGenerator, _dispatch: &ExprDispatch) -> String {
                "'0'".to_string()
            }
        }
        impl ExprCodegenWebstack for $ty {
            fn emit_js(&self, _ctx: &crate::backend::webstack::WebstackGenerator, _dispatch: &ExprDispatch) -> String {
                "JsValue::TRUE".to_string()
            }
        }
    };
}

stub_impls!(FieldAccessExpr);
stub_impls!(StructInstanceExpr);
stub_impls!(ObjectLiteralExpr);

#[cfg(all(kani, feature = "kani_full"))]
mod kani_full_tests {
    use super::*;

    #[kani::proof]
    fn verify_field_access_construct() {
        let e = FieldAccessExpr::new(Expr::Integer(0), "x".to_string());
        assert_eq!(e.field, "x");
    }

    #[kani::proof]
    fn verify_struct_instance_construct() {
        let e = StructInstanceExpr::new("Point".to_string(), vec![]);
        assert_eq!(e.name, "Point");
    }
}
