// ── Top-Level Declaration AST Definitions ──────────────────────────────
// 2026-07-12: Phase 0.2 — New architecture top-level types.
// No InopDeclaration, no TopLevel::Inop.
// Added Export struct and Contract.is_entry.

use crate::ast_new::{DerivationBlock, Expr, Formatting, PropertyValue, Type};
use crate::errors::Span;
use std::collections::HashMap;

// ── TopLevel ───────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum TopLevel {
    Definition(Definition),
    Transaction(Transaction),
    Cell(CellDef),
    Import(Import),
    Export(Export),
    Meld(Meld),
    Trigger(Trigger),
}

// ── Definition ─────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Definition {
    pub name: String,
    pub type_params: Vec<TypeParam>,
    pub parameters: Vec<(String, Type)>,
    pub output_type: Option<OutputType>,
    pub outputs: Vec<Type>,
    pub contract: Contract,
    pub body: Vec<Statement>,
    pub metadata: HashMap<String, PropertyValue>,
    pub derivation: Option<DerivationBlock>,
    pub modifiers: Vec<Annotation>,
    pub annotations: Vec<TypeBinding>,
    pub span: Option<Span>,
}

// ── Transaction ────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Transaction {
    pub name: String,
    pub is_reactive: bool,
    pub is_async: bool,
    pub type_params: Vec<TypeParam>,
    pub parameters: Vec<(String, Type)>,
    pub contract: Contract,
    pub body: Vec<Statement>,
    pub metadata: HashMap<String, PropertyValue>,
    pub derivation: Option<DerivationBlock>,
    pub modifiers: Vec<Annotation>,
    pub span: Option<Span>,
}

// ── Contract ───────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Contract {
    pub pre_condition: Expr,
    pub post_condition: Expr,
    /// 2026-07-12: [#] entry point marker. When true, the function is
    /// CLI-addressable and cannot be called from internal code.
    pub is_entry: bool,
    pub watchdog: Option<WatchdogSpec>,
    pub span: Option<Span>,
}

impl Contract {
    pub fn new(pre: Expr, post: Expr) -> Self {
        Contract {
            pre_condition: pre,
            post_condition: post,
            is_entry: false,
            watchdog: None,
            span: None,
        }
    }
}

// ── Export ─────────────────────────────────────────────────────────────

/// export defn — wraps a definition for library-mode export.
#[derive(Debug, Clone)]
pub struct Export {
    pub inner: Box<TopLevel>,
    pub export_name: Option<String>,
}

// ── Cell ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct CellDef {
    pub name: String,
    pub type_params: Vec<TypeParam>,
    pub parameters: Vec<(String, Type)>,
    pub output_type: Option<OutputType>,
    pub fields: Vec<super::Field>,
    pub transactions: Vec<Transaction>,
    pub definitions: Vec<Definition>,
    pub internal_triggers: Vec<Trigger>,
    pub is_persistent: bool,
    pub metadata: HashMap<String, PropertyValue>,
    pub span: Option<Span>,
}

// ── Statement ──────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum Statement {
    /// let name: Type = expr;
    Let {
        name: String,
        ty: Option<Type>,
        expr: Option<Expr>,
        modifiers: Vec<Annotation>,
    },
    /// dest = expr;
    Assign(Expr, Expr),
    /// term; or term expr;
    Term(Option<Expr>),
    /// term! expr;
    TermBang(Option<Expr>),
    /// return expr;
    Return(Option<Expr>),
    /// [condition] { body } or when condition { body }
    Guarded(Expr, Vec<Statement>),
    /// expr;
    Expression(Expr),
    /// if expr { ... } else { ... }
    If(Expr, Vec<Statement>, Vec<Statement>),
    /// { ... }
    Block(Vec<Statement>),
    /// key <~ value;
    MetadataAssignment(String, PropertyValue),
    /// escape expr;
    Escape(Option<Expr>),
    /// foreach(item in list) { ... }
    Foreach {
        item: String,
        list: Box<Expr>,
        body: Vec<Statement>,
    },
    /// trg name @ instance.port;
    TrgBinding {
        name: String,
        instance: Expr,
        port: String,
    },
    /// asm "instruction" { clobbers }
    InlineAsm {
        asm_string: String,
        clobbers: Vec<String>,
        span: Option<Span>,
    },
    /// sync { ... }
    SyncBlock(Vec<Statement>),
}

// ── Supporting Types ───────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct TypeParam {
    pub name: String,
    pub bound: Option<Type>,
}

#[derive(Debug, Clone)]
pub enum OutputType {
    Single(Type),
    Union(Vec<OutputType>),
    Tuple(Vec<OutputType>),
    Array(Box<OutputType>),
    Named(String, Box<OutputType>),
}

impl OutputType {
    pub fn single(ty: Type) -> Self {
        OutputType::Single(ty)
    }
}

#[derive(Debug, Clone)]
pub struct WatchdogSpec {
    pub condition: Expr,
    pub is_required: bool,
    pub cycles_bound: Option<u64>,
    pub seconds_bound: Option<u64>,
    pub is_proven: bool,
    pub retries: u64,
    pub fallback: Option<Box<Expr>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SigModifier {
    Out,
    Inline,
    Export(Option<String>),
}

#[derive(Debug, Clone)]
pub struct Import {
    pub module: String,
    pub symbols: Vec<String>,
    pub span: Option<Span>,
}

#[derive(Debug, Clone)]
pub struct Meld {
    pub name: String,
    pub target: String,
    pub bindings: HashMap<String, String>,
    pub span: Option<Span>,
}

#[derive(Debug, Clone)]
pub struct Trigger {
    pub name: String,
    pub instance: Expr,
    pub port: String,
    pub span: Option<Span>,
}

#[derive(Debug, Clone)]
pub struct Annotation {
    pub name: String,
    pub value: Option<Expr>,
}

#[derive(Debug, Clone)]
pub struct TypeBinding {
    pub name: String,
    pub ty: Type,
    pub span: Option<Span>,
}
