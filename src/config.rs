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
/// Loaded from config/alloc-strategies.toml at compile time.
#[derive(Debug, Clone)]
pub struct AllocConfig {
    strategies: HashMap<String, AllocStrategyEntry>,
}

impl AllocConfig {
    /// Load the built-in alloc strategies file.
    pub fn load() -> Self {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("config/alloc-strategies.toml");
        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => return AllocConfig { strategies: HashMap::new() },
        };
        let raw: HashMap<String, toml::Value> = toml::from_str(&content)
            .unwrap_or_default();
        let mut strategies = HashMap::new();
        for (key, value) in raw {
            if let Some(strat_key) = key.strip_prefix("alloc.") {
                if let toml::Value::Table(table) = value {
                    let template = match table.get("template") {
                        Some(toml::Value::String(t)) => t.clone(),
                        _ => continue,
                    };
                    let free = match table.get("free") {
                        Some(toml::Value::String(f)) => Some(f.clone()),
                        _ => None,
                    };
                    strategies.insert(strat_key.to_string(), AllocStrategyEntry { template, free });
                }
            }
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
