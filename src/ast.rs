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
use crate::features::binary_op::BinaryOpExpr;
use crate::features::call::CallExpr;
use crate::features::collection::*;
use crate::features::field::*;
use crate::features::literal::LiteralExpr;
use crate::features::projection::ProjectionExpr;
use crate::features::arrow::*;
use crate::features::block::BlockExpr;
use crate::features::dbvl::DbvlTableExpr;
use crate::features::ellipsis::EllipsisExpr;
use crate::features::pattern::*;
use crate::features::sigcall::SigCallExpr;
use crate::features::subtype::SubtypeProjectionExpr;
use crate::features::tuple::*;
use crate::features::unary_op::UnaryOpExpr;
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
    Nanoseconds,
}

/// Layout constraint for a universal pointer — the safe void* equivalent.
/// Describes the spatial shape of the pointee: byte size and alignment.
/// Pointers parameterized by `LayoutConstraint` can point to ANY type
/// matching these dimensions. Operations are gated to spatial-only
/// (memcpy, memcmp, hash, volatile load/store) when the pointee is Bits.
/// `Ptr64` desugars to `LayoutConstraint { bytes: 8, alignment: 8 }`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LayoutConstraint {
    pub bytes: u64,
    pub alignment: u64,
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
    Int8,
    Int16,
    Int32,
    Float,
    Float64,
    String,
    Bool,
    Data,
    Void,
    UInt,
    UInt8,
    UInt16,
    UInt32,
    Char,  // Unicode codepoint type
    // Note: HashMap, HashSet, StringBuilder, Stack, Queue, Option
    // are defined as regular structs/enums in stdlib, not as AST variants.
    // This keeps the language philosophically pure - no magic types.
    Custom(String),
    Union(Vec<Type>),
    Tuple(Vec<Type>),
    TypeVar(String),
    Generic(String, Vec<Type>),
    Applied(String, Vec<Type>),
    Sig(String),
    Vector(Box<Type>, Vec<Dimension>),
    Enum(String),
    Constrained(Box<Type>, BitRange),
    /// Layout-constrained universal pointer. Desugars from `Ptr<N>` or `Ptr<Bits @/0..N>`.
    /// Carries byte size and alignment for the pointee. Operations are spatial-only
    /// when the pointee is `Bits` — no semantic interpretation (add, field access, etc.).
    LayoutPtr(LayoutConstraint),
}

// 2026-06-29: Fixed-width type helpers for the explicit-width types feature
// bit_width() and is_signed() enable the typechecker, interpreter, and
// backends to handle all integer/float widths uniformly without match
// duplication. Used by typechecker (binary_op_type, is_cast_valid,
// types_are_width_compatible) and LLVM backend (type mapping, codegen).
impl Type {
    pub fn bit_width(&self) -> Option<u64> {
        match self {
            Type::Int8 | Type::UInt8 => Some(8),
            Type::Int16 | Type::UInt16 => Some(16),
            Type::Int32 | Type::UInt32 => Some(32),
            Type::Int | Type::UInt => Some(64),
            Type::Float => Some(32),
            Type::Float64 => Some(64),
            _ => None,
        }
    }

    pub fn is_signed(&self) -> Option<bool> {
        match self {
            Type::Int8 | Type::Int16 | Type::Int32 | Type::Int => Some(true),
            Type::UInt8 | Type::UInt16 | Type::UInt32 | Type::UInt => Some(false),
            Type::Float | Type::Float64 => Some(true), // floats are signed in representation
            _ => None,
        }
    }

    // 2026-06-29: Returns true for any fixed-width integer type
    // Used by typechecker's is_cast_valid to allow all integer↔integer casts
    pub fn is_integral(&self) -> bool {
        matches!(self,
            Type::Int8 | Type::Int16 | Type::Int32 | Type::Int |
            Type::UInt8 | Type::UInt16 | Type::UInt32 | Type::UInt
        )
    }

    // 2026-06-29: Returns true for any fixed-width float type
    // Used by typechecker's is_cast_valid to allow float↔float casts
    pub fn is_float_type(&self) -> bool {
        matches!(self, Type::Float | Type::Float64)
    }

    // 2026-06-29: Returns true for any numeric type (integer or float)
    // Used by typechecker's is_cast_valid to allow cross-family casts
    pub fn is_numeric(&self) -> bool {
        self.is_integral() || self.is_float_type()
    }

    // ── Phase 7A: Canonical universe key for backend property lookups ──
    //
    // 2026-06-29: Maps every Type variant to its canonical name for
    // TypeUniverse lookup. This is the SOLE place in the codebase where
    // Type variants are matched for backend representation decisions.
    // All other backend functions (llvm_type, tbaa_node, byte_size, etc.)
    // use this key to query the universe.
    //
    // Returns "Int" as safe fallback for types without a universe entry.
    pub fn universe_key(&self) -> &str {
        match self {
            Type::Int => "Int",
            Type::UInt => "UInt",
            Type::Int8 => "Int8",
            Type::UInt8 => "UInt8",
            Type::Int16 => "Int16",
            Type::UInt16 => "UInt16",
            Type::Int32 => "Int32",
            Type::UInt32 => "UInt32",
            Type::Float => "Float",
            Type::Float64 => "Float64",
            Type::Bool => "Bool",
            Type::Char => "Char",
            Type::String => "String",
            Type::Data => "Data",
            Type::Void => "Void",
            Type::Custom(name) => name.as_str(),
            Type::Enum(name) => name.as_str(),
            Type::Sig(name) => name.as_str(),
            // Compound types without universe entries: use Int as safe default
            Type::Union(_) | Type::Tuple(_) | Type::TypeVar(_)
            | Type::Generic(_, _) | Type::Applied(_, _)
            | Type::Vector(_, _) | Type::Constrained(_, _)
            | Type::LayoutPtr(_) => "Int",
        }
    }

    // 2026-07-03: Normalize `Ptr<Bits @/0..N>` to `LayoutPtr`.
    // Also normalizes `Ptr<Int>` (stays as Applied), `Ptr8` (already LayoutPtr),
    // and nested occurrences inside compound types.
    // Call this after parsing, before type checking, to canonicalize
    // layout-constrained pointers.
    pub fn normalize_layout_ptr(self) -> Type {
        match self {
            // Ptr<Bits @/range> → LayoutPtr
            Type::Applied(name, mut args) if name == "Ptr" && args.len() == 1 => {
                let inner = args.remove(0);
                match inner {
                    Type::Constrained(inner_ty, br) if matches!(*inner_ty, Type::Data) => {
                        let bits = match br {
                            BitRange::Range(start, end) => end - start + 1,
                            BitRange::Single(_) => 1,
                            BitRange::Any(n) => n,
                        };
                        let bytes = (bits + 7) / 8;
                        Type::LayoutPtr(LayoutConstraint { bytes: bytes as u64, alignment: bytes as u64 })
                    }
                    // Other Ptr<T> where T is not Bits @/range stays as Applied
                    other => {
                        let inner = other.normalize_layout_ptr();
                        Type::Applied(name, vec![inner])
                    }
                }
            }
            // Recurse into compound types
            Type::Applied(name, args) => {
                let args = args.into_iter().map(|a| a.normalize_layout_ptr()).collect();
                Type::Applied(name, args)
            }
            Type::Union(types) => {
                Type::Union(types.into_iter().map(|t| t.normalize_layout_ptr()).collect())
            }
            Type::Tuple(types) => {
                Type::Tuple(types.into_iter().map(|t| t.normalize_layout_ptr()).collect())
            }
            Type::Generic(name, args) => {
                Type::Generic(name, args.into_iter().map(|a| a.normalize_layout_ptr()).collect())
            }
            Type::Vector(inner, dims) => {
                Type::Vector(Box::new(inner.normalize_layout_ptr()), dims)
            }
            Type::Constrained(inner, br) => {
                Type::Constrained(Box::new(inner.normalize_layout_ptr()), br)
            }
            // All other types stay as-is
            other => other,
        }
    }
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
    pub is_pipe: bool,               // true if pipe syntax `-> T | fallback` was used
    pub fallback: Option<Expr>,      // fallback expression for pipe syntax
    pub default_watchdog: Option<(u64, TimeUnit, u64, Box<Expr>)>,
    pub span: Option<Span>,
}

impl Default for ForeignSignature {
    fn default() -> Self {
        ForeignSignature {
            name: String::new(),
            location: String::new(),
            wasm_impl: None,
            wasm_setup: None,
            inputs: Vec::new(),
            success_output: Vec::new(),
            result_type: ResultType::VoidType,
            error_type_name: "Error".to_string(),
            error_fields: Vec::new(),
            input_layout: None,
            output_layout: None,
            precondition: None,
            postcondition: None,
            buffer_mode: None,
            ffi_kind: None,
            is_out: false,
            is_pipe: false,
            fallback: None,
            default_watchdog: None,
            span: None,
        }
    }
}

impl ForeignSignature {
    pub fn new(name: String, location: String) -> Self {
        ForeignSignature {
            name,
            location,
            wasm_impl: None,
            wasm_setup: None,
            inputs: Vec::new(),
            success_output: Vec::new(),
            result_type: ResultType::VoidType,
            error_type_name: "Error".to_string(),
            error_fields: Vec::new(),
            input_layout: None,
            output_layout: None,
            precondition: None,
            postcondition: None,
            buffer_mode: None,
            ffi_kind: None,
            is_out: false,
            is_pipe: false,
            fallback: None,
            default_watchdog: None,
            span: None,
        }
    }
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
    pub default_watchdog: Option<(u64, TimeUnit, u64, Box<Expr>)>,
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
            default_watchdog: None,
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
            default_watchdog: None,
        }
    }

    /// True if this binding came from a pipe-syntax frgn declaration.
    pub fn is_pipe(&self) -> bool {
        false // ForeignBinding (TOML) never has pipe syntax
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

/// A single operation inside a `MultiSlice` bracket expression.
/// `list[::3 ; age >= 18 ::2]` parses to `[Stride(3), Mask(age >= 18), Stride(2)]`.
#[derive(Debug, Clone, PartialEq)]
pub enum BracketOp {
    /// Dimension coordinate: `5`, `0..10`, `time:5`, `@dim`, `...`
    Coord(SliceCoordinate),
    /// Filter/mask: `; age >= 18`
    Mask(Box<Expr>),
    /// Stride: `::3`
    Stride(Box<Expr>),
}

/// A named binding inside a `Type Name <: Base { ... }` block.
/// Each binding is a `Name = Expr;` or `Name(args) = Expr;` assignment.
/// This is the unified representation — both built-in metadata properties
/// and user-defined projections use this same struct.
#[derive(Debug, Clone, PartialEq)]
pub struct TypeBinding {
    /// The binding name (e.g., "Bytes", "Size", "At").
    pub name: String,
    /// Optional parameter names for parameterized projections (e.g., `At(i)`, `Contains(k)`).
    pub params: Vec<String>,
    /// The expression this binding resolves to.
    pub value: Box<Expr>,
    /// Source span for error reporting.
    pub span: Option<Span>,
}

/// Body of a `Type Name <: Base { ... }` declaration.
#[derive(Debug, Clone)]
pub struct TypeDefBody {
    /// Unified bindings: every `Name = Expr;` entry, including both metadata
    /// properties and user-defined projections.
    pub bindings: Vec<TypeBinding>,
    /// Operator declarations: `op Rune(Param) -> Ret = intrinsic;`
    /// 2026-06-29: Phase 7B — user-facing operator→intrinsic mappings.
    pub operators: Vec<OpDeclaration>,
    /// Refinement constraints with implicit self: `[ > 0 ]`.
    pub constraints: Vec<Expr>,
    /// Source span for error reporting.
    pub span: Option<Span>,
}

/// Operator rune — the symbolic operator being overloaded.
/// 2026-06-29: Phase 7B.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum OpRune {
    Add, Sub, Mul, Div, Mod, Neg,
    Eq, Ne, Lt, Le, Gt, Ge,
    And, Or, Not,
    Index, Slice,
    ArrowPush, ArrowPop,
    Cast, Box, Unbox,
}

impl OpRune {
    pub fn is_infix(&self) -> bool {
        matches!(self, OpRune::Add | OpRune::Sub | OpRune::Mul | OpRune::Div
            | OpRune::Mod | OpRune::Eq | OpRune::Ne | OpRune::Lt
            | OpRune::Le | OpRune::Gt | OpRune::Ge | OpRune::And
            | OpRune::Or | OpRune::ArrowPush)
    }
    pub fn is_prefix(&self) -> bool {
        matches!(self, OpRune::Neg | OpRune::Not | OpRune::Box | OpRune::Unbox)
    }
    pub fn is_postfix(&self) -> bool {
        matches!(self, OpRune::Index | OpRune::Slice | OpRune::Cast)
    }
}

/// An operator declaration inside a type body.
/// 2026-06-29: Phase 7B.
#[derive(Debug, Clone)]
pub struct OpDeclaration {
    pub rune: OpRune,
    pub param_type: Option<Box<Expr>>,
    pub return_type: Box<Expr>,
    pub implementation: Box<Expr>,
    pub span: Option<Span>,
}

/// A `Type Name <: Base { ... }` declaration — Pass 1: type universe.
#[derive(Debug, Clone)]
pub struct TypeDef {
    /// The new type's name.
    pub name: String,
    /// Type parameters (e.g. `T`, `K` in `List<T, K>`).
    pub type_params: Vec<String>,
    /// The base type expression (e.g. `Bits`, `List<T>`).
    pub base: Box<Expr>,
    /// Optional bit range for `Bits @/0..7` syntax.
    pub bit_range: Option<BitRange>,
    /// Property body.
    pub body: TypeDefBody,
    /// Source span.
    pub span: Option<Span>,
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
    /// Returns a List of all keys in a HashMap: `map :> Keys`
    Keys,
    /// Returns a List of all values in a HashMap: `map :> Values`
    Values,
    /// Checks if a HashMap or HashSet contains a value: `map :> Contains("key")`
    Contains(Box<Expr>),
    /// Returns true if the collection/string/tuple has zero elements: `list :> IsEmpty`
    IsEmpty,
    /// Non-mutating HashMap read: `map :> Get(key)` → Option<V>
    Get(Box<Expr>),
    /// Stack peek: `stack :> Top` → Option<V>
    Top,
    /// Queue front: `queue :> Front` → Option<V>
    Front,
    /// HashSet enumeration: `set :> Elements` → List<Value>
    Elements,
    /// List → Stack conversion: `list :> AsStack` → Stack
    AsStack,
    /// List → Queue conversion: `list :> AsQueue` → Queue
    AsQueue,
    /// Bit-range extraction: `word @/0..3` → extracts bits 0-3 via shift+mask
    /// Also used in TypeDef base types: `type MyInt <: Bits @/0..7`
    BitRange(BitRange),
    // ── Function metadata projections ────────────────────────────────────
    /// Function entry point address: `add :> Address` → Int (ptrtoint)
    Address,
    /// Declaration name: `add :> Name` → String
    Name,
    /// Comma-separated parameter types: `add :> Params` → String
    Params,
    /// Comma-separated return types: `add :> Returns` → String
    Returns,
    /// Number of parameters: `add :> Arity` → Int
    Arity,
    /// Source location `file:line:col`: `add :> Loc` → String
    Loc,
    /// Doc comment text (or empty): `add :> Doc` → String
    Doc,
    /// Stable content hash: `add :> Hash` → Int
    Hash,
    /// Serialized pre/post conditions: `add :> Contracts` → String
    Contracts,
    /// Module path (from import): `add :> Module` → String
    Module,
    /// True if defn/inop (without !): `add :> IsPure` → Bool
    IsPure,
    /// Start and end line numbers: `add :> FnSpan` → (Int, Int)
    FnSpan,
    /// User-defined projection from a type declaration: `value :> MyField`
    UserDefined(String),
    /// User-defined parameterized projection: `value :> At(0)`
    UserDefinedWithArg(String, Box<Expr>),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Intrinsic {
    Sqrt,
    Fabs,
    Ceil,
    Floor,
    Ctpop,
    Ctlz,
    Cttz,
    Abs,
    Bitreverse,
    ByteCount,
    Size,
    Pop,
    Contains,
    Keys,
    Values,
    // System I/O intrinsics (Pass A - 2026-06-11)
    Println,
    Print,
    Readln,
    Exit,
    Time,
    ReadFile,
    WriteFile,
    Sleep,
    // Data intrinsics (Pass A - 2026-06-11)
    Sort,
    Reverse,
    Range,
    // String intrinsics (2026-06-18) — C functions use __name__ convention
    TrimLeft,
    TrimRight,
    ToLower,
    ContainsAt,
    FindFrom,
    SplitN,
    IntToStr,
    /// strlen#(ptr: Ptr<Byte>) -> Int — returns length of a C string.
    /// Used by the CString lazy lens pattern: Size = _ :> Ptr :> strlen#;
    /// Zero-cost: strlen runs only when Size is explicitly queried.
    Strlen,

    // ===== Phase A: Terminal / TTY (intrinsics.md D4) =====
    TtyRawMode,
    TtySize,
    TtyReadKey,
    IoCtl,
    IsTty,

    // ===== Phase A: Process (intrinsics.md D5) =====
    SpawnWithOutput,
    Spawn,
    /// argv#() -> List<String> — command-line arguments (argc/argv)
    Argv,

    // ===== Phase B: Raw File I/O (intrinsics.md D2) =====
    Open,
    Close,
    Read,
    Write,
    LSeek,
    PRead,
    PWrite,
    Stat,
    FStat,
    /// ftruncate#(path: String, len: Int) -> Int
    /// Truncates a file on disk to the specified length.
    /// Returns 0 on success, -1 on error (errno set).
    /// Named ftruncate# to distinguish from string truncation (which is done via s[0..n] slicing).
    FTruncate,
    /// str_bytes#(s: String) -> List<Int>
    /// Converts a string's characters into a list of integer byte values.
    StrBytes,
    FSync,
    FDup,
    FDup2,
    FCntl,

    // ===== Phase C: Filesystem (intrinsics.md D3) =====
    MkDir,
    RmDir,
    Unlink,
    Rename,
    SymLink,
    ReadLink,
    Link,
    GetCwd,
    ChDir,
    ReadDir,
    ChMod,
    ChOwn,
    UMask,
    Access,

    // ===== Phase D: Memory (intrinsics.md D1) =====
    Mmap,
    MUnmap,
    MProtect,
    Brk,
    MLock,

    // ===== Phase D: Synchronization (intrinsics.md D9) =====
    AtomicLoad,
    AtomicStore,
    AtomicCas,
    AtomicXchg,
    AtomicAdd,
    Fence,
    Futex,

    // ===== Phase E: IPC (intrinsics.md D11) =====
    Pipe,
    ShmOpen,
    ShmUnlink,
    SemOpen,
    SemWait,
    SemPost,

    // ===== Phase F: Signals (intrinsics.md D8) =====
    SigAction,
    SigProcMask,
    Kill,
    SignalFd,
    TimerFdCreate,
    // ===== Phase G: Networking (intrinsics.md D10) =====
    Socket,
    Bind,
    Listen,
    Accept,
    Connect,
    Send,
    Recv,
    SendTo,
    RecvFrom,
    SetSockOpt,
    GetSockOpt,
    Shutdown,
    GetAddrInfo,
    // ===== Phase H: Everything Else (intrinsics.md D6, D7) =====
    GetEnv,
    SetEnv,
    UnsetEnv,
    GetPid,
    GetPPid,
    ClockGetTime,
    NanoSleep,
    // Benchmark intrinsics (2026-06-16) — direct libc, no brief_rt.c
    PrintInt,
    PutChar,
    PrintFloat,
    GetEnvInt,
    /// set_stdout_buf#(mode: Int) -> Bool — control stdout buffering
    SetStdoutBuf,

    // ===== Math intrinsics (2026-06-18) — trigonometric =====
    /// sin#(Float) -> Float — trigonometric sine
    Sin,
    /// cos#(Float) -> Float — trigonometric cosine
    Cos,
    /// pow#(Float, Float) -> Float — float exponentiation
    Pow,

    // ===== GPU compute intrinsics (2026-06-18) =====
    /// get_global_id#(dim: Int) -> Int — global work-item ID for the given dimension
    GetGlobalId,
    /// get_local_id#(dim: Int) -> Int — local work-item ID within workgroup
    GetLocalId,
    /// get_group_id#(dim: Int) -> Int — workgroup ID for the given dimension
    GetGroupId,
    /// get_num_groups#(dim: Int) -> Int — number of workgroups in the given dimension
    GetNumGroups,
    /// barrier#() -> Bool — workgroup-level synchronization barrier
    SubGroupBarrier,

    // ===== String conversion intrinsics (2026-06-18) =====
    /// float_to_str#(Float) -> String — format float to string
    FloatToStr,
    /// to_str#(Int|Float|Char|Bool) -> String — generic value to string
    ToStr,

    // ===== D12: Random / Entropy (2026-06-19) =====
    /// errno#() -> Int — last error code (thread-local)
    Errno,
    /// getrandom#(buf: Int, len: Int, flags: Int) -> Int — fill buffer with random bytes
    GetRandom,

    // ===== D13: System Info (2026-06-19) =====
    /// uname#() -> String — OS/kernel info (sysname:release:version:machine)
    Uname,
    /// pagesize#() -> Int — system memory page size
    PageSize,
    /// cpu_count#() -> Int — number of online CPUs
    CpuCount,
    /// hostname#() -> String — system hostname
    Hostname,
    /// strerror#(errnum: Int) -> String — human-readable error string
    StrError,
    /// strsignal#(signum: Int) -> String — human-readable signal name
    StrSignal,
    /// realpath#(path: String) -> String — canonical absolute path
    RealPath,

    // ===== D14: Debugging (2026-06-19) =====
    /// abort#() -> Void — abort with core dump
    Abort,
    /// backtrace#() -> List<Int> — stack trace addresses
    Backtrace,

    // ===== Memory-mapped I/O (2026-06-25) =====
    /// volatile_load#(ptr: Ptr<T>) -> T — volatile read from MMIO register.
    /// Return type T is the pointee type of Ptr<T>, determined at compile time.
    VolatileLoad,
    /// volatile_store#(ptr: Ptr<T>, val: T) -> Bool — volatile write to MMIO register.
    VolatileStore,

    // ===== D14b: CPU Halt (2026-06-19) =====
    /// halt#() -> Void — halt CPU (WFI on ARM, HLT on x86). Used by embedded mode term!
    Halt,

    // ===== D15: Scheduling (2026-06-19) =====
    /// sched_yield#() -> Int — yield CPU (0 on success)
    SchedYield,
    /// getpriority#(which: Int, who: Int) -> Int — get process priority
    GetPriority,
    /// setpriority#(which: Int, who: Int, prio: Int) -> Int — set process priority
    SetPriority,

    // ===== D16: User / Group (2026-06-19) =====
    /// getuid#() -> Int — real user ID
    GetUid,
    /// geteuid#() -> Int — effective user ID
    GetEUid,
    /// getgid#() -> Int — real group ID
    GetGid,
    /// getegid#() -> Int — effective group ID
    GetEGid,
    /// getpwuid#(uid: Int) -> String — user info (name:dir:shell)
    GetPwUid,
    /// getgrgid#(gid: Int) -> String — group name
    GetGrGid,

    // ===== D17: Threading (2026-06-19) =====
    /// thread_create#(fn_ptr: Int, arg: Int) -> Int — spawn thread
    ThreadCreate,
    /// thread_join#(thread: Int) -> Int — wait for thread
    ThreadJoin,
    /// thread_exit#(code: Int) -> Void — exit current thread
    ThreadExit,
    /// mutex_lock#(mptr: Int) -> Int — lock mutex
    MutexLock,
    /// mutex_unlock#(mptr: Int) -> Int — unlock mutex
    MutexUnlock,
    /// condvar_wait#(cptr: Int, mptr: Int) -> Int — wait on condition variable
    CondvarWait,
    /// condvar_signal#(cptr: Int) -> Int — signal one waiter
    CondvarSignal,
    /// condvar_broadcast#(cptr: Int) -> Int — signal all waiters
    CondvarBroadcast,

    // ===== D18: Resource Limits (2026-06-19) =====
    /// getrlimit#(resource: Int) -> Int — packed cur:max resource limit
    GetRlimit,
    /// setrlimit#(resource: Int, packed: Int) -> Int — set resource limit
    SetRlimit,

    // ===== Extra intrinsics (2026-06-19) =====
    /// mkstemp#(template: String) -> Int — create temp file (returns fd)
    MkStemp,
    /// mkdtemp#(template: String) -> String — create temp directory
    MkDtemp,
    /// dlopen#(filename: String) -> Int — open shared library (returns handle)
    DlOpen,
    /// dlsym#(handle: Int, symbol: String) -> Int — look up symbol
    DlSym,
    /// dlclose#(handle: Int) -> Int — close shared library
    DlClose,
    /// ttyname#(fd: Int) -> String — terminal device name
    TtyName,

    // ===== Macro/Template intrinsics (compile-time only) =====
    /// compile#(code: String) -> Block — parse string as Brief code at compile time
    Compile,
    /// error#(msg: String) — emit compiler error during macro expansion
    MacroError,
    /// warn#(msg: String) — emit compiler warning during macro expansion
    MacroWarn,
    /// gensym#() -> String — generate unique identifier during macro expansion
    MacroGenSym,
    /// emit_file#(filename: String, content: String) — write a file during macro expansion.
    /// Used by GLUE adapters to generate native language wrapper source files.
    /// The file is written to the compiler's output directory.
    EmitFile,
    // ===== Ring Buffer intrinsics (2026-07-01) =====
    /// ring_push#(handle: i64, val: i64) -> i64 — push value into ring buffer.
    /// Returns updated handle (handle is unchanged for ring buffers).
    /// Unboxes the handle, performs head/tail pointer arithmetic (~5 insns),
    /// stores new head/tail, returns handle.
    /// Used by RingBuffer<T> stdlib type: InsertAt = ring_push#.
    RingPush,
    /// ring_pop#(handle: i64) -> i64 — pop value from ring buffer.
    /// Returns the popped value, or 0 if empty.
    /// Does NOT modify %State — pure function of handle.
    /// Used by RingBuffer<T> stdlib type: ExtractFrom = ring_pop#.
    RingPop,

    /// User-defined intrinsic via `inop#` / `inop!#` declaration.
    /// The String stores the name for display/lookup; validation happens
    /// in the typechecker against the program's `inop_decls` map.
    UserDefined(String),
}

impl Intrinsic {
    pub fn has_side_effects(&self) -> bool {
        match self {
            // Pure/mathematical — can fold safely
            Intrinsic::Sqrt | Intrinsic::Fabs | Intrinsic::Ceil
            | Intrinsic::Floor
            | Intrinsic::Ctpop | Intrinsic::Ctlz | Intrinsic::Cttz
            | Intrinsic::Abs | Intrinsic::Bitreverse
            | Intrinsic::ByteCount | Intrinsic::Size
            | Intrinsic::TrimLeft | Intrinsic::TrimRight
            | Intrinsic::ToLower | Intrinsic::ContainsAt
            | Intrinsic::FindFrom | Intrinsic::SplitN
            | Intrinsic::StrBytes
            | Intrinsic::IntToStr | Intrinsic::Strlen
            | Intrinsic::Sin | Intrinsic::Cos | Intrinsic::Pow
            | Intrinsic::FloatToStr | Intrinsic::ToStr
            // GPU compute queries — pure (read-only state queries)
            | Intrinsic::GetGlobalId | Intrinsic::GetLocalId
            | Intrinsic::GetGroupId | Intrinsic::GetNumGroups
            // OS queries that are constant for process lifetime
            | Intrinsic::PageSize | Intrinsic::CpuCount
            | Intrinsic::Argv
            => false,
            // Everything else is observable — cannot fold
            _ => true,
        }
    }

    pub fn from_name(name: &str) -> Option<Intrinsic> {
        match name {
            "sqrt" => Some(Intrinsic::Sqrt),
            "fabs" => Some(Intrinsic::Fabs),
            "ceil" => Some(Intrinsic::Ceil),
            "floor" => Some(Intrinsic::Floor),
            "ctpop" => Some(Intrinsic::Ctpop),
            "ctlz" => Some(Intrinsic::Ctlz),
            "cttz" => Some(Intrinsic::Cttz),
            "abs" => Some(Intrinsic::Abs),
            "bitreverse" => Some(Intrinsic::Bitreverse),
            "byte_count" => Some(Intrinsic::ByteCount),
            "str_bytes" => Some(Intrinsic::StrBytes),
            "size" => Some(Intrinsic::Size),
            "pop" => Some(Intrinsic::Pop),
            "contains" => Some(Intrinsic::Contains),
            "keys" => Some(Intrinsic::Keys),
            "values" => Some(Intrinsic::Values),
            "println" => Some(Intrinsic::Println),
            "readln" => Some(Intrinsic::Readln),
            "exit" => Some(Intrinsic::Exit),
            "time" => Some(Intrinsic::Time),
            "read_file" => Some(Intrinsic::ReadFile),
            "write_file" => Some(Intrinsic::WriteFile),
            "sleep" => Some(Intrinsic::Sleep),
            "sort" => Some(Intrinsic::Sort),
            "reverse" => Some(Intrinsic::Reverse),
            "range" => Some(Intrinsic::Range),
            "print" => Some(Intrinsic::Print),
            "trim_left" => Some(Intrinsic::TrimLeft),
            "trim_right" => Some(Intrinsic::TrimRight),
            "to_lower" => Some(Intrinsic::ToLower),
            "contains_at" => Some(Intrinsic::ContainsAt),
            "find_from" => Some(Intrinsic::FindFrom),
            "splitn" => Some(Intrinsic::SplitN),
            "int_to_str" => Some(Intrinsic::IntToStr),
            "strlen" => Some(Intrinsic::Strlen),
            "sin" => Some(Intrinsic::Sin),
            "cos" => Some(Intrinsic::Cos),
            "pow" => Some(Intrinsic::Pow),
            // GPU compute intrinsics (2026-06-18)
            "get_global_id" => Some(Intrinsic::GetGlobalId),
            "get_local_id" => Some(Intrinsic::GetLocalId),
            "get_group_id" => Some(Intrinsic::GetGroupId),
            "get_num_groups" => Some(Intrinsic::GetNumGroups),
            "barrier" => Some(Intrinsic::SubGroupBarrier),
            "float_to_str" => Some(Intrinsic::FloatToStr),
            "to_str" => Some(Intrinsic::ToStr),
            // Phase A: Terminal
            "tty_raw_mode" => Some(Intrinsic::TtyRawMode),
            "tty_size" => Some(Intrinsic::TtySize),
            "tty_read_key" => Some(Intrinsic::TtyReadKey),
            "ioctl" => Some(Intrinsic::IoCtl),
            "isatty" => Some(Intrinsic::IsTty),
            // Phase A: Process
            "spawn_with_output" => Some(Intrinsic::SpawnWithOutput),
            "spawn" => Some(Intrinsic::Spawn),
            "argv" => Some(Intrinsic::Argv),
            // Phase B: Raw File I/O
            "open" => Some(Intrinsic::Open),
            "close" => Some(Intrinsic::Close),
            "read" => Some(Intrinsic::Read),
            "write" => Some(Intrinsic::Write),
            "lseek" => Some(Intrinsic::LSeek),
            "pread" => Some(Intrinsic::PRead),
            "pwrite" => Some(Intrinsic::PWrite),
            "stat" => Some(Intrinsic::Stat),
            "fstat" => Some(Intrinsic::FStat),
            "truncate" => Some(Intrinsic::FTruncate),
            "ftruncate" => Some(Intrinsic::FTruncate),
            "fsync" => Some(Intrinsic::FSync),
            "dup" => Some(Intrinsic::FDup),
            "dup2" => Some(Intrinsic::FDup2),
            "fcntl" => Some(Intrinsic::FCntl),
            // Phase C: Filesystem
            "mkdir" => Some(Intrinsic::MkDir),
            "rmdir" => Some(Intrinsic::RmDir),
            "unlink" => Some(Intrinsic::Unlink),
            "rename" => Some(Intrinsic::Rename),
            "symlink" => Some(Intrinsic::SymLink),
            "readlink" => Some(Intrinsic::ReadLink),
            "link" => Some(Intrinsic::Link),
            "getcwd" => Some(Intrinsic::GetCwd),
            "chdir" => Some(Intrinsic::ChDir),
            "readdir" => Some(Intrinsic::ReadDir),
            "chmod" => Some(Intrinsic::ChMod),
            "chown" => Some(Intrinsic::ChOwn),
            "umask" => Some(Intrinsic::UMask),
            "access" => Some(Intrinsic::Access),
            // Phase D: Memory
            "mmap" => Some(Intrinsic::Mmap),
            "munmap" => Some(Intrinsic::MUnmap),
            "mprotect" => Some(Intrinsic::MProtect),
            "brk" => Some(Intrinsic::Brk),
            "mlock" => Some(Intrinsic::MLock),
            // Phase D: Synchronization
            "atomic_load" => Some(Intrinsic::AtomicLoad),
            "atomic_store" => Some(Intrinsic::AtomicStore),
            "atomic_cas" => Some(Intrinsic::AtomicCas),
            "atomic_xchg" => Some(Intrinsic::AtomicXchg),
            "atomic_add" => Some(Intrinsic::AtomicAdd),
            "fence" => Some(Intrinsic::Fence),
            "futex" => Some(Intrinsic::Futex),
            // Phase E: IPC
            "pipe" => Some(Intrinsic::Pipe),
            "shm_open" => Some(Intrinsic::ShmOpen),
            "shm_unlink" => Some(Intrinsic::ShmUnlink),
            "sem_open" => Some(Intrinsic::SemOpen),
            "sem_wait" => Some(Intrinsic::SemWait),
            "sem_post" => Some(Intrinsic::SemPost),
            // Phase F: Signals
            "sigaction" => Some(Intrinsic::SigAction),
            "sigprocmask" => Some(Intrinsic::SigProcMask),
            "kill" => Some(Intrinsic::Kill),
            "signalfd" => Some(Intrinsic::SignalFd),
            "timerfd_create" => Some(Intrinsic::TimerFdCreate),
            // Phase G: Networking (intrinsics.md D10)
            "socket" => Some(Intrinsic::Socket),
            "bind" => Some(Intrinsic::Bind),
            "listen" => Some(Intrinsic::Listen),
            "accept" => Some(Intrinsic::Accept),
            "connect" => Some(Intrinsic::Connect),
            "send" => Some(Intrinsic::Send),
            "recv" => Some(Intrinsic::Recv),
            "sendto" => Some(Intrinsic::SendTo),
            "recvfrom" => Some(Intrinsic::RecvFrom),
            "setsockopt" => Some(Intrinsic::SetSockOpt),
            "getsockopt" => Some(Intrinsic::GetSockOpt),
            "shutdown" => Some(Intrinsic::Shutdown),
            "getaddrinfo" => Some(Intrinsic::GetAddrInfo),
            // Phase H: Everything Else (intrinsics.md D6, D7)
            "getenv" => Some(Intrinsic::GetEnv),
            "setenv" => Some(Intrinsic::SetEnv),
            "unsetenv" => Some(Intrinsic::UnsetEnv),
            "getpid" => Some(Intrinsic::GetPid),
            "getppid" => Some(Intrinsic::GetPPid),
            "clock_gettime" => Some(Intrinsic::ClockGetTime),
            "nanosleep" => Some(Intrinsic::NanoSleep),
            "print_int" => Some(Intrinsic::PrintInt),
            "putchar" => Some(Intrinsic::PutChar),
            "print_float" => Some(Intrinsic::PrintFloat),
            "getenv_int" => Some(Intrinsic::GetEnvInt),
            "set_stdout_buf" => Some(Intrinsic::SetStdoutBuf),
            // D12: Random / Entropy
            "errno" => Some(Intrinsic::Errno),
            "getrandom" => Some(Intrinsic::GetRandom),
            // D13: System Info
            "uname" => Some(Intrinsic::Uname),
            "pagesize" => Some(Intrinsic::PageSize),
            "cpu_count" => Some(Intrinsic::CpuCount),
            "hostname" => Some(Intrinsic::Hostname),
            "strerror" => Some(Intrinsic::StrError),
            "strsignal" => Some(Intrinsic::StrSignal),
            "realpath" => Some(Intrinsic::RealPath),
            // D14: Debugging
            "abort" => Some(Intrinsic::Abort),
            "backtrace" => Some(Intrinsic::Backtrace),
            "halt" => Some(Intrinsic::Halt),
            "volatile_load" => Some(Intrinsic::VolatileLoad),
            "volatile_store" => Some(Intrinsic::VolatileStore),
            // D15: Scheduling
            "sched_yield" => Some(Intrinsic::SchedYield),
            "getpriority" => Some(Intrinsic::GetPriority),
            "setpriority" => Some(Intrinsic::SetPriority),
            // D16: User / Group
            "getuid" => Some(Intrinsic::GetUid),
            "geteuid" => Some(Intrinsic::GetEUid),
            "getgid" => Some(Intrinsic::GetGid),
            "getegid" => Some(Intrinsic::GetEGid),
            "getpwuid" => Some(Intrinsic::GetPwUid),
            "getgrgid" => Some(Intrinsic::GetGrGid),
            // D17: Threading
            "thread_create" => Some(Intrinsic::ThreadCreate),
            "thread_join" => Some(Intrinsic::ThreadJoin),
            "thread_exit" => Some(Intrinsic::ThreadExit),
            "mutex_lock" => Some(Intrinsic::MutexLock),
            "mutex_unlock" => Some(Intrinsic::MutexUnlock),
            "condvar_wait" => Some(Intrinsic::CondvarWait),
            "condvar_signal" => Some(Intrinsic::CondvarSignal),
            "condvar_broadcast" => Some(Intrinsic::CondvarBroadcast),
            // D18: Resource Limits
            "getrlimit" => Some(Intrinsic::GetRlimit),
            "setrlimit" => Some(Intrinsic::SetRlimit),
            // Extra intrinsics
            "mkstemp" => Some(Intrinsic::MkStemp),
            "mkdtemp" => Some(Intrinsic::MkDtemp),
            "dlopen" => Some(Intrinsic::DlOpen),
            "dlsym" => Some(Intrinsic::DlSym),
            "dlclose" => Some(Intrinsic::DlClose),
            "ttyname" => Some(Intrinsic::TtyName),
            // Macro/template intrinsics (compile-time only)
            "compile" => Some(Intrinsic::Compile),
            "error" => Some(Intrinsic::MacroError),
            "warn" => Some(Intrinsic::MacroWarn),
            "gensym" => Some(Intrinsic::MacroGenSym),
            "emit_file" => Some(Intrinsic::EmitFile),
            // Ring buffer intrinsics
            "ring_push" => Some(Intrinsic::RingPush),
            "ring_pop" => Some(Intrinsic::RingPop),
            _ => None,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Intrinsic::Sqrt => "sqrt",
            Intrinsic::Fabs => "fabs",
            Intrinsic::Ceil => "ceil",
            Intrinsic::Floor => "floor",
            Intrinsic::Ctpop => "ctpop",
            Intrinsic::Ctlz => "ctlz",
            Intrinsic::Cttz => "cttz",
            Intrinsic::Abs => "abs",
            Intrinsic::Bitreverse => "bitreverse",
            Intrinsic::ByteCount => "byte_count",
            Intrinsic::StrBytes => "str_bytes",
            Intrinsic::Size => "size",
            Intrinsic::Pop => "pop",
            Intrinsic::Contains => "contains",
            Intrinsic::Keys => "keys",
            Intrinsic::Values => "values",
            Intrinsic::Println => "println",
            Intrinsic::Readln => "readln",
            Intrinsic::Exit => "exit",
            Intrinsic::Time => "time",
            Intrinsic::ReadFile => "read_file",
            Intrinsic::WriteFile => "write_file",
            Intrinsic::Sleep => "sleep",
            Intrinsic::Sort => "sort",
            Intrinsic::Reverse => "reverse",
            Intrinsic::Range => "range",
            Intrinsic::Print => "print",
            Intrinsic::TrimLeft => "trim_left",
            Intrinsic::TrimRight => "trim_right",
            Intrinsic::ToLower => "to_lower",
            Intrinsic::ContainsAt => "contains_at",
            Intrinsic::FindFrom => "find_from",
            Intrinsic::SplitN => "splitn",
            Intrinsic::IntToStr => "int_to_str",
            Intrinsic::Strlen => "strlen",
            Intrinsic::Sin => "sin",
            Intrinsic::Cos => "cos",
            Intrinsic::Pow => "pow",
            Intrinsic::GetGlobalId => "get_global_id",
            Intrinsic::GetLocalId => "get_local_id",
            Intrinsic::GetGroupId => "get_group_id",
            Intrinsic::GetNumGroups => "get_num_groups",
            Intrinsic::SubGroupBarrier => "barrier",
            Intrinsic::FloatToStr => "float_to_str",
            Intrinsic::ToStr => "to_str",
            // Phase A: Terminal
            Intrinsic::TtyRawMode => "tty_raw_mode",
            Intrinsic::TtySize => "tty_size",
            Intrinsic::TtyReadKey => "tty_read_key",
            Intrinsic::IoCtl => "ioctl",
            Intrinsic::IsTty => "isatty",
            // Phase A: Process
            Intrinsic::SpawnWithOutput => "spawn_with_output",
            Intrinsic::Spawn => "spawn",
            Intrinsic::Argv => "argv",
            // Phase B: Raw File I/O
            Intrinsic::Open => "open",
            Intrinsic::Close => "close",
            Intrinsic::Read => "read",
            Intrinsic::Write => "write",
            Intrinsic::LSeek => "lseek",
            Intrinsic::PRead => "pread",
            Intrinsic::PWrite => "pwrite",
            Intrinsic::Stat => "stat",
            Intrinsic::FStat => "fstat",
            Intrinsic::FTruncate => "ftruncate",
            Intrinsic::FSync => "fsync",
            Intrinsic::FDup => "dup",
            Intrinsic::FDup2 => "dup2",
            Intrinsic::FCntl => "fcntl",
            // Phase C: Filesystem
            Intrinsic::MkDir => "mkdir",
            Intrinsic::RmDir => "rmdir",
            Intrinsic::Unlink => "unlink",
            Intrinsic::Rename => "rename",
            Intrinsic::SymLink => "symlink",
            Intrinsic::ReadLink => "readlink",
            Intrinsic::Link => "link",
            Intrinsic::GetCwd => "getcwd",
            Intrinsic::ChDir => "chdir",
            Intrinsic::ReadDir => "readdir",
            Intrinsic::ChMod => "chmod",
            Intrinsic::ChOwn => "chown",
            Intrinsic::UMask => "umask",
            Intrinsic::Access => "access",
            // Phase D: Memory
            Intrinsic::Mmap => "mmap",
            Intrinsic::MUnmap => "munmap",
            Intrinsic::MProtect => "mprotect",
            Intrinsic::Brk => "brk",
            Intrinsic::MLock => "mlock",
            // Phase D: Synchronization
            Intrinsic::AtomicLoad => "atomic_load",
            Intrinsic::AtomicStore => "atomic_store",
            Intrinsic::AtomicCas => "atomic_cas",
            Intrinsic::AtomicXchg => "atomic_xchg",
            Intrinsic::AtomicAdd => "atomic_add",
            Intrinsic::Fence => "fence",
            Intrinsic::Futex => "futex",
            // Phase E: IPC
            Intrinsic::Pipe => "pipe",
            Intrinsic::ShmOpen => "shm_open",
            Intrinsic::ShmUnlink => "shm_unlink",
            Intrinsic::SemOpen => "sem_open",
            Intrinsic::SemWait => "sem_wait",
            Intrinsic::SemPost => "sem_post",
            // Phase F: Signals
            Intrinsic::SigAction => "sigaction",
            Intrinsic::SigProcMask => "sigprocmask",
            Intrinsic::Kill => "kill",
            Intrinsic::SignalFd => "signalfd",
            Intrinsic::TimerFdCreate => "timerfd_create",
            // Phase G: Networking (intrinsics.md D10)
            Intrinsic::Socket => "socket",
            Intrinsic::Bind => "bind",
            Intrinsic::Listen => "listen",
            Intrinsic::Accept => "accept",
            Intrinsic::Connect => "connect",
            Intrinsic::Send => "send",
            Intrinsic::Recv => "recv",
            Intrinsic::SendTo => "sendto",
            Intrinsic::RecvFrom => "recvfrom",
            Intrinsic::SetSockOpt => "setsockopt",
            Intrinsic::GetSockOpt => "getsockopt",
            Intrinsic::Shutdown => "shutdown",
            Intrinsic::GetAddrInfo => "getaddrinfo",
            // Phase H: Everything Else (intrinsics.md D6, D7)
            Intrinsic::GetEnv => "getenv",
            Intrinsic::SetEnv => "setenv",
            Intrinsic::UnsetEnv => "unsetenv",
            Intrinsic::GetPid => "getpid",
            Intrinsic::GetPPid => "getppid",
            Intrinsic::ClockGetTime => "clock_gettime",
            Intrinsic::NanoSleep => "nanosleep",
            Intrinsic::PrintInt => "print_int",
            Intrinsic::PutChar => "putchar",
            Intrinsic::PrintFloat => "print_float",
            Intrinsic::GetEnvInt => "getenv_int",
            Intrinsic::SetStdoutBuf => "set_stdout_buf",
            // D12: Random / Entropy
            Intrinsic::Errno => "errno",
            Intrinsic::GetRandom => "getrandom",
            // D13: System Info
            Intrinsic::Uname => "uname",
            Intrinsic::PageSize => "pagesize",
            Intrinsic::CpuCount => "cpu_count",
            Intrinsic::Hostname => "hostname",
            Intrinsic::StrError => "strerror",
            Intrinsic::StrSignal => "strsignal",
            Intrinsic::RealPath => "realpath",
            // D14: Debugging
            Intrinsic::Abort => "abort",
            Intrinsic::Backtrace => "backtrace",
            Intrinsic::Halt => "halt",
            Intrinsic::VolatileLoad => "volatile_load",
            Intrinsic::VolatileStore => "volatile_store",
            // D15: Scheduling
            Intrinsic::SchedYield => "sched_yield",
            Intrinsic::GetPriority => "getpriority",
            Intrinsic::SetPriority => "setpriority",
            // D16: User / Group
            Intrinsic::GetUid => "getuid",
            Intrinsic::GetEUid => "geteuid",
            Intrinsic::GetGid => "getgid",
            Intrinsic::GetEGid => "getegid",
            Intrinsic::GetPwUid => "getpwuid",
            Intrinsic::GetGrGid => "getgrgid",
            // D17: Threading
            Intrinsic::ThreadCreate => "thread_create",
            Intrinsic::ThreadJoin => "thread_join",
            Intrinsic::ThreadExit => "thread_exit",
            Intrinsic::MutexLock => "mutex_lock",
            Intrinsic::MutexUnlock => "mutex_unlock",
            Intrinsic::CondvarWait => "condvar_wait",
            Intrinsic::CondvarSignal => "condvar_signal",
            Intrinsic::CondvarBroadcast => "condvar_broadcast",
            // D18: Resource Limits
            Intrinsic::GetRlimit => "getrlimit",
            Intrinsic::SetRlimit => "setrlimit",
            // Extra intrinsics
            Intrinsic::MkStemp => "mkstemp",
            Intrinsic::MkDtemp => "mkdtemp",
            Intrinsic::DlOpen => "dlopen",
            Intrinsic::DlSym => "dlsym",
            Intrinsic::DlClose => "dlclose",
            Intrinsic::TtyName => "ttyname",
            // Macro/template intrinsics (compile-time only)
            Intrinsic::Compile => "compile",
            Intrinsic::MacroError => "error",
            Intrinsic::MacroWarn => "warn",
            Intrinsic::MacroGenSym => "gensym",
            Intrinsic::EmitFile => "emit_file",
            // Ring buffer intrinsics
            Intrinsic::RingPush => "ring_push",
            Intrinsic::RingPop => "ring_pop",
            Intrinsic::UserDefined(_) => "__user__",
        }
    }

    /// Return the user-defined name for `UserDefined` intrinsics, or `None`.
    /// Used by display/formatting code that needs the actual name string.
    pub fn user_defined_name(&self) -> Option<&str> {
        match self {
            Intrinsic::UserDefined(n) => Some(n.as_str()),
            _ => None,
        }
    }

    /// Returns true for intrinsics that are only valid during macro/template expansion.
    /// These should never appear in runtime code — the compiler errors if they survive
    /// past Phase 1b.
    pub fn is_compile_time_only(&self) -> bool {
        matches!(self,
            Intrinsic::Compile
            | Intrinsic::MacroError
            | Intrinsic::MacroWarn
            | Intrinsic::MacroGenSym
            | Intrinsic::EmitFile
        )
    }
}

/// User-defined intrinsic operation (`inop#` / `inop!#`).
/// Contains the declaration signature, LLVM IR body, optional Brief fallback,
/// and the side-effect flag derived from the `inop!` vs `inop` keyword.
#[derive(Debug, Clone)]
pub struct InopDeclaration {
    pub name: String,
    pub type_params: Vec<String>,
    pub params: Vec<(String, Type)>,
    pub outputs: Vec<Type>,
    pub contract: Contract,
    pub llvm_body: Vec<String>,
    /// Source spans for each line in llvm_body, for error reporting.
    pub llvm_body_spans: Vec<Span>,
    pub fallback: Option<Expr>,
    pub has_side_effects: bool,
    pub has_state_access: bool,
    /// LLVM section attribute (e.g. ".init_array") — emitted as `section "..."` on `define`.
    pub section: Option<String>,
    pub span: Option<Span>,
}

/// A single route in a `meld A <:> B` declaration.
/// Maps a field/accessor from type A to the corresponding expression in type B.
/// Example: `@0..63 -> B.ptr` maps bits 0-63 of A to B's `ptr` field.
#[derive(Debug, Clone)]
pub struct MeldRouteDef {
    /// The accessor on the source type (e.g. `@0..63` for a bit range, or `Len` for a field name)
    pub accessor: String,
    /// The destination expression to compute the corresponding value
    pub dest_expr: Expr,
}

/// A `meld A <:> B;` declaration with optional explicit route definitions.
/// If routes are empty, the compiler infers them from `@/` bit-range matching.
#[derive(Debug, Clone)]
pub struct MeldDeclaration {
    pub name_a: String,
    pub name_b: String,
    /// Optional explicit routes. Empty means "infer all routes."
    pub routes: Vec<MeldRouteDef>,
    pub span: Option<Span>,
}

/// Target for the `is` check expression: either a Type or a Variant name.
/// Variant names (Some, None, Ok, Err) are resolved against the LHS enum type.
#[derive(Debug, Clone, PartialEq)]
pub enum IsTarget {
    Type(Type),
    Variant(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    Integer(i64),
    IntegerSuffixed(i64, Type),
    Float(f64),
    Float64(f64),
    String(String),
    RegexLiteral(String), // @"..." — regex pattern literal
    Char(char),  // NEW: Char literal
    Bool(bool),
    Term,
    /// Pattern B feature struct: wraps Integer, Float, String, Char, Bool, Term
    Literal(Box<LiteralExpr>),
    Identifier(String),
    OwnedRef(String),
    PriorState(String),
    /// `...` — ellipsis, expands to fill unspecified dimensions in bracket context
    Ellipsis,
    // Pattern B — packed ellipsis
    EllipsisExpr(EllipsisExpr),
    /// Reference to a named type: `Bits`, `Int`, `U32`, etc. Used as the base
    /// expression in a `Type Name <: Base { ... }` declaration.
    TypeRef(String),
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
    /// Two-sided transfer: `&dest <- &source` or `&dest <- &source[; filter]`
    /// All matching elements move from source to dest.
    /// `filter` is None for unconditional transfer.
    ArrowTransfer {
        dest: Box<Expr>,
        source: Box<Expr>,
        filter: Option<Box<Expr>>,
    },
    // Pattern B — packed arrow variants
    ArrowMutExpr(ArrowMutExpr),
    ArrowDiscardExpr(ArrowDiscardExpr),
    ArrowTransferExpr(ArrowTransferExpr),
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
    // GPU shared memory: __shared(N) → pointer to addrspace(3) memory
    SharedMem(usize),
    /// Runtime/compile-time type check: `x is Int` or `x is Some`
    IsType(Box<Expr>, IsTarget),
    /// Derivation check: `x from Foo` (type or value against ancestor type)
    FromCheck(Box<Expr>, Type),
    /// Structural equivalence check: `x like y` (field-by-field comparison)
    Like(Box<Expr>, Box<Expr>),
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
    // Pattern B — packed binary/unary operations
    BinaryOp(Box<BinaryOpExpr>),
    UnaryOp(Box<UnaryOpExpr>),
    Concat(Box<Expr>, Box<Expr>),
    /// Type cast: expr as Type
    Cast(Box<Expr>, Type),
    /// Compile-time metadata projection: `expr :> Size`
    Projection {
        source: Box<Expr>,
        target: ProjectionTarget,
    },
    // Pattern B — packed projection
    ProjectionExpr(ProjectionExpr),
    Call(String, Vec<Expr>),
    // Pattern B — packed call
    CallExpr(CallExpr),
    /// `cell name(args)` — synchronous call to a cell.
    CellCall(Box<Expr>, Vec<Expr>),

    /// $name(args) or $name(args) { block } — template call
    TemplateCall {
        name: String,
        args: Vec<Expr>,
        block: Option<Block>,
        span: Option<Span>,
    },
    /// $!name(args) or $!name(args) { block } — macro call
    MacroCall {
        name: String,
        args: Vec<Expr>,
        block: Option<Block>,
        span: Option<Span>,
    },
    /// Compiler-known intrinsic call: `name#(args)` — e.g. `sqrt#(x)`, `pop#(list)`
    IntrinsicCall {
        intrinsic: Intrinsic,
        args: Vec<Expr>,
    },
    ListLiteral(Vec<Expr>),
    // Pattern B — packed list literal
    ListLiteralExpr(ListLiteralExpr),
    /// HashMap literal: `{"a": 1, "b": 2}`
    MapLiteral(Vec<(Expr, Expr)>),
    // Pattern B — packed map literal
    MapLiteralExpr(MapLiteralExpr),
    /// HashSet literal: `{1, 2, 3}`
    SetLiteral(Vec<Expr>),
    // Pattern B — packed set literal
    SetLiteralExpr(SetLiteralExpr),
    ListIndex(Box<Expr>, Box<Expr>),
    Slice {
        value: Box<Expr>,
        start: Option<Box<Expr>>,
        end: Option<Box<Expr>>,
        stride: Option<Box<Expr>>,
        mask: Option<Box<Expr>>,
    },
    // Pattern B — packed slice
    SliceExpr(SliceExpr),
    // Multidimensional slice: vec[coord1, coord2, ...; mask :: stride]
    MultiSlice {
        value: Box<Expr>,
        ops: Vec<BracketOp>,
    },
    // Pattern B — packed multi-slice
    MultiSliceExpr(MultiSliceExpr),

    FieldAccess(Box<Expr>, String),
    // Pattern B — packed field access
    FieldAccessExpr(FieldAccessExpr),
    StructInstance(String, Vec<(String, Expr)>),
    // Pattern B — packed struct instance
    StructInstanceExpr(StructInstanceExpr),
    ObjectLiteral(Vec<(String, Expr)>),
    // Pattern B — packed object literal
    ObjectLiteralExpr(ObjectLiteralExpr),
// Pattern matching in guards: [value Variant(field1, field2)] { ... }
    PatternMatch {
        value: Box<Expr>,
        variant: String,
        fields: Vec<Pattern>,
    },
    // Pattern B — packed pattern match
    PatternMatchExpr(PatternMatchExpr),
    // Match expression: match value { Variant(f1) => body, _ => default }
    Match {
        value: Box<Expr>,
        arms: Vec<MatchArm>,
    },
    // Pattern B — packed match
    MatchExpr(MatchExpr),
    // Block expression: { stmts...; last_expr }
    Block(Vec<Statement>, Box<Expr>),
    // Pattern B — packed block
    BlockExpr(BlockExpr),

    /// @ident — interpolation marker inside quote { } (from input arg)
    Interpolate(String),
    /// @{expr} — computed interpolation inside quote { } or compile#()
    InterpolateExpr(Box<Expr>),
    /// quote { stmts...; last_expr } — AST quasiquoting block
    QuoteBlock {
        statements: Vec<Statement>,
        trailing_expr: Option<Box<Expr>>,
    },
    // Tuple destructuring: let (a, b) = expr;
    TupleDestructure(Vec<String>, Box<Expr>),
    // Pattern B — packed tuple destructure
    TupleDestructureExpr(TupleDestructureExpr),
    // Tuple literal: (a, b, c)
    Tuple(Vec<Expr>),
    // Pattern B — packed tuple literal
    TupleExpr(TupleExpr),
    /// Sig call modifier: `sig #out expr` or `sig #inline expr`
    SigCall {
        modifier: SigModifier,
        expr: Box<Expr>,
    },
    // Pattern B — packed sig call
    SigCallExpr(SigCallExpr),
    /// `<:` subtype projection: `let result <: items { FILTER(.active); COUNT; };`
    SubtypeProjection {
        source: Box<Expr>,
        ops: Vec<SubtypeOp>,
    },
    // Pattern B — packed subtype projection
    SubtypeProjectionExpr(SubtypeProjectionExpr),
    /// Lazy-loaded DBVL table for large-file imports.
    /// Evaluates to Value::DbvlTable — users see it as a Map.
    DbvlTable {
        path: String,
        field_names: Vec<String>,
        key_offsets: HashMap<String, Vec<usize>>,
        schema_name: Option<String>,
    },
    // Pattern B — packed dbvl table
    DbvlTableExpr(DbvlTableExpr),
    /// Pipe chain: `initial |> step1 |> step2 .|> step3`
    /// Desugared to block with let-bound temporaries before typechecking.
    PipeChain(PipeChain),
    /// Temporal fallback: `foo() within N cycles (retries) ~? fallback()`
    Within {
        body: Box<Expr>,
        bound: u64,
        unit: TimeUnit,
        retries: u64,
        fallback: Box<Expr>,
    },
}

/// A full pipe chain: initial value followed by chained transformation steps.
#[derive(Debug, Clone, PartialEq)]
pub struct PipeChain {
    pub initial: Box<Expr>,
    pub steps: Vec<PipeStep>,
}

impl Expr {
    /// Normalize BinaryOp/UnaryOp to old-style variants for backward compatibility.
    /// Recursively normalizes nested expressions.
    /// Returns None if already in old-style form (no conversion needed).
    pub fn normalize_to_old(&self) -> Option<Expr> {
        match self {
            Expr::BinaryOp(bop) => {
                let l = Box::new(bop.left.normalize_to_old_recursive());
                let r = Box::new(bop.right.normalize_to_old_recursive());
                Some(match bop.kind {
                    crate::features::binary_op::BinaryOpKind::Add => Expr::Add(l, r),
                    crate::features::binary_op::BinaryOpKind::Sub => Expr::Sub(l, r),
                    crate::features::binary_op::BinaryOpKind::Mul => Expr::Mul(l, r),
                    crate::features::binary_op::BinaryOpKind::Div => Expr::Div(l, r),
                    crate::features::binary_op::BinaryOpKind::Mod => Expr::Mod(l, r),
                    crate::features::binary_op::BinaryOpKind::Eq => Expr::Eq(l, r),
                    crate::features::binary_op::BinaryOpKind::Ne => Expr::Ne(l, r),
                    crate::features::binary_op::BinaryOpKind::Lt => Expr::Lt(l, r),
                    crate::features::binary_op::BinaryOpKind::Le => Expr::Le(l, r),
                    crate::features::binary_op::BinaryOpKind::Gt => Expr::Gt(l, r),
                    crate::features::binary_op::BinaryOpKind::Ge => Expr::Ge(l, r),
                    crate::features::binary_op::BinaryOpKind::And => Expr::And(l, r),
                    crate::features::binary_op::BinaryOpKind::Or => Expr::Or(l, r),
                    crate::features::binary_op::BinaryOpKind::BitAnd => Expr::BitAnd(l, r),
                    crate::features::binary_op::BinaryOpKind::BitOr => Expr::BitOr(l, r),
                    crate::features::binary_op::BinaryOpKind::BitXor => Expr::BitXor(l, r),
                    crate::features::binary_op::BinaryOpKind::Shl => Expr::Shl(l, r),
                    crate::features::binary_op::BinaryOpKind::Shr => Expr::Shr(l, r),
                })
            }
            Expr::UnaryOp(uop) => {
                let op = Box::new(uop.operand.normalize_to_old_recursive());
                Some(match uop.kind {
                    crate::features::unary_op::UnaryOpKind::Neg => Expr::Neg(op),
                    crate::features::unary_op::UnaryOpKind::Not => Expr::Not(op),
                    crate::features::unary_op::UnaryOpKind::BitNot => Expr::BitNot(op),
                })
            }
            _ => None,
        }
    }

    /// Recursively normalize, always returning an owned Expr in old-style form.
    pub(crate) fn normalize_to_old_recursive(&self) -> Expr {
        match self.normalize_to_old() {
            Some(normalized) => normalized,
            None => self.clone(),
        }
    }

    /// Set the file path on a DbvlTable expression (used by import resolver).
    pub fn set_dbvl_path(&mut self, file_path: &str) {
        if let Self::DbvlTable { path, .. } = self {
            path.clone_from(&file_path.to_string());
        }
    }

    /// Extract integer value, handling both old variant and new Literal wrapper.
    pub fn as_integer(&self) -> Option<i64> {
        match self {
            Expr::Integer(n) => Some(*n),
            Expr::Literal(lit) => match lit.as_ref() {
                LiteralExpr::Integer(n) => Some(*n),
                _ => None,
            },
            _ => None,
        }
    }

    /// Extract float value, handling both old variant and new Literal wrapper.
    pub fn as_float(&self) -> Option<f64> {
        match self {
            Expr::Float(f) => Some(*f),
            Expr::Literal(lit) => match lit.as_ref() {
                LiteralExpr::Float(f) => Some(*f),
                _ => None,
            },
            _ => None,
        }
    }

    /// Extract string value, handling both old variant and new Literal wrapper.
    pub fn as_string(&self) -> Option<&str> {
        match self {
            Expr::String(s) => Some(s.as_str()),
            Expr::Literal(lit) => match lit.as_ref() {
                LiteralExpr::String(s) => Some(s.as_str()),
                _ => None,
            },
            _ => None,
        }
    }

    /// Extract bool value, handling both old variant and new Literal wrapper.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Expr::Bool(b) => Some(*b),
            Expr::Literal(lit) => match lit.as_ref() {
                LiteralExpr::Bool(b) => Some(*b),
                _ => None,
            },
            _ => None,
        }
    }

    /// Check if expression is Term, handling both old variant and new Literal wrapper.
    pub fn is_term(&self) -> bool {
        matches!(self, Expr::Term) || matches!(self, Expr::Literal(lit) if matches!(lit.as_ref(), LiteralExpr::Term))
    }
}

/// A single step in a pipe chain (`|>`, `.N|>`).
/// Captures the target expression and how far back in the pipeline stack to read.
#[derive(Debug, Clone, PartialEq)]
pub struct PipeStep {
    pub target: Box<Expr>,
    pub skip: usize,
}

/// A pattern in a match arm: `Variant(f1, f2)` or `_`
#[derive(Debug, Clone, PartialEq)]
pub enum MatchPattern {
    Wildcard,
    Literal(Pattern),
    Variant { name: String, fields: Vec<Pattern> },
}

/// A single arm in a match expression
#[derive(Debug, Clone, PartialEq)]
pub struct MatchArm {
    pub pattern: MatchPattern,
    pub guard: Option<Box<Expr>>,
    pub body: Box<Expr>,
}

/// A single operation inside a `<:` subtype projection block
#[derive(Debug, Clone, PartialEq)]
pub enum SubtypeOp {
    /// FILTER(.predicate) — keep elements matching predicate
    Filter(Box<Expr>),
    /// MAP(.expr) — transform each element
    Map(Box<Expr>),
    /// SORT(.key) — sort by key expression
    Sort(Box<Expr>),
    /// LIMIT(N) — take first N elements
    Limit(usize),
    /// SKIP(N) — skip first N elements
    Skip(usize),
    /// UNIQUE — remove adjacent duplicates
    Unique,
    /// JOIN(collection_expr, .key) — merge with another collection
    Join(Box<Expr>, Box<Expr>),
    /// GROUP(.key) — group by key expression
    Group(Box<Expr>),
    /// COUNT — count elements (terminal aggregate)
    Count,
    /// SUM(.expr) — sum of expression (terminal aggregate)
    Sum(Box<Expr>),
    /// AVG(.expr) — average of expression (terminal aggregate)
    Avg(Box<Expr>),
    /// MIN(.expr) — minimum of expression (terminal aggregate)
    Min(Box<Expr>),
    /// MAX(.expr) — maximum of expression (terminal aggregate)
    Max(Box<Expr>),
    /// MATCH(pattern) — regex pattern for string projection
    Match(Box<Expr>),
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
            Expr::Call(_, args) | Expr::ListLiteral(args) | Expr::CellCall(_, args) => {
                for arg in args {
                    arg.extract_deps_recursive(deps);
                }
            }
            Expr::IntrinsicCall { intrinsic: _, args } => {
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
            Expr::ArrowTransfer { dest, source, filter } => {
                dest.extract_deps_recursive(deps);
                source.extract_deps_recursive(deps);
                if let Some(f) = filter {
                    f.extract_deps_recursive(deps);
                }
            }
            Expr::MapLiteral(entries) => {
                for (k, v) in entries {
                    k.extract_deps_recursive(deps);
                    v.extract_deps_recursive(deps);
                }
            }
            Expr::SetLiteral(entries) => {
                for e in entries {
                    e.extract_deps_recursive(deps);
                }
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
        constraint: Option<Box<Expr>>,
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

    /// `trg name @ instance.port;` — bind a trigger to a component's output port.
    /// The trigger fires when the component writes to that output variable.
    TrgBinding {
        name: String,
        ty: Option<Type>,         // optional explicit type annotation
        instance: Expr,           // expression yielding a component handle
        port: String,             // named output port on the component
        modifiers: Vec<Hashtag>,
    },

    // PERMANENTLY ABANDONED — alka { ... }; or alka! { ... };
    // Code left as historical artifact. No revisit planned.
    Alka(AlkaBlock),

    // PERMANENTLY ABANDONED — #on_exit { ... };
    // Code left as historical artifact. No revisit planned.
    OnExit {
        body: Vec<Statement>,
        span: Option<Span>,
    },

    // Sync block: sync { stmt1; stmt2; ... };
    // Fork-join barrier — all statements start and finish simultaneously.
    SyncBlock {
        body: Vec<Statement>,
    },

    // For-each loop: foreach(item in list) { body };
    // Valid only inside defn/txn/rct txn bodies. Binds item: T for each
    // element of list: List<T>. Termination is structural (list is finite),
    // no convergence contract needed.
    Foreach {
        item: String,
        list: Box<Expr>,
        body: Vec<Statement>,
        modifiers: Vec<Hashtag>,
    },

    // Proof oracle: ?#[handler] { body };
    // Throws the proof engine's full strategy palette at proving the body
    // terminates. If no static strategy succeeds, injects a runtime fuel
    // counter with rollback. The handler block executes on fuel exhaustion.
    Oracle {
        handler: Vec<Statement>,
        body: Vec<Statement>,
        span: Option<Span>,
    },

    // Await: await call_expr; — blocking wait for a callable result
    Await {
        expr: Expr,
        modifiers: Vec<Hashtag>,
    },

    // Async: async stmt; or async { body }; — fire-and-forget
    Async {
        body: Box<Statement>,
        modifiers: Vec<Hashtag>,
    },

    // AsyncAwait: async await expr; or async await let x = expr;
    // Fork-join: launches immediately, barriers at term.
    // lhs: Some(name) if "async await let x = expr;" form
    AsyncAwait {
        body: Box<Statement>,
        lhs: Option<String>,
        modifiers: Vec<Hashtag>,
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
    pub cycles_bound: Option<u64>,
    pub seconds_bound: Option<u64>,
    pub is_proven: bool,
    pub retries: u64,
    pub fallback: Option<Box<Expr>>,
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
#[derive(Debug, Clone, PartialEq)]
pub enum SigModifier {
    /// `sig #out` — function has observable external effects
    Out,
    /// `sig #inline` — function is pure, safe to fold/eliminate
    Inline,
    /// `sig #export("name")` — emit globally-visible symbol with C ABI.
    /// The optional string specifies the exported symbol name
    /// (defaults to the Brief identifier name).
    Export(Option<String>),
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
    pub annotations: Vec<TypeBinding>,
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
    pub annotations: Vec<TypeBinding>,
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
    pub constraint: Option<Box<Expr>>,
    pub is_override: bool,
    pub os_mode: bool, // In OS mode, address is requested via ioctl/mmap; else embedded mode uses raw address
    pub span: Option<Span>,
    pub attrs: Vec<Attribute>,  // NEW: #[...] attributes
}

#[derive(Debug, Clone)]
pub enum LinkRef {
    Explicit(u64),
    Linked(String),
    Stdin,
    Timer(u64),
    Signal(String),
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
    pub modifiers: Vec<Hashtag>,
}

#[derive(Debug, Clone)]
pub struct Constant {
    pub name: String,
    pub ty: Type,
    pub expr: Expr,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ImportTarget {
    /// Default — import as inline TS (webstack) or LLVM (native)
    Native,
    /// Compile to WASM sidecar module (wasm32 target)
    Wasm,
    /// Compile to CIRCT / hardware via esp-circt
    Circt,
    /// Inline JavaScript/TypeScript (only valid in webstack)
    Javascript,
}

impl Default for ImportTarget {
    fn default() -> Self {
        ImportTarget::Native
    }
}

#[derive(Debug, Clone)]
pub struct Import {
    pub items: Vec<ImportItem>,
    pub path: Vec<String>,
    pub is_magic: bool,
    pub target: ImportTarget,
}

#[derive(Debug, Clone)]
pub struct ImportItem {
    pub name: String,
    pub alias: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LinkLanguage {
    C,              // .c
    Cpp,            // .cpp / .cc / .cxx
    Rust,           // .rs
    Zig,            // .zig
    Python,         // .py
    Java,           // .java → native-image --llvm --emit-llvm-bc
    AssemblyScript, // .ts   → asc → .wasm → wasm2llvm → .bc
    Bitcode,        // .bc
    Object,         // .o / .a
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
    /// User-defined intrinsic operation via `inop#` / `inop!#` declaration.
    Inop(InopDeclaration),
    ResourceDecl(ResourceDeclaration), // NEW: rsrc/resource
    /// `trg name @ instance.port;` — bind a trigger to a component's output port at top level.
    TriggerBinding {
        name: String,
        ty: Option<Type>,
        instance: Expr,
        port: String,
        modifiers: Vec<Hashtag>,
    },
    /// `cell` / `cell!` — cybernetic cell with isolated state space.
    Cell(Box<CellDef>),
    Struct(StructDefinition),
    RStruct(RStructDefinition),
    Enum(EnumDefinition),
    /// `Type Name <: Base { ... }` — type derivation system (Phase 1.5)
    TypeDef(Box<TypeDef>),
    /// `meld A <:> B;` — bidirectional type compatibility declaration.
    /// Establishes that A and B are mutually lens-compatible.
    /// The value's physical layout adapts based on usage across both lenses.
    Meld(MeldDeclaration),
    /// `#test("group")` pragma — wraps an item with test group membership.
    /// Skipped in production; included in test mode.
    Test {
        item: Box<TopLevel>,
        groups: Vec<String>,
    },
    /// `#fuzz(bindings...) -> expected` pragma — wraps an item with inline test cases.
    /// Verified at compile time via interpreter (defn/txn) or BILD simulator (inop).
    Fuzzed {
        item: Box<TopLevel>,
        cases: Vec<FuzzCase>,
    },
    /// `#!assert` directive — compile-time assertion chain.
    Assertion {
        pre: Expr,
        chain: Vec<String>,
    },
    /// `#!cfg(condition) { items }` — conditional compilation guard.
    Cfg(CfgGuard),
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
    /// Top-level executable statement — desugared to `__init` transaction at Pass 2.
    Statement(Box<Statement>),

    /// template name(params) { body } — hygienic template definition
    TemplateDef {
        name: String,
        params: Vec<(String, MacroArgType)>,
        return_type: Option<MacroArgType>,
        body: Vec<Statement>,
    },
    /// macro name(params) { body } — procedural macro definition
    MacroDef {
        name: String,
        params: Vec<(String, MacroArgType)>,
        return_type: Option<MacroArgType>,
        body: Vec<Statement>,
    },
}

/// A condition expression evaluated at parse-time for `#!cfg(...)` guards.
/// Determines whether a block of top-level items is compiled or skipped.
#[derive(Debug, Clone, PartialEq)]
pub enum CfgCondition {
    /// Equality: `key == "value"`
    Eq(String, String),
    /// Inequality: `key != "value"`
    Ne(String, String),
    /// Logical AND
    And(Box<CfgCondition>, Box<CfgCondition>),
    /// Logical OR
    Or(Box<CfgCondition>, Box<CfgCondition>),
    /// Logical NOT
    Not(Box<CfgCondition>),
    /// Literal true/false
    Bool(bool),
}

impl CfgCondition {
    /// Evaluate the condition against the given target configuration.
    /// Returns `Ok(true)` if the condition matches, `Ok(false)` if not,
    /// or `Err(unknown_key)` if a cfg key doesn't match any known key.
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
                let a = a.evaluate(target_os, target_arch, board)?;
                if !a { return Ok(false); }
                b.evaluate(target_os, target_arch, board)
            }
            CfgCondition::Or(a, b) => {
                let a = a.evaluate(target_os, target_arch, board)?;
                if a { return Ok(true); }
                b.evaluate(target_os, target_arch, board)
            }
            CfgCondition::Not(c) => {
                let c = c.evaluate(target_os, target_arch, board)?;
                Ok(!c)
            }
            CfgCondition::Bool(b) => Ok(*b),
        }
    }
}

/// A `#!cfg(...)` guard that conditionally includes top-level items.
#[derive(Debug, Clone)]
pub struct CfgGuard {
    pub condition: CfgCondition,
    pub items: Vec<TopLevel>,
}

/// A block of statements with an optional trailing expression.
/// Used for trailing block arguments in template/macro calls.
#[derive(Debug, Clone, PartialEq)]
pub struct Block {
    pub statements: Vec<Statement>,
    pub trailing_expr: Option<Box<Expr>>,
}

/// Argument types for template/macro parameters.
#[derive(Debug, Clone, PartialEq)]
pub enum MacroArgType {
    Expr,
    Stmt,
    Block,
    Type,
    Int,
    String,
    Bool,
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
    pub parent: Option<Type>,
    pub fields: Vec<StructField>,
    pub transactions: Vec<Transaction>,
    pub view_html: Option<String>,
    pub span: Option<Span>,
    pub modifiers: Vec<Hashtag>,
    pub variants: Vec<StructVariant>,
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

/// `cell` / `cell!` — Cybernetic Cell Definition.
/// An isolated Brief-in-Brief state space with private state, reactive
/// transactions, internal triggers, and a well-defined output interface.
#[derive(Debug, Clone)]
pub struct CellDef {
    /// false = cell (auto-terminating), true = cell! (persistent)
    pub is_persistent: bool,
    pub name: String,
    pub type_params: Vec<TypeParam>,
    /// Input arguments (ephemeral per invocation)
    pub parameters: Vec<(String, Type)>,
    /// Output ports: -> name: Type [, | name: Type]
    pub output_type: Option<OutputType>,
    /// Private state fields (declared in body)
    pub fields: Vec<StructField>,
    /// Reactive/non-reactive transactions inside the cell body
    pub transactions: Vec<Transaction>,
    /// Helper definitions inside the cell body
    pub definitions: Vec<Definition>,
    /// Internal triggers (scoped to the cell, invisible outside)
    pub internal_triggers: Vec<TriggerDeclaration>,
    pub span: Option<Span>,
    pub modifiers: Vec<Hashtag>,
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
            parent: None,
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

/// A single fuzz test case: bind parameters/state to values, expect an output.
#[derive(Debug, Clone)]
pub struct FuzzCase {
    pub bindings: Vec<(String, Expr)>,
    pub expected: Expr,
    pub span: Option<crate::errors::Span>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Hashtag {
    pub name: String,
    pub value: Option<String>,
    pub mandatory: bool,
    pub speculative: bool,
    pub fallback: Vec<String>,
    pub scoped: Option<String>,
}

impl Hashtag {
    pub fn new(name: String) -> Self {
        Hashtag { name, value: None, mandatory: false, speculative: false, fallback: Vec::new(), scoped: None }
    }

    pub fn mandatory(name: String) -> Self {
        Hashtag { name, value: None, mandatory: true, speculative: false, fallback: Vec::new(), scoped: None }
    }

    pub fn speculative(name: String) -> Self {
        Hashtag { name, value: None, mandatory: false, speculative: true, fallback: Vec::new(), scoped: None }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StrictMode {
    Off,
    Strict,
    /// Accelerated Brief (`.abv`) — native GPU compilation (SPIR-V).
    /// No FFI, restricted types, GPU intrinsics only.
    /// Contracts are optional (warned about, not required).
    Gpu,
}

impl StrictMode {
    pub fn is_strict(self) -> bool {
        matches!(self, StrictMode::Strict)
    }
    pub fn is_gpu(self) -> bool {
        matches!(self, StrictMode::Gpu)
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
    pub watchdog_defaults: (Option<u64>, Option<u64>), // NEW
    /// Default sig modifier for the file scope: Some(Out) or Some(Inline)
    pub default_sig_modifier: Option<SigModifier>,
}

impl Default for Program {
    fn default() -> Self {
        Program {
            items: vec![],
            comments: vec![],
            reactor_speed: None,
            attrs: Vec::new(),
            ffi: None,
            strict_mode: StrictMode::Off,
            dispatch_mode: DispatchMode::Sequential,
            exit_condition: None,
            out_pragmas: vec![],
            watchdog_defaults: (None, None),
            default_sig_modifier: None,
        }
    }
}

impl Program {
    /// Synthesize an `__init` transaction from top-level `Statement` items.
    ///
    /// Collects all `TopLevel::Statement` nodes in program order and wraps them
    /// in a synthesized `rct txn __init [!__booted_N][__booted_N] { ... }`.
    /// A collision-avoiding `__booted_N` flag is added as a state declaration.
    pub fn synthesize_init_txn(&mut self) {
        let stmt_indices: Vec<usize> = self.items.iter()
            .enumerate()
            .filter(|(_, item)| matches!(item, TopLevel::Statement(_)))
            .map(|(i, _)| i)
            .collect();

        if stmt_indices.is_empty() {
            return;
        }

        // Collect all statements in order
        let mut body: Vec<Statement> = Vec::new();
        for &i in stmt_indices.iter().rev() {
            if let TopLevel::Statement(stmt) = self.items.remove(i) {
                body.insert(0, *stmt);
            }
        }

        // Find a unique booted flag name (collision avoidance)
        let booted_name = self.find_unique_booted_name();

        // Create state declaration: let __booted_N: Bool = false;
        let state_decl = TopLevel::StateDecl(StateDecl {
            name: booted_name.clone(),
            ty: Type::Int,
            expr: Some(Expr::Integer(0)),
            address: None,
            bit_range: None,
            constraint: None,
            is_override: false,
            os_mode: false,
            attrs: vec![],
            span: None,
        });

        // Add &__booted_N = true; before term
        body.push(Statement::Assignment {
            lhs: Expr::OwnedRef(booted_name.clone()),
            expr: Expr::Integer(1),
            timeout: None,
            modifiers: vec![],
        });
        body.push(Statement::Term {
            values: vec![],
            modifiers: vec![],
            swan_song: None,
        });

        // Create the __init transaction
        let init_txn = TopLevel::Transaction(Transaction {
            name: "__init".to_string(),
            is_async: false,
            is_reactive: true, // rct — fires once when !__booted_N
            parameters: vec![],
            contract: Contract {
                pre_condition: Expr::Not(Box::new(Expr::OwnedRef(booted_name.clone()))),
                post_condition: Expr::OwnedRef(booted_name),
                watchdog: None,
                span: None,
            },
            body,
            reactor_speed: None,
            span: None,
            is_lambda: false,
            dependencies: vec![],
            annotations: vec![],
            modifiers: vec![],
            variant_bodies: vec![],
            outputs: vec![],
            output_type: None,
        });

        // Prepend the state decl (declarations must precede transactions)
        self.items.insert(0, state_decl);
        self.items.push(init_txn);
    }

    /// Inject Option<T> and Result<T, E> enum definitions into the program.
    /// These are required by the FFI system and always available.
    pub fn synthesize_builtin_types(&mut self) {
        // Only inject if not already present
        let has_option = self.items.iter().any(|item| {
            matches!(item, TopLevel::Enum(e) if e.name == "Option")
        });
        let has_result = self.items.iter().any(|item| {
            matches!(item, TopLevel::Enum(e) if e.name == "Result")
        });

        // enum Option<T> { Some(T), None }
        if !has_option {
            self.items.insert(0, TopLevel::Enum(EnumDefinition {
                name: "Option".to_string(),
                type_params: vec![TypeParam { name: "T".to_string(), bounds: vec![] }],
                variants: vec![
                    EnumVariant::Tuple("Some".to_string(), vec![Type::TypeVar("T".to_string())]),
                    EnumVariant::Unit("None".to_string()),
                ],
                span: None,
            }));
        }

        // enum Result<T, E> { Ok(T), Err(E) }
        if !has_result {
            self.items.insert(0, TopLevel::Enum(EnumDefinition {
                name: "Result".to_string(),
                type_params: vec![
                    TypeParam { name: "T".to_string(), bounds: vec![] },
                    TypeParam { name: "E".to_string(), bounds: vec![] },
                ],
                variants: vec![
                    EnumVariant::Tuple("Ok".to_string(), vec![Type::TypeVar("T".to_string())]),
                    EnumVariant::Tuple("Err".to_string(), vec![Type::TypeVar("E".to_string())]),
                ],
                span: None,
            }));
        }
    }

    fn find_unique_booted_name(&self) -> String {
        let existing: std::collections::HashSet<&str> = self.items.iter().filter_map(|item| {
            match item {
                TopLevel::StateDecl(s) => Some(s.name.as_str()),
                _ => None,
            }
        }).collect();
        for n in 0..64u32 {
            let name = format!("__booted_{}", n);
            if !existing.contains(name.as_str()) {
                return name;
            }
        }
        "__booted_overflow".to_string()
    }
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

#[cfg(kani)]
mod kani_tests {
    use super::*;
    use crate::features::literal::LiteralExpr;

    #[kani::proof]
    fn verify_as_integer_new_variant() {
        let e = Expr::Literal(Box::new(LiteralExpr::Integer(42)));
        assert_eq!(e.as_integer(), Some(42));
    }

    #[kani::proof]
    fn verify_as_integer_old_variant() {
        let e = Expr::Integer(42);
        assert_eq!(e.as_integer(), Some(42));
    }

    #[kani::proof]
    fn verify_as_bool_new_variant() {
        let e = Expr::Literal(Box::new(LiteralExpr::Bool(true)));
        assert_eq!(e.as_bool(), Some(true));
    }

    #[kani::proof]
    fn verify_as_bool_old_variant() {
        let e = Expr::Bool(false);
        assert_eq!(e.as_bool(), Some(false));
    }

    #[kani::proof]
    fn verify_is_term_new_variant() {
        let e = Expr::Literal(Box::new(LiteralExpr::Term));
        assert!(e.is_term());
    }

    #[kani::proof]
    fn verify_is_term_old_variant() {
        let e = Expr::Term;
        assert!(e.is_term());
    }

    #[kani::proof]
    fn verify_as_integer_none_for_non_int() {
        let e = Expr::Literal(Box::new(LiteralExpr::Bool(true)));
        assert_eq!(e.as_integer(), None);
    }

    #[kani::proof]
    fn verify_as_bool_none_for_non_bool() {
        let e = Expr::Literal(Box::new(LiteralExpr::Integer(0)));
        assert_eq!(e.as_bool(), None);
    }

    #[kani::proof]
    fn verify_is_term_false_for_non_term() {
        let e = Expr::Literal(Box::new(LiteralExpr::Integer(0)));
        assert!(!e.is_term());
    }
}

#[cfg(all(kani, feature = "kani_full"))]
mod kani_full_tests {
    use super::*;
    use crate::features::literal::LiteralExpr;

    #[kani::proof]
    fn verify_as_integer_none_for_non_int() {
        let e = Expr::Literal(Box::new(LiteralExpr::Bool(true)));
        assert_eq!(e.as_integer(), None);
        let f = Expr::Bool(false);
        assert_eq!(f.as_integer(), None);
    }

    #[kani::proof]
    fn verify_as_bool_none_for_non_bool() {
        let e = Expr::Literal(Box::new(LiteralExpr::Integer(0)));
        assert_eq!(e.as_bool(), None);
        let f = Expr::Integer(0);
        assert_eq!(f.as_bool(), None);
    }

    #[kani::proof]
    fn verify_is_term_false_for_non_term() {
        let e = Expr::Literal(Box::new(LiteralExpr::Integer(0)));
        assert!(!e.is_term());
        let f = Expr::Integer(0);
        assert!(!f.is_term());
    }

    #[kani::proof]
    fn verify_as_float_new_variant() {
        let e = Expr::Literal(Box::new(LiteralExpr::Float(3.14)));
        let v = e.as_float();
        assert!(v.is_some());
        let vv = v.unwrap();
        assert!((vv - 3.14).abs() < 1e-10);
    }

    #[kani::proof]
    fn verify_as_float_old_variant() {
        let e = Expr::Float(3.14);
        let v = e.as_float();
        assert!(v.is_some());
    }

    #[kani::proof]
    fn verify_as_string_new_variant() {
        let e = Expr::Literal(Box::new(LiteralExpr::String("hello".to_string())));
        assert_eq!(e.as_string(), Some(&"hello".to_string()));
    }

    #[kani::proof]
    fn verify_as_string_old_variant() {
        let e = Expr::String("hello".to_string());
        assert_eq!(e.as_string(), Some(&"hello".to_string()));
    }

    #[kani::proof]
    fn verify_as_string_none_for_non_string() {
        let e = Expr::Literal(Box::new(LiteralExpr::Integer(0)));
        assert_eq!(e.as_string(), None);
        let f = Expr::Integer(0);
        assert_eq!(f.as_string(), None);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_synthesize_builtin_types_basic() {
        let mut program = Program {
            items: vec![],
            comments: vec![], reactor_speed: None, attrs: vec![], ffi: None,
            strict_mode: StrictMode::Off, dispatch_mode: Default::default(),
            exit_condition: None, out_pragmas: vec![], watchdog_defaults: (None, None), default_sig_modifier: None,
        };
        program.synthesize_builtin_types();
        let enum_names: Vec<&str> = program.items.iter().filter_map(|item| {
            if let TopLevel::Enum(e) = item { Some(e.name.as_str()) } else { None }
        }).collect();
        assert!(enum_names.contains(&"Option"), "Option should be injected");
        assert!(enum_names.contains(&"Result"), "Result should be injected");
    }

    #[test]
    fn test_synthesize_builtin_types_no_duplicate() {
        let mut program = Program {
            items: vec![TopLevel::Enum(EnumDefinition {
                name: "Option".to_string(),
                type_params: vec![TypeParam { name: "T".to_string(), bounds: vec![] }],
                variants: vec![
                    EnumVariant::Tuple("Some".to_string(), vec![Type::TypeVar("T".to_string())]),
                    EnumVariant::Unit("None".to_string()),
                ],
                span: None,
            })],
            comments: vec![], reactor_speed: None, attrs: vec![], ffi: None,
            strict_mode: StrictMode::Off, dispatch_mode: Default::default(),
            exit_condition: None, out_pragmas: vec![], watchdog_defaults: (None, None), default_sig_modifier: None,
        };
        program.synthesize_builtin_types();
        let count = program.items.iter().filter(|item| {
            if let TopLevel::Enum(e) = item { e.name == "Option" } else { false }
        }).count();
        assert_eq!(count, 1, "Option should not be duplicated");
    }

    #[test]
    fn test_option_variants_correct() {
        let mut program = Program {
            items: vec![],
            comments: vec![], reactor_speed: None, attrs: vec![], ffi: None,
            strict_mode: StrictMode::Off, dispatch_mode: Default::default(),
            exit_condition: None, out_pragmas: vec![], watchdog_defaults: (None, None), default_sig_modifier: None,
        };
        program.synthesize_builtin_types();
        let option = program.items.iter().find_map(|item| {
            if let TopLevel::Enum(e) = item { if e.name == "Option" { Some(e) } else { None } } else { None }
        }).unwrap();
        assert_eq!(option.variants.len(), 2);
        assert_eq!(option.type_params.len(), 1);
        assert_eq!(option.type_params[0].name, "T");
    }

    #[test]
    fn test_result_variants_correct() {
        let mut program = Program {
            items: vec![],
            comments: vec![], reactor_speed: None, attrs: vec![], ffi: None,
            strict_mode: StrictMode::Off, dispatch_mode: Default::default(),
            exit_condition: None, out_pragmas: vec![], watchdog_defaults: (None, None), default_sig_modifier: None,
        };
        program.synthesize_builtin_types();
        let result = program.items.iter().find_map(|item| {
            if let TopLevel::Enum(e) = item { if e.name == "Result" { Some(e) } else { None } } else { None }
        }).unwrap();
        assert_eq!(result.variants.len(), 2);
        assert_eq!(result.type_params.len(), 2);
        assert_eq!(result.type_params[0].name, "T");
        assert_eq!(result.type_params[1].name, "E");
    }

    // ── Intrinsic::from_name / name roundtrip tests ────────────────

    #[test]
    fn test_intrinsic_from_name_tty_raw_mode() {
        assert_eq!(Intrinsic::from_name("tty_raw_mode"), Some(Intrinsic::TtyRawMode));
        assert_eq!(Intrinsic::TtyRawMode.name(), "tty_raw_mode");
    }

    #[test]
    fn test_intrinsic_from_name_tty_size() {
        assert_eq!(Intrinsic::from_name("tty_size"), Some(Intrinsic::TtySize));
        assert_eq!(Intrinsic::TtySize.name(), "tty_size");
    }

    #[test]
    fn test_intrinsic_from_name_tty_read_key() {
        assert_eq!(Intrinsic::from_name("tty_read_key"), Some(Intrinsic::TtyReadKey));
        assert_eq!(Intrinsic::TtyReadKey.name(), "tty_read_key");
    }

    #[test]
    fn test_intrinsic_from_name_ioctl() {
        assert_eq!(Intrinsic::from_name("ioctl"), Some(Intrinsic::IoCtl));
        assert_eq!(Intrinsic::IoCtl.name(), "ioctl");
    }

    #[test]
    fn test_intrinsic_from_name_isatty() {
        assert_eq!(Intrinsic::from_name("isatty"), Some(Intrinsic::IsTty));
        assert_eq!(Intrinsic::IsTty.name(), "isatty");
    }

    #[test]
    fn test_intrinsic_from_name_spawn_with_output() {
        assert_eq!(Intrinsic::from_name("spawn_with_output"), Some(Intrinsic::SpawnWithOutput));
        assert_eq!(Intrinsic::SpawnWithOutput.name(), "spawn_with_output");
    }

    #[test]
    fn test_intrinsic_from_name_spawn() {
        assert_eq!(Intrinsic::from_name("spawn"), Some(Intrinsic::Spawn));
        assert_eq!(Intrinsic::Spawn.name(), "spawn");
    }

    #[test]
    fn test_intrinsic_from_name_open() {
        assert_eq!(Intrinsic::from_name("open"), Some(Intrinsic::Open));
        assert_eq!(Intrinsic::Open.name(), "open");
    }

    #[test]
    fn test_intrinsic_from_name_close() {
        assert_eq!(Intrinsic::from_name("close"), Some(Intrinsic::Close));
        assert_eq!(Intrinsic::Close.name(), "close");
    }

    #[test]
    fn test_intrinsic_from_name_read() {
        assert_eq!(Intrinsic::from_name("read"), Some(Intrinsic::Read));
        assert_eq!(Intrinsic::Read.name(), "read");
    }

    #[test]
    fn test_intrinsic_from_name_write() {
        assert_eq!(Intrinsic::from_name("write"), Some(Intrinsic::Write));
        assert_eq!(Intrinsic::Write.name(), "write");
    }

    #[test]
    fn test_intrinsic_from_name_lseek() {
        assert_eq!(Intrinsic::from_name("lseek"), Some(Intrinsic::LSeek));
        assert_eq!(Intrinsic::LSeek.name(), "lseek");
    }

    #[test]
    fn test_intrinsic_from_name_pread() {
        assert_eq!(Intrinsic::from_name("pread"), Some(Intrinsic::PRead));
        assert_eq!(Intrinsic::PRead.name(), "pread");
    }

    #[test]
    fn test_intrinsic_from_name_pwrite() {
        assert_eq!(Intrinsic::from_name("pwrite"), Some(Intrinsic::PWrite));
        assert_eq!(Intrinsic::PWrite.name(), "pwrite");
    }

    #[test]
    fn test_intrinsic_from_name_stat() {
        assert_eq!(Intrinsic::from_name("stat"), Some(Intrinsic::Stat));
        assert_eq!(Intrinsic::Stat.name(), "stat");
    }

    #[test]
    fn test_intrinsic_from_name_fstat() {
        assert_eq!(Intrinsic::from_name("fstat"), Some(Intrinsic::FStat));
        assert_eq!(Intrinsic::FStat.name(), "fstat");
    }

    #[test]
    fn test_intrinsic_from_name_truncate() {
        assert_eq!(Intrinsic::from_name("truncate"), Some(Intrinsic::FTruncate));
        assert_eq!(Intrinsic::FTruncate.name(), "ftruncate");
    }

    #[test]
    fn test_intrinsic_from_name_ftruncate() {
        assert_eq!(Intrinsic::from_name("ftruncate"), Some(Intrinsic::FTruncate));
        assert_eq!(Intrinsic::FTruncate.name(), "ftruncate");
    }

    #[test]
    fn test_intrinsic_from_name_fsync() {
        assert_eq!(Intrinsic::from_name("fsync"), Some(Intrinsic::FSync));
        assert_eq!(Intrinsic::FSync.name(), "fsync");
    }

    #[test]
    fn test_intrinsic_from_name_dup() {
        assert_eq!(Intrinsic::from_name("dup"), Some(Intrinsic::FDup));
        assert_eq!(Intrinsic::FDup.name(), "dup");
    }

    #[test]
    fn test_intrinsic_from_name_dup2() {
        assert_eq!(Intrinsic::from_name("dup2"), Some(Intrinsic::FDup2));
        assert_eq!(Intrinsic::FDup2.name(), "dup2");
    }

    #[test]
    fn test_intrinsic_from_name_fcntl() {
        assert_eq!(Intrinsic::from_name("fcntl"), Some(Intrinsic::FCntl));
        assert_eq!(Intrinsic::FCntl.name(), "fcntl");
    }

    #[test]
    fn test_intrinsic_from_name_mkdir() {
        assert_eq!(Intrinsic::from_name("mkdir"), Some(Intrinsic::MkDir));
        assert_eq!(Intrinsic::MkDir.name(), "mkdir");
    }

    #[test]
    fn test_intrinsic_from_name_rmdir() {
        assert_eq!(Intrinsic::from_name("rmdir"), Some(Intrinsic::RmDir));
        assert_eq!(Intrinsic::RmDir.name(), "rmdir");
    }

    #[test]
    fn test_intrinsic_from_name_unlink() {
        assert_eq!(Intrinsic::from_name("unlink"), Some(Intrinsic::Unlink));
        assert_eq!(Intrinsic::Unlink.name(), "unlink");
    }

    #[test]
    fn test_intrinsic_from_name_rename() {
        assert_eq!(Intrinsic::from_name("rename"), Some(Intrinsic::Rename));
        assert_eq!(Intrinsic::Rename.name(), "rename");
    }

    #[test]
    fn test_intrinsic_from_name_symlink() {
        assert_eq!(Intrinsic::from_name("symlink"), Some(Intrinsic::SymLink));
        assert_eq!(Intrinsic::SymLink.name(), "symlink");
    }

    #[test]
    fn test_intrinsic_from_name_readlink() {
        assert_eq!(Intrinsic::from_name("readlink"), Some(Intrinsic::ReadLink));
        assert_eq!(Intrinsic::ReadLink.name(), "readlink");
    }

    #[test]
    fn test_intrinsic_from_name_link() {
        assert_eq!(Intrinsic::from_name("link"), Some(Intrinsic::Link));
        assert_eq!(Intrinsic::Link.name(), "link");
    }

    #[test]
    fn test_intrinsic_from_name_getcwd() {
        assert_eq!(Intrinsic::from_name("getcwd"), Some(Intrinsic::GetCwd));
        assert_eq!(Intrinsic::GetCwd.name(), "getcwd");
    }

    #[test]
    fn test_intrinsic_from_name_chdir() {
        assert_eq!(Intrinsic::from_name("chdir"), Some(Intrinsic::ChDir));
        assert_eq!(Intrinsic::ChDir.name(), "chdir");
    }

    #[test]
    fn test_intrinsic_from_name_readdir() {
        assert_eq!(Intrinsic::from_name("readdir"), Some(Intrinsic::ReadDir));
        assert_eq!(Intrinsic::ReadDir.name(), "readdir");
    }

    #[test]
    fn test_intrinsic_from_name_chmod() {
        assert_eq!(Intrinsic::from_name("chmod"), Some(Intrinsic::ChMod));
        assert_eq!(Intrinsic::ChMod.name(), "chmod");
    }

    #[test]
    fn test_intrinsic_from_name_chown() {
        assert_eq!(Intrinsic::from_name("chown"), Some(Intrinsic::ChOwn));
        assert_eq!(Intrinsic::ChOwn.name(), "chown");
    }

    #[test]
    fn test_intrinsic_from_name_umask() {
        assert_eq!(Intrinsic::from_name("umask"), Some(Intrinsic::UMask));
        assert_eq!(Intrinsic::UMask.name(), "umask");
    }

    #[test]
    fn test_intrinsic_from_name_access() {
        assert_eq!(Intrinsic::from_name("access"), Some(Intrinsic::Access));
        assert_eq!(Intrinsic::Access.name(), "access");
    }

    #[test]
    fn test_intrinsic_from_name_mmap() {
        assert_eq!(Intrinsic::from_name("mmap"), Some(Intrinsic::Mmap));
        assert_eq!(Intrinsic::Mmap.name(), "mmap");
    }

    #[test]
    fn test_intrinsic_from_name_munmap() {
        assert_eq!(Intrinsic::from_name("munmap"), Some(Intrinsic::MUnmap));
        assert_eq!(Intrinsic::MUnmap.name(), "munmap");
    }

    #[test]
    fn test_intrinsic_from_name_mprotect() {
        assert_eq!(Intrinsic::from_name("mprotect"), Some(Intrinsic::MProtect));
        assert_eq!(Intrinsic::MProtect.name(), "mprotect");
    }

    #[test]
    fn test_intrinsic_from_name_brk() {
        assert_eq!(Intrinsic::from_name("brk"), Some(Intrinsic::Brk));
        assert_eq!(Intrinsic::Brk.name(), "brk");
    }

    #[test]
    fn test_intrinsic_from_name_mlock() {
        assert_eq!(Intrinsic::from_name("mlock"), Some(Intrinsic::MLock));
        assert_eq!(Intrinsic::MLock.name(), "mlock");
    }

    #[test]
    fn test_intrinsic_from_name_atomic_load() {
        assert_eq!(Intrinsic::from_name("atomic_load"), Some(Intrinsic::AtomicLoad));
        assert_eq!(Intrinsic::AtomicLoad.name(), "atomic_load");
    }

    #[test]
    fn test_intrinsic_from_name_atomic_store() {
        assert_eq!(Intrinsic::from_name("atomic_store"), Some(Intrinsic::AtomicStore));
        assert_eq!(Intrinsic::AtomicStore.name(), "atomic_store");
    }

    #[test]
    fn test_intrinsic_from_name_atomic_cas() {
        assert_eq!(Intrinsic::from_name("atomic_cas"), Some(Intrinsic::AtomicCas));
        assert_eq!(Intrinsic::AtomicCas.name(), "atomic_cas");
    }

    #[test]
    fn test_intrinsic_from_name_atomic_xchg() {
        assert_eq!(Intrinsic::from_name("atomic_xchg"), Some(Intrinsic::AtomicXchg));
        assert_eq!(Intrinsic::AtomicXchg.name(), "atomic_xchg");
    }

    #[test]
    fn test_intrinsic_from_name_atomic_add() {
        assert_eq!(Intrinsic::from_name("atomic_add"), Some(Intrinsic::AtomicAdd));
        assert_eq!(Intrinsic::AtomicAdd.name(), "atomic_add");
    }

    #[test]
    fn test_intrinsic_from_name_fence() {
        assert_eq!(Intrinsic::from_name("fence"), Some(Intrinsic::Fence));
        assert_eq!(Intrinsic::Fence.name(), "fence");
    }

    #[test]
    fn test_intrinsic_from_name_futex() {
        assert_eq!(Intrinsic::from_name("futex"), Some(Intrinsic::Futex));
        assert_eq!(Intrinsic::Futex.name(), "futex");
    }

    #[test]
    fn test_intrinsic_from_name_pipe() {
        assert_eq!(Intrinsic::from_name("pipe"), Some(Intrinsic::Pipe));
        assert_eq!(Intrinsic::Pipe.name(), "pipe");
    }

    #[test]
    fn test_intrinsic_from_name_shm_open() {
        assert_eq!(Intrinsic::from_name("shm_open"), Some(Intrinsic::ShmOpen));
        assert_eq!(Intrinsic::ShmOpen.name(), "shm_open");
    }

    #[test]
    fn test_intrinsic_from_name_shm_unlink() {
        assert_eq!(Intrinsic::from_name("shm_unlink"), Some(Intrinsic::ShmUnlink));
        assert_eq!(Intrinsic::ShmUnlink.name(), "shm_unlink");
    }

    #[test]
    fn test_intrinsic_from_name_sem_open() {
        assert_eq!(Intrinsic::from_name("sem_open"), Some(Intrinsic::SemOpen));
        assert_eq!(Intrinsic::SemOpen.name(), "sem_open");
    }

    #[test]
    fn test_intrinsic_from_name_sem_wait() {
        assert_eq!(Intrinsic::from_name("sem_wait"), Some(Intrinsic::SemWait));
        assert_eq!(Intrinsic::SemWait.name(), "sem_wait");
    }

    #[test]
    fn test_intrinsic_from_name_sem_post() {
        assert_eq!(Intrinsic::from_name("sem_post"), Some(Intrinsic::SemPost));
        assert_eq!(Intrinsic::SemPost.name(), "sem_post");
    }

    #[test]
    fn test_intrinsic_from_name_sigaction() {
        assert_eq!(Intrinsic::from_name("sigaction"), Some(Intrinsic::SigAction));
        assert_eq!(Intrinsic::SigAction.name(), "sigaction");
    }

    #[test]
    fn test_intrinsic_from_name_sigprocmask() {
        assert_eq!(Intrinsic::from_name("sigprocmask"), Some(Intrinsic::SigProcMask));
        assert_eq!(Intrinsic::SigProcMask.name(), "sigprocmask");
    }

    #[test]
    fn test_intrinsic_from_name_kill() {
        assert_eq!(Intrinsic::from_name("kill"), Some(Intrinsic::Kill));
        assert_eq!(Intrinsic::Kill.name(), "kill");
    }

    #[test]
    fn test_intrinsic_from_name_signalfd() {
        assert_eq!(Intrinsic::from_name("signalfd"), Some(Intrinsic::SignalFd));
        assert_eq!(Intrinsic::SignalFd.name(), "signalfd");
    }

    #[test]
    fn test_intrinsic_from_name_timerfd_create() {
        assert_eq!(Intrinsic::from_name("timerfd_create"), Some(Intrinsic::TimerFdCreate));
        assert_eq!(Intrinsic::TimerFdCreate.name(), "timerfd_create");
    }

    #[test]
    fn test_intrinsic_from_name_socket() {
        assert_eq!(Intrinsic::from_name("socket"), Some(Intrinsic::Socket));
        assert_eq!(Intrinsic::Socket.name(), "socket");
    }

    #[test]
    fn test_intrinsic_from_name_bind() {
        assert_eq!(Intrinsic::from_name("bind"), Some(Intrinsic::Bind));
        assert_eq!(Intrinsic::Bind.name(), "bind");
    }

    #[test]
    fn test_intrinsic_from_name_listen() {
        assert_eq!(Intrinsic::from_name("listen"), Some(Intrinsic::Listen));
        assert_eq!(Intrinsic::Listen.name(), "listen");
    }

    #[test]
    fn test_intrinsic_from_name_accept() {
        assert_eq!(Intrinsic::from_name("accept"), Some(Intrinsic::Accept));
        assert_eq!(Intrinsic::Accept.name(), "accept");
    }

    #[test]
    fn test_intrinsic_from_name_connect() {
        assert_eq!(Intrinsic::from_name("connect"), Some(Intrinsic::Connect));
        assert_eq!(Intrinsic::Connect.name(), "connect");
    }

    #[test]
    fn test_intrinsic_from_name_send() {
        assert_eq!(Intrinsic::from_name("send"), Some(Intrinsic::Send));
        assert_eq!(Intrinsic::Send.name(), "send");
    }

    #[test]
    fn test_intrinsic_from_name_recv() {
        assert_eq!(Intrinsic::from_name("recv"), Some(Intrinsic::Recv));
        assert_eq!(Intrinsic::Recv.name(), "recv");
    }

    #[test]
    fn test_intrinsic_from_name_sendto() {
        assert_eq!(Intrinsic::from_name("sendto"), Some(Intrinsic::SendTo));
        assert_eq!(Intrinsic::SendTo.name(), "sendto");
    }

    #[test]
    fn test_intrinsic_from_name_recvfrom() {
        assert_eq!(Intrinsic::from_name("recvfrom"), Some(Intrinsic::RecvFrom));
        assert_eq!(Intrinsic::RecvFrom.name(), "recvfrom");
    }

    #[test]
    fn test_intrinsic_from_name_setsockopt() {
        assert_eq!(Intrinsic::from_name("setsockopt"), Some(Intrinsic::SetSockOpt));
        assert_eq!(Intrinsic::SetSockOpt.name(), "setsockopt");
    }

    #[test]
    fn test_intrinsic_from_name_getsockopt() {
        assert_eq!(Intrinsic::from_name("getsockopt"), Some(Intrinsic::GetSockOpt));
        assert_eq!(Intrinsic::GetSockOpt.name(), "getsockopt");
    }

    #[test]
    fn test_intrinsic_from_name_shutdown() {
        assert_eq!(Intrinsic::from_name("shutdown"), Some(Intrinsic::Shutdown));
        assert_eq!(Intrinsic::Shutdown.name(), "shutdown");
    }

    #[test]
    fn test_intrinsic_from_name_getaddrinfo() {
        assert_eq!(Intrinsic::from_name("getaddrinfo"), Some(Intrinsic::GetAddrInfo));
        assert_eq!(Intrinsic::GetAddrInfo.name(), "getaddrinfo");
    }

    #[test]
    fn test_intrinsic_from_name_getenv() {
        assert_eq!(Intrinsic::from_name("getenv"), Some(Intrinsic::GetEnv));
        assert_eq!(Intrinsic::GetEnv.name(), "getenv");
    }

    #[test]
    fn test_intrinsic_from_name_setenv() {
        assert_eq!(Intrinsic::from_name("setenv"), Some(Intrinsic::SetEnv));
        assert_eq!(Intrinsic::SetEnv.name(), "setenv");
    }

    #[test]
    fn test_intrinsic_from_name_unsetenv() {
        assert_eq!(Intrinsic::from_name("unsetenv"), Some(Intrinsic::UnsetEnv));
        assert_eq!(Intrinsic::UnsetEnv.name(), "unsetenv");
    }

    #[test]
    fn test_intrinsic_from_name_getpid() {
        assert_eq!(Intrinsic::from_name("getpid"), Some(Intrinsic::GetPid));
        assert_eq!(Intrinsic::GetPid.name(), "getpid");
    }

    #[test]
    fn test_intrinsic_from_name_getppid() {
        assert_eq!(Intrinsic::from_name("getppid"), Some(Intrinsic::GetPPid));
        assert_eq!(Intrinsic::GetPPid.name(), "getppid");
    }

    #[test]
    fn test_intrinsic_from_name_clock_gettime() {
        assert_eq!(Intrinsic::from_name("clock_gettime"), Some(Intrinsic::ClockGetTime));
        assert_eq!(Intrinsic::ClockGetTime.name(), "clock_gettime");
    }

    #[test]
    fn test_intrinsic_from_name_nanosleep() {
        assert_eq!(Intrinsic::from_name("nanosleep"), Some(Intrinsic::NanoSleep));
        assert_eq!(Intrinsic::NanoSleep.name(), "nanosleep");
    }

    #[test]
    fn test_intrinsic_from_name_d12_errno() {
        assert_eq!(Intrinsic::from_name("errno"), Some(Intrinsic::Errno));
        assert_eq!(Intrinsic::Errno.name(), "errno");
    }

    #[test]
    fn test_intrinsic_from_name_d12_getrandom() {
        assert_eq!(Intrinsic::from_name("getrandom"), Some(Intrinsic::GetRandom));
        assert_eq!(Intrinsic::GetRandom.name(), "getrandom");
    }

    #[test]
    fn test_intrinsic_from_name_d13_uname() {
        assert_eq!(Intrinsic::from_name("uname"), Some(Intrinsic::Uname));
        assert_eq!(Intrinsic::Uname.name(), "uname");
    }

    #[test]
    fn test_intrinsic_from_name_d13_pagesize() {
        assert_eq!(Intrinsic::from_name("pagesize"), Some(Intrinsic::PageSize));
        assert_eq!(Intrinsic::PageSize.name(), "pagesize");
    }

    #[test]
    fn test_intrinsic_from_name_d13_cpu_count() {
        assert_eq!(Intrinsic::from_name("cpu_count"), Some(Intrinsic::CpuCount));
        assert_eq!(Intrinsic::CpuCount.name(), "cpu_count");
    }

    #[test]
    fn test_intrinsic_from_name_d13_hostname() {
        assert_eq!(Intrinsic::from_name("hostname"), Some(Intrinsic::Hostname));
        assert_eq!(Intrinsic::Hostname.name(), "hostname");
    }

    #[test]
    fn test_intrinsic_from_name_d13_strerror() {
        assert_eq!(Intrinsic::from_name("strerror"), Some(Intrinsic::StrError));
        assert_eq!(Intrinsic::StrError.name(), "strerror");
    }

    #[test]
    fn test_intrinsic_from_name_d13_strsignal() {
        assert_eq!(Intrinsic::from_name("strsignal"), Some(Intrinsic::StrSignal));
        assert_eq!(Intrinsic::StrSignal.name(), "strsignal");
    }

    #[test]
    fn test_intrinsic_from_name_d13_realpath() {
        assert_eq!(Intrinsic::from_name("realpath"), Some(Intrinsic::RealPath));
        assert_eq!(Intrinsic::RealPath.name(), "realpath");
    }

    #[test]
    fn test_intrinsic_from_name_d14_abort() {
        assert_eq!(Intrinsic::from_name("abort"), Some(Intrinsic::Abort));
        assert_eq!(Intrinsic::Abort.name(), "abort");
    }

    #[test]
    fn test_intrinsic_from_name_d14_backtrace() {
        assert_eq!(Intrinsic::from_name("backtrace"), Some(Intrinsic::Backtrace));
        assert_eq!(Intrinsic::Backtrace.name(), "backtrace");
    }

    #[test]
    fn test_intrinsic_from_name_d15_sched_yield() {
        assert_eq!(Intrinsic::from_name("sched_yield"), Some(Intrinsic::SchedYield));
        assert_eq!(Intrinsic::SchedYield.name(), "sched_yield");
    }

    #[test]
    fn test_intrinsic_from_name_d15_getpriority() {
        assert_eq!(Intrinsic::from_name("getpriority"), Some(Intrinsic::GetPriority));
        assert_eq!(Intrinsic::GetPriority.name(), "getpriority");
    }

    #[test]
    fn test_intrinsic_from_name_d15_setpriority() {
        assert_eq!(Intrinsic::from_name("setpriority"), Some(Intrinsic::SetPriority));
        assert_eq!(Intrinsic::SetPriority.name(), "setpriority");
    }

    #[test]
    fn test_intrinsic_from_name_d16_getuid() {
        assert_eq!(Intrinsic::from_name("getuid"), Some(Intrinsic::GetUid));
        assert_eq!(Intrinsic::GetUid.name(), "getuid");
    }

    #[test]
    fn test_intrinsic_from_name_d16_geteuid() {
        assert_eq!(Intrinsic::from_name("geteuid"), Some(Intrinsic::GetEUid));
        assert_eq!(Intrinsic::GetEUid.name(), "geteuid");
    }

    #[test]
    fn test_intrinsic_from_name_d16_getgid() {
        assert_eq!(Intrinsic::from_name("getgid"), Some(Intrinsic::GetGid));
        assert_eq!(Intrinsic::GetGid.name(), "getgid");
    }

    #[test]
    fn test_intrinsic_from_name_d16_getegid() {
        assert_eq!(Intrinsic::from_name("getegid"), Some(Intrinsic::GetEGid));
        assert_eq!(Intrinsic::GetEGid.name(), "getegid");
    }

    #[test]
    fn test_intrinsic_from_name_d16_getpwuid() {
        assert_eq!(Intrinsic::from_name("getpwuid"), Some(Intrinsic::GetPwUid));
        assert_eq!(Intrinsic::GetPwUid.name(), "getpwuid");
    }

    #[test]
    fn test_intrinsic_from_name_d16_getgrgid() {
        assert_eq!(Intrinsic::from_name("getgrgid"), Some(Intrinsic::GetGrGid));
        assert_eq!(Intrinsic::GetGrGid.name(), "getgrgid");
    }

    #[test]
    fn test_intrinsic_from_name_d17_thread_create() {
        assert_eq!(Intrinsic::from_name("thread_create"), Some(Intrinsic::ThreadCreate));
        assert_eq!(Intrinsic::ThreadCreate.name(), "thread_create");
    }

    #[test]
    fn test_intrinsic_from_name_d17_thread_join() {
        assert_eq!(Intrinsic::from_name("thread_join"), Some(Intrinsic::ThreadJoin));
        assert_eq!(Intrinsic::ThreadJoin.name(), "thread_join");
    }

    #[test]
    fn test_intrinsic_from_name_d17_thread_exit() {
        assert_eq!(Intrinsic::from_name("thread_exit"), Some(Intrinsic::ThreadExit));
        assert_eq!(Intrinsic::ThreadExit.name(), "thread_exit");
    }

    #[test]
    fn test_intrinsic_from_name_d17_mutex_lock() {
        assert_eq!(Intrinsic::from_name("mutex_lock"), Some(Intrinsic::MutexLock));
        assert_eq!(Intrinsic::MutexLock.name(), "mutex_lock");
    }

    #[test]
    fn test_intrinsic_from_name_d17_mutex_unlock() {
        assert_eq!(Intrinsic::from_name("mutex_unlock"), Some(Intrinsic::MutexUnlock));
        assert_eq!(Intrinsic::MutexUnlock.name(), "mutex_unlock");
    }

    #[test]
    fn test_intrinsic_from_name_d17_condvar_wait() {
        assert_eq!(Intrinsic::from_name("condvar_wait"), Some(Intrinsic::CondvarWait));
        assert_eq!(Intrinsic::CondvarWait.name(), "condvar_wait");
    }

    #[test]
    fn test_intrinsic_from_name_d17_condvar_signal() {
        assert_eq!(Intrinsic::from_name("condvar_signal"), Some(Intrinsic::CondvarSignal));
        assert_eq!(Intrinsic::CondvarSignal.name(), "condvar_signal");
    }

    #[test]
    fn test_intrinsic_from_name_d17_condvar_broadcast() {
        assert_eq!(Intrinsic::from_name("condvar_broadcast"), Some(Intrinsic::CondvarBroadcast));
        assert_eq!(Intrinsic::CondvarBroadcast.name(), "condvar_broadcast");
    }

    #[test]
    fn test_intrinsic_from_name_d18_getrlimit() {
        assert_eq!(Intrinsic::from_name("getrlimit"), Some(Intrinsic::GetRlimit));
        assert_eq!(Intrinsic::GetRlimit.name(), "getrlimit");
    }

    #[test]
    fn test_intrinsic_from_name_d18_setrlimit() {
        assert_eq!(Intrinsic::from_name("setrlimit"), Some(Intrinsic::SetRlimit));
        assert_eq!(Intrinsic::SetRlimit.name(), "setrlimit");
    }

    #[test]
    fn test_intrinsic_from_name_extra_mkstemp() {
        assert_eq!(Intrinsic::from_name("mkstemp"), Some(Intrinsic::MkStemp));
        assert_eq!(Intrinsic::MkStemp.name(), "mkstemp");
    }

    #[test]
    fn test_intrinsic_from_name_extra_mkdtemp() {
        assert_eq!(Intrinsic::from_name("mkdtemp"), Some(Intrinsic::MkDtemp));
        assert_eq!(Intrinsic::MkDtemp.name(), "mkdtemp");
    }

    #[test]
    fn test_intrinsic_from_name_extra_dlopen() {
        assert_eq!(Intrinsic::from_name("dlopen"), Some(Intrinsic::DlOpen));
        assert_eq!(Intrinsic::DlOpen.name(), "dlopen");
    }

    #[test]
    fn test_intrinsic_from_name_extra_dlsym() {
        assert_eq!(Intrinsic::from_name("dlsym"), Some(Intrinsic::DlSym));
        assert_eq!(Intrinsic::DlSym.name(), "dlsym");
    }

    #[test]
    fn test_intrinsic_from_name_extra_dlclose() {
        assert_eq!(Intrinsic::from_name("dlclose"), Some(Intrinsic::DlClose));
        assert_eq!(Intrinsic::DlClose.name(), "dlclose");
    }

    #[test]
    fn test_intrinsic_from_name_extra_ttyname() {
        assert_eq!(Intrinsic::from_name("ttyname"), Some(Intrinsic::TtyName));
        assert_eq!(Intrinsic::TtyName.name(), "ttyname");
    }

    #[test]
    fn test_intrinsic_from_name_strlen() {
        assert_eq!(Intrinsic::from_name("strlen"), Some(Intrinsic::Strlen));
        assert_eq!(Intrinsic::Strlen.name(), "strlen");
    }

    #[test]
    fn test_intrinsic_from_name_unknown() {
        assert_eq!(Intrinsic::from_name("nonexistent"), None);
    }

    // ── LayoutPtr normalization tests ──────────────────────────

    #[test]
    fn test_normalize_layout_ptr_bits_range() {
        let ty = Type::Applied("Ptr".into(), vec![
            Type::Constrained(Box::new(Type::Data), BitRange::Range(0, 63))
        ]);
        let result = ty.normalize_layout_ptr();
        assert_eq!(result, Type::LayoutPtr(LayoutConstraint { bytes: 8, alignment: 8 }));
    }

    #[test]
    fn test_normalize_layout_ptr_bits_any() {
        let ty = Type::Applied("Ptr".into(), vec![
            Type::Constrained(Box::new(Type::Data), BitRange::Any(32))
        ]);
        let result = ty.normalize_layout_ptr();
        assert_eq!(result, Type::LayoutPtr(LayoutConstraint { bytes: 4, alignment: 4 }));
    }

    #[test]
    fn test_normalize_layout_ptr_typed_stays_applied() {
        // Ptr<Int> should stay as Applied("Ptr", [Int])
        let ty = Type::Applied("Ptr".into(), vec![Type::Int]);
        let result = ty.normalize_layout_ptr();
        assert_eq!(result, Type::Applied("Ptr".into(), vec![Type::Int]));
    }

    #[test]
    fn test_normalize_layout_ptr_already_layout_ptr() {
        // Already a LayoutPtr — should stay unchanged
        let ty = Type::LayoutPtr(LayoutConstraint { bytes: 16, alignment: 16 });
        let result = ty.normalize_layout_ptr();
        assert_eq!(result, Type::LayoutPtr(LayoutConstraint { bytes: 16, alignment: 16 }));
    }

    #[test]
    fn test_normalize_layout_ptr_in_union() {
        let ty = Type::Union(vec![
            Type::Int,
            Type::Applied("Ptr".into(), vec![
                Type::Constrained(Box::new(Type::Data), BitRange::Range(0, 31))
            ]),
        ]);
        let result = ty.normalize_layout_ptr();
        assert_eq!(result, Type::Union(vec![
            Type::Int,
            Type::LayoutPtr(LayoutConstraint { bytes: 4, alignment: 4 }),
        ]));
    }
}
