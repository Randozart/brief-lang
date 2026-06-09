use crate::ast::{Expr, Hashtag, Statement, Type};
use crate::errors::TypeError;
use crate::features::traits::*;
use crate::interpreter::{Interpreter, RuntimeError, Value};
use crate::typechecker::TypeChecker;

pub struct TermStmt {
    pub values: Vec<Option<Expr>>,
    pub swan_song: Option<Box<Statement>>,
    pub modifiers: Vec<Hashtag>,
}

pub struct TermBangStmt {
    pub values: Vec<Option<Expr>>,
    pub swan_song: Option<Box<Statement>>,
    pub modifiers: Vec<Hashtag>,
}

impl StmtTypecheck for TermStmt {
    fn typecheck(&self, _ctx: &mut TypeChecker, _dispatch: &StmtDispatch) -> Result<(), TypeError> { Ok(()) }
}
impl StmtEval for TermStmt {
    fn evaluate(&self, _ctx: &mut Interpreter, _dispatch: &StmtDispatch) -> Result<(), RuntimeError> {
        Err(RuntimeError::TypeMismatch("TermStmt not yet dispatched".into()))
    }
}
impl StmtCodegenLLVM for TermStmt {
    fn emit_llvm(&self, _ctx: &mut crate::backend::llvm::LlvmBackend, _out: &mut String, _dispatch: &StmtDispatch, _indent: &str) {}
}
impl StmtCodegenVHDL for TermStmt {
    fn emit_vhdl(&self, _ctx: &mut crate::backend::vhdl::VhdlGenerator, _out: &mut String, _dispatch: &StmtDispatch, _indent: &str) {}
}
impl StmtCodegenWebstack for TermStmt {
    fn emit_js(&self, _ctx: &mut crate::backend::webstack::WebstackGenerator, _out: &mut String, _dispatch: &StmtDispatch) {}
}

impl StmtTypecheck for TermBangStmt {
    fn typecheck(&self, _ctx: &mut TypeChecker, _dispatch: &StmtDispatch) -> Result<(), TypeError> { Ok(()) }
}
impl StmtEval for TermBangStmt {
    fn evaluate(&self, _ctx: &mut Interpreter, _dispatch: &StmtDispatch) -> Result<(), RuntimeError> {
        Err(RuntimeError::TypeMismatch("TermBangStmt not yet dispatched".into()))
    }
}
impl StmtCodegenLLVM for TermBangStmt {
    fn emit_llvm(&self, _ctx: &mut crate::backend::llvm::LlvmBackend, _out: &mut String, _dispatch: &StmtDispatch, _indent: &str) {}
}
impl StmtCodegenVHDL for TermBangStmt {
    fn emit_vhdl(&self, _ctx: &mut crate::backend::vhdl::VhdlGenerator, _out: &mut String, _dispatch: &StmtDispatch, _indent: &str) {}
}
impl StmtCodegenWebstack for TermBangStmt {
    fn emit_js(&self, _ctx: &mut crate::backend::webstack::WebstackGenerator, _out: &mut String, _dispatch: &StmtDispatch) {}
}
