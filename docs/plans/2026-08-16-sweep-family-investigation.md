# Sweep-family parity — investigation findings + verification probes

**Date:** 2026-08-16
**Head commit:** `83f12b79`
**Status:** Investigation complete; verification probes pending

This plan records the thorough read-only investigation of the sweep-family
C losses (backlog item 1 of `2026-08-16-next-steps.md`) BEFORE any fix is
built — the "investigate before we fix phantoms" discipline. Findings are
measured on the exact harness pipeline; no repo changes yet.

## 1. Reproduction (harness-exact, BOUND=50000000)

`brievc build <bv> --optimize-budget 256` → `.ll` → the harness clang line
(`-O3 -flto -march=native -ffast-math -fdata-sections -ffunction-sections
-Wl,--gc-sections`). C references: `clang -O3 -march=native -ffast-math`
(harness line 318 — no LTO for C refs). Timing: `LC_ALL=C /usr/bin/time -f
"%e"`, ×3, harness BOUND default.

| Benchmark | Briev | C | Ratio | Recorded (results/2026-08-15) |
|-----------|------:|---:|------:|------:|
| sweep_sparse | .22s | .16s | **1.37×** | 1.43× |
| sweep_mid | .26s | .24s | **1.08×** | 1.10× |
| sweep_dense | .40s | .27s | **1.48×** | 1.50× |
| sweep_arr | .41s | .35s | **1.17×** | 1.17× |

Reproduction matches the recorded table within noise. Machine: **Intel
i7-3770 (Ivy Bridge) — AVX1 only, NO AVX2, NO FMA.** This matters: every ymm
shuffle is port-5 µops, lane-crossers (vperm2f128, vinsertf128) are expensive,
and 256-bit integer ops are absent. C is compiled natively to the same
feature set.

## 2. The Why — the family is NOT one mechanism

Per-iteration hot-loop census (objdump, backedge-to-backedge):

| Variant | Briev instrs | C instrs | Shuffle diff | Diagnosis |
|---------|------------:|---------:|-------------:|-----------|
| dense | 40 (6 vmulps + 4 vaddps + **24 shuffle/select**) | 34 (6 vmulps + 4 vaddps + **14 shuffle**) | **+10** | throughput: extra shuffle-port ops saturate port 5 |
| sparse | 19 (3 zero-copies + 2-wide SSE pack chain) | 23 (**5-op modulo** + 12 scalar) | +3 | latency: vmovsldup/vblendps/vmovshdup serial chain lengthens the critical path |
| arr | 36 (15 shuffle) | 42 (21 shuffle) | **−6** | B has FEWER ops yet slower → scheduling/UF, not count |
| mid | 38 (32 scalar) | 34 (24 scalar + modulo) | 0 | near-parity; residual = the zero-copy idioms |

### 2.1 Common structural facts (both sides vectorize)

- dense B and C both emit 8-wide AVX1 loops with vaddps/vmulps — the earlier
  llc-only "scalar" reading was the LTO trap (rule: verify on the linked
  binary).
- The Briev countdown loop (`emit_countable_countdown_main`, counter.rs:934)
  is chosen by the frontend `batch_shape` dispatch (mod.rs:4811-4828) for
  periodic post-increment guards. It carries the state as scalar SSA phis
  (16 for dense), computes 48 fast FLOPs, fires the periodic guard via
  `%rem` countdown.
- C's `float f[16]` array form + clang produces cleaner vectorization of the
  same cross-indexed chain.
- C pays a per-iteration modulo recompute (mov/mul/shr/imul, ~5 ALU ops) on
  every loop; Briev pays only the countdown `dec;cmp`. C still wins on
  dense/arr — so the gap is Briev's VECTOR inefficiency, not loop overhead.

### 2.2 Contradictions that refute the naive op-count theory

- **sparse B (19 instr) and arr B (36 instr) both have FEWER total
  instructions than their C counterparts (23 and 42) yet lose.** Instruction
  count does not explain the ratios.
- dense B's +10 shuffles explains dense (500M extra shuffle µops × port-5
  limit ≈ 0.13s ≈ the gap). arr B's −6 shuffles shows the arr loss is
  scheduling/UF. sparse's loss is critical-path latency (2-wide SSE pack
  chain), not throughput.

### 2.3 The `%rem` guard placement

C fires the periodic guard via a backward `je` at loop top (the modulo
condition merged into the loop branch); Briev fires via `dec %rem; je` at the
loop bottom. Both cheap; not the differentiator.

## 3. This is the documented Phase-3 gap (not new)

`docs/plans/2026-07-31-fmn-countdown-vs-batch-and-new-benchmarks.md`:
the countdown is the proven-universal structure for periodic post-increment
guards (real benchmarks kalman 0.85×, fmn 0.94×, float_math 0.62×,
print_loop 0.64×, queue_drain 0.47-0.62× — all at/below C). The sweep family
was built in that same plan §Phase 2 to STRESS the countdown's boundary; its
synthetic cyclic-tridiagonal chains (`fi = a·fi + b·f{i+1} + c·f{i−1}`,
all-reads-then-all-writes) mis-vectorize into shuffle-heavy AVX1 code.
Phase 3 (structural cross-indexing detection) was deferred. This plan is
that follow-up.

## 4. Verification probes (P1–P4) — run before any fix

Each is a pure /tmp experiment on the harness pipeline: patch the emitted
`.ll` (or the emission), link, time ×5, compare to the §1 table. No repo
change until a probe wins.

| Probe | Variant | Change | Hypothesis to confirm |
|-------|---------|--------|----------------------|
| P1 | sparse | `llvm.loop.vectorize.enable=false` | the 2-wide SSE pack HURTS (latency); scalar chains ~1.0× |
| P2 | dense, arr | `llvm.loop.interleave.count=2` | gap is UF/scheduling |
| P3 | arr | `llvm.loop.unroll.count=2` | separates UF from bridge |
| P4 | mid, dense | drop the `fadd 0.0` latch copy idioms | the copies cost the residual |

## 5. Decision tree (after P1–P4)

1. **A probe wins cleanly** (variant ≤ 1.0×, full harness zero regression) →
   land it as a structural emission/metadata decision (config-driven,
   frontend-dispatched — no benchmark-keyed special-casing), with tests +
   docs.
2. **None move the ratios** → the residual is clang's codegen boundary on
   AVX1 against our scalar-phi web. The only guaranteed fix is real
   vector-state SSA (VectorPhiGroup: emit the carried group as `<8 x float>`
   phis + shufflevector, mirroring C's array form — mod.rs:4860-4865
   "Emission is identical today" becomes real). Substantial; fixes dense/arr
   but NOT sparse/mid.
3. **Accept + pivot**: record the definitive A/B in this plan, log the
   AVX1-scheduling finding in BUGS.md, mark the sweep family a documented
   codegen boundary, move to a higher-value backlog item (D2 pre-grow, coll
   track).

## 6. Cost-benefit (honest)

Real-program benchmarks are all 0.6–1.0× (at/beating C). The sweep family is
a synthetic stress case. Total possible recovery ≈ 0.24s across four
benchmarks on a 2012 AVX1 CPU. Not a 7×-style structural win. Probes are
cheap and decisive; the vector-state SSA endgame is justified ONLY if a
probe shows the loss is real and structural across the family.

## 7. House rules compliance

- Rule 19: hypotheses measured on the ACTUAL linked binary before building.
- Golden rule 5: additive only; no existing match arm modified.
- Rule 11/11b: baseline recorded here (§1); A/B after any change; full
  harness gates.
- No benchmark-keyed special-casing — landing is structural + config-driven.
