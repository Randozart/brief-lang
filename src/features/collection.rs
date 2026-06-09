use crate::ast::{BracketOp, Expr, Type};
use crate::features::traits::*;
use crate::interpreter::{Interpreter, RuntimeError, Value};
use crate::typechecker::TypeChecker;

#[derive(Debug, Clone, PartialEq)]
pub struct ListLiteralExpr {
    pub elements: Vec<Expr>,
}

impl ListLiteralExpr {
    pub fn new(elements: Vec<Expr>) -> Self {
        ListLiteralExpr { elements }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct MapLiteralExpr {
    pub entries: Vec<(Expr, Expr)>,
}

impl MapLiteralExpr {
    pub fn new(entries: Vec<(Expr, Expr)>) -> Self {
        MapLiteralExpr { entries }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SetLiteralExpr {
    pub elements: Vec<Expr>,
}

impl SetLiteralExpr {
    pub fn new(elements: Vec<Expr>) -> Self {
        SetLiteralExpr { elements }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ListIndexExpr {
    pub value: Box<Expr>,
    pub index: Box<Expr>,
}

impl ListIndexExpr {
    pub fn new(value: Expr, index: Expr) -> Self {
        ListIndexExpr { value: Box::new(value), index: Box::new(index) }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct SliceExpr {
    pub value: Box<Expr>,
    pub start: Option<Box<Expr>>,
    pub end: Option<Box<Expr>>,
    pub stride: Option<Box<Expr>>,
    pub mask: Option<Box<Expr>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MultiSliceExpr {
    pub value: Box<Expr>,
    pub ops: Vec<BracketOp>,
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
                crate::backend::llvm::TypedRegister { name: "%col".to_string(), ty: Type::Void }
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

stub_impls!(ListLiteralExpr);
stub_impls!(MapLiteralExpr);
stub_impls!(SetLiteralExpr);
stub_impls!(ListIndexExpr);
stub_impls!(SliceExpr);
stub_impls!(MultiSliceExpr);

#[cfg(all(kani, feature = "kani_full"))]
mod kani_full_tests {
    use super::*;

    #[kani::proof]
    fn verify_list_literal_construct() {
        let e = ListLiteralExpr::new(vec![Expr::Integer(1)]);
        assert_eq!(e.elements.len(), 1);
    }

    #[kani::proof]
    fn verify_list_index_construct() {
        let e = ListIndexExpr::new(Expr::Integer(0), Expr::Integer(1));
        let _ = e;
    }
}
