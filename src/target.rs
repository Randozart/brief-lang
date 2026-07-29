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
    Vm,
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
    /// 2026-07-29: Assembler backend for inline assembly validation.
    /// "keystone" — Keystone Engine (default, requires libkeystone).
    /// "platform" — system assembler (as / ml64).
    /// "none" — no validation, warn at compile time.
    #[serde(default = "default_assembler")]
    pub assembler: String,
    /// 2026-07-29: Number of random samples for cross-verification
    /// in the := verification chain. Default 50.
    #[serde(default = "default_cross_verify_samples")]
    pub cross_verify_samples: u32,
}

fn default_assembler() -> String { "none".to_string() }
fn default_cross_verify_samples() -> u32 { 50 }

/// Loaded config/targets.toml.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct TargetConfig {
    #[serde(flatten)]
    entries: HashMap<String, TargetEntry>,
}

// ── Protocol Map ─────────────────────────────────────────────────────────
// 2026-07-26: Phase 1 — Protocol-to-library resolution for from #System etc.
// Maps a target triple → { protocol_name → library_or_none }.
// None means the protocol is unavailable on that target.

/// Loaded config/protocols.toml.
///
/// 2026-07-26: Maps protocol names to linker library names per target triple.
/// `#System` abstracts "the platform's standard system library" (libc on Linux,
/// libSystem on macOS, WASI preview1 on wasm). `#Web` routes through the GLUE
/// wasm_runtime bridge (WASM targets only, no linker flag needed).
/// Any other protocol hashword produces a compile error.
/// Loaded by ProtocolConfig::load() and consulted during frgn dispatch resolution.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ProtocolConfig {
    /// Key = target triple (e.g. "x86_64-linux"),
    /// Value = { protocol_name → library_name_or_none }.
    #[serde(flatten)]
    per_target: HashMap<String, HashMap<String, Option<String>>>,
}

impl ProtocolConfig {
    /// Load the compiled-in protocol config (baked at compile time).
    pub fn load() -> Self {
        let content = include_str!("../config/protocols.toml");
        toml::from_str(content).unwrap_or_else(|e| panic!("config/protocols.toml parse error: {}", e))
    }

    /// Resolve a protocol name to a library name for the given target.
    ///
    /// `#System` and `#Web` are the two recognized protocols.
    /// `#System` links against the platform's system library (libc, WASI).
    /// `#Web` routes through the GLUE wasm_runtime bridge (valid on WASM targets only).
    ///
    /// Returns:
    /// - `Ok(Some(lib))` — protocol maps to library `lib`, link with `-l<lib>`.
    /// - `Ok(None)` — protocol is available but needs no extra linker flag
    ///   (e.g., libc is linked by default with clang).
    /// - `Err(msg)` — protocol is unrecognized or unavailable on target.
    pub fn resolve(&self, target_triple: &str, protocol: &str) -> Result<Option<&str>, String> {
        if protocol != "#System" && protocol != "#Web" {
            return Err(format!(
                "'{}' is not a valid protocol hashword. \
                 #System and #Web are the supported protocols",
                protocol
            ));
        }
        let target_map = self.per_target.get(target_triple).ok_or_else(|| {
            format!(
                "target '{}' not found in config/protocols.toml. \
                 Add an entry for this target to configure protocol support",
                target_triple
            )
        })?;
        match target_map.get(protocol) {
            Some(Some(lib)) => Ok(Some(lib.as_str())),
            Some(None) => Ok(None),
            None => Err(format!(
                "target '{}' has no '{}' entry in config/protocols.toml",
                target_triple, protocol
            )),
        }
    }

    /// Check if a protocol is available on a given target.
    pub fn is_available(&self, target_triple: &str, protocol: &str) -> bool {
        self.per_target
            .get(target_triple)
            .and_then(|m| m.get(protocol))
            .map(|v| v.is_some())
            .unwrap_or(false)
    }
}

impl TargetConfig {
    /// Load the compiled-in target config (fallback).
    pub fn load() -> Self {
        let content = include_str!("../config/targets.toml");
        toml::from_str(content).unwrap_or_else(|e| panic!("config/targets.toml parse error: {}", e))
    }

    /// 2026-07-16: P1 — load from a concrete file path.
    pub fn load_from(path: &std::path::Path) -> Result<Self, String> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("cannot read '{}': {}", path.display(), e))?;
        toml::from_str(&content)
            .map_err(|e| format!("parse error in '{}': {}", path.display(), e))
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
            "vm" => Ok(BackendKind::Vm),
            _ => Err(format!("unknown backend '{}'. Supported: llvm, circt, webstack, vm", name)),
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

    #[test]
    fn test_protocol_config_loads() {
        let config = ProtocolConfig::load();
        assert!(config.is_available("x86_64-linux", "#System"),
            "x86_64-linux should support #System");
        assert!(!config.is_available("x86_64-linux", "#NonExistent"),
            "non-existent protocol should be unavailable");
    }

    #[test]
    fn test_protocol_config_resolve_system() {
        let config = ProtocolConfig::load();
        let lib = config.resolve("x86_64-linux", "#System").unwrap();
        assert_eq!(lib, Some("c"), "#System should resolve to 'c' on linux");
    }

    #[test]
    fn test_protocol_config_resolve_system_wasi() {
        let config = ProtocolConfig::load();
        let lib = config.resolve("wasm32-wasi", "#System").unwrap();
        assert_eq!(lib, Some("wasi_snapshot_preview1"),
            "#System on wasm32-wasi should resolve to wasi_snapshot_preview1");
    }

    #[test]
    fn test_protocol_config_resolve_unknown_protocol() {
        let config = ProtocolConfig::load();
        let err = config.resolve("x86_64-linux", "#SomethingElse")
            .unwrap_err();
        assert!(err.contains("supported protocols"),
            "error should mention supported protocols (got: '{}')", err);
    }

    #[test]
    fn test_protocol_config_resolve_unknown_target() {
        let config = ProtocolConfig::load();
        let result = config.resolve("nonexistent-target", "#System");
        assert!(result.is_err(), "unknown target should error");
    }
}
