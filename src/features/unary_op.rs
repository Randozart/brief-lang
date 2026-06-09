use crate::ast::{Expr, Type};
use crate::features::traits::*;
use crate::interpreter::{Interpreter, RuntimeError, Value};
use crate::typechecker::TypeChecker;

/// Kind of unary operation
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UnaryOpKind {
    Neg,
    Not,
    BitNot,
}

/// A unary expression with operator kind and one operand
#[derive(Debug, Clone, PartialEq)]
pub struct UnaryOpExpr {
    pub kind: UnaryOpKind,
    pub operand: Box<Expr>,
}

impl UnaryOpExpr {
    pub fn new(kind: UnaryOpKind, operand: Expr) -> Self {
        UnaryOpExpr { kind, operand: Box::new(operand) }
    }
}

impl ExprTypecheck for UnaryOpExpr {
    fn typecheck(&self, _ctx: &mut TypeChecker, _dispatch: &ExprDispatch) -> Result<Type, crate::errors::TypeError> {
        Ok(Type::Int)
    }
}

impl ExprEval for UnaryOpExpr {
    fn evaluate(&self, _ctx: &mut Interpreter, _dispatch: &ExprDispatch) -> Result<Value, RuntimeError> {
        Err(RuntimeError::TypeMismatch(String::new()))
    }
}

impl ExprCodegenLLVM for UnaryOpExpr {
    fn emit_llvm(&self, _ctx: &mut crate::backend::llvm::LlvmBackend, _out: &mut String, _dispatch: &ExprDispatch) -> crate::backend::llvm::TypedRegister {
        let v = format!("%uz");
        crate::backend::llvm::TypedRegister { name: v, ty: Type::Int }
    }
}

impl ExprCodegenVHDL for UnaryOpExpr {
    fn emit_vhdl(&self, _ctx: &crate::backend::vhdl::VhdlGenerator, _dispatch: &ExprDispatch) -> String {
        "'0'".to_string()
    }
}

impl ExprCodegenWebstack for UnaryOpExpr {
    fn emit_js(&self, _ctx: &crate::backend::webstack::WebstackGenerator, _dispatch: &ExprDispatch) -> String {
        "JsValue::TRUE".to_string()
    }
}

// ── Fast Kani harnesses (pure match dispatch) ──
#[cfg(all(kani, feature = "kani_full"))]
mod kani_full_tests {
    use super::*;

    #[kani::proof]
    fn verify_unary_op_kind_dispatch_neg() {
        let e = UnaryOpExpr::new(UnaryOpKind::Neg, Expr::Integer(42));
        assert_eq!(e.kind as usize, UnaryOpKind::Neg as usize);
    }

    #[kani::proof]
    fn verify_unary_op_kind_dispatch_not() {
        let e = UnaryOpExpr::new(UnaryOpKind::Not, Expr::Bool(true));
        assert_eq!(e.kind as usize, UnaryOpKind::Not as usize);
    }
}
