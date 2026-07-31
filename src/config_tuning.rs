// ── Tuning Configuration (plan §8.1 / §8.2, frontend-driven-dispatch) ─
//
// 2026-07-31: Phase 3 — moves hardcoded codegen tuning constants out of the
// compiler into config, so silent x86_64 assumptions never apply to unknown
// targets and per-target/per-program knobs can evolve without recompiling.
//
// Two files:
//   - `config/targets.toml` `[target.<triple-prefix>]` tables → TargetSettings
//     (per-target: float_registers, dense_compute_density, vector_min_width).
//     Looked up by matching the compiler's target_triple PREFIX. An unknown
//     prefix falls back to x86_64 defaults + `warn_unknown_target` so the
//     fallback is never silent.
//   - `config/ir-lowering.toml` → IrLoweringSettings (global tuning knobs:
//     arena budget/size, stack threshold, SROA chunking, SSO/SVO caps, inline
//     weight). SSO's 6-byte default is derived from the String handle
//     representation (align 8 − 2 tag bits); the config entry is an override.
//
// Both are baked at compile time via include_str! and cached with LazyLock.

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

/// Is the triple's prefix known to config/targets.toml?
pub fn known_target_triple(triple: &str) -> bool {
    TARGET_SETTINGS.keys().any(|p| triple.starts_with(p.as_str()))
}

/// Walk config/targets.toml's `[target.<prefix>]` tables.
fn load_target_settings() -> HashMap<String, TargetSettings> {
    let content = include_str!("../config/targets.toml");
    let raw: toml::Value = toml::from_str(content).unwrap_or_else(|e| {
        panic!("config/targets.toml parse error: {}", e)
    });
    let mut out = HashMap::new();
    let Some(toml::Value::Table(target_table)) = raw.get("target") else {
        return out;
    };
    for (prefix, value) in target_table {
        let toml::Value::Table(t) = value else { continue };
        let float_registers = t.get("float_registers")
            .and_then(toml::Value::as_integer)
            .map(|v| v as usize)
            .unwrap_or(DEFAULT_TARGET_SETTINGS.float_registers);
        let dense_compute_density = t.get("dense_compute_density")
            .and_then(toml::Value::as_float)
            .unwrap_or(DEFAULT_TARGET_SETTINGS.dense_compute_density);
        let vector_min_width = t.get("vector_min_width")
            .and_then(toml::Value::as_integer)
            .map(|v| v as usize)
            .unwrap_or(DEFAULT_TARGET_SETTINGS.vector_min_width);
        out.insert(
            prefix.clone(),
            TargetSettings {
                float_registers,
                dense_compute_density,
                vector_min_width,
            },
        );
    }
    out
}

/// Parse config/ir-lowering.toml with per-key defaults.
fn load_ir_lowering() -> IrLoweringSettings {
    let content = include_str!("../config/ir-lowering.toml");
    let raw: toml::Value = toml::from_str(content).unwrap_or_else(|e| {
        panic!("config/ir-lowering.toml parse error: {}", e)
    });
    let i64_of = |k: &str| -> Option<i64> {
        raw.get(k).and_then(toml::Value::as_integer)
    };
    IrLoweringSettings {
        arena_min_budget: i64_of("arena_min_budget").unwrap_or(DEFAULT_IR_LOWERING.arena_min_budget as i64) as u32,
        arena_initial_size: i64_of("arena_initial_size").unwrap_or(DEFAULT_IR_LOWERING.arena_initial_size as i64) as u64,
        stack_threshold: i64_of("stack_threshold").unwrap_or(DEFAULT_IR_LOWERING.stack_threshold as i64) as u64,
        max_fields_per_alloca: i64_of("max_fields_per_alloca").unwrap_or(DEFAULT_IR_LOWERING.max_fields_per_alloca as i64) as usize,
        sso_max_bytes: i64_of("sso_max_bytes").unwrap_or(DEFAULT_IR_LOWERING.sso_max_bytes as i64) as usize,
        svo_max_elements: i64_of("svo_max_elements").unwrap_or(DEFAULT_IR_LOWERING.svo_max_elements as i64) as usize,
        callable_inline_weight_threshold: i64_of("callable_inline_weight_threshold")
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
    fn test_target_settings_x86_64() {
        let s = target_settings_for("x86_64-unknown-linux-gnu");
        assert_eq!(s.float_registers, 16);
        assert_eq!(s.dense_compute_density, 4.0);
        assert_eq!(s.vector_min_width, 4);
        assert!(known_target_triple("x86_64-unknown-linux-gnu"));
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
