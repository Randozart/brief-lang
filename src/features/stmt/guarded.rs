use crate::ast::{Expr, Statement, Type};
use crate::errors::TypeError;
use crate::features::traits::*;
use crate::interpreter::{Interpreter, RuntimeError, Value};
use crate::typechecker::TypeChecker;

pub struct GuardedStmt {
    pub condition: Expr,
    pub statements: Vec<Statement>,
}

impl StmtTypecheck for GuardedStmt {
    fn typecheck(&self, _ctx: &mut TypeChecker, _dispatch: &StmtDispatch) -> Result<(), TypeError> { Ok(()) }
}
impl StmtEval for GuardedStmt {
    fn evaluate(&self, _ctx: &mut Interpreter, _dispatch: &StmtDispatch) -> Result<(), RuntimeError> {
        Err(RuntimeError::TypeMismatch("GuardedStmt not yet dispatched".into()))
    }
}
impl StmtCodegenLLVM for GuardedStmt {
    fn emit_llvm(&self, _ctx: &mut crate::backend::llvm::LlvmBackend, _out: &mut String, _dispatch: &StmtDispatch, _indent: &str) {}
}
impl StmtCodegenVHDL for GuardedStmt {
    fn emit_vhdl(&self, _ctx: &mut crate::backend::vhdl::VhdlGenerator, _out: &mut String, _dispatch: &StmtDispatch, _indent: &str) {}
}
impl StmtCodegenWebstack for GuardedStmt {
    fn emit_js(&self, _ctx: &mut crate::backend::webstack::WebstackGenerator, _out: &mut String, _dispatch: &StmtDispatch) {}
}
