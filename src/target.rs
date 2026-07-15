// ── Target Config — Backend Selection ─────────────────────────────────
// 2026-07-14: Reads config/targets.toml at compile time.
// Maps file extension → (backend, default CLI flags).
// --backend flag overrides the config.

use std::collections::HashMap;

/// Backend kinds that the compiler can dispatch to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BackendKind {
    Llvm,
    Circt,
    Webstack,
    Gpu,
    Spirv,
}

/// One entry from config/targets.toml.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct TargetEntry {
    pub backend: String,
    pub defaults: Vec<String>,
    /// System plugins enabled for this extension. None = default set.
    pub plugins: Option<Vec<String>>,
    /// Override LLVM target triple (e.g. "wasm32-unknown-wasi").
    /// 2026-07-15: Phase 7 — optional, defaults to x86_64-unknown-linux-gnu.
    pub target_triple: Option<String>,
    /// Override LLVM data layout string.
    /// 2026-07-15: Phase 7 — optional, auto-derived from target_triple if not set.
    pub data_layout: Option<String>,
}

/// Loaded config/targets.toml.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct TargetConfig {
    #[serde(flatten)]
    entries: HashMap<String, TargetEntry>,
}

impl TargetConfig {
    /// Load the compiled-in target config.
    pub fn load() -> Self {
        let content = include_str!("../config/targets.toml");
        toml::from_str(content).unwrap_or_else(|e| panic!("config/targets.toml parse error: {}", e))
    }

    /// Look up a target entry by file extension (e.g. ".bv").
    pub fn lookup(&self, extension: &str) -> Option<&TargetEntry> {
        let key = if extension.starts_with('.') { extension.to_string() } else { format!(".{}", extension) };
        self.entries.get(&key)
    }

    /// Resolve a backend name string to a BackendKind.
    pub fn resolve(name: &str) -> Result<BackendKind, String> {
        match name {
            "llvm" => Ok(BackendKind::Llvm),
            "circt" => Ok(BackendKind::Circt),
            "webstack" => Ok(BackendKind::Webstack),
            "gpu" => Ok(BackendKind::Gpu),
            "spirv" => Ok(BackendKind::Spirv),
            _ => Err(format!("unknown backend '{}'. Supported: llvm, circt, webstack", name)),
        }
    }
}

/// Get the file extension from a path, including the dot.
pub fn get_extension(file_path: &str) -> String {
    let p = std::path::Path::new(file_path);
    match p.extension().and_then(|s| s.to_str()) {
        Some(ext) => format!(".{}", ext),
        None => ".bv".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_target_config_loads() {
        let config = TargetConfig::load();
        assert!(config.lookup(".bv").is_some(), "should have .bv entry");
    }

    #[test]
    fn test_target_config_has_extensions() {
        let config = TargetConfig::load();
        for ext in &[".bv", ".ebv", ".cbv", ".rbv", ".abv"] {
            assert!(config.lookup(ext).is_some(), "missing entry for {}", ext);
        }
    }

    #[test]
    fn test_resolve_backend() {
        assert_eq!(TargetConfig::resolve("llvm").unwrap(), BackendKind::Llvm);
        assert_eq!(TargetConfig::resolve("circt").unwrap(), BackendKind::Circt);
        assert_eq!(TargetConfig::resolve("webstack").unwrap(), BackendKind::Webstack);
        assert_eq!(TargetConfig::resolve("spirv").unwrap(), BackendKind::Spirv);
        assert!(TargetConfig::resolve("unknown").is_err());
    }

    #[test]
    fn test_get_extension() {
        assert_eq!(get_extension("foo.bv"), ".bv");
        assert_eq!(get_extension("foo.ebv"), ".ebv");
        assert_eq!(get_extension("foo.cbv"), ".cbv");
        assert_eq!(get_extension("foo"), ".bv");
    }
}
