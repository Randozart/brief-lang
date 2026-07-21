// ── Layout Pattern AST ──────────────────────────────────────────────────
// 2026-07-14: AST types for the Layout DSL. Parsed from raw layout text
// by src/beast/layout.rs, stored in ResolvedType.properties["layout"].

/// Endianness of a layout pattern.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Endianness {
    Little,
    Big,
    Target,
}

/// A single field in a fixed-width layout slice.
#[derive(Debug, Clone, PartialEq)]
pub struct LayoutField {
    pub name: String,
    pub bits: u64,
    pub mutable: bool,
    pub structural: bool,
}

/// A parsed layout pattern — the AST of the Layout DSL.
#[derive(Debug, Clone, PartialEq)]
pub enum LayoutPattern {
    /// Fixed-width slice: [name: N, ...]
    Slice(Vec<LayoutField>),
    /// Sequence: A B C
    Sequence(Vec<LayoutPattern>),
    /// Alternation: A | B | C
    Alternation(Vec<LayoutPattern>),
    /// Zero or more: (...)*
    Repetition(Box<LayoutPattern>),
    /// Optional: (...)?
    Optional(Box<LayoutPattern>),
    /// Literal byte: 0xNN
    ByteLiteral(u8),
    /// Byte range: 0xNN..0xNN
    ByteRange(u8, u8),
    /// N bytes of any value: {N}
    AnyBytes(u64),
    /// Variable-length reference: {$name}
    VariableRef(String),
    /// Typed reference: {$count, T} or {$count, ($K, $V)}
    TypedRef(String, Box<LayoutPattern>),
    /// Pointer-to-region binding: *elements
    PointerRef(String),
    /// Semantic label: @name: pattern
    SemanticLabel(String, Box<LayoutPattern>),
    /// Generic type placeholder: $T, $K, $V
    GenericParam(String),
}
