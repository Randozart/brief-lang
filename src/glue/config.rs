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
    /// Brief type name → language-native type name (for safe wrappers)
    pub type_map: HashMap<String, String>,
    /// Brief type name → C ABI type name mapping (e.g., Int → int64_t)
    pub c_type_map: HashMap<String, String>,
    /// Type conversion expressions. Keys are "{Type}.to_abi" and "{Type}.from_abi",
    /// values are template expressions with {name} as the variable placeholder.
    /// E.g., "String.to_abi" = "{name}.as_ptr() as i64"
    pub conversions: HashMap<String, String>,
    /// Output path → template content. Special keys:
    ///   "fn_template" — per-function safe wrapper (rendered into {{exports}})
    ///   "ffi_template" — per-function FFI declaration (rendered into {{ffi_decls}})
    pub templates: HashMap<String, String>,
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
    type_map: HashMap<String, String>,
    #[serde(default)]
    c_type_map: HashMap<String, String>,
    #[serde(default)]
    conversions: HashMap<String, ConversionEntry>,
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
            type_map: entry.type_map,
            c_type_map: entry.c_type_map,
            conversions: flatten_conversions(entry.conversions),
            templates: entry.templates,
        });
    }

    Ok(targets)
}

/// Flatten nested conversion entries into flat "Type.to_abi"/"Type.from_abi" keys.
fn flatten_conversions(
    conv: HashMap<String, ConversionEntry>,
) -> HashMap<String, String> {
    let mut flat = HashMap::new();
    for (ty, entry) in conv {
        flat.insert(format!("{}.to_abi", ty), entry.to_abi);
        flat.insert(format!("{}.from_abi", ty), entry.from_abi);
    }
    flat
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
            type_map: HashMap::new(),
            c_type_map: HashMap::new(),
            conversions: HashMap::new(),
            templates: HashMap::new(),
        });
        targets.insert("rust".to_string(), GlueTarget {
            language: "rust".to_string(),
            types_module: PathBuf::from("glue/rust/types.bv"),
            extension: "rs".to_string(),
            bridge_kind: "extern_c_crate".to_string(),
            calling_convention: "lto".to_string(),
            type_map: HashMap::new(),
            c_type_map: HashMap::new(),
            conversions: HashMap::new(),
            templates: HashMap::new(),
        });
        assert_eq!(extension_to_language("py", &targets), Some("python"));
        assert_eq!(extension_to_language("rs", &targets), Some("rust"));
        assert_eq!(extension_to_language("js", &targets), None);
        assert_eq!(extension_to_language("kotlin", &targets), None);
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
            conversions: HashMap::new(),
            type_map: HashMap::new(),
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
            c_type_map: HashMap::new(),
            conversions: HashMap::new(),
            type_map: HashMap::new(),
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
