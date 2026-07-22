// ── Top-Level Declaration AST Definitions ──────────────────────────────
// 2026-07-12: Phase 0.2 — New architecture top-level types.
// No InopDeclaration, no TopLevel::Inop.
// Added Export struct and Contract.is_entry.
//
// 2026-07-13: Added backend-compat types (TriggerDeclaration, ForeignSignature,
// EnumDefinition, etc.) and expanded TopLevel variants so that legacy backend
// code (mod.rs, dispatch.rs, circt.rs, verilog.rs, c.rs) can still pattern-match.
// These will be migrated to the new AST types as each backend is rewritten.

use crate::ast::{BitRange, DerivationBlock, Expr, Formatting, PropertyValue, Type};
use crate::errors::Span;
use std::collections::HashMap;
use std::path::PathBuf;

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
    // ── Backend-compat variants (old AST) ─────────────────────────────
    Constant(Constant),
    ForeignBinding(ForeignBinding),
    Inop(InopDeclaration),
    Struct(StructDefinition),
    Enum(EnumDefinition),
    TriggerBinding {
        name: String,
        ty: Option<Type>,
        instance: Expr,
        port: String,
        modifiers: Vec<Annotation>,
    },
    StateDecl(StateDecl),
    Signature(Signature),
    LinkDependency(LinkDependency),
    ResourceDecl(ResourceDeclaration),
    RStruct(RStructDefinition),
    TypeDef(Box<TypeDef>),
    Codec(CodecDeclaration),
    Assertion {
        pre: Expr,
        chain: Vec<String>,
    },
    Fuzzed {
        item: Box<TopLevel>,
        cases: Vec<FuzzCase>,
    },
    Statement(Box<Statement>),
    // 2026-07-15: $(Stage) compile-time metaprogramming block
    StageBlock(StageBlock),
    RenderBlock(RenderBlock),
    Stylesheet(String),
    SvgComponent {
        name: String,
        content: String,
    },
    SyncGroup {
        domains: Vec<String>,
        item: Box<TopLevel>,
    },
    Cfg(CfgGuard),
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
    pub output_type: Option<OutputType>,
    pub outputs: Vec<Type>,
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

#[derive(Debug, Clone, PartialEq)]
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

    pub fn all_types(&self) -> Vec<Type> {
        match self {
            OutputType::Single(ty) => vec![ty.clone()],
            OutputType::Union(types) | OutputType::Tuple(types) => {
                types.iter().flat_map(|t| t.all_types()).collect()
            }
            OutputType::Array(inner) => inner.all_types(),
            OutputType::Named(_, inner) => inner.all_types(),
        }
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

/// Whether an import path is a literal file path or a registry lookup.
#[derive(Debug, Clone, PartialEq)]
pub enum ImportKind {
    /// import "path" — file/project-anchored path
    Literal(String),
    /// import <name> — compiler registry lookup
    Registry(String),
}

#[derive(Debug, Clone)]
pub struct Import {
    pub kind: ImportKind,
    pub symbols: Vec<String>,
    pub span: Option<Span>,
}

impl Import {
    /// Get the path string regardless of import kind.
    /// For `Literal("path")` returns `"path"`.
    /// For `Registry("name")` returns `"name"` (without angle brackets).
    pub fn path(&self) -> &str {
        match &self.kind {
            ImportKind::Literal(p) => p.as_str(),
            ImportKind::Registry(n) => n.as_str(),
        }
    }

    /// Compatibility accessor — returns the same as `path()`.
    /// Allows existing code using `.module` to continue working
    /// after the field was renamed to `.kind`.
    pub fn module(&self) -> &str {
        self.path()
    }

    /// Create a new literal import with the given path.
    pub fn literal(path: impl Into<String>, symbols: Vec<String>) -> Self {
        Import {
            kind: ImportKind::Literal(path.into()),
            symbols,
            span: None,
        }
    }

    /// Create a new registry import with the given name.
    pub fn registry(name: impl Into<String>, symbols: Vec<String>) -> Self {
        Import {
            kind: ImportKind::Registry(name.into()),
            symbols,
            span: None,
        }
    }
}

/// A compile-time $(Stage) block.
/// The body is executed at compile time during the specified pipeline stage.
/// 2026-07-15: Phase 1b — Plugin architecture.
#[derive(Debug, Clone)]
pub struct StageBlock {
    pub stage: StageKind,
    pub priority: u32,
    pub body: Vec<Statement>,
    pub span: Option<Span>,
}

/// Pipeline stages at which compile-time plugins can run.
/// 2026-07-21: Expanded from 4 to 11 granular stages.
/// Each stage maps to one compiler pass and has a default data target:
///   PreLex        — Source$ (source text, mutable)
///   Parsed–Provenanced — AST (implicit, tree operations)
///   Generated–Optimized — Ir$ (text operations)
///   Linked        — Bin$ (binary path operations)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum StageKind {
    /// Raw source text before lexing. Default: Source$.
    PreLex,
    /// Freshly parsed AST, imports not resolved. Default: AST.
    Parsed,
    /// All imports resolved and merged. Default: AST.
    Resolved,
    /// Type checking complete. Full TypeUniverse. Default: AST.
    Typed,
    /// Backend normalization applied. Default: AST.
    Normalized,
    /// Protocol round-trip verification done. Default: AST.
    Verified,
    /// Allocation strategies assigned. Default: AST.
    Allocated,
    /// Pointer provenance validated. Default: AST.
    Provenanced,
    /// Backend IR generated (.ll, .mlir, .ts). Default: Ir$.
    Generated,
    /// Backend optimizations applied. Default: Ir$.
    Optimized,
    /// Final binary linked. Default: Bin$.
    Linked,
}

impl StageKind {
    /// True if this stage operates on AST data (tree operations).
    pub fn is_ast_stage(&self) -> bool {
        matches!(self, StageKind::Parsed | StageKind::Resolved | StageKind::Typed
            | StageKind::Normalized | StageKind::Verified | StageKind::Allocated
            | StageKind::Provenanced)
    }

    /// True if this stage operates on IR text.
    pub fn is_ir_stage(&self) -> bool {
        matches!(self, StageKind::Generated | StageKind::Optimized)
    }
}

#[derive(Debug, Clone)]
pub struct Meld {
    pub name: String,
    pub target: String,
    pub bindings: HashMap<String, String>,
    pub span: Option<Span>,
}

/// A meld route defines how one type's field is derived from another type.
#[derive(Debug, Clone)]
pub struct MeldRouteDef {
    /// The projection target name (e.g. "Ptr", "Size")
    pub accessor: String,
    /// The expression that computes this field from the partner type
    pub dest_expr: Expr,
}

/// A bidirectional type compatibility declaration.
/// Allows viewing type A as type B and vice versa via named routes.
#[derive(Debug, Clone)]
pub struct MeldDeclaration {
    pub name_a: String,
    pub name_b: String,
    pub routes: Vec<MeldRouteDef>,
    pub span: Option<Span>,
}

#[derive(Debug, Clone)]
pub struct Trigger {
    pub name: String,
    pub instance: Expr,
    pub port: String,
    pub span: Option<Span>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Annotation {
    pub name: String,
    pub value: Option<Expr>,
}

impl Annotation {
    /// Check if this annotation is speculative (#? prefix).
    pub fn speculative(&self) -> bool {
        self.name.starts_with('?')
    }

    /// Extract the string value from the annotation's value field.
    pub fn string_value(&self) -> Option<String> {
        self.value.as_ref().and_then(|v| {
            if let Expr::Quoted(bytes) = v {
                Some(String::from_utf8_lossy(bytes).to_string())
            } else {
                None
            }
        })
    }
}

#[derive(Debug, Clone)]
pub struct TypeBinding {
    pub name: String,
    pub ty: Type,
    pub span: Option<Span>,
}

// ── Backend-Compat Types (old AST) ─────────────────────────────────────

#[derive(Debug, Clone)]
pub struct Constant {
    pub name: String,
    pub ty: Type,
    pub expr: Expr,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ResultType {
    Projection(Vec<Type>),
    TrueAssertion,
    VoidType,
}

impl ResultType {
    /// 2026-07-16: P5 — Get the first return type, if any.
    pub fn return_type(&self) -> Option<Type> {
        match self {
            ResultType::Projection(ts) => ts.first().cloned(),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ForeignTarget {
    Native,
    Wasm,
    C,
    Python,
    Js,
    Swift,
    Go,
    Metropolitan,
}

impl ForeignTarget {
    /// 2026-07-16: P3 — Look up a target variant by string name.
    pub fn from_name(name: &str) -> Option<ForeignTarget> {
        match name {
            "native" => Some(ForeignTarget::Native),
            "wasm" => Some(ForeignTarget::Wasm),
            "c" => Some(ForeignTarget::C),
            "python" => Some(ForeignTarget::Python),
            "js" => Some(ForeignTarget::Js),
            "swift" => Some(ForeignTarget::Swift),
            "go" => Some(ForeignTarget::Go),
            "metropolitan" => Some(ForeignTarget::Metropolitan),
            _ => None,
        }
    }
}

/// 2026-07-16: P3 — Where a frgn function's implementation comes from.
#[derive(Debug, Clone)]
pub enum FromSpec {
    /// from "path/to/file" — literal path (CWD-relative or absolute).
    Literal(PathBuf),
    /// from <name> — compiler-relative lookup (same pattern as import <name>).
    CompilerRegistry(String),
}

impl Default for FromSpec {
    fn default() -> Self {
        Self::Literal(PathBuf::new())
    }
}

impl FromSpec {
    /// Extract the file extension for convention derivation.
    pub fn extension(&self) -> Option<String> {
        match self {
            Self::Literal(p) => p.extension().and_then(|s| s.to_str()).map(|s| s.to_string()),
            Self::CompilerRegistry(name) => name.rsplit('.').next().map(|s| s.to_string()),
        }
    }

    /// Return the string representation for registry matching.
    pub fn as_str(&self) -> String {
        match self {
            Self::Literal(p) => p.to_string_lossy().to_string(),
            Self::CompilerRegistry(n) => n.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ForeignSignature {
    pub name: String,
    pub from: FromSpec,
    pub inputs: Vec<(String, Type)>,
    pub result_type: ResultType,
    pub wasm_impl: Option<String>,
    pub wasm_setup: Option<String>,
    pub span: Option<Span>,
}

impl Default for ForeignSignature {
    fn default() -> Self {
        ForeignSignature {
            name: String::new(),
            from: FromSpec::default(),
            inputs: Vec::new(),
            result_type: ResultType::VoidType,
            wasm_impl: None,
            wasm_setup: None,
            span: None,
        }
    }
}

/// 2026-07-22: Fallback strategy when a frgn call's return violates its
/// contract or the foreign function cannot be reached.
/// The program must always produce a valid result — this is the safety net.
#[derive(Debug, Clone, PartialEq)]
pub enum Fallback {
    /// Return a static expression (literal, constructor call, etc.)
    Static(Expr),
    /// Call a Brief function with the frgn's parameters
    FnCall(String, Vec<Expr>),
    /// Void-return frgn — just skip the call
    Implicit,
    /// No fallback declared (codegen uses zero-value of return type)
    None,
}

/// Foreign function binding — a `frgn` declaration that wraps an external function.
#[derive(Debug, Clone)]
pub struct ForeignBinding {
    pub name: String,
    /// 2026-07-22: The foreign symbol name when it differs from `name`.
    /// `None` means the foreign symbol equals `name`.
    pub as_name: Option<String>,
    pub from: FromSpec,
    pub target: ForeignTarget,
    pub inputs: Vec<(String, Type)>,
    pub success_output: Vec<(String, Type)>,
    pub error_type: String,
    pub error_fields: Vec<(String, Type)>,
    pub input_layout: Option<()>,
    pub output_layout: Option<()>,
    pub precondition: Option<String>,
    pub postcondition: Option<String>,
    pub buffer_mode: Option<String>,
    pub default_watchdog: Option<(u64, u64, u64, Box<Expr>)>,
    pub wasm_impl: Option<String>,
    pub wasm_setup: Option<String>,
    /// 2026-07-22: Fallback strategy when the foreign call fails.
    pub fallback: Fallback,
    pub span: Option<Span>,
}

impl ForeignBinding {
    /// 2026-07-22: Extended with `as_name` and `fallback` for the
    /// frgn/export/GLUE architecture. `as_name` renames the foreign symbol;
    /// `fallback` declares what happens when the foreign call fails.
    pub fn new(
        name: String,
        as_name: Option<String>,
        from: FromSpec,
        target: ForeignTarget,
        fallback: Fallback,
    ) -> Self {
        ForeignBinding {
            name,
            as_name,
            from,
            target,
            inputs: Vec::new(),
            success_output: Vec::new(),
            error_type: "Error".to_string(),
            error_fields: Vec::new(),
            input_layout: None,
            output_layout: None,
            precondition: None,
            postcondition: None,
            buffer_mode: None,
            default_watchdog: None,
            wasm_impl: None,
            wasm_setup: None,
            fallback,
            span: None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum LinkRef {
    Explicit(u64),
    Linked(String),
    Stdin,
    Timer(u64),
    Signal(String),
    Deref(Box<Expr>),
}

#[derive(Debug, Clone)]
pub struct TriggerDeclaration {
    pub name: String,
    pub ty: Type,
    pub address: LinkRef,
    pub bit_range: Option<BitRange>,
    pub stages: Vec<String>,
    pub condition: Option<Expr>,
    pub is_wake: bool,
    pub is_const: bool,
    pub span: Option<Span>,
    pub annotations: Vec<TypeBinding>,
    pub modifiers: Vec<Annotation>,
}

#[derive(Debug, Clone)]
pub struct InopDeclaration {
    pub name: String,
    pub params: Vec<(String, Type)>,
    pub outputs: Vec<Type>,
    pub contract: Contract,
    pub llvm_body: Vec<String>,
    pub has_side_effects: bool,
    pub has_state_access: bool,
    pub span: Option<Span>,
}

#[derive(Debug, Clone)]
pub struct EnumDefinition {
    pub name: String,
    pub type_params: Vec<TypeParam>,
    pub variants: Vec<EnumVariant>,
    pub span: Option<Span>,
}

#[derive(Debug, Clone)]
pub enum EnumVariant {
    Unit(String),
    Tuple(String, Vec<Type>),
    Struct(String, Vec<(String, Type)>),
}

#[derive(Debug, Clone)]
pub struct StateDecl {
    pub name: String,
    pub ty: Type,
    pub span: Option<Span>,
}

#[derive(Debug, Clone)]
pub struct Signature {
    pub name: String,
    pub params: Vec<(String, Type)>,
    pub outputs: Vec<Type>,
    pub span: Option<Span>,
}

#[derive(Debug, Clone)]
pub struct LinkDependency {
    pub path: String,
    pub source_lang: LinkLanguage,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LinkLanguage {
    C, Cpp, Rust, Zig, Python, Java, AssemblyScript, Bitcode, Object,
}

#[derive(Debug, Clone)]
pub struct ResourceDeclaration {
    pub name: String,
    pub ty: Type,
    pub span: Option<Span>,
}

#[derive(Debug, Clone)]
pub struct RStructDefinition {
    pub name: String,
    pub fields: Vec<StructField>,
    pub transactions: Vec<Transaction>,
    pub view_html: String,
    pub span: Option<Span>,
}

#[derive(Debug, Clone)]
pub struct StructDefinition {
    pub name: String,
    pub type_params: Vec<String>,
    pub parent: Option<Type>,
    pub fields: Vec<StructField>,
    pub transactions: Vec<Transaction>,
    pub view_html: Option<String>,
    pub span: Option<Span>,
    pub modifiers: Vec<Annotation>,
    pub variants: Vec<StructVariant>,
}

#[derive(Debug, Clone)]
pub struct StructVariant {
    pub contract: Option<Contract>,
    pub fields: Vec<StructField>,
    pub additions: Vec<StructField>,
    pub removals: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Visibility {
    Public,
    Sedentary,
    Private,
}

#[derive(Debug, Clone)]
pub struct StructField {
    pub name: String,
    pub ty: Type,
    pub default: Option<Expr>,
    pub visibility: Visibility,
}

#[derive(Debug, Clone)]
pub struct TypeDef {
    pub name: String,
    pub type_params: Vec<TypeParam>,
    pub base: Box<Expr>,
    pub bit_range: Option<BitRange>,
    pub body: TypeDefBody,
    pub span: Option<Span>,
}

#[derive(Debug, Clone)]
pub struct TypeDefBody {
    pub slots: Vec<TypeDefSlot>,
    pub metadata: HashMap<String, PropertyValue>,
    pub projections: Vec<ProjectionDef>,
    pub bindings: Vec<TypeBinding>,
    pub operators: Vec<OperatorDef>,
    pub constraints: Vec<Expr>,
    pub span: Option<Span>,
}

#[derive(Debug, Clone)]
pub struct TypeDefSlot {
    pub name: String,
    pub ty: Type,
    pub bit_range: Option<BitRange>,
}

#[derive(Debug, Clone)]
pub struct ProjectionDef {
    pub name: String,
    pub expr: Expr,
    pub span: Option<Span>,
}

/// 2026-07-20: Operator definition from a type body.
/// Two forms:
///   op Add(#Int, #Int);              — declarative: params are hashword categories
///   op InsertAt(#RingBuffer) = fn(#L,#R); — binding: explicit implementation
///
/// The `impl_args` field stores the implementation function + marker references
/// as a PropertyValue, matching the format produced by parse_metadata_value_standalone():
///   Identifier("ring_push")                    — convention-based (no markers)
///   List([Identifier("ring_push"), HashL, HashR]) — marker-based (#L = first arg)
#[derive(Debug, Clone)]
pub struct OperatorDef {
    pub op: String,
    /// 2026-07-20: Parameter types (hashwords or concrete types).
    pub params: Vec<Type>,
    /// 2026-07-20: Optional prefix discriminator for Parse ops: pre: "0x"
    pub pre: Option<String>,
    /// 2026-07-20: Optional suffix discriminator for Parse ops: suf: "h"
    pub suf: Option<String>,
    /// 2026-07-20: Implementation args: fn name + marker references.
    /// None for declarative ops, Some(PropertyValue) for binding ops.
    pub impl_args: Option<PropertyValue>,
    /// Old-style implementation name string (from `op Add ~> "string"`).
    pub impl_name: String,
    pub span: Option<Span>,
}

#[derive(Debug, Clone)]
pub struct CodecDeclaration {
    pub name: String,
    pub parse_handler: Option<String>,
    pub format_handler: Option<String>,
    pub constraints: Vec<Expr>,
    pub span: Option<Span>,
}

#[derive(Debug, Clone)]
pub struct FuzzCase {
    pub bindings: Vec<(String, Expr)>,
    pub expected: Expr,
    pub span: Option<Span>,
}

#[derive(Debug, Clone)]
pub struct RenderBlock {
    pub struct_name: String,
    pub view_html: String,
    pub span: Option<Span>,
}

#[derive(Debug, Clone)]
pub struct CfgGuard {
    pub condition: CfgCondition,
    pub items: Vec<TopLevel>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CfgCondition {
    Eq(String, String),
    Ne(String, String),
    And(Box<CfgCondition>, Box<CfgCondition>),
    Or(Box<CfgCondition>, Box<CfgCondition>),
    Not(Box<CfgCondition>),
    Bool(bool),
}

impl CfgCondition {
    pub fn evaluate(&self, target_os: &str, target_arch: &str, board: &str) -> Result<bool, String> {
        match self {
            CfgCondition::Eq(key, val) => {
                let actual = match key.as_str() {
                    "target_os" => target_os,
                    "target_arch" => target_arch,
                    "board" => board,
                    _ => return Err(format!("unknown cfg key \"{}\"", key)),
                };
                Ok(actual == val)
            }
            CfgCondition::Ne(key, val) => {
                let actual = match key.as_str() {
                    "target_os" => target_os,
                    "target_arch" => target_arch,
                    "board" => board,
                    _ => return Err(format!("unknown cfg key \"{}\"", key)),
                };
                Ok(actual != val)
            }
            CfgCondition::And(a, b) => {
                if !a.evaluate(target_os, target_arch, board)? { return Ok(false); }
                b.evaluate(target_os, target_arch, board)
            }
            CfgCondition::Or(a, b) => {
                if a.evaluate(target_os, target_arch, board)? { return Ok(true); }
                b.evaluate(target_os, target_arch, board)
            }
            CfgCondition::Not(c) => {
                Ok(!c.evaluate(target_os, target_arch, board)?)
            }
            CfgCondition::Bool(b) => Ok(*b),
        }
    }
}
