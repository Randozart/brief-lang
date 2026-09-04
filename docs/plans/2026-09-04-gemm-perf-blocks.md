# GEMM perf blocks: hardened measurement → R sweep → shape sweep

**2026-09-04.** Follows the smem A/B parity verdict (ledger row 2026-09-04).
State: smem+f16acc+R=4 best window 8.15ms ≈ 16.9 TFLOP/s; ggml-cuda anchor
3.2ms = 42.0 TFLOP/s; gap 2.6×. No ncu/nsys on the box; root lost (no clock
pinning). DVFS bimodality (8ms boost vs 17-25ms throttle windows) poisons
A/Bs — fixed before any more perf conclusions.

## Baseline table (recorded BEFORE changes)

Benchmark: `gemm_h_bench`, 4096³, batched ×20, dispatch 131072, UNLOCKED
clocks. All runs 2026-09-04 (commit 743661b1).

| Kernel | per-call ms (6 interleaved rounds) | best-ever window | correctness |
|--------|-----------------------------------|------------------|-------------|
| smem (51b791cd) | 18.2 / 21.3 / 16.6 / 23.3 / 19.7 / 21.8 | 8.15ms = 16.9 TFLOP/s | 4.476e-03 OK |
| direct (3d6ffdb2 compiler) | 19.7 / 20.2 / 8.62 / 23.1 / 21.2 / 25.4 | 8.62ms = 15.9 TFLOP/s | 4.436e-03 OK |

Paired rounds 3-3 → parity. Best-vs-best 8.15 vs 8.62 (single boost-window
samples each — NOT a result).

## Block 1 — measurement hardening (do first)

1. `gemm_h_bench.c`: WARMUP becomes argv-tunable (`warmup` arg, default 5,
   use 50 for boost forcing). Fingerprint line prints it.
2. In-process A/B mode: argv `--ab <spv2> [dispatch2]` — init BOTH kernels
   (idx 0 = kernel A, idx 1 = kernel B), each with its own state allocation;
   alternate launches inside the batched submission loop; download and verify
   both. Same clocks, same submission window, launch-by-launch alternation —
   the between-round DVFS skew dies.
3. Re-run the smem-vs-direct A/B under the hardened harness. This number
   REPLACES the parity verdict's best-window comparison if it disagrees.

## Block 2 — R sweep (config-only experiments)

Knobs (config/ir-lowering.dbvl): `spirv_coopmat_tile_rows` 4→2 (halves
accumulator fragments per lane: 16→8 16×16 fragments ≈ 64→32 regs → more
resident workgroups), optionally `spirv_coopmat_subgroups` 2 revisit under
smem. Each variant: edit dbvl → rebuild brievc → emit SPV → in-process A/B
vs the R=4 SPV. Hypothesis under test: register pressure from 16 fragments
limits occupancy; R=2 trades smem reuse for parallelism.

Record in ledger: numbers + verdict per variant; revert losing configs.

## Block 3 — shape sweep

Shapes: 2048³ (64MB footprint), 8192³ (384MB ≫ 3MB L2 — smem should pull
ahead), 4096×4096×512 (skinny K, pipeline depth 32 panels). Both kernels per
shape via the A/B mode. Contextualizes the 4096³ parity (L2 cover) and
checks the default across footprints.

## Hygiene (same session if time)

- `spirv_coopmat_smem` knob is DEAD (dbvl:68, never read in Rust). Gate
  kernel.rs's smem-array creation on it (default 1 keeps current behavior).
- Praetor flags on the new emitters: deferred to post-campaign refactor
  (recorded in 51b791cd).

## Success criteria

- One trustworthy A/B number per (kernel, shape, R) cell.
- A ledger verdict per Block-2 variant: keep / revert, with the reason.
- No config change without the in-process A/B backing it.

## Undo

All config edits revert via git (dbvl tracked); bench changes are additive
argv modes; no codegen changes in Blocks 1-3 (SPV regenerations only).
