use crate::ast::{Expr, MatchPattern, Type};
use crate::features::traits::*;
use crate::interpreter::{Interpreter, RuntimeError, Value};
use crate::typechecker::TypeChecker;

#[derive(Debug, Clone, PartialEq)]
pub struct PatternMatchExpr {
    pub value: Box<Expr>,
    pub variant: String,
    pub fields: Vec<crate::ast::Pattern>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatchArm {
    pub pattern: MatchPattern,
    pub guard: Option<Box<Expr>>,
    pub body: Box<Expr>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MatchExpr {
    pub value: Box<Expr>,
    pub arms: Vec<MatchArm>,
}

impl ExprTypecheck for PatternMatchExpr { fn typecheck(&self, _: &mut TypeChecker, _: &ExprDispatch) -> Result<Type, crate::errors::TypeError> { Ok(Type::int()) } }
impl ExprTypecheck for MatchExpr { fn typecheck(&self, _: &mut TypeChecker, _: &ExprDispatch) -> Result<Type, crate::errors::TypeError> { Ok(Type::int()) } }

impl ExprEval for PatternMatchExpr {
    fn evaluate(&self, ctx: &mut Interpreter, _: &ExprDispatch) -> Result<Value, RuntimeError> {
        let matched_value = ctx.eval_expr(&self.value)?;
        match matched_value {
            Value::Enum(_, ref matched_variant, ref enum_fields) if *matched_variant == self.variant => {
                let mut keys: Vec<&String> = enum_fields.keys().collect();
                keys.sort();
                let vals: Vec<&Value> = keys.iter().filter_map(|k| enum_fields.get(*k)).collect();
                let all_matched = self.fields.iter().zip(vals.iter()).all(|(pat, val)| {
                    Interpreter::pattern_match(pat, val, &mut ctx.state)
                });
                Ok(Value::Bool(all_matched))
            }
            _ => Ok(Value::Bool(false)),
        }
    }
}

impl ExprEval for MatchExpr {
    fn evaluate(&self, ctx: &mut Interpreter, _: &ExprDispatch) -> Result<Value, RuntimeError> {
        let target = ctx.eval_expr(&self.value)?;
        for arm in &self.arms {
            let matched = match &arm.pattern {
                MatchPattern::Wildcard => true,
                MatchPattern::Literal(pat) => {
                    Interpreter::pattern_match(pat, &target, &mut ctx.state)
                }
                MatchPattern::Variant { name, fields } => {
                    match &target {
                         Value::Enum(_, variant, enum_fields) if variant == name => {
                            let mut keys: Vec<&String> = enum_fields.keys().collect();
                            keys.sort();
                            let vals: Vec<&Value> = keys.iter().filter_map(|k| enum_fields.get(*k)).collect();
                            !vals.is_empty() && fields.iter().zip(vals.iter()).all(|(pat, val)| {
                                Interpreter::pattern_match(pat, val, &mut ctx.state)
                            })
                        }
                        _ => false,
                    }
                }
            };
            if matched {
                if let Some(guard) = &arm.guard {
                    let guard_val = ctx.eval_expr(guard)?;
                    if guard_val != Value::Bool(true) { continue; }
                }
                return ctx.eval_expr(&arm.body);
            }
        }
        Err(RuntimeError::TypeMismatch("Non-exhaustive match: no arm matched".into()))
    }
}

impl ExprCodegenLLVM for PatternMatchExpr {
    fn emit_llvm(
        &self,
        _ctx: &mut crate::backend::llvm::LlvmBackend,
        _out: &mut String,
        _builder: &mut crate::backend::llvm::LLVMBuilder,
        _dispatch: &ExprDispatch,
        _emit_expr: &mut dyn FnMut(
            &mut crate::backend::llvm::LlvmBackend,
            &mut String,
            &mut crate::backend::llvm::LLVMBuilder,
            &crate::ast::Expr,
            &str,
        ) -> crate::backend::llvm::TypedRegister,
    ) -> crate::backend::llvm::TypedRegister {
        _ctx.emit_expr(_out, &Expr::PatternMatch { value: self.value.clone(), variant: self.variant.clone(), fields: self.fields.clone() }, "")
    }
}
impl ExprCodegenLLVM for MatchExpr { fn emit_llvm(&self, _ctx: &mut crate::backend::llvm::LlvmBackend, _out: &mut String,
        _builder: &mut crate::backend::llvm::LLVMBuilder,
        _dispatch: &ExprDispatch,
        _emit_expr: &mut dyn FnMut(
            &mut crate::backend::llvm::LlvmBackend,
            &mut String,
            &mut crate::backend::llvm::LLVMBuilder,
            &crate::ast::Expr,
            &str,
        ) -> crate::backend::llvm::TypedRegister,
    ) -> crate::backend::llvm::TypedRegister { crate::backend::llvm::TypedRegister { name: "%mtc".into(), ty: Type::Void } } }
impl ExprCodegenWebstack for PatternMatchExpr { fn emit_js(&self, _: &crate::backend::webstack::WebstackGenerator, _: &ExprDispatch) -> String { "JsValue::TRUE".into() } }
impl ExprCodegenWebstack for MatchExpr { fn emit_js(&self, _: &crate::backend::webstack::WebstackGenerator, _: &ExprDispatch) -> String { "JsValue::TRUE".into() } }
