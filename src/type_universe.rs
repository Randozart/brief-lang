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

use crate::ast::{CodecDeclaration, Expr, MeldDeclaration, Program, TopLevel, TypeBinding, TypeDef, TypeDefBody};
use crate::features::binary_op::{BinaryOpExpr, BinaryOpKind};

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

    // 2026-07-03: Module that defined this type. Used for opaque handle
    // boundary enforcement — only the defining module may cast Ptr<Bits @/N>
    // to Ptr<ConcreteType>. Built-in types have defining_module = "builtin".
    pub defining_module: String,
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

    // ── Phase 2B: Expanded TypeUniverse fields ────────────────
    /// Default type parameter values (e.g., Int → Width(64)).
    pub default_params: Vec<(String, crate::ast::Type)>,
    /// Whether this type's arithmetic operators commute.
    pub commuting: bool,
    /// Whether operations on this type are constant-time.
    pub constant_time: bool,
    /// Struct layout for compound types (String, user-defined structs).
    pub struct_layout: Option<StructLayout>,

    /// Source TypeDef for reference.
    pub source: TypeDef,
    /// Generic property map populated from `<~` bindings during resolution.
    /// 2026-07-11: Phase 1B — dual-written alongside hardcoded fields during
    /// migration. After Phase 2, only this map remains.
    pub properties: HashMap<String, crate::ast::PropertyValue>,
}

impl ResolvedType {
    /// Get a string property from the generic properties map.
    /// 2026-07-11: Phase 1B.
    pub fn get_property_str(&self, key: &str) -> Option<&str> {
        self.properties.get(key).and_then(|v| {
            if let crate::ast::PropertyValue::String(s) = v { Some(s.as_str()) } else { None }
        })
    }

    /// Get an integer property from the generic properties map.
    /// 2026-07-11: Phase 1B.
    pub fn get_property_int(&self, key: &str) -> Option<i64> {
        self.properties.get(key).and_then(|v| {
            if let crate::ast::PropertyValue::Int(n) = v { Some(*n) } else { None }
        })
    }

    /// Get a bool property from the generic properties map.
    /// 2026-07-11: Phase 1B.
    pub fn get_property_bool(&self, key: &str) -> Option<bool> {
        self.properties.get(key).and_then(|v| {
            if let crate::ast::PropertyValue::Bool(b) = v { Some(*b) } else { None }
        })
    }

    /// Get a PropertyValue by key, or None.
    /// 2026-07-11: Phase 1B.
    pub fn get_property(&self, key: &str) -> Option<&crate::ast::PropertyValue> {
        self.properties.get(key)
    }

    /// Check if a property key exists.
    /// 2026-07-11: Phase 1B.
    pub fn has_property(&self, key: &str) -> bool {
        self.properties.contains_key(key)
    }
}

/// 2026-07-08: Phase 2B — struct layout for compound types.
#[derive(Debug, Clone)]
pub struct StructLayout {
    pub fields: Vec<StructField>,
    pub packed: bool,
    pub total_bytes: u64,
    pub alignment: u64,
}

/// 2026-07-08: Phase 2B — a single field in a struct layout.
#[derive(Debug, Clone)]
pub struct StructField {
    pub name: String,
    pub ty: crate::ast::Type,
    pub offset_bits: u64,
    pub size_bits: u64,
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
    /// Registered codec declarations. Phase 4 — codec system.
    /// Populated from `TopLevel::Codec` items during `build()`.
    pub codecs: HashMap<String, CodecDeclaration>,
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
            codecs: HashMap::new(),
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

    /// Register built-in primitive types from the bootstrap file.
    /// Uses Annotation Arrow (<~) syntax from lib/std/types/bootstrap.bv.
    fn init_primitives_from_bootstrap(&mut self) {
        let src = include_str!("../lib/std/types/bootstrap.bv");
        let mut parser = crate::parser::Parser::new(src);
        let program = match parser.parse() {
            Ok(p) => p,
            Err(e) => {
                eprintln!("Warning: failed to parse bootstrap type universe: {}", e);
                return;
            }
        };
        for item in &program.items {
            if let crate::ast::TopLevel::TypeDef(td) = item {
                if let Some(resolved) = self.resolve_type_def(&td) {
                    let name = resolved.name.clone();
                    let mut resolved = resolved;
                    resolved.defining_module = "builtin".to_string();
                    self.types.insert(name.clone(), resolved);
                    self.resolution_order.push(name);
                }
            }
        }
    }

    // ── Post-Bootstrap Validation ─────────────────────────────────
    //
    // 2026-07-01: After loading the bootstrap type universe, validate
    // that all built-in types have correct properties. This catches
    // silent binding failures (e.g., apply_binding using old-style Expr
    // patterns while the parser produces new-style Literal-packed Expr)
    // that would otherwise produce wrong LLVM types in the compiled IR.
    //
    // Each check verifies that the bootstrap file's annotations were
    // correctly applied. If a binding silently failed, the type would
    // retain its default value (e.g., llvm_type="i64" instead of "i32"
    // for Char), causing invalid LLVM IR in the backend.
    //
    fn validate_primitives(&self) {
        // Known built-in type names and their expected properties after
        // bootstrap loading. Each entry: (type_name, property, expected_value).
        let checks: &[(&str, &str, &str)] = &[
            ("Int",     "llvm",    "i64"),
            ("Int",     "storage", "Boxed"),
            ("Int",     "bytes",   "8"),
            ("Int",     "tbaa",    "Int"),
            ("UInt",    "llvm",    "i64"),
            ("UInt",    "storage", "Boxed"),
            ("Int8",    "llvm",    "i8"),
            ("Int8",    "storage", "Boxed"),
            ("Int8",    "bytes",   "1"),
            ("Int8",    "box",     "sext.i8.to.i64#"),
            ("Int8",    "unbox",   "trunc.i64.to.i8#"),
            ("UInt8",   "llvm",    "i8"),
            ("UInt8",   "box",     "zext.i8.to.i64#"),
            ("UInt8",   "unbox",   "trunc.i64.to.i8#"),
            ("Int16",   "llvm",    "i16"),
            ("Int16",   "box",     "sext.i16.to.i64#"),
            ("Int16",   "unbox",   "trunc.i64.to.i16#"),
            ("UInt16",  "llvm",    "i16"),
            ("UInt16",  "box",     "zext.i16.to.i64#"),
            ("UInt16",  "unbox",   "trunc.i64.to.i16#"),
            ("Int32",   "llvm",    "i32"),
            ("Int32",   "box",     "sext.i32.to.i64#"),
            ("Int32",   "unbox",   "trunc.i64.to.i32#"),
            ("UInt32",  "llvm",    "i32"),
            ("UInt32",  "box",     "zext.i32.to.i64#"),
            ("UInt32",  "unbox",   "trunc.i64.to.i32#"),
            ("Float",   "llvm",    "float"),
            ("Float",   "storage", "Native"),
            ("Float",   "bytes",   "4"),
            ("Float",   "box",     "bitcast.f32.to.i64#"),
            ("Float",   "unbox",   "bitcast.i64.to.f32#"),
            ("Float64", "llvm",    "double"),
            ("Float64", "storage", "Native"),
            ("Float64", "bytes",   "8"),
            ("Float64", "box",     "bitcast.f64.to.i64#"),
            ("Float64", "unbox",   "bitcast.i64.to.f64#"),
            ("Bool",    "llvm",    "i8"),
            ("Bool",    "storage", "Boxed"),
            ("Bool",    "bytes",   "1"),
            ("Bool",    "box",     "zext.i1.to.i64#"),
            ("Bool",    "unbox",   "trunc.i64.to.i1#"),
            ("Char",    "llvm",    "i32"),
            ("Char",    "storage", "Boxed"),
            ("Char",    "bytes",   "4"),
            ("Char",    "box",     "zext.i32.to.i64#"),
            ("Char",    "unbox",   "trunc.i64.to.i32#"),
            ("String",  "llvm",    "%String"),
            ("String",  "storage", "Native"),
            ("String",  "bytes",   "24"),
            ("String",  "box",     "ptrtoint#"),
            ("String",  "unbox",   "inttoptr#"),
            ("Data",    "llvm",    "i8*"),
            ("Data",    "storage", "Boxed"),
            ("Data",    "bytes",   "8"),
            ("Data",    "box",     "ptrtoint#"),
            ("Data",    "unbox",   "inttoptr#"),
        ];
        for &(type_name, property, expected) in checks {
            let rt = self.types.get(type_name).unwrap_or_else(|| {
                panic!(
                    "TypeUniverse validation FAILED: built-in type '{}' not found. \
                     Bootstrap loading failed silently.",
                    type_name
                )
            });
            let actual: &str = match property {
                "llvm" => &rt.llvm_type,
                "storage" => &rt.storage,
                "tbaa" => &rt.tbaa_node,
                "bytes" => {
                    // bytes is a u64 — convert to string for comparison
                    let s = rt.bytes.to_string();
                    // Leak the string for &str comparison (validation only, called once)
                    let leaked: &'static str = Box::leak(s.into_boxed_str());
                    leaked
                }
                "box" => rt.box_op.as_deref().unwrap_or("(missing)"),
                "unbox" => rt.unbox_op.as_deref().unwrap_or("(missing)"),
                _ => panic!("Unknown validation property '{}'", property),
            };
            assert_eq!(
                actual, expected,
                "TypeUniverse validation FAILED: {} {} = '{}', expected '{}'. \
                 This means a bootstrap binding silently failed to apply.",
                type_name, property, actual, expected
            );
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
            properties: HashMap::new(),
            default_params: vec![],
            commuting: true,
            constant_time: false,
            struct_layout: None,
            defining_module: "builtin".to_string(),
            source: crate::ast::TypeDef {
                name: String::new(),
                type_params: vec![],
                base: Box::new(Expr::TypeRef("Bits".into())),
                bit_range: None,
                body: crate::ast::TypeDefBody {
                    slots: vec![],
                    metadata: HashMap::new(),
                    projections: vec![],
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

        // Phase 0: Register built-in primitive types from bootstrap file
        universe.init_primitives_from_bootstrap();
        // 2026-07-01: Validate that all built-in types have correct properties.
        // If a bootstrap binding silently failed (e.g., apply_binding using
        // wrong Expr variant), this assertion catches it at compiler startup
        // rather than producing invalid LLVM IR.
        universe.validate_primitives();

        // Phase 1: Collect codec declarations
        for item in &program.items {
            if let TopLevel::Codec(codec) = item {
                universe.codecs.insert(codec.name.clone(), codec.clone());
            }
        }

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
            defining_module: "user".to_string(),
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
            properties: HashMap::new(),
            default_params: vec![],
            commuting: true,
            constant_time: false,
            struct_layout: None,
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
            // Inherit Phase 2B fields from base
            rt.default_params = base.default_params.clone();
            rt.commuting = base.commuting;
            rt.constant_time = base.constant_time;
            rt.struct_layout = base.struct_layout.clone();
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

        // Apply bindings — known metadata names populate ResolvedType fields.
        // Uses legacy bindings field during migration (Phase 1A.2).
        // Phase 1B will read from metadata + projections separately.
        for binding in &td.body.bindings {
            self.apply_binding(&mut rt, binding);
        }

        // Populate generic properties map from TypeDefBody metadata.
        // 2026-07-11: Phase 1B — enables property-system queries in codegen.
        for (key, value) in &td.body.metadata {
            rt.properties.insert(key.clone(), value.clone());
        }

        // D-7: Synthesize runtime guard expressions from constraints
        // Each constraint expression is stored as a guard that must pass
        // when a value of this type is constructed or assigned.
        for constraint in &td.body.constraints {
            rt.guards.push(constraint.clone());
        }

        // ── Phase 4: Link codec constraints ─────────────────────
        // 2026-07-11: If this type references a registered codec, merge
        // the codec's validation constraints into the type's guards.
        if let Some(ref codec_name) = rt.codec {
            if let Some(codec_decl) = self.codecs.get(codec_name) {
                for constraint in &codec_decl.constraints {
                    rt.guards.push(constraint.clone());
                }
            }
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

        // 2026-07-11: Compute struct_layout from slot declarations.
        // Slots declare how the type's bits are partitioned: `name: Type;`
        // Each slot's offset is computed sequentially from the previous slot,
        // using the slot type's byte size from the universe.
        if !td.body.slots.is_empty() {
            let mut offset_bits: u64 = 0;
            let mut fields: Vec<StructField> = Vec::new();
            for slot in &td.body.slots {
                let slot_bytes = self.byte_size(&slot.ty).unwrap_or(8);
                let slot_bits = slot_bytes * 8;
                fields.push(StructField {
                    name: slot.name.clone(),
                    ty: slot.ty.clone(),
                    offset_bits,
                    size_bits: slot_bits,
                });
                offset_bits += slot_bits;
            }
            let total_bytes = if offset_bits == 0 { 0 } else { (offset_bits + 7) / 8 };
            rt.struct_layout = Some(StructLayout {
                fields,
                packed: true,
                total_bytes,
                alignment: rt.alignment,
            });
        }

        Some(rt)
    }

    /// Apply a TypeBinding to a ResolvedType.
    /// Known metadata property names override the corresponding field;
    /// unknown names are stored in the projections map.
    fn apply_binding(&self, rt: &mut ResolvedType, binding: &TypeBinding) {
        // 2026-06-30: Use Expr helper methods (as_integer, as_bool, as_string) that
        // handle BOTH old-style direct variants (Expr::Integer, Expr::String, Expr::Bool)
        // AND new-style Literal-packed variants (Expr::Literal(LiteralExpr::Integer(...))).
        // Without this, bindings from parsed source (bootstrap file, user code) silently
        // fail to apply, leaving default values (llvm_type="i64", storage="Boxed", etc.).
        match binding.name.to_lowercase().as_str() {
            "bytes" => {
                if let Some(n) = binding.value.as_integer() {
                    rt.bytes = n as u64;
                }
            }
            "alignment" => {
                if let Some(n) = binding.value.as_integer() {
                    rt.alignment = n as u64;
                }
            }
            "endian" => {
                if let Expr::Identifier(name) = binding.value.as_ref() {
                    rt.endian = if name == "Big" || name == "big" { 1 } else { 0 };
                }
            }
            "volatile" => {
                if let Some(b) = binding.value.as_bool() {
                    rt.volatile = b;
                }
            }
            "atomic" => {
                if let Some(b) = binding.value.as_bool() {
                    rt.atomic = b;
                }
            }
            "elementtype" => {
                if let Expr::TypeRef(name) = binding.value.as_ref() {
                    rt.element_type = Some(name.clone());
                }
            }
            "fixedsize" => {
                if let Some(b) = binding.value.as_bool() {
                    rt.fixed_size = Some(b);
                }
            }
            "insertat" => {
                rt.insert_at = type_universe_expr_to_string(&binding.value);
            }
            "extractfrom" => {
                rt.extract_from = type_universe_expr_to_string(&binding.value);
            }
            "allowindex" => {
                if let Some(b) = binding.value.as_bool() {
                    rt.allow_index = b;
                }
            }
            "allowslice" => {
                if let Some(b) = binding.value.as_bool() {
                    rt.allow_slice = b;
                }
            }
            "allowarrow" => {
                if let Some(b) = binding.value.as_bool() {
                    rt.allow_arrow = b;
                }
            }
            "codec" => {
                if let Some(s) = binding.value.as_string() {
                    if !KNOWN_CODECS.contains(&s) {
                        // Unknown codec — warn but accept (forward compat)
                    }
                    rt.codec = Some(s.to_string());
                } else if let Expr::Identifier(id) = binding.value.as_ref() {
                    if !KNOWN_CODECS.contains(&id.as_str()) {
                        // Unknown codec identifier — warn but accept
                    }
                    rt.codec = Some(id.clone());
                }
            }
            "onexit" => {
                // Foreign destructor function
                if let Some(s) = binding.value.as_string() {
                    rt.on_exit = Some(s.to_string());
                } else if let Expr::Identifier(id) = binding.value.as_ref() {
                    rt.on_exit = Some(id.clone());
                } else if let Expr::IntrinsicCall { intrinsic, .. } = binding.value.as_ref() {
                    rt.on_exit = Some(intrinsic.name().to_string());
                }
            }
            // ── Codegen Property Handlers ───────────────────────
            "llvm" => {
                if let Some(s) = binding.value.as_string() {
                    rt.llvm_type = s.to_string();
                }
            }
            "storage" => {
                if let Some(s) = binding.value.as_string() {
                    rt.storage = s.to_string();
                }
            }
            "tbaa" => {
                if let Some(s) = binding.value.as_string() {
                    rt.tbaa_node = s.to_string();
                }
            }
            "box" => {
                if let Some(s) = binding.value.as_string() {
                    rt.box_op = Some(s.to_string());
                } else if let Expr::Identifier(id) = binding.value.as_ref() {
                    rt.box_op = Some(id.clone());
                }
            }
            "unbox" => {
                if let Some(s) = binding.value.as_string() {
                    rt.unbox_op = Some(s.to_string());
                } else if let Expr::Identifier(id) = binding.value.as_ref() {
                    rt.unbox_op = Some(id.clone());
                }
            }
            // ── Phase 2B binding handlers ────────────────────────
            "default_width" => {
                if let Some(n) = binding.value.as_integer() {
                    rt.default_params.push(("W".to_string(), crate::ast::Type::Width(n as u64)));
                }
            }
            "commuting" => {
                if let Some(b) = binding.value.as_bool() {
                    rt.commuting = b;
                }
            }
            "default_codec" => {
                if let Some(n) = binding.value.as_integer() {
                    rt.codec = Some(n.to_string());
                } else if let Some(s) = type_universe_expr_to_string(&binding.value) {
                    rt.codec = Some(s);
                }
            }
            "constant_time" => {
                if let Some(b) = binding.value.as_bool() {
                    rt.constant_time = b;
                }
            }
            // Unknown name → user-defined projection
            _ => {
                rt.projections.insert(binding.name.clone(), binding.clone());
            }
        }

        // Phase 1B: Dual-write to generic properties map.
        if let Some(pv) = crate::ast::binding_to_property_value(binding) {
            rt.properties.insert(binding.name.to_lowercase(), pv);
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

    /// Get the LLVM type string from the property system.
    /// Phase 2: replaces hardcoded `Custom("Float")` etc. matches.
    pub fn llvm_type_for(&self, ty: &crate::ast::Type) -> Option<&str> {
        self.get_by_type(ty)?.get_property_str("llvm")
    }

    /// Get byte size from the property system.
    /// Phase 2: replaces hardcoded `Custom("Int") → 8` etc. matches.
    pub fn byte_size_for(&self, ty: &crate::ast::Type) -> Option<u64> {
        self.get_by_type(ty)?.get_property_int("bytes").map(|n| n as u64)
    }

    /// Check if a type has Native storage from the property system.
    /// Phase 2: replaces hardcoded `Custom("Float")` etc. matches.
    pub fn is_native(&self, ty: &crate::ast::Type) -> Option<bool> {
        self.get_by_type(ty).map(|rt| rt.get_property_str("storage") == Some("Native"))
    }

    /// Get TBAA tag from the property system.
    /// Phase 2: replaces hardcoded `Custom("String") → "String"` matches.
    pub fn tbaa_for(&self, ty: &crate::ast::Type) -> Option<&str> {
        self.get_by_type(ty)?.get_property_str("tbaa")
    }

    /// Get alignment from the property system.
    /// Phase 2: replaces hardcoded alignment matches.
    pub fn alignment_for(&self, ty: &crate::ast::Type) -> Option<u64> {
        self.get_by_type(ty)?.get_property_int("alignment").map(|n| n as u64)
    }

    /// Check if a type has a given canonical name (property system + legacy fallback).
    /// Phase 2: replaces `ty == Type::int()` etc.
    pub fn type_is(&self, ty: &crate::ast::Type, name: &str) -> bool {
        // Property system check
        if let Some(rt) = self.get_by_type(ty) {
            if rt.name == name { return true; }
        }
        // Legacy fallback
        *ty == crate::ast::Type::Custom(name.to_string())
    }

    // 2026-07-08: Phase 2B — compute LLVM type string from base type + width.
    // For Int<8>: base "Int" + width 8 → "i8"
    // For Float<32>: base "Float" + width 32 → "float"
    pub fn llvm_type_for_width(&self, base_name: &str, width: u64) -> Option<std::borrow::Cow<'static, str>> {
        let rt = self.get(base_name)?;
        match rt.storage.as_str() {
            "Native" => match base_name {
                "Float" if width <= 32 => Some(std::borrow::Cow::Borrowed("float")),
                "Float" if width <= 64 => Some(std::borrow::Cow::Borrowed("double")),
                _ => Some(std::borrow::Cow::Owned(format!("i{}", width))),
            },
            "Boxed" => Some(std::borrow::Cow::Borrowed("i64")),
            _ => Some(std::borrow::Cow::Owned(format!("i{}", width))),
        }
    }

    pub fn byte_size(&self, ty: &crate::ast::Type) -> Option<u64> {
        match ty {
            crate::ast::Type::LayoutPtr(lc) => Some(lc.bytes),
            crate::ast::Type::Custom(__t) if __t == "Int" || __t == "UInt" => Some(8),
            crate::ast::Type::Custom(__t) if __t == "Int8" || __t == "UInt8" => Some(1),
            crate::ast::Type::Custom(__t) if __t == "Int16" || __t == "UInt16" => Some(2),
            crate::ast::Type::Custom(__t) if __t == "Int32" || __t == "UInt32" => Some(4),
            crate::ast::Type::Custom(__t) if __t == "Float" => Some(4),
            crate::ast::Type::Custom(__t) if __t == "Float64" => Some(8),
            crate::ast::Type::Custom(__t) if __t == "Bool" || __t == "Char" => Some(1),
            crate::ast::Type::Void => Some(0),
            crate::ast::Type::Custom(__t) if __t == "String" || __t == "Data" => Some(8),
            crate::ast::Type::Custom(name) => {
                self.get(name).map(|rt| rt.bytes)
            }
            crate::ast::Type::Applied(name, _) => {
                // Ptr<T> is always pointer-width (8 bytes on 64-bit)
                if name == "Ptr" {
                    return Some(8);
                }
                // For custom generic types like List<T>, look up the base type
                self.get(name).map(|rt| rt.bytes)
            }
            crate::ast::Type::Enum(name) => {
                self.get(name).map(|rt| rt.bytes)
            }
            // Compound types — default to 8 as safe fallback
            _ => Some(8),
        }
    }

    // 2026-07-03: Return the alignment requirement of a type.
    // Handles primitives, LayoutPtr, compound types, and universe-resolved types.
    pub fn alignment(&self, ty: &crate::ast::Type) -> Option<u64> {
        match ty {
            crate::ast::Type::LayoutPtr(lc) => Some(lc.alignment),
            crate::ast::Type::Custom(__t) if __t == "Int" || __t == "UInt" => Some(8),
            crate::ast::Type::Custom(__t) if __t == "Int8" || __t == "UInt8" => Some(1),
            crate::ast::Type::Custom(__t) if __t == "Int16" || __t == "UInt16" => Some(2),
            crate::ast::Type::Custom(__t) if __t == "Int32" || __t == "UInt32" => Some(4),
            crate::ast::Type::Custom(__t) if __t == "Float" => Some(4),
            crate::ast::Type::Custom(__t) if __t == "Float64" => Some(8),
            crate::ast::Type::Custom(__t) if __t == "Bool" => Some(1),
            crate::ast::Type::Custom(__t) if __t == "Char" => Some(4),
            crate::ast::Type::Void => Some(1),
            crate::ast::Type::Custom(__t) if __t == "String" || __t == "Data" => Some(8),
            crate::ast::Type::Custom(name) => {
                self.get(name).map(|rt| rt.alignment)
            }
            crate::ast::Type::Applied(name, _) => {
                if name == "Ptr" {
                    return Some(8);
                }
                self.get(name).map(|rt| rt.alignment)
            }
            crate::ast::Type::Enum(name) => {
                self.get(name).map(|rt| rt.alignment)
            }
            _ => Some(8),
        }
    }

    // 2026-07-03: Extract the pointee layout (bytes, alignment) from a pointer type.
    // For Ptr<T>, returns the layout of T. For LayoutPtr(lc), returns (lc.bytes, lc.alignment).
    // For non-pointer types, returns None. Used by layout-compatible cast checking.
    pub fn pointer_pointee_layout(&self, ty: &crate::ast::Type) -> Option<(u64, u64)> {
        match ty {
            crate::ast::Type::LayoutPtr(lc) => Some((lc.bytes, lc.alignment)),
            crate::ast::Type::Applied(name, args) if name == "Ptr" && args.len() == 1 => {
                let inner = &args[0];
                // For Ptr<Bits @/range>, compute from the bit range
                if let crate::ast::Type::Constrained(inner, br) = inner {
                    if **inner == crate::ast::Type::data() {
                        let bits = match br {
                            crate::ast::BitRange::Range(start, end) => end - start + 1,
                            crate::ast::BitRange::Single(_) => 1,
                            crate::ast::BitRange::Any(n) => *n,
                        };
                        let bytes = (bits + 7) / 8;
                        return Some((bytes as u64, bytes as u64));
                    }
                }
                // For Ptr<CustomType>, look up the type's layout from the universe
                if let crate::ast::Type::Custom(name) = inner {
                    if let Some(rt) = self.get(name) {
                        return Some((rt.bytes, rt.alignment));
                    }
                }
                // For Ptr<PrimitiveType>, use byte_size/alignment
                let bytes = self.byte_size(inner)?;
                let align = self.alignment(inner)?;
                Some((bytes, align))
            }
            // Not a pointer type
            _ => None,
        }
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

    // ── Phase 7B: Operator Validation ────────────────────────
    //
    // 2026-06-29: Validates that all operator→intrinsic mappings in
    // the universe reference known intrinsics. Called after universe
    // build to catch typos and unsupported intrinsics early.

    /// Validate all operator declarations in the universe.
    /// Returns a list of errors for any invalid operator mappings.
    pub fn validate_operators(&self) -> Vec<String> {
        let mut errors = vec![];

        for (type_name, rt) in &self.types {
            for ((rune, param), op) in &rt.operators {
                let param_str = param.as_deref().unwrap_or("none");
                let location = format!("type '{}' operator '{:?}({})'",
                    type_name, rune, param_str);

                // Validate implementation expression
                match &op.implementation.as_ref() {
                    Expr::IntrinsicCall { intrinsic, .. } => {
                        // Intrinsic variants are always valid at the AST level.
                        // Backend-specific support checking happens during codegen.
                        // Check for unknown/placeholder intrinsics.
                        let name = format!("{:?}", intrinsic);
                        if name.starts_with("Unknown") || name.contains("__unknown") {
                            errors.push(format!("{}: unknown intrinsic '{:?}'",
                                location, intrinsic));
                        }
                    }
                    Expr::Identifier(name) => {
                        // Identifiers reference defns or frgn functions.
                        // Validation deferred to link phase.
                    }
                    _ => {
                        // Inop blocks, identifiers, and unknown expressions — valid
                    }
                }
            }
        }

        errors
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

/// Returns true if `ty` is `PtrConst<T>` (a read-only pointer).
/// Returns false for `Ptr<T>` (mutable pointer) and all other types.
pub fn is_const_ptr(ty: &crate::ast::Type) -> bool {
    match ty {
        crate::ast::Type::Applied(name, _) => name == "PtrConst",
        _ => false,
    }
}

/// Returns the pointee type for `Ptr<T>` or `PtrConst<T>`.
/// Returns `None` for any other type.
pub fn pointee_type(ty: &crate::ast::Type) -> Option<crate::ast::Type> {
    match ty {
        crate::ast::Type::Applied(name, args) if (name == "Ptr" || name == "PtrConst") && args.len() == 1 => {
            Some(args[0].clone())
        }
        _ => None,
    }
}

/// Returns true if `expr` names a mutable storage location (can be borrowed as `Ptr<T>`).
/// Non-mutable locations (e.g., `let` bindings, literals) are borrowed as `PtrConst<T>`.
pub fn is_mutable_location(expr: &Expr) -> bool {
    match expr {
        Expr::Identifier(_) | Expr::AddrOf(_) => true,
        Expr::PriorState(_) | Expr::Deref(_) => true,
        Expr::FieldAccess(obj, _) | Expr::ListIndex(obj, _) => is_mutable_location(obj),
        _ => false,
    }
}

// ── Tests ───────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::{Comment, DispatchMode, Expr, MeldDeclaration, OpDeclaration, OpRune, StrictMode, TopLevel, TypeBinding, TypeDef, TypeDefBody, TypeSlot, Program};

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
                slots: vec![],
                metadata: HashMap::new(),
                projections: vec![],
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
                slots: vec![],
                metadata: HashMap::new(),
                projections: vec![],
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
                slots: vec![],
                metadata: HashMap::new(),
                projections: vec![],
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
                slots: vec![],
                metadata: HashMap::new(),
                projections: vec![],
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
                slots: vec![],
                metadata: HashMap::new(),
                projections: vec![],
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
                slots: vec![],
                metadata: HashMap::new(),
                projections: vec![],
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
                slots: vec![],
                metadata: HashMap::new(),
                projections: vec![],
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
                slots: vec![],
                metadata: HashMap::new(),
                projections: vec![],
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
                slots: vec![],
                metadata: HashMap::new(),
                projections: vec![],
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
    /// 2026-07-11: Phase 4 — Test codec declaration is collected by TypeUniverse.
    fn test_codec_declaration_collected() {
        let codec = TopLevel::Codec(CodecDeclaration {
            name: "PositiveInt".into(),
            constraints: vec![
                Expr::BinaryOp(Box::new(BinaryOpExpr::new(BinaryOpKind::Gt, Expr::Identifier("value".into()), Expr::Integer(0)))),
            ],
            span: None,
        });
        let program = make_program(vec![codec]);
        let universe = TypeUniverse::build(&program);
        assert!(universe.codecs.contains_key("PositiveInt"));
        assert_eq!(universe.codecs["PositiveInt"].constraints.len(), 1);
    }

    #[test]
    /// 2026-07-11: Phase 4 — Test codec constraints are merged into type guards.
    fn test_codec_constraints_merged_into_type() {
        let codec = TopLevel::Codec(CodecDeclaration {
            name: "PositiveInt".into(),
            constraints: vec![
                Expr::BinaryOp(Box::new(BinaryOpExpr::new(BinaryOpKind::Gt, Expr::Identifier("value".into()), Expr::Integer(0)))),
            ],
            span: None,
        });
        let td = TypeDef {
            name: "MyInt".into(),
            type_params: vec![],
            bit_range: None,
            base: Box::new(Expr::TypeRef("Int".into())),
            body: TypeDefBody {
                slots: vec![],
                metadata: HashMap::new(),
                projections: vec![],
                bindings: vec![
                    TypeBinding { name: "Codec".into(), params: vec![], value: Box::new(Expr::Identifier("PositiveInt".into())), span: None },
                ],
                operators: vec![],
                constraints: vec![],
                span: None,
            },
            span: None,
        };
        let program = make_program(vec![codec, TopLevel::TypeDef(Box::new(td))]);
        let universe = TypeUniverse::build(&program);
        let my_int = universe.get("MyInt").unwrap();
        // Should have the codec's constraint merged into guards
        assert_eq!(my_int.guards.len(), 1);
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
                slots: vec![],
                metadata: HashMap::new(),
                projections: vec![],
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
                slots: vec![],
                metadata: HashMap::new(),
                projections: vec![],
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
                slots: vec![],
                metadata: HashMap::new(),
                projections: vec![],
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
                slots: vec![],
                metadata: HashMap::new(),
                projections: vec![],
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
                slots: vec![],
                metadata: HashMap::new(),
                projections: vec![],
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
                slots: vec![],
                metadata: HashMap::new(),
                projections: vec![],
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

    // ── Phase 7B: Operator Validation Tests ───────────────────
    #[test]
    fn test_validate_operators_valid_intrinsic() {
        use crate::ast::OpRune;
        let td = TypeDef {
            name: "TestType".into(), type_params: vec![], bit_range: None,
            base: Box::new(Expr::TypeRef("Bits".into())),
            body: TypeDefBody::from_bindings(
                vec![],
                vec![TypeBinding { name: "Bytes".into(), params: vec![], value: Box::new(Expr::Integer(8)), span: None }],
                vec![OpDeclaration {
                    rune: OpRune::Add,
                    param_type: Some(Box::new(Expr::TypeRef("TestType".into()))),
                    return_type: Box::new(Expr::TypeRef("TestType".into())),
                    implementation: Box::new(Expr::Identifier("my_op".into())),
                    span: None,
                }],
                vec![],
                None,
            ), span: None,
        };
        let program = make_program(vec![TopLevel::TypeDef(Box::new(td))]);
        let universe = TypeUniverse::build(&program);
        let errors = universe.validate_operators();
        assert!(errors.is_empty(), "Valid identifier op should pass: {:?}", errors);
    }

    // ── is_const_ptr / pointee_type / is_mutable_location tests ──

    #[test]
    fn test_is_const_ptr_on_ptr_returns_false() {
        let ty = crate::ast::Type::Applied("Ptr".to_string(), vec![crate::ast::Type::int()]);
        assert!(!is_const_ptr(&ty), "Ptr<T> is not const");
    }

    #[test]
    fn test_is_const_ptr_on_ptr_const_returns_true() {
        let ty = crate::ast::Type::Applied("PtrConst".to_string(), vec![crate::ast::Type::int()]);
        assert!(is_const_ptr(&ty), "PtrConst<T> is const");
    }

    #[test]
    fn test_is_const_ptr_on_other_returns_false() {
        assert!(!is_const_ptr(&crate::ast::Type::int()));
        assert!(!is_const_ptr(&crate::ast::Type::Void));
    }

    #[test]
    fn test_pointee_type_ptr() {
        let inner = crate::ast::Type::int();
        let ty = crate::ast::Type::Applied("Ptr".to_string(), vec![inner.clone()]);
        assert_eq!(pointee_type(&ty), Some(inner));
    }

    #[test]
    fn test_pointee_type_ptr_const() {
        let inner = crate::ast::Type::bool_();
        let ty = crate::ast::Type::Applied("PtrConst".to_string(), vec![inner.clone()]);
        assert_eq!(pointee_type(&ty), Some(inner));
    }

    #[test]
    fn test_pointee_type_non_ptr_returns_none() {
        assert_eq!(pointee_type(&crate::ast::Type::int()), None);
        assert_eq!(pointee_type(&crate::ast::Type::Void), None);
    }

    #[test]
    fn test_pointee_type_wrong_arg_count_returns_none() {
        let ty = crate::ast::Type::Applied("Ptr".to_string(), vec![]);
        assert_eq!(pointee_type(&ty), None);
    }

    #[test]
    fn test_is_mutable_location_identifier() {
        let expr = Expr::Identifier("x".to_string());
        assert!(is_mutable_location(&expr));
    }

    #[test]
    fn test_is_mutable_location_addr_of() {
        let expr = Expr::AddrOf(Box::new(Expr::Identifier("x".to_string())));
        assert!(is_mutable_location(&expr));
    }

    #[test]
    fn test_is_mutable_location_field_access() {
        let expr = Expr::FieldAccess(
            Box::new(Expr::Identifier("obj".to_string())),
            "field".to_string(),
        );
        assert!(is_mutable_location(&expr));
    }

    #[test]
    fn test_is_mutable_location_literal_returns_false() {
        assert!(!is_mutable_location(&Expr::Integer(42)));
        assert!(!is_mutable_location(&Expr::Bool(true)));
    }

    // ── Type slot syntax tests ─────────────────────────────────

    #[test]
    fn test_slot_struct_layout_computed() {
        let td = TypeDef {
            name: "MyPoint".into(),
            type_params: vec![],
            bit_range: None,
            base: Box::new(Expr::TypeRef("Bits".into())),
            body: TypeDefBody {
                slots: vec![
                    TypeSlot { name: "x".into(), ty: crate::ast::Type::Custom("Int".into()), span: None },
                    TypeSlot { name: "y".into(), ty: crate::ast::Type::Custom("Int".into()), span: None },
                ],
                metadata: HashMap::new(),
                projections: vec![],
                bindings: vec![],
                operators: vec![], constraints: vec![],
                span: None,
            },
            span: None,
        };
        let program = make_program(vec![TopLevel::TypeDef(Box::new(td))]);
        let universe = TypeUniverse::build(&program);
        let rt = universe.types.get("MyPoint").expect("MyPoint should be resolved");
        let layout = rt.struct_layout.as_ref().expect("MyPoint should have struct_layout");
        assert_eq!(layout.fields.len(), 2);
        assert_eq!(layout.fields[0].name, "x");
        assert_eq!(layout.fields[0].offset_bits, 0);
        assert_eq!(layout.fields[0].size_bits, 64);
        assert_eq!(layout.fields[1].name, "y");
        assert_eq!(layout.fields[1].offset_bits, 64);
        assert_eq!(layout.fields[1].size_bits, 64);
        assert!(layout.packed);
    }

    #[test]
    fn test_slot_struct_layout_with_ptr() {
        let td = TypeDef {
            name: "CBuffer".into(),
            type_params: vec![],
            bit_range: None,
            base: Box::new(Expr::TypeRef("Bits".into())),
            body: TypeDefBody {
                slots: vec![
                    TypeSlot { name: "ptr".into(), ty: crate::ast::Type::Applied("Ptr".into(), vec![crate::ast::Type::Custom("UInt8".into())]), span: None },
                    TypeSlot { name: "len".into(), ty: crate::ast::Type::Custom("Int".into()), span: None },
                ],
                metadata: HashMap::new(),
                projections: vec![],
                bindings: vec![],
                operators: vec![], constraints: vec![],
                span: None,
            },
            span: None,
        };
        let program = make_program(vec![TopLevel::TypeDef(Box::new(td))]);
        let universe = TypeUniverse::build(&program);
        let rt = universe.types.get("CBuffer").expect("CBuffer should be resolved");
        let layout = rt.struct_layout.as_ref().expect("CBuffer should have struct_layout");
        assert_eq!(layout.fields.len(), 2);
        assert_eq!(layout.fields[0].size_bits, 64);
        assert_eq!(layout.fields[0].offset_bits, 0);
        assert_eq!(layout.fields[1].size_bits, 64);
        assert_eq!(layout.fields[1].offset_bits, 64);
    }

    #[test]
    fn test_no_slots_no_struct_layout() {
        let td = TypeDef {
            name: "U64".into(),
            type_params: vec![],
            bit_range: None,
            base: Box::new(Expr::TypeRef("Bits".into())),
            body: TypeDefBody {
                slots: vec![],
                metadata: HashMap::new(),
                projections: vec![],
                bindings: vec![
                    TypeBinding { name: "Bytes".into(), params: vec![], value: Box::new(Expr::Integer(8)), span: None },
                ],
                operators: vec![], constraints: vec![],
                span: None,
            },
            span: None,
        };
        let program = make_program(vec![TopLevel::TypeDef(Box::new(td))]);
        let universe = TypeUniverse::build(&program);
        let rt = universe.types.get("U64").expect("U64 should be resolved");
        assert!(rt.struct_layout.is_none(), "U64 without slots should have no struct_layout");
    }
}
