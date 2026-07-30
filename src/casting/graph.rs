use std::collections::{HashMap, HashSet, VecDeque};

use crate::ast::top::{CastDirection, ProtocolDef};
use crate::ast::Type;

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
        };
        graph.seed_base_lanes();
        graph.seed_defaults();
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
        // #Bit→#String: CastFrom(#Bit) callback by default; fallback is bitcast i64→{i64,i64}
        self.set_lane("Bit", "String", LaneKind::Bitcast);
        // #String→#Bit: always extractvalue 0 (never overridable)
        self.set_lane("String", "Bit", LaneKind::ExtractData);
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
            // Forward edge
            let step = CastStep {
                lane: LaneKind::Bitcast, // placeholder — real emission determined by binding
                src_category: key.0.clone(),
                src_variant: key.1.clone(),
                dst_category: edge.target_category.clone(),
                dst_variant: edge.target_variant.clone(),
            };
            self.variant_edges.entry(key.clone()).or_default().push(step);

            // Reverse edge from CastFrom
            if edge.direction == CastDirection::CastFrom {
                let rev_step = CastStep {
                    lane: LaneKind::Bitcast,
                    src_category: edge.target_category.clone(),
                    src_variant: edge.target_variant.clone(),
                    dst_category: key.0.clone(),
                    dst_variant: key.1.clone(),
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
    pub fn type_to_protocol(&self, universe: &crate::type_universe::TypeUniverse, ty: &Type) -> (String, String) {
        match ty {
            // Compiler constructs (not in universe) — permitted direct handling per Rule 18a.
            Type::Bits(_) => return ("Bit".to_string(), String::new()),
            Type::Void => return ("Bit".to_string(), String::new()),
            Type::Ptr(_) | Type::PtrConst(_) => return ("Data".to_string(), String::new()),
            Type::HashWord(name) => return (name.clone(), String::new()),
            Type::HashWordVariant(name, variant) => return (name.clone(), variant.clone()),
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
            // Every type is a member of #Bit via Cast.#Bit injection in normalizer.
            ("Bit".to_string(), String::new())
        }
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
        let graph = CastingGraph::new();
        let path = graph.find_path("String", "", "Bit", "");
        assert!(path.is_some());
        assert_eq!(path.unwrap()[0].lane, LaneKind::ExtractData);
    }

    #[test]
    fn test_bit_to_string() {
        let graph = CastingGraph::new();
        let path = graph.find_path("Bit", "", "String", "");
        assert!(path.is_some());
        // Default bit→string is bitcast
        assert_eq!(path.unwrap()[0].lane, LaneKind::Bitcast);
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
        let universe = crate::type_universe::TypeUniverse::new();

        // Compiler constructs (no universe needed)
        assert_eq!(graph.type_to_protocol(&universe, &Type::Bits(42)), ("Bit".to_string(), String::new()));
        assert_eq!(graph.type_to_protocol(&universe, &Type::Ptr(Box::new(Type::Custom("Int".to_string())))), ("Data".to_string(), String::new()));

        // Universe-resolved types (seeded primordials)
        assert_eq!(graph.type_to_protocol(&universe, &Type::Custom("Int".to_string())), ("Int".to_string(), String::new()));
        assert_eq!(graph.type_to_protocol(&universe, &Type::Custom("Float".to_string())), ("Float".to_string(), String::new()));
        assert_eq!(graph.type_to_protocol(&universe, &Type::Custom("Bool".to_string())), ("Bool".to_string(), String::new()));
        assert_eq!(graph.type_to_protocol(&universe, &Type::Custom("Data".to_string())), ("Data".to_string(), String::new()));
        // Fallback — no Cast.# properties for unknown types → Bit
        assert_eq!(graph.type_to_protocol(&universe, &Type::Custom("UnknownType".to_string())), ("Bit".to_string(), String::new()));
    }
}
