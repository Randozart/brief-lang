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
    /// 2026-07-27: Tagged quoted string: sql"SELECT", my"hello"
    /// First field is the string content, second is the prefix tag.
    TaggedQuotedLiteral(Vec<u8>, String),
    Bool(bool),      // true, false
    Float(f64),      // 3.14

    // ── References ──────────────────────────────────────────────
    Identifier(String), // foo, Sqrt#, AddI64#

    // ── Operations ──────────────────────────────────────────────
    Call(String, Vec<Expr>, Option<usize>), // f(x), Sqrt#(x) — analysis_id for Alloc# strategy
    BinaryOp(BinaryOpKind, Box<Expr>, Box<Expr>),
    UnaryOp(UnaryOpKind, Box<Expr>),
    Field(Box<Expr>, String),
    /// 2026-07-31: Method call with a receiver: a.f(x). The receiver is
    /// preserved and bound to the `self` parameter of the obj member.
    MethodCall(Box<Expr>, String, Vec<Expr>, Option<usize>),
    /// 2026-07-31: Reflection access: `a.^Len` (runtime) / `a.^^Size`
    /// (compile-time). The receiver is preserved; the target is a PascalCase
    /// compiler-known identifier resolved by the D1 reflection table.
    Reflect(Box<Expr>, String, ReflectKind),
    Index(Box<Expr>, Box<Expr>),
    /// arr[start:end:stride] — zero-copy slice view
    Slice {
        array: Box<Expr>,
        start: Option<Box<Expr>>,
        end: Option<Box<Expr>>,
        stride: Option<Box<Expr>>,
    },

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
    DerivationBlock(Box<DerivationBlock>),

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
    FormattingAnnotation(super::Formatting),
    /// 2026-07-25: fn? — compile-time existence check. Evaluates to
    /// Bool(true) if the function linked, Bool(false) otherwise.
    /// Used for guarding frgn?/frgn!/frgn?! calls.
    Exists(String),
}

/// 2026-07-31: Reflection kind — distinguishes value-derived (runtime)
/// reflection from type-derived (compile-time, foldable) reflection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReflectKind {
    /// `x.^Len`, `x.^Ptr` — runtime value-derived.
    Runtime,
    /// `x.^^Size`, `x.^^Bytes` — compile-time type-derived, foldable.
    CompileTime,
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
/// 2026-07-28: Added tolerance field for FP relaxed equivalence — optional
/// f64 relative tolerance. When Some(tol), the synthesized expression must
/// produce output within tol * |expected| of each example's expected output.
#[derive(Debug, Clone, PartialEq)]
pub struct DerivationExample {
    pub inputs: Vec<Expr>,
    pub output: Box<Expr>,
    /// 2026-07-28: Optional relative tolerance for FP relaxed equivalence.
    /// Syntax: `input -> [tol] output;` in derivation blocks.
    pub tolerance: Option<f64>,
    pub span: Span,
}

/// 2026-07-29: A segment in the := verification chain.
/// Each segment is one body: an asm function name, a derivation block,
/// or a reference function name.
#[derive(Debug, Clone, PartialEq)]
pub enum ChainSegment {
    /// Reference to an asm function or defn: := name
    Ref(String),
    /// Derivation block with examples and optional reference
    Derivation(Box<DerivationBlock>),
}

#[derive(Debug, Clone, PartialEq)]
pub struct DerivationBlock {
    pub examples: Vec<DerivationExample>,
    pub synthesized: Option<Box<Expr>>,
    /// 2026-07-28: Optional postcondition for full-spec CEGIS verification.
    /// Parsed from [[post]] syntax: [[ post ] or [[ pre ]][ post ].
    /// The postcondition is a predicate using #Term to refer to the
    /// function's return value (e.g., [[ #Term >= 0 ]).
    pub postcondition: Option<Expr>,
    /// 2026-07-29: Optional precondition stating valid input states.
    /// Parsed from [[ pre ]] syntax before the postcondition block.
    /// Example: [[ x0 >= 0 ]][ #Term >= 0 ] — only verify for non-negative inputs.
    pub precondition: Option<Expr>,
    /// 2026-07-29: Optional reference function for correctness verification.
    /// `verifying ref_fn` declares an existing function as the oracle for CEGIS.
    /// The synthesized function must match ref_fn for ALL inputs (within tolerance).
    /// ref_name: name of the reference function (must be a defn in the same unit).
    /// ref_tolerance: allowed deviation (default 0.0 for exact match).
    pub ref_name: Option<String>,
    pub ref_tolerance: Option<f64>,
    /// 2026-07-29: Multi-segment verification chain.
    /// If non-empty, body is selected by cross-verification at compile time.
    pub chain: Vec<ChainSegment>,
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
            Expr::MethodCall(recv, _, args, _) => {
                recv.collect_vars_into(acc);
                for a in args {
                    a.collect_vars_into(acc);
                }
            }
            Expr::Reflect(recv, _, _) => recv.collect_vars_into(acc),
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
