use std::collections::{HashMap, HashSet, VecDeque};

use crate::ast::top::{CastDirection, ProtocolDef};
use crate::ast::Type;
use crate::ast::PropertyValue;
use crate::type_universe::TypeUniverse;

// ── Lane Kinds ──────────────────────────────────────────────────────────

// ── Lane Kinds ──────────────────────────────────────────────────────────

/// The kind of transformation a single cast step performs.
/// Each variant maps to a specific LLVM IR instruction or call pattern.
#[derive(Debug, Clone, PartialEq)]
pub enum LaneKind {
    /// LLVM bitcast: src_ty to dst_ty (same-width reinterpretation)
    Bitcast,
    /// Signed integer to float: sitofp i64 %v to double
    IntToFloat,
    /// Float to signed integer: fptosi double %v to i64
    FloatToInt,
    /// Call an external/intrinsic conversion function: call @fn_name
    ExtCall(&'static str),
    /// Call a proto-binding transform function (owned name — user-declared
    /// `proto C_String: #String { CastTo(...) = cstr_to_brief(#L); }`).
    /// 2026-08-03: distinct from ExtCall so seeded base lanes keep their
    /// `&'static str` without changing all call sites.
    ExtCallDyn(String),
    /// Extract first field of a struct: extractvalue {i64,i64} %v, 0
    ExtractData,
    /// Pointer to integer: ptrtoint ptr %v to i64
    PtrToInt,
    /// Integer to pointer: inttoptr i64 %v to ptr
    IntToPtr,
    /// Zero-extend: zext i8 %v to i64
    ZExt,
    /// Truncate: trunc i64 %v to i8 (or i64 to i32, etc.)
    Trunc,
    /// Type-level CastFrom(#Bit) override — function name resolved at emission time
    CastFromBitCallback,
    /// Composite: chain two consecutive lanes
    Chain(Box<LaneKind>, Box<LaneKind>),
}

// ── Cast Step ───────────────────────────────────────────────────────────

/// A single resolved step in a protocol-to-protocol cast path.
#[derive(Debug, Clone, PartialEq)]
pub struct CastStep {
    /// The lane to traverse
    pub lane: LaneKind,
    /// Source protocol category name (e.g., "Int", "String")
    pub src_category: String,
    /// Source protocol variant (empty for base protocols)
    pub src_variant: String,
    /// Destination protocol category
    pub dst_category: String,
    /// Destination protocol variant
    pub dst_variant: String,
}

// ── LLVM Type Resolver ──────────────────────────────────────────────────

/// How a protocol category + variant maps to an LLVM type string.
#[derive(Debug, Clone, PartialEq)]
pub enum LlvmTypeResolver {
    /// Fixed LLVM type string (e.g., "double", "ptr", "{ i64, i64 }")
    Fixed(&'static str),
    /// Width-parametric: !> bits → !> maxbits → !> minbits → int_bits
    WidthParametric,
}

// ── Casting Graph ───────────────────────────────────────────────────────

/// Protocol-to-protocol casting graph.
///
/// Every base protocol has a hardcoded direct lane to every other base
/// protocol (64 entries). Variant edges from `proto` declarations add
/// additional edges for sub-protocols. BFS resolves variant→variant
/// and variant→base paths through the union of base lanes and variant edges.
///
/// `CastTo(#Bit)` is banned at declaration time — the `→ #Bit` direction
/// is always a hardcoded mechanical operation (bitcast/extractvalue/ptrtoint).
/// `CastFrom(#Bit)` is the sole user-extensible edge direction.
#[derive(Debug, Clone)]
pub struct CastingGraph {
    /// Base protocol → base protocol direct lanes.
    /// Indexed by (src_category, dst_category) where both are base protocol
    /// category names (e.g., ("Int", "Float")).
    base_lanes: HashMap<String, HashMap<String, LaneKind>>,

    /// Per-variant CastTo edges from proto declarations.
    /// Indexed by (category, variant_name).
    variant_edges: HashMap<(String, String), Vec<CastStep>>,

    /// Per-variant reverse edges (from CastFrom declarations).
    variant_reverse: HashMap<(String, String), Vec<CastStep>>,

    /// Default variant per category (e.g., String→UTF8, Float→IEEE754, Char→unicode).
    defaults: HashMap<String, String>,

    /// Type-level CastFrom(#Bit) overrides: type_name → function_name.
    cast_from_bit_overrides: HashMap<String, String>,

    /// Protocol (category, variant) → LLVM type resolver.
    /// Used by resolve_llvm_type() to derive LLVM types from protocol + metadata.
    protocol_llvm_types: HashMap<(String, String), LlvmTypeResolver>,
}

impl CastingGraph {
    /// Create a new casting graph seeded with all base protocol lanes.
    pub fn new() -> Self {
        let mut graph = CastingGraph {
            base_lanes: HashMap::new(),
            variant_edges: HashMap::new(),
            variant_reverse: HashMap::new(),
            defaults: HashMap::new(),
            cast_from_bit_overrides: HashMap::new(),
            protocol_llvm_types: HashMap::new(),
        };
        graph.seed_base_lanes();
        graph.seed_defaults();
        graph.seed_protocol_llvm_types();
        graph
    }

    /// Seed all 64 base protocol → base protocol lanes.
    fn seed_base_lanes(&mut self) {
        // All 8 base protocol categories.
        // "Bit" is the root — every other protocol has a direct lane to/from Bit.
        //
        // Convention: we populate both directions for clarity. The graph is
        // symmetric: (A,B) means A→B lane, (B,A) means B→A lane.

        // ── Bit ⇄ Int ──────────────────────────────────────────────
        self.set_lane("Bit", "Int", LaneKind::Bitcast);
        self.set_lane("Int", "Bit", LaneKind::Bitcast);
        // ── Bit ⇄ UInt ─────────────────────────────────────────────
        self.set_lane("Bit", "UInt", LaneKind::Bitcast);
        self.set_lane("UInt", "Bit", LaneKind::Bitcast);
        // ── Bit ⇄ Float ────────────────────────────────────────────
        self.set_lane("Bit", "Float", LaneKind::Bitcast);
        self.set_lane("Float", "Bit", LaneKind::Bitcast);
        // ── Bit ⇄ String ───────────────────────────────────────────
        // 2026-08-01 (B2): #Bit→#String is the ENCODING DOOR — a CastFrom(#Bit)
        // callback (UTF8 wrap default, materializing the [len][bytes] header by
        // construction; sub-protocols override via register_cast_from_bit). It
        // is NOT a bitcast: wrapping must produce a header-prefixed buffer.
        self.set_lane("Bit", "String", LaneKind::CastFromBitCallback);
        // #String→#Bit: the CONTENT VIEW — a String value IS a ptr to
        // [len][bytes], so the cast yields the buffer address (ptrtoint, one
        // instruction). Zero-length/identity at the register level; never
        // overridable (the representation is compiler-guaranteed).
        self.set_lane("String", "Bit", LaneKind::PtrToInt);
        // ── Bit ⇄ Bool ─────────────────────────────────────────────
        self.set_lane("Bit", "Bool", LaneKind::Trunc);    // i64 → i8
        self.set_lane("Bool", "Bit", LaneKind::ZExt);     // i8 → i64
        // ── Bit ⇄ Char ─────────────────────────────────────────────
        self.set_lane("Bit", "Char", LaneKind::Trunc);    // i64 → i32
        self.set_lane("Char", "Bit", LaneKind::ZExt);     // i32 → i64
        // ── Bit ⇄ Data ──────────────────────────────────────────────
        self.set_lane("Bit", "Data", LaneKind::IntToPtr); // i64 → ptr
        self.set_lane("Data", "Bit", LaneKind::PtrToInt); // ptr → i64

        // ── Int ⇄ UInt ────────────────────────────────────────────
        self.set_lane("Int", "UInt", LaneKind::Bitcast); // same representation
        self.set_lane("UInt", "Int", LaneKind::Bitcast);
        // ── Int ⇄ Float ───────────────────────────────────────────
        self.set_lane("Int", "Float", LaneKind::IntToFloat);
        self.set_lane("Float", "Int", LaneKind::FloatToInt);
        // ── Int ⇄ String ──────────────────────────────────────────
        self.set_lane("Int", "String", LaneKind::ExtCall("int_to_str"));
        self.set_lane("String", "Int", LaneKind::ExtCall("str_to_int"));
        // ── Int ⇄ Bool ────────────────────────────────────────────
        self.set_lane("Int", "Bool", LaneKind::Trunc);   // i64 → i8
        self.set_lane("Bool", "Int", LaneKind::ZExt);    // i8 → i64
        // ── Int ⇄ Char ────────────────────────────────────────────
        self.set_lane("Int", "Char", LaneKind::Trunc);   // i64 → i32
        self.set_lane("Char", "Int", LaneKind::ZExt);    // i32 → i64
        // ── Int ⇄ Data ────────────────────────────────────────────
        self.set_lane("Int", "Data", LaneKind::IntToPtr); // i64 → ptr
        self.set_lane("Data", "Int", LaneKind::PtrToInt); // ptr → i64

        // ── UInt ⇄ Float ───────────────────────────────────────────
        self.set_lane("UInt", "Float", LaneKind::IntToFloat);
        self.set_lane("Float", "UInt", LaneKind::FloatToInt);
        // ── UInt ⇄ String ─────────────────────────────────────────
        self.set_lane("UInt", "String", LaneKind::ExtCall("uint_to_str"));
        self.set_lane("String", "UInt", LaneKind::ExtCall("str_to_uint"));
        // ── UInt ⇄ Bool ───────────────────────────────────────────
        self.set_lane("UInt", "Bool", LaneKind::Trunc);
        self.set_lane("Bool", "UInt", LaneKind::ZExt);
        // ── UInt ⇄ Char ───────────────────────────────────────────
        self.set_lane("UInt", "Char", LaneKind::Trunc);
        self.set_lane("Char", "UInt", LaneKind::ZExt);
        // ── UInt ⇄ Data ───────────────────────────────────────────
        self.set_lane("UInt", "Data", LaneKind::IntToPtr);
        self.set_lane("Data", "UInt", LaneKind::PtrToInt);

        // ── Float ⇄ String ────────────────────────────────────────
        self.set_lane("Float", "String", LaneKind::ExtCall("float_to_str"));
        self.set_lane("String", "Float", LaneKind::ExtCall("str_to_float"));
        // ── Float ⇄ Bool ──────────────────────────────────────────
        // Float→Bool: fptosi i64 + trunc to i8 (chain)
        self.set_lane("Float", "Bool", LaneKind::Chain(
            Box::new(LaneKind::FloatToInt),
            Box::new(LaneKind::Trunc),
        ));
        self.set_lane("Bool", "Float", LaneKind::Chain(
            Box::new(LaneKind::ZExt),
            Box::new(LaneKind::IntToFloat),
        ));
        // ── Float ⇄ Char ──────────────────────────────────────────
        self.set_lane("Float", "Char", LaneKind::FloatToInt);
        self.set_lane("Char", "Float", LaneKind::IntToFloat);
        // ── Float ⇄ Data ──────────────────────────────────────────
        self.set_lane("Float", "Data", LaneKind::Chain(
            Box::new(LaneKind::FloatToInt),
            Box::new(LaneKind::IntToPtr),
        ));
        self.set_lane("Data", "Float", LaneKind::Chain(
            Box::new(LaneKind::PtrToInt),
            Box::new(LaneKind::IntToFloat),
        ));

        // ── String ⇄ Bool ─────────────────────────────────────────
        self.set_lane("String", "Bool", LaneKind::ExtCall("str_to_bool"));
        self.set_lane("Bool", "String", LaneKind::ExtCall("bool_to_str"));
        // ── String ⇄ Char ─────────────────────────────────────────
        self.set_lane("String", "Char", LaneKind::ExtCall("str_first_char"));
        self.set_lane("Char", "String", LaneKind::ExtCall("char_to_str"));
        // ── String ⇄ Data ─────────────────────────────────────────
        self.set_lane("String", "Data", LaneKind::Chain(
            Box::new(LaneKind::ExtractData),
            Box::new(LaneKind::IntToPtr),
        ));
        self.set_lane("Data", "String", LaneKind::Chain(
            Box::new(LaneKind::PtrToInt),
            Box::new(LaneKind::Bitcast), // bitcast i64 to {i64,i64}
        ));

        // ── Bool ⇄ Char ──────────────────────────────────────────
        self.set_lane("Bool", "Char", LaneKind::ZExt);
        self.set_lane("Char", "Bool", LaneKind::Trunc);
        // ── Bool ⇄ Data ──────────────────────────────────────────
        self.set_lane("Bool", "Data", LaneKind::Chain(
            Box::new(LaneKind::ZExt),
            Box::new(LaneKind::IntToPtr),
        ));
        self.set_lane("Data", "Bool", LaneKind::Chain(
            Box::new(LaneKind::PtrToInt),
            Box::new(LaneKind::Trunc),
        ));

        // ── Char ⇄ Data ──────────────────────────────────────────
        self.set_lane("Char", "Data", LaneKind::Chain(
            Box::new(LaneKind::ZExt),
            Box::new(LaneKind::IntToPtr),
        ));
        self.set_lane("Data", "Char", LaneKind::Chain(
            Box::new(LaneKind::PtrToInt),
            Box::new(LaneKind::Trunc),
        ));
    }

    /// Seed default variant names per category.
    fn seed_defaults(&mut self) {
        self.defaults.insert("String".to_string(), "UTF8".to_string());
        self.defaults.insert("Float".to_string(), "IEEE754".to_string());
        self.defaults.insert("Char".to_string(), "unicode".to_string());
    }

    /// Seed hardcoded protocol (category, variant) → LLVM type mappings.
    /// 2026-07-30: Replaces the normalizer's three-phase llvm_type derivation
    /// and the disamb metadata hack. Protocol variants are first-class graph nodes.
    fn seed_protocol_llvm_types(&mut self) {
        // Base protocols
        self.set_llvm_type("Bit", "",    LlvmTypeResolver::WidthParametric);
        self.set_llvm_type("Int", "",    LlvmTypeResolver::WidthParametric);
        self.set_llvm_type("UInt", "",   LlvmTypeResolver::WidthParametric);
        self.set_llvm_type("Float", "",  LlvmTypeResolver::Fixed("float"));
        self.set_llvm_type("Bool", "",   LlvmTypeResolver::Fixed("i8"));
        self.set_llvm_type("Char", "",   LlvmTypeResolver::Fixed("i32"));
        // 2026-08-01 (B0): A Brief String value IS a pointer to a
        // length-prefixed [len: i64][bytes] buffer, in every type-claiming
        // site. The old { i64, i64 } fat-pointer claim caused the 4-way
        // representation split-brain (ptr vs i64 vs {i64,i64} vs i128). The
        // casting graph is the single source of truth, so it now says ptr.
        self.set_llvm_type("String", "", LlvmTypeResolver::Fixed("ptr"));
        self.set_llvm_type("Data", "",   LlvmTypeResolver::Fixed("ptr"));

        // Float protocol variants (hardcoded — no disamb hack)
        self.set_llvm_type("Float", "IEEE754",  LlvmTypeResolver::Fixed("float"));
        self.set_llvm_type("Float", "Half",     LlvmTypeResolver::Fixed("half"));
        self.set_llvm_type("Float", "BFloat",   LlvmTypeResolver::Fixed("bfloat"));
        self.set_llvm_type("Float", "Double",   LlvmTypeResolver::Fixed("double"));
        self.set_llvm_type("Float", "FP128",    LlvmTypeResolver::Fixed("fp128"));
        self.set_llvm_type("Float", "X86_FP80", LlvmTypeResolver::Fixed("x86_fp80"));
        // 2026-08-03: C-boundary Float widths (FFI). #Float<C_Float> is the
        // C `float` (32-bit), #Float<C_Double> the C `double` (64-bit) — the
        // boundary types in lib/glue/c.bv declare these variants so an export
        // can request the exact ABI width instead of the default float32.
        self.set_llvm_type("Float", "C_Float",  LlvmTypeResolver::Fixed("float"));
        self.set_llvm_type("Float", "C_Double", LlvmTypeResolver::Fixed("double"));

        // String protocol variants — all encode as ptr to [len][bytes] (B0).
        // The default #String variant is UTF8 (seed_defaults); ASCII and any
        // future sub-protocols keep the same pointer representation.
        self.set_llvm_type("String", "UTF8",  LlvmTypeResolver::Fixed("ptr"));
        self.set_llvm_type("String", "ASCII", LlvmTypeResolver::Fixed("ptr"));
    }

    /// Insert a protocol (category, variant) → LLVM type resolver entry.
    fn set_llvm_type(&mut self, category: &'static str, variant: &'static str, resolver: LlvmTypeResolver) {
        self.protocol_llvm_types.insert((category.to_string(), variant.to_string()), resolver);
    }

    /// Get the LLVM type resolver for a (category, variant) pair.
    pub fn get_llvm_type(&self, category: &str, variant: &str) -> Option<&LlvmTypeResolver> {
        self.protocol_llvm_types.get(&(category.to_string(), variant.to_string()))
    }

    /// Insert a base lane between two protocol categories.
    fn set_lane(&mut self, src: &'static str, dst: &'static str, lane: LaneKind) {
        self.base_lanes
            .entry(src.to_string())
            .or_default()
            .insert(dst.to_string(), lane);
    }

    /// Get the lane from src_category to dst_category, if one exists.
    pub fn get_lane(&self, src_category: &str, dst_category: &str) -> Option<&LaneKind> {
        self.base_lanes
            .get(src_category)
            .and_then(|inner| inner.get(dst_category))
    }

    /// Get the default variant for a category (empty string if none).
    pub fn default_variant(&self, category: &str) -> &str {
        self.defaults.get(category).map(|s| s.as_str()).unwrap_or("")
    }

    // ── Proto Declaration Registration ────────────────────────────────

    /// Register a ProtocolDef item (proto declaration) into the graph.
    /// Adds variant edges, reverse edges, and cross-variant op overrides.
    pub fn register_protocol_def(&mut self, pd: &ProtocolDef) {
        let key = (pd.category.clone(), pd.name.clone());

        for edge in &pd.cast_edges {
            // 2026-08-03: a CastBinding is the delta transform. CastTo binds
            // proto → target (e.g. cstr_to_brief); CastFrom binds target →
            // proto (e.g. str_to_c). Each edge gets the binding's function as
            // its lane so emit_cast_steps emits a real call, not a bitcast.
            let lane = match &edge.binding {
                Some(b) => LaneKind::ExtCallDyn(b.fn_name.clone()),
                None => LaneKind::Bitcast, // placeholder for unbound edges
            };
            // Forward edge (proto → target). For a CastFrom-only declaration
            // there is no proto→target transform, so skip the forward edge
            // (the reverse edge below carries the CastFrom binding).
            if edge.direction == CastDirection::CastTo {
                let step = CastStep {
                    lane: lane.clone(),
                    src_category: key.0.clone(),
                    src_variant: key.1.clone(),
                    dst_category: edge.target_category.clone(),
                    dst_variant: edge.target_variant.clone(),
                };
                self.variant_edges.entry(key.clone()).or_default().push(step);
            }

            // Reverse edge from CastFrom: the CastFrom binding converts
            // target → proto (str_to_c: UTF8 → C_String). Stored under the
            // TARGET key; BFS from the target follows it to the proto. The
            // step's src is the PROTO (the neighbor reached), dst the target
            // (the current node) — the lane still transforms current→neighbor.
            if edge.direction == CastDirection::CastFrom {
                let rev_step = CastStep {
                    lane,
                    src_category: key.0.clone(),
                    src_variant: key.1.clone(),
                    dst_category: edge.target_category.clone(),
                    dst_variant: edge.target_variant.clone(),
                };
                self.variant_reverse
                    .entry((edge.target_category.clone(), edge.target_variant.clone()))
                    .or_default()
                    .push(rev_step);
            }
        }
    }

    // ── Type-Level CastFrom(#Bit) Override Registration ────────────────

    /// Register a type-level CastFrom(#Bit) override.
    /// `type_name` → `function_name` for constructing the type from raw bits.
    pub fn register_cast_from_bit(&mut self, type_name: &str, function_name: &str) {
        self.cast_from_bit_overrides
            .insert(type_name.to_string(), function_name.to_string());
    }

    /// Check if a type has a CastFrom(#Bit) override.
    pub fn get_cast_from_bit(&self, type_name: &str) -> Option<&str> {
        self.cast_from_bit_overrides.get(type_name).map(|s| s.as_str())
    }

    // ── Path Resolution ────────────────────────────────────────────────

    /// Find a protocol cast path from (src_cat, src_var) to (dst_cat, dst_var).
    ///
    /// Returns the sequence of CastSteps if a path exists. For base→base
    /// (no variants), this is O(1) — direct lane lookup. For variant→variant
    /// or variant→base, BFS through variant edges + default fallbacks.
    pub fn find_path(
        &self,
        src_cat: &str,
        src_var: &str,
        dst_cat: &str,
        dst_var: &str,
    ) -> Option<Vec<CastStep>> {
        // Fast path: both are base protocols with no variants
        if src_var.is_empty() && dst_var.is_empty() {
            return self.find_base_path(src_cat, dst_cat);
        }

        // BFS through variant edges + base lanes
        self.bfs_path(src_cat, src_var, dst_cat, dst_var)
    }

    /// O(1) direct lane lookup between two base protocol categories.
    fn find_base_path(&self, src_cat: &str, dst_cat: &str) -> Option<Vec<CastStep>> {
        if src_cat == dst_cat {
            return Some(vec![]); // identity
        }
        let lane = self.get_lane(src_cat, dst_cat)?;
        Some(vec![CastStep {
            lane: lane.clone(),
            src_category: src_cat.to_string(),
            src_variant: String::new(),
            dst_category: dst_cat.to_string(),
            dst_variant: String::new(),
        }])
    }

    /// BFS through variant edges + base lane fallback for the last hop.
    fn bfs_path(
        &self,
        src_cat: &str,
        src_var: &str,
        dst_cat: &str,
        dst_var: &str,
    ) -> Option<Vec<CastStep>> {
        let start = (src_cat.to_string(), src_var.to_string());
        let target = (dst_cat.to_string(), dst_var.to_string());

        let mut visited: HashSet<(String, String)> = HashSet::new();
        let mut queue: VecDeque<((String, String), Vec<CastStep>)> = VecDeque::new();

        visited.insert(start.clone());
        queue.push_back((start, vec![]));

        while let Some((current, path)) = queue.pop_front() {
            // Direct target match (variant→variant within same category)
            if current == target {
                return Some(path);
            }

            // Check if we can reach the target via a single base lane
            if current.1.is_empty() || current.1 == *self.default_variant(&current.0) {
                if let Some(lane) = self.get_lane(&current.0, dst_cat) {
                    if dst_var.is_empty() || dst_var == current.1 {
                        let mut full_path = path.clone();
                        full_path.push(CastStep {
                            lane: lane.clone(),
                            src_category: current.0.clone(),
                            src_variant: current.1.clone(),
                            dst_category: dst_cat.to_string(),
                            dst_variant: dst_var.to_string(),
                        });
                        return Some(full_path);
                    }
                }
            }

            // Follow variant edges
            if let Some(edges) = self.variant_edges.get(&current) {
                for edge in edges {
                    let neighbor = (edge.dst_category.clone(), edge.dst_variant.clone());
                    if visited.insert(neighbor.clone()) {
                        let mut new_path = path.clone();
                        new_path.push(edge.clone());
                        queue.push_back((neighbor, new_path));
                    }
                }
            }

            // Follow reverse edges
            if let Some(edges) = self.variant_reverse.get(&current) {
                for edge in edges {
                    let neighbor = (edge.src_category.clone(), edge.src_variant.clone());
                    if visited.insert(neighbor.clone()) {
                        let mut new_path = path.clone();
                        new_path.push(edge.clone());
                        queue.push_back((neighbor, new_path));
                    }
                }
            }

            // Fallback: try default variant of current category
            if let Some(default_var) = self.defaults.get(&current.0) {
                if current.1 != *default_var {
                    let default_target = (current.0.clone(), default_var.clone());
                    if visited.insert(default_target.clone()) {
                        queue.push_back((default_target, path.clone()));
                    }
                }
            }
        }

        None
    }

    // ── Type-to-Protocol Resolution ────────────────────────────────────

    /// Map a Type to its (protocol_category, variant) for graph lookup.
    /// Uses TypeUniverse protocol membership properties rather than type name matching
    /// (per AGENTS.md Rule 18: NO TYPE NAME MATCHING).
    ///
    /// Compiler constructs not stored in the universe (Bits, Ptr, Void, HashWord) are
    /// handled directly as permitted exceptions (Rule 18a).
    pub fn type_to_protocol(&self, universe: &TypeUniverse, ty: &Type) -> (String, String) {
        match ty {
            // Compiler constructs (not in universe) — permitted direct handling per Rule 18a.
            Type::Bits(_) => return ("Bit".to_string(), String::new()),
            Type::Void => return ("Bit".to_string(), String::new()),
            // 2026-07-30: Ptr<T> deliberately NOT mapped to "Data" here.
            // Mapping Ptr→Data would cause is_protocol_member(Ptr, "#Data")
            // to return true, breaking adapt_to_i64 which expects Ptr fields
            // (stored as i64 in %State) to NOT undergo ptrtoint conversion.
            // resolve_llvm_type() handles Ptr directly before calling this.
            // Type::Ptr(_) => ("Data", ...) moved to resolve_llvm_type only.
            Type::HashWord(name) => {
                // 2026-08-01 (B2): strip the `#` prefix so the category key
                // matches the graph's bare base-lane keys ("Bit", "String").
                // Without this, find_path(HashWord("#Bit"), ...) looked up
                // category "#Bit" which has no lanes — casts to/from #Bit
                // silently fell through to LLVM coercion (e.g. `s as #Bit`
                // emitted `bitcast i64 ptr` — invalid). is_protocol_member
                // already strips the target's `#` before comparing, so the
                // bare category is the consistent representation.
                let bare = name.strip_prefix('#').unwrap_or(name);
                return (bare.to_string(), String::new());
            }
            Type::HashWordVariant(name, variant) => {
                let bare = name.strip_prefix('#').unwrap_or(name);
                return (bare.to_string(), variant.clone());
            }
            Type::Custom(..) | Type::Applied(..) => {} // fall through to universe lookup
            _ => return ("Bit".to_string(), String::new()),
        }

        // Resolve protocol category from universe properties.
        // 2026-07-30: Queries Cast.#<Category> properties instead of matching type names.
        // Checking order: Float → UInt → Int → String → Bool → Char → Data → Bit (universal fallback).
        let key = ty.universe_key().and_then(|k| universe.get(k));
        let rt = match key {
            Some(rt) => rt,
            None => return ("Bit".to_string(), String::new()),
        };

        if rt.properties.contains_key("Cast.#Float") {
            ("Float".to_string(), String::new())
        } else if rt.properties.contains_key("Cast.#UInt") {
            ("UInt".to_string(), String::new())
        } else if rt.properties.contains_key("Cast.#Int") {
            ("Int".to_string(), String::new())
        } else if rt.properties.contains_key("Cast.#String") {
            ("String".to_string(), String::new())
        } else if rt.properties.contains_key("Cast.#Bool") {
            ("Bool".to_string(), String::new())
        } else if rt.properties.contains_key("Cast.#Char") {
            ("Char".to_string(), String::new())
        } else if rt.properties.contains_key("Cast.#Data") {
            ("Data".to_string(), String::new())
        } else {
            // 2026-08-01 (B2): no Cast.# property (the normalizer no longer
            // injects them) — follow the type's declared `base` parent
            // (`type Latin1String: #String` ⇒ base "String"). This makes
            // subtypes resolve to their protocol category so the casting
            // graph's lanes (e.g. #Bit → #String encoding door with a
            // CastFrom(#Bit) override) apply to them. General: walks the
            // base chain, never matches specific type names (rule #18).
            // 2026-08-03: variant bases (`type CStr: #String<C_String>` ⇒
            // base "#String<C_String>") now resolve to (category, variant) —
            // previously the variant form fell through to (Bit, "").
            if let Some((cat, var)) = Self::parse_protocol_base(&rt.base) {
                match cat.as_str() {
                    "Float" | "UInt" | "Int" | "String" | "Bool" | "Char" | "Data" => {
                        return (cat, var);
                    }
                    _ => return ("Bit".to_string(), String::new()),
                }
            }
            ("Bit".to_string(), String::new())
        }
    }

    /// Parse a protocol base string (`#Cat`, `#Cat<Variant>`, or bare `Cat`)
    /// into `(category, variant)`. Returns None for empty/unparseable strings.
    /// 2026-08-03: `#String<C_String>` → `("String", "CString")`; `#String` →
    /// `("String", "")`.
    fn parse_protocol_base(base: &str) -> Option<(String, String)> {
        let b = base.trim_start_matches('#');
        if let Some(lt) = b.find('<') {
            let cat = b[..lt].to_string();
            let variant = b[lt + 1..].trim_end_matches('>').to_string();
            if !cat.is_empty() {
                return Some((cat, variant));
            }
        } else if !b.is_empty() {
            return Some((b.to_string(), String::new()));
        }
        None
    }

    // ── LLVM Type Resolution ──────────────────────────────────────────

    /// Resolve the LLVM type string for a given Brief type.
    ///
    /// Derived from (protocol, metadata) by the casting graph. This replaces
    /// the normalizer's three-phase llvm_type derivation and primordial
    /// llvm_type properties.
    ///
    /// Width resolution priority (WidthParametric protocols):
    /// 1. `!> bits: N` — exact width (hard contract)
    /// 2. `!> maxbits: N` — upper bound
    /// 3. `!> minbits: N` — lower bound
    /// 4. `int_bits` — target default (64 for x86_64, 32 for wasm32)
    pub fn resolve_llvm_type(&self, universe: &TypeUniverse, ty: &Type, int_bits: u64) -> String {
        // Compiler constructs handled directly
        match ty {
            Type::Ptr(_) | Type::PtrConst(_) => return "ptr".to_string(),
            Type::Bits(n) => return format!("i{}", n * 8),
            Type::Void => return "void".to_string(),
            _ => {}
        }

        let (category, variant) = self.type_to_protocol(universe, ty);
        // 2026-08-03: an unseeded `#Category<Variant>` falls back to the
        // category's default variant, then the base category — a `#String`
        // sub-protocol IS a String (ptr); only its encoding differs. Without
        // this, `type CStr: #String<C_String>` resolved to `i64`.
        let resolver = self.get_llvm_type(&category, &variant)
            .or_else(|| self.get_llvm_type(&category, self.default_variant(&category)))
            .or_else(|| self.get_llvm_type(&category, ""));
        match resolver {
            Some(LlvmTypeResolver::Fixed(ty_str)) => return ty_str.to_string(),
            Some(LlvmTypeResolver::WidthParametric) => {
                // Check metadata from universe entry
                let key = ty.universe_key().and_then(|k| universe.get(k));
                let bits = key.and_then(|rt| {
                    // Priority: !> bits → !> maxbits → !> minbits
                    rt.properties.get("bits")
                        .or_else(|| rt.properties.get("maxbits"))
                        .or_else(|| rt.properties.get("minbits"))
                        .and_then(|pv| match pv {
                            PropertyValue::Int(n) => Some(*n as u64),
                            _ => None,
                        })
                }).unwrap_or(int_bits);
                return format!("i{}", bits);
            }
            None => {}
        }

        // Fallback for non-protocol types (plain structs, user types without protocol)
        if let Some(rt) = ty.universe_key().and_then(|k| universe.get(k)) {
            if !rt.fields.is_empty() {
                let field_tys: Vec<String> = rt.fields.iter()
                    .map(|(_, fty)| self.resolve_llvm_type(universe, fty, int_bits))
                    .collect();
                return format!("{{ {} }}", field_tys.join(", "));
            }
        }

        // Ultimate fallback
        "i64".to_string()
    }
}

impl Default for CastingGraph {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ──────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_base_pairs_have_lanes() {
        let graph = CastingGraph::new();
        let protocols = &["Bit", "Int", "UInt", "Float", "String", "Bool", "Char", "Data"];
        for src in protocols {
            for dst in protocols {
                if src == dst {
                    continue; // identity
                }
                let path = graph.find_path(src, "", dst, "");
                assert!(
                    path.is_some(),
                    "missing lane: {} → {}",
                    src,
                    dst
                );
            }
        }
    }

    #[test]
    fn test_identity_path() {
        let graph = CastingGraph::new();
        let path = graph.find_path("Int", "", "Int", "");
        assert!(path.is_some());
        assert_eq!(path.unwrap().len(), 0);
    }

    #[test]
    fn test_int_to_float() {
        let graph = CastingGraph::new();
        let path = graph.find_path("Int", "", "Float", "");
        assert!(path.is_some());
        assert_eq!(path.unwrap().len(), 1);
    }

    #[test]
    fn test_string_to_bit() {
        // 2026-08-01 (B2): #String → #Bit is the CONTENT VIEW — a String value
        // is a ptr to [len][bytes], so the cast yields the buffer address
        // (PtrToInt). Never overridable.
        let graph = CastingGraph::new();
        let path = graph.find_path("String", "", "Bit", "");
        assert!(path.is_some());
        assert_eq!(path.unwrap()[0].lane, LaneKind::PtrToInt);
    }

    #[test]
    fn test_bit_to_string() {
        // 2026-08-01 (B2): #Bit → #String is the ENCODING DOOR — a
        // CastFrom(#Bit) callback (UTF8 wrap default materializing the
        // [len][bytes] header by construction). Not a bitcast.
        let graph = CastingGraph::new();
        let path = graph.find_path("Bit", "", "String", "");
        assert!(path.is_some());
        assert_eq!(path.unwrap()[0].lane, LaneKind::CastFromBitCallback);
    }

    #[test]
    fn test_hashword_category_strip() {
        // 2026-08-01 (B2): a `#Bit` HashWord type must resolve to the bare
        // "Bit" category so find_path finds the base lanes. Previously the
        // category kept the `#` ("#Bit") and every cast to/from #Bit silently
        // fell through to LLVM coercion.
        let graph = CastingGraph::new();
        let u = crate::type_universe::TypeUniverse::new();
        let b_ty = crate::ast::Type::HashWord("#Bit".to_string());
        let (cat, var) = graph.type_to_protocol(&u, &b_ty);
        assert_eq!(cat, "Bit");
        assert_eq!(var, "");
        // And the String → Bit path (via hashword target) resolves to PtrToInt.
        let path = graph.find_path("String", "", &cat, &var);
        assert!(path.is_some());
        assert_eq!(path.unwrap()[0].lane, LaneKind::PtrToInt);
    }

    #[test]
    fn test_default_variants() {
        let graph = CastingGraph::new();
        assert_eq!(graph.default_variant("String"), "UTF8");
        assert_eq!(graph.default_variant("Float"), "IEEE754");
        assert_eq!(graph.default_variant("Int"), "");
    }

    #[test]
    fn test_variant_edge() {
        let mut graph = CastingGraph::new();
        // Simulate proto ASCII: #String { CastTo(#String): ascii_to_utf8(#L); }
        graph.register_protocol_def(&ProtocolDef {
            name: "ASCII".to_string(),
            category: "String".to_string(),
            contract: None,
            cast_edges: vec![crate::ast::top::CastEdge {
                direction: crate::ast::top::CastDirection::CastTo,
                target_category: "String".to_string(),
                target_variant: "UTF8".to_string(),
                binding: None,
            }],
            cross_ops: vec![],
            span: None,
        });

        let path = graph.find_path("String", "ASCII", "String", "UTF8");
        assert!(path.is_some());
        assert_eq!(path.unwrap().len(), 1);
    }

    #[test]
    fn test_cast_from_bit_override() {
        let mut graph = CastingGraph::new();
        graph.register_cast_from_bit("MyString", "construct_from_bits");
        assert_eq!(graph.get_cast_from_bit("MyString"), Some("construct_from_bits"));
        assert_eq!(graph.get_cast_from_bit("Other"), None);
    }

    #[test]
    fn test_type_to_protocol_primitives() {
        let graph = CastingGraph::new();
        let universe = crate::type_universe::TypeUniverse::new();        // Compiler constructs (no universe needed)
        assert_eq!(graph.type_to_protocol(&universe, &Type::Bits(42)), ("Bit".to_string(), String::new()));
        // 2026-07-30: Ptr<T> is no longer mapped to Data — it's not in
        // type_to_protocol. resolve_llvm_type handles Ptr directly.
        // Ptr fields are stored as i64 in %State to avoid ptrtoint conversion.

        // Universe-resolved types (seeded primordials)
        assert_eq!(graph.type_to_protocol(&universe, &Type::Custom("Int".to_string())), ("Int".to_string(), String::new()));
        assert_eq!(graph.type_to_protocol(&universe, &Type::Custom("Float".to_string())), ("Float".to_string(), String::new()));
        assert_eq!(graph.type_to_protocol(&universe, &Type::Custom("Bool".to_string())), ("Bool".to_string(), String::new()));
        assert_eq!(graph.type_to_protocol(&universe, &Type::Custom("Data".to_string())), ("Data".to_string(), String::new()));
        // Fallback — no Cast.# properties for unknown types → Bit
        assert_eq!(graph.type_to_protocol(&universe, &Type::Custom("UnknownType".to_string())), ("Bit".to_string(), String::new()));
    }

    #[test]
    fn test_proto_binding_becomes_ext_call_lane() {
        // 2026-08-03: `proto C_String: #String { CastTo(#String<UTF8>) =
        // cstr_to_brief(#L); CastFrom(#String<UTF8>) = str_to_c(#L); }` — the
        // bindings must become real call lanes (ExtCallDyn), not the old
        // Bitcast placeholder.
        let mut graph = CastingGraph::new();
        graph.register_protocol_def(&ProtocolDef {
            name: "C_String".to_string(),
            category: "String".to_string(),
            contract: None,
            cast_edges: vec![
                crate::ast::top::CastEdge {
                    direction: crate::ast::top::CastDirection::CastTo,
                    target_category: "String".to_string(),
                    target_variant: "UTF8".to_string(),
                    binding: Some(crate::ast::top::CastBinding {
                        fn_name: "cstr_to_brief".to_string(),
                        param: "#L".to_string(),
                    }),
                },
                crate::ast::top::CastEdge {
                    direction: crate::ast::top::CastDirection::CastFrom,
                    target_category: "String".to_string(),
                    target_variant: "UTF8".to_string(),
                    binding: Some(crate::ast::top::CastBinding {
                        fn_name: "str_to_c".to_string(),
                        param: "#L".to_string(),
                    }),
                },
            ],
            cross_ops: vec![],
            span: None,
        });

        // C_String → UTF8 uses the CastTo binding.
        let path = graph.find_path("String", "C_String", "String", "UTF8")
            .expect("C_String -> UTF8 path");
        assert_eq!(path.len(), 1);
        assert_eq!(path[0].lane, LaneKind::ExtCallDyn("cstr_to_brief".to_string()));

        // UTF8 → C_String uses the CastFrom binding.
        let rev = graph.find_path("String", "UTF8", "String", "C_String")
            .expect("UTF8 -> C_String path");
        assert_eq!(rev.len(), 1);
        assert_eq!(rev[0].lane, LaneKind::ExtCallDyn("str_to_c".to_string()));
    }

    #[test]
    fn test_resolve_llvm_type_variant_fallback() {
        // 2026-08-03: unseeded `#String<C_String>` must resolve like any other
        // String (ptr), not fall through to i64; `#Float<C_Double>` → double.
        let graph = CastingGraph::new();
        let universe = crate::type_universe::TypeUniverse::new();
        assert_eq!(
            graph.resolve_llvm_type(&universe, &Type::HashWordVariant("#String".into(), "C_String".into()), 64),
            "ptr"
        );
        assert_eq!(
            graph.resolve_llvm_type(&universe, &Type::HashWordVariant("#Float".into(), "C_Double".into()), 64),
            "double"
        );
        assert_eq!(
            graph.resolve_llvm_type(&universe, &Type::HashWordVariant("#String".into(), "UTF8".into()), 64),
            "ptr"
        );
        assert_eq!(
            graph.resolve_llvm_type(&universe, &Type::HashWordVariant("#Float".into(), "Double".into()), 64),
            "double"
        );
    }

    #[test]
    fn test_parse_protocol_base_variants() {        // 2026-08-03: variant bases — the FFI boundary types declare
        // `type CStr: #String<C_String>` ⇒ base "#String<C_String>".
        assert_eq!(CastingGraph::parse_protocol_base("#String<C_String>"),
            Some(("String".to_string(), "C_String".to_string())));
        assert_eq!(CastingGraph::parse_protocol_base("#Float<C_Double>"),
            Some(("Float".to_string(), "C_Double".to_string())));
        assert_eq!(CastingGraph::parse_protocol_base("#Int<C_I32>"),
            Some(("Int".to_string(), "C_I32".to_string())));
        assert_eq!(CastingGraph::parse_protocol_base("#String"),
            Some(("String".to_string(), String::new())));
        assert_eq!(CastingGraph::parse_protocol_base("String"),
            Some(("String".to_string(), String::new())));
        assert_eq!(CastingGraph::parse_protocol_base(""), None);
        assert_eq!(CastingGraph::parse_protocol_base("<"), None);
    }
}
