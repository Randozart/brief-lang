use crate::ast::Expr;
use crate::features::traits::{ExprCodegenWebstack, ExprDispatch};

/// Unary operator kind. Mirrors `crate::ast::UnaryOpKind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOpKind {
    Neg,
    Not,
    BitNot,
}

impl UnaryOpKind {
    #[allow(dead_code)]
    pub fn to_ast(self) -> crate::ast::UnaryOpKind {
        match self {
            UnaryOpKind::Neg => crate::ast::UnaryOpKind::Neg,
            UnaryOpKind::Not => crate::ast::UnaryOpKind::Not,
            UnaryOpKind::BitNot => crate::ast::UnaryOpKind::BitNot,
        }
    }

    /// Map operator kind to its property name (e.g. Neg → "Neg").
    pub fn name(&self) -> &'static str {
        match self {
            UnaryOpKind::Neg => "Neg",
            UnaryOpKind::Not => "Not",
            UnaryOpKind::BitNot => "BitNot",
        }
    }
}

/// A unary expression with operator kind and one operand.
#[derive(Debug, Clone)]
pub struct UnaryOpExpr {
    pub kind: UnaryOpKind,
    pub operand: Box<Expr>,
}

impl UnaryOpExpr {
    pub fn new(kind: UnaryOpKind, operand: Expr) -> Self {
        UnaryOpExpr { kind, operand: Box::new(operand) }
    }
}

impl ExprCodegenWebstack for UnaryOpExpr {
    fn emit_js(&self, _ctx: &crate::backend::webstack::WebstackGenerator, _dispatch: &ExprDispatch) -> String {
        let op = match self.operand.as_ref() {
            Expr::Decimal(n) => n.to_string(),
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
