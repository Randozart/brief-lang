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

impl ExprCodegenLLVM for DbvlTableExpr { fn emit_llvm(&self, _: &mut crate::backend::llvm::LlvmBackend, _: &mut String, _: &ExprDispatch) -> crate::backend::llvm::TypedRegister { crate::backend::llvm::TypedRegister { name: "%dbvl".into(), ty: Type::Void } } }
impl ExprCodegenVHDL for DbvlTableExpr { fn emit_vhdl(&self, _: &crate::backend::vhdl::VhdlGenerator, _: &ExprDispatch) -> String { "'0'".into() } }
impl ExprCodegenWebstack for DbvlTableExpr { fn emit_js(&self, _: &crate::backend::webstack::WebstackGenerator, _: &ExprDispatch) -> String { "JsValue::TRUE".into() } }
