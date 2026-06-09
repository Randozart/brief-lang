use crate::ast::{ArrowDir, Expr, Type};
use crate::features::traits::*;
use crate::interpreter::{Interpreter, RuntimeError, Value};
use crate::typechecker::TypeChecker;

#[derive(Debug, Clone, PartialEq)]
pub struct ArrowMutExpr { pub dir: ArrowDir, pub target: Box<Expr>, pub index: Box<Expr>, pub value: Option<Box<Expr>> }
#[derive(Debug, Clone, PartialEq)]
pub struct ArrowDiscardExpr { pub target: Box<Expr>, pub index: Box<Expr> }
#[derive(Debug, Clone, PartialEq)]
pub struct ArrowTransferExpr { pub dest: Box<Expr>, pub source: Box<Expr>, pub filter: Option<Box<Expr>> }

macro_rules! stub_impls {
    ($ty:ty) => {
        impl ExprTypecheck for $ty { fn typecheck(&self, _: &mut TypeChecker, _: &ExprDispatch) -> Result<Type, crate::errors::TypeError> { Ok(Type::Void) } }
        impl ExprEval for $ty { fn evaluate(&self, _: &mut Interpreter, _: &ExprDispatch) -> Result<Value, RuntimeError> { Err(RuntimeError::TypeMismatch(String::new())) } }
        impl ExprCodegenLLVM for $ty { fn emit_llvm(&self, _: &mut crate::backend::llvm::LlvmBackend, _: &mut String, _: &ExprDispatch) -> crate::backend::llvm::TypedRegister { crate::backend::llvm::TypedRegister { name: "%arr".into(), ty: Type::Void } } }
        impl ExprCodegenVHDL for $ty { fn emit_vhdl(&self, _: &crate::backend::vhdl::VhdlGenerator, _: &ExprDispatch) -> String { "'0'".into() } }
        impl ExprCodegenWebstack for $ty { fn emit_js(&self, _: &crate::backend::webstack::WebstackGenerator, _: &ExprDispatch) -> String { "JsValue::TRUE".into() } }
    };
}
stub_impls!(ArrowMutExpr); stub_impls!(ArrowDiscardExpr); stub_impls!(ArrowTransferExpr);

#[cfg(all(kani, feature = "kani_full"))]
mod kani_full_tests {
    use super::*;
    #[kani::proof]
    fn verify_arrow_mut_construct() {
        let e = ArrowMutExpr { dir: ArrowDir::Push, target: Box::new(Expr::Integer(0)), index: Box::new(Expr::Term), value: None };
        assert_eq!(e.dir, ArrowDir::Push);
    }
}
