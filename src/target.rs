// ── Target Config — Backend Selection ─────────────────────────────────
// 2026-07-14: Reads config/targets.dbvl at compile time.
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

/// One entry from config/targets.dbvl.
///
/// 2026-07-31: `backend`/`defaults` are optional so the `[target.<prefix>]`
/// tuning tables (plan §8.1) coexist in the same file without failing the
/// flatten parse — extension entries always set them; target-tuning entries
/// do not (they carry float_registers/dense_compute_density/vector_min_width,
/// read by config_tuning.rs, which serde ignores here).
#[derive(Debug, Clone, serde::Deserialize)]
pub struct TargetEntry {
    #[serde(default)]
    pub backend: Option<String>,
    #[serde(default)]
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

/// Split a whitespace-separated list field into words (parser has no array
/// grammar — space-separated values round-trip as one String field).
/// 2026-08-03 (Phase 3, data-brief-config plan).
fn split_words(s: &str) -> Vec<String> {
    s.split_whitespace().map(|w| w.to_string()).collect()
}

/// Loaded config/targets.dbvl.
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
    ///
    /// 2026-08-03 (Phase 3, data-brief-config plan): reads config/protocols.dbvl
    /// (quoted mode — the `#System`/`#Web` map keys are quoted because `#` is
    /// not an identifier char). Shape is `<triple>: { "<protocol>": "<lib>"; }`.
    pub fn load() -> Self {
        let content = include_str!("../config/protocols.dbvl");
        let db = crate::dbrief::config_db::ConfigDb::from_quoted_str(content)
            .unwrap_or_else(|e| panic!("config/protocols.dbvl parse error: {}", e));
        let mut per_target = HashMap::new();
        for key in db.keys() {
            let mut map = HashMap::new();
            if let Some(crate::dbrief::v2::DataValue::Map(entries)) = db.field(&key, 0) {
                for (protocol, lib) in entries {
                    map.insert(protocol.clone(), match lib {
                        crate::dbrief::v2::DataValue::String(s) => Some(s.clone()),
                        _ => None,
                    });
                }
            }
            per_target.insert(key, map);
        }
        ProtocolConfig { per_target }
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
    ///
    /// 2026-08-03 (Phase 3, data-brief-config plan): reads config/targets.dbvl.
    /// Extension entries are `<.ext>: <backend>; <defaults space-sep>;
    /// <plugins space-sep>; [assembler]; [cross_verify_samples];
    /// [target_triple]; [data_layout];` — the two overrides are optional and
    /// never set in the shipped file, so they sit AFTER the common fields.
    /// The `target.*` tuning rows are consumed by config_tuning, not here.
    pub fn load() -> Self {
        let content = include_str!("../config/targets.dbvl");
        let db = crate::dbrief::config_db::ConfigDb::from_str(content)
            .unwrap_or_else(|e| panic!("config/targets.dbvl parse error: {}", e));
        let mut entries = HashMap::new();
        for key in db.keys() {
            if key.starts_with("target.") {
                continue; // tuning rows — config_tuning's table
            }
            let entry = TargetEntry {
                backend: db.field_string(&key, 0).map(|s| s.to_string()),
                defaults: db.field_string(&key, 1).map(split_words).unwrap_or_default(),
                plugins: db.field_string(&key, 2).map(split_words),
                assembler: db.field_string(&key, 3).map(|s| s.to_string()).unwrap_or_else(default_assembler),
                cross_verify_samples: db.field_int(&key, 4).map(|v| v as u32)
                    .unwrap_or_else(default_cross_verify_samples),
                target_triple: db.field_string(&key, 5).map(|s| s.to_string()),
                data_layout: db.field_string(&key, 6).map(|s| s.to_string()),
            };
            entries.insert(key, entry);
        }
        TargetConfig { entries }
    }

    /// 2026-07-16: P1 — load from a concrete file path (TOML or .dbvl).
    pub fn load_from(path: &std::path::Path) -> Result<Self, String> {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext == "dbvl" || ext == "dbv" {
            let content = std::fs::read_to_string(path)
                .map_err(|e| format!("cannot read '{}': {}", path.display(), e))?;
            let db = crate::dbrief::config_db::ConfigDb::from_str(&content)
                .map_err(|e| format!("parse error in '{}': {}", path.display(), e))?;
            let mut entries = HashMap::new();
            for key in db.keys() {
                if key.starts_with("target.") {
                    continue;
                }
                let entry = TargetEntry {
                    backend: db.field_string(&key, 0).map(|s| s.to_string()),
                    defaults: db.field_string(&key, 1).map(split_words).unwrap_or_default(),
                    plugins: db.field_string(&key, 2).map(split_words),
                    assembler: db.field_string(&key, 3).map(|s| s.to_string()).unwrap_or_else(default_assembler),
                    cross_verify_samples: db.field_int(&key, 4).map(|v| v as u32)
                        .unwrap_or_else(default_cross_verify_samples),
                    target_triple: db.field_string(&key, 5).map(|s| s.to_string()),
                    data_layout: db.field_string(&key, 6).map(|s| s.to_string()),
                };
                entries.insert(key, entry);
            }
            return Ok(TargetConfig { entries });
        }
        // Legacy TOML path (pre-migration profiles).
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

    #[test]
    fn parity_targets_dbvl_matches_toml() {
        // Phase 3 migration gate: config/targets.dbvl must produce exactly the
        // extension→TargetEntry map the targets.toml it replaces produces. The
        // `target.*` tuning rows are skipped here — they feed config_tuning,
        // whose own parity test covers them. The .toml is deleted only after
        // both stay green.
        let db = TargetConfig::load();
        let content = include_str!("../config/targets.toml");
        let toml_entries: HashMap<String, TargetEntry> =
            toml::from_str(content).unwrap();

        let toml_exts: Vec<String> = toml_entries
            .keys()
            // The `[target.<prefix>]` tuning tables flatten to a single `target`
            // key (nested table), not `target.*` — exclude it.
            .filter(|k| !k.starts_with("target.") && *k != "target")
            .cloned()
            .collect();
        assert!(!toml_exts.is_empty(), "targets.toml should have extension entries");

        assert_eq!(db.entries.len(), toml_exts.len(),
            "extension-entry count diverges between .dbvl and .toml");
        for ext in &toml_exts {
            let db_entry = db.entries.get(ext)
                .unwrap_or_else(|| panic!("extension '{}' missing from targets.dbvl", ext));
            let toml_entry = &toml_entries[ext];
            assert_eq!(db_entry.backend, toml_entry.backend,
                "backend for '{}' diverges", ext);
            assert_eq!(db_entry.defaults, toml_entry.defaults,
                "defaults for '{}' diverge", ext);
            assert_eq!(db_entry.plugins, toml_entry.plugins,
                "plugins for '{}' diverge", ext);
            assert_eq!(db_entry.assembler, toml_entry.assembler,
                "assembler for '{}' diverges", ext);
            assert_eq!(db_entry.cross_verify_samples, toml_entry.cross_verify_samples,
                "cross_verify_samples for '{}' diverges", ext);
        }
    }

    #[test]
    fn parity_protocols_dbvl_matches_toml() {
        // Phase 3 migration gate: config/protocols.dbvl must produce exactly
        // the target→protocol→library map the .toml it replaces produces. The
        // .toml is deleted only after this stays green.
        let db = ProtocolConfig::load();
        let content = include_str!("../config/protocols.toml");
        let toml_map: HashMap<String, HashMap<String, Option<String>>> =
            toml::from_str(content).unwrap();

        assert_eq!(db.per_target.len(), toml_map.len());
        for (triple, toml_protos) in &toml_map {
            let db_map = db.per_target.get(triple)
                .unwrap_or_else(|| panic!("target '{}' missing from protocols.dbvl", triple));
            assert_eq!(db_map.len(), toml_protos.len());
            for (protocol, lib) in toml_protos {
                assert_eq!(
                    db_map.get(protocol),
                    Some(lib),
                    "protocol '{}' on '{}' diverges between .dbvl and .toml",
                    protocol, triple
                );
            }
        }
    }
}
