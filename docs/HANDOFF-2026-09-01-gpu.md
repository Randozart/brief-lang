# HANDOFF — GPU backend (.abv / SPIR-V): top-level goals, state, next steps

**Date:** 2026-09-01 (end of session)
**Read together with:** `docs/plans/2026-08-31-gpu-next.md` (master work
order), `docs/plans/2026-08-31-vitriol-gemm-comparison.md` (the ledger —
every number and verdict lives there), `docs/plans/2026-08-31-o3-float4-loads.md`
(O3 outcome), `docs/plans/2026-09-01-cooperative-row-kernels.md` (the open
work package), and `docs/HANDOFF-2026-08-31-gpu.md` (previous handoff,
doctrine sections still accurate).

---

## 1. Top-level goals (the why, stable across sessions)

1. **`.abv` is the pure GPU language.** Every eligible counted loop is a
   kernel; compile emits kernels + a runnable program. No CPU fallback in
   the artifact. `.bv` remains the CPU language with annotated,
   probe-verified GPU offload.
2. **The SPIR-V backend gets the same optimization commitment as LLVM.**
   Frontend-driven (analysis computes, backend consumes), config-tuned,
   measured before built. A losing optimization is a VERDICT entry in the
   ledger, not a silent revert.
3. **The race target is llama.cpp-class GEMM/GEMV on this box** (VITRIOL's
   certified-numbers culture, same machine). Milestone ladder M0 elementwise
   → M1 GEMV → M2 GEMM → M3 quantized → M4 attention slice. Correctness
   first, then measured trajectory; losses are recorded, never hidden.
4. **Max Efficient Default applies to the GPU lane**: a user should never
   need a keyword to reach competitive performance. The analysis recognizes
   shapes (elementwise, dot-product reduction, …); the backend chooses
   emission. Knobs exist for intent, never for speed.

## 2. Where we are (measured, RTX 3060, this box)

| GEMV M=K=4096 | time | GFLOP/s |
|---|---|---|
| M1 baseline (session start) | 17.9ms | 1.9 |
| + O2 FMA fusion | 15.5ms | 2.2 |
| + device-local working set + LocalSize 256 | **0.89ms best-run** | **~38** |
| + O3 float4 + cooperative scalar | 0.93ms | ~36 |
| + vec4 inside the cooperative loop | 0.25ms min / 0.28ms avg | ~120 |
| + vec4 projection layout + vector-Fma accumulator | 0.26ms min / 0.29ms avg | ~115-128 |
| + hybrid spin fence wait (per-call sync row) | 0.228ms | 147 |
| + **batched launches** (current HEAD, per-call = wall/ITERS) | **0.205ms** | **163 — BEATS the anchor** |
| ggml-cuda GEMV (SAME BOX — the "20GB pair" is this box's 3060+1070Ti; pin to the 3060 for the final same-GPU row) | 0.213ms | 157.8 (~83% of VRAM roofline) |
| ggml-cpu 1T | 4.4ms | 7.7 |

### GEMM ladder (4096³ fp32, plan 2026-09-01-m2-gemm)

| 4096³ GEMM | time | GFLOP/s |
|---|---|---|
| naive flat lowering (M2.0, correct) | 6717ms | 20 |
| **+ tiled synthesis (M2.1, current HEAD)** | **25.3ms min / 26.2ms batched** | **5250 (~40% of SIMT peak)** |
| RTX 3060 SIMT fp32 peak | — | ~13000 |
| **ggml-cuda GEMM anchor (SAME GPU — Device 0 = the 3060, measured)** | **10.9ms avg / 10.6ms min** | **12600** |

The GEMM gap to ggml is 2.4× on identical silicon — and ggml's 12.6 TFLOP/s
on a ~13 TFLOP/s SIMT card means they are on the TENSOR-CORE path (TF32
mma). The gap quantifies M2.2 (VK_KHR_cooperative_matrix) precisely: that
rung is worth up to ~2.4× and is the difference between "derived kernel,
respectable" and "derived kernel, cuBLAS-class".

Verdicts on record (full trail in the ledger): O2 KEEP (−13%, exact
numerics), O3 VERDICT ~5% (loads were already coalesced — instruction
count, not DRAM traffic), device-local residency KEEP (**17.5× — the big
one**), LocalSize 256 KEEP (3.4× over 64), O1 unroll 16 KEEP (stability).

## 3. What exists and works (verified on device)

- `.abv` → kernels + self-contained C runner; `spirv-val vulkan1.3` clean;
  subgroup ops (`SubgroupFAdd#`) verified on device.
- **Device-local working set** (the big lever): DEVICE_LOCAL buffer is the
  shader's working set; host-visible buffer is only seed/scalar-sync
  staging. Seed + scalar counters cross inside the dispatch submission
  (vkCmdCopyBuffer + transfer→compute barriers); download pulls VRAM →
  staging. All-host fallback preserved. Root doctrine lesson: *device
  residency means VRAM, not mapped host memory* (mapped-host cost us 17×).
- **Flattened cooperative grids**: the Y dimension of vkCmdDispatch is
  inert on this driver (probe-proven: gid.y ≡ 0 under (1,64) dispatch).
  ALL grids are flattened into X. Cooperative row kernels derive
  `row = gid.x >> 5`, `lane = gid.x & 31` (LocalSize 32).
- **Cooperative row kernels are implemented but GATED OFF**
  (`spirv_row_cooperative: false` in the ir-lowering tuning table):
  recognition of foreach dot-product reductions, strided lane accumulation,
  `SubgroupFAdd#` → `OpGroupNonUniformFAdd` (bit-exact tree, no atomics),
  lane-store, 2D→flattened dispatch. The minimal subgroup probe verifies
  exact on device; the full GEMV integration verified correct at **parity**
  (~0.9ms) — not yet the hoped 4×.
- O1 unroll (16, config), O2 explicit GLSL Fma (`a*b+c` → one rounding),
  O3 vec4-typed SSBO members + shifted scalar fallback + vector remainder
  loop (all behind the analysis, additive).
- 2D dispatch geometry (`work_cols` from the body's own shift/mask),
  bounds guard required for cooperative shapes and non-multiple literal
  counts (a latent tail-write hole closed), multi-const fast-forward
  regression-pinned, standalone .spv entry points always "main".

## 4. The gap, decomposed (what the numbers say)

We are 4.2× behind ggml-cuda at GEMV. NOT load width (O3 verdict), NOT
DRAM bandwidth ceiling (they hit 83% of roofline at the same size), NOT
occupancy from thread count alone (cooperative gave 32× threads, parity).
The remaining gap is **memory-level parallelism per warp**: ggml's kernel
has each lane loading vec4 (16B) with multiple loads in flight — 512B
coalesced per warp-load vs our 128B — plus tuned ILP. The successor rung
is therefore: **vec4 loads inside the cooperative strided loop** (lane
handles 4 consecutive floats per iteration), which the O3 machinery
(member retype + group-load matcher) was built to support.

## 5. Next steps, in order

1. **DONE (this session)**: vec4 inside the cooperative strided loop —
   0.93ms → 0.25ms. Bar (within 2×) met and beaten; knob flipped ON by
   default; the vec4 path is picked automatically when fields qualify.
   Next squeeze: x[] vec4-eligibility (16B-align x in the state projection
   so BOTH sides load vec4; x at offset 8 mod 16 stays scalar today).
2. **Split-K for small M**: partition K across workgroups +
   reduction kernel; needs a partials surface in `.abv` (language gap —
   design before building).
3. **`.bv` resident-launch wrapper gate** (handoff 08-31 §5.4): the
   all-readers-are-kernels analysis gate; UNSOUND without it — the
   runtime ABI exists, the gate is the work.
4. **`brievc run x.abv`** subcommand (small).
5. **Re-benchmark the full ladder into the ledger** after each rung;
   llama.cpp numbers now exist (ggml-cuda 0.213ms / ggml-cpu 4.4ms rows)
   — keep them current as shapes change.

## 6. Traps that cost real time (all fixed — do not regress)

1. **vkCmdDispatch Y dimension is inert on this driver.** Probe-proven.
   All grids flatten into X; never trust a multi-workgroup Y dispatch.
2. **VkBufferMemoryBarrier.buffer must be set.** A null barrier invalidates
   the submission and the GPU silently drops the whole command buffer
   (symptom: outputs stay zero, submit "succeeds").
3. **Stale runtime copies**: the runner #includes runtime files COPIED
   beside it; after editing lib/runtime, recompile against the fresh copy
   (or `-I lib/runtime`) — stale-copy symptoms look like driver bugs.
4. **Standalone .spv entry point must be "main"** (driver hardcodes
   pName); pinned by test.
5. **Bounds guard**: required for cooperative shapes (tail reaches
   lanes-1) and literal counts not divisible by the workgroup size.
6. **Cooperative blobs and 1D dispatch are incompatible** — the runner
   must dispatch the geometry the blob was built for; the knob
   (`spirv_row_cooperative`) gates blob emission and dispatch together.
7. Per-launch verbose prints inside the hot loop poison the timing
   (3ms vs 0.9ms runs). Keep the hot path print-free.
8. spirv-dis + a value probe beats theory: every session-level "why is it
   slow/wrong" question was answered by measurement, never by reading.
9. **First resident launch always falls back to full-copy** — the staging
   buffer is lazily allocated by that first full-copy launch. The fallback
   divides work_n by the module local size now (parsed), so coverage is
   correct for any local size; but a single-launch program still runs the
   full-copy path (correct, PCIe-bound).
10. **Download must pull VRAM→staging** (`briev_accel_download` →
   `download_dev`) — with the device-local working set, results land in
   VRAM and the host-visible staging window is STALE until pulled. Symptom
   before the fix: rows written by the fallback launch visible, rows written
   by resident launches zero.
11. **Dispatch must divide by the module's LocalSize**, not a global
   constant — cooperative kernels declare 32, flat kernels 256. Both drivers
   parse OpExecutionMode LocalSize (opcode 16, mode 17) at create_kernel.
12. **Bench harness geometry**: `gemv_bench <spv> M K 1` for cooperative
   blobs — (32, M) → one 32-lane workgroup per row. (M, 1) dispatches
   ceil(M/32) workgroups → only M/32 rows, by design.

## 7. Session-start checklist

```bash
cargo test --lib && cargo test --bin brievc   # both green at handoff
env BOUND=1 ./target/release/brievc build examples/gpu/gemv.abv \
    --out /tmp/gemv_bench_out --optimize-budget 2048
spirv-val --target-env vulkan1.3 /tmp/gemv_bench_out/gemv.spv
cc -O2 -I lib/runtime benchmarks/gpu/gemv_bench.c -o /tmp/gemv_bench -lm -ldl
env BRIEV_ACCEL_DEVICE=vulkan /tmp/gemv_bench /tmp/gemv_bench_out/gemv.spv 4096 4096
# expect: max_rel_err ~7e-4 OK (1e-3 tolerance), ~0.9ms, ~36 GFLOP/s
env BRIEV_ACCEL_DEVICE=vulkan timeout 300 /tmp/pairs2d/_p   # i = 16777216
```

Pre-existing diagnostics baseline (not yours to fix opportunistically):
clangd noise on the single-TU runtime/driver files (`g_verbose`,
`BrievDeviceDriver` unknown — single-TU include pattern); `emit_runner` /
`emit_scalar_read` / `prove_kernel` complexity at or near the pre-session
baseline; `tests/ffi_*` crates reference the old `briv_compiler` name.
