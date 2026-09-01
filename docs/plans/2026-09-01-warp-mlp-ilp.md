# Plan: Warp memory-level parallelism — multi-vec4 ILP + loop-var phi

**2026-09-01.** Successor to `2026-09-01-vec4-projection-layout.md` (flat
verdict: the M1 shape is DRAM/warp-MLP-bound, not instruction-bound).

## Objective

M1 GEMV min 0.259ms vs ggml-cuda 0.213ms (~1.15×, 261 vs 300 GB/s effective
= 73% vs 83% of the 360 GB/s VRAM roofline). Hypothesis: each lane issues
ONE a[] load + ONE x[] load per iteration with no other independent work in
flight — memory-level parallelism per warp is 1. ggml's kernel keeps several
float4 loads in flight. Secondary: the hand-built loop carries its induction
variable through Function storage (OpLoad+OpStore per iteration in the
continue block).

## Baseline (current HEAD d9a97a07, RTX 3060, M=K=4096, WARMUP=5, interleaved)

| variant | min (ms) | avg (ms) | GFLOP/s | rel err |
|---|---|---|---|---|
| cooperative vec4 (layout leg) | 0.245 | 0.281 | ~120 | 0.000e+00 |
| + vec4 layout + vector Fma (current) | 0.259 | 0.290 | ~115-128 | 0.000e+00 |
| ggml-cuda (the bar) | 0.213 | — | 157.8 | — |
| ggml-cpu 1T | 4.4 | — | 7.7 | — |

Full CPU suite untouched by this leg (SPIR-V kernel emission + runtime only);
`compare_baseline.sh` at the end if anything surprising appears.

## Phases (each gated by measurement — Rule 20: a refuted hypothesis blocks)

- **P1 — ILP=2**: stride 256 (8 elems × 32 lanes), `repl = lane*8 + t*256`;
  per iteration two independent (a[], x[]) vec4-load pairs and
  `acc = Fma(a1, x1, Fma(a0, x0, acc))` through the same OpPhi. Vec4 bases
  generalize to `(idx subst loop_var→repl + j*4) >> 2`. Gate: rel 0.000e+00,
  spirv-val, interleaved min/avg ×4 vs HEAD binary. Keep only if min drops.
- **P2 — ILP=4** (if P1 wins): same shape, stride 512, four pairs in flight.
  Pick the better of {1, 2, 4} as the DEFAULT selection rule — encode as
  "largest ILP in {4,2,1} with `inner_len % (128*ILP) == 0`" if measured
  monotone, else the measured winner's rule. A keyword is forbidden
  (MAXIMUM EFFICIENT DEFAULT).
- **P3 — loop-var phi** (if instruction issue still shows): induction
  variable becomes a header OpPhi (value vars, not Function storage);
  continue block computes only next+condition. Independent of ILP.

## Design notes

- ILP lives in the vector-Fma cooperative form only (the matched
  dot-product shape); the unrolled per-component fallback stays ILP=1.
- The scalar-side handling is symmetric — x[] pairs with a[] per j.
- Loop bound: `groups = inner_len / (128*ILP)`; shape gate requires
  `inner_len % 32 == 0` already — ILP>1 additionally requires
  `inner_len % (128*ILP) == 0`, else fall back to smaller ILP.
- Generalization: any both-sides-vec4 dot product benefits; no Briev-type
  knowledge anywhere (fields stay opaque names).

## Tests

- Existing spirv kernel tests green; new: ILP=2 emission test (stride 256,
  two AccessChains per field per iteration, phi order) if P1 keeps.
- rel gate on device after every phase; runner standalone.

## Docs

- Handoff ladder row + verdict per phase; trap list if phi machinery adds
  one; plan verdict appended here.

## Verdict (2026-09-01, same session — hypotheses MEASURED, both REFUTED at M1)

Interleaved A/B ×4-5 (quiet runs; the box shows 2-3ms interference outliers
on BOTH binaries — min-of-many is the metric):

| variant | min (ms) | rel err |
|---|---|---|
| HEAD (layout leg) | 0.243 - 0.257 | 0.000e+00 |
| ILP=2 | 0.240 - 0.247 | 0.000e+00 |
| ILP=2 + loop-var phi | 0.242 - 0.253 | 0.000e+00 |
| phi only (final) | 0.244 - 0.268 | 0.000e+00 |

- **Warp-MLP hypothesis (P1/P2): REFUTED at M1 occupancy.** With M=4096
  rows the dispatch is 4096 independent 32-lane warps; latency is already
  hidden by warp parallelism, and DRAM streaming (~73-77% of practical
  roofline) is the wall. Per-lane ILP adds nothing where warps abound.
  The ILP machinery was REVERTED (Rule 20: a refuted hypothesis blocks the
  fix). Do not re-add per-lane ILP without a measurement at a LATENCY-BOUND
  shape (small M — the Split-K rung's domain, where it must be re-derived
  on that rung's own analysis).
- **Loop-overhead hypothesis (P3): REFUTED as a perf lever, KEPT as the
  better shape.** The induction variable is now a header OpPhi read
  through `lower.value_vars` (SSA value bindings resolved without a
  storage load); the continue block computes next+condition directly into
  the pre-reserved back-edge id. Equal time, strictly fewer instructions,
  no Function-storage round-trips — and the machinery (pre-reserved
  back-edge ids + `glsl_fma_with_id`) is what the vector accumulator
  already needed.

Remaining gap to ggml (~1.15×) is NOT addressable by this kernel's shape at
M=4096. Next candidates, in order: Split-K / small-M shapes (new benchmark
program — the current .abv bakes M=K=4096), a[] access granularity
(aligned 128B per warp-load is already optimal), or accepting parity-band
(~87% of ggml's rate at min).
