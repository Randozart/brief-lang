use crate::errors::Span;

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Int,
    Float,
    String,
    Bool,
    Data,
    Void,
    Custom(String),
    Union(Vec<Type>),
    ContractBound(Box<Type>, Box<Expr>),
    TypeVar(String),
    Generic(String, Vec<Type>),
    Applied(String, Vec<Type>),
    Sig(String),       // Signature used as function type: sig name -> ...
    Option(Box<Type>), // Option<T> - Some(T) or None
    Enum(String),      // Enum type: Result, Color, etc.
}

#[derive(Debug, Clone)]
pub struct TypeParam {
    pub name: String,
    pub bounds: Vec<TypeBound>,
}

#[derive(Debug, Clone)]
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

/// Foreign Function Signature (from frgn declaration)
#[derive(Debug, Clone)]
pub struct ForeignSignature {
    pub name: String,
    pub location: String,            // TOML location (e.g., "std::f64::sqrt")
    pub inputs: Vec<(String, Type)>, // param_name -> type
    pub success_output: Vec<(String, Type)>, // named fields (can be empty for void)
    pub error_type_name: String,     // e.g., "IoError"
    pub error_fields: Vec<(String, Type)>, // error shape
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
    pub inputs: Vec<(String, Type)>, // Parameter names and types
    pub success_output: Vec<(String, Type)>, // Success output shape
    pub error_type: String,     // Error type name
    pub error_fields: Vec<(String, Type)>, // Error fields
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
    Call(String, Vec<Expr>),
    ListLiteral(Vec<Expr>),
    ListIndex(Box<Expr>, Box<Expr>),
    ListLen(Box<Expr>),
    FieldAccess(Box<Expr>, String),
    StructInstance(String, Vec<(String, Expr)>),
    ObjectLiteral(Vec<(String, Expr)>),
    // Pattern matching in guards: [value Variant(field1, field2)] { ... }
    PatternMatch {
        value: Box<Expr>,
        variant: String,
        fields: Vec<String>,
    },
}

impl Expr {
    pub fn span(&self) -> Option<Span> {
        None
    }
}

#[derive(Debug, Clone)]
pub enum Statement {
    // Assignment: &identifier = expr; or identifier = expr;
    Assignment {
        is_owned: bool,
        name: String,
        expr: Expr,
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
    },
}

#[derive(Debug, Clone)]
pub struct Contract {
    pub pre_condition: Expr,
    pub post_condition: Expr,
    pub span: Option<Span>,
}

impl Contract {
    pub fn new(pre: Expr, post: Expr) -> Self {
        Contract {
            pre_condition: pre,
            post_condition: post,
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
}

#[derive(Debug, Clone)]
pub struct StateDecl {
    pub name: String,
    pub ty: Type,
    pub expr: Option<Expr>,
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
pub struct ForeignSig {
    pub name: String,
    pub input_types: Vec<Type>,
    pub outputs: Vec<Type>,
}

#[derive(Debug, Clone)]
pub enum TopLevel {
    Signature(Signature),
    Definition(Definition),
    Transaction(Transaction),
    StateDecl(StateDecl),
    Constant(Constant),
    Import(Import),
    ForeignSig(ForeignSig),
    ForeignBinding {
        name: String,
        toml_path: String,
        signature: ForeignSignature,
        target: ForeignTarget,
        span: Option<Span>,
    },
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
