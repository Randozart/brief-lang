# Field Interleaving for SLP Vectorization

Date: 2026-07-05
Status: Execution-ready
Target: Reduce nbody_sqrt from 1.25x to ≤1.05x

## 1. Problem

nbody_sqrt runs at 1.25x vs C. The gap is NOT algorithmic — both compute
the exact same gravity physics. The gap is **field layout preventing SLP
vectorization**.

### Current field order (declaration order in source)

```
vx0, vx1, vx2, vx3, vx4, vy0, vy1, vy2, vy3, vy4, vz0, vz1, vz2, vz3, vz4
```

### Result: stride-5 stores

The body has:
```
&vx0 = ... ;  idx N
&vx1 = ... ;  idx N+1   (sibling vx — stride-1, packable)
...
&vy0 = ... ;  idx N+5   (different vy — stride-5, NOT packable)
```

LLVM's SLP vectorizer requires stride-1 stores to pack into vectors.
At stride-5, it creates separate v2f32 packs for each dimension
(vx's, vy's, vz's) instead of one v4f32 pack for the whole group.

### Impact: 14% more shuffle ops

Briv: 8× v2f32 vector phis, 133 shuffle/insert/extract ops
C:     3× v4f32 vector phis, 117 shuffle/insert/extract ops

## 2. Fix: Field Interleaving

Reorder the field indices so that components of the same group
(vxN, vyN, vzN) are contiguous:

### Before (declaration order):
```
vx0 vx1 vx2 vx3 vx4  vy0 vy1 vy2 vy3 vy4  vz0 vz1 vz2 vz3 vz4
```

### After (interleaved):
```
vx0 vy0 vz0  vx1 vy1 vz1  vx2 vy2 vz2  vx3 vy3 vz3  vx4 vy4 vz4
```

Now `&vx0` (idx N), `&vy0` (idx N+1), `&vz0` (idx N+2) are stride-1.
SLP packs them into a single vector operation.

### Same pattern for positions:
```
bx0 by0 bz0  bx1 by1 bz1  bx2 by2 bz2  bx3 by3 bz3  bx4 by4 bz4
```

## 3. Implementation

### 3.1 Detection

A function `reindex_fields_for_slp` that:

1. Scans the body for `&field = expr` assignments
2. Groups fields by their "component prefix":
   - vxN, vyN, vzN → base="v", digit=N, component=M (v=0, y=1, z=2)
   - bxN, byN, bzN → same pattern
3. For numeric prefix patterns, identifies group boundaries:
   - Group 0: {vx0, vy0, vz0} — digit 0
   - Group 1: {vx1, vy1, vz1} — digit 1
   - ...
4. Computes new indices:
   - Each group gets stride-3 contiguous indices
   - Groups are placed sequentially: group 0 then group 1 then group 2 etc.

### 3.2 Reindexing

Build a mapping from old field index → new field index. Apply as a
permutation to:
- `field_index_map` (field name → new index)
- `field_types` Vec (reorder by new index)
- `field_initializers` Vec (reorder by new index)
- `field_briv_types` Vec (reorder by new index)

### 3.3 Integration

Call `reindex_fields_for_slp` in `emit_countable_main` (or in the
dispatch code in `mod.rs`) before the body is emitted. The function
modifies `self.ctx` fields directly.

## 4. Files to Modify

| File | Change |
|------|--------|
| `src/backend/llvm/loop_engine.rs` | Add `reindex_fields_for_slp` function + call it in `emit_countable_main` |
| No other files | The permutation is applied locally to the context |

## 5. Verification

1. `cargo test --lib` — all 1398+ tests pass (no semantic change)
2. `bash benchmarks/build_and_bench.sh --correctness` — nbody_sqrt MATCH
3. `bash benchmarks/build_and_bench.sh --runtime` — nbody_sqrt from 1.25x to ≤1.05x

## 6. False Paths Investigated and Rejected

| Path | Why Rejected |
|------|-------------|
| FMA codegen | Both Briv and C produce zero FMA on Ivybridge (no -fma flag). Not the gap. |
| Phi emission order | SLP traces def-use chains, not phi list position. Order irrelevant. |
| Energy computation location | Both correctly place it outside the hot loop. No difference. |
| Sqrt vectorization | Both cover all 10 gravity pairs. Briv uses 2 scalar vs C's fully packed, but these add <2% to runtime. |
| MAX_FIELDS_PER_ALLLOCA tuning | Increasing from 15 would make SROA fail for 31-field states. |
| Parallel-safe exemptions | All 30 fields are exempt for both positions and velocities. Disabling parallel-safe entirely would hurt vectorization, not help. |
