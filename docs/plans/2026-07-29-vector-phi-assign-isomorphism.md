# Vector Phi: Fix `Statement::Assign` Isomorphism Detection

## Current Results

Commit `88818123` + `35158e2f`:

**All 19/19 runtime benchmarks MATCH. Zero MISMATCH.**

| Benchmark | Ratio | Winner | Correct |
|-----------|-------|--------|---------|
| ring_buffer | 1.09x | C | MATCH |
| float_math | 1.00x | ~tie | MATCH |
| float_math_nonzero | 0.96x | Brief | MATCH |
| sparse_dispatch | 0.80x | Brief | MATCH |
| print_loop | 1.03x | C | MATCH |
| nbody_newton | **1.21x** | C | MATCH |
| nbody_sqrt | 0.78x | Brief | MATCH |
| nbody_sqrt_idio | 0.72x | Brief | MATCH |
| fasta | **0.98x** | Brief | MATCH |
| fannkuch_redux | 0.90x | Brief | MATCH |
| mandelbrot | 0.99x | Brief | MATCH |
| kalman_filter_runtime | 0.98x | Brief | MATCH |
| knucleotide | **1.00x** | ~tie | MATCH |
| cancel_math | 0.89x | Brief | MATCH |
| bit_clear | 0.66x | Brief | MATCH |
| queue_drain | 0.92x | Brief | MATCH |
| queue_drain_sym | 0.88x | Brief | MATCH |
| queue_drain_idio | 0.93x | Brief | MATCH |
| interval_step | 1.02x | C | MATCH |

fasta and knucleotide went from MISMATCH to MATCH. All other benchmarks stable.

## Remaining Investigation: nbody_newton

### Current State
- **Current**: 10.256s (1.21x C) — dispatched via PerFieldPhi
- **Baseline** (`b39461e2`): 9.81s (1.16x C) — also dispatched via PerFieldPhi
- **Regression**: ~4.5% slower (9.81s → 10.26s, 1.16x → 1.21x)

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
