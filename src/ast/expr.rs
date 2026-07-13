// ── Expression AST Definitions ─────────────────────────────────────────
// 2026-07-12: Phase 0.2 — New architecture expression types.
// No IntrinsicCall variant — Sqrt#(x) is Call("Sqrt#", [x]).
// No "feature" wrapper types — unified Expr enum only.

use crate::ast::Type;
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

    // ── Legacy backward-compatible variants (deprecated) ────────────
    // These mirror old ast::Expr variants. New code should use the
    // modern variants above. These exist only for migration compatibility.
    Add(Box<Expr>, Box<Expr>),
    Sub(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
    Div(Box<Expr>, Box<Expr>),
    Mod(Box<Expr>, Box<Expr>),
    Eq(Box<Expr>, Box<Expr>),
    Ne(Box<Expr>, Box<Expr>),
    Lt(Box<Expr>, Box<Expr>),
    Le(Box<Expr>, Box<Expr>),
    Gt(Box<Expr>, Box<Expr>),
    Ge(Box<Expr>, Box<Expr>),
    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
    BitAnd(Box<Expr>, Box<Expr>),
    BitOr(Box<Expr>, Box<Expr>),
    BitXor(Box<Expr>, Box<Expr>),
    Shl(Box<Expr>, Box<Expr>),
    Shr(Box<Expr>, Box<Expr>),
    Concat(Box<Expr>, Box<Expr>),
    Not(Box<Expr>),
    Neg(Box<Expr>),
    BitNot(Box<Expr>),
    IntrinsicCall { intrinsic: String, args: Vec<Expr> },
    Projection { source: Box<Expr>, target: ProjectionTarget },
    FieldAccess(Box<Expr>, String),
    ListLiteral(Vec<Expr>),
    /// Legacy Block with separate last expression. Modern code uses Block(Vec<Statement>).
    LegacyBlock(Vec<Statement>, Box<Expr>),
    StructInstance(String, Vec<(String, Expr)>),
    ObjectLiteral(Vec<(String, Expr)>),
    PriorState(String),
    AddrOf(Box<Expr>),
    Deref(Box<Expr>),
    TupleDestructure(Vec<String>, Box<Expr>),
    PatternMatch { value: Box<Expr>, variant: String, fields: Vec<PatternField> },
    Slice { value: Box<Expr>, start: Option<Box<Expr>>, end: Option<Box<Expr>>, stride: Option<Box<Expr>>, mask: Option<Box<Expr>> },
    ArrowMut { dir: ArrowDir, consume: bool, target: Box<Expr>, index: Box<Expr>, value: Option<Box<Expr>> },
    ArrowDiscard { target: Box<Expr>, index: Box<Expr> },
    ArrowTransfer { dest: Box<Expr>, source: Box<Expr>, filter: Option<Box<Expr>>, consume: bool },
    SigCall { modifier: SigModifier, expr: Box<Expr> },
    SubtypeProjection { source: Box<Expr>, ops: Vec<SubtypeOp> },
    Like(Box<Expr>, Box<Expr>),
    FromCheck(Box<Expr>, Type),
    SharedMem(SharedMem),
    CellCall(Box<Expr>, Vec<Expr>),
    QuoteBlock { statements: Vec<Statement>, trailing_expr: Option<Box<Expr>> },
    Interpolate(String),
    DeferredLiteral { text: String, expected_type: Option<Type> },
    Ellipsis,
    TypeRef(String),
    TemplateCall { name: String, args: Vec<Expr>, block: String, span: Option<Span> },
    MacroCall { name: String, args: Vec<Expr>, block: String, span: Option<Span> },
    DbvlTable { path: String, field_names: Vec<String>, key_offsets: Vec<usize>, schema_name: String },
    PipeChain(PipeChain),

}
