// ── Config — Remaining Config Types ─────────────────────────────────────
// 2026-07-20: TypeConfig and OpConfig removed — hashword protocol replaces
// TOML-driven op dispatch. Only AllocConfig remains (allocation templates).
//
// TOML files removed:
// - config/ctd-llvm-mappings.toml (replaced by primordial seed + structure)
// - config/llvm-ops.toml (replaced by hashword op signatures)
// - config/spirv-ops.toml (replaced by hashword op signatures)

use std::collections::HashMap;
use std::path::Path;

/// 2026-07-18: An allocation strategy config entry with LLVM IR template
/// and optional Free# dispatch override.
#[derive(Debug, Clone)]
pub struct AllocStrategyEntry {
    pub template: String,
    /// How Free# should handle this strategy:
    /// None: call @free (default)
    /// Some("none"): no-op (arena, ring buffer, pool with bulk free)
    /// Some("fn_name"): call custom function
    pub free: Option<String>,
}

/// 2026-07-18: Maps named allocation strategies to LLVM IR templates.
/// Loaded from config/alloc-strategies.dbvl at compile time.
#[derive(Debug, Clone)]
pub struct AllocConfig {
    strategies: HashMap<String, AllocStrategyEntry>,
}

impl AllocConfig {
    /// Load the built-in alloc strategies file.
    ///
    /// 2026-08-03 (Phase 3, data-brief-config plan): reads config/alloc-strategies.dbvl
    /// in quoted mode — the LLVM IR templates carry `{v}`/`{size}` braces that
    /// bare mode would take as a nested sub-record. Row shape:
    /// `<name>: "<template with \n escapes>"; [free];`. The .toml remains as
    /// the parity-test source until identical output is proven.
    pub fn load() -> Self {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("config/alloc-strategies.dbvl");
        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => return AllocConfig { strategies: HashMap::new() },
        };
        let db = match crate::dbrief::config_db::ConfigDb::from_quoted_str(&content) {
            Ok(db) => db,
            Err(e) => {
                eprintln!("warning: config/alloc-strategies.dbvl parse error: {} — using empty set", e);
                return AllocConfig { strategies: HashMap::new() };
            }
        };
        let mut strategies = HashMap::new();
        for key in db.keys() {
            let Some(template) = db.field_string(&key, 0).map(|s| s.to_string()) else {
                continue;
            };
            let free = db.field_string(&key, 1).map(|s| s.to_string());
            strategies.insert(key, AllocStrategyEntry { template, free });
        }
        AllocConfig { strategies }
    }

    /// Look up the LLVM IR template for a strategy name.
    pub fn lookup(&self, name: &str) -> Option<&str> {
        self.strategies.get(name).map(|e| e.template.as_str())
    }

    /// 2026-07-18: Look up the Free# behavior for a strategy name.
    /// Returns None → call @free (default). Some("none") → no-op.
    /// Some("fn_name") → call custom free function.
    pub fn lookup_free(&self, name: &str) -> Option<&str> {
        self.strategies.get(name).and_then(|e| e.free.as_deref())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pre-migration config/alloc-strategies.toml, frozen as the golden
    /// reference for parity_alloc_strategies_dbvl_matches_toml. 2026-08-03:
    /// the .toml file is deleted; edits to config/alloc-strategies.dbvl must
    /// keep this test green.
    const ALLOC_STRATEGIES_TOML_GOLDEN: &str = r#"
[alloc.pool_serial]
template = """
%{v}_p = call ptr @pool_alloc(i64 {size})
%{v} = ptrtoint ptr %{v}_p to i64
"""
free = "none"

[alloc.mmap_shared]
template = """
%{v}_p = call ptr @mmap_shared(i64 {size})
%{v} = ptrtoint ptr %{v}_p to i64
"""
free = "munmap_shared"

[alloc.pinned_dma]
template = """
%{v}_p = call ptr @alloc_dma_pinned(i64 {size})
%{v} = ptrtoint ptr %{v}_p to i64
"""
free = "free_dma_pinned"
"#;

    #[test]
    fn alloc_config_loads_strategies() {
        let config = AllocConfig::load();
        assert!(config.lookup("pool_serial").is_some(), "should have pool_serial");
        assert!(config.lookup("mmap_shared").is_some(), "should have mmap_shared");
        assert!(config.lookup("pinned_dma").is_some(), "should have pinned_dma");
    }

    #[test]
    fn alloc_config_preserves_templates_and_free() {
        let config = AllocConfig::load();
        let template = config.lookup("pool_serial").unwrap();
        assert!(template.contains("call ptr @pool_alloc(i64 {size})"),
            "template must keep the {{size}} placeholder (got: '{}')", template);
        assert!(template.contains('\n'), "multi-line template must keep its newline");
        assert_eq!(config.lookup_free("pool_serial"), Some("none"));
        assert_eq!(config.lookup_free("mmap_shared"), Some("munmap_shared"));
        assert_eq!(config.lookup_free("pinned_dma"), Some("free_dma_pinned"));
    }

    #[test]
    fn parity_alloc_strategies_dbvl_matches_toml() {
        // Phase 3 migration gate: config/alloc-strategies.dbvl must produce
        // exactly the template+free map the alloc-strategies.toml INTENDED.
        //
        // 2026-08-03: the pre-migration TOML loader had a latent bug —
        // `[alloc.pool_serial]` parses to a nested `alloc` table, so
        // `strip_prefix("alloc.")` never matched and the old config always
        // loaded an EMPTY map. The DBVL loader fixes this (strategies now
        // actually load). The parity test therefore walks the nested `alloc`
        // table to compare against the TOML's intent, not its broken output.
        // The .toml is deleted; this is now a GOLDEN test — the pre-migration
        // TOML is baked below and re-parsed.
        let config = AllocConfig::load();
        let raw: toml::Value = toml::from_str(ALLOC_STRATEGIES_TOML_GOLDEN).unwrap();
        let alloc = raw.get("alloc").and_then(toml::Value::as_table).unwrap();
        let mut toml_strategies = HashMap::new();
        for (name, value) in alloc {
            let table = value.as_table().unwrap();
            let template = table.get("template").and_then(toml::Value::as_str).unwrap();
            let free = table.get("free").and_then(toml::Value::as_str);
            toml_strategies.insert(name.clone(), (template.to_string(), free.map(|s| s.to_string())));
        }

        assert_eq!(config.strategies.len(), toml_strategies.len(),
            "strategy count diverges between .dbvl and .toml");
        for (name, (toml_template, toml_free)) in &toml_strategies {
            let entry = config.strategies.get(name)
                .unwrap_or_else(|| panic!("strategy '{}' missing from alloc-strategies.dbvl", name));
            // Compare templates with trailing whitespace trimmed: the TOML
            // multi-line `"""` string carries a trailing newline that the
            // emitter's writeln! makes cosmetic (an extra blank line in valid
            // LLVM IR), so a trailing-newline-only divergence is not semantic.
            assert_eq!(entry.template.trim_end(), toml_template.trim_end(),
                "template for '{}' diverges", name);
            assert_eq!(&entry.free, toml_free,
                "free for '{}' diverges", name);
        }
    }
}
