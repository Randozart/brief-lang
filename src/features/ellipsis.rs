use crate::ast::Type;
use crate::features::traits::*;
use crate::interpreter::{Interpreter, RuntimeError, Value};
use crate::typechecker::TypeChecker;

#[derive(Debug, Clone, PartialEq)]
pub struct EllipsisExpr;

impl ExprTypecheck for EllipsisExpr { fn typecheck(&self, _: &mut TypeChecker, _: &ExprDispatch) -> Result<Type, crate::errors::TypeError> { Ok(Type::Void) } }
impl ExprEval for EllipsisExpr { fn evaluate(&self, _: &mut Interpreter, _: &ExprDispatch) -> Result<Value, RuntimeError> { Err(RuntimeError::TypeMismatch(String::new())) } }
impl ExprCodegenLLVM for EllipsisExpr { fn emit_llvm(&self, _: &mut crate::backend::llvm::LlvmBackend, _: &mut String, _: &ExprDispatch) -> crate::backend::llvm::TypedRegister { crate::backend::llvm::TypedRegister { name: "%elp".into(), ty: Type::Void } } }
impl ExprCodegenVHDL for EllipsisExpr { fn emit_vhdl(&self, _: &crate::backend::vhdl::VhdlGenerator, _: &ExprDispatch) -> String { "'0'".into() } }
impl ExprCodegenWebstack for EllipsisExpr { fn emit_js(&self, _: &crate::backend::webstack::WebstackGenerator, _: &ExprDispatch) -> String { "JsValue::TRUE".into() } }

#[cfg(all(kani, feature = "kani_full"))]
mod kani_full_tests {
    use super::*;
    #[kani::proof]
    fn verify_ellipsis_construct() { let _ = EllipsisExpr; }
}
