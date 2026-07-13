use crate::ast::Expr;
use crate::features::traits::{ExprCodegenWebstack, ExprDispatch};

/// Binary operator kind. Thin compat layer over `crate::ast::BinaryOpKind`.
// 2026-07-13: Uses `Ne` (not `Neq`) to match existing webstack and LLVM match arms.
// Convert via `to_ast()` when `ast::BinaryOpKind` is required.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOpKind {
    Add, Sub, Mul, Div, Mod,
    Eq, Ne, Lt, Le, Gt, Ge,
    And, Or,
    BitAnd, BitOr, BitXor,
    Shl, Shr,
}

impl BinaryOpKind {
    #[allow(dead_code)]
    pub fn to_ast(self) -> crate::ast::BinaryOpKind {
        match self {
            BinaryOpKind::Add => crate::ast::BinaryOpKind::Add,
            BinaryOpKind::Sub => crate::ast::BinaryOpKind::Sub,
            BinaryOpKind::Mul => crate::ast::BinaryOpKind::Mul,
            BinaryOpKind::Div => crate::ast::BinaryOpKind::Div,
            BinaryOpKind::Mod => crate::ast::BinaryOpKind::Mod,
            BinaryOpKind::Eq => crate::ast::BinaryOpKind::Eq,
            BinaryOpKind::Ne => crate::ast::BinaryOpKind::Neq,
            BinaryOpKind::Lt => crate::ast::BinaryOpKind::Lt,
            BinaryOpKind::Le => crate::ast::BinaryOpKind::Le,
            BinaryOpKind::Gt => crate::ast::BinaryOpKind::Gt,
            BinaryOpKind::Ge => crate::ast::BinaryOpKind::Ge,
            BinaryOpKind::And => crate::ast::BinaryOpKind::And,
            BinaryOpKind::Or => crate::ast::BinaryOpKind::Or,
            BinaryOpKind::BitAnd => crate::ast::BinaryOpKind::BitAnd,
            BinaryOpKind::BitOr => crate::ast::BinaryOpKind::BitOr,
            BinaryOpKind::BitXor => crate::ast::BinaryOpKind::BitXor,
            BinaryOpKind::Shl => crate::ast::BinaryOpKind::Shl,
            BinaryOpKind::Shr => crate::ast::BinaryOpKind::Shr,
        }
    }
}

/// A binary expression with operator kind and two operands.
#[derive(Debug, Clone)]
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
    pub fn kind_name(&self) -> &'static str {
        match self.kind {
            BinaryOpKind::Add => "Add", BinaryOpKind::Sub => "Sub",
            BinaryOpKind::Mul => "Mul", BinaryOpKind::Div => "Div",
            BinaryOpKind::Mod => "Mod",
            BinaryOpKind::Eq => "Eq", BinaryOpKind::Ne => "Ne",
            BinaryOpKind::Lt => "Lt", BinaryOpKind::Le => "Le",
            BinaryOpKind::Gt => "Gt", BinaryOpKind::Ge => "Ge",
            BinaryOpKind::And => "And", BinaryOpKind::Or => "Or",
            BinaryOpKind::BitAnd => "BitAnd", BinaryOpKind::BitOr => "BitOr",
            BinaryOpKind::BitXor => "BitXor",
            BinaryOpKind::Shl => "Shl", BinaryOpKind::Shr => "Shr",
        }
    }
}

impl ExprCodegenWebstack for BinaryOpExpr {
    fn emit_js(&self, _ctx: &crate::backend::webstack::WebstackGenerator, _dispatch: &ExprDispatch) -> String {
        let l = match self.left.as_ref() {
            Expr::Decimal(n) => n.to_string(),
            Expr::Float(f) => f.to_string(),
            Expr::Bool(b) => b.to_string(),
            Expr::Identifier(name) => name.clone(),
            _ => "value".to_string(),
        };
        let r = match self.right.as_ref() {
            Expr::Decimal(n) => n.to_string(),
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
