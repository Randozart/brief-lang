// ── GLUE Configuration (TOML) ─────────────────────────────────────────
// 2026-07-22: Reads lib/glue.toml to resolve language targets for frgn
// dispatch and export generation. Replaces the old dbvl-based registry.
//
// Why TOML over dbvl: TOML is a mature, widely-supported format with
// existing Rust ecosystem (toml + serde). The dbvl format remains as the
// output format for bridge-exports metadata (machine consumption), but
// the compiler's own registry is TOML for maintainability.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// A language target entry from the GLUE registry.
///
/// 2026-07-22: Each target describes how to bridge with one foreign language.
/// Fields map directly to `[language]` sections in `lib/glue.toml`.
#[derive(Debug, Clone)]
pub struct GlueTarget {
    /// Language identifier (e.g., "python", "rust", "node")
    pub language: String,
    /// Path to .bv file declaring foreign type representations
    pub types_module: PathBuf,
    /// Native source file extension (without dot)
    pub extension: String,
    /// Bridge strategy: "native_module", "esm_module", "extern_c_crate", etc.
    pub bridge_kind: String,
    /// Calling convention: "c_abi", "lto", etc.
    pub calling_convention: String,
    /// Brief type name → C ABI type name mapping (e.g., Int → int64_t)
    pub c_type_map: HashMap<String, String>,
}

// ── Serde helpers ─────────────────────────────────────────────────────
// 2026-07-22: The TOML structure has inline tables for c_type_map, so we
// need serde Deserialize for the intermediate representation.

#[derive(serde::Deserialize)]
struct GlueConfigFile {
    #[serde(default)]
    python: Option<LanguageEntry>,
    #[serde(default)]
    node: Option<LanguageEntry>,
    #[serde(default)]
    rust: Option<LanguageEntry>,
}

#[derive(serde::Deserialize)]
struct LanguageEntry {
    types_module: String,
    extension: String,
    bridge_kind: String,
    calling_convention: String,
    #[serde(default)]
    c_type_map: HashMap<String, String>,
}

/// Load the GLUE registry from a TOML file.
///
/// Searches the built-in path (compiler-shipped `lib/glue.toml`) by default,
/// or a project-level override via `--glue-config`.
///
/// Returns a map from language identifier → GlueTarget.
///
/// 2026-07-22: The default path is computed at compile time via
/// `CARGO_MANIFEST_DIR`, so the shipped `lib/glue.toml` is always found
/// regardless of the user's working directory.
pub fn load_glue_config(path: Option<&Path>) -> Result<HashMap<String, GlueTarget>, String> {
    let config_path = match path {
        Some(p) => p.to_path_buf(),
        None => {
            // 2026-07-22: CARGO_MANIFEST_DIR is the compiler's source root,
            // so lib/glue.toml ships with the compiler binary.
            let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
            manifest.join("lib").join("glue.toml")
        }
    };

    let source = std::fs::read_to_string(&config_path)
        .map_err(|e| format!("Failed to read GLUE config '{}': {}", config_path.display(), e))?;

    let parsed: GlueConfigFile = toml::from_str(&source)
        .map_err(|e| format!("Failed to parse GLUE config '{}': {}", config_path.display(), e))?;

    let mut targets: HashMap<String, GlueTarget> = HashMap::new();

    if let Some(py) = parsed.python {
        targets.insert("python".to_string(), GlueTarget {
            language: "python".to_string(),
            types_module: PathBuf::from(py.types_module),
            extension: py.extension,
            bridge_kind: py.bridge_kind,
            calling_convention: py.calling_convention,
            c_type_map: py.c_type_map,
        });
    }

    if let Some(node) = parsed.node {
        targets.insert("node".to_string(), GlueTarget {
            language: "node".to_string(),
            types_module: PathBuf::from(node.types_module),
            extension: node.extension,
            bridge_kind: node.bridge_kind,
            calling_convention: node.calling_convention,
            c_type_map: node.c_type_map,
        });
    }

    if let Some(rust) = parsed.rust {
        targets.insert("rust".to_string(), GlueTarget {
            language: "rust".to_string(),
            types_module: PathBuf::from(rust.types_module),
            extension: rust.extension,
            bridge_kind: rust.bridge_kind,
            calling_convention: rust.calling_convention,
            c_type_map: rust.c_type_map,
        });
    }

    Ok(targets)
}

/// Find a language target by extension.
///
/// 2026-07-22: Matches the file extension (without dot) against known
/// language targets. Returns the language name and its GlueTarget.
/// This is the primary lookup used during frgn dispatch resolution.
pub fn find_language_by_extension<'a>(
    targets: &'a HashMap<String, GlueTarget>,
    ext: &str,
) -> Option<&'a GlueTarget> {
    let ext = ext.trim_start_matches('.');
    targets.values().find(|t| t.extension == ext)
}

/// Map a file extension to a language identifier.
///
/// 2026-07-22: Baked per-backend but exposed via TOML for debugging.
/// Returns Some(language_name) if the extension is recognized for the
/// given backend, None otherwise.
pub fn extension_to_language(ext: &str, backend: &str) -> Option<&'static str> {
    match backend {
        "llvm" => match ext {
            "py" | "pyc" => Some("python"),
            "js" | "ts" | "mjs" => Some("node"),
            "rs" => Some("rust"),
            "c" | "cpp" | "cxx" => Some("c"),
            _ => None,
        },
        "webstack" => match ext {
            "c" => Some("c"),
            "py" => Some("python"),
            "rs" => Some("rust"),
            _ => None,
        },
        "circt" => None,  // All frgn rejected by hardware validator
        "spirv" => match ext {
            "c" => Some("c"),
            "py" => Some("python"),
            _ => None,
        },
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extension_to_language_llvm() {
        assert_eq!(extension_to_language("py", "llvm"), Some("python"));
        assert_eq!(extension_to_language("rs", "llvm"), Some("rust"));
        assert_eq!(extension_to_language("js", "llvm"), Some("node"));
        assert_eq!(extension_to_language("mjs", "llvm"), Some("node"));
    }

    #[test]
    fn test_extension_to_language_circt() {
        assert_eq!(extension_to_language("py", "circt"), None);
        assert_eq!(extension_to_language("rs", "circt"), None);
    }

    #[test]
    fn test_extension_to_language_unknown() {
        assert_eq!(extension_to_language("kotlin", "llvm"), None);
        assert_eq!(extension_to_language("swift", "llvm"), None);
    }

    #[test]
    fn test_find_language_by_extension() {
        let mut targets = HashMap::new();
        targets.insert("python".to_string(), GlueTarget {
            language: "python".to_string(),
            types_module: PathBuf::from("glue/python/types.bv"),
            extension: "py".to_string(),
            bridge_kind: "native_module".to_string(),
            calling_convention: "c_abi".to_string(),
            c_type_map: HashMap::new(),
        });
        targets.insert("rust".to_string(), GlueTarget {
            language: "rust".to_string(),
            types_module: PathBuf::from("glue/rust/types.bv"),
            extension: "rs".to_string(),
            bridge_kind: "extern_c_crate".to_string(),
            calling_convention: "lto".to_string(),
            c_type_map: HashMap::new(),
        });

        let found = find_language_by_extension(&targets, "py");
        assert!(found.is_some());
        assert_eq!(found.unwrap().language, "python");

        let found = find_language_by_extension(&targets, "rs");
        assert!(found.is_some());
        assert_eq!(found.unwrap().language, "rust");

        let found = find_language_by_extension(&targets, "js");
        assert!(found.is_none());
    }

    #[test]
    fn test_find_language_by_extension_with_dot() {
        let mut targets = HashMap::new();
        targets.insert("python".to_string(), GlueTarget {
            language: "python".to_string(),
            types_module: PathBuf::from("glue/python/types.bv"),
            extension: "py".to_string(),
            bridge_kind: "native_module".to_string(),
            calling_convention: "c_abi".to_string(),
            c_type_map: HashMap::new(),
        });

        // Should work with or without leading dot
        assert_eq!(find_language_by_extension(&targets, ".py").unwrap().language, "python");
        assert_eq!(find_language_by_extension(&targets, "py").unwrap().language, "python");
    }

    #[test]
    fn test_load_glue_config_custom_path() {
        let dir = std::env::temp_dir();
        let config_path = dir.join("test_glue_config.toml");
        let content = r#"
[python]
types_module = "glue/python/types.bv"
extension = "py"
bridge_kind = "native_module"
calling_convention = "c_abi"

[python.c_type_map]
Int = "int64_t"
Float = "double"

[rust]
types_module = "glue/rust/types.bv"
extension = "rs"
bridge_kind = "extern_c_crate"
calling_convention = "lto"
"#;
        std::fs::write(&config_path, content).unwrap();

        let result = load_glue_config(Some(&config_path));
        assert!(result.is_ok(), "load_glue_config failed: {:?}", result.err());
        let targets = result.unwrap();

        assert!(targets.contains_key("python"));
        let py = &targets["python"];
        assert_eq!(py.language, "python");
        assert_eq!(py.extension, "py");
        assert_eq!(py.bridge_kind, "native_module");
        assert_eq!(py.calling_convention, "c_abi");
        assert_eq!(py.c_type_map.get("Int"), Some(&"int64_t".to_string()));
        assert_eq!(py.c_type_map.get("Float"), Some(&"double".to_string()));

        assert!(targets.contains_key("rust"));
        let rust = &targets["rust"];
        assert_eq!(rust.language, "rust");
        assert_eq!(rust.extension, "rs");
        assert_eq!(rust.bridge_kind, "extern_c_crate");
        assert_eq!(rust.calling_convention, "lto");

        let _ = std::fs::remove_file(&config_path);
    }

    #[test]
    fn test_load_glue_config_not_found() {
        let dir = std::env::temp_dir();
        let missing = dir.join("nonexistent_glue.toml");
        let result = load_glue_config(Some(&missing));
        assert!(result.is_err());
    }

    #[test]
    fn test_load_glue_config_default_path_exists() {
        // The compiler-shipped default should exist at compile time
        let result = load_glue_config(None);
        assert!(result.is_ok(), "Default lib/glue.toml not found: {:?}", result.err());
    }
}
