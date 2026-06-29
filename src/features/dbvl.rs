use std::sync::Arc;
use std::collections::HashMap;
use crate::ast::{Expr, Type};
use crate::features::traits::*;
use crate::interpreter::{DbvlTableInner, Interpreter, RuntimeError, Value};
use crate::typechecker::TypeChecker;

#[derive(Debug, Clone, PartialEq)]
pub struct DbvlTableExpr {
    pub path: String,
    pub field_names: Vec<String>,
    pub key_offsets: HashMap<String, Vec<usize>>,
    pub schema_name: Option<String>,
}

impl ExprTypecheck for DbvlTableExpr { fn typecheck(&self, _: &mut TypeChecker, _: &ExprDispatch) -> Result<Type, crate::errors::TypeError> { Ok(Type::Int) } }

impl ExprEval for DbvlTableExpr {
    fn evaluate(&self, _ctx: &mut Interpreter, _dispatch: &ExprDispatch) -> Result<Value, RuntimeError> {
        Ok(Value::DbvlTable(Arc::new(DbvlTableInner {
            path: self.path.clone(),
            key_offsets: self.key_offsets.clone(),
            field_names: self.field_names.clone(),
            schema_name: self.schema_name.clone(),
            schema_key_index: Some(0),
        })))
    }
}

impl ExprCodegenLLVM for DbvlTableExpr { fn emit_llvm(&self, _ctx: &mut crate::backend::llvm::LlvmBackend, _out: &mut String,
        _builder: &mut crate::backend::llvm::LLVMBuilder,
        _dispatch: &ExprDispatch,
        _emit_expr: &mut dyn FnMut(
            &mut crate::backend::llvm::LlvmBackend,
            &mut String,
            &mut crate::backend::llvm::LLVMBuilder,
            &crate::ast::Expr,
            &str,
        ) -> crate::backend::llvm::TypedRegister,
    ) -> crate::backend::llvm::TypedRegister { crate::backend::llvm::TypedRegister { name: "%dbvl".into(), ty: Type::Void } } }
