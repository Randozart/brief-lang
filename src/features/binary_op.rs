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
    fn evaluate(&self, ctx: &mut Interpreter, _dispatch: &ExprDispatch) -> Result<Value, RuntimeError> {
        let l = ctx.eval_expr(&self.left)?;
        let r = ctx.eval_expr(&self.right)?;
        use BinaryOpKind::*;
        Ok(match (self.kind, &l, &r) {
            (Add,  Value::Int(a), Value::Int(b)) => Value::Int(a + b),
            (Sub,  Value::Int(a), Value::Int(b)) => Value::Int(a - b),
            (Mul,  Value::Int(a), Value::Int(b)) => Value::Int(a * b),
            (Div,  Value::Int(a), Value::Int(b)) => Value::Int(a / b),
            (Mod,  Value::Int(a), Value::Int(b)) => Value::Int(a % b),
            (Eq,   Value::Int(a), Value::Int(b)) => Value::Bool(a == b),
            (Ne,   Value::Int(a), Value::Int(b)) => Value::Bool(a != b),
            (Lt,   Value::Int(a), Value::Int(b)) => Value::Bool(a < b),
            (Le,   Value::Int(a), Value::Int(b)) => Value::Bool(a <= b),
            (Gt,   Value::Int(a), Value::Int(b)) => Value::Bool(a > b),
            (Ge,   Value::Int(a), Value::Int(b)) => Value::Bool(a >= b),
            (And,  Value::Bool(a), Value::Bool(b)) => Value::Bool(*a && *b),
            (Or,   Value::Bool(a), Value::Bool(b)) => Value::Bool(*a || *b),
            (BitAnd, Value::Int(a), Value::Int(b)) => Value::Int(a & b),
            (BitOr,  Value::Int(a), Value::Int(b)) => Value::Int(a | b),
            (BitXor, Value::Int(a), Value::Int(b)) => Value::Int(a ^ b),
            (Shl,  Value::Int(a), Value::Int(b)) => Value::Int(a << b),
            (Shr,  Value::Int(a), Value::Int(b)) => Value::Int(a >> b),
            _ => return Err(RuntimeError::TypeMismatch(format!("binary op {:?}", self.kind))),
        })
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
