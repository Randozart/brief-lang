// ── Frgn Dispatch Resolution ──────────────────────────────────────────
// 2026-07-22: Resolved during the main compilation pass (before codegen),
// not inside the backend. The backend receives a ResolvedFrgn and emits
// the appropriate IR without re-implementing dispatch logic.
//
// Why resolve pre-backend:
//   1. The dispatch decision depends on the protocol graph (type_universe),
//      the GLUE registry, and the backend's capabilities — all available
//      at compile time.
//   2. Backends should not reimplement extension matching or BFS.
//   3. A single error point for "no bridge available" is cleaner than
//      per-backend error messages.
//
// Why NOT resolve in the backend:
//   The backend already knows its own capabilities. The ResolvedFrgn is
//   the intersection of "what the type system says" and "what the backend
//   can do." The backend still validates that it can handle the result.

use std::collections::HashMap;

use crate::ast::top::{Fallback, ForeignBinding};
use crate::glue::config::{find_language_by_extension, GlueTarget};
use crate::target::BackendKind;

/// The dispatch strategy for a single frgn declaration.
///
/// 2026-07-22: Determined during the main compilation pass before any
/// backend runs. The backend receives this and emits the appropriate IR.
#[derive(Debug, Clone)]
pub enum ResolvedFrgn {
    /// Backend inlines directly (compile/link the source, call the symbol)
    Inline {
        /// The foreign symbol name (from `as` or brief_name)
        symbol: String,
        /// If true, the backend should compile this source to .o first
        compile_source: bool,
    },
    /// Route through the GLUE bridge
    Bridge {
        /// Language identifier
        language: String,
        /// Protocol transform chain for each parameter
        param_paths: Vec<ProtocolStep>,
        /// Protocol transform chain for the return value
        return_path: Option<ProtocolStep>,
        /// Fallback strategy
        fallback: Fallback,
    },
    /// Not supported by this backend
    Unsupported(String),
}

/// A single step in a protocol transform chain.
///
/// 2026-07-22: Describes how to go from one type representation to another
/// at the FFI boundary. Multiple steps form a path from Brief type to
/// foreign type (or vice versa).
#[derive(Debug, Clone)]
pub struct ProtocolStep {
    /// The source type in the chain
    pub source: crate::ast::Type,
    /// The target type in the chain
    pub target: crate::ast::Type,
    /// The kind of transform needed
    pub kind: TransformKind,
}

/// Cost category for a protocol transform.
///
/// 2026-07-22: Used by compute_protocol_path() to find the cheapest path.
/// The categories form a partial order: Identity < Bitcast < MeldShuffle < ProtocolTransform.
#[derive(Debug, Clone)]
pub enum TransformKind {
    /// No transform needed — types are structurally identical
    Identity,
    /// Meld shuffle — bit permutation, field reordering
    MeldShuffle,
    /// Protocol transform — CastTo/CastFrom with actual encoding work
    ProtocolTransform(String),
    /// Raw bitcast — implicit Cast(#Bits)
    Bitcast,
}

/// Resolve the dispatch strategy for a single frgn declaration.
///
/// 2026-07-22: Given the frgn declaration, its file extension, the GLUE
/// registry, and the backend kind, determines whether the call can be
/// inlined, needs a GLUE bridge, or is unsupported.
pub fn resolve_single_frgn(
    fb: &ForeignBinding,
    ext: &str,
    glue_targets: &HashMap<String, GlueTarget>,
    backend: BackendKind,
    universe: Option<&crate::type_universe::TypeUniverse>,
) -> Result<ResolvedFrgn, String> {
    // 2026-07-22: Empty extension means the foreign path has no known type —
    // treat as unsupported with a clear error message.
    if ext.is_empty() {
        return Ok(ResolvedFrgn::Unsupported(format!(
            "frgn '{}' has no file extension in its 'from' path. \
             Add a file extension so the compiler can determine the dispatch strategy",
            fb.effective_brief_name()
        )));
    }

    // 2026-07-22: First check if the extension maps to a GLUE language target.
    // If found, this is a bridge candidate.
    let language = find_language_by_extension(glue_targets, ext);

    // 2026-07-22: Check if the extension is inlineable for this backend.
    // Inlineable means the backend can compile the source directly and
    // link the object code. Currently only .c/.cpp for LLVM backend.
    if backend == BackendKind::Llvm && matches!(ext, "c" | "cpp" | "cxx" | "rs") {
        let symbol = fb.foreign_name.clone();
        return Ok(ResolvedFrgn::Inline {
            symbol,
            compile_source: true,
        });
    }

    // 2026-07-22: If a GLUE language target exists, this is a bridge call.
    if let Some(target) = language {
        // 2026-07-22: Compute protocol transform for each parameter (one step each),
        // using the target's protocol mapping to derive the foreign type.
        let param_paths: Vec<ProtocolStep> = fb.inputs.iter()
            .map(|(_, brief_type)| {
                let foreign_type = lookup_foreign_type(brief_type, &target.protocols, universe);
                compute_protocol_path(brief_type, &foreign_type, universe)
                    .and_then(|steps| steps.into_iter().next().ok_or_else(|| "empty path".to_string()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let return_path: Option<ProtocolStep> = fb.success_output.first()
            .and_then(|(_, ty)| {
                let foreign_type = lookup_foreign_type(ty, &target.protocols, universe);
                compute_protocol_path(ty, &foreign_type, universe).ok()?.into_iter().next()
            });
        return Ok(ResolvedFrgn::Bridge {
            language: target.language.clone(),
            param_paths,
            return_path,
            fallback: fb.fallback.clone(),
        });
    }

    // 2026-07-22: Native (Metropolitan) targets always inline since they
    // are already compiled. The backend calls them directly.
    if ext == "native" || ext == "o" || ext == "so" || ext == "a" {
        let symbol = fb.foreign_name.clone();
        return Ok(ResolvedFrgn::Inline {
            symbol,
            compile_source: false,
        });
    }

    Ok(ResolvedFrgn::Unsupported(format!(
        "frgn '{}' from '{}': extension '.{}' is not supported by the {} backend. \
         Add a GLUE registry entry in lib/glue.toml or use a supported extension (.c, .rs, .py, .js, .mjs)",
        fb.effective_brief_name(),
        ext,
        ext,
        match backend {
            BackendKind::Llvm => "LLVM",
            BackendKind::Circt => "CIRCT",
            BackendKind::Webstack => "Webstack",
            BackendKind::Gpu => "GPU",
            BackendKind::Spirv => "SPIR-V",
            BackendKind::Vm => "VM",
        }
    )))
}

/// Compute the protocol path between two types for a frgn boundary.
///
/// 2026-07-22: Uses the existing BFS in find_cast_path() + meld lookup
/// to determine how to transform a Brief type to/from a foreign type.
/// Returns the shortest path by cost.
///
/// Stub: Returns an identity path. Full implementation in Phase 3.
pub fn compute_protocol_path(
    brief_type: &crate::ast::Type,
    _foreign_type: &crate::ast::Type,
    universe: Option<&crate::type_universe::TypeUniverse>,
) -> Result<Vec<ProtocolStep>, String> {
    // 2026-07-22: If types are structurally identical, return identity.
    if brief_type == _foreign_type {
        return Ok(vec![ProtocolStep {
            source: brief_type.clone(),
            target: _foreign_type.clone(),
            kind: TransformKind::Identity,
        }]);
    }

    // 2026-07-22: Use BFS via find_cast_path if universe is available.
    if let Some(u) = universe {
        let brief_key = type_to_key(brief_type);
        let foreign_key = type_to_key(_foreign_type);
        if let Some(path) = crate::analysis::layout_optimizer::find_cast_path(u, &brief_key, &foreign_key, None) {
            let steps = path_to_protocol_steps(&path);
            if !steps.is_empty() {
                return Ok(steps);
            }
        }
    }

    // 2026-07-22: Fallback: return a bitcast path (Cast(#Bits)).
    Ok(vec![ProtocolStep {
        source: brief_type.clone(),
        target: _foreign_type.clone(),
        kind: TransformKind::Bitcast,
    }])
}

/// Look up the foreign protocol category for a Brief type, then map it
/// to a foreign type via the target's protocol mapping.
/// Falls back to the Brief type's universe key if no protocol exists.
fn lookup_foreign_type(
    brief_type: &crate::ast::Type,
    protocols: &std::collections::HashMap<String, crate::glue::config::ProtocolEntry>,
    universe: Option<&crate::type_universe::TypeUniverse>,
) -> crate::ast::Type {
    // Find the protocol category that this Brief type participates in
    if let Some(u) = universe {
        if let Some(key) = brief_type.universe_key() {
            if let Some(rt) = u.get(key) {
                // Look for a CastTo property that points to a protocol category
                for prop_key in rt.properties.keys() {
                    if let Some(cat) = prop_key.strip_prefix("Cast.#") {
                        let protocol_key = format!("#{}", cat);
                        if protocols.contains_key(&protocol_key) {
                            return crate::ast::Type::HashWord(cat.to_string());
                        }
                    }
                }
            }
        }
    }
    // Fallback: derive from the type's name
    match brief_type {
        crate::ast::Type::Custom(name) => {
            let protocol_key = format!("#{}", name);
            if protocols.contains_key(&protocol_key) {
                crate::ast::Type::HashWord(name.clone())
            } else {
                brief_type.clone()
            }
        }
        _ => brief_type.clone(),
    }
}

/// Convert a Type to a string key for use with find_cast_path BFS.
fn type_to_key(ty: &crate::ast::Type) -> String {
    match ty {
        crate::ast::Type::Custom(name) => name.clone(),
        crate::ast::Type::Applied(name, _) => name.clone(),
        crate::ast::Type::Void => "Void".to_string(),
        crate::ast::Type::Ptr(_) => "Ptr".to_string(),
        crate::ast::Type::Bits(w) => format!("Bits({})", w),
        crate::ast::Type::HashWord(name) => format!("#{}", name),
        crate::ast::Type::HashWordVariant(name, var) => format!("#{}<{}>", name, var),
        crate::ast::Type::Tuple(_) => "Tuple".to_string(),
        crate::ast::Type::TypeVar(name) => name.clone(),
        _ => format!("{:?}", ty),
    }
}

/// Convert a path of type names from find_cast_path BFS into ProtocolStep entries.
fn path_to_protocol_steps(path: &[String]) -> Vec<ProtocolStep> {
    if path.len() < 2 {
        return vec![];
    }
    let mut steps = Vec::new();
    for pair in path.windows(2) {
        let kind = if pair[0] == pair[1] {
            TransformKind::Identity
        } else if pair[0] == "#Bits" || pair[1] == "#Bits" {
            TransformKind::Bitcast
        } else {
            TransformKind::ProtocolTransform(pair[1].clone())
        };
        steps.push(ProtocolStep {
            source: crate::ast::Type::Custom(pair[0].clone()),
            target: crate::ast::Type::Custom(pair[1].clone()),
            kind,
        });
    }
    steps
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::top::{ForeignTarget, FromSpec};
    use std::path::PathBuf;

    fn make_frgn(name: &str, ext: &str) -> ForeignBinding {
        ForeignBinding::new(
            name.to_string(),
            None,
            FromSpec::Literal(PathBuf::from(format!("lib.{}", ext))),
            ForeignTarget::Native,
            Fallback::None,
        )
    }

    fn make_frgn_with_as(name: &str, as_name: &str, ext: &str) -> ForeignBinding {
        ForeignBinding::new(
            name.to_string(),
            Some(as_name.to_string()),
            FromSpec::Literal(PathBuf::from(format!("lib.{}", ext))),
            ForeignTarget::Native,
            Fallback::None,
        )
    }

    fn sample_glue_targets() -> HashMap<String, GlueTarget> {
        let mut map = HashMap::new();
        map.insert("python".to_string(), GlueTarget {
            language: "python".to_string(),
            types_module: PathBuf::from("glue/python/types.bv"),
            extension: "py".to_string(),
            bridge_kind: "native_module".to_string(),
            calling_convention: "c_abi".to_string(),
            module_init: false,
            protocols: HashMap::new(),
            templates: HashMap::new(),
        });
        map.insert("rust".to_string(), GlueTarget {
            language: "rust".to_string(),
            types_module: PathBuf::from("glue/rust/types.bv"),
            extension: "rs".to_string(),
            bridge_kind: "extern_c_crate".to_string(),
            calling_convention: "lto".to_string(),
            module_init: false,
            protocols: HashMap::new(),
            templates: HashMap::new(),
        });
        map
    }

    #[test]
    fn test_resolve_single_frgn_inline_c() {
        let fb = make_frgn("my_func", "c");
        let targets = sample_glue_targets();
        let result = resolve_single_frgn(&fb, "c", &targets, BackendKind::Llvm, None).unwrap();
        match result {
            ResolvedFrgn::Inline { symbol, compile_source } => {
                assert_eq!(symbol, "my_func");
                assert!(compile_source);
            }
            other => panic!("Expected Inline, got {:?}", other),
        }
    }

    #[test]
    fn test_resolve_single_frgn_inline_rs() {
        let fb = make_frgn("my_func", "rs");
        let targets = sample_glue_targets();
        let result = resolve_single_frgn(&fb, "rs", &targets, BackendKind::Llvm, None).unwrap();
        match result {
            ResolvedFrgn::Inline { symbol, .. } => {
                assert_eq!(symbol, "my_func");
            }
            other => panic!("Expected Inline, got {:?}", other),
        }
    }

    #[test]
    fn test_resolve_single_frgn_inline_with_as() {
        // foreign_name = "c_symbol", brief_name = Some("brief_alias")
        let fb = make_frgn_with_as("c_symbol", "brief_alias", "c");
        let targets = sample_glue_targets();
        let result = resolve_single_frgn(&fb, "c", &targets, BackendKind::Llvm, None).unwrap();
        match result {
            ResolvedFrgn::Inline { symbol, .. } => {
                // Backend links against the C symbol, not the Brief alias
                assert_eq!(symbol, "c_symbol");
            }
            other => panic!("Expected Inline, got {:?}", other),
        }
    }

    #[test]
    fn test_resolve_single_frgn_bridge_python() {
        let fb = make_frgn("py_func", "py");
        let targets = sample_glue_targets();
        let result = resolve_single_frgn(&fb, "py", &targets, BackendKind::Llvm, None).unwrap();
        match result {
            ResolvedFrgn::Bridge { language, .. } => {
                assert_eq!(language, "python");
            }
            other => panic!("Expected Bridge, got {:?}", other),
        }
    }

    #[test]
    fn test_resolve_single_frgn_unsupported_unknown_ext() {
        let fb = make_frgn("kotlin_func", "kt");
        let targets = sample_glue_targets();
        let result = resolve_single_frgn(&fb, "kt", &targets, BackendKind::Llvm, None).unwrap();
        match result {
            ResolvedFrgn::Unsupported(msg) => {
                assert!(msg.contains("kt"));
            }
            other => panic!("Expected Unsupported, got {:?}", other),
        }
    }

    #[test]
    fn test_resolve_single_frgn_unsupported_empty_ext() {
        let fb = make_frgn("no_ext", "");
        let targets = sample_glue_targets();
        let result = resolve_single_frgn(&fb, "", &targets, BackendKind::Llvm, None).unwrap();
        match result {
            ResolvedFrgn::Unsupported(msg) => {
                assert!(msg.contains("no file extension"));
            }
            other => panic!("Expected Unsupported, got {:?}", other),
        }
    }

    #[test]
    fn test_resolve_single_frgn_native_object() {
        let fb = make_frgn("native_fn", "so");
        let targets = sample_glue_targets();
        let result = resolve_single_frgn(&fb, "so", &targets, BackendKind::Llvm, None).unwrap();
        match result {
            ResolvedFrgn::Inline { symbol, compile_source } => {
                assert_eq!(symbol, "native_fn");
                assert!(!compile_source);
            }
            other => panic!("Expected Inline, got {:?}", other),
        }
    }

    #[test]
    fn test_compute_protocol_path_identity() {
        let int_type = crate::ast::Type::Custom("Int".to_string());
        let result = compute_protocol_path(&int_type, &int_type, None).unwrap();
        assert_eq!(result.len(), 1);
        assert!(matches!(result[0].kind, TransformKind::Identity));
    }
}
