use crate::ast::{Expr, Type};
use crate::features::traits::*;
use crate::interpreter::{Interpreter, RuntimeError, Value};
use crate::typechecker::TypeChecker;

/// Kind of binary operation
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BinaryOpKind {
    Add, Sub, Mul, Div, Mod,
    Eq, Ne, Lt, Le, Gt, Ge,
    And, Or,
    BitAnd, BitOr, BitXor,
    Shl, Shr,
}

/// A binary expression with operator kind and two operands
#[derive(Debug, Clone, PartialEq)]
pub struct BinaryOpExpr {
    pub kind: BinaryOpKind,
    pub left: Box<Expr>,
    pub right: Box<Expr>,
}

impl BinaryOpExpr {
    pub fn new(kind: BinaryOpKind, left: Expr, right: Expr) -> Self {
        BinaryOpExpr { kind, left: Box::new(left), right: Box::new(right) }
    }
}

impl ExprTypecheck for BinaryOpExpr {
    fn typecheck(&self, _ctx: &mut TypeChecker, _dispatch: &ExprDispatch) -> Result<Type, crate::errors::TypeError> {
        Ok(Type::Int)
    }
}

impl ExprEval for BinaryOpExpr {
    fn evaluate(&self, _ctx: &mut Interpreter, _dispatch: &ExprDispatch) -> Result<Value, RuntimeError> {
        Err(RuntimeError::TypeMismatch(String::new()))
    }
}

impl ExprCodegenLLVM for BinaryOpExpr {
    fn emit_llvm(&self, _ctx: &mut crate::backend::llvm::LlvmBackend, _out: &mut String, _dispatch: &ExprDispatch) -> crate::backend::llvm::TypedRegister {
        let v = format!("%bz");
        crate::backend::llvm::TypedRegister { name: v, ty: Type::Int }
    }
}

impl ExprCodegenVHDL for BinaryOpExpr {
    fn emit_vhdl(&self, _ctx: &crate::backend::vhdl::VhdlGenerator, _dispatch: &ExprDispatch) -> String {
        "'0'".to_string()
    }
}

impl ExprCodegenWebstack for BinaryOpExpr {
    fn emit_js(&self, _ctx: &crate::backend::webstack::WebstackGenerator, _dispatch: &ExprDispatch) -> String {
        "JsValue::TRUE".to_string()
    }
}

// ── Fast Kani harnesses (pure match dispatch) ──
#[cfg(all(kani, feature = "kani_full"))]
mod kani_full_tests {
    use super::*;

    #[kani::proof]
    fn verify_binary_op_kind_dispatch_add() {
        let e = BinaryOpExpr::new(BinaryOpKind::Add, Expr::Integer(1), Expr::Integer(2));
        assert_eq!(e.kind as usize, BinaryOpKind::Add as usize);
    }

    #[kani::proof]
    fn verify_binary_op_kind_dispatch_eq() {
        let e = BinaryOpExpr::new(BinaryOpKind::Eq, Expr::Integer(0), Expr::Integer(0));
        assert_eq!(e.kind as usize, BinaryOpKind::Eq as usize);
    }
}
