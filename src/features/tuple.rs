use crate::ast::{Expr, Type};
use crate::features::traits::*;
use crate::interpreter::{Interpreter, RuntimeError, Value};
use crate::typechecker::TypeChecker;

#[derive(Debug, Clone, PartialEq)]
pub struct TupleExpr {
    pub elements: Vec<Expr>,
}

impl TupleExpr {
    pub fn new(elements: Vec<Expr>) -> Self {
        TupleExpr { elements }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TupleDestructureExpr {
    pub names: Vec<String>,
    pub source: Box<Expr>,
}

impl TupleDestructureExpr {
    pub fn new(names: Vec<String>, source: Expr) -> Self {
        TupleDestructureExpr { names, source: Box::new(source) }
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
                crate::backend::llvm::TypedRegister { name: "%tup".to_string(), ty: Type::Void }
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

stub_impls!(TupleExpr);
stub_impls!(TupleDestructureExpr);

#[cfg(kani)]
mod kani_tests {
    use super::*;

    #[kani::proof]
    fn verify_tuple_construct() {
        let e = TupleExpr::new(vec![Expr::Integer(1), Expr::Integer(2)]);
        assert_eq!(e.elements.len(), 2);
    }

    #[kani::proof]
    fn verify_tuple_destructure_construct() {
        let e = TupleDestructureExpr::new(vec!["a".to_string()], Expr::Integer(0));
        assert_eq!(e.names.len(), 1);
    }
}
