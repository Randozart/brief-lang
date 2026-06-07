// Copyright 2026 Randy Smits-Schreuder Goedheijt
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.
//
// Runtime Exception for Use as a Language:
// When the Work or any Derivative Work thereof is used to generate code
// ("generated code"), such generated code shall not be subject to the
// terms of this License, provided that the generated code itself is not
// a Derivative Work of the Work. This exception does not apply to code
// that is itself a compiler, interpreter, or similar tool that incorporates
// or embeds the Work.

use crate::errors::Span;
use crate::ffi::types::MemoryLayout;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Deserialize)]
pub struct HardwareConfig {
    pub project: ProjectConfig,
    pub target: TargetConfig,
    pub interface: InterfaceConfig,
    pub memory: HashMap<String, MemoryMapping>,
    pub io: Option<HashMap<String, IoMapping>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ProjectConfig {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TargetConfig {
    pub fpga: String,
    pub clock_hz: u32,
    #[serde(default)]
    pub platform: Option<String>,
    #[serde(default)]
    pub synthesis: Option<SynthesisConfig>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct SynthesisConfig {
    #[serde(default)]
    pub mode: String,
    #[serde(default)]
    pub max_jobs: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InterfaceConfig {
    pub name: String,
    pub address_width: Option<u32>,
    pub data_width: Option<u32>,
    #[serde(default)]
    pub controller: Option<String>,
    #[serde(default)]
    pub situs: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MemoryMapping {
    pub size: usize,
    #[serde(rename = "type")]
    pub mem_type: String,
    pub element_bits: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct IoMapping {
    pub pin: String,
    pub direction: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TimeUnit {
    Cycles,
    Ms,
    Seconds,
    Minutes,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BitRange {
    Single(usize),
    Range(usize, usize),
    Any(usize), // /xN
}



#[derive(Debug, Clone, PartialEq)]
pub enum Dimension {
    Anonymous(usize),
    Named(String, usize),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Int,
    Float,
    String,
    Bool,
    Data,
    Void,
    UInt,
    Char,  // Unicode codepoint type
    // Note: HashMap, HashSet, StringBuilder, Stack, Queue, Option
    // are defined as regular structs/enums in stdlib, not as AST variants.
    // This keeps the language philosophically pure - no magic types.
    Custom(String),
    Union(Vec<Type>),
    Tuple(Vec<Type>),
    ContractBound(Box<Type>, Box<Expr>),
    TypeVar(String),
    Generic(String, Vec<Type>),
    Applied(String, Vec<Type>),
    Sig(String),
    Vector(Box<Type>, Vec<Dimension>),
    Enum(String),
    Constrained(Box<Type>, BitRange),
}

#[derive(Debug, Clone)]
pub struct TypeParam {
    pub name: String,
    pub bounds: Vec<TypeBound>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TypeBound {
    Eq(Type),
    SubTypeOf(Type),
    SuperTypeOf(Type),
    HasTrait(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ResultType {
    Projection(Vec<Type>),
    TrueAssertion,
    VoidType,
}

/// Foreign Function Target Platform
#[derive(Debug, Clone, PartialEq)]
pub enum ForeignTarget {
    Native,       // Rust FFI (v6.2)
    Wasm,         // WebAssembly
    C,            // C library
    Python,       // Python extension
    Js,           // JavaScript
    Swift,        // Swift
    Go,           // Go
    Metropolitan, // Metropolitan FFI (shared memory IPC)
}

impl std::fmt::Display for ForeignTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ForeignTarget::Native => write!(f, "native"),
            ForeignTarget::Wasm => write!(f, "wasm"),
            ForeignTarget::C => write!(f, "c"),
            ForeignTarget::Python => write!(f, "python"),
            ForeignTarget::Js => write!(f, "js"),
            ForeignTarget::Swift => write!(f, "swift"),
            ForeignTarget::Go => write!(f, "go"),
            ForeignTarget::Metropolitan => write!(f, "metropolitan"),
        }
    }
}

/// The kind of FFI call determines error handling
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FfiKind {
    Frgn,        // Foreign function -> Result<T, Error>
    FrgnBang,    // Foreign function -> void (fire-and-forget)
    Syscall,     // Kernel call -> Result<Int, Error>
    SyscallBang, // Kernel call -> void (fire-and-forget)
}

/// Foreign Function Signature (from frgn declaration)
#[derive(Debug, Clone)]
pub struct ForeignSignature {
    pub name: String,
    pub location: String,            // TOML location (e.g., "std::f64::sqrt")
    pub wasm_impl: Option<String>,   // WASM JavaScript implementation
    pub wasm_setup: Option<String>,  // WASM JavaScript setup/imports
    pub inputs: Vec<(String, Type)>, // param_name -> type
    pub success_output: Vec<(String, Type)>, // named fields (can be empty for void)
    pub result_type: ResultType,
    pub error_type_name: String,     // e.g., "IoError"
    pub error_fields: Vec<(String, Type)>, // error shape
    pub input_layout: Option<MemoryLayout>, // Explicit layout (NEW v2)
    pub output_layout: Option<MemoryLayout>, // Explicit layout (NEW v2)
    pub precondition: Option<String>, // Pre-call validation (NEW v2)
    pub postcondition: Option<String>, // Post-call validation (NEW v2)
    pub buffer_mode: Option<String>, // stack | heap | static
    pub ffi_kind: Option<FfiKind>,   // NEW: frgn, frgn!, syscall, syscall!
    pub is_out: bool,                // #out modifier — function has observable output
    pub span: Option<Span>,
}

/// Resource declaration (rsrc/resource)
#[derive(Debug, Clone)]
pub struct ResourceDeclaration {
    pub name: String,
    pub resource_type: String, // FrameBuffer, File, etc.
    pub args: Vec<i64>,        // Constructor args: width, height, etc.
    pub span: Option<Span>,
}

/// Foreign Function Binding (loaded from TOML)
#[derive(Debug, Clone)]
pub struct ForeignBinding {
    pub name: String,
    pub description: Option<String>,
    pub location: String, // Rust module path: std::fs::read_to_string
    pub target: ForeignTarget,
    pub mapper: Option<String>, // Mapper name (e.g., "rust", "c", "wasm")
    pub path: Option<String>,   // Explicit path to mapper (optional)
    pub wasm_impl: Option<String>, // WASM JavaScript implementation (for wasm target)
    pub wasm_setup: Option<String>, // WASM JavaScript setup/imports
    pub inputs: Vec<(String, Type)>, // Parameter names and types
    pub success_output: Vec<(String, Type)>, // Success output shape
    pub error_type: String,     // Error type name
    pub error_fields: Vec<(String, Type)>, // Error fields
    pub input_layout: Option<MemoryLayout>, // Explicit layout (NEW v2)
    pub output_layout: Option<MemoryLayout>, // Explicit layout (NEW v2)
    pub precondition: Option<String>, // Pre-call validation (NEW v2)
    pub postcondition: Option<String>, // Post-call validation (NEW v2)
    pub buffer_mode: Option<String>, // stack | heap | static
}

impl ForeignBinding {
    pub fn new(name: String, location: String, target: ForeignTarget) -> Self {
        Self {
            name,
            description: None,
            location,
            target,
            mapper: None,
            path: None,
            wasm_impl: None,
            wasm_setup: None,
            inputs: Vec::new(),
            success_output: Vec::new(),
            error_type: "Error".to_string(),
            error_fields: Vec::new(),
            input_layout: None,
            output_layout: None,
            precondition: None,
            postcondition: None,
            buffer_mode: None,
        }
    }

    pub fn from_signature(sig: &ForeignSignature) -> Self {
        Self {
            name: sig.name.clone(),
            description: None,
            location: sig.location.clone(),
            target: ForeignTarget::Native, // Default
            mapper: None,
            path: None,
            wasm_impl: sig.wasm_impl.clone(),
            wasm_setup: sig.wasm_setup.clone(),
            inputs: sig.inputs.clone(),
            success_output: sig.success_output.clone(),
            error_type: sig.error_type_name.clone(),
            error_fields: sig.error_fields.clone(),
            input_layout: sig.input_layout.clone(),
            output_layout: sig.output_layout.clone(),
            precondition: sig.precondition.clone(),
            postcondition: sig.postcondition.clone(),
            buffer_mode: sig.buffer_mode.clone(),
        }
    }
}

/// A single coordinate in a multidimensional slice
#[derive(Debug, Clone, PartialEq)]
pub enum SliceCoordinate {
    /// Single index: `5`
    Index(Box<Expr>),
    /// Range: `0..10`, `5..`, `..10`
    Range { start: Option<Box<Expr>>, end: Option<Box<Expr>> },
    /// Named dimension: `time:5` or `time:0..10`
    Named { name: String, coord: Box<SliceCoordinate> },
    /// `@dim: coord` — positional dimension targeting
    AtDimension { dimension: usize, coord: Box<SliceCoordinate> },
    /// `...` — ellipsis, expands to fill all unspecified dimensions
    Ellipsis,
}

/// Direction of arrow mutation
#[derive(Debug, Clone, PartialEq)]
pub enum ArrowDir {
    /// `&list <- x` — value flows into the list
    Push,
    /// `x <- &list` — value flows out of the list
    Pop,
}

/// Target of a `:>` projection: `expr :> Size`
#[derive(Debug, Clone, PartialEq)]
pub enum ProjectionTarget {
    Size,
    Bytes,
    Ptr,
    Alignment,
    Range,
    Popcount,
    LeadingZeros,
    TrailingZeros,
    Absolute,
    BitReverse,
    Type,
    PtrBang,
    /// Compile-time DFA regex: `input :> Match("pattern")`
    Match(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Integer(i64),
    Float(f64),
    String(String),
    Char(char),  // NEW: Char literal
    Bool(bool),
    Term,
    Identifier(String),
    OwnedRef(String),
    PriorState(String),
    /// `...` — ellipsis, expands to fill unspecified dimensions in bracket context
    Ellipsis,
    /// Collection structural mutation: `&list <- x`, `x <- &list`, or `&list[i] <- x`
    /// `index` is `Expr::Term` for full-range (end operations)
    ArrowMut {
        dir: ArrowDir,
        target: Box<Expr>,
        index: Box<Expr>,
        value: Option<Box<Expr>>,
    },
    /// Discard pop/remove: `<- &list` or `<- &list[i]`
    ArrowDiscard {
        target: Box<Expr>,
        index: Box<Expr>,
    },
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
    Or(Box<Expr>, Box<Expr>),
    And(Box<Expr>, Box<Expr>),
    Not(Box<Expr>),
    Neg(Box<Expr>),
    BitNot(Box<Expr>),
    BitAnd(Box<Expr>, Box<Expr>),
    BitOr(Box<Expr>, Box<Expr>),
    BitXor(Box<Expr>, Box<Expr>),
    Shl(Box<Expr>, Box<Expr>),
    Shr(Box<Expr>, Box<Expr>),
    Concat(Box<Expr>, Box<Expr>),
    /// Type cast: expr as Type
    Cast(Box<Expr>, Type),
    /// Compile-time metadata projection: `expr :> Size`
    Projection {
        source: Box<Expr>,
        target: ProjectionTarget,
    },
    Call(String, Vec<Expr>),
    ListLiteral(Vec<Expr>),
    ListIndex(Box<Expr>, Box<Expr>),
    Slice {
        value: Box<Expr>,
        start: Option<Box<Expr>>,
        end: Option<Box<Expr>>,
        stride: Option<Box<Expr>>,
        mask: Option<Box<Expr>>,
    },
    // Multidimensional slice: vec[coord1, coord2, ...; mask]
    MultiSlice {
        value: Box<Expr>,
        coordinates: Vec<SliceCoordinate>,
        mask: Option<Box<Expr>>,
    },

    FieldAccess(Box<Expr>, String),
    StructInstance(String, Vec<(String, Expr)>),
    ObjectLiteral(Vec<(String, Expr)>),
// Pattern matching in guards: [value Variant(field1, field2)] { ... }
    PatternMatch {
        value: Box<Expr>,
        variant: String,
        fields: Vec<Pattern>,
    },
    // Match expression: match value { Variant(f1) => body, _ => default }
    Match {
        value: Box<Expr>,
        arms: Vec<MatchArm>,
    },
    // Block expression: { stmts...; last_expr }
    Block(Vec<Statement>, Box<Expr>),
    // Tuple destructuring: let (a, b) = expr;
    TupleDestructure(Vec<String>, Box<Expr>),
    // Tuple literal: (a, b, c)
    Tuple(Vec<Expr>),
    /// Sig call modifier: `sig #out expr` or `sig #inline expr`
    SigCall {
        modifier: SigModifier,
        expr: Box<Expr>,
    },
}

/// A pattern in a match arm: `Variant(f1, f2)` or `_`
#[derive(Debug, Clone, PartialEq)]
pub enum MatchPattern {
    Wildcard,
    Variant { name: String, fields: Vec<Pattern> },
}

/// A single arm in a match expression
#[derive(Debug, Clone, PartialEq)]
pub struct MatchArm {
    pub pattern: MatchPattern,
    pub guard: Option<Box<Expr>>,
    pub body: Box<Expr>,
}

impl std::fmt::Display for Pattern {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Pattern::Var(name) => write!(f, "{}", name),
            Pattern::Wildcard => write!(f, "_"),
            Pattern::Tuple(elems) => {
                let inner = elems.iter().map(|e| e.to_string()).collect::<Vec<_>>().join(", ");
                write!(f, "({})", inner)
            }
            Pattern::LitInt(n) => write!(f, "{}", n),
            Pattern::LitFloat(n) => write!(f, "{}", n),
            Pattern::LitString(s) => write!(f, "\"{}\"", s),
            Pattern::LitChar(c) => write!(f, "'{}'", c),
            Pattern::LitBool(b) => write!(f, "{}", b),
        }
    }
}

impl Expr {
    pub fn span(&self) -> Option<Span> {
        None
    }

    pub fn extract_dependencies(&self) -> HashSet<String> {
        let mut deps = HashSet::new();
        self.extract_deps_recursive(&mut deps);
        deps
    }

    fn extract_deps_recursive(&self, deps: &mut HashSet<String>) {
        match self {
            Expr::Identifier(name) => {
                deps.insert(name.clone());
            }
            Expr::OwnedRef(name) => {
                deps.insert(name.clone());
            }
            Expr::PriorState(name) => {
                deps.insert(name.clone());
            }
            Expr::Add(l, r)
            | Expr::Sub(l, r)
            | Expr::Mul(l, r)
            | Expr::Div(l, r)
            | Expr::BitAnd(l, r)
            | Expr::BitOr(l, r)
            | Expr::BitXor(l, r)
            | Expr::Eq(l, r)
            | Expr::Ne(l, r)
            | Expr::Lt(l, r)
            | Expr::Le(l, r)
            | Expr::Gt(l, r)
            | Expr::Ge(l, r)
            | Expr::Or(l, r)
            | Expr::And(l, r) => {
                l.extract_deps_recursive(deps);
                r.extract_deps_recursive(deps);
            }

            Expr::Not(e) | Expr::Neg(e) | Expr::BitNot(e) | Expr::Projection { source: e, .. } => {
                e.extract_deps_recursive(deps);
            }
            Expr::Call(_, args) | Expr::ListLiteral(args) => {
                for arg in args {
                    arg.extract_deps_recursive(deps);
                }
            }
            Expr::ListIndex(l, i) => {
                l.extract_deps_recursive(deps);
                i.extract_deps_recursive(deps);
            }
            Expr::Slice {
                value,
                start,
                end,
                stride,
                mask,
            } => {
                value.extract_deps_recursive(deps);
                if let Some(s) = start {
                    s.extract_deps_recursive(deps);
                }
                if let Some(e) = end {
                    e.extract_deps_recursive(deps);
                }
                if let Some(st) = stride {
                    st.extract_deps_recursive(deps);
                }
                if let Some(m) = mask {
                    m.extract_deps_recursive(deps);
                }
            }
            Expr::FieldAccess(e, _) => {
                e.extract_deps_recursive(deps);
            }
            Expr::StructInstance(_, fields) | Expr::ObjectLiteral(fields) => {
                for (_, expr) in fields {
                    expr.extract_deps_recursive(deps);
                }
            }
            Expr::PatternMatch { value, .. } => {
                value.extract_deps_recursive(deps);
            }
            _ => {} // Float, String, Bool don't add dependencies
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Pattern {
    Var(String),
    Wildcard,
    Tuple(Vec<Pattern>),
    LitInt(i64),
    LitFloat(f64),
    LitString(String),
    LitChar(char),
    LitBool(bool),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    // Assignment: &lhs = expr; or lhs = expr;
    Assignment {
        lhs: Expr,
        expr: Expr,
        timeout: Option<(Expr, TimeUnit)>,
        modifiers: Vec<Hashtag>,
    },

    // Unification: identifier(pattern) = expr;
    Unification {
        name: String,
        variant: String,
        fields: Vec<Pattern>,
        expr: Expr,
    },

    // Guarded statement: [expr] statement or [expr] { statements }
    Guarded {
        condition: Expr,
        statements: Vec<Statement>,
    },

    // Term statement: term expr?, expr?, ...
    // Optional -> swan_song executes only on postcondition acceptance (commit action)
    Term {
        values: Vec<Option<Expr>>,
        swan_song: Option<Box<Statement>>,
        modifiers: Vec<Hashtag>,
    },

    // TermBang statement: term! expr?, expr?, ...
    // Program exit with centralized exit block; optional -> cleanup
    TermBang {
        values: Vec<Option<Expr>>,
        swan_song: Option<Box<Statement>>,
        modifiers: Vec<Hashtag>,
    },

    // Escape statement: escape expr?;
    Escape(Option<Expr>),

    // Expression statement: expr;
    Expression(Expr),

    // Let binding: let name: Type = expr;
    Let {
        name: String,
        ty: Option<Type>,
        expr: Option<Expr>,
        address: Option<u64>,
        address_expr: Option<Box<Expr>>,
        bit_range: Option<BitRange>,
        is_override: bool,
        modifiers: Vec<Hashtag>,
    },

    // Inline assembly: asm "instruction" { "clobber1", "clobber2" };
    InlineAsm {
        asm_string: String,
        clobbers: Vec<String>,
        span: Option<Span>,
    },

    // Local trigger declaration (inside transactions): trg! name: Type = expr;
    // The ! suffix is a psychological speedbump warning of async rollback risk
    LocalTrigger {
        name: String,
        ty: Type,
        expr: Option<Expr>,
        span: Option<Span>,
    },

    // Alka escape hatch: alka { ... }; or alka! { ... };
    Alka(AlkaBlock),

    // Block pragma: #on_exit { ... };
    OnExit {
        body: Vec<Statement>,
        span: Option<Span>,
    },

    // Sync block: sync { stmt1; stmt2; ... };
    // Fork-join barrier — all statements start and finish simultaneously.
    SyncBlock {
        body: Vec<Statement>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct AlkaBlock {
    pub dangerous: bool,
    pub content: String,
    pub span: Option<Span>,
}

#[derive(Debug, Clone)]
pub struct Contract {
    pub pre_condition: Expr,
    pub post_condition: Expr,
    pub watchdog: Option<WatchdogSpec>,
    pub span: Option<Span>,
}

#[derive(Debug, Clone)]
pub struct WatchdogSpec {
    pub condition: Expr,
    pub is_required: bool,  // false = ? (optional), true = ! (required)
}

impl Contract {
    pub fn new(pre: Expr, post: Expr) -> Self {
        Contract {
            pre_condition: pre,
            post_condition: post,
            watchdog: None,
            span: None,
        }
    }
}

/// Side-effect modifier for sig declarations.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SigModifier {
    /// `sig #out` — function has observable external effects
    Out,
    /// `sig #inline` — function is pure, safe to fold/eliminate
    Inline,
}

#[derive(Debug, Clone)]
pub struct Signature {
    pub name: String,
    /// Parameter list: (name: Type, ...)
    pub params: Vec<(String, Type)>,
    pub result_type: ResultType,
    pub source: Option<String>,
    pub alias: Option<String>,
    /// NEW: Bind sig to a specific defn for path verification
    pub bound_defn: Option<String>,
    /// sig modifier: #out, #inline, or None
    pub modifier: Option<SigModifier>,
    /// Complex output type structure (union/tuple/array/named)
    pub output_type: Option<OutputType>,
}

impl Signature {
    /// Convenience: get just the parameter types (drop names)
    pub fn input_types(&self) -> Vec<Type> {
        self.params.iter().map(|(_, t)| t.clone()).collect()
    }
}

/// Multi-output type structure for Feature A
/// Represents: Single | Union | Tuple | Mixed combinations
#[derive(Debug, Clone)]
pub enum OutputType {
    /// Single type: -> Bool
    Single(Type),

    /// Union of types: -> Bool | Error | Timeout (caller must handle all)
    Union(Vec<OutputType>),

    /// Tuple of types: -> Bool, String, Int (all produced, caller binds all)
    Tuple(Vec<OutputType>),

    /// Array of types: -> Bool[] (dynamic-length collection)
    Array(Box<Type>),

    /// Named slot: -> name: Type (labeled output for destructuring)
    Named(String, Box<OutputType>),
}

impl OutputType {
    /// Get all types in this output structure (flattened)
    pub fn all_types(&self) -> Vec<Type> {
        match self {
            OutputType::Single(ty) => vec![ty.clone()],
            OutputType::Union(types) => types.iter().flat_map(|ot| ot.all_types()).collect(),
            OutputType::Tuple(types) => types.iter().flat_map(|ot| ot.all_types()).collect(),
            OutputType::Array(ty) => vec![ty.as_ref().clone()],
            OutputType::Named(_, inner) => inner.all_types(),
        }
    }

    /// Check if this is a union type (multiple alternatives)
    pub fn is_union(&self) -> bool {
        matches!(self, OutputType::Union(_))
    }

    /// Check if this is a tuple type (all required)
    pub fn is_tuple(&self) -> bool {
        matches!(self, OutputType::Tuple(_))
    }

    /// Check if this is an array type
    pub fn is_array(&self) -> bool {
        matches!(self, OutputType::Array(_))
    }

    /// Check if this is a named slot
    pub fn is_named(&self) -> bool {
        matches!(self, OutputType::Named(_, _))
    }

    /// Get number of output slots
    pub fn slot_count(&self) -> usize {
        match self {
            OutputType::Single(_) | OutputType::Array(_) => 1,
            OutputType::Named(_, inner) => inner.slot_count(),
            OutputType::Union(types) => types.len(),
            OutputType::Tuple(types) => types.len(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Definition {
    pub name: String,
    pub type_params: Vec<TypeParam>,
    pub parameters: Vec<(String, Type)>,
    pub outputs: Vec<Type>,
    pub output_type: Option<OutputType>,
    pub output_names: Vec<Option<String>>,
    pub contract: Contract,
    pub body: Vec<Statement>,
    pub is_lambda: bool,
    pub modifiers: Vec<Hashtag>,
    pub variant_bodies: Vec<(Option<Contract>, Vec<Statement>)>,
}

#[derive(Debug, Clone)]
pub struct Transaction {
    pub is_async: bool,
    pub is_reactive: bool,
    pub name: String,
    pub parameters: Vec<(String, Type)>,
    pub contract: Contract,
    pub body: Vec<Statement>,
    pub reactor_speed: Option<u32>,
    pub span: Option<Span>,
    pub is_lambda: bool,
    pub dependencies: Vec<String>,
    pub attrs: Vec<Attribute>,
    pub modifiers: Vec<Hashtag>,
    pub variant_bodies: Vec<(Option<Contract>, Vec<Statement>)>,
    pub outputs: Vec<Type>,
    pub output_type: Option<OutputType>,
}

#[derive(Debug, Clone)]
pub struct Attribute {
    pub target: Option<String>,  // None = all targets, Some("c") = C only
    pub key: String,
    pub value: Option<String>,
}

#[derive(Debug, Clone)]
pub struct StateDecl {
    pub name: String,
    pub ty: Type,
    pub expr: Option<Expr>,
    pub address: Option<u64>,
    pub bit_range: Option<BitRange>,
    pub is_override: bool,
    pub os_mode: bool, // In OS mode, address is requested via ioctl/mmap; else embedded mode uses raw address
    pub span: Option<Span>,
    pub attrs: Vec<Attribute>,  // NEW: #[...] attributes
}

#[derive(Debug, Clone)]
pub enum LinkRef {
    Explicit(u64),
    Linked(String),
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
    pub span: Option<Span>,
}

#[derive(Debug, Clone)]
pub struct Constant {
    pub name: String,
    pub ty: Type,
    pub expr: Expr,
}

#[derive(Debug, Clone)]
pub struct Import {
    pub items: Vec<ImportItem>,
    pub path: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ImportItem {
    pub name: String,
    pub alias: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LinkLanguage {
    C,        // .c
    Cpp,      // .cpp / .cc / .cxx
    Rust,     // .rs
    Zig,      // .zig
    Python,   // .py
    Bitcode,  // .bc
    Object,   // .o / .a
}

#[derive(Debug, Clone)]
pub struct LinkDependency {
    pub path: String,
    pub source_lang: LinkLanguage,
}

#[derive(Debug, Clone)]
pub enum TopLevel {
    Signature(Signature),
    Definition(Definition),
    Transaction(Transaction),
    StateDecl(StateDecl),
    Trigger(TriggerDeclaration),
    Constant(Constant),
    Import(Import),
    LinkDependency(LinkDependency),
    ForeignBinding {
        name: String,
        toml_path: String,
        signature: ForeignSignature,
        target: ForeignTarget,
        span: Option<Span>,
    },
    ResourceDecl(ResourceDeclaration), // NEW: rsrc/resource
    Struct(StructDefinition),
    RStruct(RStructDefinition),
    Enum(EnumDefinition),
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
}

#[derive(Debug, Clone)]
pub struct StructVariant {
    pub contract: Option<Contract>,
    pub fields: Vec<StructField>,
    pub additions: Vec<StructField>,
    pub removals: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct StructDefinition {
    pub name: String,
    pub type_params: Vec<String>,
    pub fields: Vec<StructField>,
    pub transactions: Vec<Transaction>,
    pub view_html: Option<String>,
    pub span: Option<Span>,
    pub modifiers: Vec<Hashtag>,
    pub variants: Vec<StructVariant>,
}

#[derive(Debug, Clone)]
pub struct StructField {
    pub name: String,
    pub ty: Type,
    pub default: Option<Expr>,
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

impl StructDefinition {
    pub fn new(name: String) -> Self {
        StructDefinition {
            name,
            type_params: Vec::new(),
            fields: Vec::new(),
            transactions: Vec::new(),
            view_html: None,
            span: None,
            modifiers: Vec::new(),
            variants: Vec::new(),
        }
    }

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
pub struct RenderBlock {
    pub struct_name: String,
    pub view_html: String,
    pub span: Option<Span>,
}

#[derive(Debug, Clone)]
pub struct Comment {
    pub line: usize,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Hashtag {
    pub name: String,
    pub value: Option<String>,
    pub mandatory: bool,
    pub fallback: Vec<String>,
    pub scoped: Option<String>,
}

impl Hashtag {
    pub fn new(name: String) -> Self {
        Hashtag { name, value: None, mandatory: false, fallback: Vec::new(), scoped: None }
    }

    pub fn mandatory(name: String) -> Self {
        Hashtag { name, value: None, mandatory: true, fallback: Vec::new(), scoped: None }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StrictMode {
    Off,
    Strict,
}

impl StrictMode {
    pub fn is_strict(self) -> bool {
        matches!(self, StrictMode::Strict)
    }
}

/// Dispatch mode for the reactor loop.
/// Sequential (default): first-true-wins fallthrough chain.
/// Parallel: evaluate all preconditions upfront, fire every
/// non-conflicting transaction in one tick.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DispatchMode {
    Sequential,
    Parallel,
}

impl Default for DispatchMode {
    fn default() -> Self {
        DispatchMode::Sequential
    }
}

#[derive(Debug, Clone)]
pub struct Program {
    pub items: Vec<TopLevel>,
    pub comments: Vec<Comment>,
    pub reactor_speed: Option<u32>, // NEW: file-level @Hz default
    pub attrs: Vec<Attribute>,  // NEW: file-level #![...] attributes
    pub ffi: Option<FfiState>,  // NEW: FFI state from #![ffi.*, ...]
    pub strict_mode: StrictMode,
    pub dispatch_mode: DispatchMode,
    pub exit_condition: Option<Box<Expr>>, // NEW: #!exit <expr>;
    pub out_pragmas: Vec<String>,         // NEW: #!out(x, y);
    /// Default sig modifier for the file scope: Some(Out) or Some(Inline)
    pub default_sig_modifier: Option<SigModifier>,
}

/// FFI State captured from file-level attribute
/// Example: #![ffi.c, bind("./c.toml"), import("./libc.a"), map("uint","uint32_t")]
#[derive(Debug, Clone)]
pub struct FfiState {
    pub lang: String,              // "c", "js", "rust", etc.
    pub bind_path: Option<String>, // Profile TOML path
    pub import_path: Option<String>, // Script/library path
    pub global_maps: Vec<(String, String)>, // [(brief_type, foreign_type)]
}

/// Helper for exhaustiveness checking (Feature A)
impl OutputType {
    /// Determine what types the CALLER must handle
    /// For union types: caller must handle each type
    /// For tuple types: caller must bind all slots
    /// For single: caller binds one type
    pub fn required_caller_bindings(&self) -> Vec<Type> {
        match self {
            OutputType::Single(ty) => vec![ty.clone()],
            OutputType::Union(types) => types.iter().flat_map(|ot| ot.all_types()).collect(),
            OutputType::Tuple(types) => types.iter().flat_map(|ot| ot.all_types()).collect(),
            OutputType::Array(ty) => vec![ty.as_ref().clone()],
            OutputType::Named(_, inner) => inner.required_caller_bindings(),
        }
    }

    /// Check if caller's binding is sufficient for this output
    /// This is a placeholder for full exhaustiveness checking
    pub fn is_caller_binding_sufficient(&self, caller_type: &Type) -> bool {
        // For now: simple check
        // Future: implement full exhaustiveness verification
        match self {
            OutputType::Single(ty) => ty == caller_type,
            OutputType::Array(ty) => ty.as_ref() == caller_type,
            OutputType::Named(_, inner) => inner.is_caller_binding_sufficient(caller_type),
            OutputType::Union(_) => true, // Deferred to type checker
            OutputType::Tuple(_) => true, // Deferred to type checker
        }
    }
}

/// Sig Casting Support (Feature B)
/// Allows projecting specific output types from multi-output functions
#[derive(Debug, Clone)]
pub struct SigProjection {
    /// The signature name being projected to
    pub sig_name: String,

    /// The types this sig projects from the defn
    pub projected_types: Vec<Type>,

    /// The source defn this sig casts from
    pub source_defn: String,
}
