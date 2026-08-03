// ── Tuning Configuration (plan §8.1 / §8.2, frontend-driven-dispatch) ─
//
// 2026-07-31: Phase 3 — moves hardcoded codegen tuning constants out of the
// compiler into config, so silent x86_64 assumptions never apply to unknown
// targets and per-target/per-program knobs can evolve without recompiling.
//
// Two files:
//   - `config/targets.dbvl` `target.<triple-prefix>` entries → TargetSettings
//     (per-target: float_registers, dense_compute_density, vector_min_width).
//     Looked up by matching the compiler's target_triple PREFIX. An unknown
//     prefix falls back to x86_64 defaults + `warn_unknown_target` so the
//     fallback is never silent.
//   - `config/ir-lowering.dbvl` → IrLoweringSettings (global tuning knobs:
//     arena budget/size, stack threshold, SROA chunking, SSO/SVO caps, inline
//     weight). SSO's 6-byte default is derived from the String handle
//     representation (align 8 − 2 tag bits); the config entry is an override.
//
// 2026-08-03 (Phase 3, data-brief-config plan): both files migrated from TOML
// to the flat .dbvl line-table form, still baked at compile time via include_str!
// and cached with LazyLock.

use std::collections::HashMap;
use std::sync::LazyLock;

/// Per-target codegen tuning (plan §8.1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TargetSettings {
    /// Register-pressure budget for vector-phi promotion.
    pub float_registers: usize,
    /// Cross-float-ops-per-field threshold for the `#11 → #0` downgrade.
    pub dense_compute_density: f64,
    /// Minimum isomorphic-group width for vector-phi promotion.
    pub vector_min_width: usize,
}

/// Global (target-independent) IR lowering tuning (plan §8.2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct IrLoweringSettings {
    /// Below this `--optimize-budget`, skip the bump arena (direct malloc).
    pub arena_min_budget: u32,
    /// Initial per-txn bump arena size.
    pub arena_initial_size: u64,
    /// Stack-allocation threshold for transient collections.
    pub stack_threshold: u64,
    /// %StateChunk<N> field cap so SROA can decompose each chunk.
    pub max_fields_per_alloca: usize,
    /// SSO small-string inline payload cap (derived: align 8 − 2 tag bits).
    pub sso_max_bytes: usize,
    /// SVO small-vector inline element cap.
    pub svo_max_elements: usize,
    /// Weighted body-cost threshold for callable-txn auto-inline.
    pub callable_inline_weight_threshold: u32,
}

/// x86_64 defaults — also the fallback for unknown target prefixes.
pub const DEFAULT_TARGET_SETTINGS: TargetSettings = TargetSettings {
    float_registers: 16,
    dense_compute_density: 4.0,
    vector_min_width: 4,
};

const DEFAULT_IR_LOWERING: IrLoweringSettings = IrLoweringSettings {
    arena_min_budget: 128,
    arena_initial_size: 65536,
    stack_threshold: 4096,
    max_fields_per_alloca: 15,
    sso_max_bytes: 6,
    svo_max_elements: 3,
    callable_inline_weight_threshold: 40,
};

/// Per-target-prefix tuning tables, keyed by triple prefix (e.g. "x86_64").
static TARGET_SETTINGS: LazyLock<HashMap<String, TargetSettings>> =
    LazyLock::new(load_target_settings);

/// Global IR-lowering tuning (baked config/ir-lowering.toml).
static IR_LOWERING: LazyLock<IrLoweringSettings> = LazyLock::new(load_ir_lowering);

/// Return the global IR-lowering settings.
pub fn ir_lowering() -> &'static IrLoweringSettings {
    &IR_LOWERING
}

/// Resolve tuning settings for a target triple by longest-prefix match.
///
/// 2026-07-31: Matches `ctx.target_triple` against the `[target.<prefix>]`
/// keys. Falls back to the x86_64 defaults; `warn_unknown_target` reports the
/// fallback so unknown targets never silently inherit x86 assumptions.
pub fn target_settings_for(triple: &str) -> TargetSettings {
    let mut best: Option<(usize, &TargetSettings)> = None;
    for (prefix, settings) in TARGET_SETTINGS.iter() {
        if triple.starts_with(prefix.as_str()) && best.map_or(true, |(blen, _)| prefix.len() > blen) {
            best = Some((prefix.len(), settings));
        }
    }
    best.map(|(_, s)| *s).unwrap_or(DEFAULT_TARGET_SETTINGS)
}

/// Is the triple's prefix known to config/targets.dbvl?
pub fn known_target_triple(triple: &str) -> bool {
    TARGET_SETTINGS.keys().any(|p| triple.starts_with(p.as_str()))
}

/// Walk config/targets.dbvl's `target.<prefix>` rows.
///
/// 2026-08-03 (Phase 3, data-brief-config plan): migrated from the
/// `[target.<prefix>]` tables in targets.dbvl. Row shape:
/// `target.<prefix>: <float_registers>; <dense_compute_density>; <vector_min_width>;`.
fn load_target_settings() -> HashMap<String, TargetSettings> {
    let content = include_str!("../config/targets.dbvl");
    let db = match crate::dbrief::config_db::ConfigDb::from_str(content) {
        Ok(db) => db,
        Err(e) => panic!("config/targets.dbvl parse error: {}", e),
    };
    let mut out = HashMap::new();
    for key in db.keys() {
        let Some(prefix) = key.strip_prefix("target.") else { continue };
        let float_registers = db
            .field_int(&key, 0)
            .map(|v| v as usize)
            .unwrap_or(DEFAULT_TARGET_SETTINGS.float_registers);
        let dense_compute_density = db
            .field_float(&key, 1)
            .unwrap_or(DEFAULT_TARGET_SETTINGS.dense_compute_density);
        let vector_min_width = db
            .field_int(&key, 2)
            .map(|v| v as usize)
            .unwrap_or(DEFAULT_TARGET_SETTINGS.vector_min_width);
        out.insert(
            prefix.to_string(),
            TargetSettings {
                float_registers,
                dense_compute_density,
                vector_min_width,
            },
        );
    }
    out
}

/// Parse config/ir-lowering.dbvl with per-key defaults.
///
/// 2026-08-03 (Phase 3, data-brief-config plan): migrated from ir-lowering.toml
/// to the flat .dbvl line-table form; still compile-time baked via include_str!.
/// Absent keys fall back to the hardcoded defaults, matching pre-migration.
fn load_ir_lowering() -> IrLoweringSettings {
    let content = include_str!("../config/ir-lowering.dbvl");
    let db = match crate::dbrief::config_db::ConfigDb::from_str(content) {
        Ok(db) => db,
        Err(e) => panic!("config/ir-lowering.dbvl parse error: {}", e),
    };
    IrLoweringSettings {
        arena_min_budget: db
            .field_int("arena_min_budget", 0)
            .unwrap_or(DEFAULT_IR_LOWERING.arena_min_budget as i64) as u32,
        arena_initial_size: db
            .field_int("arena_initial_size", 0)
            .unwrap_or(DEFAULT_IR_LOWERING.arena_initial_size as i64) as u64,
        stack_threshold: db
            .field_int("stack_threshold", 0)
            .unwrap_or(DEFAULT_IR_LOWERING.stack_threshold as i64) as u64,
        max_fields_per_alloca: db
            .field_int("max_fields_per_alloca", 0)
            .unwrap_or(DEFAULT_IR_LOWERING.max_fields_per_alloca as i64) as usize,
        sso_max_bytes: db
            .field_int("sso_max_bytes", 0)
            .unwrap_or(DEFAULT_IR_LOWERING.sso_max_bytes as i64) as usize,
        svo_max_elements: db
            .field_int("svo_max_elements", 0)
            .unwrap_or(DEFAULT_IR_LOWERING.svo_max_elements as i64) as usize,
        callable_inline_weight_threshold: db
            .field_int("callable_inline_weight_threshold", 0)
            .unwrap_or(DEFAULT_IR_LOWERING.callable_inline_weight_threshold as i64) as u32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ir_lowering_defaults_match_hardcoded() {
        let s = ir_lowering();
        assert_eq!(s.arena_min_budget, 128);
        assert_eq!(s.arena_initial_size, 65536);
        assert_eq!(s.stack_threshold, 4096);
        assert_eq!(s.max_fields_per_alloca, 15);
        assert_eq!(s.sso_max_bytes, 6);
        assert_eq!(s.svo_max_elements, 3);
        assert_eq!(s.callable_inline_weight_threshold, 40);
    }

    #[test]
    fn parity_ir_lowering_dbvl_matches_toml() {
        // Phase 3 migration gate: config/ir-lowering.dbvl must produce exactly
        // the values the TOML it replaces produces. The .toml is deleted only
        // after this stays green.
        let s = ir_lowering();
        let toml_content =
            include_str!("../config/ir-lowering.toml");
        let raw: toml::Value = toml::from_str(toml_content).unwrap();
        let i64_of = |k: &str| raw.get(k).and_then(toml::Value::as_integer).unwrap();
        assert_eq!(s.arena_min_budget as i64, i64_of("arena_min_budget"));
        assert_eq!(s.arena_initial_size as i64, i64_of("arena_initial_size"));
        assert_eq!(s.stack_threshold as i64, i64_of("stack_threshold"));
        assert_eq!(s.max_fields_per_alloca as i64, i64_of("max_fields_per_alloca"));
        assert_eq!(s.sso_max_bytes as i64, i64_of("sso_max_bytes"));
        assert_eq!(s.svo_max_elements as i64, i64_of("svo_max_elements"));
        assert_eq!(s.callable_inline_weight_threshold as i64, i64_of("callable_inline_weight_threshold"));
    }

    #[test]
    fn test_target_settings_x86_64() {
        let s = target_settings_for("x86_64-unknown-linux-gnu");
        assert_eq!(s.float_registers, 16);
        assert_eq!(s.dense_compute_density, 4.0);
        assert_eq!(s.vector_min_width, 4);
        assert!(known_target_triple("x86_64-unknown-linux-gnu"));
    }

    #[test]
    fn parity_target_settings_dbvl_matches_toml() {
        // Phase 3 migration gate (targets): config/targets.dbvl's `target.*`
        // rows must produce exactly the per-prefix tuning the targets.toml
        // `[target.<prefix>]` tables produce. The .toml is deleted only after
        // this AND parity_targets_dbvl_matches_toml both stay green.
        let db_map = load_target_settings();
        let content = include_str!("../config/targets.toml");
        let raw: toml::Value = toml::from_str(content).unwrap();
        let toml_targets = raw.get("target").and_then(toml::Value::as_table).unwrap();

        assert_eq!(db_map.len(), toml_targets.len(),
            "target-prefix count diverges between .dbvl and .toml");
        for (prefix, value) in toml_targets {
            let t = value.as_table().unwrap();
            let settings = db_map.get(prefix)
                .unwrap_or_else(|| panic!("prefix '{}' missing from targets.dbvl", prefix));
            assert_eq!(settings.float_registers as i64,
                t.get("float_registers").and_then(toml::Value::as_integer).unwrap(),
                "float_registers for '{}' diverges", prefix);
            assert_eq!(settings.dense_compute_density,
                t.get("dense_compute_density").and_then(toml::Value::as_float).unwrap(),
                "dense_compute_density for '{}' diverges", prefix);
            assert_eq!(settings.vector_min_width as i64,
                t.get("vector_min_width").and_then(toml::Value::as_integer).unwrap(),
                "vector_min_width for '{}' diverges", prefix);
        }
    }

    #[test]
    fn test_target_settings_aarch64() {
        let s = target_settings_for("aarch64-unknown-linux-gnu");
        assert_eq!(s.float_registers, 32);
    }

    #[test]
    fn test_target_settings_wasm_unlimited() {
        let s = target_settings_for("wasm32-unknown-wasi");
        assert_eq!(s.float_registers, 4294967295);
    }

    #[test]
    fn test_unknown_target_falls_back_to_x86() {
        let s = target_settings_for("mips-unknown-linux");
        assert_eq!(s, DEFAULT_TARGET_SETTINGS);
        assert!(!known_target_triple("mips-unknown-linux"));
    }

    #[test]
    fn test_longest_prefix_wins() {
        // A more specific prefix beats a shorter one (both present).
        let s = target_settings_for("x86_64-foo");
        assert_eq!(s.float_registers, 16);
    }
}
