use crate::ast::{Expr, SubtypeOp, Type};
use crate::features::traits::*;
use crate::interpreter::{Interpreter, RuntimeError, Value};
use crate::typechecker::TypeChecker;

#[derive(Debug, Clone, PartialEq)]
pub struct SubtypeProjectionExpr {
    pub source: Box<Expr>,
    pub ops: Vec<SubtypeOp>,
}

impl ExprTypecheck for SubtypeProjectionExpr {
    fn typecheck(&self, _ctx: &mut TypeChecker, _dispatch: &ExprDispatch) -> Result<Type, crate::errors::TypeError> { Ok(Type::Int) }
}

impl ExprEval for SubtypeProjectionExpr {
    fn evaluate(&self, ctx: &mut Interpreter, _dispatch: &ExprDispatch) -> Result<Value, RuntimeError> {
        let source_val = ctx.eval_expr(&self.source)?;
        ctx.eval_subtype_projection(source_val, &self.ops)
    }
}

impl ExprCodegenLLVM for SubtypeProjectionExpr { fn emit_llvm(&self, _: &mut crate::backend::llvm::LlvmBackend, _: &mut String, _: &ExprDispatch) -> crate::backend::llvm::TypedRegister { crate::backend::llvm::TypedRegister { name: "%sub".into(), ty: Type::Void } } }
impl ExprCodegenVHDL for SubtypeProjectionExpr { fn emit_vhdl(&self, _: &crate::backend::vhdl::VhdlGenerator, _: &ExprDispatch) -> String { "'0'".into() } }
impl ExprCodegenWebstack for SubtypeProjectionExpr { fn emit_js(&self, _: &crate::backend::webstack::WebstackGenerator, _: &ExprDispatch) -> String { "JsValue::TRUE".into() } }
