use crate::ast::{Expr, Statement, Type};
use crate::features::traits::*;
use crate::interpreter::{Interpreter, RuntimeError, Value};
use crate::typechecker::TypeChecker;

#[derive(Debug, Clone, PartialEq)]
pub struct BlockExpr { pub statements: Vec<Statement>, pub final_expr: Box<Expr> }
impl BlockExpr { pub fn new(s: Vec<Statement>, e: Expr) -> Self { BlockExpr { statements: s, final_expr: Box::new(e) } } }

impl ExprTypecheck for BlockExpr { fn typecheck(&self, _: &mut TypeChecker, _: &ExprDispatch) -> Result<Type, crate::errors::TypeError> { Ok(Type::Void) } }
impl ExprEval for BlockExpr { fn evaluate(&self, _: &mut Interpreter, _: &ExprDispatch) -> Result<Value, RuntimeError> { Err(RuntimeError::TypeMismatch(String::new())) } }
impl ExprCodegenLLVM for BlockExpr { fn emit_llvm(&self, _: &mut crate::backend::llvm::LlvmBackend, _: &mut String, _: &ExprDispatch) -> crate::backend::llvm::TypedRegister { crate::backend::llvm::TypedRegister { name: "%blk".into(), ty: Type::Void } } }
impl ExprCodegenVHDL for BlockExpr { fn emit_vhdl(&self, _: &crate::backend::vhdl::VhdlGenerator, _: &ExprDispatch) -> String { "'0'".into() } }
impl ExprCodegenWebstack for BlockExpr { fn emit_js(&self, _: &crate::backend::webstack::WebstackGenerator, _: &ExprDispatch) -> String { "JsValue::TRUE".into() } }

#[cfg(all(kani, feature = "kani_full"))]
mod kani_full_tests {
    use super::*;
    #[kani::proof]
    fn verify_block_expr_construct() { let _ = BlockExpr::new(vec![], Expr::Term); }
}
