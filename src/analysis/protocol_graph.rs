// 2026-07-23: Protocol Graph — variant-aware edge resolution for CastTo/CastFrom.
// Built from TopLevel::ProtocolDef items and TypeDef.protocol fields.
// Used by find_cast_path BFS as additional edge source alongside the TypeUniverse.

use std::collections::{HashMap, VecDeque};
use crate::ast::top::{CastDirection, CastEdge, OperatorDef, ProtocolDef, TopLevel};
use crate::ast::Contract;
use crate::type_universe::TypeUniverse;

/// Protocol graph: variant-aware edges between protocol categories.
/// Stores CastTo/CastFrom edges, cross-variant op overrides, and optional contracts.
#[derive(Debug, Clone)]
pub struct ProtocolGraph {
    /// (category, variant) → outgoing Cast edges
    edges: HashMap<(String, String), Vec<CastEdge>>,
    /// Reverse edges: (category, variant) → incoming Cast edges
    /// Built from CastFrom edges — if Y has CastFrom(X), then X → Y.
    reverse_edges: HashMap<(String, String), Vec<CastEdge>>,
    /// Pre-registered primordial defaults: category → default variant
    defaults: HashMap<String, String>,
    /// Cross-variant op overrides: (self_variant, op_name, target_variant) → function name
    cross_ops: HashMap<(String, String, String), String>,
    /// Optional contracts per variant: (category, variant) → Contract
    contracts: HashMap<(String, String), Contract>,
}

impl ProtocolGraph {
    /// Create a new protocol graph with primordial defaults.
    pub fn new() -> Self {
        let mut defaults = HashMap::new();
        defaults.insert("String".to_string(), "utf8".to_string());
        defaults.insert("Float".to_string(), "ieee754".to_string());
        defaults.insert("Char".to_string(), "unicode".to_string());
        ProtocolGraph {
            edges: HashMap::new(),
            reverse_edges: HashMap::new(),
            defaults,
            cross_ops: HashMap::new(),
            contracts: HashMap::new(),
        }
    }

    /// Build the graph from AST top-level items.
    /// Scans for ProtocolDef items and TypeDef items with protocol fields.
    pub fn build_from(items: &[TopLevel]) -> Self {
        let mut graph = Self::new();

        for item in items {
            match item {
                TopLevel::ProtocolDef(pd) => {
                    graph.register_protocol_def(pd);
                }
                // 2026-07-24: TypeDef.protocol field creates an implicit CastTo edge.
                TopLevel::TypeDef(td) => {
                    if let Some(ref proto) = td.protocol {
                        let cat = proto.strip_prefix('#').unwrap_or(proto).to_string();
                        // Register implicit CastTo(#Category) from the type's name to the protocol
                        let edge = CastEdge {
                            direction: CastDirection::CastTo,
                            target_category: cat.clone(),
                            target_variant: graph.default_variant(&cat),
                            binding: None,
                        };
                        graph.edges
                            .entry((td.name.clone(), String::new()))
                            .or_default()
                            .push(edge);
                    }
                }
                _ => {}
            }
        }

        graph
    }

    /// Register a ProtocolDef declaration into the graph.
    fn register_protocol_def(&mut self, pd: &ProtocolDef) {
        let key = (pd.category.clone(), pd.name.clone());

        // Register cast edges and reverse edges
        for edge in &pd.cast_edges {
            // Forward edge: key → target
            self.edges.entry(key.clone()).or_default().push(edge.clone());

            // Reverse edge from CastFrom: if key has CastFrom(X), then X → key
            if edge.direction == CastDirection::CastFrom {
                let rev_edge = CastEdge {
                    direction: CastDirection::CastTo,
                    target_category: key.0.clone(),
                    target_variant: key.1.clone(),
                    binding: None,
                };
                let rev_key = (edge.target_category.clone(), edge.target_variant.clone());
                self.reverse_edges.entry(rev_key).or_default().push(rev_edge);
            }
        }

        // Register contract if present
        if let Some(ref contract) = pd.contract {
            self.contracts.insert(key.clone(), contract.clone());
        }

        // Register cross-variant op overrides
        for op in &pd.cross_ops {
            // Cross-ops reference a target variant: op Add(#Target<variant>) = fn(#L, #R)
            for param in &op.params {
                if let crate::ast::Type::HashWordVariant(cat, var) = param {
                    let cat = cat.strip_prefix('#').unwrap_or(cat).to_string();
                    let cross_key = (pd.name.clone(), op.op.clone(), var.clone());
                    if let Some(ref args) = op.impl_args {
                        let fn_name = format!("{:?}", args);
                        self.cross_ops.insert(cross_key, fn_name);
                    }
                }
            }
        }
    }

    /// Get the default variant for a category.
    pub fn default_variant(&self, category: &str) -> String {
        self.defaults.get(category).cloned().unwrap_or_default()
    }

    /// Find a protocol path from (source_cat, source_var) to (target_cat, target_var).
    /// Returns the sequence of CastEdge steps if a path exists.
    pub fn find_protocol_path(
        &self,
        source_cat: &str,
        source_var: &str,
        target_cat: &str,
        target_var: &str,
    ) -> Option<Vec<CastEdge>> {
        // BFS through (category, variant) nodes
        let mut visited = std::collections::HashSet::new();
        let mut queue: VecDeque<((String, String), Vec<CastEdge>)> = VecDeque::new();

        let start = (source_cat.to_string(), source_var.to_string());
        let target = (target_cat.to_string(), target_var.to_string());

        visited.insert(start.clone());
        queue.push_back((start, vec![]));

        while let Some((current, path)) = queue.pop_front() {
            if current == target {
                return Some(path);
            }

            // Find outgoing edges from this node (forward CastTo + reverse CastFrom)
            if let Some(edges) = self.edges_for(&current.0, &current.1) {
                for edge in &edges {
                    let neighbor = (edge.target_category.clone(), edge.target_variant.clone());
                    if visited.insert(neighbor.clone()) {
                        let mut new_path = path.clone();
                        new_path.push(edge.clone());
                        queue.push_back((neighbor, new_path));
                    }
                }
            }

            // Try the default variant of the same category as fallback
            if let Some(default_var) = self.defaults.get(&current.0) {
                let default_target = (current.0.clone(), default_var.clone());
                if current.1 != *default_var && visited.insert(default_target.clone()) {
                    let edge = CastEdge {
                        direction: CastDirection::CastTo,
                        target_category: default_target.0.clone(),
                        target_variant: default_target.1.clone(),
                        binding: None,
                    };
                    let mut new_path = path.clone();
                    new_path.push(edge);
                    queue.push_back((default_target, new_path));
                }
            }
        }

        None
    }

    /// Find the delegation path from a variant to its category's default.
    pub fn find_default_delegation(&self, category: &str, variant: &str) -> Option<Vec<CastEdge>> {
        let default_var = self.default_variant(category);
        if variant == default_var {
            return Some(vec![]); // Already at default
        }
        self.find_protocol_path(category, variant, category, &default_var)
    }

    /// Get outgoing edges for a (category, variant) pair (forward + reverse).
    pub fn edges_for(&self, category: &str, variant: &str) -> Option<Vec<CastEdge>> {
        let key = (category.to_string(), variant.to_string());
        let mut all = Vec::new();
        if let Some(fwd) = self.edges.get(&key) {
            all.extend(fwd.iter().cloned());
        }
        if let Some(rev) = self.reverse_edges.get(&key) {
            all.extend(rev.iter().cloned());
        }
        if all.is_empty() { None } else { Some(all) }
    }

    /// Get the contract for a (category, variant) pair, if any.
    pub fn get_contract(&self, category: &str, variant: &str) -> Option<&Contract> {
        self.contracts.get(&(category.to_string(), variant.to_string()))
    }

    /// Inject protocol graph edges as Cast.# properties into the TypeUniverse.
    /// This makes the edges visible to the existing find_cast_path BFS.
    ///
    /// Two-pass approach:
    /// 1. Inject edges for explicitly declared variant names.
    /// 2. Propagate edges to all types that participate in a protocol category
    ///    (so `type Foo: #String` auto-inherits CastTo(#String<ascii>), etc.).
    pub fn inject_edges(&self, universe: &mut TypeUniverse) {
        // Pass 1: Inject edges for explicitly named variants
        for ((cat, var), edges) in &self.edges {
            let type_name = if var.is_empty() {
                cat.clone()
            } else {
                format!("#{}<{}>", cat, var)
            };
            if let Some(rt) = universe.types.get_mut(&type_name) {
                for edge in edges {
                    let prop_key = format!(
                        "Cast.{}<{}>",
                        edge.target_category, edge.target_variant
                    );
                    rt.properties.insert(
                        prop_key,
                        crate::ast::PropertyValue::Bool(true),
                    );
                }
            }
        }

        // Pass 2: Propagate edges to types that participate in this protocol
        // category. For each universe type with a Cast.# property, look up
        // the protocol graph and inject additional edges for that category.
        let type_names: Vec<String> = universe.types.keys().cloned().collect();
        for type_name in &type_names {
            let rt = match universe.types.get(type_name) {
                Some(rt) => rt,
                None => continue,
            };
            // Collect the protocol categories this type participates in
            let categories: Vec<String> = rt.properties.keys()
                .filter_map(|k| k.strip_prefix("Cast.#"))
                .filter(|cat| !cat.is_empty() && !cat.contains('<'))
                .map(|cat| cat.to_string())
                .collect();

            if categories.is_empty() {
                continue;
            }

            // For each category, inject edges from the protocol graph
            let rt = universe.types.get_mut(type_name).unwrap();
            for cat in &categories {
                let default_var = self.default_variant(cat);
                if let Some(edges) = self.edges.get(&(cat.clone(), default_var)) {
                    for edge in edges {
                        let prop_key = format!(
                            "Cast.{}<{}>",
                            edge.target_category, edge.target_variant
                        );
                        // Use entry().or_insert() to avoid overwriting
                        // explicit type-level declarations
                        rt.properties.entry(prop_key).or_insert(
                            crate::ast::PropertyValue::Bool(true),
                        );
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_edge(dir: CastDirection, cat: &str, var: &str) -> CastEdge {
        CastEdge {
            direction: dir,
            target_category: cat.to_string(),
            target_variant: var.to_string(),
            binding: None,
        }
    }

    #[test]
    fn test_graph_single_edge() {
        let mut graph = ProtocolGraph::new();
        let key = ("String".to_string(), "ascii".to_string());
        graph.edges.insert(key, vec![make_edge(CastDirection::CastTo, "String", "utf8")]);

        let path = graph.find_protocol_path("String", "ascii", "String", "utf8");
        assert!(path.is_some());
        assert_eq!(path.unwrap().len(), 1);
    }

    #[test]
    fn test_graph_multi_hop() {
        let mut graph = ProtocolGraph::new();
        let ascii_key = ("String".to_string(), "ascii".to_string());
        graph.edges.insert(ascii_key, vec![make_edge(CastDirection::CastTo, "String", "utf8")]);

        let utf16_key = ("String".to_string(), "utf16".to_string());
        graph.edges.insert(utf16_key.clone(), vec![make_edge(CastDirection::CastFrom, "String", "utf8")]);
        // CastFrom(utf8) on utf16 means utf8 → utf16 is a reverse edge
        graph.reverse_edges.entry(
            ("String".to_string(), "utf8".to_string())
        ).or_default().push(CastEdge {
            direction: CastDirection::CastTo,
            target_category: "String".to_string(),
            target_variant: "utf16".to_string(),
            binding: None,
        });

        let path = graph.find_protocol_path("String", "ascii", "String", "utf16");
        assert!(path.is_some(), "path should exist: ascii → utf8 → utf16");
        let steps = path.unwrap();
        assert_eq!(steps.len(), 2);
    }

    #[test]
    fn test_graph_default_delegation() {
        let mut graph = ProtocolGraph::new();
        let key = ("String".to_string(), "ascii".to_string());
        graph.edges.insert(key, vec![make_edge(CastDirection::CastTo, "String", "utf8")]);

        let path = graph.find_default_delegation("String", "ascii");
        assert!(path.is_some());
    }

    #[test]
    fn test_graph_no_path() {
        let graph = ProtocolGraph::new();
        // No edges registered between these
        let path = graph.find_protocol_path("String", "ascii", "Float", "ieee754");
        assert!(path.is_none());
    }

    #[test]
    fn test_graph_cross_op_lookup() {
        let mut graph = ProtocolGraph::new();
        graph.cross_ops.insert(
            ("ascii".to_string(), "Add".to_string(), "utf8".to_string()),
            "add_utf8_to_ascii".to_string(),
        );
    }

    #[test]
    fn test_graph_build_from_protocol_def() {
        let items = vec![
            TopLevel::ProtocolDef(ProtocolDef {
                name: "ascii".to_string(),
                category: "String".to_string(),
                contract: None,
                cast_edges: vec![make_edge(CastDirection::CastTo, "String", "utf8")],
                cross_ops: vec![],
                span: None,
            }),
        ];

        let graph = ProtocolGraph::build_from(&items);
        let path = graph.find_protocol_path("String", "ascii", "String", "utf8");
        assert!(path.is_some());
    }

    #[test]
    fn test_graph_primordial_defaults() {
        let graph = ProtocolGraph::new();
        assert_eq!(graph.default_variant("String"), "utf8");
        assert_eq!(graph.default_variant("Float"), "ieee754");
        assert_eq!(graph.default_variant("Char"), "unicode");
    }
}
