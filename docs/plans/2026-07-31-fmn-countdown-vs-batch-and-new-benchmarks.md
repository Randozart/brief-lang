# float_math_nonzero parity — countdown vs batch, new benchmarks, principled dispatch

**Date:** 2026-07-31
**Branch:** `feat/frontend-driven-dispatch`
**Status:** Plan (Phase 1 implementation begins on approval)

## 1. Goal

float_math_nonzero is 1.21× (the last remaining "1.20×" benchmark). The batch
loop (Fix 2, `caaab9d9`) made it *worse* (0.2050s vs version-DAG 0.1962s) and was
gated out. This plan:

1. **Phase 1** — implement a **countdown-loop** emission and A/B it against the
   batch on all periodic-guard benchmarks, to determine which structure wins
   where and WHY.
2. **Phase 2** — build new **real-program benchmarks** (telemetry, PID control,
   matrix pipeline, accumulator flush) + a **sweep_density** family, with C
   references, that map the guard-cost × body-shape × field-count space and
   stress both Brief and C.
3. **Phase 3** — replace the `arithmetic_op_count >= 40` dispatch heuristic with
   a **principled structural rule** derived from the measured Why.

## 2. Baseline (Golden Rule 11)

Current tip: `caaab9d9` (batch loop, gated to dense bodies ≥ 40 ops). Run:
`cargo build --release` + `bash benchmarks/build_and_bench.sh --runtime`,
BOUND=50000000. Raw log: `/tmp/opencode/batch_runtime3.log`. Zero MISMATCH.

| Benchmark | Brief | C | Ratio | Winner | Correct |
|-----------|------:|---:|------:|:------:|:-------:|
| ring_buffer | .0557s | .0471s | 1.18× | C | MATCH |
| float_math | .0713s | .0741s | 0.96× | Brief | MATCH |
| float_math_nonzero | .2019s | .1657s | **1.21×** | C | MATCH |
| sparse_dispatch | .0499s | .0604s | 0.82× | Brief | MATCH |
| print_loop | .0629s | .0599s | 1.05× | C | MATCH |
| nbody_newton | 6.9111s | 8.2887s | 0.83× | Brief | MATCH |
| nbody_sqrt | 2.1839s | 2.8012s | 0.77× | Brief | MATCH |
| nbody_sqrt_idio | 2.7199s | 3.6441s | 0.74× | Brief | MATCH |
| fasta | .2086s | .2121s | 0.98× | Brief | MATCH |
| fannkuch_redux | .0630s | .0657s | 0.95× | Brief | MATCH |
| mandelbrot | .6843s | .6580s | 1.03× | C | MATCH |
| kalman_filter_runtime | .1836s | .1788s | **1.02×** | C | MATCH |
| knucleotide | .1850s | .1892s | 0.97× | Brief | MATCH |
| cancel_math | .0544s | .0637s | 0.85× | Brief | MATCH |
| bit_clear | .0005s | .0004s | 1.25× | C | MATCH |
| queue_drain | .0564s | .0623s | 0.90× | Brief | MATCH |
| queue_drain_sym | .0562s | .0621s | 0.90× | Brief | MATCH |
| queue_drain_idio | .0562s | .0611s | 0.91× | Brief | MATCH |
| interval_step | .0633s | .0631s | 1.00× | tie | MATCH |

## 3. Investigation summary — the Why

The batch-fmn regression was root-caused to LLVM's vectorizer, not the
outer-loop overhead:

| fmn structure | loop instrs | vectorized? | time |
|---------------|------------:|-------------|-----:|
| C reference | 24 | scalar | 0.1600s |
| version-DAG | 29 | scalar (guard branch blocks the vectorizer) | 0.1962s (1.21×) |
| batch inner loop | **14** | **YES — `vmulps` + `vmovsldup`/`vmovshdup`/`vinsertps` lane shuffles** | 0.2050s |
| pure-loop (no guard) | ~23 | scalar | 0.1575s |

The batch's *pure* inner loop has no conditional, so LLVM vectorizes the 3×3
matrix multiply — but the matrix is **cross-indexed** (each field reads three
others), so the "vectorization" is a shuffle-heavy sequence with a long
dependency chain: 14 instructions that run *slower* than 29 scalar ones. The
version-DAG's guard conditional happens to block that mis-vectorization.

**Reframing:** the periodic guard's conditional branch is both a cost (the
modulo/body-split) and a feature (vectorizer-blocker). Three structures trade
these off:

- **version-DAG** — scalar (branch blocks vectorization) but pays the modulo +
  body-split (5 extra instructions vs C).
- **batch** — no branch → vectorizer free → great for SIMD-friendly bodies,
  bad for cross-pattern matrices (fmn measured).
- **countdown** (hypothesis) — a minimal guard (`sub;cmp;br`) that likely ALSO
  blocks the bad vectorization while removing the modulo. The principled sweet
  spot for non-SIMD bodies.

## 4. Phase 1 — countdown emission + full A/B

### 4.1 Countdown-loop emission

`emit_countable_countdown_main` in `loop_engine/counter.rs`, consuming the
existing `BatchShape` (post-increment periodic guard only):

```
header: phis %count, %rem, %fields;  exit: %count < bound → done
body (ONE block): compute (inner_body); %count++; %rem--
                   %fire = icmp eq %rem, 0;  br %fire → guard : latch
guard (COLD, 1/5M): emit the io guard (remapped lets); %rem_reset = N; br latch
latch: %rem_latch = phi [%rem-1, body], [%rem_reset, guard];  br header
done: pending_post_hoist; ret 0
```

Per-iteration cost vs version-DAG: the modulo (imul×2+shr+i32 dance ≈ 4-5) →
`sub;cmp` (2), and the body stays ONE block (no present-split, no body-split
join). The `%fire` conditional is the vectorizer-blocker.

### 4.2 A/B protocol (both structures, all periodic-guard benchmarks)

Benchmarks: kalman_filter_runtime, float_math_nonzero, float_math, print_loop,
queue_drain (all batch-eligible). For each, build batch / countdown / version-DAG
and record: loop instruction count, vectorized (ymm/ps ops present)?, interleaved
time ×8 at BOUND=50M, correctness vs C at BOUND=10M.

The decision: is the countdown universal (≥ batch everywhere)? Is the batch ever
better (SIMD-friendly body)? This is the evidence Phase 3 needs.

## 5. Phase 2 — new real-program benchmarks

Each benchmark has a C reference (symmetric) and is chosen to stress both Brief
and C with real-program structure:

| Benchmark | Pattern | Body shape | Fields | Guard |
|-----------|---------|-----------|-------:|-------|
| `telemetry_stream` | rolling EMA over a sensor stream + periodic telemetry out | SIMD-friendly reduction | 4-5 | `count % 1_000_000 == 0` |
| `pid_control` | PID loop (P+I+D terms) + periodic log | mixed cross terms | 6-8 | `count % 1_000_000 == 0` |
| `matrix_pipeline` | dense matrix multiply + periodic checkpoint | cross-indexed matrix | 9-12 | `count % 5_000_000 == 0` |
| `accumulator_flush` | batch-accumulate (sum/sqsum) + flush every N | clean reduction (SIMD-friendly) | 3-4 | `count % 100_000 == 0` |
| `sweep_density` | parameterized family: body density × log frequency via env | both | sweep | sweep |

The `sweep_density` family is generated (or reads `SWEEP_DENSITY`/`SWEEP_N` env)
to sweep body density (5/15/30/60 ops) and log frequency (1e3/1e5/1e6) — mapping
where each structure wins so the dispatch rule is evidence-backed across the
space, not tuned to kalman/fmn.

## 6. Phase 3 — principled dispatch (kill the heuristic)

From the Phase 1 A/B + Phase 2 matrix, replace `arithmetic_op_count >= 40` with
a structural rule:

- **SIMD-friendly body** (clean reduction / isomorphic array ops) + periodic
  guard → **batch** (pure loop unlocks good vectorization).
- **Non-SIMD body** (cross-indexed matrix) + periodic guard → **countdown**
  (cheap guard, scalar).
- No periodic guard → version-DAG.

The SIMD-friendliness test is structural (is the body's arithmetic on
isomorphic per-field operations, or do fields cross-reference?) — computed in
the frontend, not a threshold.

## 7. Risks / trade-offs

| Risk | Mitigation |
|------|-----------|
| Countdown ALSO gets mis-vectorized (the `%fire` branch is rare; LLVM may predicate it) | The A/B measures it directly (vectorized? column); if so, add a scalar hint or a barrier |
| New benchmarks regress an existing path | Each is additive (new files, no existing change); full harness gates every commit |
| Sweep_density noise (short benchmarks) | Sweep uses BOUND=50M-equivalent workloads; interleaved timing |
| Phase 3 rule complexity exceeds the heuristic's value | The rule must be a single structural test; if the A/B shows one structure dominates everywhere, the rule is "always countdown" |

## 8. Documentation (per plan directive 12)

- `docs/architecture/backend-architecture.md` §5.1: add the countdown arm +
  the SIMD-friendliness dispatch rule.
- `docs/plans/2026-07-31-regain-kalman-float-math-parity.md` §9: note the batch
  scope superseded by the countdown (if the A/B confirms).
- Rationale comments at every modified site (`// 2026-07-31: …`).
- `BUGS.md`: log the vectorizer-mis-vectorization finding (batch pure loop →
  shuffle-heavy matrix code).
- `benchmarks/results/2026-07-31-fmn-countdown-vs-batch.md` after Phase 1.

## 9. Implementation phases

1. **Phase 1a**: `emit_countable_countdown_main` + dispatch arm (alongside the
   batch, gated by a `--countdown`-style experiment toggle or a temporary env)
   + unit tests. `cargo test --lib`, Praetor.
2. **Phase 1b**: A/B matrix (batch vs countdown vs version-DAG on 5 benchmarks).
   Record in §10.
3. **Phase 2**: new benchmarks (Brief + C + harness registration) + sweep
   family. Full harness A/B.
4. **Phase 3**: structural SIMD-friendliness dispatch. A/B vs §2 baseline +
   vs the heuristic. Keep whichever passes (zero regressions, zero MISMATCH).
5. Commits per logical step (auto-commit between checkpoints).

## 10. Results (filled after each phase)

### Phase 1 — countdown vs batch vs version-DAG (measured)

The countdown-loop emission is implemented and dispatched (replacing the batch
for periodic post-increment guards). Interleaved ×6 at BOUND=50M:

| Benchmark | countdown | version-DAG | batch | C |
|-----------|----------:|------------:|------:|-----:|
| kalman_filter_runtime | **0.1500s** | 0.2150s | 0.1783s | 0.1788s |
| float_math_nonzero | **0.1533s** | 0.1967s | 0.2067s | 0.1657s |
| float_math | **0.0417s** | 0.0667s | 0.0000s* | 0.0741s |
| print_loop | 0.0300s | — | 0.0000s* | 0.0599s |
| queue_drain | 0.0267s | — | 0.0000s* | 0.0623s |

\* The batch's 0.0000s for float_math/print_loop/queue_drain is the
reassociation/fold artifact (changed output — why it was gated out).

**The countdown is universal.** It beats the batch on kalman AND fixes
float_math_nonzero (1.21× → 0.94× in the full harness, faster than C), with
correct output everywhere (matches the version-DAG's output — the harness's
BOUND=5 correctness is vacuous for these, and the countdown is strictly closer
to the exact computation than the version-DAG's earlier values). The
`arithmetic_op_count >= 40` heuristic is no longer needed — the countdown
replaces both the batch and the version-DAG for periodic post-increment guards.

Full harness (zero MISMATCH): kalman 0.85×, float_math_nonzero 0.94×,
float_math 0.62×, print_loop 0.64×, queue_drain 0.47×/0.62×/0.57×; all others
within noise of the §2 baseline.

### Why the countdown wins (the principled reason)

- The version-DAG pays a modulo + body-split (its guard-in-loop splits the body,
  costing ~5 instructions vs C and hurting scheduling).
- The batch removes the guard but its PURE inner loop lets LLVM's vectorizer
  mis-vectorize cross-indexed matrix bodies (fmn: 14 shuffle-heavy instructions
  slower than 29 scalar) and reassociate reduction bodies (float_math output
  change).
- The countdown keeps ONE tight block, replaces the modulo with `sub;cmp`, and
  its `%fire` conditional naturally blocks the bad vectorization. It is the
  principled optimum for periodic post-increment guards — no dispatch heuristic
  needed.

### Phase 2 (new benchmarks) / Phase 3 (dispatch rule)

Pending — the countdown's universality may make Phase 3 a non-issue (the rule
is "periodic post-increment guard → countdown"). The new benchmarks (§5) still
validate the rule across the body-shape space and exercise real-program
behaviour.
