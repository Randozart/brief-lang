use crate::ast::{Expr, Type};
use crate::features::traits::*;
use crate::interpreter::{Interpreter, RuntimeError, Value};
use crate::typechecker::TypeChecker;
use std::fmt::Write;

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

    /// Map operator kind to its property name (e.g. Add → "Add").
    /// 2026-07-11: Phase 8B — property-based operator dispatch.
    pub fn kind_name(&self) -> &'static str {
        use BinaryOpKind::*;
        match self.kind {
            Add => "Add", Sub => "Sub", Mul => "Mul", Div => "Div", Mod => "Mod",
            Eq => "Eq", Ne => "Ne", Lt => "Lt", Le => "Le", Gt => "Gt", Ge => "Ge",
            And => "And", Or => "Or",
            BitAnd => "BitAnd", BitOr => "BitOr", BitXor => "BitXor",
            Shl => "Shl", Shr => "Shr",
        }
    }
}



/// Try to dispatch a binary op through property-based intrinsic lookup.
/// Both operands must be Value::Bits and the expected type must have an
/// operator binding. Returns None to fall back to legacy typed dispatch.
/// 2026-07-11: Phase 8B — flat control flow, max 2 levels.
fn try_bits_dispatch(
    l: &Value,
    r: &Value,
    op_name: &str,
    ctx: &Interpreter,
) -> Option<Result<Value, RuntimeError>> {
    let (Value::Bits(_), Value::Bits(_)) = (l, r) else { return None; };
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
    Some(crate::interpreter::execute_intrinsic(intrinsic, &[l.clone(), r.clone()]))
}

impl ExprTypecheck for BinaryOpExpr {
    fn typecheck(&self, _ctx: &mut TypeChecker, _dispatch: &ExprDispatch) -> Result<Type, crate::errors::TypeError> {
        // DEFERRED (Phase 4): infer_expression is private. Router arms
        // in typechecker.rs destructure sub-expressions directly.
        Ok(Type::int())
    }
}

impl ExprEval for BinaryOpExpr {
    fn evaluate(&self, ctx: &mut Interpreter, _dispatch: &ExprDispatch) -> Result<Value, RuntimeError> {
        let l = ctx.eval_expr(&self.left)?;
        let r = ctx.eval_expr(&self.right)?;

        // Phase 8B: property-based dispatch for Bits operands
        if let Some(result) = try_bits_dispatch(&l, &r, self.kind_name(), ctx) {
            return result;
        }

        use BinaryOpKind::*;

        let result = match self.kind {
            Add | Sub | Mul | Div | Mod | BitAnd | BitOr | BitXor | Shl | Shr => {
                let lb = crate::interpreter::value_as_i64(&l).ok_or_else(|| RuntimeError::TypeMismatch(
                    format!("binary op {:?} requires integer operands", self.kind)))?;
                let rb = crate::interpreter::value_as_i64(&r).ok_or_else(|| RuntimeError::TypeMismatch(
                    format!("binary op {:?} requires integer operands", self.kind)))?;
                match self.kind {
                    Add => crate::interpreter::i64_to_bits(lb.wrapping_add(rb)),
                    Sub => crate::interpreter::i64_to_bits(lb.wrapping_sub(rb)),
                    Mul => crate::interpreter::i64_to_bits(lb.wrapping_mul(rb)),
                    Div => crate::interpreter::i64_to_bits(lb.wrapping_div(rb)),
                    Mod => crate::interpreter::i64_to_bits(lb.wrapping_rem(rb)),
                    BitAnd => crate::interpreter::i64_to_bits(lb & rb),
                    BitOr => crate::interpreter::i64_to_bits(lb | rb),
                    BitXor => crate::interpreter::i64_to_bits(lb ^ rb),
                    Shl => crate::interpreter::i64_to_bits(lb.wrapping_shl(rb as u32)),
                    Shr => crate::interpreter::i64_to_bits(lb.wrapping_shr(rb as u32)),
                    _ => unreachable!(),
                }
            }
            Lt | Le | Gt | Ge => {
                let lb = crate::interpreter::value_as_i64(&l).ok_or_else(|| RuntimeError::TypeMismatch(
                    format!("binary op {:?} requires integer operands", self.kind)))?;
                let rb = crate::interpreter::value_as_i64(&r).ok_or_else(|| RuntimeError::TypeMismatch(
                    format!("binary op {:?} requires integer operands", self.kind)))?;
                vec![match self.kind {
                    Lt => lb < rb,
                    Le => lb <= rb,
                    Gt => lb > rb,
                    Ge => lb >= rb,
                    _ => unreachable!(),
                } as u8]
            }
            Eq => vec![(l == r) as u8],
            Ne => vec![(l != r) as u8],
            And | Or => {
                let lb = match &l { Value::Bits(b) => b.first().copied().unwrap_or(0) != 0, _ => false };
                let rb = match &r { Value::Bits(b) => b.first().copied().unwrap_or(0) != 0, _ => false };
                vec![match self.kind {
                    And => lb && rb,
                    Or => lb || rb,
                    _ => unreachable!(),
                } as u8]
            }
        };
        Ok(Value::Bits(result))
    }
}

impl ExprCodegenLLVM for BinaryOpExpr {
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
        // Emit the operands and select the correct operation directly,
        // avoiding a nested emit_expr call that would waste %tN registers.
        match self.kind {
            BinaryOpKind::Add => {
                // 2026-06-29: Check is_string_chain before emit_binop, matching
                // the old-style Expr::Add handler at emit_expr.rs:404. Without this,
                // string concatenation via new-style BinaryOp becomes integer addition
                // of tagged pointer addresses, producing garbage runtime addresses.
                if ctx.is_string_chain(&self.left) || ctx.is_string_chain(&self.right) {
                    let a = ctx.emit_expr(out, &self.left, "");
                    let b = ctx.emit_expr(out, &self.right, "");
                    return ctx.emit_inline_concat(out, "", &a, &b);
                }
                ctx.emit_binop(out, "", &self.left, &self.right, "add nsw", "fadd")
            }
            BinaryOpKind::Sub => ctx.emit_binop(out, "", &self.left, &self.right, "sub nsw", "fsub"),
            BinaryOpKind::Mul => ctx.emit_binop(out, "", &self.left, &self.right, "mul nsw", "fmul"),
            BinaryOpKind::Div => ctx.emit_binop(out, "", &self.left, &self.right, "sdiv", "fdiv"),
            BinaryOpKind::Mod => {
                let (a, b) = (ctx.emit_expr(out, &self.left, ""), ctx.emit_expr(out, &self.right, ""));
                let v = format!("%t{}", ctx.fun.txn_counter); ctx.fun.txn_counter += 1;
                writeln!(out, "{} = srem i64 {}, {}", v, a, b).ok();
                crate::backend::llvm::TypedRegister { name: v, ty: crate::ast::Type::int() }
            }
            // Comparison operators return Bool
            BinaryOpKind::Eq => ctx.emit_fcmp(out, "", &self.left, &self.right, "oeq"),
            BinaryOpKind::Ne => ctx.emit_fcmp(out, "", &self.left, &self.right, "one"),
            BinaryOpKind::Lt => ctx.emit_fcmp(out, "", &self.left, &self.right, "olt"),
            BinaryOpKind::Le => ctx.emit_fcmp(out, "", &self.left, &self.right, "ole"),
            BinaryOpKind::Gt => ctx.emit_fcmp(out, "", &self.left, &self.right, "ogt"),
            BinaryOpKind::Ge => ctx.emit_fcmp(out, "", &self.left, &self.right, "oge"),
            BinaryOpKind::And | BinaryOpKind::Or => {
                // Fall through to old Expr::And/Or via nested emit_expr
                let old = if self.kind == BinaryOpKind::And {
                    Expr::And(self.left.clone(), self.right.clone())
                } else {
                    Expr::Or(self.left.clone(), self.right.clone())
                };
                ctx.emit_expr(out, &old, "")
            }
            // Bitwise operators return Int
            BinaryOpKind::BitAnd => ctx.emit_binop(out, "", &self.left, &self.right, "and", "and"),
            BinaryOpKind::BitOr => ctx.emit_binop(out, "", &self.left, &self.right, "or", "or"),
            BinaryOpKind::BitXor => ctx.emit_binop(out, "", &self.left, &self.right, "xor", "xor"),
            BinaryOpKind::Shl => ctx.emit_binop(out, "", &self.left, &self.right, "shl", "shl"),
            BinaryOpKind::Shr => ctx.emit_binop(out, "", &self.left, &self.right, "lshr", "lshr"),
        }
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
#[cfg(all(feature = "kani", feature = "kani_full"))]
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
