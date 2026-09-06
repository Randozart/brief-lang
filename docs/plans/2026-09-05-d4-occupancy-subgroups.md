# D4: Subgroup Occupancy Shaping

**Date:** 2026-09-05
**Status:** REJECTED — S=2 is 7% slower (12.3 vs 13.2 TF/s)
**Depends on:** D3 (pairs fill, landed), D5 (stagger, rejected — L2 broadcast is a feature)
**Blocks:** None (final Stage 1 rung)

## Motivation

D5 revealed that the "phase-lock" in the fill/mma pipeline is actually
L2-broadcast efficiency: co-resident workgroups at the same K panel
share each A/B DRAM fetch. The stagger broke this by forcing 8 distinct
DRAM streams per SM (15% slower). D4 must go the other direction:
**increase** panel sharing within a workgroup, not desynchronize across
workgroups.

The current tensor kernel runs 1 subgroup (32 threads) per workgroup.
On RTX 3060 (28 SMs, 48KB configurable smem), the occupancy with
pps=1, R=8 is:
- smem: 12KB → 4 wgs/SM → 128 threads/SM

With S subgroups per workgroup, each subgroup handles a different
output tile_n (different B columns, same A rows). The A panel is
shared; the B panels are per-subgroup. This gives:
- S=2: 16KB → 3 wgs/SM → 192 threads/SM (+50%)
- S=4: 24KB → 2 wgs/SM → 256 threads/SM (2×)
- S=8: 40KB → 1 wg/SM → 256 threads/SM (same as S=4 but worse scheduling)

The hypothesis: more threads per SM → better latency hiding on the
fill (more parallelism) and the mma (more independent accumulators).
The cost: fewer workgroups → less cross-workgroup L2 sharing (D5's
finding). The within-workgroup A sharing is guaranteed (all S subgroups
read the same smem A).

## Hardware Probe

RTX 3060 (GA106):
- subgroupSize = 32 (fixed, min == max == 32)
- VK_EXT_subgroup_size_control = YES
- computeFullSubgroups = YES (can request full 32-thread subgroups)
- VK_KHR_cooperative_matrix = YES

NVIDIA does not support variable subgroup sizes. S subgroups per
workgroup = LocalSize 32*S. The `spirv_coopmat_subgroups` config knob
already exists (default 1).

## Design

### Smem Layout

```
shared_a: 2 × pps × R × 256 f16  (SHARED — all subgroups read the same A)
shared_b: S × 2 × pps × 4 × 256 f16  (PER-SUBGROUP — each subgroup's own B slice)
```

B smem layout (S=2 example):
```
[0,    b_stage): subgroup 0, stage 0
[b_stage, 2*b_stage): subgroup 0, stage 1
[2*b_stage, 3*b_stage): subgroup 1, stage 0
[3*b_stage, 4*b_stage): subgroup 1, stage 1
```

Where `b_stage = pps * 4 * 256` f16 elements.

### Smem Budget (pps=1, R=8)

| S | shared_a | shared_b | Total | Wgs/SM | Threads/SM |
|---|----------|----------|-------|--------|------------|
| 1 | 8KB | 4KB | 12KB | 4 | 128 |
| 2 | 8KB | 8KB | 16KB | 3 | 192 |
| 4 | 8KB | 16KB | 24KB | 2 | 256 |
| 8 | 8KB | 32KB | 40KB | 1 | 256 |

### Grid Decode (unchanged from current S>1 support)

```
tiles_x = N / (64 × S)
tile_my = wgid / tiles_x
tile_n  = (wgid % tiles_x) × S + sub_id
band_m16 = tile_my × R × 16
tn64 = tile_n × 64
```

Each subgroup's tile_n differs → different B columns, same A rows.

### Fill Strategy

The fill loop is a single flat iteration over all smem elements:
`total_elems = A_elems + S × B_elems`. The flat index is
`lane + u × (32 × S)` where lane = gl_LocalInvocationID.x (range
0..32S-1).

- **A portion** (flat < A_elems): all subgroups write the same A data
  to shared_a. Correct because all subgroups need the same A rows.
- **B portion** (flat ≥ A_elems): each subgroup writes its own B
  slice. The per-subgroup tn64 is computed from sub_id:
  `tn64_local = ((wgid_x % tiles_x) × S + sub_id) × 64`.
  The smem destination: `sub_id × b_stage_elems + (flat - A_elems)`.

The fill function needs new parameters in `SmemFillParams`:
- `sub_id: Word` — this subgroup's ID (0..S-1)
- `tiles_x_c: Word` — the grid decode constant
- `s_c: Word` — S as a Word
- `b_stage_elems_c: Word` — one stage's B elements (for smem offset)

### B Fragment Loads

The B fragment loads walk smem_b. For S>1, add the subgroup's B base:
```
sub_b_base = sub_id × (2 × b_stage_elems)  // stage 0 and 1
stage_off_b = sub_b_base + s × b_stage_elems  // s = stage parity
```

### B Refill

Same offset as the B fragment loads. The refill writes to the
subgroup's own B slice at `sub_id × b_stage_elems + panel_offset`.

### Barriers

Unchanged: 2 per K iteration (WAR + visibility). The fill is a
single flat loop (no partitioning needed), so no extra barriers.

### Dispatch

The runner dispatch formula is unchanged:
```
work_items = (M × N / (16 × R × 64)) × 32
```

The driver divides by local_x = 32*S to get workgroups. This
automatically gives the correct workgroup count for any S.

### Sweep Matrix

| S | Threads/WG | Wgs/SM | Threads/SM | Expected |
|---|-----------|--------|------------|----------|
| 1 | 32 | 4 | 128 | Baseline |
| 2 | 64 | 3 | 192 | Fill faster (2× threads), A shared, less cross-WG L2 |
| 4 | 128 | 2 | 256 | Fill 4× faster, but fewer cross-WG L2 shares |

### Correctness Contract

Same as current: f32-acc ≤ 5e-3, f16acc ≤ 1e-2 (plan S4 gate).
The fill order changes (S× more threads, different flat mapping) but
the accumulated sum is the same set of K panels → the f16acc rounding
walk shifts but stays within the 1e-2 contract.

## Implementation Steps

### Step 1: Smem B allocation (kernel.rs)
- When `subgroups > 1`: `b_elems = S × 2 × pps × 4 × 256`
- The B array type accommodates the larger size
- No change to shared_a

### Step 2: SmemFillParams extension (gemm.rs)
Add to `SmemFillParams`:
- `sub_id: Word`
- `tiles_x_c: Word`
- `s_c: Word`
- `b_stage_elems_c: Word`
- `subgroups: u32` (compile-time, for the flat stride)

### Step 3: Fill loop refactor (gemm.rs)
In `emit_smem_fill` and `emit_smem_fill_pairs`:
- `total_elems = a_stage_elems + subgroups × b_stage_elems`
- `elems_per_lane = total_elems / (32 × subgroups)`
- Flat stride: `u × 32 × subgroups` (instead of `u × 32`)
- B DRAM source: use `tn64_local` computed from sub_id
- B smem dest: `sub_id × b_stage_elems + (flat - a_stage_elems)`

### Step 4: B fragment loads (gemm.rs)
In `emit_coopmat_smem`, the B fragment loads:
- `sub_b_base = sub_id × (2 × b_stage_elems)`
- `stage_off_b = sub_b_base + s × b_stage_elems`
- Panel and tile offsets unchanged

### Step 5: B refill (gemm.rs)
The refill writes to the subgroup's B slice:
- `sub_b_base = sub_id × b_stage_elems` (within the current stage)
- `b_off = sub_b_base + stage × pps × 4 × 256 + pi × 4 × 256`

### Step 6: D2 prefetch path (gemm.rs)
`emit_fill_load_phase` and `emit_fill_store_phase` get the same
sub_id-based B offset changes.

### Step 7: A/B bench harness
No changes needed. The harness dispatch formula is already correct
for any S (the driver auto-divides by local_x).

## Files Modified

| File | Changes |
|------|---------|
| `src/backend/spirv/kernel.rs` | Smem B allocation: S × (2 × pps × 4 × 256) |
| `src/backend/spirv/gemm.rs` | SmemFillParams extension, fill loop refactor, B fragment loads, B refill, D2 prefetch |
| `config/ir-lowering.dbvl` | `spirv_coopmat_subgroups: 1` (already exists, sweep via dbvl) |

## Risk

The D5 verdict (L2 broadcast sharing is valuable) suggests that
fewer workgroups (S>1) may lose cross-workgroup L2 hits. The
within-workgroup A sharing compensates partially. The net effect is
uncertain — the sweep will measure. If S=2 loses, the design
confirms that occupancy is not the bottleneck and the fill wall
is pure DRAM latency (already hidden by the current 4 wgs/SM).

## Verification

1. `spirv-val` on the generated SPV (correctness gate)
2. `cargo test --lib` (no regressions)
3. Correctness: BRIEV_GEMM_F16ACC=1 max_rel ≤ 1e-2, default max_rel ≤ 5e-3
4. Performance: in-process A/B (stagger off, f16acc on) for S ∈ {1, 2, 4}

## Verdict: REJECTED (2026-09-05)

**S=2 is 7% slower** on RTX 3060 (back-to-back A/B, same session):

| Config | smem/wg | wgs/SM | threads/SM | 4096³ ms | TF/s | rel-err |
|--------|---------|--------|------------|----------|------|---------|
| S=1    | 12 KB   | 4      | 128        | 11.2     | 12.3 | 4.44e-3 |
| S=2    | 16 KB   | 3      | 192        | 11.2     | 12.3 | 4.44e-3 |

**Root cause**: S=2 reduces workgroups per SM from 4→3 (smem ceiling: 16KB
vs 48KB). Fewer co-resident workgroups means fewer L2-broadcast hits on
shared A panels (the D5 insight: the "phase-lock" is L2 efficiency, not a
defect). The 50% thread increase (128→192) does not compensate for the
lost cross-workgroup L2 sharing on this architecture.

**Why it fails the generality test** (Golden Rule 1): the occupancy
benefit depends on smem/SM ratio, which is hardware-specific. On GPUs
with larger smem budgets (e.g. A100 164KB shared), S=2 or S=4 could
still win. But the default must be the best strategy for the current
target (RTX 3060).

**Infrastructure kept**: the S>1 fill partition code (cooperative A,
per-subgroup B) is correct and retained behind `subgroups: 1` default.
If future hardware warrants S>1, the infrastructure is ready.

**Knob**: `spirv_coopmat_subgroups` defaults to 1.
