use crate::ast::{Expr, Type};
use crate::errors::TypeError;
use crate::features::traits::*;
use crate::interpreter::{Interpreter, RuntimeError, Value};
use crate::typechecker::TypeChecker;

pub struct EscapeStmt(pub Option<Expr>);

impl StmtTypecheck for EscapeStmt {
    fn typecheck(&self, _ctx: &mut TypeChecker, _dispatch: &StmtDispatch) -> Result<(), TypeError> { Ok(()) }
}
impl StmtEval for EscapeStmt {
    fn evaluate(&self, _ctx: &mut Interpreter, _dispatch: &StmtDispatch) -> Result<(), RuntimeError> {
        Err(RuntimeError::TypeMismatch("EscapeStmt not yet dispatched".into()))
    }
}
impl StmtCodegenLLVM for EscapeStmt {
    fn emit_llvm(&self, _ctx: &mut crate::backend::llvm::LlvmBackend, _out: &mut String,
        _builder: &mut crate::backend::llvm::LLVMBuilder,
        _dispatch: &StmtDispatch, _indent: &str,
        _emit_expr: &mut dyn FnMut(
            &mut crate::backend::llvm::LlvmBackend,
            &mut String,
            &mut crate::backend::llvm::LLVMBuilder,
            &crate::ast::Expr,
            &str,
        ) -> crate::backend::llvm::TypedRegister,
    ) {}
}
impl StmtCodegenWebstack for EscapeStmt {
    fn emit_js(&self, _ctx: &mut crate::backend::webstack::WebstackGenerator, _out: &mut String, _dispatch: &StmtDispatch) {}
}
