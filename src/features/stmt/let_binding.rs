use crate::ast::{Expr, Type, Type as AstType};
use crate::errors::TypeError;
use crate::features::traits::*;
use crate::interpreter::{Interpreter, RuntimeError, Value};
use crate::typechecker::TypeChecker;

pub struct LetBindingStmt {
    pub name: String,
    pub ty: Option<AstType>,
    pub expr: Option<Expr>,
    pub address: Option<u64>,
    pub address_expr: Option<Box<Expr>>,
    pub bit_range: Option<crate::ast::BitRange>,
    pub is_override: bool,
    pub modifiers: Vec<crate::ast::Hashtag>,
}

impl StmtTypecheck for LetBindingStmt {
    fn typecheck(&self, _ctx: &mut TypeChecker, _dispatch: &StmtDispatch) -> Result<(), TypeError> { Ok(()) }
}
impl StmtEval for LetBindingStmt {
    fn evaluate(&self, _ctx: &mut Interpreter, _dispatch: &StmtDispatch) -> Result<(), RuntimeError> {
        Err(RuntimeError::TypeMismatch("LetBindingStmt not yet dispatched".into()))
    }
}
impl StmtCodegenLLVM for LetBindingStmt {
    fn emit_llvm(&self, _ctx: &mut crate::backend::llvm::LlvmBackend, _out: &mut String, _dispatch: &StmtDispatch, _indent: &str) {}
}
impl StmtCodegenWebstack for LetBindingStmt {
    fn emit_js(&self, _ctx: &mut crate::backend::webstack::WebstackGenerator, _out: &mut String, _dispatch: &StmtDispatch) {}
}
