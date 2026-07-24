// ── Expression AST Definitions ─────────────────────────────────────────
// 2026-07-12: Phase 0.2 — New architecture expression types.
// No IntrinsicCall variant — Sqrt#(x) is Call("Sqrt#", [x]).
// No "feature" wrapper types — unified Expr enum only.

use crate::ast::Type;
use crate::errors::Span;

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    // ── Literals ────────────────────────────────────────────────
    Quoted(Vec<u8>), // "..." raw bytes
    Decimal(i64),    // 42, 16711680
    /// 2026-07-20: Tagged literal with discriminator prefix/suffix.
    /// Second field is the discriminator tag: "0x", "h", "bf", etc.
    /// Used by op Parse(Decimal, pre: "0x") for routing.
    TaggedLiteral(i64, String), // 0xFF00FF, FF00FFh, 1.5bf
    Bool(bool),      // true, false
    Float(f64),      // 3.14

    // ── References ──────────────────────────────────────────────
    Identifier(String), // foo, Sqrt#, AddI64#

    // ── Operations ──────────────────────────────────────────────
    Call(String, Vec<Expr>, Option<usize>), // f(x), Sqrt#(x) — analysis_id for Alloc# strategy
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
    /// struct Name { field: expr; ... } — struct literal.
    /// 2026-07-24: Constructs a value of a static struct type.
    StructLiteral {
        type_name: String,
        fields: Vec<(String, Expr)>,
    },

    // ── Functions ───────────────────────────────────────────────
    Lambda(Vec<String>, Box<Expr>),

    // ── Type operations ─────────────────────────────────────────
    Cast(Box<Expr>, Type),
    IsType(Box<Expr>, Type),

    // ── Scope ───────────────────────────────────────────────────
    Within(Box<Expr>, Box<Expr>),

    // ── Derivation ──────────────────────────────────────────────
    DerivationBlock(DerivationBlock),

    // ── Pointers ────────────────────────────────────────────────
    Deref(Box<Expr>),
    AddrOf(Box<Expr>),

    // ── Plugin intercept ────────────────────────────────────────
    // 2026-07-19: name!(args). Resolved by Front or Mid stage plugins.
    PluginIntercept {
        name: String,
        args: Vec<Expr>,
        type_args: Vec<Type>,
    },

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

#[derive(Debug, Clone, PartialEq)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub guard: Option<Expr>,
    pub body: Box<Expr>,
}

#[derive(Debug, Clone, PartialEq)]
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
#[derive(Debug, Clone, PartialEq)]
pub struct DerivationExample {
    pub inputs: Vec<Expr>,
    pub output: Box<Expr>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DerivationBlock {
    pub examples: Vec<DerivationExample>,
    pub synthesized: Option<Box<Expr>>,
    pub span: Span,
}

impl Expr {
    /// Return the identifier name if this expression is a simple variable reference.
    pub fn as_var_name(&self) -> Option<&str> {
        if let Expr::Identifier(name) = self {
            Some(name.as_str())
        } else {
            None
        }
    }

    /// Collect all variable (Identifier) names referenced in this expression tree.
    pub fn collect_vars(&self) -> Vec<String> {
        let mut vars = Vec::new();
        self.collect_vars_into(&mut vars);
        vars
    }

    fn collect_vars_into(&self, acc: &mut Vec<String>) {
        match self {
            Expr::Identifier(name) => acc.push(name.clone()),
            Expr::BinaryOp(_, l, r) => {
                l.collect_vars_into(acc);
                r.collect_vars_into(acc);
            }
            Expr::UnaryOp(_, e) => e.collect_vars_into(acc),
            Expr::Field(e, _) => e.collect_vars_into(acc),
            Expr::Index(l, r) => {
                l.collect_vars_into(acc);
                r.collect_vars_into(acc);
            }
            Expr::Call(_, args, _) => {
                for a in args {
                    a.collect_vars_into(acc);
                }
            }
            Expr::Block(_stmts) => {}
            Expr::If(c, t, e) => {
                c.collect_vars_into(acc);
                t.collect_vars_into(acc);
                if let Some(e) = e {
                    e.collect_vars_into(acc);
                }
            }
            Expr::Match(e, arms) => {
                e.collect_vars_into(acc);
                for arm in arms {
                    arm.body.collect_vars_into(acc);
                    if let Some(g) = &arm.guard {
                        g.collect_vars_into(acc);
                    }
                }
            }
            Expr::Tuple(items) => {
                for i in items {
                    i.collect_vars_into(acc);
                }
            }
            Expr::List(items) => {
                for i in items {
                    i.collect_vars_into(acc);
                }
            }
            Expr::Lambda(_, body) => body.collect_vars_into(acc),
            Expr::Cast(e, _) => e.collect_vars_into(acc),
            Expr::IsType(e, _) => e.collect_vars_into(acc),
            Expr::Within(l, r) => {
                l.collect_vars_into(acc);
                r.collect_vars_into(acc);
            }
            Expr::DerivationBlock(d) => {
                for ex in &d.examples {
                    for inp in &ex.inputs {
                        inp.collect_vars_into(acc);
                    }
                }
            }
            Expr::Deref(inner) | Expr::AddrOf(inner) => inner.collect_vars_into(acc),
            _ => {}
        }
    }
}
