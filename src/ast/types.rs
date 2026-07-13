// ── Type System Definitions ─────────────────────────────────────────────
// 2026-07-12: Phase 0.2 — New architecture type system.
// All types are Bits(N) with metadata overlays. No built-in primitives.

use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    /// Raw bit sequence: Bits(8) = 8 bytes = 64 bits. The sole physical primitive.
    Bits(u64),
    /// Void = Bits(0). Not a separate concept.
    Void,
    /// User-named type: Custom("Int"), Custom("String"), Custom("MyType").
    Custom(String),
    /// Generic type reference: Generic("List", vec!["T".to_string()])
    Generic(String, Vec<Type>),
    /// Applied generic: Applied("List", [Custom("Int")])
    Applied(String, Vec<Type>),
    /// Union type: Union([Int, String, Error])
    Union(Vec<Type>),
    /// Tuple type: Tuple([Int, String])
    Tuple(Vec<Type>),
    /// Type variable for generics
    TypeVar(String),
    /// Pointer type: Ptr(T)
    Ptr(Box<Type>),
    /// Function type: Function([Int, Int], Box::new(Int))
    Function(Vec<Type>, Box<Type>),
    /// Type-level width: Width(8)
    Width(u64),
    /// Vector type with dimensions
    Vector(Box<Type>, Vec<Dimension>),
    /// Constrained type: Constrained(Bits(8), Range(0, 255))
    Constrained(Box<Type>, BitRange),
    /// Layout-constrained pointer
    LayoutPtr(LayoutConstraint),
}

// 2026-07-12: Named type factories. These create Custom() references that
// are resolved through the TypeUniverse at type-check time.
impl Type {
    /// Return the universe lookup key for this type.
    /// Custom("Float") → "Float", Applied("List", [T]) → "List".
    pub fn universe_key(&self) -> Option<&str> {
        match self {
            Type::Custom(name) => Some(name.as_str()),
            Type::Applied(name, _) => Some(name.as_str()),
            _ => None,
        }
    }

    pub fn int() -> Type {
        Type::Custom("Int".to_string())
    }
    pub fn float() -> Type {
        Type::Custom("Float".to_string())
    }
    pub fn float64() -> Type {
        Type::Custom("Float64".to_string())
    }
    pub fn bool_() -> Type {
        Type::Custom("Bool".to_string())
    }
    pub fn string() -> Type {
        Type::Custom("String".to_string())
    }
    pub fn char_() -> Type {
        Type::Custom("Char".to_string())
    }
    pub fn data() -> Type {
        Type::Custom("Data".to_string())
    }
    pub fn void() -> Type {
        Type::Void
    }
    pub fn bits(bytes: u64) -> Type {
        Type::Bits(bytes)
    }
    pub fn ptr(inner: Type) -> Type {
        Type::Ptr(Box::new(inner))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum BitRange {
    Single(usize),
    Range(usize, usize),
    Any(usize),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Dimension {
    Anonymous(usize),
    Named(String, usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LayoutConstraint {
    pub bytes: u64,
    pub alignment: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub enum TimeUnit {
    Cycles,
    Ms,
    Seconds,
    Minutes,
    Nanoseconds,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Formatting {
    /// Accepts "..." quoted literals
    Quoted,
    /// Accepts numeric literals (42, 3.14)
    Decimal,
    /// Accepts bare identifiers (Red, Green)
    Bare,
    /// Accepts all three forms
    Any,
    /// No literal syntax — constructor functions only
    None,
}

impl Formatting {
    pub fn from_name(name: &str) -> Option<Formatting> {
        match name {
            "Quoted" => Some(Formatting::Quoted),
            "Decimal" => Some(Formatting::Decimal),
            "Bare" => Some(Formatting::Bare),
            "Any" => Some(Formatting::Any),
            "None" => Some(Formatting::None),
            _ => None,
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Formatting::Quoted => "Quoted",
            Formatting::Decimal => "Decimal",
            Formatting::Bare => "Bare",
            Formatting::Any => "Any",
            Formatting::None => "None",
        }
    }
}

/// Value type for metadata properties in `<~` declarations.
#[derive(Debug, Clone, PartialEq)]
pub enum PropertyValue {
    Int(i64),
    Float(f64),
    Bool(bool),
    String(String),
    Identifier(String),
    List(Vec<PropertyValue>),
}

/// How an operator binding resolves to an implementation.
#[derive(Debug, Clone, PartialEq)]
pub enum OpBinding {
    /// Intrinsic function: AddI64#, Sqrt#, Malloc#
    Intrinsic(String),
    /// User-defined Brief function: my_add
    Function(String),
}

/// How a type is defined in the type universe.
#[derive(Debug, Clone)]
pub enum TypeKind {
    Struct(Vec<Field>),
    Enum(Vec<Variant>),
    Codec {
        formatting: Formatting,
        parse: Option<String>,
    },
    Alias(Box<Type>),
}

#[derive(Debug, Clone)]
pub struct Field {
    pub name: String,
    pub ty: Type,
    pub metadata: HashMap<String, PropertyValue>,
}

#[derive(Debug, Clone)]
pub struct Variant {
    pub name: String,
    pub fields: Vec<Field>,
    pub metadata: HashMap<String, PropertyValue>,
}

/// The constraint language for contract pre/post conditions.
/// Used by the SMT proof engine. Mirrors the Bool-valued Expr subset.
#[derive(Debug, Clone)]
pub enum Constraint {
    Eq(Box<Constraint>, Box<Constraint>),
    Neq(Box<Constraint>, Box<Constraint>),
    Lt(Box<Constraint>, Box<Constraint>),
    Gt(Box<Constraint>, Box<Constraint>),
    Le(Box<Constraint>, Box<Constraint>),
    Ge(Box<Constraint>, Box<Constraint>),
    And(Box<Constraint>, Box<Constraint>),
    Or(Box<Constraint>, Box<Constraint>),
    Not(Box<Constraint>),
    Implies(Box<Constraint>, Box<Constraint>),
    Forall(String, Box<Constraint>),
    Exists(String, Box<Constraint>),
    Literal(Box<crate::ast::Expr>),
    Bool(bool),
}
