# The .abv GPU Doctrine — One Program, Peak Everywhere, Briev-Owned Codegen

**2026-09-04.** Codifies the strategic direction locked by the user
("for Briev, the answer is always a"): a single `.abv` program runs at
peak performance on every probed device, and every byte of generated
machine code is the compiler's own. This document is the durable
statement; `docs/plans/2026-09-04-beyond-coopmat.md` is the staged
campaign that realizes it.

Read together with: `gpu-model.md` (borrowing-not-barriers thesis),
`backend-contracts.md` (analysis-once + capability matrix),
`benchmark-strategy.md` (anti-overfit doctrine, VERDICT discipline),
`spec/SPEC.md` §9.8 (kernel synthesis tiers).

---

## 1. The doctrine

1. **One language, one program.** The author writes intent: a counted
   loop over a proven-disjoint counter (`accel` + `[i < N][i == N]`,
   SPEC §9.7). No GPU vendor appears in the source. No strategy
   keywords, no per-device pragmas, no tile sizes. The program is
   portable by construction.

2. **Peak is the compiler's job.** The compiler carries ALL machine
   knowledge: tile geometry, pipeline depth, fragment allocation,
   barrier placement, dispatch shape. It derives what CUDA programmers
   hand-write — per device probe, per shape bucket, per program
   (`M`, `N`, `K` are compile-time constants in `.abv`; a library
   cannot specialize per program — we do).

3. **Briev-owned codegen only.** The compiler's answer is never
   "call cuBLAS" and never "run nvcc". FFI lanes (`frgn`, `#Link<>`)
   exist for users; they are not the codegen path. Every tier of GPU
   code — portable or vendor-specialized — is emitted by Briev from
   Briev's own frontend decisions. (GGML's kernels are legitimate
   *references* to validate against; never a dependency.)

4. **Specialize per device, from one frontend.** The frontend analysis
   (`GemmPlan`, tier eligibility, LoopShape, dispatch predicates)
   computes ONCE. Every backend consumes the same decisions
   (backend-contracts.md §2). Vendor backends are *projections* of one
   plan into one ISA's primitives — not independent codepaths that can
   disagree. Device probing at runtime selects the tier; the source
   program never changes.

5. **Measure before build; VERDICT the losers.** Every mechanism is an
   A/B on device (the in-process harness discipline, ledger
   2026-09-04b) before it becomes a default. A losing optimization is
   a VERDICT entry, never a silent revert, never a heuristic that
   "usually helps".

## 2. Why this doctrine exists — the ceiling physics

The f16-vs-f16 race target (ledger 2026-09-02): ggml-cuda at
**42.0 TFLOP/s** on the anchor shape (4096³), = 83% of the RTX 3060's
50.6 TFLOP/s FP16-accumulate tensor peak — via hand-written PTX
(`mma.sync` double-pumped, `ldmatrix`, `cp.async`).

Our portable path (SPIR-V `VK_KHR_cooperative_matrix`) measured
16.5 TFLOP/s = 33% of peak. The f16acc-vs-f32acc delta was only ~11%
— if the vendor's SPIR-V lowering actually double-pumped mma, that
switch would move the ceiling ~2×. The strong hypothesis: **vendor
SPIR-V lowering is a structural variable** that caps the portable path
below the CUDA-class ceiling, independent of how smart our SPIR-V is.

Consequence: "peak everywhere" cannot mean "SPIR-V only". It means
**backend tiers**: the portable tier ships and keeps its full
optimization commitment (doctrine: "the SPIR-V backend gets the same
optimization commitment as LLVM"); vendor-specialized tiers — Briev's
own PTX generator first — carry the race where the portable ceiling
provably falls short. The standing measurement that governs this is
the **coopmat ceiling microkernel** (register-resident mma chain, no
memory): one number, per driver, that decides where each workload's
peak can live. Stage 0 of the campaign measures it; the number goes in
the ledger and stays current per driver era.

## 3. The backend tier architecture

| Tier | Backend | Scope | Status |
|------|---------|-------|--------|
| Portable | SPIR-V (Vulkan compute; OpenCL driver present) | All coopmat/row/flat tiers, all vendors | default, fully committed |
| Specialized | **PTX** (Briev-emitted, driver-JIT via `cuModuleLoadData`) | NVIDIA tensor-class workloads | planned (campaign Stage 2) |
| Specialized | AMD / Intel native (ROCm-shaped / Level Zero+SPIRV-direct) | future — same pattern, one vendor at a time | future |

Rules that hold across all tiers:

- **Same plans in.** A specialized tier consumes `GemmPlan` + tier
  eligibility + dispatch predicates. It never re-derives strategy from
  syntax. Kernel emission and host dispatch derive from ONE predicate
  (SPEC §9.8, dispatch geometry contract) — per backend.
- **Same contract out.** Every tier verifies against the naive
  reference tier (the general path) on every benchmark shape. f16acc
  tiers carry their own ≤1e-2 numerics gate; the f32-acc and naive
  tiers remain the correctness reference (anti-overfit doctrine).
- **Capability matrix.** A specialized tier declares its surface in
  `src/backend/capabilities.rs`; out-of-surface programs still compile
  on the portable tier — probe failure falls back, never fails the
  program (SPEC §9.8 gate discipline).
- **Probe-driven, config-disclosed.** Tier selection is runtime device
  probe + `config/targets.*` profiles — disclosed machinery, never
  hidden heuristics (golden rule 3). The author sees which tier ran
  (`brievc` diagnostics, bench fingerprints); the source never encodes
  it.

## 4. What "ultimate GPU code" means operationally

- For the **author**: `.abv` reads as the algorithm. No CUDA dialect,
  no vendor intrinsics, no tuning folklore. Contracts state the
  semantic obligation; proofs carry the disjointness (no manual
  barriers — `gpu-model.md`).
- For the **compiler**: each new device class is a new *projection* of
  existing plans, not a new research project. The expensive knowledge
  (tiering, tiling, pipelining, verification) lives in the frontend
  and is paid once.
- For the **ecosystem**: auto-tuning (the `derive --stochastic` MCMC
  machinery, campaign Stage 2 S6) sweeps the strategy space per device
  profile and caches winners — derived tile tables instead of
  hand-written ones. This is the durable "smarter than CUDA": CUDA
  expert intuition, systematized, re-derived per device and per shape,
  and specialized per program.

## 5. Non-goals

- **No runtime codegen dependency**: no shipping nvcc/ptxas as a
  *build* dependency of the compiler; driver-JIT (cuModuleLoadData)
  uses the driver's own compiler at runtime, with init-time
  diagnostics when absent.
- **No vendor lock-in via libraries**: cuBLAS/cuDNN/hipBLAS calls are
  user FFI lanes, not benchmarks of the compiler and never defaults.
- **No source-level specialization**: strategy keywords stay
  correctness/intent-only (golden rule 2). A keyword never carries a
  performance win; a beaten default is a compiler bug.

## 6. Standing obligations

- The coopmat ceiling number is re-measured per driver era (it is a
  vendor variable) and recorded in the vitriol ledger.
- Every tier lands with: harness A/B (in-process, alternating-order),
  correctness sweep over the benchmark shape portfolio, ledger VERDICT
  row, capability-surface declaration, and this document updated in the
  same commit if the tier architecture itself moved.
