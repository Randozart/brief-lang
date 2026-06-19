use crate::ast::{Expr, Type};
use crate::errors::Span;
use crate::errors::TypeError;
use crate::features::traits::*;
use crate::interpreter::{Interpreter, RuntimeError, Value};
use crate::typechecker::TypeChecker;

pub struct InlineAsmStmt {
    pub asm_string: String,
    pub clobbers: Vec<String>,
    pub span: Option<Span>,
}

impl StmtTypecheck for InlineAsmStmt {
    fn typecheck(&self, _ctx: &mut TypeChecker, _dispatch: &StmtDispatch) -> Result<(), TypeError> { Ok(()) }
}
impl StmtEval for InlineAsmStmt {
    fn evaluate(&self, _ctx: &mut Interpreter, _dispatch: &StmtDispatch) -> Result<(), RuntimeError> {
        Err(RuntimeError::TypeMismatch("InlineAsmStmt not yet dispatched".into()))
    }
}
impl StmtCodegenLLVM for InlineAsmStmt {
    fn emit_llvm(&self, _ctx: &mut crate::backend::llvm::LlvmBackend, _out: &mut String, _dispatch: &StmtDispatch, _indent: &str) {}
}
impl StmtCodegenWebstack for InlineAsmStmt {
    fn emit_js(&self, _ctx: &mut crate::backend::webstack::WebstackGenerator, _out: &mut String, _dispatch: &StmtDispatch) {}
}
