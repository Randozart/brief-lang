use crate::ast::{Expr, Statement, Type};
use crate::errors::Span;
use crate::errors::TypeError;
use crate::features::traits::*;
use crate::interpreter::{Interpreter, RuntimeError, Value};
use crate::typechecker::TypeChecker;

pub struct OnExitStmt {
    pub body: Vec<Statement>,
    pub span: Option<Span>,
}

impl StmtTypecheck for OnExitStmt {
    fn typecheck(&self, _ctx: &mut TypeChecker, _dispatch: &StmtDispatch) -> Result<(), TypeError> { Ok(()) }
}
impl StmtEval for OnExitStmt {
    fn evaluate(&self, _ctx: &mut Interpreter, _dispatch: &StmtDispatch) -> Result<(), RuntimeError> {
        Err(RuntimeError::TypeMismatch("OnExitStmt not yet dispatched".into()))
    }
}
impl StmtCodegenLLVM for OnExitStmt {
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
impl StmtCodegenWebstack for OnExitStmt {
    fn emit_js(&self, _ctx: &mut crate::backend::webstack::WebstackGenerator, _out: &mut String, _dispatch: &StmtDispatch) {}
}
