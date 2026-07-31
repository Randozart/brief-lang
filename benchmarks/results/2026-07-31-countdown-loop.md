# Countdown-loop — fmn 1.21×→0.94×, kalman 0.85× (universal periodic-guard emission)

**Date:** 2026-07-31
**Plan:** `docs/plans/2026-07-31-fmn-countdown-vs-batch-and-new-benchmarks.md`
**Baseline:** `2026-07-31-regain-kalman-parity-batch-loop.md` (batch loop)
**Harness:** `bash benchmarks/build_and_bench.sh --runtime`, BOUND=50000000
**Raw output:** `/tmp/opencode/cd_runtime.log`

## What changed

A **countdown-loop** emission (`emit_countable_countdown_main`) replaces the
batch loop for periodic post-increment guards (`when count % N == 0` AFTER
`count++`): a single tight loop with a loop-carried `%rem` counter that
decrements each iteration and, on 0, branches to a COLD guard block that prints
and resets `%rem = N`. The `%fire` conditional keeps the loop in one block (no
body-split) and naturally blocks LLVM's mis-vectorization.

## Why (measured)

| fmn structure | loop instrs | vectorized? | time |
|---------------|------------:|-------------|-----:|
| C reference | 24 | scalar | 0.1600s |
| version-DAG | 29 | scalar (guard branch blocks vectorizer) | 0.1962s |
| batch inner loop | 14 | **YES — `vmulps`+shuffle, slower** | 0.2050s |
| countdown | — | scalar | **0.1533s** |

The batch's pure inner loop let LLVM vectorize the cross-indexed 3×3 matrix
into shuffle-heavy code that runs slower than the scalar version-DAG loop, and
reassociate reduction bodies (changing float_math's output). The countdown
avoids both: the guard's conditional blocks the mis-vectorization and the
`sub;cmp` replaces the modulo.

## Full harness (zero MISMATCH)

| Benchmark | baseline (batch) | with countdown |
|-----------|-----------------:|---------------:|
| kalman_filter_runtime | 1.02× | **0.85×** |
| float_math_nonzero | 1.21× | **0.94×** |
| float_math | 0.96× | **0.62×** |
| print_loop | 1.05× | **0.64×** |
| queue_drain (×3) | 0.90×/0.90×/0.91× | **0.47×/0.62×/0.57×** |
| all others | within noise | unchanged |

The countdown is **universal** — faster than C for kalman and fmn, with correct
output (matches the version-DAG's output; the earlier float_math/batch
reassociation artifact is gone). The `arithmetic_op_count >= 40` dispatch
heuristic is removed (no longer needed).

## Correctness

- kalman output matches the version-DAG's value (a valid -ffast-math result the
  harness accepts; strictly closer to the exact computation than the batch-era
  values).
- float_math output matches the version-DAG EXACTLY (1434824.38 — the
  reassociation artifact the batch introduced is gone).
- print_loop output matches C exactly; queue_drain harness MATCH.

## Tests

`cargo test --lib`: **1275 passed** (dispatch test updated: post-increment →
countdown `.cd_`/`.cdg_`; pre-increment still rejected). Praetor clean.

## Files

| File | Change |
|------|--------|
| `src/backend/llvm/loop_engine/counter.rs` | `emit_countable_countdown_main` (single loop, `%rem` phi, cold guard block) |
| `src/backend/llvm/mod.rs` | dispatch: periodic post-increment guard → countdown (replaces the batch arm) |
| `src/analysis/batch_shape.rs` | `BatchShape.guard_body` (guard inner statements for the cold block) |
| `src/backend/llvm/tests.rs` | dispatch tests updated to the countdown |
| `docs/plans/2026-07-31-fmn-countdown-vs-batch-and-new-benchmarks.md` | §10 results |
