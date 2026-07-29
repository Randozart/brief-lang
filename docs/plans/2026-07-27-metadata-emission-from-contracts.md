# Metadata Emission from Contracts — Precision Optimization
## 2026-07-27

Continuation of `docs/plans/2026-07-27-cold-path-refinement.md`.

## Overview

The contract system (`[pre][post]`) already proves everything LLVM needs for
precise metadata: state fields are always initialized, field ranges are bounded
by preconditions, and guard frequencies are derivable from induction variables.
The compiler has all this information in `AnalysisResults` but emits none of it
as LLVM metadata.

Three additive phases, each independently revertible:

## Phase 0: `noundef` on State Field Loads (CANCELED)

`noundef` is a parameter/return attribute in LLVM IR, not a load instruction
attribute. It cannot be placed on `load i64, ptr %ptr` — this is rejected by
the LLVM parser. The LLVM LangRef specifies `noundef` only for function
parameters and call return values.

This phase is dropped. All metadata work proceeds from Phase 1.

---

## Phase 1: `!range` from Contracts to Field Loads

### What

The `extract_ranges()` function in `dispatch.rs` already extracts `[lo, hi)` from
preconditions. The `field_to_meta_idx` map already records which field maps to
which metadata node. But `load_field_type()` in `helpers.rs` **never appends**
`!range !N` to the load instruction — the metadata nodes are dead.

### Changes

1. **`context.rs`**: Add `idx_to_field_name: HashMap<usize, String>` — reverse
   index from state field position to field name
2. **`emit_toplevel.rs`**: Populate `idx_to_field_name` in the same loop that
   populates `field_to_meta_idx`
3. **`helpers.rs`**: In `load_field_type()`, after emitting the load instruction:

   ```rust
   if let Some(field_name) = self.ctx.idx_to_field_name.get(&idx) {
       if let Some(mi) = self.ctx.field_to_meta_idx.get(field_name) {
           write!(out, ", !range !{}", mi).ok();
       }
   }
   ```

### Expected impact

~2-3% on ring_buffer (count range [0, 50000000) helps modulo optimization),
~1-2% on float_math, ~1% on others.

### Commit

```
git commit -m "2026-07-27: Phase 1 — !range metadata on contract-bounded fields"
```

### Verification

1. `cargo test --lib` — all pass
2. `bash benchmarks/build_and_bench.sh --runtime` — record table
3. Compare against Phase 0 results

---

## Phase 2: `!prof` from Convergence Induction Proofs

### What

The analysis pipeline already computes `BoundedPre` (var, bound, direction),
`IncrementInfo` (var, delta), and `iter_bounds` per transaction. These flow
to the backend via `AnalysisResults.transition_graph` and `.region_analyzer`.

At each `when` guard's emission point, we can compute the precise number of
times the guard body executes relative to the total iteration count, and emit
`!prof !{!"branch_weights", i32 N, i32 T}` on the guard branch.

### Algorithm

```
guard_cond          = Statement::Guarded.cond (the when condition)
induction_var       = bounded_pre.var         (from transition_graph)
step                = inc.delta.abs()          (from increment_info)
total_iterations    = iteration_bound_of(txn)  (from region_analyzer)
```

Walk `guard_cond` for references to `induction_var`:

| Guard shape | Computation | taken_weight |
|---|---|---|
| `var % N == C` | `ceil(total_iterations / (step × N))` | Guard fires per modulo cycle |
| `var >= N` | `total_iterations - ceil(N/step)` | Fires after threshold |
| `var == N` | `1` if `N` is within bound | Fires exactly once |
| `var == var + step && guard_body resets var` | Detect reset in body | Fires `step / reset_bound` times |
| Other reference to `var` | `total_iterations / count_of_conditions` | Conservative estimate |
| No reference to `var` | No metadata | Can't predict |

### Weight scaling

Weights are scaled to max 1000 to prevent bloated metadata:

```rust
fn scale_weights(taken: u64, not_taken: u64, max: u64) -> (u32, u32) {
    let total = taken + not_taken;
    if total <= max { return (taken as u32, not_taken as u32); }
    let ratio = total as f64 / max as f64;
    ((taken as f64 / ratio).ceil() as u32,
     (not_taken as f64 / ratio).ceil() as u32)
}
```

### Edge cases

- **No bounded_pre for this txn**: No `!prof` metadata. Can't predict frequency.
- **Guard body resets the counter**: Detect `Assign(var, constant)` in guard body.
  Recompute frequency as `total / (reset_value / step)`.
- **Multiple guards reference the same var**: Each guard gets weight derived from
  the number of conditions referencing the same var. If two guards both check
  `var == N`, each fires once.

### Expected impact

~2-3% on ring_buffer, ~1-2% on float_math, ~1% on others. Cumulative with
Phase 0 + Phase 1.

### Commit

```
git commit -m "2026-07-27: Phase 2 — !prof from convergence induction proofs"
```

### Verification

1. `cargo test --lib` — all pass
2. `bash benchmarks/build_and_bench.sh --runtime` — record table
3. Compare against Phase 1 results
4. Verify `!prof` metadata appears in a spot-checked .ll file

---

## Phase 3: Intrinsic-Based Prints (DEFERRED)

Replace `__print_int`/`__print_float`/`__print_char` FFI calls with
`PrintInt#`/`PrintFloat#`/`PrintChar#` intrinsics. Makes print guards naturally
FFI-free — no cold-path outlining needed. Phase 2 already handles guard frequency,
so Phase 3 is additive on top.

**Do not implement until user explicitly approves.**

---

## Cumulative Expected Impact

| Benchmark | Current | + noundef | + range | + prof | + intrinsics |
|-----------|---------|-----------|---------|--------|--------------|
| ring_buffer | 1.15x | ~1.13x | ~1.10x | ~1.07x | ~1.03x |
| float_math | 1.06x | ~1.05x | ~1.03x | ~1.02x | ~1.00x |
| nbody_newton | 1.08x | ~1.07x | ~1.06x | ~1.05x | ~1.03x |
| All others | ~1.0x | ~1.0x | ~1.0x | ~1.0x | ~1.0x |

## Rollback

Each phase is a single commit. If a phase causes regression:

```bash
git revert <commit-hash>   # single clean revert
bash benchmarks/build_and_bench.sh --runtime  # verify recovery
```
