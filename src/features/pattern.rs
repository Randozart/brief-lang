use crate::ast::{Expr, MatchArm, Pattern, Type};
use crate::features::traits::*;
use crate::interpreter::{Interpreter, RuntimeError, Value};
use crate::typechecker::TypeChecker;

#[derive(Debug, Clone, PartialEq)]
pub struct PatternMatchExpr { pub value: Box<Expr>, pub variant: String, pub fields: Vec<Pattern> }
#[derive(Debug, Clone, PartialEq)]
pub struct MatchExpr { pub value: Box<Expr>, pub arms: Vec<MatchArm> }

macro_rules! stub_impls {
    ($ty:ty) => {
        impl ExprTypecheck for $ty { fn typecheck(&self, _: &mut TypeChecker, _: &ExprDispatch) -> Result<Type, crate::errors::TypeError> { Ok(Type::Void) } }
        impl ExprEval for $ty { fn evaluate(&self, _: &mut Interpreter, _: &ExprDispatch) -> Result<Value, RuntimeError> { Err(RuntimeError::TypeMismatch(String::new())) } }
        impl ExprCodegenLLVM for $ty { fn emit_llvm(&self, _: &mut crate::backend::llvm::LlvmBackend, _: &mut String, _: &ExprDispatch) -> crate::backend::llvm::TypedRegister { crate::backend::llvm::TypedRegister { name: "%pat".into(), ty: Type::Void } } }
        impl ExprCodegenVHDL for $ty { fn emit_vhdl(&self, _: &crate::backend::vhdl::VhdlGenerator, _: &ExprDispatch) -> String { "'0'".into() } }
        impl ExprCodegenWebstack for $ty { fn emit_js(&self, _: &crate::backend::webstack::WebstackGenerator, _: &ExprDispatch) -> String { "JsValue::TRUE".into() } }
    };
}
stub_impls!(PatternMatchExpr); stub_impls!(MatchExpr);

#[cfg(all(kani, feature = "kani_full"))]
mod kani_full_tests {
    use super::*;
    #[kani::proof]
    fn verify_pattern_match_construct() { let _ = PatternMatchExpr { value: Box::new(Expr::Integer(0)), variant: "Ok".into(), fields: vec![] }; }
    #[kani::proof]
    fn verify_match_expr_construct() { let _ = MatchExpr { value: Box::new(Expr::Integer(0)), arms: vec![] }; }
}
