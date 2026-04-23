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
}

#[derive(Debug, Clone, Deserialize)]
pub struct InterfaceConfig {
    pub name: String,
    pub address_width: Option<u32>,
    pub data_width: Option<u32>,
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
pub enum Type {
    Int,
    Float,
    String,
    Bool,
    Data,
    Void,
    UInt,
    Custom(String),
    Union(Vec<Type>),
    ContractBound(Box<Type>, Box<Expr>),
    TypeVar(String),
    Generic(String, Vec<Type>),
    Applied(String, Vec<Type>),
    Sig(String),
    Vector(Box<Type>, usize),
    Option(Box<Type>),
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
    Native, // Rust FFI (v6.2)
    Wasm,   // WebAssembly
    C,      // C library
    Python, // Python extension
    Js,     // JavaScript
    Swift,  // Swift
    Go,     // Go
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

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Integer(i64),
    Float(f64),
    String(String),
    Bool(bool),
    Identifier(String),
    OwnedRef(String),
    PriorState(String),
    Add(Box<Expr>, Box<Expr>),
    Sub(Box<Expr>, Box<Expr>),
    Mul(Box<Expr>, Box<Expr>),
    Div(Box<Expr>, Box<Expr>),
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
    Call(String, Vec<Expr>),
    ListLiteral(Vec<Expr>),
    ListIndex(Box<Expr>, Box<Expr>),
    ListLen(Box<Expr>),
    Slice {
        value: Box<Expr>,
        start: Option<Box<Expr>>,
        end: Option<Box<Expr>>,
        stride: Option<Box<Expr>>,
    },
    FieldAccess(Box<Expr>, String),
    StructInstance(String, Vec<(String, Expr)>),
    ObjectLiteral(Vec<(String, Expr)>),
    // Pattern matching in guards: [value Variant(field1, field2)] { ... }
    PatternMatch {
        value: Box<Expr>,
        variant: String,
        fields: Vec<String>,
    },
    ForAll {
        var: String,
        expr: Box<Expr>,
    },
    Exists {
        var: String,
        expr: Box<Expr>,
    },
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

            Expr::Not(e) | Expr::Neg(e) | Expr::BitNot(e) | Expr::ListLen(e) => {
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
            Expr::ForAll { expr, .. } | Expr::Exists { expr, .. } => {
                expr.extract_deps_recursive(deps);
            }
            _ => {} // Float, String, Bool don't add dependencies
        }
    }
}

#[derive(Debug, Clone)]
pub enum Statement {
    // Assignment: &lhs = expr; or lhs = expr;
    Assignment {
        lhs: Expr,
        expr: Expr,
        timeout: Option<(Expr, TimeUnit)>,
    },

    // Unification: identifier(pattern) = expr;
    Unification {
        name: String,
        pattern: String,
        expr: Expr,
    },

    // Guarded statement: [expr] statement or [expr] { statements }
    Guarded {
        condition: Expr,
        statements: Vec<Statement>, // Changed from single statement to vec
    },

    // Term statement: term expr?, expr?, ... (multi-output with trailing commas for void)
    Term(Vec<Option<Expr>>),

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
        bit_range: Option<BitRange>,
        is_override: bool,
    },
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

#[derive(Debug, Clone)]
pub struct Signature {
    pub name: String,
    pub input_types: Vec<Type>,
    pub result_type: ResultType,
    pub source: Option<String>,
    pub alias: Option<String>,
    /// NEW: Bind sig to a specific defn for path verification
    pub bound_defn: Option<String>,
}

/// Multi-output type structure for Feature A
/// Represents: Single | Union | Tuple | Mixed combinations
#[derive(Debug, Clone)]
pub enum OutputType {
    /// Single type: -> Bool
    Single(Type),

    /// Union of types: -> Bool | Error | Timeout (caller must handle all)
    Union(Vec<Type>),

    /// Tuple of types: -> Bool, String, Int (all produced, caller binds all)
    Tuple(Vec<Type>),
}

impl OutputType {
    /// Get all types in this output structure (flattened)
    pub fn all_types(&self) -> Vec<Type> {
        match self {
            OutputType::Single(ty) => vec![ty.clone()],
            OutputType::Union(types) => types.clone(),
            OutputType::Tuple(types) => types.clone(),
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

    /// Get number of output slots
    pub fn slot_count(&self) -> usize {
        match self {
            OutputType::Single(_) => 1,
            OutputType::Union(_) | OutputType::Tuple(_) => self.all_types().len(),
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
    pub is_lambda: bool, // Lambda-style: no body, postcondition must be provable
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
    pub is_lambda: bool, // Lambda-style: no body, postcondition must be provable
    pub dependencies: Vec<String>, // Variables read in preconditions
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
}

#[derive(Debug, Clone)]
pub struct TriggerDeclaration {
    pub name: String,
    pub ty: Type,
    pub address: u64,
    pub bit_range: Option<BitRange>,
    pub stages: Vec<String>,
    pub condition: Option<Expr>,
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

#[derive(Debug, Clone)]
pub enum TopLevel {
    Signature(Signature),
    Definition(Definition),
    Transaction(Transaction),
    StateDecl(StateDecl),
    Trigger(TriggerDeclaration),
    Constant(Constant),
    Import(Import),
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
}

#[derive(Debug, Clone)]
pub struct StructDefinition {
    pub name: String,
    pub fields: Vec<StructField>,
    pub transactions: Vec<Transaction>,
    pub view_html: Option<String>,
    pub span: Option<Span>,
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
            fields: Vec::new(),
            transactions: Vec::new(),
            view_html: None,
            span: None,
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

#[derive(Debug, Clone)]
pub struct Program {
    pub items: Vec<TopLevel>,
    pub comments: Vec<Comment>,
    pub reactor_speed: Option<u32>, // NEW: file-level @Hz default
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
            OutputType::Union(types) => types.clone(), // All must be handled
            OutputType::Tuple(types) => types.clone(), // All must be bound
        }
    }

    /// Check if caller's binding is sufficient for this output
    /// This is a placeholder for full exhaustiveness checking
    pub fn is_caller_binding_sufficient(&self, caller_type: &Type) -> bool {
        // For now: simple check
        // Future: implement full exhaustiveness verification
        match self {
            OutputType::Single(ty) => ty == caller_type,
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
