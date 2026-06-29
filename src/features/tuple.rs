use crate::ast::{Expr, Type};
use crate::features::traits::*;
use crate::interpreter::{Interpreter, RuntimeError, Value};
use crate::typechecker::TypeChecker;

#[derive(Debug, Clone, PartialEq)]
pub struct TupleExpr {
    pub exprs: Vec<Expr>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TupleDestructureExpr {
    pub names: Vec<String>,
    pub expr: Box<Expr>,
}

impl ExprTypecheck for TupleExpr {
    fn typecheck(&self, _ctx: &mut TypeChecker, _dispatch: &ExprDispatch) -> Result<Type, crate::errors::TypeError> { Ok(Type::Int) }
}
impl ExprTypecheck for TupleDestructureExpr {
    fn typecheck(&self, _ctx: &mut TypeChecker, _dispatch: &ExprDispatch) -> Result<Type, crate::errors::TypeError> { Ok(Type::Void) }
}

impl ExprEval for TupleExpr {
    fn evaluate(&self, ctx: &mut Interpreter, _dispatch: &ExprDispatch) -> Result<Value, RuntimeError> {
        let mut values = Vec::new();
        for e in &self.exprs { values.push(ctx.eval_expr(e)?); }
        Ok(Value::Tuple(values))
    }
}

impl ExprEval for TupleDestructureExpr {
    fn evaluate(&self, ctx: &mut Interpreter, _dispatch: &ExprDispatch) -> Result<Value, RuntimeError> {
        let value = ctx.eval_expr(&self.expr)?;
        match value {
            Value::Tuple(items) | Value::List(items) => {
                for (i, name) in self.names.iter().enumerate() {
                    if i < items.len() {
                        if name != "_" {
                            ctx.state.insert(name.clone(), items[i].clone());
                        }
                    }
                }
                Ok(Value::Void)
            }
            _ => Err(RuntimeError::TypeMismatch("Tuple destructure requires a list value".into())),
        }
    }
}

impl ExprCodegenLLVM for TupleExpr {
    fn emit_llvm(&self, 
        ctx: &mut crate::backend::llvm::LlvmBackend,
        out: &mut String,
        builder: &mut crate::backend::llvm::LLVMBuilder,
        _dispatch: &ExprDispatch,
        _emit_expr: &mut dyn FnMut(
            &mut crate::backend::llvm::LlvmBackend,
            &mut String,
            &mut crate::backend::llvm::LLVMBuilder,
            &crate::ast::Expr,
            &str,
        ) -> crate::backend::llvm::TypedRegister,
    ) -> crate::backend::llvm::TypedRegister {
        ctx.emit_expr(out, &Expr::Tuple(self.exprs.clone()), "")
    }
}
impl ExprCodegenLLVM for TupleDestructureExpr {
    fn emit_llvm(&self, 
        ctx: &mut crate::backend::llvm::LlvmBackend,
        out: &mut String,
        builder: &mut crate::backend::llvm::LLVMBuilder,
        _dispatch: &ExprDispatch,
        _emit_expr: &mut dyn FnMut(
            &mut crate::backend::llvm::LlvmBackend,
            &mut String,
            &mut crate::backend::llvm::LLVMBuilder,
            &crate::ast::Expr,
            &str,
        ) -> crate::backend::llvm::TypedRegister,
    ) -> crate::backend::llvm::TypedRegister {
        ctx.emit_expr(out, &Expr::TupleDestructure(self.names.clone(), self.expr.clone()), "")
    }
}
impl ExprCodegenWebstack for TupleExpr { fn emit_js(&self, _: &crate::backend::webstack::WebstackGenerator, _: &ExprDispatch) -> String { "JsValue::TRUE".into() } }
impl ExprCodegenWebstack for TupleDestructureExpr { fn emit_js(&self, _: &crate::backend::webstack::WebstackGenerator, _: &ExprDispatch) -> String { "JsValue::TRUE".into() } }
