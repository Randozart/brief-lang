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

impl ExprCodegenLLVM for SubtypeProjectionExpr { fn emit_llvm(&self, _ctx: &mut crate::backend::llvm::LlvmBackend, _out: &mut String,
        _builder: &mut crate::backend::llvm::LLVMBuilder,
        _dispatch: &ExprDispatch,
        _emit_expr: &mut dyn FnMut(
            &mut crate::backend::llvm::LlvmBackend,
            &mut String,
            &mut crate::backend::llvm::LLVMBuilder,
            &crate::ast::Expr,
            &str,
        ) -> crate::backend::llvm::TypedRegister,
    ) -> crate::backend::llvm::TypedRegister { crate::backend::llvm::TypedRegister { name: "%sub".into(), ty: Type::Void } } }
