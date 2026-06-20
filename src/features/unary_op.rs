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
    fn evaluate(&self, ctx: &mut Interpreter, _dispatch: &ExprDispatch) -> Result<Value, RuntimeError> {
        let v = ctx.eval_expr(&self.operand)?;
        use UnaryOpKind::*;
        Ok(match (self.kind, v) {
            (Neg,    Value::Int(a)) => Value::Int(-a),
            (Not,    Value::Bool(a)) => Value::Bool(!a),
            (BitNot, Value::Int(a)) => Value::Int(!a),
            (_, Value::Regex(_)) => {
                return Err(RuntimeError::TypeMismatch(format!("unary op {:?} on Regex", self.kind)))
            }
            _ => return Err(RuntimeError::TypeMismatch(format!("unary op {:?}", self.kind))),
        })
    }
}

impl ExprCodegenLLVM for UnaryOpExpr {
    fn emit_llvm(&self, ctx: &mut crate::backend::llvm::LlvmBackend, out: &mut String, _dispatch: &ExprDispatch) -> crate::backend::llvm::TypedRegister {
        let old_expr = match self.kind {
            UnaryOpKind::Neg => Expr::Neg(self.operand.clone()),
            UnaryOpKind::Not => Expr::Not(self.operand.clone()),
            UnaryOpKind::BitNot => Expr::BitNot(self.operand.clone()),
        };
        ctx.emit_expr(out, &old_expr, "")
    }
}


impl ExprCodegenWebstack for UnaryOpExpr {
    fn emit_js(&self, _ctx: &crate::backend::webstack::WebstackGenerator, _dispatch: &ExprDispatch) -> String {
        // Webstack handles Expr::UnaryOp directly in expr_to_ts — this trait
        // path is a fallback for dispatch chains that route through feature structs.
        let op = match self.operand.as_ref() {
            Expr::Integer(n) => n.to_string(),
            Expr::Float(f) => f.to_string(),
            Expr::Bool(b) => b.to_string(),
            Expr::Identifier(name) => name.clone(),
            _ => "value".to_string(),
        };
        match self.kind {
            UnaryOpKind::Neg => format!("(-{})", op),
            UnaryOpKind::Not => format!("(!{})", op),
            UnaryOpKind::BitNot => format!("(~{})", op),
        }
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
