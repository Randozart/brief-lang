// ── Top-Level Declaration AST Definitions ──────────────────────────────
// 2026-07-12: Phase 0.2 — New architecture top-level types.
// No InopDeclaration — removed in 2026-07-22 Ship of Theseus cleanup.
// Added Export struct. 2026-08-01 (Phase 2): Contract.is_entry removed — the
// [#] entry-point marker is replaced by the entry!/args! macros (Phase 3).
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
    Obj(StructDefinition),
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
    /// Static struct: `struct Name { field: Type; }`. Fixed-layout, C-compatible.
    /// 2026-07-24: Pure data, no methods. Offsets computed from platform ABI.
    StaticStruct(StructDef),
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
    /// $defn name(params) -> Type { body } — compile-time-only definition.
    /// 2026-07-23: Top-level item, extracted before codegen.
    CompileTimeDefn(Definition),
    /// $txn name(params) [pre][post] -> Type { body } — compile-time-only tx.
    /// 2026-07-23: Convergent loop with pre/post, top-level before codegen.
    CompileTimeTxn(Transaction),
    /// 2026-07-29: Inline assembly function declaration.
    /// asm<x86_64> name(params) -> ReturnType { "instruction"; };
    AsmFn(AsmFn),
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
    // 2026-07-23: Protocol variant declaration: proto name: #Category { ... }
    ProtocolDef(ProtocolDef),
    /// 2026-08-05 (Phase 4): `trait Name<T> { ... }` — reusable behavioral
    /// requirements/defaults (SPEC §8.6). No storage or target meaning.
    Trait(TraitDef),
    /// 2026-08-05 (Phase 4): `impl Name<T> { ... }` — inherent behavior for
    /// data-only declarations (struct/enum/imported shape) (SPEC §8.8).
    Impl(ImplDef),
    /// $let name = expr; — compile-time mutable variable.
    /// 2026-07-25: Persists across stage blocks, removed before codegen.
    CompileTimeLet(String, Expr),
    /// $const name = expr; — compile-time immutable constant.
    /// 2026-07-25: Same lifetime, error on reassignment.
    CompileTimeConst(String, Expr),
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
    /// 2026-07-24: Doc comment text (/// or /** */), without the /// prefix.
    pub doc: Option<String>,
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
    /// 2026-07-24: Doc comment text.
    pub doc: Option<String>,
}

// ── Contract ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct Contract {
    pub pre_condition: Expr,
    pub post_condition: Expr,
    pub watchdog: Option<WatchdogSpec>,
    pub span: Option<Span>,
    /// 2026-07-31: Whether the contract was written explicitly in source.
    /// A defn/txn with no brackets has a default `[true][true]` — that is NOT
    /// a tautology; only an explicit `[true][true]` is rejected at proof time.
    pub explicit: bool,
}

impl Contract {
    pub fn new(pre: Expr, post: Expr) -> Self {
        Contract {
            pre_condition: pre,
            post_condition: post,
            watchdog: None,
            span: None,
            explicit: false,
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
    /// 2026-07-24: Doc comment text.
    pub doc: Option<String>,
}

// ── Statement ──────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum Statement {
    /// let name: Type = expr;
    Let {
        name: String,
        names: Vec<String>,
        ty: Option<Type>,
        expr: Option<Expr>,
        modifiers: Vec<Annotation>,
    },
    /// dest = expr;
    Assign(Expr, Expr),
    /// 2026-08-01 (Phase 3): the arrow — `dest <- value;` (copy into lhs),
    /// `dest ~<- value;` (copy into lhs then destroy/remove the rhs — the
    /// destructive extract), and `<- value;` / `~<- value;` discards (target
    /// None). The dispatch finds the collection by the op binding on each side
    /// (InsertAt on the lhs = insert; ExtractFrom/CopyFrom on the rhs = read
    /// or destructive extract) — the old `&` fake-pointer marker is removed.
    ArrowAssign {
        target: Option<Box<Expr>>,
        value: Box<Expr>,
        consume: bool,
    },
    /// term; or term expr;
    Term(Option<Expr>),
    /// exit program; or exit program code; — process boundary (replaces term!).
    /// 2026-08-05 (Phase 3): the interpreter signals program termination; true
    /// process-exit codegen is staged (SPEC §11.5).
    ExitProgram(Option<Expr>),
    /// when condition { body } or [condition] stmt;
    Guarded(Expr, Vec<Statement>),
    /// [condition]; — convergence gate (static assertion, re-convergence point)
    Gate(Expr),
    /// expr;
    Expression(Expr),
    /// if expr { ... } else { ... }
    If(Expr, Vec<Statement>, Vec<Statement>),
    /// { ... }
    Block(Vec<Statement>),
    /// key <~ value;
    MetadataAssignment(String, PropertyValue),
    /// escape expr;
    Rollback(Option<Expr>),
    /// 2026-08-01 (Phase 5): `free x;` — a VERIFIED lifetime hint: the backing
    /// of the local/field `x` is freed here; a later read of `x` is a compile
    /// error. The scheduler excludes a manually-freed field from its auto-free.
    FreeHint(String),
    /// 2026-08-01 (Phase 5): `keep x;` — a SUPPRESS hint: the scheduler must
    /// NOT auto-free `x` (it escapes or is freed elsewhere). A `keep` on a
    /// field the scheduler would not free anyway is redundant (a warning).
    KeepHint(String),
    /// foreach(item in list) { ... }
    Foreach {
        item: String,
        list: Box<Expr>,
        body: Vec<Statement>,
    },
    /// trg name @ instance; — whole-target form (the .port is removed).
    TrgBinding {
        name: String,
        instance: Expr,
    },
    /// asm "instruction" { clobbers }
    InlineAsm {
        asm_string: String,
        clobbers: Vec<String>,
        span: Option<Span>,
    },
    /// sync { ... }
    SyncBlock(Vec<Statement>),
    /// $defn name(params) -> Type { body } — compile-time-only definition.
    /// 2026-07-23: Only valid inside $(Stage) blocks. Body can call $ intrinsics.
    InlineDefn(Definition),
    /// $txn name(params) [pre][post] -> Type { body } — compile-time-only tx.
    /// 2026-07-23: Evaluated as a convergent loop with pre/post checks.
    InlineTxn(Transaction),
    /// match expr { pattern => body; ... }; — compile-time match.
    /// 2026-07-24: Added for clean $defn branching (replaces when chains).
    Match {
        expr: Box<Expr>,
        arms: Vec<StmtMatchArm>,
    },
}

/// 2026-07-24: A single arm in a statement-level match (inside $defn bodies).
/// Differs from ast::expr::MatchArm which is for expression-level match.
#[derive(Debug, Clone)]
pub struct StmtMatchArm {
    pub pattern: StmtMatchPattern,
    pub body: Vec<Statement>,
}

impl PartialEq for StmtMatchArm {
    fn eq(&self, other: &Self) -> bool {
        self.pattern == other.pattern && self.body == other.body
    }
}

/// 2026-07-24: A pattern in a statement-level match arm.
#[derive(Debug, Clone, PartialEq)]
pub enum StmtMatchPattern {
    Literal(i128),
    String(String),
    Wildcard,
    /// 2026-07-30: Multiple patterns separated by `|`: `0x30 | 0x31 => body;`
    Multi(Vec<StmtMatchPattern>),
}

// 2026-07-23: Manual PartialEq — InlineDefn/InlineTxn wrap Definition/Transaction
// which don't implement PartialEq. All other variants compare field-by-field.
impl PartialEq for Statement {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Statement::InlineDefn(_), _) | (_, Statement::InlineDefn(_)) => false,
            (Statement::InlineTxn(_), _) | (_, Statement::InlineTxn(_)) => false,
            (Statement::Let { name: n1, ty: t1, expr: e1, modifiers: m1, .. },
             Statement::Let { name: n2, ty: t2, expr: e2, modifiers: m2, .. }) =>
                n1 == n2 && t1 == t2 && e1 == e2 && m1 == m2,
            (Statement::Assign(l1, r1), Statement::Assign(l2, r2)) => l1 == l2 && r1 == r2,
             (Statement::Term(e1), Statement::Term(e2)) => e1 == e2,
             (Statement::ExitProgram(e1), Statement::ExitProgram(e2)) => e1 == e2,
             (Statement::Guarded(c1, b1), Statement::Guarded(c2, b2)) => c1 == c2 && b1 == b2,
            (Statement::Gate(c1), Statement::Gate(c2)) => c1 == c2,
            (Statement::Expression(e1), Statement::Expression(e2)) => e1 == e2,
            (Statement::If(c1, t1, e1), Statement::If(c2, t2, e2)) => c1 == c2 && t1 == t2 && e1 == e2,
            (Statement::Block(b1), Statement::Block(b2)) => b1 == b2,
            (Statement::MetadataAssignment(k1, v1), Statement::MetadataAssignment(k2, v2)) => k1 == k2 && v1 == v2,
            (Statement::Rollback(e1), Statement::Rollback(e2)) => e1 == e2,
            (Statement::Foreach { item: i1, list: l1, body: b1 },
             Statement::Foreach { item: i2, list: l2, body: b2 }) => i1 == i2 && l1 == l2 && b1 == b2,
            (Statement::TrgBinding { name: n1, instance: i1 },
             Statement::TrgBinding { name: n2, instance: i2 }) => n1 == n2 && i1 == i2,
            (Statement::InlineAsm { asm_string: a1, clobbers: c1, span: s1 },
             Statement::InlineAsm { asm_string: a2, clobbers: c2, span: s2 }) => a1 == a2 && c1 == c2 && s1 == s2,
            (Statement::SyncBlock(b1), Statement::SyncBlock(b2)) => b1 == b2,
            (Statement::Match { expr: e1, arms: a1 }, Statement::Match { expr: e2, arms: a2 }) => e1 == e2 && a1 == a2,
            (Statement::Match { .. }, _) | (_, Statement::Match { .. }) => false,
            _ => false,
        }
    }
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

#[derive(Debug, Clone, PartialEq)]
pub struct WatchdogSpec {
    pub condition: Expr,
    pub is_required: bool,
    pub cycles_bound: Option<u64>,
    pub seconds_bound: Option<u64>,
    /// 2026-08-01 (D2): the `within N ms/seconds/minute` deadline in
    /// NANOSECONDS (10 ms = 10_000_000). Integer `seconds_bound` truncates
    /// sub-second deadlines to 0; the emission compares Now#() - start against
    /// this directly.
    pub deadline_ns: Option<u64>,
    pub is_proven: bool,
    pub retries: u64,
    pub fallback: Option<Box<Expr>>,
    /// 2026-08-01 (C1): `-> handler(val)` on-fire callback. The loop calls the
    /// handler with the last computed value on the fire path — never a
    /// reference to state that may be reset.
    pub on_fire: Option<WatchdogOnFire>,
}

/// The `-> handler(val)` on-fire callback of a watchdog.
#[derive(Debug, Clone, PartialEq)]
pub struct WatchdogOnFire {
    pub handler: String,
    /// 2026-08-01 (C2): the value passed to the handler — `val` in
    /// `-> handler(val)`. Names the field/let whose current value the loop
    /// passes on the fire path (the "last computed value"). None for
    /// `-> handler()` (no argument).
    pub arg: Option<String>,
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
    /// 2026-07-26: from #System — protocol-based linking. #System is the sole protocol.
    Protocol(String),
    /// 2026-07-26: from #Link<user32> — link against system library -l<name>.
    /// No per-target config or registry lookup. `-l<name>` is emitted directly.
    Linked(String),
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
            Self::Protocol(_) => None,
            Self::Linked(_) => None,
        }
    }

    /// Return the string representation for registry matching.
    pub fn as_str(&self) -> String {
        match self {
            Self::Literal(p) => p.to_string_lossy().to_string(),
            Self::CompilerRegistry(n) => n.clone(),
            Self::Protocol(n) => n.clone(),
            Self::Linked(n) => format!("#Link<{}>", n),
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
    /// Call a Briv function with the frgn's parameters
    FnCall(String, Vec<Expr>),
    /// Void-return frgn — just skip the call
    Implicit,
    /// No fallback declared (codegen uses zero-value of return type)
    None,
}

/// Foreign function binding — a `frgn` declaration that wraps an external function.
/// 2026-07-22: Treated as an import. `foreign_name` is the C/foreign symbol the
/// linker looks for. `briv_name` (from `as` clause) is what Briv code calls it.
#[derive(Debug, Clone)]
pub struct ForeignBinding {
    /// The C/foreign symbol name — what the linker looks for in the foreign module.
    pub foreign_name: String,
    /// The Briv name for this foreign function. `None` means `foreign_name` is used.
    /// Set via the `as <briv_name>` clause in `frgn` declarations.
    pub briv_name: Option<String>,
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
    /// 2026-07-24: Doc comment text.
    pub doc: Option<String>,
    /// 2026-07-25: frgn? — optional, must check fn? before calling.
    pub is_optional: bool,
    /// 2026-07-25: frgn! — fire-and-forget, non-blocking, void return.
    pub is_fire_forget: bool,
    /// 2026-07-25: frgn?! — fire-and-forget with Bool(delivered) return.
    pub is_delivery: bool,
}

impl ForeignBinding {
    /// 2026-07-22: Construct a ForeignBinding.
    pub fn new(
        foreign_name: String,
        briv_name: Option<String>,
        from: FromSpec,
        target: ForeignTarget,
        fallback: Fallback,
    ) -> Self {
        ForeignBinding {
            foreign_name,
            briv_name,
            from,
            target,
            inputs: vec![],
            success_output: vec![],
            error_type: "Error".to_string(),
            error_fields: vec![],
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
            doc: None,
            is_optional: false,
            is_fire_forget: false,
            is_delivery: false,
        }
    }

    /// The effective Briv name — uses `briv_name` if set, otherwise `foreign_name`.
    pub fn effective_briv_name(&self) -> &str {
        self.briv_name.as_deref().unwrap_or(&self.foreign_name)
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
pub struct StructDef {
    pub name: String,
    /// 2026-07-31: Type parameters (`struct ListBuffer<T>`) — a generic,
    /// C-compatible data container. Instantiated at first use.
    pub type_params: Vec<TypeParam>,
    pub fields: Vec<(String, Type)>,
    pub metadata: HashMap<String, PropertyValue>,
    pub span: Option<Span>,
    /// 2026-08-05 (Phase 4): `seq struct` preserves field order and
    /// containment (SPEC §8.2). A plain struct is layout-adaptive.
    pub seq: bool,
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
    /// 2026-07-24: Parent type (e.g., Int for i64). Optional.
    /// Replaces the old `base` field which was `Box<Expr>` from `<:` syntax.
    pub parent: Option<Box<Expr>>,
    /// 2026-07-24: Protocol hashword (e.g., "#Int", "#String"). Optional.
    /// If both parent and protocol are None, the type is abstract.
    pub protocol: Option<String>,
    pub bit_range: Option<BitRange>,
    pub body: TypeDefBody,
    pub span: Option<Span>,
}

/// 2026-08-05 (Phase 4): `trait Name<T> { ... }` — reusable behavioral
/// requirements, logical field requirements, defaults, and op bindings.
/// Structural conformance is verified in Phase 5.
#[derive(Debug, Clone)]
pub struct TraitDef {
    pub name: String,
    pub type_params: Vec<TypeParam>,
    /// Required function signatures (no body) and default functions (body).
    pub functions: Vec<Definition>,
    /// Required/default op bindings.
    pub op_bindings: Vec<OperatorBinding>,
    /// Logical field requirements (`Size: Int;`).
    pub fields: Vec<(String, Type)>,
    pub span: Option<Span>,
}

/// 2026-08-05 (Phase 4): `impl Name<T> { ... }` — inherent behavior attached to
/// a data-only declaration. Coherence rules enforced in Phase 5.
#[derive(Debug, Clone)]
pub struct ImplDef {
    /// The target declaration name (`impl Point { ... }`).
    pub target: String,
    pub type_params: Vec<TypeParam>,
    pub functions: Vec<Definition>,
    pub op_bindings: Vec<OperatorBinding>,
    pub span: Option<Span>,
}

#[derive(Debug, Clone)]
pub struct TypeDefBody {
    pub slots: Vec<TypeDefSlot>,
    pub metadata: HashMap<String, PropertyValue>,
    pub projections: Vec<ProjectionDef>,
    pub bindings: Vec<TypeBinding>,
    pub operators: Vec<OperatorDef>,
    /// 2026-07-26: Operator bindings: op Name(Proto?): expr;
    pub op_bindings: Vec<OperatorBinding>,
    pub constraints: Vec<Expr>,
    /// 2026-07-31: obj member declarations (txn/defn) — self-parameterized
    /// methods on the obj. Populated by parse_obj_like.
    pub members: Vec<TopLevel>,
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

/// 2026-07-26: Operator binding: op Name(Proto?): expr;
/// Replaces the old op Name(Types) -> Type = fn(#L, #R) form.
/// protocol_variant is None for concrete bindings (InsertAt, Init, etc.)
/// or Some("#Int") / Some("MyType") for overloaded operators.
/// pre/suf/reg are discriminator fields for op Parse:
///   op Parse(Decimal, pre:"0x", reg:"[0-9a-fA-F]+"): parse_hex(#L);
#[derive(Debug, Clone)]
pub struct OperatorBinding {
    pub name: String,
    pub protocol_variant: Option<String>,
    pub pre: Option<String>,
    pub suf: Option<String>,
    pub reg: Option<String>,
    pub expr: Expr,
    pub span: Option<Span>,
}

// 2026-07-23: Protocol variant declaration: proto name: #Category { ... }
// Defines a protocol variant with CastTo/CastFrom edges and optional contract.
#[derive(Debug, Clone)]
pub struct ProtocolDef {
    pub name: String,
    pub category: String,
    pub contract: Option<Contract>,
    pub cast_edges: Vec<CastEdge>,
    pub cross_ops: Vec<OperatorDef>,
    pub span: Option<Span>,
}

#[derive(Debug, Clone)]
pub struct CastEdge {
    pub direction: CastDirection,
    pub target_category: String,
    pub target_variant: String,
    /// 2026-07-23: Required binding — the transformation function.
    /// e.g., CastTo(#String<UTF8>) = ASCII_to_UTF8(#L);
    pub binding: Option<CastBinding>,
}

#[derive(Debug, Clone)]
pub struct CastBinding {
    /// The function name or expression defining the transform.
    pub fn_name: String,
    /// The parameter slot (#L for self, #R for target).
    pub param: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CastDirection {
    CastTo,
    CastFrom,
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

// ── AsmFn ─────────────────────────────────────────────────────────────

/// 2026-07-29: Assembly function declaration.
/// asm<x86_64> name(params) -> ReturnType { "instruction"; };
/// Target-annotated inline assembly body. Cross-verified against other
/// bodies in the := chain at compile time (see verification-chain plan).
#[derive(Debug, Clone, PartialEq)]
pub struct AsmFn {
    pub target: String,
    pub name: String,
    pub params: Vec<(String, Type)>,
    pub ret_type: Type,
    /// 2026-08-05 (Phase 6): contracts are mandatory (present and non-trivial)
    /// on asm declarations (SPEC §20).
    pub contract: Contract,
    pub body: Vec<String>,
    pub span: Span,
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
