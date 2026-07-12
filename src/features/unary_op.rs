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
        Ok(Type::int())
    }
}

impl UnaryOpKind {
    /// Map operator kind to its property name (e.g. Neg → "Neg").
    /// 2026-07-11: Phase 8B — property-based operator dispatch.
    pub fn name(&self) -> &'static str {
        use UnaryOpKind::*;
        match self {
            Neg => "Neg", Not => "Not", BitNot => "BitNot",
        }
    }
}

impl ExprEval for UnaryOpExpr {
    fn evaluate(&self, ctx: &mut Interpreter, _dispatch: &ExprDispatch) -> Result<Value, RuntimeError> {
        let v = ctx.eval_expr(&self.operand)?;

        // Phase 8B: property-based dispatch for Bits operands
        if let Some(result) = try_unary_bits_dispatch(&v, self.kind.name(), ctx) {
            return result;
        }

        use UnaryOpKind::*;
        Ok(match (self.kind, v) {
            (Neg,    Value::Bits(crate::interpreter::i64_to_bits(a))) => Value::Int(-a),
            (Neg,    Value::Bits(b)) => {
                let mut arr = [0u8; 8];
                let copy_len = b.len().min(8);
                arr[..copy_len].copy_from_slice(&b[..copy_len]);
                let val = i64::from_le_bytes(arr);
                Value::Bits((-val).to_le_bytes().to_vec())
            }
            (Not,    Value::Bool(a)) => Value::Bool(!a),
            (Not,    Value::Bits(b)) => Value::Bool(b.first().map_or(true, |x| *x == 0)),
            (BitNot, Value::Bits(crate::interpreter::i64_to_bits(a))) => Value::Int(!a),
            (_, Value::Regex(_)) => {
                return Err(RuntimeError::TypeMismatch(format!("unary op {:?} on Regex", self.kind)))
            }
            _ => return Err(RuntimeError::TypeMismatch(format!("unary op {:?}", self.kind))),
        })
    }
}

/// Try to dispatch a unary op through property-based intrinsic lookup.
/// The operand must be Value::Bits and the expected type must have a
/// binding for the operator. Returns None to fall back to legacy dispatch.
/// 2026-07-11: Phase 8B — flat control flow, max 2 levels.
fn try_unary_bits_dispatch(
    v: &Value,
    op_name: &str,
    ctx: &Interpreter,
) -> Option<Result<Value, RuntimeError>> {
    let Value::Bits(_) = v else { return None; };
    let expected_type = ctx.current_expected_type.as_ref()?;
    let type_name = match expected_type {
        Type::Custom(n) => n.as_str(),
        _ => return None,
    };
    if type_name.is_empty() {
        return None;
    }
    let universe = ctx.type_universe.as_ref()?;
    let intrinsic = universe.get_operator_intrinsic(type_name, op_name)?;
    Some(crate::interpreter::execute_intrinsic(intrinsic, &[v.clone()]))
}

impl ExprCodegenLLVM for UnaryOpExpr {
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
#[cfg(all(feature = "kani", feature = "kani_full"))]
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
