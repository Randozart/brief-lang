use crate::ast::{Expr, Pattern, Type};
use crate::errors::TypeError;
use crate::features::traits::*;
use crate::interpreter::{Interpreter, RuntimeError, Value};
use crate::typechecker::TypeChecker;

pub struct UnificationStmt {
    pub name: String,
    pub variant: String,
    pub fields: Vec<Pattern>,
    pub expr: Expr,
}

impl StmtTypecheck for UnificationStmt {
    fn typecheck(&self, _ctx: &mut TypeChecker, _dispatch: &StmtDispatch) -> Result<(), TypeError> { Ok(()) }
}
impl StmtEval for UnificationStmt {
    fn evaluate(&self, _ctx: &mut Interpreter, _dispatch: &StmtDispatch) -> Result<(), RuntimeError> {
        Err(RuntimeError::TypeMismatch("UnificationStmt not yet dispatched".into()))
    }
}
impl StmtCodegenLLVM for UnificationStmt {
    fn emit_llvm(&self, _ctx: &mut crate::backend::llvm::LlvmBackend, _out: &mut String, _dispatch: &StmtDispatch, _indent: &str) {}
}
impl StmtCodegenWebstack for UnificationStmt {
    fn emit_js(&self, _ctx: &mut crate::backend::webstack::WebstackGenerator, _out: &mut String, _dispatch: &StmtDispatch) {}
}
