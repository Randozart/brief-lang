// ── Expression AST Definitions ─────────────────────────────────────────
// 2026-07-12: Phase 0.2 — New architecture expression types.
// No IntrinsicCall variant — Sqrt#(x) is Call("Sqrt#", [x]).
// No "feature" wrapper types — unified Expr enum only.

use crate::ast_new::Type;
use crate::errors::Span;

#[derive(Debug, Clone)]
pub enum Expr {
    // ── Literals ────────────────────────────────────────────────
    Quoted(Vec<u8>), // "..." raw bytes
    Decimal(i64),    // 42, 0xFF
    Bool(bool),      // true, false
    Float(f64),      // 3.14

    // ── References ──────────────────────────────────────────────
    Identifier(String), // foo, Sqrt#, AddI64#

    // ── Operations ──────────────────────────────────────────────
    Call(String, Vec<Expr>), // f(x), Sqrt#(x)
    BinaryOp(BinaryOpKind, Box<Expr>, Box<Expr>),
    UnaryOp(UnaryOpKind, Box<Expr>),
    Field(Box<Expr>, String),
    Index(Box<Expr>, Box<Expr>),

    // ── Control flow ────────────────────────────────────────────
    Block(Vec<super::Statement>),
    If(Box<Expr>, Box<Expr>, Option<Box<Expr>>),
    Match(Box<Expr>, Vec<MatchArm>),

    // ── Compound values ─────────────────────────────────────────
    Tuple(Vec<Expr>),
    List(Vec<Expr>),

    // ── Functions ───────────────────────────────────────────────
    Lambda(Vec<String>, Box<Expr>),

    // ── Type operations ─────────────────────────────────────────
    Cast(Box<Expr>, Type),
    IsType(Box<Expr>, Type),

    // ── Scope ───────────────────────────────────────────────────
    Within(Box<Expr>, Box<Expr>),

    // ── Derivation ──────────────────────────────────────────────
    DerivationBlock(DerivationBlock),

    // ── Metadata ────────────────────────────────────────────────
    PropertyGet(String),
    FormattingAnnotation(super::Formatting),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOpKind {
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Eq,
    Neq,
    Lt,
    Gt,
    Le,
    Ge,
    And,
    Or,
    BitAnd,
    BitOr,
    BitXor,
    Shl,
    Shr,
    Concat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOpKind {
    Neg,
    Not,
    BitNot,
}

#[derive(Debug, Clone)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub guard: Option<Expr>,
    pub body: Box<Expr>,
}

#[derive(Debug, Clone)]
pub enum Pattern {
    Wildcard,
    Literal(Expr),
    Binding(String),
    EnumVariant(String, Vec<Pattern>),
    Tuple(Vec<Pattern>),
    Range(Expr, Expr),
}

/// A derivation block attached to a definition or transaction via `:=`.
/// Contains input-output examples for inductive synthesis.
#[derive(Debug, Clone)]
pub struct DerivationExample {
    pub inputs: Vec<Expr>,
    pub output: Box<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct DerivationBlock {
    pub examples: Vec<DerivationExample>,
    pub synthesized: Option<Box<Expr>>,
    pub span: Span,
}
