# Cluster Investigation Plan: Recovering All-Time Best Results for All 19 Benchmarks

**Date:** 2026-07-28
**Based on:** Research from `docs/research/slp-sroa-attribute-system-analysis.md`
**Principle:** No sacrifices. Every improvement must be independently verified to not regress any benchmark.

---

## Table of Contents

1. [Cluster A: Pure Loop Fold](#cluster-a-pure-loop-fold)
2. [Cluster B: nbody Memory Stores (post-hoist read tracking)](#cluster-b-nbody-memory-stores)
3. [Cluster C: Dispatch Path Selection](#cluster-c-dispatch-path-selection)
4. [Cluster D: Dead Code Elimination (field-level)](#cluster-d-dead-code-elimination)
5. [Cluster E: General IR Bloat / Small Regressions](#cluster-e-general-ir-bloat)
6. [Verification Protocol](#verification-protocol)

---

## Cluster A: Pure Loop Fold

### Affected Benchmarks

| Benchmark | Current Ratio | Best Ratio | Era | Briv Gap |
|-----------|-------------|------------|-----|-----------|
| sparse_dispatch | 0.82x | 0.09x | 5 | **0.73x → Briv loses** (previously won by 11×) |
| queue_drain | 0.97x | 0.01x | 5 | **0.96x → Briv loses** (previously won by 100×) |
| interval_step | 1.01x | 0.01x | 4 | **1.00x → ~tie** (previously won by 100×) |

These benchmarks currently run full 50M-iteration loops because the pure-counter fold
no longer fires. In their best era, the compiler proved the loop body was "pure enough"
to fold to a constant: one `store i64 N` instruction, zero iterations.

### Root Cause

The fold detection at `transition_graph.rs:710` (`is_pure_body`) rejects a body if:

1. **Any non-counter state field is written** (line 729-731). queue_drain writes `queue`,
   interval_step writes `acc`. sparse_dispatch writes only `count` but has 8 reactor nodes
   (fold requires exactly 1).
2. **Any `when` guard body contains FFI** (line 756-758). All three have `PrintLn!` in
   guard bodies.
3. **The bound is a runtime `let` variable** (`GetEnvInt!("BOUND")`) — total_val is None,
   so the fold doesn't fire even if the body passes is_pure_body.

The best era used a DIFFERENT fold mechanism (pre-observability) that either:
- Allowed non-counter writes, or
- Analyzed guard bodies separately from the hot path

### Hypothesis

**H0:** The hot path (excluding `when` guards) can be proven pure for all three benchmarks.
Guard bodies contain the only FFI calls. If we analyze guard bodies and hot path separately,
the hot path passes is_pure_body, and the loop can be folded.

**H1:** queue_drain and interval_step fail is_pure_body because they write to non-counter
state fields (`queue`, `acc`). These fields are DEAD after the loop ends — they are
only read by the swan song guard. If we mark them as post-loop-consumed-only, the fold
can ignore them.

### Verification Tests

**V1 (sparse_dispatch):** Force fold by editing the benchmark to have 1 node instead of 8.
Compile and run. Expected: Briv time drops from ~0.05s to ~0.001s.

**V2 (queue_drain):** Manually hoist the guard body from the hot path and fold the remaining
body. Expected: Briv time drops from ~0.06s to ~0.001s.

**V3 (interval_step):** Same as V2. Expected: Briv time drops from ~0.06s to ~0.001s.

**V4 (global):** Implement a two-phase purity analysis in `transition_graph.rs`:
- Phase 1: analyze guard bodies separately (mark as "has FFI" or "pure")
- Phase 2: analyze hot path body, excluding guard bodies
- If hot path is pure AND all non-counter writes are to fields only read by guards, fold

### Implementation Plan

**Phase 1 (1 commit):** Add `classify_guard_bodies()` to `transition_graph.rs` that returns
a set of "pure guards" (no FFI, no side effects) and "impure guards". The `is_pure_body`
check should skip pure guards.

**Phase 2 (1 commit):** Add `post_guard_read_set` tracking — fields that are only read
by guard bodies. Non-counter writes to these fields should not disqualify the fold.
Uses the existing dead-field analysis infrastructure.

**Phase 3 (1 commit):** Adjust the fold decision in `mod.rs` to allow fold when:
- Hot path is pure (is_pure_body returns true after Phase 1)
- OR body is effectively pure (non-counter writes are to post-guard-read-only fields)
- Bound is constant OR total_val is provided by runtime → fold to call to runtime
  (degraded fold: run the loop but use a single store)

### Risk Assessment

| Risk | Impact | Mitigation |
|------|--------|------------|
| False positive: hot path not actually pure | Wrong output | Phase 1 guard classification must be conservative |
| Non-counter write to a field that IS read by loop | Wrong output | Phase 2 must use live-field analysis correctly |
| Bound is runtime-determined | Fold can't fire | Accept degraded fold (runtime call) or skip fold |
| Multiple nodes | Fold requires 1 node | This is architectural — skip fold for multi-node programs |

---

## Cluster B: nbody Memory Stores (post-hoist read tracking)

### Affected Benchmarks

| Benchmark | Current Ratio | Best Ratio | Era | Briv Gap |
|-----------|-------------|------------|-----|-----------|
| nbody_newton | 1.05x | 0.75x | 5 | **0.30x → Briv loses** (previously won by 33%) |
| nbody_sqrt | 0.97x | 0.85x | 10 | **0.12x → within parity** |
| nbody_sqrt_idio | 0.93x | 0.67x | 10 | **0.26x → Briv loses** (previously won by 49%) |

### Root Cause

When a txn has a swan song guard (`when count % N == 0 { term! -> PrintLn!(energy) }`),
the codegen must ensure the swan song's `%State` reads see the final iteration's values.
Currently, this is implemented as `needs_state_stores_in_body = true` (counter.rs:433-434),
which forces **ALL** state fields to be GEP+store every iteration.

For nbody with 33 fields, this means 33 stores per iteration when only `energy` (1 field)
actually needs to be stored for the swan song.

**Data flow:**
```
pending_post_hoist (counter.rs:433-434)
  → if non-empty: needs_state_stores_in_body = true
    → counter.rs:715-734: for EVERY body assignment, emit GEP+store
      → 33 stores/iteration for nbody
```

### Hypothesis

**H0:** Replace `needs_state_stores_in_body: bool` with
`post_hoist_read_set: HashSet<String>` containing only the fields read by swan song guards.
Only those fields get GEP+store. For nbody: `{"energy"}` → 1 store instead of 33.

**H1:** The remaining gap from 0.67x (Era 10 best) to ~0.90x (current) is entirely from
these redundant stores. Removing them recovers nbody to ~0.80–0.85x. The remaining gap
to 0.67x is from slope-optimized field layout changes between Era 5 and Era 10.

### Verification Tests

**V1:** Force `needs_state_stores_in_body = false` for nbody. Count stores in `.ll`:
- Baseline: 33 stores/iteration → `grep -c "store.*%state" nbody.ll`
- Forced: 0–1 stores → `grep -c "store.*%state" nbody.ll`

**V2:** Run nbody_sqrt_idio with forced off. If Briv time drops from ~3.6s toward ~2.6s,
hypothesis confirmed.

**V3:** Implement `post_hoist_read_set` and run FULL benchmark suite. All 19 must MATCH.
No benchmark should regress (most have no swan song → post_hoist_read_set is empty →
behavior identical to current).

### Implementation Plan

**Step 1 (1 commit):** Add `post_hoist_read_set: HashSet<String>` to `FunctionContext`
(context.rs). Default: empty.

**Step 2 (1 commit):** In `emit_countable_main` (counter.rs:430-434), populate
`post_hoist_read_set` by scanning `pending_post_hoist` statements for field references.

**Step 3 (1 commit):** Replace the blanket store emission (counter.rs:715-734) with a
per-field check:
```rust
if self.fun.needs_state_stores_in_body {
    // Only store fields that are in the post_hoist_read_set
    // OR fields explicitly required by the dispatch path (capped_set)
    if self.fun.post_hoist_read_set.is_empty()
        || self.fun.post_hoist_read_set.contains(n)
    {
        // emit GEP+store
    }
}
```

**Step 4 (1 commit):** Remove `needs_state_stores_in_body` entirely (now unused),
rename to `post_hoist_pending: bool` as a gating flag.

### Risk Assessment

| Risk | Impact | Mitigation |
|------|--------|------------|
| Missing a field needed by swan song | Wrong output | Scan ALL post-hoist statements recursively |
| Another benchmark writes a field the swan song reads | None — currently all fields are stored; reducing to needed subset is always correct | Post_hoist_read_set is a SUBSET of all fields → strictly fewer stores |
| SLP vector_codegen.rs also needs the same fix | Double stores | Apply same post_hoist_read_set check in vector_codegen.rs:316-325 |

---

## Cluster C: Dispatch Path Selection

### Affected Benchmarks

| Benchmark | Current Ratio | Best Ratio | Era | Briv Gap |
|-----------|-------------|------------|-----|-----------|
| float_math | 0.97x | 0.81x | 5 | **0.16x → Briv loses** (previously won by 23%) |
| fannkuch_redux | 1.02x | 0.96x | 5 | **0.06x → C wins** (previously won by 4%) |

### Root Cause

The dispatch path in `mod.rs:2720` selects between:

- **Direct while-loop** (line 2727): `has_body_ffi && total_fields < 16` → GEP+load+store
  everywhere, no phi nodes, `needs_state_stores_in_body = true`.
- **Per-field phi** (line 2730): `!has_body_ffi || total_fields >= 16` → phi-based dispatch,
  Path A zero stores (when no swan song).

float_math has 15 fields and `has_body_ffi = true` (PrintLn! in guard). With `total_fields
= 15 < 16`, it goes direct while-loop. With per-field phi, it would use phi nodes for the
hot path, eliminating redundant stores.

### Hypothesis

**H0:** Moving float_math to per-field phi dispatch (by raising or removing the
`total_fields < 16` guard) eliminates the GEP+load+store overhead, recovering the
13% regression.

**H1:** fannkuch_redux (~8 fields, has FFI) also benefits (smaller gain, ~3%).

**H2:** The `total_fields < 16` guard was added speculatively without empirical data.
Removing it entirely (always use per-field phi) benefits ALL benchmarks because phi nodes
are strictly cheaper than GEP+store for hot-path values.

### Verification Tests

**V1:** Force per-field phi for float_math by temporarily removing `total_fields < 16` check.
Compile and run. Expected: Briv time drops from ~0.071s toward ~0.063s.

**V2:** Run FULL benchmark suite with per-field phi forced for ALL benchmarks. If any
benchmark regresses, the guard has a purpose; analyze which and why.

**V3:** If V2 shows regressions, add a principled criterion (live-field count vs register
count — same 14-field threshold from Axis 1) instead of the crude `total_fields < 16`.

### Implementation Plan

**Step 1 (1 commit):** Change the dispatch path condition in `mod.rs:2720`:
```rust
// Before:
} else if has_body_ffi && total_fields < 16 {
// After:
} else if has_body_ffi && total_fields < 4 {  // Only for tiny states
```
OR remove the condition entirely (always per-field phi).

**Step 2 (verify):** Run full benchmark suite. If any regression, fall back to a principled
criterion: `live_register_pressure < REGISTER_THRESHOLD` instead of `total_fields < 16`.

**Step 3 (if needed):** Apply the register-pressure criterion from Axis 1 analysis
(live fields vs 14-register x86-64 limit).

### Risk Assessment

| Risk | Impact | Mitigation |
|------|--------|------------|
| Per-field phi uses more registers | Register spilling for many-field benchmarks | The threshold already limits phi-tracked fields via `capped_set` |
| `has_body_ffi` txns need stores for swan song | Store emission still needed | This is about FORMAT (GEP vs phi), not presence of stores |
| Edge case: txn with `total_fields=1` | Overhead from phi setup | Inline the single phi → no overhead |

---

## Cluster D: Dead Code Elimination (field-level)

### Affected Benchmarks

| Benchmark | Current Ratio | Best Ratio | Era | Briv Gap |
|-----------|-------------|------------|-----|-----------|
| bit_clear | 0.50x (noise) | 0.50x | 10 | **None — noise floor** |
| cancel_math | 0.97x | 0.96x | 14 | **None — within noise** |

### Root Cause

These benchmarks are at the noise floor (~0.0001s–0.06s). The "regression" is
within single-run variance. No action needed unless a cluster A–C fix causes a
measurable regression.

### Hypothesis

**H0:** These benchmarks are at their all-time-best within noise. No fix needed.

### Verification

Run 3+ consecutive benchmarks. If the min time is within 5% of the all-time best,
the benchmark is at its best. Document the result.

---

## Cluster E: General IR Bloat / Small Regressions

### Affected Benchmarks

| Benchmark | Current Ratio | Best Ratio | Era | Briv Gap |
|-----------|-------------|------------|-----|-----------|
| knucleotide | 0.99x | 0.97x | 1 | **0.02x → within noise** |
| kalman_filter_runtime | 0.99x | 0.95x | 1 | **0.04x → within noise** |
| float_math_nonzero | 0.99x | 0.98x | 10 | **0.01x → within noise** |
| queue_drain_sym | 0.99x | 0.95x | 5 | **0.04x → within noise** |
| queue_drain_idio | 0.98x | 0.93x | 14 | **0.05x → within noise** |

### Root Cause

These are within 3–5% of their all-time best. The gap is single-run noise or marginal
IR changes from the SLP/outlining/attribute fixes. They should be RE-TESTED after
clusters A–C are resolved, as those fixes may incidentally affect the minor candidates.

### Hypothesis

**H0:** After clusters A–C, all of these return to their all-time bests within noise.

**H1:** kalman_filter_runtime at 0.99x is at its best. The 0.95x from Era 1 was a
fluke (pre-SLP, pre-outlining, very different compiler).

**H2:** queue_drain_sym at 0.95x has the MI pipeline issue (host_irq = true → thread
synchronization cost). This is architectural, not an optimization target.

### Verification

Run full benchmark suite after ALL other clusters are resolved. If any of these
remain below their all-time best by >5%, open a targeted investigation.

---

## Inter-Cluster Dependencies

```
Cluster B (memory stores) ──→ reduces nbody stores 33→1
    │
    └──→ Cluster C (dispatch path) ──→ float_math goes to phi path
            │
            └──→ Cluster A (pure fold) ──→ queue_drain etc. fold
                    │
                    └──→ Clusters D/E (noise floor) ──→ re-measure
```

**B and C are independent** — can be executed in parallel.
**A is independent of B/C** — but harder to implement correctly.
**D/E are dependent on A–C** — resolve last.

---

## Verification Protocol (Every Commit)

```
For each commit in any cluster:

1. cargo test --lib                          # ~30s — must pass
2. cargo build --release                     # ~45s — no new warnings
3. sleep 60                                  # Cooldown for thermal stability
4. rm -f benchmarks/*.ll
5. bash benchmarks/build_and_bench.sh --runtime    # ~7min — ALL 19 at parity or better
6. bash benchmarks/build_and_bench.sh --correctness # ~2min — ALL MATCH
7. Compare against PREDICTED improvement:
   - If cluster B: nbody_sqrt_idio should improve from 0.93x toward 0.67x
   - If cluster C: float_math should improve from 0.97x toward 0.81x
   - If cluster A: queue_drain should improve from 0.97x toward 0.01x
8. If ANY benchmark regresses >0.03x from pre-commit state: git revert HEAD, investigate
```

**Absolute rule:** No benchmark may regress, even temporarily, in service of another.
Every improvement must be independently verified to not harm any other benchmark.
