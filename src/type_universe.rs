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

    // ── Phase 7A: LLVM-specific codegen properties ─────────────
    //
    // 2026-06-29: These replace the hardcoded match arms in the LLVM
    // backend. Populated for built-in types at universe-build time;
    // inherited/overridden for user-defined types.
    //
    // See .opencode/plans/2026-06-29-type-system-refactoring.md

    /// LLVM IR type string for register values (e.g. "float", "i64", "i8*").
    pub llvm_type: String,
    /// How this type is stored in %State: "Native" or "Boxed".
    /// Native = lives in its own registers (e.g., float in float regs).
    /// Boxed = stored as i64 (e.g., Int, Bool, Char).
    pub storage: String,
    /// TBAA type tree node name (e.g. "Int", "Float", "Bool", "Char", "String").
    pub tbaa_node: String,
    /// Intrinsic name for boxing native→i64 (None = identity, already i64).
    pub box_op: Option<String>,
    /// Intrinsic name for unboxing i64→native (None = identity, already i64).
    pub unbox_op: Option<String>,

    // ── End Phase 7A properties ─────────────────────────────────

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
    /// Operator→intrinsic mappings. Key is (rune, optional param type name).
    /// 2026-06-29: Phase 7B — user-facing operator declarations.
    pub operators: std::collections::HashMap<(crate::ast::OpRune, Option<String>), crate::ast::OpDeclaration>,
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
    /// Warnings from meld validation (e.g., circular meld cycles).
    pub meld_warnings: Vec<String>,
}

/// Known codec names for D-2 validation.
const KNOWN_CODECS: &[&str] = &["Utf8", "Utf16", "Big5", "ShiftJIS", "EucJP", "Binary"];

/// Strategy for inserting into a collection.
/// Maps from InsertAt binding strings to dispatch logic.
#[derive(Debug, Clone, PartialEq)]
pub enum InsertStrategy {
    /// Append to end (List::push / queue::push_back). Default for List.
    Append,
    /// Insert at front (List::unshift / queue::push_front).
    Prepend,
    /// Binary search + insert at correct position (sorted list).
    Sorted,
    /// Hash-based insert (HashMap insert).
    Hash,
    /// User-defined function name for insert. The TypeDef's InsertAt binding
    /// resolved to a string that doesn't match any built-in strategy.
    /// `<-` dispatch calls this function with (collection, value).
    Custom(String),
}

/// Strategy for extracting from a collection.
/// Maps from ExtractFrom binding strings to dispatch logic.
#[derive(Debug, Clone, PartialEq)]
pub enum ExtractStrategy {
    /// Pop from end (List::pop / stack::pop). Default for List/Stack.
    Pop,
    /// Remove from front (List::shift / queue::pop_front). Default for Queue.
    Shift,
    /// Hash-based extract (HashMap remove).
    Hash,
    /// User-defined function name for extract. The TypeDef's ExtractFrom
    /// binding resolved to a string that doesn't match any built-in strategy.
    /// `<-` dispatch calls this function with (collection).
    Custom(String),
}

impl TypeUniverse {
    pub fn new() -> Self {
        TypeUniverse {
            types: HashMap::new(),
            resolution_order: Vec::new(),
            melds: HashMap::new(),
            meld_warnings: Vec::new(),
        }
    }

    // ── Phase 7A: Built-in primitive type table ─────────────────
    //
    // 2026-06-29: Populates the universe with built-in primitive types
    // (Int, Float, Float64, Bool, Char, String, Data, etc.). These types
    // get their LLVM codegen properties from this table instead of from
    // hardcoded match arms in the backend.
    //
    // When a user creates `type MyFloat <: Float { ... }`, the inheritance
    // chain copies these properties, then applies overrides.
    //
    // See .opencode/plans/2026-06-29-type-system-refactoring.md

    /// Register all built-in primitive types in the universe.
    /// Called at the start of `build()` before processing user TypeDefs.
    fn init_primitives(&mut self) {
        let primitives: Vec<ResolvedType> = vec![
            ResolvedType {
                name: "Int".into(), base: "Bits".into(),
                bytes: 8, alignment: 8,
                llvm_type: "i64".into(), storage: "Boxed".into(),
                tbaa_node: "Int".into(), box_op: None, unbox_op: None,
                ..Self::default_primitive()
            },
            ResolvedType {
                name: "UInt".into(), base: "Bits".into(),
                bytes: 8, alignment: 8,
                llvm_type: "i64".into(), storage: "Boxed".into(),
                tbaa_node: "Int".into(), box_op: None, unbox_op: None,
                ..Self::default_primitive()
            },
            ResolvedType {
                name: "Int8".into(), base: "Bits".into(),
                bytes: 1, alignment: 1,
                llvm_type: "i8".into(), storage: "Boxed".into(),
                tbaa_node: "Int".into(),
                box_op: Some("sext.i8.to.i64#".into()),
                unbox_op: Some("trunc.i64.to.i8#".into()),
                ..Self::default_primitive()
            },
            ResolvedType {
                name: "UInt8".into(), base: "Bits".into(),
                bytes: 1, alignment: 1,
                llvm_type: "i8".into(), storage: "Boxed".into(),
                tbaa_node: "Int".into(),
                box_op: Some("zext.i8.to.i64#".into()),
                unbox_op: Some("trunc.i64.to.i8#".into()),
                ..Self::default_primitive()
            },
            ResolvedType {
                name: "Int16".into(), base: "Bits".into(),
                bytes: 2, alignment: 2,
                llvm_type: "i16".into(), storage: "Boxed".into(),
                tbaa_node: "Int".into(),
                box_op: Some("sext.i16.to.i64#".into()),
                unbox_op: Some("trunc.i64.to.i16#".into()),
                ..Self::default_primitive()
            },
            ResolvedType {
                name: "UInt16".into(), base: "Bits".into(),
                bytes: 2, alignment: 2,
                llvm_type: "i16".into(), storage: "Boxed".into(),
                tbaa_node: "Int".into(),
                box_op: Some("zext.i16.to.i64#".into()),
                unbox_op: Some("trunc.i64.to.i16#".into()),
                ..Self::default_primitive()
            },
            ResolvedType {
                name: "Int32".into(), base: "Bits".into(),
                bytes: 4, alignment: 4,
                llvm_type: "i32".into(), storage: "Boxed".into(),
                tbaa_node: "Int".into(),
                box_op: Some("sext.i32.to.i64#".into()),
                unbox_op: Some("trunc.i64.to.i32#".into()),
                ..Self::default_primitive()
            },
            ResolvedType {
                name: "UInt32".into(), base: "Bits".into(),
                bytes: 4, alignment: 4,
                llvm_type: "i32".into(), storage: "Boxed".into(),
                tbaa_node: "Int".into(),
                box_op: Some("zext.i32.to.i64#".into()),
                unbox_op: Some("trunc.i64.to.i32#".into()),
                ..Self::default_primitive()
            },
            ResolvedType {
                name: "Float".into(), base: "Bits".into(),
                bytes: 4, alignment: 4,
                llvm_type: "float".into(), storage: "Native".into(),
                tbaa_node: "Float".into(),
                box_op: Some("bitcast.f32.to.i64#".into()),
                unbox_op: Some("bitcast.i64.to.f32#".into()),
                ..Self::default_primitive()
            },
            ResolvedType {
                name: "Float64".into(), base: "Bits".into(),
                bytes: 8, alignment: 8,
                llvm_type: "double".into(), storage: "Native".into(),
                tbaa_node: "Float".into(),
                box_op: Some("bitcast.f64.to.i64#".into()),
                unbox_op: Some("bitcast.i64.to.f64#".into()),
                ..Self::default_primitive()
            },
            ResolvedType {
                name: "Bool".into(), base: "Bits".into(),
                bytes: 1, alignment: 1,
                llvm_type: "i8".into(), storage: "Boxed".into(),
                tbaa_node: "Bool".into(),
                box_op: Some("zext.i1.to.i64#".into()),
                unbox_op: Some("trunc.i64.to.i1#".into()),
                ..Self::default_primitive()
            },
            ResolvedType {
                name: "Char".into(), base: "Bits".into(),
                bytes: 4, alignment: 4,
                llvm_type: "i32".into(), storage: "Boxed".into(),
                tbaa_node: "Char".into(),
                box_op: None,  // already i64 in state
                unbox_op: None,
                ..Self::default_primitive()
            },
            ResolvedType {
                name: "String".into(), base: "Bits".into(),
                bytes: 8, alignment: 8,
                llvm_type: "i8*".into(), storage: "Boxed".into(),
                tbaa_node: "String".into(),
                box_op: Some("ptrtoint#".into()),
                unbox_op: Some("inttoptr#".into()),
                ..Self::default_primitive()
            },
            ResolvedType {
                name: "Data".into(), base: "Bits".into(),
                bytes: 8, alignment: 8,
                llvm_type: "i8*".into(), storage: "Boxed".into(),
                tbaa_node: "String".into(),
                box_op: Some("ptrtoint#".into()),
                unbox_op: Some("inttoptr#".into()),
                ..Self::default_primitive()
            },
        ];

        for p in primitives {
            let name = p.name.clone();
            self.types.insert(name.clone(), p);
            self.resolution_order.push(name);
        }
    }

    /// Default values for primitive type initialization.
    /// Uses the `..` struct update syntax for ResolvedType.
    fn default_primitive() -> ResolvedType {
        ResolvedType {
            name: String::new(),
            type_params: vec![],
            base: String::new(),
            bytes: 0,
            alignment: 1,
            llvm_type: "i64".into(),
            storage: "Boxed".into(),
            tbaa_node: "Int".into(),
            box_op: None,
            unbox_op: None,
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
            operators: std::collections::HashMap::new(),
            projections: HashMap::new(),
            source: crate::ast::TypeDef {
                name: String::new(),
                type_params: vec![],
                base: Box::new(Expr::TypeRef("Bits".into())),
                bit_range: None,
                body: crate::ast::TypeDefBody {
                    bindings: vec![],
                    operators: vec![],
                    constraints: vec![],
                    span: None,
                },
                span: None,
            },
        }
    }

    /// Build the type universe from a program's TopLevel items.
    /// Collects TypeDef declarations, resolves derivation chains.
    pub fn build(program: &Program) -> Self {
        let mut universe = TypeUniverse::new();

        // Phase 0: Register built-in primitive types
        universe.init_primitives();
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
                // E002: Detect cycles — check if a path already exists between the types
                if universe.has_meld_path(&meld.name_a, &meld.name_b) {
                    universe.meld_warnings.push(format!(
                        "warning[E002]: circular meld — `{}` and `{}` are already connected through other melds",
                        meld.name_a, meld.name_b,
                    ));
                }
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
            llvm_type: "i64".to_string(),
            storage: "Boxed".to_string(),
            tbaa_node: "Int".to_string(),
            box_op: None,
            unbox_op: None,
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
            operators: std::collections::HashMap::new(),
            projections: HashMap::new(),
            source: td.clone(),
        };

        // Inherit from base type if it exists in the universe
        // DEFERRED (D-1): full chain resolution
        if let Some(base) = self.types.get(&base_name) {
            rt.bytes = base.bytes;
            rt.alignment = base.alignment;
            rt.llvm_type = base.llvm_type.clone();
            rt.storage = base.storage.clone();
            rt.tbaa_node = base.tbaa_node.clone();
            rt.box_op = base.box_op.clone();
            rt.unbox_op = base.unbox_op.clone();
            rt.endian = base.endian;
            rt.volatile = base.volatile;
            rt.atomic = base.atomic;
            rt.allow_index = base.allow_index;
            rt.allow_slice = base.allow_slice;
            rt.allow_arrow = base.allow_arrow;
            rt.on_exit = base.on_exit.clone();
            // Inherit operators from base type
            rt.operators = base.operators.clone();
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

        // ── Phase 7B: Resolve operator declarations ────────────
        //
        // 2026-06-29: Each operator declaration maps a rune (+ param type)
        // to an implementation expression (intrinsic call, defn, or inop).
        // Operators are stored keyed by (OpRune, Option<param_type_name>)
        // for quick lookup during type-checking.
        for op_decl in &td.body.operators {
            let param_name = op_decl.param_type.as_ref()
                .and_then(|e| match e.as_ref() {
                    crate::ast::Expr::TypeRef(name) => Some(name.clone()),
                    crate::ast::Expr::Identifier(name) => Some(name.clone()),
                    _ => None,
                });
            let key = (op_decl.rune, param_name);
            rt.operators.insert(key, op_decl.clone());
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

    /// Look up a Type by its universe key (canonical name).
    /// Convenience wrapper around `get(ty.universe_key())`.
    /// 2026-06-29: Added for Phase 7A backend match arm replacement.
    pub fn get_by_type(&self, ty: &crate::ast::Type) -> Option<&ResolvedType> {
        self.get(ty.universe_key())
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

    /// Check if a path exists between types `a` and `b` in the meld graph.
    /// Used for cycle detection (E002). BFS over the undirected meld graph.
    pub fn has_meld_path(&self, a: &str, b: &str) -> bool {
        // Build adjacency from existing melds
        let mut adj: std::collections::HashMap<&str, Vec<&str>> = std::collections::HashMap::new();
        for (key, _decl) in &self.melds {
            adj.entry(key.0.as_str()).or_default().push(key.1.as_str());
            adj.entry(key.1.as_str()).or_default().push(key.0.as_str());
        }
        // BFS from a to b
        let mut visited: std::collections::HashSet<&str> = std::collections::HashSet::new();
        let mut queue: std::collections::VecDeque<&str> = std::collections::VecDeque::new();
        visited.insert(a);
        queue.push_back(a);
        while let Some(node) = queue.pop_front() {
            if node == b {
                return true;
            }
            if let Some(neighbors) = adj.get(node) {
                for n in neighbors {
                    if visited.insert(n) {
                        queue.push_back(n);
                    }
                }
            }
        }
        false
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
            _ => Some(InsertStrategy::Custom(strat.clone())),
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
            _ => Some(ExtractStrategy::Custom(strat.clone())),
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

    // ── Phase 7B: Operator Resolution ─────────────────────────
    //
    // 2026-06-29: Resolves operator calls to their implementation.
    // Handles both exact type matches and cross-type composition.

    /// Resolve an operator call `type_name . rune(param_type_name)` to its
    /// implementation expression. Returns the OpDeclaration if found.
    /// Tries exact match first, then cross-type composition via conversion.
    pub fn resolve_operator(
        &self,
        type_name: &str,
        rune: crate::ast::OpRune,
        param_type_name: Option<&str>,
    ) -> Option<&crate::ast::OpDeclaration> {
        let rt = self.types.get(type_name)?;

        // 1. Exact match: look up (rune, Some(param_type))
        if let Some(param) = param_type_name {
            if let Some(op) = rt.operators.get(&(rune, Some(param.to_string()))) {
                return Some(op);
            }
        }

        // 2. Look up (rune, None) — unary operator (no param)
        if let Some(op) = rt.operators.get(&(rune, None)) {
            return Some(op);
        }

        // 3. Cross-type composition: param type differs from declared param
        // Check if there's a base operator and try to compose via conversion
        if let Some(param) = param_type_name {
            if let Some(base_op) = rt.operators.iter().find(|((r, _), _)| *r == rune) {
                let (_key, op) = base_op;
                // If there's a conversion path from param_type to the operator's
                // expected parameter type, the composition is valid in principle.
                let expected_param = op.param_type.as_ref()
                    .and_then(|e| match e.as_ref() {
                        crate::ast::Expr::TypeRef(n) => Some(n.as_str()),
                        _ => None,
                    });
                if let Some(expected) = expected_param {
                    if param != expected && self.has_conversion_path(param, expected) {
                        return Some(op);
                    }
                }
            }
        }

        None
    }

    /// Check if a conversion path exists from source type to target type.
    /// A conversion exists if:
    /// 1. Source == Target (same type)
    /// 2. Direct meld between types
    /// 3. Source can unbox to i64 AND target can box from i64 (round trip)
    /// 4. Source is already i64 (box_op = None = identity) — trivially converts
    /// 5. Target is already i64 (unbox_op = None = identity) — trivially converts
    /// 6. Base type inheritance chain
    fn has_conversion_path(&self, source: &str, target: &str) -> bool {
        // 1. Same type — trivially convertible
        if source == target {
            return true;
        }

        // 2. Direct meld between source and target
        if self.find_meld(source, target).is_some() {
            return true;
        }

        // 3-5. Check via i64 round-trip
        if let (Some(src_rt), Some(tgt_rt)) = (self.types.get(source), self.types.get(target)) {
            // Source can produce i64 (either via box_op or because it IS i64)
            let src_to_i64 = src_rt.box_op.is_some() || src_rt.llvm_type == "i64";
            // Target can receive i64 (either via unbox_op or because it IS i64)
            let tgt_from_i64 = tgt_rt.unbox_op.is_some() || tgt_rt.llvm_type == "i64";
            if src_to_i64 && tgt_from_i64 {
                return true;
            }
        }

        // 6. Base type inheritance chain
        if let Some(src_rt) = self.types.get(source) {
            if src_rt.base == target || self.types.get(&src_rt.base).map(|b| b.base.as_str() == target).unwrap_or(false) {
                return true;
            }
        }
        if let Some(tgt_rt) = self.types.get(target) {
            if tgt_rt.base == source {
                return true;
            }
        }

        false
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
    use crate::ast::{Comment, DispatchMode, Expr, MeldDeclaration, OpDeclaration, OpRune, StrictMode, TopLevel, TypeBinding, TypeDef, TypeDefBody, Program};

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
                watchdog_defaults: (None, None),
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
                operators: vec![], constraints: vec![],
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
                operators: vec![], constraints: vec![],
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
        assert!(universe.resolution_order.len() >= 14,
            "Should include built-in primitives + user types, got {}",
            universe.resolution_order.len());
        // Last two should be our user-defined types (registered after primitives)
        assert!(universe.get("U8").is_some());
        assert!(universe.get("U32").is_some());
        assert!(universe.get("Int").is_some(), "Built-in Int should exist");
        assert!(universe.get("Float").is_some(), "Built-in Float should exist");
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
                operators: vec![], constraints: vec![],
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
                operators: vec![], constraints: vec![],
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
                operators: vec![], constraints: vec![],
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
                operators: vec![], constraints: vec![],
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
                operators: vec![], constraints: vec![],
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
                operators: vec![], constraints: vec![],
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
                operators: vec![], constraints: vec![],
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
                operators: vec![],
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
                operators: vec![], constraints: vec![],
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
                operators: vec![], constraints: vec![],
                span: None,
            },
            span: None,
        };
        let program = make_program(vec![TopLevel::TypeDef(Box::new(td))]);
        let universe = TypeUniverse::build(&program);
        assert_eq!(universe.insert_strategy("Custom"), Some(InsertStrategy::Custom("custom_strat".into())));
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

    // ── Phase 7B: Operator Resolution Tests ───────────────────
    #[test]
    fn test_resolve_operator_add() {
        let td = TypeDef {
            name: "MyFloat".into(),
            type_params: vec![],
            bit_range: None,
            base: Box::new(Expr::TypeRef("Bits".into())),
            body: TypeDefBody {
                bindings: vec![
                    TypeBinding { name: "Bytes".into(), params: vec![], value: Box::new(Expr::Integer(4)), span: None },
                ],
                operators: vec![
                    OpDeclaration {
                        rune: OpRune::Add,
                        param_type: Some(Box::new(Expr::TypeRef("MyFloat".into()))),
                        return_type: Box::new(Expr::TypeRef("MyFloat".into())),
                        implementation: Box::new(Expr::Identifier("my_add".into())),
                        span: None,
                    },
                ],
                constraints: vec![],
                span: None,
            },
            span: None,
        };
        let program = make_program(vec![TopLevel::TypeDef(Box::new(td))]);
        let universe = TypeUniverse::build(&program);
        let op = universe.resolve_operator("MyFloat", OpRune::Add, Some("MyFloat"));
        assert!(op.is_some(), "Should resolve Add(MyFloat) -> MyFloat");
        assert_eq!(op.unwrap().rune, OpRune::Add);
    }

    #[test]
    fn test_resolve_operator_no_match() {
        let td = TypeDef {
            name: "MyInt".into(),
            type_params: vec![],
            bit_range: None,
            base: Box::new(Expr::TypeRef("Bits".into())),
            body: TypeDefBody {
                bindings: vec![
                    TypeBinding { name: "Bytes".into(), params: vec![], value: Box::new(Expr::Integer(8)), span: None },
                ],
                operators: vec![],
                constraints: vec![],
                span: None,
            },
            span: None,
        };
        let program = make_program(vec![TopLevel::TypeDef(Box::new(td))]);
        let universe = TypeUniverse::build(&program);
        let op = universe.resolve_operator("MyInt", OpRune::Mul, Some("MyInt"));
        assert!(op.is_none(), "Should not resolve undeclared operator");
    }

    #[test]
    fn test_has_conversion_path_same_type() {
        let td = TypeDef {
            name: "T".into(),
            type_params: vec![],
            bit_range: None,
            base: Box::new(Expr::TypeRef("Bits".into())),
            body: TypeDefBody {
                bindings: vec![
                    TypeBinding { name: "Bytes".into(), params: vec![], value: Box::new(Expr::Integer(4)), span: None },
                ],
                operators: vec![],
                constraints: vec![],
                span: None,
            },
            span: None,
        };
        let program = make_program(vec![TopLevel::TypeDef(Box::new(td))]);
        let universe = TypeUniverse::build(&program);
        assert!(universe.has_conversion_path("T", "T"), "Same type should always be convertible");
    }

    #[test]
    fn test_has_conversion_path_via_box_unbox() {
        // Float has box_op and unbox_op — it should be convertible to/from Int
        let program = make_program(vec![]);
        let universe = TypeUniverse::build(&program);
        // Both Float and Int have box/unbox, so conversion via i64 is possible
        assert!(universe.has_conversion_path("Float", "Int"),
            "Float should have conversion path via i64 (box→unbox)");
        assert!(universe.has_conversion_path("Int", "Float"),
            "Int should have conversion path via i64 (box→unbox)");
    }
}
