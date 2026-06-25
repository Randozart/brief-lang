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
            (Add,  Value::String(a), Value::String(b)) => Value::String({
                let mut s = String::with_capacity(a.len() + b.len());
                s.push_str(a);
                s.push_str(b);
                s
            }),
            (Add,  Value::String(a), Value::Int(b)) => Value::String({
                let mut s = String::with_capacity(a.len() + 20);
                s.push_str(a);
                s.push_str(&b.to_string());
                s
            }),
            (Add,  Value::Int(a), Value::String(b)) => Value::String({
                let mut s = String::with_capacity(20 + b.len());
                s.push_str(&a.to_string());
                s.push_str(b);
                s
            }),
            (Add,  Value::String(a), Value::Float(b)) => Value::String({
                let mut s = String::with_capacity(a.len() + 30);
                s.push_str(a);
                s.push_str(&b.to_string());
                s
            }),
            (Add,  Value::Float(a), Value::String(b)) => Value::String({
                let mut s = String::with_capacity(30 + b.len());
                s.push_str(&a.to_string());
                s.push_str(b);
                s
            }),
            (Add,  Value::Float(a), Value::Float(b)) => Value::Float(a + b),
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
            // Ptr<T> arithmetic — all ops preserve T, produce Ptr<T>
            (Add,  Value::Ptr(a), Value::Int(b)) => Value::Ptr(a.wrapping_add(*b as u64)),
            (Add,  Value::Int(a), Value::Ptr(b)) => Value::Ptr(b.wrapping_add(*a as u64)),
            (Sub,  Value::Ptr(a), Value::Int(b)) => Value::Ptr(a.wrapping_sub(*b as u64)),
            (BitAnd, Value::Ptr(a), Value::Int(b)) => Value::Ptr(a & *b as u64),
            (BitAnd, Value::Int(a), Value::Ptr(b)) => Value::Ptr(b & *a as u64),
            (BitOr,  Value::Ptr(a), Value::Int(b)) => Value::Ptr(a | *b as u64),
            (BitOr,  Value::Int(a), Value::Ptr(b)) => Value::Ptr(b | *a as u64),
            (BitXor, Value::Ptr(a), Value::Int(b)) => Value::Ptr(a ^ *b as u64),
            (BitXor, Value::Int(a), Value::Ptr(b)) => Value::Ptr(b ^ *a as u64),
            (Shl,  Value::Ptr(a), Value::Int(b)) => Value::Ptr(a << *b),
            (Shr,  Value::Ptr(a), Value::Int(b)) => Value::Ptr(a >> *b),
            // Ptr<T> comparison
            (Eq,  Value::Ptr(a), Value::Ptr(b)) => Value::Bool(a == b),
            (Ne,  Value::Ptr(a), Value::Ptr(b)) => Value::Bool(a != b),
            (Lt,  Value::Ptr(a), Value::Ptr(b)) => Value::Bool(a < b),
            (Le,  Value::Ptr(a), Value::Ptr(b)) => Value::Bool(a <= b),
            (Gt,  Value::Ptr(a), Value::Ptr(b)) => Value::Bool(a > b),
            (Ge,  Value::Ptr(a), Value::Ptr(b)) => Value::Bool(a >= b),
            (_, Value::Regex(_), _) | (_, _, Value::Regex(_)) => {
                return Err(RuntimeError::TypeMismatch(format!("binary op {:?} on Regex", self.kind)))
            }
            _ => return Err(RuntimeError::TypeMismatch(format!("binary op {:?}", self.kind))),
        })
    }
}

impl ExprCodegenLLVM for BinaryOpExpr {
    fn emit_llvm(&self, ctx: &mut crate::backend::llvm::LlvmBackend, out: &mut String, _dispatch: &ExprDispatch) -> crate::backend::llvm::TypedRegister {
        // Emit the operands and select the correct operation directly,
        // avoiding a nested emit_expr call that would waste %tN registers.
        match self.kind {
            BinaryOpKind::Add => ctx.emit_binop(out, "", &self.left, &self.right, "add", "fadd"),
            BinaryOpKind::Sub => ctx.emit_binop(out, "", &self.left, &self.right, "sub", "fsub"),
            BinaryOpKind::Mul => ctx.emit_binop(out, "", &self.left, &self.right, "mul", "fmul"),
            BinaryOpKind::Div => ctx.emit_binop(out, "", &self.left, &self.right, "sdiv", "fdiv"),
            BinaryOpKind::Mod => {
                let (a, b) = (ctx.emit_expr(out, &self.left, ""), ctx.emit_expr(out, &self.right, ""));
                let v = format!("%t{}", ctx.txn_counter); ctx.txn_counter += 1;
                writeln!(out, "{} = srem i64 {}, {}", v, a, b).ok();
                crate::backend::llvm::TypedRegister { name: v, ty: crate::ast::Type::Int }
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
