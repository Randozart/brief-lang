use crate::ast::{Expr, Type};
use crate::features::traits::*;
use crate::interpreter::{Interpreter, RuntimeError, Value};
use crate::typechecker::TypeChecker;

#[derive(Debug, Clone, PartialEq)]
pub struct EllipsisExpr;

impl ExprTypecheck for EllipsisExpr { fn typecheck(&self, _: &mut TypeChecker, _: &ExprDispatch) -> Result<Type, crate::errors::TypeError> { Ok(Type::Void) } }

impl ExprEval for EllipsisExpr {
    fn evaluate(&self, _: &mut Interpreter, _: &ExprDispatch) -> Result<Value, RuntimeError> {
        Err(RuntimeError::TypeMismatch("Ellipsis must be resolved at parse time".into()))
    }
}

impl ExprCodegenLLVM for EllipsisExpr { fn emit_llvm(&self, _: &mut crate::backend::llvm::LlvmBackend, _: &mut String, _: &ExprDispatch) -> crate::backend::llvm::TypedRegister { crate::backend::llvm::TypedRegister { name: "%elp".into(), ty: Type::Void } } }
