# Kernel Profiling Analysis — Fill Instruction Dominance

**Date:** 2026-09-05
**Status:** Research complete — optimization opportunities identified
**Baseline:** RTX 3060, 4096³ FP16 GEMM, f16acc, smem, fill_pairs, R=4, pps=1, S=1

## Executive Summary

The GPU kernel runs at **5.3 ms (23.3 TFLOP/s)** vs the **3.2 ms target
(42 TFLOP/s)**. The gap is 1.66×. Root cause: **891 integer div/mod
instructions per workgroup** for shared memory fill index decomposition,
drowning the 16 useful MMA operations. This is a compiler codegen issue,
not a hardware limitation.

## Baseline Measurements

| Metric | Value | Source |
|--------|-------|--------|
| GPU kernel time (solo) | 5.332 ms | Vulkan timestamp |
| GPU kernel time (batched ×10) | 5.858 ms | Vulkan timestamp |
| Host-side per-call | 5.892 ms | Host timer |
| Host overhead | ~0.034 ms | Difference |
| FLOPS | 23.3 TFLOP/s | 2×4096³ / 5.332ms |
| Target (ggml-cuda) | 42.0 TFLOP/s | 3.205 ms |
| Mma ceiling | ≥107 TFLOP/s | Stage 0 measurement |

## Instruction Mix Analysis

### Production Kernel (R=4, pps=1, S=1, fill_pairs=1)

| Category | Count | % of Total | Notes |
|----------|-------|-----------|-------|
| IDiv/IMod | 891 | 19.5% | Fill index decomposition |
| Bitwise (shift/and) | 193 | 4.2% | Partial strength reduction |
| Select (A/B branch) | 386 | 8.4% | Dual-matrix pointer selection |
| AccessChain | 409 | 8.9% | Pointer arithmetic |
| OpLoad (SSBO) | 98 | 2.1% | Global memory reads |
| OpStore (SSBO) | 96 | 2.1% | Global memory writes |
| CM Load (smem→frag) | 8 | 0.2% | Cooperative matrix loads |
| CM Store (frag→smem) | 16 | 0.4% | Cooperative matrix stores |
| **CooperativeMatrixMulAdd** | **16** | **0.3%** | **The actual computation** |
| Control flow | 4 | 0.1% | Barriers |
| Other | ~2478 | 54.1% | CompositeExtract, labels, etc. |

**Overhead ratio: 1470 fill-adjacent instructions for 16 MMA ops = 92:1**

### Main Loop Breakdown (per K-step)

```
Fill phase:     ~1200 instructions (6 div + 8 mod + 6 bitwise + 16 select + loads + stores)
CM load phase:   ~25 instructions (8 div/2 + 8 accesschain + 8 CM loads)
MMA phase:       16 CM multiply-adds
Barrier:         1 control barrier
Per-K-step:     ~1241 instructions
Total (K=4096): 1241 × 256 = ~317,696 instructions (loop body only)
Prologue:        ~200 instructions (2 fills × 100 each)
Grand total:     ~317,896 + prologue = ~320,000 (loop body)
```

## Fill Index Decomposition: The Root Cause

### What happens per fill element

The fill loop loads 256 elements (16×16 panel) per iteration using 32 threads.
Each thread processes `elems_per_lane = 256 / 32 = 8` elements. For each element:

```
flat = lane + u × 32                    // 1 IAdd
tile = flat / 256                       // 1 UDiv
elem256 = flat & 255                    // 1 BitwiseAnd (optimized)
row = elem256 / 16                      // 1 UDiv
col = elem256 & 14                      // 1 BitwiseAnd (optimized)
col_pair = col / 2                      // 1 UDiv

A_src = (band_m16 + tile×16 + row) × K/2 + kt×8 + col_pair
       = 3 IMul + 4 IAdd + 1 UDiv (for K/2)
       = ~8 instructions

B_src = (kt×16 + row) × N/2 + (tn64 + j×16)/2 + col_pair
       = similar ~8 instructions

Select(in_a, a_src, b_src)             // 1 Select
AccessChain(ssbo, member, idx)          // 1-2 AccessChain
Load(v2half, ptr)                       // 1 OpLoad
Select(in_a, a_off, b_off)             // 1 Select
AccessChain(wg, idx)                    // 1 AccessChain
Store(v2half, ptr, val)                 // 1 OpStore
```

**Total per element: ~20-25 instructions**

### The problem with divisors

Most divisors ARE power-of-two (16, 64, 256, 1024, 4096), meaning they
*should* compile to shifts/masks. But the SPIR-V codegen emits full UDiv/UMod
because:

1. The divisor is computed at runtime (e.g., `K/2` where K=4096)
2. The SPIR-V backend doesn't have a strength-reduction pass
3. LLVM opt doesn't run on SPIR-V output

**Evidence:** `spirv-dis` shows `OpUDiv %uint %101 %uint_256` — divisor is
a constant (256), yet it's a full UDiv, not `OpShiftRightLogical` with
`OpBitwiseAnd` mask.

### Fix opportunity

Replace all power-of-two UDiv/UMod with shift+mask:
- `flat / 256` → `flat >> 8`
- `flat & 255` → `flat & 0xFF`
- `flat / 16` → `flat >> 4`
- `flat % 256` → `flat & 0xFF`

This eliminates ~697 UDiv + 194 UMod = **891 instructions** → ~0
(replaced by ~80 shift+mask operations).

## Comparison: Briev vs ggml-cuda

| Aspect | Briev (current) | ggml-cuda | Gap |
|--------|----------------|-----------|-----|
| **MMA atom** | cooperativeMatrixMulAddKHR (16×16) | mma.sync.m16n8k16 (raw PTX) | 2× tile area |
| **Fill overhead** | 891 div/mod per workgroup | 0 (cp.async bypasses registers) | **Infinite** |
| **Smem→frag load** | CM load from smem (explicit) | ldmatrix (warp-collective, hardware) | Hardware vs software |
| **Thread count** | 32 (1 warp) | 64–256 (2–8 warps) | 2–8× more |
| **Barriers/K-step** | 2 (pre+post fill) | 0 (cp.async + warp-synchronous) | **2 per step** |
| **Pipeline depth** | 2 stages (double-buffer) | Multi-stage via cp.async | 1 fewer |
| **Smem usage** | ~8 KB | ~4–16 KB | Similar |
| **Bank conflict padding** | None (implicit) | +4 K-dim padding | Missing |

### Why ggml-cuda achieves 42 TFLOP/s

1. **Zero fill overhead**: `cp.async` moves global→smem without touching
   registers or computing addresses in the shader. The hardware does it.
2. **ldmatrix**: Warp-collective smem→register load in exact MMA layout.
   Zero per-element address computation.
3. **2–8 warps per block**: More instruction-level parallelism, better
   latency hiding.
4. **Bank-conflict-free padding**: +4 on K-dimension prevents 8-way conflicts.
5. **Raw PTX MMA**: 1 HMMA instruction per m16n8k16 vs 2 for WMMA.

### What Briev has that ggml-cuda doesn't

1. **Portable SPIR-V codegen**: No nvcc, no cuBLAS dependency
2. **Structural dispatch tables**: Can specialize per program shape
3. **Cooperative matrix (16×16)**: Larger atom than m16n8k16
4. **B-reuse**: R A-fragments share 4 B-fragments per K-step

## Optimization Opportunities

### O1: SPIR-V Strength Reduction for Fill Index Math (HIGH IMPACT)

**Impact:** ~20–40% of kernel time (891 UDiv/UMod → ~80 shift/mask)
**Effort:** Add a SPIR-V peephole pass that recognizes power-of-two divisors
and replaces with shift+mask.

**Implementation options:**
1. Post-process the SPIR-V binary with `spirv-opt` (if it has this pass)
2. Add a custom SPIR-V pass in the compiler backend
3. Modify the fill codegen to emit shift+mask directly (preferred)

**For option 3** (modify `emit_smem_fill_pairs`):
- Instead of `flat / N` where N is power-of-two, emit `flat >> log2(N)`
- Instead of `flat % N` where N is power-of-two, emit `flat & (N-1)`
- The divisors are known at codegen time (K, N are constants in the SSBO layout)

### O2: Eliminate Fill Per-Element Address Recomputation (HIGH IMPACT)

**Impact:** ~15–25% (eliminate redundant mul/add chains)
**Effort:** Medium — restructure fill loop to use linear addressing

Current: For each element, recompute `(band_m16 + tile×16 + row) × K/2 + ...`
Alternative: Compute base address once per thread, then increment by stride

```
base_a = (lane * 2) * K/2         // one multiply per thread
for u in 0..elems_per_lane:
    a_addr = base_a + u * 32 * 2   // one add per iteration (stride constant)
    // No div/mod needed — stride is constant
```

This works because `flat = lane + u × 32` is linear in `u`, so the address
is linear in `u` and can be incremented rather than recomputed.

### O3: Reduce Barrier Count (MEDIUM IMPACT)

**Impact:** ~10–15% (eliminate 1 of 2 barriers per K-step)
**Effort:** Medium — requires pipeline restructuring

Current: `fill → barrier → MMA → barrier → (loop back)`
Target: `prefetch_fill → MMA → barrier → store_fill → barrier` (D2 prefetch)

The D2 prefetch path already exists (`fill_prefetch=1`) but is disabled.
Re-enabling it with the fill_pairs optimization could reduce effective
barrier cost by overlapping DRAM fetches with MMA compute.

### O4: Bank-Conflict Padding (MEDIUM IMPACT)

**Impact:** ~5–10% (depends on access pattern)
**Effort:** Low — add +4 padding to smem layout

ggml-cuda pads the K-dimension by 4 to avoid 8-way bank conflicts.
Briev's smem layout has row stride = 16 (for v2f16: stride = 8 pairs).
Adjacent threads reading different rows at the same column will hit the
same bank.

### O5: Multi-Warp Workgroups (LOW-MEDIUM IMPACT)

**Impact:** ~5–15% (more ILP, better latency hiding)
**Effort:** Medium — requires S>1 with proper fill partitioning

Currently S=1 (32 lanes). S=2 was rejected because it reduced WGs/SM from
4→3, hurting L2 broadcast sharing. But with the fill overhead eliminated,
S=2 may now help because the remaining work (MMA + CM loads) benefits
from more ILP within the workgroup.

**Prerequisite:** O1 and O2 must land first to reduce fill cost.

### O6: Increase R (Tile Rows) (LOW IMPACT, MAY BE NEGATIVE)

R=8 would halve B-traffic but R=16 was rejected (register spill).
R=4→R=8 may help but needs careful testing with O1/O2 in place.

## Recommended Implementation Order

| Phase | Rung | Expected Gain | Risk |
|-------|------|--------------|------|
| **Phase 1** | O1: Strength reduction | 20–40% | Low — mechanical transform |
| **Phase 1** | O2: Linear fill addressing | 15–25% | Medium — requires fill loop restructure |
| **Phase 2** | O3: D2 prefetch re-enable | 10–15% | Low — existing code path |
| **Phase 2** | O4: Bank-conflict padding | 5–10% | Low — layout change only |
| **Phase 3** | O5: S=2 with reduced fill | 5–15% | Medium — needs L2 re-evaluation |
| **Phase 3** | O6: R=8 | 5–10% | Medium — register pressure |

**Conservative estimate:** O1+O2 → 35–50% improvement → 3.4–4.0 ms → 29–35 TFLOP/s
**Optimistic estimate:** O1+O2+O3+O4 → 45–60% improvement → 2.7–3.5 ms → 35–45 TFLOP/s

## O1 Implementation Plan: Strength-Reduce Power-of-Two Div/Mod

### What changes

In `src/backend/spirv/gemm.rs`, replace every `OpUDiv` by power-of-two with
`OpShiftRightLogical` and every `OpUMod` by power-of-two with `OpBitwiseAnd`.

### Affected functions

1. **`emit_fill_pair_dram`** (lines 2390-2517):
   - `tile_idx = flat2 / 256` → `flat2 >> 8`
   - `row_in = elem256 / 16` → `elem256 >> 4`
   - `col_pair = (elem256 & 14) / 2` → `(elem256 & 14) >> 1`
   - `b_stage_pairs = b_stage_elems / 2` → `b_stage_elems >> 1`
   - `b_tile_idx = b_flat_within / 256` → `b_flat_within >> 8`
   - `bcol_half = bcol / 2` → `bcol >> 1`

2. **`emit_fill_pair_smem`** (lines 2523-2593):
   - `smem_pair_idx = smem_idx_pair / 2` → `smem_idx_pair >> 1`
   - `b_flat_within = b_flat % b_stage_elems` → `b_flat & (b_stage_elems - 1)` (when b_stage_elems is power-of-two)

3. **Scalar fill** (`emit_smem_fill`, lines 1942-2296):
   - `tile_idx = flat / 256` → `flat >> 8`
   - `row_in = elem256 / 16` → `elem256 >> 4`
   - All other UDiv/UMod with power-of-two constants

4. **Quad fill** (`emit_fill_quad_dram/smem`, lines 2609-2824):
   - Same pattern as pairs but with /4, /256 etc.

### Helper to add

```rust
/// Emit ShiftRightLogical for power-of-two division.
fn u32_shr(builder: &mut SpirvBuilder, val: Word, shift: u32) -> Word {
    let c = u32_const(builder, shift);
    let id = builder.gen_id();
    builder.emit(Instruction::new(
        spirv::Op::ShiftRightLogical, Some(builder.undefined_u32()), Some(id),
        vec![Operand::IdRef(val), Operand::IdRef(c)],
    ));
    id
}
```

### Known power-of-two divisors

| Divisor | Log2 | Replace with |
|---------|------|-------------|
| 2 | 1 | `>> 1` |
| 4 | 2 | `>> 2` |
| 8 | 3 | `>> 3` |
| 16 | 4 | `>> 4` |
| 256 | 8 | `>> 8` |
| 1024 | 10 | `>> 10` |
| b_stage_elems | runtime | need `b_stage_elems_c` to be power-of-two |

### The b_stage_elems caveat

`b_flat_within = b_flat % b_stage_pairs` — `b_stage_pairs` is NOT a
compile-time constant (it depends on `b_stage_elems` which is in
`SmemFillParams`). However, in the current config (S=1), `b_stage_pairs = 1024`
which IS power-of-two. We can add a `b_stage_pairs_log2` field to
`SmemFillParams` and use `b_flat & (b_stage_pairs - 1)` when it's set.

### Expected instruction reduction

| Before | After | Delta |
|--------|-------|-------|
| 697 UDiv | ~0 | -697 |
| 194 UMod | ~5 (non-power-of-two) | -189 |
| 193 Bitwise | ~280 (+shifts) | +87 |
| **Net** | | **-800 instructions** |

### Verification

1. `cargo test --lib` — 2048 tests pass
2. `spirv-val` on generated SPV
3. `spirv-dis` to verify ShiftRightLogical replaces UDiv
4. Run bench: GPU time should drop from 5.3ms toward 3.5-4.0ms

## Verification Plan

1. Implement O1 (strength reduction) → measure GPU time
2. Implement O2 (linear addressing) → measure GPU time
3. Run `cargo test --lib` after each phase
4. Run `spirv-val` on all generated SPV
5. Run bench with Vulkan timestamps for accurate GPU kernel timing
6. Document results in ledger

## Appendix: Key SPIR-V Constants

```
wgid / 64 → tile_my, tile_n (grid decode)
flat / 256 → tile_idx (fill element → panel tile)
flat / 16 → row_in_tile (within-panel row)
flat & 255 → elem256 (within-tile position)
K/2 → v2f16 addressing stride for A
N/2 → v2f16 addressing stride for B
4096 → N (output dimension, row stride)
1024 → total elements in v2f16 smem arrays
```

All divisors are power-of-two → all should be shift+mask.
