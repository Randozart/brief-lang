# Plan: Small-M GEMV — Split-K via two-node .abv (latency-bound regime)

**2026-09-01.** Successor to `2026-09-01-warp-mlp-ilp.md` (per-lane ILP
refuted at high occupancy — this rung targets the LOW-occupancy regime).

## Everything tried so far (the ledger — one row per rung, all on device)

| rung | M1 time (min) | vs ggml-cuda 0.213ms | verdict |
|---|---|---|---|
| baseline (LLVM-era serial) | 17.9ms | 84× | start |
| O2 FMA fusion | 15.5ms | 73× | KEEP (−13%, exact) |
| device-local working set | 0.89ms | 4.2× | KEEP (17.5× — the big one) |
| LocalSize 256 + O3 vec4 members | 0.93ms | 4.4× | KEEP (O3 alone ~5%: instruction-bound) |
| cooperative rows, scalar | 0.437ms | 2.1× | KEEP (2× over serial) |
| + vec4 in the loop | 0.245ms | 1.15× | KEEP (1.75×; acc=0-after-loop bug fixed) |
| + vec4 projection layout (x aligned) | 0.259ms | 1.22× | KEEP (flat perf; 3 layout impls → 1, drift killed) |
| + vector-Fma accumulator (OpPhi) | 0.259ms | 1.22× | KEEP (flat; −60% loop instructions) |
| per-lane ILP=2 / ILP=4 | 0.240ms | 1.13× | **REVERTED** — refuted: 4096 warps hide latency; DRAM is the wall |
| loop-var phi | 0.244ms | 1.15× | KEEP (flat perf, strictly cleaner SSA) |

ggml-cuda anchor: M=K=4096 → 0.213ms avg (their CUDA device, 20GB — cross-
device caveat on file). ggml-cpu 1T: 4.4ms. **M1 is at ~87% of ggml's rate;
the M=4096 shape is closed** — remaining gap is DRAM efficiency, not kernel
structure. New shapes, new regime.

## The small-M regime (why this rung exists)

ggml anchor at M=64, K=4096: **cuda avg 0.016ms / min 0.008ms** (33.8
GFLOP/s — latency-bound: 1MB traffic in 8µs = 131 GB/s, nobody is
bandwidth-limited here), cpu-1T 0.059ms. At M=64 our cooperative kernel
dispatches 64 warps on a 28-SM GPU — ~2 warps/SM, deeply latency-bound.
Both known levers apply: more warps (Split-K) and per-lane ILP (refuted at
high occupancy, unproven here — measure, don't assume).

## Findings already banked (this session, probe-verified)

- **Multi-node `.abv` works today.** `two_node_probe.abv`: two async nodes
  → two SPIR-V kernels (N_KERNELS=2), runner phase machine fires them in
  DECLARATION order, each node fast-forwarded into ONE synchronous dispatch
  before the next node's check. Kernel B sees all of kernel A's writes.
  The handoff's "partials surface language gap" is smaller than feared —
  a partials array is an ordinary state field, Split-K needs NO language
  change, only frontend routing.
- Unknown: whether the accel analysis accepts a SLICE dot product
  (`foreach k in s*K/S .. (s+1)*K/S` — non-literal bounds, affine in the
  item id). The cooperative gate hard-errors on non-literal inner length;
  the flat path may accept it. This is experiment E2.

## Phases

- **P0 — small-M benchmark set + baseline**: `gemv_m64.abv` (M=64, K=4096),
  `gemv_m256.abv`; measure the CURRENT cooperative kernel at those shapes
  (bench harness with matching blobs). Also re-anchor ggml numbers at both
  shapes. This is the plan's required baseline table.
- **P1 — hand-peeled Split-K experiment** (Rule 20): write the two-node
  Split-K .abv BY HAND (partials field `Float[M*S]`, partial node computes
  slice dots per item, reduce node sums them). Compile → fix whatever the
  frontend rejects (expect: slice-dot eligibility, non-literal foreach
  bounds) → measure at M=64/M=256 vs P0. If it does not beat the P0
  baseline, STOP and record the refutation.
- **P2 — frontend routing (only if P1 wins)**: the accel analysis emits the
  Split-K pair automatically when the shape warrants (work-item count M
  below a config threshold in `config/targets.toml` — target tunables are
  the sanctioned home; no keyword). The reduce node is synthesized by the
  compiler; the user writes ONE node.
- **P3 — per-lane ILP re-measure at M=64** (the refuted-at-4096 lever gets
  its latency-bound hearing, on Split-K's own terms).

## Gates

Every phase: max_rel_err = 0.000e+00 vs double reference, spirv-val clean,
2012 lib tests, runner standalone. Perf phases: interleaved ×4 vs the
prior binary, min-of-many (the box throws 2-3ms interference outliers).

## Docs

Handoff ladder rows per phase; the ledger above is the canonical
"everything we tried" record — update it, never rewrite it.
