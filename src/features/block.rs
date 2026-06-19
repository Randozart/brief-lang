use crate::ast::{Expr, Type};
use crate::features::traits::*;
use crate::interpreter::{Interpreter, RuntimeError, Value};
use crate::typechecker::TypeChecker;

#[derive(Debug, Clone, PartialEq)]
pub struct BlockExpr {
    pub stmts: Vec<crate::ast::Statement>,
    pub last: Box<Expr>,
}

impl ExprTypecheck for BlockExpr {
    fn typecheck(&self, _ctx: &mut TypeChecker, _dispatch: &ExprDispatch) -> Result<Type, crate::errors::TypeError> {
        Ok(Type::Int)
    }
}

impl ExprEval for BlockExpr {
    fn evaluate(&self, ctx: &mut Interpreter, _dispatch: &ExprDispatch) -> Result<Value, RuntimeError> {
        let old_state = ctx.state.clone();
        for stmt in &self.stmts {
            ctx.exec_stmt(stmt)?;
        }
        let result = ctx.eval_expr(&self.last)?;
        ctx.state = old_state;
        Ok(result)
    }
}

impl ExprCodegenLLVM for BlockExpr {
    fn emit_llvm(&self, _ctx: &mut crate::backend::llvm::LlvmBackend, _out: &mut String, _dispatch: &ExprDispatch) -> crate::backend::llvm::TypedRegister {
        crate::backend::llvm::TypedRegister { name: "%blk".into(), ty: Type::Void }
    }
}


impl ExprCodegenWebstack for BlockExpr {
    fn emit_js(&self, _ctx: &crate::backend::webstack::WebstackGenerator, _dispatch: &ExprDispatch) -> String {
        "JsValue::TRUE".to_string()
    }
}
