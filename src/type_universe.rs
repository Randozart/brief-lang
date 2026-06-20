// ── Pass 1: Type-Universe Resolver ──────────────────────────────────
//
// Phase 1.5: This module collects all `TopLevel::TypeDef` declarations,
// resolves their derivation chains to `Bits`, inherits/overrides metadata,
// and freezes the type universe for Pass 2.
//
// DESIGN (REFACTOR_PLAN.md §Phase 1.5):
//   The Type-Universe Pass runs before all other analysis. It builds a
//   frozen map of type metadata that Pass 2 uses for:
//     - Resolving `let x: Stack<T>` against the universe
//     - Validating `:>` projections against defined type properties
//     - Synthesizing bracket/arrow access from AllowIndex/AllowArrow
//     - Compile-time literal encoding through Codec
//
// DEFERRED:
//   See Phase 1.5+ deferred items in REFACTOR_PLAN.md:
//   D-1 (Expression type parameters), D-2 (Full codec validation),
//   D-3 (Strategy synthesis), D-5 (Size uniformity),
//   D-6 (Volatile/Atomic pragmas), D-7 (Runtime guard synthesis).

use std::collections::HashMap;
use crate::ast::{Expr, Program, TopLevel, TypeDef, TypeDefBody, TypeProperty};

/// Resolved metadata for a single type in the universe.
#[derive(Debug, Clone)]
pub struct ResolvedType {
    /// The type's name.
    pub name: String,
    /// Type parameters (inherited from derivation chain).
    pub type_params: Vec<String>,
    /// Base type name (resolved, e.g. "Bits" from any ancestor).
    pub base: String,
    /// Resolved physical width in bytes.
    pub bytes: u64,
    /// Resolved alignment boundary.
    pub alignment: u64,
    /// Byte order: 0 = Little, 1 = Big.
    pub endian: u8,
    /// Volatile flag.
    pub volatile: bool,
    /// Atomic flag.
    pub atomic: bool,
    /// Element type (if collection).
    pub element_type: Option<String>,
    /// Whether size is fixed at compile time.
    pub fixed_size: Option<bool>,
    /// InsertAt expression string (for strategy synthesis in later phase).
    pub insert_at: Option<String>,
    /// ExtractFrom expression string (for strategy synthesis).
    pub extract_from: Option<String>,
    /// Allow index access.
    pub allow_index: bool,
    /// Allow slice access.
    pub allow_slice: bool,
    /// Allow arrow mutation.
    pub allow_arrow: bool,
    /// Codec struct name (if any).
    pub codec: Option<String>,
    /// Source TypeDef for reference.
    pub source: TypeDef,
}

/// The frozen type universe — built in Pass 1, read-only in Pass 2.
#[derive(Debug, Clone)]
pub struct TypeUniverse {
    /// Map from type name to resolved metadata.
    pub types: HashMap<String, ResolvedType>,
    /// Ordered list of resolution (for deterministic output).
    pub resolution_order: Vec<String>,
}

impl TypeUniverse {
    pub fn new() -> Self {
        TypeUniverse {
            types: HashMap::new(),
            resolution_order: Vec::new(),
        }
    }

    /// Build the type universe from a program's TopLevel items.
    /// Collects TypeDef declarations, resolves derivation chains.
    pub fn build(program: &Program) -> Self {
        let mut universe = TypeUniverse::new();

        // Phase 1: Collect all TypeDef declarations
        let mut type_defs: Vec<&TypeDef> = Vec::new();
        for item in &program.items {
            if let TopLevel::TypeDef(td) = item {
                type_defs.push(td.as_ref());
            }
        }

        // Resolve each TypeDef in order (support forward references?)
        // Current: single pass — requires base types to be declared first.
        // DEFERRED (D-1): Topological sort for forward references.
        for td in &type_defs {
            let resolved = universe.resolve_type_def(td);
            if let Some(resolved) = resolved {
                universe.types.insert(td.name.clone(), resolved.clone());
                universe.resolution_order.push(td.name.clone());
            }
        }

        universe
    }

    /// Resolve a single TypeDef against the existing universe.
    /// Returns None if the base type is not yet resolved.
    fn resolve_type_def(&self, td: &TypeDef) -> Option<ResolvedType> {
        let base_name = match td.base.as_ref() {
            Expr::TypeRef(name) => name.clone(),
            Expr::Identifier(name) => name.clone(),
            _ => return None,
        };

        // Start with defaults
        let mut rt = ResolvedType {
            name: td.name.clone(),
            type_params: td.type_params.clone(),
            base: base_name.clone(),
            bytes: 0,
            alignment: 1,
            endian: 0,
            volatile: false,
            atomic: false,
            element_type: None,
            fixed_size: None,
            insert_at: None,
            extract_from: None,
            allow_index: true,
            allow_slice: true,
            allow_arrow: true,
            codec: None,
            source: td.clone(),
        };

        // Inherit from base type if it exists in the universe
        // DEFERRED (D-1): full chain resolution
        if let Some(base) = self.types.get(&base_name) {
            rt.bytes = base.bytes;
            rt.alignment = base.alignment;
            rt.endian = base.endian;
            rt.volatile = base.volatile;
            rt.atomic = base.atomic;
            rt.allow_index = base.allow_index;
            rt.allow_slice = base.allow_slice;
            rt.allow_arrow = base.allow_arrow;
        }

        // Apply overrides from this TypeDef's property body
        // DEFERRED (D-7): Evaluate constraint expressions
        for prop in &td.body.properties {
            self.apply_property(&mut rt, prop);
        }

        Some(rt)
    }

    /// Apply a TypeProperty to a ResolvedType, overriding inherited value.
    fn apply_property(&self, rt: &mut ResolvedType, prop: &TypeProperty) {
        match prop {
            TypeProperty::Bytes(e) => {
                if let Expr::Integer(n) = e.as_ref() {
                    rt.bytes = *n as u64;
                }
            }
            TypeProperty::Alignment(e) => {
                if let Expr::Integer(n) = e.as_ref() {
                    rt.alignment = *n as u64;
                }
            }
            TypeProperty::Endian(e) => {
                if let Expr::Identifier(name) = e.as_ref() {
                    rt.endian = if name == "Big" || name == "big" { 1 } else { 0 };
                }
            }
            TypeProperty::Volatile(e) => {
                if let Expr::Bool(b) = e.as_ref() {
                    rt.volatile = *b;
                }
            }
            TypeProperty::Atomic(e) => {
                if let Expr::Bool(b) = e.as_ref() {
                    rt.atomic = *b;
                }
            }
            TypeProperty::ElementType(e) => {
                if let Expr::TypeRef(name) = e.as_ref() {
                    rt.element_type = Some(name.clone());
                }
            }
            TypeProperty::FixedSize(e) => {
                if let Expr::Bool(b) = e.as_ref() {
                    rt.fixed_size = Some(*b);
                }
            }
            TypeProperty::InsertAt(e) => {
                rt.insert_at = type_universe_expr_to_string(e);
            }
            TypeProperty::ExtractFrom(e) => {
                rt.extract_from = type_universe_expr_to_string(e);
            }
            TypeProperty::AllowIndex(e) => {
                if let Expr::Bool(b) = e.as_ref() {
                    rt.allow_index = *b;
                }
            }
            TypeProperty::AllowSlice(e) => {
                if let Expr::Bool(b) = e.as_ref() {
                    rt.allow_slice = *b;
                }
            }
            TypeProperty::AllowArrow(e) => {
                if let Expr::Bool(b) = e.as_ref() {
                    rt.allow_arrow = *b;
                }
            }
            TypeProperty::Codec(name) => {
                rt.codec = Some(name.clone());
            }
        }
    }

    /// Look up a type by name.
    pub fn get(&self, name: &str) -> Option<&ResolvedType> {
        self.types.get(name)
    }

    /// Check if a type allows bracket indexing.
    pub fn allows_index(&self, name: &str) -> bool {
        self.types.get(name).map(|t| t.allow_index).unwrap_or(true)
    }

    /// Check if a type allows arrow mutation.
    pub fn allows_arrow(&self, name: &str) -> bool {
        self.types.get(name).map(|t| t.allow_arrow).unwrap_or(true)
    }
}

/// Convert a TypeDef expression to a display string for metadata storage.
/// DEFERRED (D-3, D-5): Full expression normalization.
fn type_universe_expr_to_string(e: &Expr) -> Option<String> {
    match e {
        Expr::Integer(n) => Some(n.to_string()),
        Expr::Identifier(name) => Some(name.clone()),
        Expr::Projection { source, target } => {
            let src = type_universe_expr_to_string(source).unwrap_or_default();
            let tgt = format!("{:?}", target);
            Some(format!("{} :> {}", src, tgt))
        }
        Expr::SubtypeProjection { ops, .. } => {
            // DEFERRED: serialize ops for heap strategy synthesis
            Some("<: { ... }".into())
        }
        _ => None,
    }
}

// ── Tests ───────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Comment, DispatchMode, Expr, StrictMode, TopLevel, TypeDef, TypeDefBody, TypeProperty, Program};

    fn make_program(items: Vec<TopLevel>) -> Program {
        Program {
            items,
            comments: vec![],
            reactor_speed: None,
            attrs: vec![],
            ffi: None,
            strict_mode: StrictMode::Off,
            dispatch_mode: DispatchMode::Sequential,
            exit_condition: None,
            out_pragmas: vec![],
            default_sig_modifier: None,
        }
    }

    fn make_u8_type_def() -> TypeDef {
        TypeDef {
            name: "U8".into(),
            type_params: vec![],
            base: Box::new(Expr::TypeRef("Bits".into())),
            body: TypeDefBody {
                properties: vec![
                    TypeProperty::Bytes(Box::new(Expr::Integer(1))),
                    TypeProperty::Alignment(Box::new(Expr::Integer(1))),
                ],
                bindings: vec![],
                constraints: vec![],
                span: None,
            },
            span: None,
        }
    }

    fn make_u32_type_def() -> TypeDef {
        TypeDef {
            name: "U32".into(),
            type_params: vec![],
            base: Box::new(Expr::TypeRef("Bits".into())),
            body: TypeDefBody {
                properties: vec![
                    TypeProperty::Bytes(Box::new(Expr::Integer(4))),
                    TypeProperty::Alignment(Box::new(Expr::Integer(4))),
                ],
                bindings: vec![],
                constraints: vec![],
                span: None,
            },
            span: None,
        }
    }

    #[test]
    fn test_resolve_single_typedef() {
        let program = make_program(vec![TopLevel::TypeDef(Box::new(make_u8_type_def()))]);
        let universe = TypeUniverse::build(&program);
        let u8 = universe.get("U8").expect("U8 should be resolved");
        assert_eq!(u8.bytes, 1);
        assert_eq!(u8.alignment, 1);
        assert_eq!(u8.allow_index, true);
    }

    #[test]
    fn test_resolve_multiple_typedefs() {
        let program = make_program(vec![
            TopLevel::TypeDef(Box::new(make_u8_type_def())),
            TopLevel::TypeDef(Box::new(make_u32_type_def())),
        ]);
        let universe = TypeUniverse::build(&program);
        let u8 = universe.get("U8").unwrap();
        let u32 = universe.get("U32").unwrap();
        assert_eq!(u8.bytes, 1);
        assert_eq!(u32.bytes, 4);
        assert_eq!(universe.resolution_order.len(), 2);
    }

    #[test]
    fn test_resolve_typedef_with_override() {
        let base = TypeDef {
            name: "BaseList".into(),
            type_params: vec!["T".into()],
            base: Box::new(Expr::TypeRef("Bits".into())),
            body: TypeDefBody {
                properties: vec![
                    TypeProperty::Bytes(Box::new(Expr::Integer(8))),
                    TypeProperty::ElementType(Box::new(Expr::TypeRef("T".into()))),
                    TypeProperty::AllowIndex(Box::new(Expr::Bool(true))),
                ],
                bindings: vec![],
                constraints: vec![],
                span: None,
            },
            span: None,
        };
        let derived = TypeDef {
            name: "Stack".into(),
            type_params: vec!["T".into()],
            base: Box::new(Expr::TypeRef("BaseList".into())),
            body: TypeDefBody {
                properties: vec![
                    TypeProperty::AllowIndex(Box::new(Expr::Bool(false))),
                ],
                bindings: vec![],
                constraints: vec![],
                span: None,
            },
            span: None,
        };
        let program = make_program(vec![
            TopLevel::TypeDef(Box::new(base)),
            TopLevel::TypeDef(Box::new(derived)),
        ]);
        let universe = TypeUniverse::build(&program);
        let stack = universe.get("Stack").expect("Stack should be resolved");
        assert_eq!(stack.allow_index, false, "Stack should block indexing");
        // AllowSlice and AllowArrow should inherit from BaseList
        assert_eq!(stack.allow_slice, true);
        assert_eq!(stack.allow_arrow, true);
    }

    #[test]
    fn test_allows_index_gate() {
        let disable_index = TypeDef {
            name: "NoIndex".into(),
            type_params: vec![],
            base: Box::new(Expr::TypeRef("Bits".into())),
            body: TypeDefBody {
                properties: vec![TypeProperty::AllowIndex(Box::new(Expr::Bool(false)))],
                bindings: vec![],
                constraints: vec![],
                span: None,
            },
            span: None,
        };
        let program = make_program(vec![TopLevel::TypeDef(Box::new(disable_index))]);
        let universe = TypeUniverse::build(&program);
        assert!(!universe.allows_index("NoIndex"));
        assert_eq!(universe.allows_arrow("NoIndex"), true);
    }

    #[test]
    fn test_volatile_atomic_flags() {
        let mmio = TypeDef {
            name: "MmioReg".into(),
            type_params: vec![],
            base: Box::new(Expr::TypeRef("U32".into())),
            body: TypeDefBody {
                properties: vec![
                    TypeProperty::Volatile(Box::new(Expr::Bool(true))),
                ],
                bindings: vec![],
                constraints: vec![],
                span: None,
            },
            span: None,
        };
        let program = make_program(vec![
            TopLevel::TypeDef(Box::new(make_u32_type_def())),
            TopLevel::TypeDef(Box::new(mmio)),
        ]);
        let universe = TypeUniverse::build(&program);
        let reg = universe.get("MmioReg").unwrap();
        assert!(reg.volatile);
        assert!(!reg.atomic);
    }

    #[test]
    fn test_codec_typedef() {
        let string_def = TypeDef {
            name: "String".into(),
            type_params: vec![],
            base: Box::new(Expr::TypeRef("List".into())),
            body: TypeDefBody {
                properties: vec![TypeProperty::Codec("Utf8".into())],
                bindings: vec![],
                constraints: vec![],
                span: None,
            },
            span: None,
        };
        let program = make_program(vec![TopLevel::TypeDef(Box::new(string_def))]);
        let universe = TypeUniverse::build(&program);
        let s = universe.get("String").unwrap();
        assert_eq!(s.codec, Some("Utf8".into()));
    }

    #[test]
    fn test_endian_typedef() {
        let big_endian = TypeDef {
            name: "BeInt".into(),
            type_params: vec![],
            base: Box::new(Expr::TypeRef("Bits".into())),
            body: TypeDefBody {
                properties: vec![
                    TypeProperty::Bytes(Box::new(Expr::Integer(4))),
                    TypeProperty::Endian(Box::new(Expr::Identifier("Big".into()))),
                ],
                bindings: vec![],
                constraints: vec![],
                span: None,
            },
            span: None,
        };
        let program = make_program(vec![TopLevel::TypeDef(Box::new(big_endian))]);
        let universe = TypeUniverse::build(&program);
        assert_eq!(universe.get("BeInt").unwrap().endian, 1);
    }
}
