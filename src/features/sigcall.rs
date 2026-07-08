use crate::ast::{Expr, Type};
use crate::features::traits::*;
use crate::interpreter::{Interpreter, RuntimeError, Value};
use crate::typechecker::TypeChecker;

#[derive(Debug, Clone, PartialEq)]
pub struct SigCallExpr {
    pub modifier: crate::ast::SigModifier,
    pub expr: Box<Expr>,
}

impl ExprTypecheck for SigCallExpr {
    fn typecheck(&self, _ctx: &mut TypeChecker, _dispatch: &ExprDispatch) -> Result<Type, crate::errors::TypeError> { Ok(Type::Custom("Int".to_string())) }
}

impl ExprEval for SigCallExpr {
    fn evaluate(&self, ctx: &mut Interpreter, _dispatch: &ExprDispatch) -> Result<Value, RuntimeError> {
        let inner = ctx.eval_expr(&self.expr)?;
        Ok(inner)
    }
}

impl ExprCodegenLLVM for SigCallExpr { fn emit_llvm(&self, _ctx: &mut crate::backend::llvm::LlvmBackend, _out: &mut String,
        _builder: &mut crate::backend::llvm::LLVMBuilder,
        _dispatch: &ExprDispatch,
        _emit_expr: &mut dyn FnMut(
            &mut crate::backend::llvm::LlvmBackend,
            &mut String,
            &mut crate::backend::llvm::LLVMBuilder,
            &crate::ast::Expr,
            &str,
        ) -> crate::backend::llvm::TypedRegister,
    ) -> crate::backend::llvm::TypedRegister { crate::backend::llvm::TypedRegister { name: "%sig".into(), ty: Type::Void } } }
