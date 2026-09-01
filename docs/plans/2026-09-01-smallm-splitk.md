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

## Verdict (2026-09-01, same session — P0/P1 executed; Split-K pre-empted)

**P0 baseline** (exact correctness, quiet-run mins): M=64 sync 0.036-0.044ms
(~12 GFLOP/s); M=256 sync 0.047-0.061ms; M=4096 sync 0.242-0.255ms.

**The dispatch-floor experiment (Rule 20) pre-empted Split-K.** A trivial
64-store kernel costs the SAME ~40µs per resident launch as the M=64 GEMV —
the kernel's compute is invisible under the launch tax. Decomposition
(probe, `BRIEV_ACCEL_NO_WAIT`): submit ≈ 6.7µs + fence-wake ≈ 33µs. The
first no-wait probe attempt also proved the per-launch wait is LOAD-BEARING
(single staging buffer reuse + one-fence-one-submission) — pipelining needs
batching, not flag-flipping. Split-K's extra warps would attack the
invisible µs; refuted before building.

**P1' — launch batching** (the real fix): `launch_dev2d_batch` driver op —
`times` identical dispatches recorded in ONE command buffer, ONE submit,
ONE fence wait; `briev_accel_launch_resident_batch` in the RT (scalars sync
once per batch; contract: launch-invariant host scalar state — true for the
cooperative i-reset loop and the flat i=0 push). Sequential fallback for
drivers without the op. Measured (per-call = wall/ITERS, exact correctness):

| shape | per-call sync | **per-call batched** | ggml-cuda (their GPU) |
|---|---|---|---|
| M=64, K=4096 | 0.039-0.044ms | **0.004ms — 10×** | 0.016ms avg |
| M=256, K=4096 | 0.047-0.061ms | (not the target; scales as M=64) | 2.29ms (their mul_mat path) |
| M=4096 (M1) | 0.242-0.255ms | **0.205-0.206ms — 163 GFLOP/s** | 0.213ms avg |

- **M1 BEATS the ggml-cuda anchor number** — on our RTX 3060 against their
  20GB CUDA device (cross-device caveat stands, the direction is right):
  kernel-only was already faster; the 40µs launch tax was the benchmark
  loss. Batched per-call is now the deployment-loop row in the ledger.
- **M=64 batched runs at the SAME 123 GFLOP/s as M=4096** — the
  "latency-bound regime" collapses entirely once the launch tax is gone;
  there is nothing left for Split-K to win at these shapes. The small-M
  benchmark programs (`gemv_m64.abv`, `gemv_m256.abv`) are banked with
  their baselines.

Infra notes: driver `BrievDeviceDriver` ABI +1 op (`launch_dev2d_batch`,
LAST member — zero-fill keeps OpenCL's table valid); the batched path
shares `launch_core` with the sync path (`times` dispatch loop inside one
begin/end). Bench 5th arg `batch=1`. Praetor clean, 2012 tests, RT
self-test passed, probe scaffolding removed.

## P2 — hybrid spin fence wait (2026-09-01, same session)

The ~33µs fence wake is now attacked directly: `vkGetFenceStatus` spin
(~60µs window) → blocking `vkWaitForFences` fallback
(`BRIEV_ACCEL_BLOCKING_WAIT=1` restores pure blocking for A/B). Measured
per-call sync, quiet runs, interleaved:

| shape | blocking | **spin hybrid** |
|---|---|---|
| M=64 | 0.052-0.089ms | **0.022-0.025ms** |
| M=4096 | 0.248-0.294ms | **0.227-0.230ms** |

Per-call sync M=64 is now 23µs vs ggml-cuda's 16µs avg (their GPU) — the
sync row is no longer embarrassing; the batch row (0.004ms, now 149
GFLOP/s — the batch's single wait also spins) remains the loop-deployment
number. CPU burn is bounded by the spin window (~60µs) before the blocking
fallback takes over for long kernels.
