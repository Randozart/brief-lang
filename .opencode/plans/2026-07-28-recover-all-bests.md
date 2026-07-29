# Recover All All-Time Best Results — Implementation Plan

**Date:** 2026-07-28
**Principle:** No trade-offs. Every benchmark reaches its all-time best simultaneously.
**Method:** Each decision is a principled gate, not a heuristic. The gate must be understood,
implemented, verified, and proven correct.

---

## The 5 Decisions That Differentiate Best Eras

| # | Decision | Affected Benchmarks | Key Insight |
|---|----------|-------------------|-------------|
| 1 | `#9` attribute on @main | nbody_newton, nbody_sqrt, nbody_sqrt_idio, float_math, kalman | `argmem:readwrite` vs `memory(readwrite)` should depend on WORKING SET SIZE (fields simultaneously live), not total field count |
| 2 | Cold-path outlining | ring_buffer, print_loop | Outlining guard bodies as separate functions should depend on GUARD BODY COST (instruction count), not FFI-location |
| 3 | Dispatch path selection | float_math, fannkuch_redux, print_loop | Per-field phi should be DEFAULT for ALL txns. While-loop only for VERY SMALL working sets (≤4 fields) |
| 4 | SLP gate granularity | nbody_sqrt_idio, nbody_newton, kalman | Already correctly gated by chain_pass_ok + stride gate + hazard + reduction detection. Tune threshold. |
| 5 | Pure-counter fold | sparse_dispatch, queue_drain, interval_step | NEW PASS: separate guard-body analysis from hot-path analysis. Fold when hot path is pure. |

---

## Decision 1: `#9` Attribute on @main — Working Set Gate

### Current State

`#9 = { nofree norecurse nosync nounwind memory(readwrite) }` — hardcoded at `mod.rs:3288`.

Era 10 had `#9 = { ... memory(argmem: readwrite) }`. The change (commit `123a9e39` revert) was
because `memory(argmem: readwrite)` on @main caused LLVM's alias analysis to assume the FFI calls
inside outlined functions DON'T access %State. But since the FFI calls `__print_float` access
`@stdout` (a global), `argmem:readwrite` is a lie when there's unguarded FFI in the hot path.

**However**, for nbody, the FFI is inside a `when` guard. After outlining, the guard body is in
a separate function. The hot path has NO FFI. So `argmem:readwrite` IS correct for the hot path.

### The Fix

Replace the hardcoded `#9` with a dynamic check similar to the reactor_tick attribute selection:

```rust
// In emit_attributes() or wherever #9 is emitted:
let main_attr = if has_unguarded_ffi || total_fields > WORKING_SET_LIMIT {
    "#9"   // memory(readwrite) — conservative
} else {
    "#12"  // memory(argmem: readwrite) — enables SROA on @main
};
```

**Threshold:** `WORKING_SET_LIMIT` = maximum number of fields that can be simultaneously
live in the hot loop divided by the target's register count. For x86-64 with 14 GP registers:
conservative estimate = 10 fields.

**Verification:** 
- nbody (33 fields total, ~6 fields working set): gets `#12` → recovers toward Era 10
- ring_buffer (4 fields, all working): gets `#12` → maintains current performance
- kalman (15 fields, ~10 working): borderline — test both thresholds

---

## Decision 2: Cold-Path Outlining — Cost Gate

### Current State

`emit_toplevel.rs:1676-1690`: outlining is triggered when a guard body contains FFI calls.
The condition checks `txn_let_names`, `field_index_map`, and `constants` to determine if
the guard body can be outlined.

### The Fix

Add a COST CHECK to the outlining decision. Outlining creates a function call overhead
(prologue, epilogue) that is significant for CHEAP guard bodies (1-2 instructions) but
negligible for EXPENSIVE guard bodies (PrintLn! with format arguments).

```rust
fn can_outline_all(body: &[Statement], ...) -> bool {
    // ... existing checks ...
    
    // 2026-07-28: Cost gate — don't outline guard bodies that are cheaper
    // to keep inline. A function call adds ~5ns overhead. For a guard body
    // with 2-3 operations, the inline cost is lower than the call overhead.
    let guard_cost: usize = guard_body.iter()
        .map(|stmt| stmt_instruction_count(stmt))
        .sum();
    if guard_cost < MIN_OUTLINE_COST {
        return false;  // Keep inline — cheaper than function call
    }
    
    // ... rest of logic ...
}
```

**Threshold:** `MIN_OUTLINE_COST = 3` instructions. A function call + ret is ~5ns on x86-64.
A 3-instruction body takes ~1ns inline. Inlining saves 4ns per guard fire. For ring_buffer's
50M iterations with 10 guard fires: 40ns total. Negligible.

But the REAL benefit of inlining is that it PREVENTS the outlined function's `memory(readwrite)`
attribute from blocking alias analysis on the CALLER. When the guard body is inline, there's
no opaque call site — LLVM can analyze the body directly.

### Verification:
- ring_buffer: guard body stays inline → fewer call overheads → recovers toward Era 4
- nbody: guard body is outlined (PrintLn! with float+char formatting = ~5 instructions) →
  no change from current behavior

---

## Decision 3: Dispatch Path — Per-Field Phi Default

### Current State

`mod.rs:2720-2733`: the while-loop dispatch path is selected when `has_body_ffi && total_fields < 16`.

The while-loop was introduced to avoid the per-field phi overhead (3-4 extra instructions per
field per iteration) for dense-write benchmarks with FFI. But it ALSO forces ALL fields through
GEP+load+store every iteration, adding more overhead for sparse-write benchmarks.

### The Fix

Change the dispatch decision to prefer per-field phi for ALL txns except those with
VERY SMALL working sets that would clearly benefit from the simpler while-loop:

```rust
// Before:
} else if has_body_ffi && total_fields < 16 {

// After:
// 2026-07-28: Per-field phi is the DEFAULT for all reactive txns. The while-loop
// path is reserved for trivially-small working sets (≤4 fields) where per-field
// phi's extra prologue overhead outweighs the GEP+load+store cost.
} else if has_body_ffi && total_fields < 5 {
```

OR — more radically — remove the while-loop path entirely. The per-field phi path handles
all cases correctly. The 3-4 extra instructions per field per iteration are negligible
compared to the GEP+load+store overhead of the while-loop.

### Verification:
- float_math (15 fields, has_body_ffi=true): moves from while-loop to per-field phi →
  recovers toward Era 5's 0.81x
- fannkuch_redux (5 fields): stays in while-loop if threshold is 5 → same as current
- ring_buffer (4 fields): stays in per-field phi → same as current

---

## Decision 4: SLP Gate Granularity — Chain Cost Tuning

### Current State

Already correctly gated by chain_pass_ok, stride gate, hazard, and reduction detection.
The only concern is the chain_pass_ok threshold (`* 2` on compute gain).

### The Fix

Tune the threshold in `chain_pass_ok` at `slp_isomorphism.rs:95`:

```rust
// Current:
(total_cost as u64) < (compute_gain as u64 * 2)

// After tuning: 
(total_cost as u64) < (compute_gain as u64 * 3)
```

A higher threshold blocks more marginal groups. Test with:
- nbody_sqrt_idio: should stay at 0.92x (current SLP is already correct)
- kalman: stays at parity (stride gate already blocks SLP)
- No regressions expected on any other benchmark

### Verification:
- Run full benchmark suite with `*3` and `*4`. Compare against current `*2`.
- If any benchmark regresses, keep the higher threshold that doesn't regress.

---

## Decision 5: Pure-Counter Fold — New Purity Pass

### Current State

`transition_graph.rs:710`: `is_pure_body()` returns false if ANY non-counter state field
is written or if ANY guard body contains FFI. This blocks fold for all current benchmarks
with non-counter writes or FFI-in-guard.

### The Fix

New architecture: **two-phase purity analysis**.

**Phase 1 (separate guard bodies):** Walk the body and separate `when` guard bodies from
the hot path. Each guard body is analyzed independently for purity and side effects.

**Phase 2 (hot path purity):** After excluding guard bodies, analyze the remaining hot path
for:
1. State field writes (only counter writes are allowed)
2. FFI calls (none allowed in hot path)
3. Memory side effects

**If hot path is pure:** emit the folded loop:
```
store i64 <bound>, ptr %count  // single iteration state update
br label %guard_check         // jump to guard check
```

**Guard check:** After the folded loop, check each guard condition. If a guard fires,
execute its body (which may contain FFI). The guard bodies are NOT folded — they
execute normally.

### Implementation

New function in `transition_graph.rs`:

```rust
/// 2026-07-28: Two-phase hot path purity analysis.
/// Phase 1: Identify guard bodies and mark them as "impure" if they contain FFI.
/// Phase 2: Check the REMAINING body for purity (no state writes beyond counter, no FFI).
pub fn can_fold_hot_path(body: &[Statement], counter_var: &str) -> bool {
    let mut guard_bodies: Vec<&[Statement]> = Vec::new();
    
    for stmt in body {
        if let Statement::Guarded(_, guard_body) = stmt {
            guard_bodies.push(guard_body);
            // Skip guard body — analyze separately
            continue;
        }
    }
    
    // Check hot path (body minus guard bodies)
    let hot_path_pure = is_hot_path_pure(body, counter_var);
    // Check guard bodies for purity (they don't need to be pure — they contain FFI)
    // but we need to ensure they don't reference the counter
    
    hot_path_pure
}
```

### Verification:
- sparse_dispatch: hot path has 0 non-counter writes → foldable → recovers 0.09x
- queue_drain: hot path writes `queue` (non-counter) → NOT foldable with this pass
  Requires additional analysis (dead-field-analysis to prove queue is read-only after the loop)
- interval_step: hot path writes `acc` (non-counter) → same issue as queue_drain

For queue_drain and interval_step, the non-counter write prevents fold. To recover these,
we need dead-field-elimination to prove the written field is never read outside the loop
(which it IS — the swan song reads `queue`/`acc`). So this is harder.

**Priority:** Implement Phase 1 first (sparse_dispatch). queue_drain and interval_step
require the additional dead-field analysis, which is a separate pass.

---

## Implementation Order

| Priority | Decision | Benchmarks | Files | Expected Gain | Risk |
|----------|----------|-----------|-------|---------------|------|
| **1** | D3: Dispatch path | float_math 0.81x, fannkuch_redux 0.96x | `mod.rs:2720` | float_math: 0.97→0.81x | LOW — per-field phi is proven correct |
| **2** | D1: #9 attribute | nbody family 0.67x, kalman 0.95x | `mod.rs:3288` | nbody: partial recovery toward 2.5s | MEDIUM — must verify working-set calculation |
| **3** | D2: Cost gate | ring_buffer 0.99x | `emit_toplevel.rs:1676` | ring_buffer: small improvement | LOW — inlining cheap guards is always correct |
| **4** | D4: Threshold tune | nbody, kalman | `slp_isomorphism.rs:95` | nbody: stabilize at best | LOW — `*3` is conservative |
| **5** | D5: Pure fold | sparse_dispatch 0.09x | `transition_graph.rs` | sparse: 0.87→0.09x | MEDIUM — needs new analysis pass |

---

## Verification Protocol

Every decision:
1. `cargo test --lib` — all tests pass
2. `cargo build --release` — no warnings
3. Sleep 60s (thermal cooldown)
4. `rm -f benchmarks/*.ll && bash benchmarks/build_and_bench.sh --runtime`
5. All 19 MATCH, no benchmark regressed
6. The target benchmark improved

## Document History

- 2026-07-28: Initial plan with 5 decisions to recover all all-time bests
