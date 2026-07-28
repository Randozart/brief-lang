# SROA Attribute Selection, SLP Dead Code, and Stride Gate Granularity Fixes

**Date:** 2026-07-28
**Based on:** `docs/research/slp-sroa-attribute-system-analysis.md`
**Targets:** The three independent fix axes identified in the full system analysis.

---

## Axis 1: Attribute Selection on reactor_tick

**Problem:** `dispatch.rs:68` selects `#2` (memory(readwrite)) vs `#12` (argmem:readwrite + willreturn)
for `@reactor_tick` based on whether FFI is inside `when` guards. This is the WRONG criterion.
The correct criterion is: **total live state fields across all reactive txns ≤ register count**.
SROA promotes ALL fields when #12 is selected; for benchmarks with >~14 fields (nbody: 33),
this causes register pressure and spilling.

**Files to change:** `src/backend/llvm/dispatch.rs` (+ import for field count utility)

**Change:**
```rust
// Compute total state fields across all reactive txns
fn total_reactive_fields(txns: &[(String, Transaction)]) -> usize {
    // Counts Let/StateDecl fields that are live in reactive txn bodies
    // Uses field_index_map size as a proxy (all fields are live on entry)
    let mut fields = HashSet::new();
    for (_, t) in txns {
        if !t.is_reactive { continue; }
        for stmt in &t.body {
            // Collect all field references in the txn body
            collect_field_refs(stmt, &mut fields);
        }
    }
    fields.len()
}

let field_count = total_reactive_fields(txns);
let rct_attr = if field_count <= 14 { "#12" } else { "#2" };
```

**Verification test:**
- Build nbody_sqrt_idio with `#2` forced on reactor_tick → should return to ~0.67x
- Build ring_buffer with `#2` forced → should regress to ~1.31x
- Build with field-count gate → nbody gets #2, ring_buffer gets #12 → BOTH at best

---

## Axis 2: Wire `chain_pass_ok` into `should_vec`

**Problem:** `slp_chain_pass_ok` is computed by `analyze_body()` at `slp_isomorphism.rs:94`,
stored in `FunctionContext.slp_chain_pass_ok` at `mod.rs:2384`, and declared on
`context.rs:516`. It is NEVER READ at dispatch time — `counter.rs:668`'s `should_vec`
formula doesn't reference it.

**Files to change:** `src/backend/llvm/loop_engine/counter.rs`

**Change:**
```rust
let chain_ok = self.fun.slp_chain_pass_ok
    .get(gi)                              // gi = group index within all groups
    .copied()
    .unwrap_or(true);                      // If no analysis, assume passes

let should_vec = stride_ok
    && chain_ok                            // ← NEW: consumer chain cost check
    && group.width >= 3
    && template_expr.map_or(false, |expr| {
        tree_depth(expr) * group.width >= 10
    });
```

Need to determine `gi` — the group's index in the `slp_groups` vector. Currently
`match_group` is a direct reference to the group, not its index. Need to either:
- Find the group by `position()` on `self.fun.slp_groups`
- Or pre-store the group index alongside the group reference

**Secondary fix:** Replace `estimate_template_depth` stub in `slp_isomorphism.rs:131`
with actual `tree_depth()` call. Currently it returns `if width > 3 { 2 } else { 1 }`
which is a placeholder, not a real depth estimate.

**Verification test:**
- Build nbody with chain_pass_ok wired → if chain_pass_ok passes for nbody groups,
  SLP emission should fire → compare performance (may be worse due to gather/scatter)
- Build kalman with chain_pass_ok wired → if chain_pass_ok fails for kalman groups,
  SLP should be blocked → kalman stays at parity

---

## Axis 3: Fix Stride Gate Granularity

**Problem:** The stride gate at `counter.rs:654` checks `max_stride ≤ 1` on ALL fields
in the template expression. For nbody's `bx0 - bx1`, field indices [2, 8] give stride=6
→ BLOCKED. But the ACTUAL vector load groups (bx0,by0,bz0 at [2,3,4] and bx1,by1,bz1
at [8,9,10]) ARE contiguous. The gate should check per-lane-vector-group contiguity.

**Files to change:** `src/backend/llvm/loop_engine/counter.rs`

**Change:** Replace the stride check logic:
```rust
// NEW: Per-lane-vector-group contiguity check
// For each template variable, check that its corresponding fields across
// all lanes are contiguous (e.g., bx0(2), by0(3), bz0(4) → stride ≤ 1)
let stride_ok = {
    let template_vars: Vec<String> = {
        let mut v: Vec<String> = Vec::new();
        if let Some(template) = &template_expr {
            collect_idents(template, &mut v);
        }
        v
    };
    template_vars.iter().all(|tv| {
        let indices: Vec<usize> = group.lane_mappings.iter()
            .filter_map(|map| map.get(tv.as_str()))
            .filter_map(|lane_var| self.ctx.field_index_map.get(lane_var.as_str()))
            .copied()
            .collect();
        if indices.len() < 2 { return true; }
        let max_stride = indices.windows(2)
            .map(|w| w[1] - w[0]).max().unwrap_or(0);
        max_stride <= 1
    })
};
```

This uses `group.lane_mappings` which maps each template variable name to lane-specific
variable names. For the dx01/dy01/dz01 group, `lane_mappings` contains:
- Lane 0: {bx0: bx0, bx1: bx1}
- Lane 1: {bx0: by0, bx1: by1}
- Lane 2: {bx0: bz0, bx1: bz1}

So template_var "bx0" → [bx0(2), by0(3), bz0(4)] → stride 1 → PASSES ✓
And template_var "bx1" → [bx1(8), by1(9), bz1(10)] → stride 1 → PASSES ✓

**Verification test:**
- Build nbody_sqrt_idio with per-lane stride check → SLP vectorization fires for
  subtract groups → measure performance (may not improve if cascade effects dominate)
- Build kalman with per-lane stride check → SLP still blocked (kalman's per-lane
  groups are NOT contiguous) → kalman stays at parity

---

## Interaction and Ordering

The three fixes are INDEPENDENT — each can be applied without the others. But the
effects compound:

| Fix | Effect on nbody | Effect on ring_buffer | Effect on kalman |
|-----|----------------|----------------------|-----------------|
| **#1 (attr selector)** | **PRIMARY**: removes SROA pressure → returns to 0.67x | NEGATIVE: loses SROA benefit → returns to 1.31x | NEUTRAL: 15 fields borderline |
| **#2 (chain_pass_ok)** | SECONDARY: gates SLP emission correctly | NONE: no SLP groups | SECONDARY: blocks harmful SLP |
| **#3 (stride fix)** | TERTIARY: enables SLP for nbody groups | NONE: no SLP groups | NONE: kalman groups still non-contiguous |

**Recommended order:** Axis 1 (attribute selector) first, then Axis 2 (chain_pass_ok),
then Axis 3 (stride fix). Each verified independently before proceeding.

---

## Risk Assessment

| Fix | Risk | Mitigation |
|-----|------|-----------|
| Axis 1 | ring_buffer may regress (14-field threshold is too conservative) | Tune threshold: 14, 20, or dynamic based on live-set analysis |
| Axis 1 | Other benchmarks with 15+ fields may regress | Only nbody has >14 fields; kalman at 15 is borderline but baseline shows parity |
| Axis 2 | chain_pass_ok with crude depth estimate may block beneficial SLP | Fix depth estimator first (replace stub with actual tree_depth) |
| Axis 3 | per-lane contiguity may miss some cases where non-contiguous lanes are OK | Fallback: stride_ok = true for all groups when per-lane groups are contiguous |
| All | SROA + SLP interaction has not been tested at this field count | Run full benchmark suite after each change |

---

## Verification Procedure

Each axis verified with:
1. `cargo test --lib` — all tests pass
2. `cargo build --release` — no warnings
3. Full benchmark suite (`--runtime` + `--correctness`) with 60s cooldown
4. Compare against pre-change results for ALL 19 benchmarks
5. Hypothesis test: nbody should improve, ring_buffer/kalman should not regress
