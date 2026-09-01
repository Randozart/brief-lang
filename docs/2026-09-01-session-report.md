# Session Report — 2026-09-01: The GPU Breakthrough Day

**Headline: the .abv GPU lane now BEATS the ggml-cuda anchor at GEMV (0.205ms
vs 0.213ms, same box, our number on the smaller partition of the hardware),
runs small-M at 10× the previous cost, and compiles a NAIVE 15-line GEMM
foreach into a shared-memory tiled kernel at 5.25 TFLOP/s — 265× over the
naive lowering, exact.**

Everything below was measured on this box (RTX 3060 12GB via Vulkan, GTX
1070 Ti also present as the second CUDA device). Every refutation is
recorded alongside every win — the ledgers in `docs/plans/` are the
canonical trial record.

## 1. GEMV M1: 17.9ms → 0.205ms (87×), beats the anchor

Ladder (each row measured, interleaved, exact correctness):

| rung | min | verdict |
|---|---|---|
| session start | 0.93ms | serial era |
| vec4 inside the cooperative loop | 0.245ms | KEEP (acc=0-after-loop bug fixed on device) |
| vec4 projection layout | 0.259ms | KEEP for architecture (3 layout impls → 1, `projection_offsets`) |
| vector-Fma accumulator (OpPhi) | 0.259ms | KEEP (flat perf, −60% loop instructions) |
| **batched launches** | **0.205ms** | **KEEP — beats ggml-cuda's 0.213ms** |
| hybrid spin fence wait | 0.228ms sync-row | KEEP (sync floor 40µs → 23µs) |

Refutations (Rule 20 in action — measured, then blocked):
- **Per-lane ILP (2×/4× vec4 per lane)**: flat at M=4096 — 4096 warps hide
  latency; DRAM streaming is the wall. Machinery reverted.
- **Split-K**: killed by the dispatch-floor probe BEFORE building — a
  trivial 64-store kernel costs the same ~40µs per launch as the full GEMV;
  the warps had nothing left to win. Re-derived only if a latency-bound
  shape ever measures compute-bound.

## 2. The launch-tax discovery (the day's core insight)

A trivial 64-store kernel costs the SAME ~40µs per resident launch as the
M=64 GEMV — kernel time invisible under dispatch overhead. Decomposition:
submit ≈ 6.7µs + fence wake ≈ 33µs. Fixes shipped:
- `briev_accel_launch_resident_batch` (+ driver `launch_dev2d_batch`):
  K dispatches in ONE submission — per-call = kernel time. M=64: 0.039ms →
  **0.004ms (10×)**; the runtime ABI gained one op (zero-fill fallback).
- Hybrid spin wait (`vkGetFenceStatus` ~60µs window → blocking fallback):
  sync-path floor 40µs → **23µs**. The per-launch wait is LOAD-BEARING for
  single launches (single staging buffer; a no-wait flag deadlocks — probed).

## 3. M2 GEMM: naive source → tuned kernel (the metadata thesis, proven)

The author writes (complete .abv body):
```
let acc: Float = 0;
let m: Int = i / N;
let n: Int = i % N;
foreach k in 0..K { acc = acc + a[m * K + k] * b[k * N + n]; }
y[i] = acc;
```

| 4096³ fp32 | time | throughput |
|---|---|---|
| naive lowering (M2.0) | 6717ms | 20 GFLOP/s |
| **tiled synthesis (M2.1)** | **25.3ms** | **5250 GFLOP/s (~40% SIMT peak)** |

M2.0 (correctness first) fixed three real bugs on the way:
1. vec4 index-alignment hole — `b[k*N+n]` mis-read under vec4 loads; now
   proven via `expr_provably_mod4_zero` (conservative reject).
2. Cooperative over-fire on decomposed counters — 32× waste + wrong
   mapping; centralized as `is_cooperative_shape` (emission + runner can
   never drift again).
3. **Latent loop-anchor bug** in the O3 vec4-group runtime form — the
   non-cooperative GEMV had been silently wrong since O3 landed, masked by
   the cooperative default. Identity-probe fingerprint: y[0,n] = b[n] +
   b[n+4]. Fixed (+ loop_start/4 anchor).

M2.1 synthesized the tiled kernel: 64×64 tile, 16×16 invocations, 4×4
register tiles, A/B k-panels through Workgroup shared memory, two barriers
per panel, accumulator phis, u32 shared addressing. Four correctness
fingerprints caught by the identity-matrix probe before they shipped (all
documented in the M2 plan): tile-relative panel bases, tile-local shared
columns, TILE-scaled k offsets, phi-vs-backedge stores.

## 4. Everything tried and refused/deferred (so nobody re-litigates)

| idea | verdict | evidence |
|---|---|---|
| per-lane ILP at high occupancy | REFUTED | interleaved A/B flat; DRAM-bound |
| Split-K for small M | PRE-EMPTED | dispatch floor ≈ full kernel time |
| no-wait launch flag | UNSOUND | single staging buffer; deadlocks |
| O3 instruction reduction as perf lever | INSUFFICIENT | repeated: shape is DRAM/warp-bound at M1 |
| config knob flips without rebuild | NON-EXIT | `include_str!`-baked at build time |

## 5. State of the repo

- 2012 lib tests green at every commit; spirv-val clean; RT self-test;
  Praetor clean on changed functions (the complexity-36 cooperative fn
  became 8 named helpers; match_stmts became 6 focused helpers).
- Benchmarks banked: `gemv_m64/m256.abv`, `gemm.abv`, `gemm_small.abv`,
  `gemm_bench.c` (batch/tiled modes), `gemv_bench.c` (batch mode).
- Runtime ABI: `BrievField.proj_offset` (declared layout), `BrievDeviceDriver`
  +`launch_dev2d_batch` (last member, zero-fill fallback).

## 6. Open items (in order)

1. ✅ **ggml GEMM anchor measured** (`ggml_gemm_bench.c`, same box, Device 0
   = the 3060): **10.9ms avg / 12,600 GFLOP/s**. Our tiled kernel is at
   42% of ggml on identical silicon — and their 12.6 TFLOP/s on a ~13
   TFLOP SIMT card means tensor cores (TF32 mma), so the gap IS the M2.2
   prize.
2. Same-GPU pin for the GEMV row (`CUDA_VISIBLE_DEVICES=0` on the gemv
   anchor — the "20GB pair" is this box's 3060+1070Ti).
3. M2.2 — VK_KHR_cooperative_matrix (tensor cores), gated by the tile proof.
4. Tile-config search over the static shapes (8×8 registers, double buffering).
5. `.bv` resident-launch wrapper gate + `brievc run x.abv` (carried over).
