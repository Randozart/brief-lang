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
| + O3 float4 + cooperative (current HEAD, knob off) | 0.93ms | ~36 |
| **ggml-cuda (the bar)** | **0.213ms** | **157.8** (~83% of VRAM roofline) |
| ggml-cpu 1T | 4.4ms | 7.7 |

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

1. **Vec4 inside the cooperative strided loop** (the perf rung): the
   lane's element set is `lane*4 + t*128 .. +3` — 4 consecutive floats per
   lane per iteration = one vec4 load. Needs the group-load matcher to
   accept the cooperative form (lane-call breaks the current alignment
   proof — restructure as `base = row*K/128*...` + lane*4 so all literals
   divide). Bar: within 2× of 0.213ms; then flip
   `spirv_row_cooperative: true` and make it the default.
2. **Split-K for small M** (after vec4): partition K across workgroups +
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
