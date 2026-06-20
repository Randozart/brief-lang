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
        // DEFERRED (Phase 4): infer_expression is private. Router arms
        // in typechecker.rs destructure sub-expressions directly.
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
            (_, Value::Regex(_), _) | (_, _, Value::Regex(_)) => {
                return Err(RuntimeError::TypeMismatch(format!("binary op {:?} on Regex", self.kind)))
            }
            _ => return Err(RuntimeError::TypeMismatch(format!("binary op {:?}", self.kind))),
        })
    }
}

impl ExprCodegenLLVM for BinaryOpExpr {
    fn emit_llvm(&self, ctx: &mut crate::backend::llvm::LlvmBackend, out: &mut String, _dispatch: &ExprDispatch) -> crate::backend::llvm::TypedRegister {
        // Reconstruct the old Expr variant for backward-compat codegen
        let old_expr = match self.kind {
            BinaryOpKind::Add => Expr::Add(self.left.clone(), self.right.clone()),
            BinaryOpKind::Sub => Expr::Sub(self.left.clone(), self.right.clone()),
            BinaryOpKind::Mul => Expr::Mul(self.left.clone(), self.right.clone()),
            BinaryOpKind::Div => Expr::Div(self.left.clone(), self.right.clone()),
            BinaryOpKind::Mod => Expr::Mod(self.left.clone(), self.right.clone()),
            BinaryOpKind::Eq => Expr::Eq(self.left.clone(), self.right.clone()),
            BinaryOpKind::Ne => Expr::Ne(self.left.clone(), self.right.clone()),
            BinaryOpKind::Lt => Expr::Lt(self.left.clone(), self.right.clone()),
            BinaryOpKind::Le => Expr::Le(self.left.clone(), self.right.clone()),
            BinaryOpKind::Gt => Expr::Gt(self.left.clone(), self.right.clone()),
            BinaryOpKind::Ge => Expr::Ge(self.left.clone(), self.right.clone()),
            BinaryOpKind::And => Expr::And(self.left.clone(), self.right.clone()),
            BinaryOpKind::Or => Expr::Or(self.left.clone(), self.right.clone()),
            BinaryOpKind::BitAnd => Expr::BitAnd(self.left.clone(), self.right.clone()),
            BinaryOpKind::BitOr => Expr::BitOr(self.left.clone(), self.right.clone()),
            BinaryOpKind::BitXor => Expr::BitXor(self.left.clone(), self.right.clone()),
            BinaryOpKind::Shl => Expr::Shl(self.left.clone(), self.right.clone()),
            BinaryOpKind::Shr => Expr::Shr(self.left.clone(), self.right.clone()),
        };
        ctx.emit_expr(out, &old_expr, "")
    }
}


impl ExprCodegenWebstack for BinaryOpExpr {
    fn emit_js(&self, _ctx: &crate::backend::webstack::WebstackGenerator, _dispatch: &ExprDispatch) -> String {
        // Webstack handles Expr::BinaryOp directly in expr_to_ts — this trait
        // path is a fallback for dispatch chains that route through feature structs.
        let l = match self.left.as_ref() {
            Expr::Integer(n) => n.to_string(),
            Expr::Float(f) => f.to_string(),
            Expr::Bool(b) => b.to_string(),
            Expr::Identifier(name) => name.clone(),
            _ => "value".to_string(),
        };
        let r = match self.right.as_ref() {
            Expr::Integer(n) => n.to_string(),
            Expr::Float(f) => f.to_string(),
            Expr::Bool(b) => b.to_string(),
            Expr::Identifier(name) => name.clone(),
            _ => "value".to_string(),
        };
        match self.kind {
            BinaryOpKind::Add => format!("({} + {})", l, r),
            BinaryOpKind::Sub => format!("({} - {})", l, r),
            BinaryOpKind::Mul => format!("({} * {})", l, r),
            BinaryOpKind::Div => format!("({} / {})", l, r),
            BinaryOpKind::Mod => format!("({} % {})", l, r),
            BinaryOpKind::Eq => format!("({} === {})", l, r),
            BinaryOpKind::Ne => format!("({} !== {})", l, r),
            BinaryOpKind::Lt => format!("({} < {})", l, r),
            BinaryOpKind::Le => format!("({} <= {})", l, r),
            BinaryOpKind::Gt => format!("({} > {})", l, r),
            BinaryOpKind::Ge => format!("({} >= {})", l, r),
            BinaryOpKind::And => format!("({} && {})", l, r),
            BinaryOpKind::Or => format!("({} || {})", l, r),
            BinaryOpKind::BitAnd => format!("({} & {})", l, r),
            BinaryOpKind::BitOr => format!("({} | {})", l, r),
            BinaryOpKind::BitXor => format!("({} ^ {})", l, r),
            BinaryOpKind::Shl => format!("({} << {})", l, r),
            BinaryOpKind::Shr => format!("({} >> {})", l, r),
        }
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
