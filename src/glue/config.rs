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
/// Protocol mapping replaces old type_map/c_type_map/conversions —
/// the TOML only knows about protocol categories (#String, #Int, #Float),
/// not about Brief-internal type names.
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
    /// 2026-07-23: Emit native C extension module init metadata
    /// (PyMethodDef/PyModuleDef for Python, NAPI for Node, etc.).
    /// When true, the LLVM backend emits module init at codegen time.
    pub module_init: bool,
    /// Protocol category → native/C ABI type mapping.
    /// Keys like "#String", "#Int", "#Float" — protocol categories only,
    /// never Brief-internal type names.
    pub protocols: HashMap<String, ProtocolEntry>,
    /// Output path → template content. Special keys:
    ///   "fn_template" — per-function safe wrapper (rendered into {{exports}})
    ///   "ffi_template" — per-function FFI declaration (rendered into {{ffi_decls}})
    pub templates: HashMap<String, String>,
}

/// A protocol category mapping for a single language.
///
/// 2026-07-22: Each protocol category (#String, #Int, #Float) maps to
/// the language's native type and its C ABI representation. The compiler
/// uses this when the BFS finds a path through that protocol category.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ProtocolEntry {
    /// Language-native type name (e.g., "str", "String", "int")
    pub native: String,
    /// C ABI type name (e.g., "i64", "ctypes.c_int64")
    pub c_abi: String,
}

// ── Serde helpers ─────────────────────────────────────────────────────
// 2026-07-22: The TOML structure has inline tables for c_type_map, so we
// need serde Deserialize for the intermediate representation.

#[derive(serde::Deserialize)]
struct GlueConfigFile {
    /// All top-level keys that aren't recognized as special config are
    /// language targets. Adding a language = adding a [lang] section.
    #[serde(flatten)]
    languages: HashMap<String, LanguageEntry>,
}

#[derive(serde::Deserialize)]
struct LanguageEntry {
    types_module: String,
    extension: String,
    bridge_kind: String,
    calling_convention: String,
    #[serde(default)]
    module_init: bool,
    #[serde(default)]
    protocols: HashMap<String, ProtocolEntry>,
    #[serde(default)]
    templates: HashMap<String, String>,
}

/// A conversion entry for a type at the FFI boundary.
/// Expresses how to convert between the safe Rust type and the C ABI type.
#[derive(serde::Deserialize, Debug, Clone)]
pub struct ConversionEntry {
    /// Expression to convert a safe value to ABI, with {name} as placeholder.
    /// E.g., "{name}.as_ptr() as i64"
    pub to_abi: String,
    /// Expression to convert an ABI value back to a safe value.
    /// E.g., "String::from_raw_parts({name} as *mut u8, len)"
    pub from_abi: String,
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

    for (name, entry) in parsed.languages {
        targets.insert(name.clone(), GlueTarget {
            language: name,
            types_module: PathBuf::from(entry.types_module),
            extension: entry.extension,
            bridge_kind: entry.bridge_kind,
            calling_convention: entry.calling_convention,
            module_init: entry.module_init,
            protocols: entry.protocols,
            templates: entry.templates,
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

/// Map a file extension to a language identifier using the loaded targets.
/// Returns Some(language_name) if the extension is recognized, None otherwise.
pub fn extension_to_language<'a>(ext: &str, targets: &'a HashMap<String, GlueTarget>) -> Option<&'a str> {
    let ext = ext.trim_start_matches('.');
    targets.values().find(|t| t.extension == ext).map(|t| t.language.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extension_to_language_via_targets() {
        let mut targets = HashMap::new();
        targets.insert("python".to_string(), GlueTarget {
            language: "python".to_string(),
            types_module: PathBuf::from("glue/python/types.bv"),
            extension: "py".to_string(),
            bridge_kind: "native_module".to_string(),
            calling_convention: "c_abi".to_string(),
            protocols: HashMap::new(),
            templates: HashMap::new(),
        });
        targets.insert("rust".to_string(), GlueTarget {
            language: "rust".to_string(),
            types_module: PathBuf::from("glue/rust/types.bv"),
            extension: "rs".to_string(),
            bridge_kind: "extern_c_crate".to_string(),
            calling_convention: "lto".to_string(),
            protocols: HashMap::new(),
            templates: HashMap::new(),
        });

        // Should work with or without leading dot
        assert_eq!(find_language_by_extension(&targets, ".py").unwrap().language, "python");
        assert_eq!(find_language_by_extension(&targets, "py").unwrap().language, "python");
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
            protocols: HashMap::new(),
            templates: HashMap::new(),
        });

        // Should work with or without leading dot
        assert_eq!(find_language_by_extension(&targets, ".py").unwrap().language, "python");
        assert_eq!(find_language_by_extension(&targets, "py").unwrap().language, "python");
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
            protocols: HashMap::new(),
            templates: HashMap::new(),
        });

        // Should work with or without leading dot
        assert_eq!(find_language_by_extension(&targets, ".py").unwrap().language, "python");
        assert_eq!(find_language_by_extension(&targets, "py").unwrap().language, "python");
    }

    #[test]
    fn test_load_glue_config_custom_path() {
        let dir = std::env::temp_dir();
        let config_path = dir.join("test_glue_config.toml");
        let content = r##"
[python]
types_module = "glue/python/types.bv"
extension = "py"
bridge_kind = "native_module"
calling_convention = "c_abi"

[python.protocols]
"#String" = { native = "str", c_abi = "ctypes.c_void_p" }
"#Int" = { native = "int", c_abi = "ctypes.c_int64" }

[rust]
types_module = "glue/rust/types.bv"
extension = "rs"
bridge_kind = "extern_c_crate"
calling_convention = "lto"
"##;
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
        assert!(py.protocols.contains_key("#String"));
        assert_eq!(py.protocols.get("#String").unwrap().native, "str");
        assert_eq!(py.protocols.get("#String").unwrap().c_abi, "ctypes.c_void_p");

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
