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

use crate::ast::{Expr, MeldDeclaration, Program, TopLevel, TypeBinding, TypeDef, TypeDefBody};

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
    /// User-defined projections not matching any known metadata property name.
    pub projections: HashMap<String, TypeBinding>,
    /// Optional foreign destructor function name. When a value of this type
    /// goes out of scope, the backend emits a call to this function with
    /// the value's pointer. Used for FFI types with ownership semantics
    /// (e.g., Rust Vec, C++ std::vector). Example:
    ///   OnExit = __rust_vec_drop#;
    pub on_exit: Option<String>,
    /// Runtime guard expressions synthesized from TypeDef constraints.
    /// Each guard is an Expr that must evaluate to true. If any guard
    /// fails at runtime, the program traps.
    pub guards: Vec<crate::ast::Expr>,
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
    /// Meld declarations indexed by sorted (name_a, name_b) pair.
    /// Casts `a as B` are valid only if a direct meld exists between A and B.
    pub melds: HashMap<(String, String), MeldDeclaration>,
}

/// Known codec names for D-2 validation.
const KNOWN_CODECS: &[&str] = &["Utf8", "Utf16", "Big5", "ShiftJIS", "EucJP", "Binary"];

/// Strategy for inserting into a collection.
/// Maps from InsertAt binding strings to dispatch logic.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InsertStrategy {
    /// Append to end (List::push / queue::push_back). Default for List.
    Append,
    /// Insert at front (List::unshift / queue::push_front).
    Prepend,
    /// Binary search + insert at correct position (sorted list).
    Sorted,
    /// Hash-based insert (HashMap insert).
    Hash,
}

/// Strategy for extracting from a collection.
/// Maps from ExtractFrom binding strings to dispatch logic.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ExtractStrategy {
    /// Pop from end (List::pop / stack::pop). Default for List/Stack.
    Pop,
    /// Remove from front (List::shift / queue::pop_front). Default for Queue.
    Shift,
    /// Hash-based extract (HashMap remove).
    Hash,
}

impl TypeUniverse {
    pub fn new() -> Self {
        TypeUniverse {
            types: HashMap::new(),
            resolution_order: Vec::new(),
            melds: HashMap::new(),
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

        // Phase 2: Topological sort — resolve base types before derived types
        // Build a dependency graph: which type names does each TypeDef depend on?
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        let mut edges: HashMap<String, Vec<String>> = HashMap::new();
        let mut name_to_td: HashMap<String, &TypeDef> = HashMap::new();

        for td in &type_defs {
            in_degree.entry(td.name.clone()).or_insert(0);
            name_to_td.entry(td.name.clone()).or_insert(td);
        }

        for td in &type_defs {
            let base_name = match td.base.as_ref() {
                Expr::TypeRef(name) => name.as_str(),
                Expr::Identifier(name) => name.as_str(),
                _ => continue,
            };
            if name_to_td.contains_key(&td.name) && name_to_td.contains_key(base_name) {
                edges.entry(base_name.to_string()).or_default().push(td.name.clone());
                *in_degree.entry(td.name.clone()).or_insert(0) += 1;
            }
        }

        let mut queue: Vec<String> = in_degree.iter()
            .filter(|(_, deg)| **deg == 0)
            .map(|(name, _)| name.clone())
            .collect();
        let mut sorted: Vec<&TypeDef> = Vec::new();

        while let Some(name) = queue.pop() {
            if let Some(td) = name_to_td.get(&name) {
                sorted.push(td);
            }
            if let Some(deps) = edges.get(&name) {
                for dep in deps {
                    let deg = in_degree.get_mut(dep).unwrap();
                    *deg -= 1;
                    if *deg == 0 {
                        queue.push(dep.clone());
                    }
                }
            }
        }

        for td in &type_defs {
            if !sorted.iter().any(|s| s.name == td.name) {
                sorted.push(td);
            }
        }

        // Resolve in topological order
        for td in &sorted {
            let resolved = universe.resolve_type_def(td);
            if let Some(resolved) = resolved {
                universe.types.insert(td.name.clone(), resolved.clone());
                universe.resolution_order.push(td.name.clone());
            }
        }

        // Phase 3: Collect meld declarations
        for item in &program.items {
            if let TopLevel::Meld(meld) = item {
                let key = if meld.name_a <= meld.name_b {
                    (meld.name_a.clone(), meld.name_b.clone())
                } else {
                    (meld.name_b.clone(), meld.name_a.clone())
                };
                universe.melds.insert(key, meld.clone());
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
            on_exit: None,
            guards: vec![],
            projections: HashMap::new(),
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
            rt.on_exit = base.on_exit.clone();
        }

        // Phase 4: Auto-compute Bytes from bit_range for `Bits @/lo..hi` syntax
        if let Some(ref br) = td.bit_range {
            let bits = match br {
                crate::ast::BitRange::Single(_) => 1,
                crate::ast::BitRange::Range(lo, hi) => hi - lo + 1,
                crate::ast::BitRange::Any(w) => *w,
            };
            let bytes = if bits == 0 { 0 }
                        else { (bits + 7) / 8 };
            rt.bytes = bytes as u64;
            rt.fixed_size = Some(true);
        }

        // Apply bindings — known metadata names populate ResolvedType fields,
        // unknown names are stored as user-defined projections
        // DEFERRED (D-7): Evaluate constraint expressions
        for binding in &td.body.bindings {
            self.apply_binding(&mut rt, binding);
        }

        // D-7: Synthesize runtime guard expressions from constraints
        // Each constraint expression is stored as a guard that must pass
        // when a value of this type is constructed or assigned.
        for constraint in &td.body.constraints {
            rt.guards.push(constraint.clone());
        }

        Some(rt)
    }

    /// Apply a TypeBinding to a ResolvedType.
    /// Known metadata property names override the corresponding field;
    /// unknown names are stored in the projections map.
    fn apply_binding(&self, rt: &mut ResolvedType, binding: &TypeBinding) {
        match binding.name.as_str() {
            "Bytes" => {
                if let Expr::Integer(n) = binding.value.as_ref() {
                    rt.bytes = *n as u64;
                }
            }
            "Alignment" => {
                if let Expr::Integer(n) = binding.value.as_ref() {
                    rt.alignment = *n as u64;
                }
            }
            "Endian" => {
                if let Expr::Identifier(name) = binding.value.as_ref() {
                    rt.endian = if name == "Big" || name == "big" { 1 } else { 0 };
                }
            }
            "Volatile" => {
                if let Expr::Bool(b) = binding.value.as_ref() {
                    rt.volatile = *b;
                }
            }
            "Atomic" => {
                if let Expr::Bool(b) = binding.value.as_ref() {
                    rt.atomic = *b;
                }
            }
            "ElementType" => {
                if let Expr::TypeRef(name) = binding.value.as_ref() {
                    rt.element_type = Some(name.clone());
                }
            }
            "FixedSize" => {
                if let Expr::Bool(b) = binding.value.as_ref() {
                    rt.fixed_size = Some(*b);
                }
            }
            "InsertAt" => {
                rt.insert_at = type_universe_expr_to_string(&binding.value);
            }
            "ExtractFrom" => {
                rt.extract_from = type_universe_expr_to_string(&binding.value);
            }
            "AllowIndex" => {
                if let Expr::Bool(b) = binding.value.as_ref() {
                    rt.allow_index = *b;
                }
            }
            "AllowSlice" => {
                if let Expr::Bool(b) = binding.value.as_ref() {
                    rt.allow_slice = *b;
                }
            }
            "AllowArrow" => {
                if let Expr::Bool(b) = binding.value.as_ref() {
                    rt.allow_arrow = *b;
                }
            }
            "Codec" => {
                match &binding.value.as_ref() {
                    Expr::String(s) => {
                        if !KNOWN_CODECS.contains(&s.as_str()) {
                            // Unknown codec — warn but accept (forward compat)
                        }
                        rt.codec = Some(s.clone());
                    }
                    Expr::Identifier(id) => {
                        if !KNOWN_CODECS.contains(&id.as_str()) {
                            // Unknown codec identifier — warn but accept
                        }
                        rt.codec = Some(id.clone());
                    }
                    _ => {}
                }
            }
            "OnExit" => {
                // Foreign destructor function: OnExit = __rust_vec_drop#;
                // The value is the function name (string or identifier).
                match &binding.value.as_ref() {
                    Expr::String(s) => rt.on_exit = Some(s.clone()),
                    Expr::Identifier(id) => rt.on_exit = Some(id.clone()),
                    // For Expr::IntrinsicCall like __rust_vec_drop#, extract name
                    Expr::IntrinsicCall { intrinsic, .. } => {
                        rt.on_exit = Some(intrinsic.name().to_string());
                    }
                    _ => {}
                }
            }
            // Unknown name → user-defined projection
            _ => {
                rt.projections.insert(binding.name.clone(), binding.clone());
            }
        }
    }

    /// Look up a type by name.
    pub fn get(&self, name: &str) -> Option<&ResolvedType> {
        self.types.get(name)
    }

    /// Check if a direct meld exists between types `a` and `b`.
    /// Transitive melds are NOT resolved — only explicit `meld A <:> B` declarations.
    /// Returns the MeldDeclaration if found.
    pub fn find_meld(&self, a: &str, b: &str) -> Option<&MeldDeclaration> {
        let key = if a <= b {
            (a.to_string(), b.to_string())
        } else {
            (b.to_string(), a.to_string())
        };
        self.melds.get(&key)
    }

    /// Resolve the InsertAt strategy string to a known InsertStrategy.
    /// Returns None if the type has no InsertAt binding or the strategy
    /// is unrecognized.
    pub fn insert_strategy(&self, type_name: &str) -> Option<InsertStrategy> {
        let rt = self.types.get(type_name)?;
        let strat = rt.insert_at.as_ref()?;
        match strat.as_str() {
            "append" => Some(InsertStrategy::Append),
            "prepend" => Some(InsertStrategy::Prepend),
            "sorted" => Some(InsertStrategy::Sorted),
            "hash" => Some(InsertStrategy::Hash),
            _ => None,
        }
    }

    /// Resolve the ExtractFrom strategy string to a known ExtractStrategy.
    pub fn extract_strategy(&self, type_name: &str) -> Option<ExtractStrategy> {
        let rt = self.types.get(type_name)?;
        let strat = rt.extract_from.as_ref()?;
        match strat.as_str() {
            "pop" => Some(ExtractStrategy::Pop),
            "shift" => Some(ExtractStrategy::Shift),
            "head" => Some(ExtractStrategy::Shift),
            "tail" => Some(ExtractStrategy::Pop),
            "hash" => Some(ExtractStrategy::Hash),
            _ => None,
        }
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
        Expr::SubtypeProjection { source, ops } => {
            let src = type_universe_expr_to_string(source).unwrap_or_default();
            let ops_str = ops.iter().map(|o| format!("{:?}", o)).collect::<Vec<_>>().join(", ");
            Some(format!("{} :< [{}]", src, ops_str))
        }
        _ => None,
    }
}

// ── Tests ───────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Comment, DispatchMode, Expr, MeldDeclaration, StrictMode, TopLevel, TypeBinding, TypeDef, TypeDefBody, Program};

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
            bit_range: None,
            base: Box::new(Expr::TypeRef("Bits".into())),
            body: TypeDefBody {
                bindings: vec![
                    TypeBinding { name: "Bytes".into(), params: vec![], value: Box::new(Expr::Integer(1)), span: None },
                    TypeBinding { name: "Alignment".into(), params: vec![], value: Box::new(Expr::Integer(1)), span: None },
                ],
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
            bit_range: None,
            base: Box::new(Expr::TypeRef("Bits".into())),
            body: TypeDefBody {
                bindings: vec![
                    TypeBinding { name: "Bytes".into(), params: vec![], value: Box::new(Expr::Integer(4)), span: None },
                    TypeBinding { name: "Alignment".into(), params: vec![], value: Box::new(Expr::Integer(4)), span: None },
                ],
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
            bit_range: None,
            base: Box::new(Expr::TypeRef("Bits".into())),
            body: TypeDefBody {
                bindings: vec![
                    TypeBinding { name: "Bytes".into(), params: vec![], value: Box::new(Expr::Integer(8)), span: None },
                    TypeBinding { name: "ElementType".into(), params: vec![], value: Box::new(Expr::TypeRef("T".into())), span: None },
                    TypeBinding { name: "AllowIndex".into(), params: vec![], value: Box::new(Expr::Bool(true)), span: None },
                ],
                constraints: vec![],
                span: None,
            },
            span: None,
        };
        let derived = TypeDef {
            name: "Stack".into(),
            type_params: vec!["T".into()],
            bit_range: None,
            base: Box::new(Expr::TypeRef("BaseList".into())),
            body: TypeDefBody {
                bindings: vec![
                    TypeBinding { name: "AllowIndex".into(), params: vec![], value: Box::new(Expr::Bool(false)), span: None },
                ],
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
            bit_range: None,
            base: Box::new(Expr::TypeRef("Bits".into())),
            body: TypeDefBody {
                bindings: vec![
                    TypeBinding { name: "AllowIndex".into(), params: vec![], value: Box::new(Expr::Bool(false)), span: None },
                ],
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
            bit_range: None,
            base: Box::new(Expr::TypeRef("U32".into())),
            body: TypeDefBody {
                bindings: vec![
                    TypeBinding { name: "Volatile".into(), params: vec![], value: Box::new(Expr::Bool(true)), span: None },
                ],
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
            bit_range: None,
            base: Box::new(Expr::TypeRef("List".into())),
            body: TypeDefBody {
                bindings: vec![
                    TypeBinding { name: "Codec".into(), params: vec![], value: Box::new(Expr::String("Utf8".into())), span: None },
                ],
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
            bit_range: None,
            base: Box::new(Expr::TypeRef("Bits".into())),
            body: TypeDefBody {
                bindings: vec![
                    TypeBinding { name: "Bytes".into(), params: vec![], value: Box::new(Expr::Integer(4)), span: None },
                    TypeBinding { name: "Endian".into(), params: vec![], value: Box::new(Expr::Identifier("Big".into())), span: None },
                ],
                constraints: vec![],
                span: None,
            },
            span: None,
        };
        let program = make_program(vec![TopLevel::TypeDef(Box::new(big_endian))]);
        let universe = TypeUniverse::build(&program);
        assert_eq!(universe.get("BeInt").unwrap().endian, 1);
    }

    #[test]
    fn test_codec_utf8_valid() {
        let td = TypeDef {
            name: "Utf8Str".into(),
            type_params: vec![],
            bit_range: None,
            base: Box::new(Expr::TypeRef("String".into())),
            body: TypeDefBody {
                bindings: vec![
                    TypeBinding { name: "Codec".into(), params: vec![], value: Box::new(Expr::String("Utf8".into())), span: None },
                ],
                constraints: vec![],
                span: None,
            },
            span: None,
        };
        let program = make_program(vec![TopLevel::TypeDef(Box::new(td))]);
        let universe = TypeUniverse::build(&program);
        assert_eq!(universe.get("Utf8Str").unwrap().codec, Some("Utf8".into()));
    }

    #[test]
    fn test_constraint_guard_synthesis() {
        // D-7: TypeDef constraints become runtime guards
        let td = TypeDef {
            name: "Positive".into(),
            type_params: vec![],
            bit_range: None,
            base: Box::new(Expr::TypeRef("Int".into())),
            body: TypeDefBody {
                bindings: vec![],
                constraints: vec![Expr::Gt(
                    Box::new(Expr::Identifier("_".into())),
                    Box::new(Expr::Integer(0)),
                )],
                span: None,
            },
            span: None,
        };
        let program = make_program(vec![TopLevel::TypeDef(Box::new(td))]);
        let universe = TypeUniverse::build(&program);
        assert_eq!(universe.get("Positive").unwrap().guards.len(), 1);
        assert!(matches!(&universe.get("Positive").unwrap().guards[0],
            Expr::Gt(e, _) if matches!(e.as_ref(), Expr::Identifier(name) if name == "_")));
    }

    #[test]
    fn test_insert_strategy_resolution() {
        let td = TypeDef {
            name: "Fifo".into(),
            type_params: vec![],
            bit_range: None,
            base: Box::new(Expr::TypeRef("List".into())),
            body: TypeDefBody {
                bindings: vec![
                    TypeBinding { name: "InsertAt".into(), params: vec![], value: Box::new(Expr::Identifier("append".into())), span: None },
                    TypeBinding { name: "ExtractFrom".into(), params: vec![], value: Box::new(Expr::Identifier("shift".into())), span: None },
                ],
                constraints: vec![],
                span: None,
            },
            span: None,
        };
        let program = make_program(vec![TopLevel::TypeDef(Box::new(td))]);
        let universe = TypeUniverse::build(&program);
        assert_eq!(universe.get("Fifo").unwrap().insert_at, Some("append".into()));
        assert_eq!(universe.get("Fifo").unwrap().extract_from, Some("shift".into()));
        assert_eq!(universe.insert_strategy("Fifo"), Some(InsertStrategy::Append));
        assert_eq!(universe.extract_strategy("Fifo"), Some(ExtractStrategy::Shift));
    }

    #[test]
    fn test_insert_strategy_resolution_unknown() {
        let td = TypeDef {
            name: "Custom".into(),
            type_params: vec![],
            bit_range: None,
            base: Box::new(Expr::TypeRef("List".into())),
            body: TypeDefBody {
                bindings: vec![
                    TypeBinding { name: "InsertAt".into(), params: vec![], value: Box::new(Expr::Identifier("custom_strat".into())), span: None },
                ],
                constraints: vec![],
                span: None,
            },
            span: None,
        };
        let program = make_program(vec![TopLevel::TypeDef(Box::new(td))]);
        let universe = TypeUniverse::build(&program);
        assert_eq!(universe.insert_strategy("Custom"), None);
    }

    #[test]
    fn test_meld_registration() {
        let meld = TopLevel::Meld(MeldDeclaration {
            name_a: "A".into(),
            name_b: "B".into(),
            routes: vec![],
            span: None,
        });
        let program = make_program(vec![meld]);
        let universe = TypeUniverse::build(&program);
        assert!(universe.find_meld("A", "B").is_some(), "meld A <:> B should be found");
        assert!(universe.find_meld("B", "A").is_some(), "meld B <:> A should also be found (bidirectional)");
        assert!(universe.find_meld("A", "C").is_none(), "meld A <:> C should NOT be found (no declaration)");
    }
}
