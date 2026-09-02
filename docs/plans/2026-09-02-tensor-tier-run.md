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

---

## Result (2026-09-02, same session — the tier is CORRECT on device)

**The M2.2 blocker is resolved.** Root cause found by bisecting against
the naive reference (doctrine step 2 in action):

1. **The coopmat grid decode had tile_m/tile_n swapped** (gemm.rs) —
   X-flatten is row-MAJOR: `tile_m = wgx / tiles_x; tile_n = wgx %
   tiles_x`. The swapped decode computed only 64 of 256 tile-rows —
   exactly the "~25% y-fill" the M2.2 session attributed to
   driver/NVVM. It was never the driver.
2. **The runner's work-count used the v1 formula** (n/(16·16)) — 4×
   over-dispatch for the 16×64-tile kernel; out-of-range tile_n values
   smeared garbage over correct outputs. Fixed: (n/(16·64))·32.
3. Runtime bisect leftovers restored (the features2 probe call and the
   app-info had been left commented from crash isolation — diagnosed by
   `nm` on the built archive object showing the symbol missing).

**Verdict row**: 4096³, rel 2.442e-04 (identical to the naive bound —
two independent implementations agree), min 41.2ms = **6.85 TFLOP/s** —
beats the f32 tiled tier (5.25), 0.54× the ggml anchor. spirv-val
vulkan1.3 clean; `brievc run` counter = 16777216 both tiers; 2039 lib
tests green. The knob stays OFF in the committed config (opt-in
strategy, per doctrine).

**Next perf rungs** (each measured, VERDICT-disciplined):
- A-panel shared-memory reuse across the 4 warp fragments (each
  workgroup re-reads its A strip from DRAM per k-panel today).
- Occupancy: 16384 workgroups × 32 lanes underfills the 3060 — bigger
  tiles per workgroup or multi-tile workgroups.
- The features2 probe reads 0 for 16bit/coop on this driver while
  create + kernels work (NVIDIA under-enforcement) — diagnostic-only,
  but the probe's request-what-is-supported contract is not yet met;
  investigate the chain traversal before trusting it on OTHER vendors.
- The 256³ small-shape quirk (pre-existing, both runtimes) — generality
  work per doctrine step 5.

## B-reuse rung result (2026-09-02, later same session)

`spirv_coopmat_tile_rows` — R 16-row strips per workgroup, B fragments
load once per workgroup and feed R mma chains (B DRAM traffic ÷ R — the
tier's binding limit). Kernel + runner share ONE clamp
(`GemmPlan::coopmat_tile_rows`: cap 8 + power-of-two divisibility
ladder) so grid decode and dispatch can never disagree.

**Ledger row**: R=2 default — min 13.2ms stable = **10.4 TFLOP/s**
(0.83× the anchor's time; 3.1× over R=1). R=4 flashed 9.9ms (13.9
TFLOP/s, past the anchor) but bimodal under co-tenant load — re-A/B on
an idle box. R=16 VERDICT-rejected (slower + miscomputed on device).

**En-route lessons (recorded)**: `field_int(key, idx)` — the idx is a
field INDEX, not a default (passing 8 silently pinned every build to
the fallback, invalidating the first sweep); config knobs must be added
to BOTH the struct and the dbvl line table; the dbvl is include_str!-
baked — knob changes need a compiler rebuild, not just a rebuild of the
blob.

**Next**: quiet-box re-A/B (R=2 vs R=4 promotion decision) · A-panel
DRAM staging · the features2 probe-chain investigation · the 256³
small-shape quirk.

## Investigation results (2026-09-02, evening)

1. **Probe mystery SOLVED — every feature sType was wrong.** The code's
   "16BitStorage" sType (1000146000) is UniformBufferStandardLayout-era;
   "Float16Int8" (1000083000) is actually 16BIT_STORAGE — the printed
   "f16=1" was the driver filling 8-bit storage into our struct — and
   "CooperativeMatrix" (1000246000) matched nothing. Correct values from
   vulkan_core.h: 1000083000 / 1000082000 / 1000506000. The probe now
   reads TRUE on device (16bit=1 uniform16=1 f16=1 coop=1) — the
   request-what-is-supported contract holds, so strict vendors are safe.
   NVIDIA's under-enforcement had masked all of it (kernels ran with
   features never requested).
2. **R=2 vs R=4 re-A/B (3 interleaved rounds)**: R=2 mins stable
   13.38-13.56ms; R=4 bimodal 9.8ms/70ms — the bimodality is systematic
   (occupancy cliff at 128 acc regs/lane), not co-tenant luck. VERDICT:
   R=2 stays default; R=4 documented as the high-variance option.
3. **256³ quirk RESOLVED — not a bug.** gemm_bench derives offsets from
   its M·N·K args; the blob's arrays were literal-sized (16777216) —
   member offsets diverged and y landed outside the harness's view. With
   arrays sized to the shape: 256³ passes at rel 0.0 (1.73ms). Harness
   rule recorded: derive offsets from the generated runner's layout —
   it is the authority.

**Remaining perf rungs**: A-panel DRAM staging across k-panels ·
occupancy shaping (LocalSize search) · quiet-box confirmation of the
R=4 promotion question.

## Clean-window re-A/B + the DVFS finding (2026-09-02, night)

- GPU idle window: R=1 42.5ms stable · R=2 13.3ms stable · R=4 9.6ms
  best-mode, bimodal 66-83ms otherwise. The R=4 bimodality survived a
  quiet box — systematic, not co-tenant.
- Mechanism probed: clocks PULSE 1837↔139MHz with GPU_IDLE throttle
  flags even during one-shot batched submission; power 8-44W/170W, 44°C.
  Not thermal, not power-capped — the driver drops to idle between
  submissions and the DVFS ramp lands inside short kernels.
- Anchor methodology verified: ggml_gemm_bench is sync-per-iter — our
  sync mins are the comparable numbers (R=2 0.82×, R=4 good-mode 1.11×
  the anchor).
- gemm_h_bench gained batched mode (arg 7) for sustained rows.

**Open**: why sustained (batched) runs settle 2.5× above burst mins
(R=2 33ms, R=4 18ms) — persistence mode / clock pinning (root/NVML)
is the next lever; it likely recovers the burst rate for deployment
loops, which is where an LLM server actually lives.

## Clock lock + R=4 promotion (2026-09-02, latest)

User locked clocks: `sudo nvidia-smi -pm 1 && sudo nvidia-smi -lgc 1800,1837`.
With the lock, the R=4 bimodal mode is GONE — unfed mins 9.56-9.78ms
stable = **14.3 TFLOP/s, 1.14× past the ggml anchor (12.6)**. R=2 stays
13.4ms (DRAM-bound, clock-insensitive).

**DEFAULT PROMOTED TO R=4** (config + dbvl): the committed pipeline now
builds the anchor-beating tier; runner formula (16·4·64), spirv-val
clean, rel 2.442e-04, min 9.68ms verified end-to-end.

> **REMINDER (user request): unlock clocks later —**
> `sudo nvidia-smi -rgc && sudo nvidia-smi -pm 0`
> (both also reset on reboot unless systemd-persisted). Without the
> lock, bursty callers on this box should set spirv_coopmat_tile_rows=2
> (stable 13.4ms) — R=4 reverts to its bimodal 9.8/70ms modes.

Ledger: naive 502ms → R=1 41ms → R=2 13.3ms → **R=4 9.6ms (14.3 TFLOP/s
locked)** vs anchor 10.9ms / 12.6 TFLOP/s.
