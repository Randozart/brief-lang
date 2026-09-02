# Plan: tensor tier run — `spirv_coopmat` ON (M2.2 completion)

**2026-09-02.** Doctrine for this arc: `docs/architecture/benchmark-strategy.md`
§ Anti-Overfit Doctrine (shape tiers, general path as reference,
general-syntax performance parity). Resume context:
`docs/plans/2026-09-02-exponent-notation.md` post-plan note + the ledger row
2026-09-02 in `2026-08-31-vitriol-gemm-comparison.md`.

## State at start

- f16 naive tier: **correct on device** (rel 2.442e-04 @ 4096³, 502ms /
  274 GFLOP/s) — the y-fill fault is gone.
- Runtime: probed features (features2, apiVersion 1.2 instance),
  tensor-capable device preference, real device names. spirv-val clean
  at vulkan1.3 for both tiers' emitters.
- The coopmat tensor emitter exists (M2.2: 16×64 tile, f16 fragments,
  fp32 accumulate, v1 at 2.4 TFLOP/s → rewrite at 5.25 → target 12.6)
  but has never run on device without faulting.
- Anchors: ggml-cuda 10.9ms / **12.6 TFLOP/s** (race target); f32 tiled
  25.3ms / 5.25 TFLOP/s; f16 naive 502ms.

## Steps

1. **Enable + validate**: `spirv_coopmat=1` in `config/ir-lowering.dbvl`
   (session-local, never committed ON — the knob is opt-in strategy).
   Rebuild gemm_h → spirv-val → `brievc run` → `gemm_h_bench` with
   correctness gate. Expected numerics: f32 accumulate ≈ naive tier's
   2.4e-04 bound (possibly better — fp32 fragment accumulate vs the
   naive tier's f32 chain; both single-rounding at store).
2. **If it faults**: bisect with the correct-reference runtime — the
   original fault's suspected enabler (unconditional feature requests on
   possibly-unsupported paths) is removed; dump the kernel with
   spirv-dis, cross-check fragment math against nvcc-compiled reference
   if needed (nvcc present, no ncu).
3. **Bench**: full 4096³, batched + sync rows, vs the ledger anchors.
   A tensor row ≥ 5.25 TFLOP/s (f32 tiled) with rel ≤ 1e-3 is a KEEP;
   ≥ 12.6 (anchor parity) closes M2.2.
4. **Generality gate (doctrine step 3)**: the tensor tier must key on
   shape + `fields_are_f16` capability probes only — audit the tier
   selection for any name/workload match while in there.
5. **Small-shape generality** (doctrine step 5, after the run): the
   256³ dispatch quirk (rel=1.0 on old AND new runtime) — root-cause in
   the plan tiers' geometry assumptions; the general path must be
   shape-robust.

## Ledger discipline

Every row: fingerprint (device, spv size, shape, warmup/iters),
correctness gate BEFORE timing, batched and sync rows, VERDICT against
the threshold. A losing rung is a VERDICT entry.
