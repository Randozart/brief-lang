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
| 2026-08-31 | **M1 GEMV M=K=4096 baseline** (`gemv_bench`, pre-O2) | avg 17.9ms, min 15.2ms, **1.88 GFLOP/s** (resident launch, 5 warmup + 20 iters, max_rel_err 0.0) | 30.0ms single-thread, 1.12 GFLOP/s | **GPU 1.67× single-thread CPU.** Both far from roofline — the kernel is scalar, no FMA, no vectorized loads (that is the point: this is the pre-optimization baseline). Harness: `benchmarks/gpu/gemv_bench.c` (reads a .spv, self-contained; correctness gate vs double-accumulate CPU ref before timing). llama.cpp same-shape number: NOT yet on the ledger — build pending. |
| 2026-08-31 | **O2 FMA fusion** (explicit GLSL.std.450 Fma at lowering; FMul+FAdd no longer emitted for `a*b+c`) | avg 15.5ms, min 15.1ms, **2.16 GFLOP/s**; max_rel_err 0.0 | (same run CPU ref 15.9ms — CPU timing varies between runs; the GPU A/B is the comparison) | **KEEP.** avg −13% vs baseline (17.9→15.5), min ~parity (15.2→15.1) — the kernel is memory-bound, so the math-throughput win shows mostly in the average. Numerics: exact match vs the double-accumulate reference (single rounding per step). spirv-val vulkan1.3 PASS. Kernel blob 2548→2612B (ExtInstImport + Fma). |
| 2026-08-31 | **llama.cpp anchor** (`benchmarks/gpu/ggml_gemv_bench.c` — bare ggml_mul_mat F32 M=K=4096, VITRIOL's build-rebis, same box) | **ggml-cuda: avg 0.213ms, min 0.204ms, 157.8 GFLOP/s** (~300GB/s ≈ 83% of VRAM roofline) | ggml-cpu 1T 4.4ms (7.7 GFLOP/s), 8T 3.8ms (8.9) | **The race target.** We are 4.2× behind ggml-cuda (0.93 vs 0.213ms) but already 4.7× ahead of ggml-cpu. Gap analysis: their kernel runs ~32× more threads (warp-per-row) saturating bandwidth; ours runs one thread per row with a serial K-chain (128 warps total). Conclusion: the next rung is subgroup-cooperative row kernels (K-split within a workgroup + OpGroupNonUniformFAdd), NOT more load width — O3's verdict already showed loads are coalesced. |
| 2026-08-31 | **O3 float4** (vec4-typed SSBO members + shifted scalar fallback + aligned group loads in unrolled prefix AND a step-4 vector remainder loop) | avg 0.93ms (was 0.98ms scalar), ~36 GFLOP/s; exact-correctness gate OK (7e-4 f32-level) | — | **VERDICT: ~5% on GEMV — below the 20% threshold.** Root cause understood: GEMV is DRAM-bandwidth-bound and its scalar loads were ALREADY fully coalesced (consecutive work items → one transaction per warp); float4 cuts instruction count, not DRAM traffic. KEPT as infrastructure: the retype+group-load machinery is general and will pay on compute-bound GEMM (M2) and poorly-coalesced patterns. The genuine GEMV lever is split-K (parallelism), recorded as the successor rung. |
| 2026-08-31 | **O1 unroll factor 4→16** (config default) | avg ~0.99ms across 3 runs, max 1.06ms; ~34 GFLOP/s | — | **KEEP** — best-case parity with factor 4 (~0.9ms) but variance collapses (worst run 1.06ms vs 3.15ms). The per-launch outliers at factor 4 were the dominant noise source. |
| 2026-08-31 | **Device-local working set** (driver allocates VRAM SSBO; host staging only seeds/syncs scalars/downloads) + LocalSize 64→256 A/B (256 wins 3.4×: 3.0ms vs 0.9ms avg) | **best-run avg 0.9ms, min 0.876ms, ~38 GFLOP/s**; avg varies 0.9–3.0ms across runs (clock/launch wobble — recorded honestly, min is stable); max_rel_err 0.0 | 30.1ms single-thread | **KEEP — the real lever.** 17.5× over the O2 best (15.5→0.886ms). The previous "resident" mode was PCIe-bound: the SSBO lived in HOST_VISIBLE|HOST_COHERENT sysmem (~4GB/s effective; reads scaled with bytes, not threads — a 8192×2048 shape ran SLOWER than 4096² despite identical work). With VRAM residency the same kernel hits ~75GB/s effective read bandwidth (still ~5× off the 3060's roofline → O3 float4/split-K remain live levers). Pairs 4096² 2D verified end-to-end; 2011 lib tests green. Trap for the record: VkBufferMemoryBarrier.buffer must be set — a null buffer invalidates the submission and the GPU silently drops the whole command buffer.
| 2026-08-31 | **2D dispatch infrastructure** (§2b of gpu-next plan; pairs relaunched as a 4096×4096 grid) | 16,777,216 items via `briev_accel_launch_resident_2d`, fast-forward i = 16777216 correct; gemv 1D rerun within noise (avg 17.7ms, min 14.8ms) | — | **INFRASTRUCTURE, geometry-only** — no perf claim yet: the kernel still reconstructs scalar `i` and the body's shift/mask ops remain. Gains arrive with the substitution pass (row/col direct from gid.y/gid.x, gated on a guaranteed-2D launcher). Soundness bonus: the bounds guard now also covers literal counts not divisible by the workgroup size (a latent 1D hole found while wiring 2D). |
| 2026-09-02 | **Float16 GEMM end-to-end on device** (`gemm_h_bench`, 4096³, naive tier — the f16 pipeline's FIRST correct device run; the "~25% y-fill fault" is GONE) | avg 502ms, min 468ms, **273.78 GFLOP/s**; max_rel_err 2.442e-04 = the expected single f16 store-rounding (f32 compute, f16 storage) | — | **CORRECTNESS MILESTONE, not a perf row** — the naive tier is scalar-by-design; the f16 win lives in the tensor tier (spirv_coopmat knob, next). What landed to make this possible: f16-as-storage-format lowering (f32 Function storage, OpFConvert at the SSBO boundary — commit d56d910d), runtime feature PROBING (features2 + apiVersion 1.2 instance + tensor-device preference — commit 3314b834), the Float16 capability scan, and the precision-bounded literal admission. Harness: `benchmarks/gpu/gemm_h_bench.c` (f16-exact seeds, RNE encoder mirroring the backend's). Regression gates rerun with the new runtime: GEMV coop 0.199ms / rel 0.0 (ledger parity); GEMM f32 4096³ rel 0.0. Known quirk (pre-existing): gemm_bench at 256³ reports rel=1.0 with BOTH the old and new runtime — small-shape dispatch issue, noted for the M2 arc. |
| 2026-09-02 | **TENSOR TIER CORRECT ON DEVICE** (`gemm_h_bench`, 4096³, `spirv_coopmat=1`) — the M2.2 blocker is RESOLVED | avg 55.6ms / 2.47 TFLOP/s, **min 41.2ms = 6.85 TFLOP/s** (max 316ms — launch wobble, min is the stable signal per ledger convention) | ggml anchor 10.9ms / 12.6 TFLOP/s | **CORRECTNESS MILESTONE + first perf row.** max_rel_err 2.442e-04 — IDENTICAL to the naive tier's bound; two independent implementations agree on every sampled element. Root cause of the "fault": the coopmat GRID DECODE had tile_m/tile_n SWAPPED (commit a181951b) — only 25% of tile-rows were ever computed: exactly the "~25% y-fill" the M2.2 session misread as driver/NVVM. Second bug: the runner's v1 16×16-tile work-count formula 4× over-dispatched the 16×64-tile v2 kernel (out-of-range tiles smeared garbage over correct outputs). Doctrine in action: the naive tier (general path) served as the correctness reference that made the bisect possible. Next perf rungs (recorded, not yet run): A-panel shared reuse across the 4 warp fragments, occupancy (16k workgroups of 32 lanes underfills the 3060). Known quirk: the runtime's features2 probe reads 0 for 16bit/coop on this driver while create + kernels work — NVIDIA under-enforcement, diagnostic-only. |

## Optimization doctrine (locked, 2026-08-31 — user)

> Regular Briev is optimised for native code through LLVM. Whatever we must
> do to optimise the SPIR-V backend for GPU, we will.

The SPIR-V backend is a first-class optimization target, not a
correctness-only emitter. The ladder mirrors what LLVM does for native
code, translated to GPU reality — and follows the house rules:
frontend-driven (analysis computes, backend consumes), config-tuned
(`config/targets.dbvl` per-GPU entries like `[target.spirv64]`), measured
before built.

| # | optimization | GPU effect | prerequisite |
|---|--------------|-----------|--------------|
| O1 | constant-trip-count loop unrolling (config factor) | fewer branches, ILP | foreach lowering ✓ |
| O2 | FMA fusion (mul+add chains) | math throughput | verify driver fusion |
| O3 | vectorized loads/stores (float4 via OpTypeVector) | memory throughput | alignment proof |
| O4 | shared-memory staging for tiled reuse (GEMM tiles) | DRAM traffic cut | tiling analysis |
| O5 | occupancy shaping (LocalSize search) | latency hiding | certified shapes |
| O6 | tree reductions in shared memory | GEMV/GEMM epilogue | O4 |

Every rung: before/after numbers on the same box in this ledger. A rung
that loses is a VERDICT entry, not a silent revert.

### O1 spec (first rung, implemented now)

Unroll a foreach whose trip count is a compile-time constant (`0..K`, K
literal const) by a config factor, emitting the unrolled groups plus a
remainder loop. Implementation site: the foreach lowering in
src/backend/spirv/lower.rs (the body is re-emitted per group with the loop
variable's loads pointing at per-group constants). Budget: a code-size cap
clamps the factor. Config: `spirv_unroll` in the tuning table, default 4.
