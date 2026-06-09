use crate::ast::{Expr, Type};
use crate::errors::TypeError;
use crate::features::traits::*;
use crate::interpreter::{Interpreter, RuntimeError, Value};
use crate::typechecker::TypeChecker;

pub struct ExpressionStmt(pub Expr);

impl StmtTypecheck for ExpressionStmt {
    fn typecheck(&self, _ctx: &mut TypeChecker, _dispatch: &StmtDispatch) -> Result<(), TypeError> { Ok(()) }
}
impl StmtEval for ExpressionStmt {
    fn evaluate(&self, _ctx: &mut Interpreter, _dispatch: &StmtDispatch) -> Result<(), RuntimeError> {
        Err(RuntimeError::TypeMismatch("ExpressionStmt not yet dispatched".into()))
    }
}
impl StmtCodegenLLVM for ExpressionStmt {
    fn emit_llvm(&self, _ctx: &mut crate::backend::llvm::LlvmBackend, _out: &mut String, _dispatch: &StmtDispatch, _indent: &str) {}
}
impl StmtCodegenVHDL for ExpressionStmt {
    fn emit_vhdl(&self, _ctx: &mut crate::backend::vhdl::VhdlGenerator, _out: &mut String, _dispatch: &StmtDispatch, _indent: &str) {}
}
impl StmtCodegenWebstack for ExpressionStmt {
    fn emit_js(&self, _ctx: &mut crate::backend::webstack::WebstackGenerator, _out: &mut String, _dispatch: &StmtDispatch) {}
}
