# Beyond-vendor-lowering GPU campaign

**2026-09-04.** Realizes the doctrine in
`docs/architecture/abv-gpu-doctrine.md`: one `.abv` program, peak on
every probed device, Briev-owned codegen only (no nvcc, no cuBLAS in
the compiler path). Locked by the user ("the answer is always a").

## Baseline (recorded 2026-09-04, ledger row 2026-09-04b)

| cell | number |
|------|--------|
| 4096³ smem-R4 (default tier) | 8.3ms best window = 16.5 TFLOP/s |
| 4096³ direct loads | 8.6ms best window ≈ parity |
| 2048³ smem-R4 | 0.905ms = 19.0 TFLOP/s |
| 4096²×512 smem-R4 | 1.504ms = 11.4 TFLOP/s |
| 8192³ smem-R4 | ~84.8ms = 13.0 TFLOP/s (+23% vs direct) |
| R sweep | R2 0.637, **R4 = 1.00**, R8 0.628 |
| **Race target** | ggml-cuda 3.205ms = **42.0 TFLOP/s** (83% of 50.6 peak) |
| Key hypothesis | f16acc-vs-f32acc delta was only ~11% → the KHR lowering likely does NOT double-pump → portable ceiling ≈ 25.3 TFLOP/s |

Measurement discipline: in-process A/B only (alternating-order batched
submissions; self-A/B ratio 0.985–1.001). Unlocked clocks — ratios
within one run are the signal, absolute best-window numbers second.

## Stage 0 — Ceiling truth (1 session, decisive)

Emit a register-resident mma-chain microkernel through the existing
`SpirvBuilder`: depth-N `OpCooperativeMatrixMulAddKHR` chain on
constant-initialized fragments, no smem, no DRAM access in the loop.
Variants: f16acc + f32acc. Host side: y-only variant of
`gemm_h_bench` (state = accumulator scalars; correctness = output
count/finite check).

Measure: mma throughput ceiling through the vendor lowering, per
precision. Secondary: fill+barrier cost share of the current smem
pipeline (A/B the fill against an empty-fill variant).

**Decision gate:**
- Ceiling ≥ ~40 TFLOP/s → the portable road can win; Stage 2 becomes
  optional polish.
- Ceiling ≈ 25 TFLOP/s (no double-pump) → the portable path is
  structurally capped on this driver; Stage 1 still ships (it recovers
  what it can), Stage 2 becomes the main NVIDIA road.

VERDICT row in the vitriol ledger either way. The ceiling number is a
standing per-driver-era measurement (doctrine §6).

## Stage 1 — Portable extraction (~1 session, ships regardless)

Universal SPIR-V levers, each landing behind the in-process A/B, each
with a VERDICT row, losers reverted:

- **D1 — 2 panels per stage.** Double-buffer depth 4 panels total;
  halves barrier count per panel. Touch: `emit_coopmat_smem` staging
  geometry + fill scheduling. smem budget: 4 panels ≈ 32 KB — fits.
- **D2 — register-prefetch software pipeline.** Issue panel k+2 global
  loads into registers during mma(k); store to smem after the mma
  batch; one barrier pair per iteration retained. The portable
  emulation of `cp.async` (SPIR-V has no async global→workgroup copy).
- **D3 — f16x2 fill loads.** The scalar fill loads one half per
  `OpLoad`; uint/ushort2 loads halve fill instruction count.
- **D4 — occupancy shaping.** LocalSize search (O5 rung) under the
  hardened harness — the R sweep showed the accumulator-register trade;
  LocalSize × workgroup count is the remaining axis.

Target: 16.5 → 20–22 TFLOP/s at 4096³ if the pipeline is the binding
constraint (Stage 0's cost-share probe says which).

## Stage 2 — Briev PTX tier (gated on Stage 0's verdict; multi-session)

- **S1 — driver module** `lib/runtime/briev_dev_cuda.c`: cuInit /
  cuModuleLoadData / cuLaunchKernel in the existing driver-plugin
  shape (`briev_accel_rt.c` registration). Probe-driven selection
  alongside the Vulkan/OpenCL drivers; init-time diagnostic if the
  driver JIT (ptxas) is absent.
- **S2 — emitter** `src/backend/ptx/`: text PTX emission consuming
  `GemmPlan` + tier eligibility + dispatch predicates (analysis-once,
  backend-contracts §2). Surface declared in
  `src/backend/capabilities.rs`. `--backend` name: `ptx`.
- **S3 — tensor GEMM family first**: `mma.sync.aligned.m16n8k16`
  (f32-acc reference tier + f16-acc race tier), `ldmatrix` fragment
  loads, `cp.async` global→smem, warp-tile geometry from `GemmPlan`
  (64×64 default), fragment layout per lane. ggml's kernels are the
  open layout reference; per-op microtests (single mma, single
  ldmatrix) precede integration.
- **S4 — correctness gate**: identical contract vs the naive reference
  tier across the whole shape portfolio (2048³ / 4096³ / 8192³ /
  skinny-K / small shapes), gates as per tier (5e-3 f32-acc, 1e-2
  f16acc).
- **S5 — performance gate**: match the 42.0 TFLOP/s anchor at 4096³,
  then beat it (shape-specialized configs are the structural edge:
  our kernels specialize per program; ggml's dispatch tables cannot).
- **S6 — auto-tune loop**: `derive --stochastic` sweeps
  (tile × stages × warps) per device profile, winners cached in the
  target config (`config/targets.*`). Derived tile tables replace
  hand-written ones — the durable "smarter" (doctrine §4).

Docs to touch when S2 lands (same commit):
`docs/architecture/backend-contracts.md` (new charter row),
`docs/HANDOFF-2026-08-31-gpu.md` (status block),
`spec/SPEC.md` §9.8 (Tier 4 paragraph — written as planned already),
`AGENTS.md` reference index.

## Risks

- **PTX fragment layouts** (m16n8k16 lane mappings, ldmatrix
  addressing) are the classic source of silent wrong answers —
  mitigated: per-op microtests with known-value fragments before any
  integration.
- **Driver-JIT variance** (ptxas version changes codegen): pin the
  ceiling measurement per driver era; SASS inspection optional via
  `cuobjdump` when claiming scheduling properties (the LTO lesson,
  rule 20).
- **Scope creep in S3**: GEMM family ONLY until S5 passes. No other
  workload enters the PTX tier before the anchor race is won.
- **Stage 0 gate discipline**: if the ceiling lands between 25 and 40
  TFLOP/s, re-run the decision with the fill-cost share measured —
  the gate is a number, not a mood.

## Undo

Stage 1 experiments: config + emitter-local, reverted via git; losers
get VERDICT rows. Stage 2 adds new modules/driver files only — the
SPIR-V path remains byte-identical when `--backend spirv` is used and
when the probe selects it. Doctrine doc lives; only the tier table
rows change status.
