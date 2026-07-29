# Two-Pass SLP Consumer Analysis — The Principled Fix
## 2026-07-28

## The Problem

Every SLP gate tried so far has been a heuristic that trades off one benchmark
against another:

| Gate approach | nbody protects | kalman protects | Result |
|---------------|---------------|----------------|--------|
| No gates (SLP unleashed) | ❌ Regressed | ❌ 3.5x | Both fail |
| Depth×width ≥10 | ✅ 1.04x | ✅ 1.01x | Neither benefits |
| Stride gate | ❌ 1.31x | ✅ 1.01x | Nbody regresses |
| Stride gate + lower depth threshold | ❌ Mixed | ❌ Mixed | Both unstable |
| Total gap check | ❌ 1.35x | ❌ 3.6x | Both fail |

The root cause is always the same: **extract→insert chains between vectorized SLP groups**.
When one SLP group's results (extracts) are immediately consumed by another SLP group's
operands (inserts), the overhead doubles without additional compute gain. This is a
**producer-consumer dependency** within the vectorization, not a per-group profitability
issue.

## The Fix: Two-Pass Consumer Analysis

### Pass 1: Form all SLP groups (existing)

`analyze_body()` in `slp_isomorphism.rs` already finds all isomorphic groups and runs
the merge step when ≥10 groups exist. Output: `Vec<SlpIsomorphicGroup>`.

### New: Consumer graph construction

Add `consumer_group_indices: Vec<usize>` to `SlpIsomorphicGroup`. After all groups
are formed, walk the body and record which groups consume which:

```rust
for each group G at index gi:
    for each statement at position pos > G.lane_positions.last():
        for each identifier ref in statement:
            if ref matches G.lhs_names[lane k]:
                if this statement belongs to another SLP group H at index hj:
                    G.consumer_group_indices.push(hj)
```

### Pass 2: Cost-gain analysis

For each group, compute the chain cost:

```rust
fn chain_cost(groups: &[SlpIsomorphicGroup], gi: usize, visited: &mut HashSet<usize>) -> u32 {
    if !visited.insert(gi) { return 0; }  // cycle detection
    let group = &groups[gi];
    // Each consumer lane needs inserts for each unique variable in its template
    let vars_per_lane = count_unique_vars(&group.template_expr, &group.lane_mappings);
    let insert_cost = group.width * vars_per_lane;  // one insert per unique var per lane
    let extract_cost = group.width;  // one extract per lane to get result out
    let compute_gain = group.width * tree_depth(&group.template_expr);
    
    let mut total = insert_cost + extract_cost;
    for &ci in &group.consumer_group_indices {
        total += chain_cost(groups, ci, visited);
    }
    total
}
```

Reject when `chain_cost > compute_gain * 2` (cost exceeds double the direct gain).

### Benchmark predictions

| Benchmark | Pass 1 groups | Consumer chain | Chain cost | Compute gain | Result |
|-----------|--------------|----------------|------------|-------------|--------|
| **nbody_newton** | 48+ (dx/dy/dz/mag) | dx→dsq→mag→vx (4 deep) | ~72 | ~96 (4×24) | ✅ Passes |
| **nbody_sqrt_idio** | 48+ (same + sqrt) | Same + vector Sqrt# | ~72 | ~96 | ✅ Passes |
| **kalman_filter** | 3 (ap rows) + x assign | ap→p (2 deep) | ~144 | ~72 | ❌ Rejected |
| **mandelbrot** | 0 | — | — | — | Already blocked by has_lane_dependency |
| **float_math_nonzero** | ~6 | — | — | — | ✅ No consumers |
| **ring_buffer** | 0 | — | — | — | No SLP groups |

### Implementation Plan

| Step | File | Change | Lines |
|------|------|--------|-------|
| 1 | `slp_isomorphism.rs` | Add `consumer_group_indices` field to `SlpIsomorphicGroup` | 2 |
| 2 | `slp_isomorphism.rs` | Build consumer graph after merge step | 40 |
| 3 | `slp_isomorphism.rs` | Add `chain_cost` + `chain_pass_ok` functions | 40 |
| 4 | `slp_isomorphism.rs` | Return `chain_pass_ok` per group in `SlpAnalysisResult` | 5 |
| 5 | `counter.rs` | Remove stride gate, total_gap check. Keep `consumer_group_indices.is_empty()` | 15 |
| 6 | — | Run full benchmark suite | — |

Total: ~6 hours of implementation work.

## Rollback

If any benchmark regresses, revert the commit and fall back to the stride gate era
(commit `a53ddf14`) which had nbody at 1.04x and kalman at 3.65x.
