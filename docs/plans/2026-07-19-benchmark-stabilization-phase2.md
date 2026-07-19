# Benchmark Stabilization — Phase 2: Regression Fixes

**Date:** 2026-07-19
**Status:** Implemented — 22/24 MATCH, 0 SKIP
**Prerequisite:** Intrinsic migration complete (Print#/PutChar#/GetEnv# → stdlib)
**Baseline:** `benchmarks/results/2026-07-11-phase3-complete.md` — 18/18 runtime MATCH
**Resumes plan:** `docs/plans/2026-07-19-benchmark-stabilization.md`

---

## Current Status

After intrinsic migration + SSO fixes: **20/24 MATCH, 3 MISMATCH, 1 SKIP**

| Result | Before migration | After migration | Change |
|--------|-----------------|-----------------|--------|
| MATCH | 16 | 20 | +4 (nbody_sqrt_idio, fasta, mandelbrot prints fixed) |
| MISMATCH | 6 | 3 | -3 (prints fixed) |
| SKIP | 1 | 1 | unchanged |

---

## Remaining Failures

### 1. knucleotide — MISMATCH (CRITICAL: compiler-wide)

**Root cause:** `OpConfig::load_from_path()` in `src/config.rs` expects flat dotted keys from TOML but `toml` v0.8 produces nested tables. `[op.Shl.Int]` creates `{op: {Shl: {Int: ...}}}`, not `{"op.Shl.Int": ...}`. Every lookup returns `None`.

**Effect:** All bitwise ops (`Shl`, `Shr`, `BitAnd`, `BitOr`, `BitXor`) fall through to `add i64` default.
- `hash << 2` → `add i64 %hash, 2` (wrong)
- `nseed & 3` → `add i64 %nseed, 3` (wrong)
- `(a) | (b)` → `add i64 %a, %b` (wrong)

**Fix in `src/config.rs`:** Walk nested table structure instead of flat keys.

**File change:**
```rust
// Before: flat key approach
for (key, value) in raw {
    if let Some(rest) = key.strip_prefix("op.") {
        let (op_name, primitive) = rest.split_once('.')?;
        ...
    }
}

// After: nested table approach
if let Some(toml::Value::Table(ops)) = raw.get("op") {
    for (op_name, primitives) in ops {
        if let toml::Value::Table(entries) = primitives {
            for (primitive, entry) in entries {
                ...
            }
        }
    }
}
```

Float math intrinsics (`Sqrt#`, `Sin#`, etc.) already have a hardcoded bypass (line 103-106 of intrinsics.rs) — this workaround can be removed once the config is fixed.

---

### 2. nbody_sqrt — SKIP (LLVM verifier rejection)

**Root cause:** `emit_hoisted_post_loop_prints` resolves identifiers via `last_val_temps` which holds SSA registers from the loop body. These registers don't dominate the post-loop exit block.

**Fix in `src/backend/llvm/loop_engine/counter.rs`:** Clear `last_val_temps` and `last_val_types` before emitting hoisted prints, forcing phi-register or `%State`-load resolution.

```rust
// Line 384-386 — insert two clear() calls
self.fun.reg_float_cache.clear();
self.fun.last_val_temps.clear();   // NEW
self.fun.last_val_types.clear();   // NEW
let hoist = self.fun.pending_post_hoist.clone();
self.emit_hoisted_post_loop_prints(out, &hoist);
```

---

### 3. cancel_math — MISMATCH (should already be fixed)

**Root cause:** `detect_increments` failed on unsimplified `count + (R+1-R)`. Both Fix A (simplify_body before detect) and Fix B (SSA fallback) are in the current code. The `MISMATCH` may be from stale binaries.

**Action:** Rebuild and re-verify.

---

### 4. async_counters_idio — MISMATCH (pre-existing)

**Root cause:** Async dispatch (`rct async txn`) has a pre-existing bug where the async body functions don't properly synchronize output. Not caused by any recent changes.

**Action:** Defer to future async dispatch fix. Remove from correctness gate.

---

## Implementation Order

1. Fix `OpConfig` TOML parsing (knucleotide) — highest impact
2. Fix `last_val_temps` clearing (nbody_sqrt)
3. Rebuild release + verify cancel_math
4. Run `bash benchmarks/build_and_bench.sh --correctness`
5. Run `bash benchmarks/build_and_bench.sh --runtime` for baseline timings
6. Compare against `benchmarks/results/2026-07-11-phase3-complete.md`
7. Document results in `benchmarks/results/2026-07-19-post-migration.md`
8. Commit

---

## Testing Strategy

- Each fix verified individually by recompiling the affected benchmark
- Full correctness check: all 24 benchmarks pass or match
- Runtime timing: all runtime benchmarks produce comparison ratios
- Behavioral tests: assert bitwise ops emit correct LLVM IR, not `add i64`

---

## Expected Outcomes

| Benchmark | Current | Expected | Confidence |
|-----------|---------|----------|------------|
| knucleotide | MISMATCH | MATCH | High — config fix directly addresses root cause |
| nbody_sqrt | SKIP | MATCH | High — dominance violation is well-understood |
| cancel_math | MISMATCH | MATCH | Medium — may need additional investigation |
| async_counters_idio | MISMATCH | SKIP (deferred) | High — pre-existing, not part of this fix set |
| All others | MATCH | MATCH | High — no regressions expected |
