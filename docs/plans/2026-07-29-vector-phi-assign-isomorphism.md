# Vector Phi: Fix `Statement::Assign` Isomorphism Detection

## Current Results

Commit `066b86a7` (dispatch guardrail + RHS mapping fix + vector phi infrastructure fixes):

**All 19/19 runtime benchmarks MATCH. Zero MISMATCH.**

| Benchmark | Ratio | Winner | Correct |
|-----------|-------|--------|---------|
| ring_buffer | 1.02x | C | MATCH |
| float_math | 0.99x | Briev | MATCH |
| float_math_nonzero | 0.96x | Briev | MATCH |
| sparse_dispatch | 0.83x | Briev | MATCH |
| print_loop | 0.97x | Briev | MATCH |
| nbody_newton | **1.22x** | C | MATCH |
| nbody_sqrt | **0.77x** | Briev | MATCH |
| nbody_sqrt_idio | **0.74x** | Briev | MATCH |
| fasta | 1.02x | C | MATCH |
| fannkuch_redux | 0.99x | Briev | MATCH |
| mandelbrot | 1.00x | ~tie | MATCH |
| kalman_filter_runtime | 1.00x | ~tie | MATCH |
| knucleotide | 0.98x | Briev | MATCH |
| cancel_math | 0.84x | Briev | MATCH |
| bit_clear | ~0 | ~tie | MATCH |
| queue_drain | 0.91x | Briev | MATCH |
| queue_drain_sym | 0.88x | Briev | MATCH |
| queue_drain_idio | 0.88x | Briev | MATCH |
| interval_step | 0.97x | Briev | MATCH |

fasta and knucleotide went from MISMATCH to MATCH (dispatch guardrail fix).
nbody_sqrt/nbody_sqrt_idio improved slightly (0.78→0.77, 0.72→0.74).
nbody_newton remains at ~1.22x (pre-existing, independent of these changes).

### Key Fixes

1. **Dispatch guardrail** (mod.rs, commit `88818123`): Skip InlineSsa when body writes
   non-counter state fields. emit_folded_loop passes empty write_set, silently
   dropping non-counter writes. See docs/plans/2026-07-29-dispatch-bug-analysis.md.

2. **RHS mapping for Assign isomorphism** (slp_isomorphism.rs, commit `066b86a7`):
   statements_isomorphic now builds a variable mapping from BOTH LHS and RHS of
   Assignment statements. Fixes false negatives for nbody's velocity/position groups.

3. **Vector phi infrastructure disabled** inside emit_countable_main (counter.rs):
   The vector phi emission has correctness edge cases (duplicate fields, let-binding
   groups, non-power-of-2 widths, backedge naming conflicts). The dispatch-level
   detection in mod.rs still checks for groups, but emission is deferred.

## Remaining Investigation: nbody_newton

### Current State
- **Current**: 11.02s (1.22x C) — dispatched via PerFieldPhi
- **Baseline** (`b39461e2`): 9.81s (1.16x C) — also dispatched via PerFieldPhi
- **Regression**: ~12% slower (9.81s → 11.02s, 1.16x → 1.22x) — pre-existing,
  not caused by these changes. Likely from Phase 4 dispatch simplification.

### Root Cause: Missing RHS Mapping in `statements_isomorphic` for `Statement::Assign`

The vector phi detection chain:
`detect_vector_groups()` → `analyze_body()` → `find_isomorphic_groups()` → `statements_isomorphic()`

`statements_isomorphic` at `src/analysis/slp_isomorphism.rs:113-137` handles two statement types:

**`Statement::Let` (line 119-124)** — CORRECT: builds mapping from RHS expressions:
```rust
let mapping = build_mapping(e1.as_ref()?, e2.as_ref()?)?;
```

**`Statement::Assign` (line 125-134)** — BUG: builds mapping from LHS only:
```rust
let mut mapping = HashMap::new();
try_build_mapping_lhs(l1, l2, &mut mapping)?;  // maps LHS only
exprs_isomorphic(e1, e2, &mapping)?;            // RHS check uses incomplete mapping
```

**Example**: `vx0 = nvx0` vs `vx1 = nvx1`
- LHS mapping: `{"vx0": "vx1"}` ✓
- RHS check: `Expr::Identifier("nvx0")` vs `Expr::Identifier("nvx1")`
  - `mapping.get("nvx0")` → None
  - falls through to `n1 == n2` → `"nvx0" != "nvx1"` → **FALSE**
- No mapping built for RHS identifiers → isomorphism fails

This affects ALL 15 velocity assignments (vx0..vz4 = nvx0..nvz4) and ALL 15 position updates (bx0..bz4 = bx0..bz4 + dt * nvx0..nvz4).

`build_mapping` already exists and correctly builds mappings for arbitrary expressions (it recursively traverses the expression tree). It's already used for `Statement::Let`. The fix is to also call it for `Statement::Assign` and merge the RHS mapping into the LHS mapping.

### Philosophy

The fix is strictly about **phi storage structure**, not about emitting vector instructions. The vector phi infrastructure (`emit_vector_header`, `emit_vector_backedge`, `record_field_update`) uses `insertelement`/`extractelement` to consolidate phis. The per-lane body codegen remains identical scalar operations. LLVM decides whether to auto-vectorize the resulting SSA.

### Implementation

#### File: `src/analysis/slp_isomorphism.rs`

**Location**: `statements_isomorphic` function, the `Statement::Assign` arm (~lines 125-134).

**Current code**:
```rust
(Statement::Assign(l1, e1), Statement::Assign(l2, e2)) => {
    let mut mapping = HashMap::new();
    if !try_build_mapping_lhs(l1, l2, &mut mapping) {
        return None;
    }
    if exprs_isomorphic(e1, e2, &mapping) {
        Some(mapping)
    } else {
        None
    }
}
```

**Proposed code**:
```rust
(Statement::Assign(l1, e1), Statement::Assign(l2, e2)) => {
    let mut mapping = HashMap::new();
    if !try_build_mapping_lhs(l1, l2, &mut mapping) {
        return None;
    }
    // 2026-07-29: Also build mapping from RHS expressions. The LHS-only
    // mapping misses identifiers in the assignment RHS (e.g., nvx0 → nvx1
    // in vx0 = nvx0 vs vx1 = nvx1), causing false negatives for nbody's
    // velocity assignments and position updates.
    if let Some(rhs_map) = build_mapping(e1, e2) {
        mapping.extend(rhs_map);
    }
    if exprs_isomorphic(e1, e2, &mapping) {
        Some(mapping)
    } else {
        None
    }
}
```

### Tests to Add

Add two test cases to `src/analysis/slp_isomorphism.rs` (in the `#[cfg(test)] mod tests` block):

1. **`test_assign_isomorphic_simple_rhs_rename`**: `vx0 = nvx0` vs `vx1 = nvx1` — should be isomorphic (width 2)

2. **`test_nbody_velocity_assign_pattern`**: 5 consecutive velocity assignments (`vx0 = nvx0` through `vx4 = nvx4`) — should form a width-5 group via `analyze_body`

### Verification

1. `cargo test --lib` — all existing tests pass
2. Run new isomorphism tests specifically: `cargo test --lib -- slp_isomorphism`
3. `bash benchmarks/build_and_bench.sh --correctness` — all benchmarks still MATCH
4. `bash benchmarks/build_and_bench.sh --runtime` — check nbody_newton ratio change

Expected: nbody_newton should improve from 1.21x toward the baseline 1.16x. Other benchmarks should be unaffected since their fields don't form vector phi groups.
