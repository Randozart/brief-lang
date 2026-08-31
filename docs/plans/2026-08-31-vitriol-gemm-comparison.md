# VITRIOL comparison — GEMM as the .abv GPU benchmark

**Date:** 2026-08-31
**Status:** spec — milestone work starts here
**Comparison target:** VITRIOL (`~/Desktop/Projects/VITRIOL`) — LLM inference
engine on THIS box: i7-3770 (AVX, no AVX2), RTX 3060 12GB, GTX 1070 Ti 8GB,
DDR3. Chimera CUDA+Vulkan backends, certified-numbers evidence culture.

## Why VITRIOL is the right target

1. **Same machine** — its certified numbers (BENCHMARKS.md) were measured on
   the exact GPUs the .abv stack runs on.
2. **The evidence culture** — certified full-workload runs, VERDICTS.md for
   negative results, fingerprint trails. This ledger adopts those rules.
3. **LLM inference IS GEMM.** `C = A×B` at llama.cpp shapes has a directly
   comparable number on this box (llama.cpp CUDA and Vulkan paths).
4. **Portability is a real .abv differentiator.** VITRIOL's build mandates
   CUDA archs `61;86` (two compile targets). One .abv/SPIR-V binary runs on
   both GPUs unchanged.
5. **The residency convergence.** VITRIOL's VERDICTS.md: "streaming a
   fitting model is a pessimization — resident execution beats every
   streamed configuration." Our device-residency work hit the same verdict
   independently. Same box, same lesson — documented as a shared result.

## Workload ladder (milestones)

| M | workload | kernel surface needed | reference |
|---|----------|----------------------|-----------|
| M0 | elementwise + RoPE-style passes | none (works today) | llama.cpp elementwise CUDA |
| M1 | **GEMV** `y = A·x` (M×K, one dot product per item) | in-body loop (`foreach` over range) + FMA | llama.cpp GEMV |
| M2 | **GEMM** `C = A×B` (N=M=K=4096) | M1 + tiling/shared memory (later) | llama.cpp GEMM CUDA + Vulkan |
| M3 | quantized row (Q4/Q8 dequant + dot) | M2 + integer/dequant ops | llama.cpp quant kernels |
| M4 | attention-probe scoring slice | M3 | VITRIOL LULL numbers |

## The language gap M1 needs (this is the driver)

Briev has `foreach` over ranges (SPEC §11.4 — the sole iteration keyword,
lowers to a counted `0..Count` loop). The kernel surface does not accept it
yet. GEMV in .abv would read:

```abv
async node gemv [i < M][i == M] {
    let acc: Float = 0;
    foreach k in 0..K {
        acc = acc + a[i * K + k] * x[k];
    }
    y[i] = acc;
}
```

## M1 status: LANDED (2026-08-31, same day)

- **Proof**: `stmt_is_kernel` accepts bounded `foreach k in start..end` —
  loop var is work-item-private (added to locals), bounds pure, body obeys
  the existing rules (affine writes, pure reads). Non-range collections
  stay host-side.
- **Lowering**: structured loop in the SPIR-V kernel (OpLoopMerge +
  OpPhi in the header + preheader/continue condition computation).
  Subtleties that cost real debugging time, recorded for posterity:
  the guard body must branch into the preheader (unterminated-block panic);
  OpPhi must OPEN the header (before OpLoopMerge); the condition must be
  recomputed in the continue block AFTER the increment (a stale test ran
  the body once past the end and faulted the GPU).
- **Capability gate**: foreach + slices_ranges enabled for the SPIR-V table.
- **Let-with-float-initializer fix**: `let acc: Float = 0;` materializes a
  float zero (storing an i64 zero into a float variable was an OpStore
  mismatch).
- **Verified on device**: identity-matrix GEMV (64×64) returns y[i] = x[i]
  exactly through the real dispatch path — dot-product accumulation,
  indexing, and the loop all correct on the RTX.
- Example: `examples/gpu/gemv.abv` (M=K=4096) compiles, spirv-val
  vulkan1.3 PASS.

## Next (M1 → M2)

- GEMV performance run (M=K=4096): GPU vs single-thread CPU vs llama.cpp
  GEMV — the first ledger number for M1.
- GEMM M2: multiple gemv-like launches vs a tiled kernel; shared-memory
  tiling is the M2 performance item.

Honest note: llama.cpp's GEMV/GEMM are years-tuned. M1/M2 acceptance is
**correctness first, then measured trajectory** — every milestone logs its
number in this ledger (VERDICTS.md rules: losses are recorded, not hidden).

## Evidence rules (adopted from VITRIOL)

- Every number is a full-workload run (whole kernel, warm), not a
  first-launch JIT figure; JIT warm-up runs are labeled
- GPU/CPU/losses all recorded; negative results get a VERDICTS entry
- Config fingerprint: GPU name, driver, kernel blob size, dispatch shape
- Same-box comparisons only

## Ledger

| date | milestone | GPU (3060) | CPU ref | verdict |
|------|-----------|-----------|---------|---------|
| 2026-08-31 | pairs elementwise 16M | 70ms (PCIe-bound single launch) | 36ms (in-cache) | workload memory-bound; residency required — see first-gpu-dispatch results |
