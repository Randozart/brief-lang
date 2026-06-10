use std::collections::HashMap;
use crate::ast::{Expr, Type};
use crate::features::traits::*;
use crate::interpreter::{Interpreter, RuntimeError, Value};
use crate::typechecker::TypeChecker;

#[derive(Debug, Clone, PartialEq)]
pub struct FieldAccessExpr {
    pub obj: Box<Expr>,
    pub field: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StructInstanceExpr {
    pub typename: String,
    pub fields: Vec<(String, Expr)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ObjectLiteralExpr {
    pub fields: Vec<(String, Expr)>,
}

impl ExprTypecheck for FieldAccessExpr {
    fn typecheck(&self, _ctx: &mut TypeChecker, _dispatch: &ExprDispatch) -> Result<Type, crate::errors::TypeError> { Ok(Type::Int) }
}
impl ExprTypecheck for StructInstanceExpr {
    fn typecheck(&self, _ctx: &mut TypeChecker, _dispatch: &ExprDispatch) -> Result<Type, crate::errors::TypeError> { Ok(Type::Int) }
}
impl ExprTypecheck for ObjectLiteralExpr {
    fn typecheck(&self, _ctx: &mut TypeChecker, _dispatch: &ExprDispatch) -> Result<Type, crate::errors::TypeError> { Ok(Type::Int) }
}

impl ExprEval for FieldAccessExpr {
    fn evaluate(&self, ctx: &mut Interpreter, _dispatch: &ExprDispatch) -> Result<Value, RuntimeError> {
        let obj_val = ctx.eval_expr(&self.obj)?;
        match obj_val {
            Value::Instance { fields, .. } => fields.get(&self.field).cloned()
                .ok_or_else(|| RuntimeError::UndefinedVariable(format!("field '{}'", self.field))),
            _ => Err(RuntimeError::TypeMismatch("field access requires Instance".into())),
        }
    }
}

impl ExprEval for StructInstanceExpr {
    fn evaluate(&self, ctx: &mut Interpreter, _dispatch: &ExprDispatch) -> Result<Value, RuntimeError> {
        let mut instance_fields = HashMap::new();
        for (field_name, field_expr) in &self.fields {
            instance_fields.insert(field_name.clone(), ctx.eval_expr(field_expr)?);
        }
        Ok(Value::Instance { typename: self.typename.clone(), fields: instance_fields })
    }
}

impl ExprEval for ObjectLiteralExpr {
    fn evaluate(&self, ctx: &mut Interpreter, _dispatch: &ExprDispatch) -> Result<Value, RuntimeError> {
        let mut instance_fields = HashMap::new();
        for (field_name, field_expr) in &self.fields {
            instance_fields.insert(field_name.clone(), ctx.eval_expr(field_expr)?);
        }
        Ok(Value::Instance { typename: String::from("ObjectLiteral"), fields: instance_fields })
    }
}

impl ExprCodegenLLVM for FieldAccessExpr {
    fn emit_llvm(&self, ctx: &mut crate::backend::llvm::LlvmBackend, out: &mut String, _dispatch: &ExprDispatch) -> crate::backend::llvm::TypedRegister {
        ctx.emit_expr(out, &Expr::FieldAccess(self.obj.clone(), self.field.clone()), "")
    }
}
impl ExprCodegenLLVM for StructInstanceExpr {
    fn emit_llvm(&self, ctx: &mut crate::backend::llvm::LlvmBackend, out: &mut String, _dispatch: &ExprDispatch) -> crate::backend::llvm::TypedRegister {
        ctx.emit_expr(out, &Expr::StructInstance(self.typename.clone(), self.fields.clone()), "")
    }
}
impl ExprCodegenLLVM for ObjectLiteralExpr {
    fn emit_llvm(&self, ctx: &mut crate::backend::llvm::LlvmBackend, out: &mut String, _dispatch: &ExprDispatch) -> crate::backend::llvm::TypedRegister {
        ctx.emit_expr(out, &Expr::ObjectLiteral(self.fields.clone()), "")
    }
}
impl ExprCodegenVHDL for FieldAccessExpr { fn emit_vhdl(&self, _: &crate::backend::vhdl::VhdlGenerator, _: &ExprDispatch) -> String { "'0'".into() } }
impl ExprCodegenVHDL for StructInstanceExpr { fn emit_vhdl(&self, _: &crate::backend::vhdl::VhdlGenerator, _: &ExprDispatch) -> String { "'0'".into() } }
impl ExprCodegenVHDL for ObjectLiteralExpr { fn emit_vhdl(&self, _: &crate::backend::vhdl::VhdlGenerator, _: &ExprDispatch) -> String { "'0'".into() } }
impl ExprCodegenWebstack for FieldAccessExpr { fn emit_js(&self, _: &crate::backend::webstack::WebstackGenerator, _: &ExprDispatch) -> String { "JsValue::TRUE".into() } }
impl ExprCodegenWebstack for StructInstanceExpr { fn emit_js(&self, _: &crate::backend::webstack::WebstackGenerator, _: &ExprDispatch) -> String { "JsValue::TRUE".into() } }
impl ExprCodegenWebstack for ObjectLiteralExpr { fn emit_js(&self, _: &crate::backend::webstack::WebstackGenerator, _: &ExprDispatch) -> String { "JsValue::TRUE".into() } }
