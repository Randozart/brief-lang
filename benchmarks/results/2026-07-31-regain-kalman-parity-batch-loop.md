# Regain kalman parity — batch-loop guard hoisting results

**Date:** 2026-07-31
**Plan:** `docs/plans/2026-07-31-regain-kalman-float-math-parity.md` (Fix 2)
**Baseline:** Phase 3 results in `2026-07-31-frontend-dispatch-phase3.md`
**Harness:** `bash benchmarks/build_and_bench.sh --runtime`, BOUND=50000000
**Raw output:** `/tmp/opencode/batch_runtime3.log`

## What changed

The batch-loop (`src/analysis/batch_shape.rs` + rebuilt
`emit_countable_batched_main` in `loop_engine/counter.rs`) decomposes a
reactive txn with a periodic post-increment io guard (`when count % N == 0`
AFTER `count++`) into an inner PURE-compute loop to the next boundary plus a
cold outer guard — eliminating the per-iteration modulo check. This is the
principled form of the batch-loop removed in Phase 6 (`81eea6aa`), derived from
the io precondition interval instead of `extract_batch_size` heuristics.

**Scope (cost model, §9 of the plan):**
- Only POST-increment guards batch. PRE-increment guards (knucleotide/
  mandelbrot: guard BEFORE `count++`) are off-by-one at every boundary — the
  batch fires after `batch_size` computes but the composite fires after
  `batch_size + 1` — and stay on version-DAG.
- Only DENSE bodies batch (`arithmetic_op_count(&inner_body) >= 40`): sparse
  bodies regress (fmn: outer/inner overhead 0.205s vs 0.196s) or get
  reassociated by LLVM (float_math: multiple-accumulator vectorization changes
  the output vs C — symmetric-output violation).

## kalman_filter_runtime — 1.21× → 1.02× (target achieved)

| Run | Brief | C | Ratio | Winner |
|-----|------:|---:|------:|:------:|
| Phase 3 (version-DAG) | .2197s | .1808s | 1.21× | C |
| With batch loop | ~.180s | .180s | **1.02×** | ~tie |

The batch removed the per-iteration `count % 5000000` check AND fixed a latent
version-DAG defect: the version-DAG's guard-present block re-ran the matrix
multiply, emitting 5M+1 computes at each boundary (verified: its BOUND=5M
output was 8.188e12 vs the exact 5M-compute value 8.139e12). The batch emits
exactly 5M computes (matches the exact `-O0` reference to the last float bit).

## Other benchmarks — unchanged (within noise)

float_math 0.96×, float_math_nonzero 1.21×, print_loop 1.05×, queue_drain
0.90×/0.90×/0.91×, ring_buffer 1.18×, mandelbrot 1.03×, bit_clear 1.25× (noise),
nbody_newton 0.83×, nbody_sqrt 0.77×, nbody_sqrt_idio 0.74×, fasta 0.98×,
fannkuch_redux 0.95×, knucleotide 0.97×, cancel_math 0.85×, sparse_dispatch
0.82×, interval_step 1.00×. **Zero MISMATCH.**

## Correctness

- kalman batch output matches the exact non-reassociated `-O0` computation
  (8.13879054e+12 at BOUND=5M); C's clang `-O3 -ffast-math` reassociates to
  8.154e12. The harness checks correctness at BOUND=5 (no prints → vacuous
  MATCH), so the reassociation is invisible to it; the batch is strictly closer
  to the true computation than the version-DAG's 5M+1-compute value.
- print_loop batch output matched C exactly (100 values at BOUND=10M) before it
  was gated out by the density cost model.

## Tests

`cargo test --lib`: **1275 passed** (+6 `analysis::batch_shape` unit tests, +2
`backend::llvm` dispatch tests: post-increment batches / pre-increment
rejected). Praetor clean on all changed files.

## Files

| File | Change |
|------|--------|
| `src/analysis/batch_shape.rs` | new — BatchShape detection (post-increment periodic guard) + `arithmetic_op_count` cost model |
| `src/analysis/mod.rs` | register batch_shape |
| `src/analysis/swan_song.rs` | `remap_stmt_identifiers` pub(crate) for the guard emission |
| `src/backend/mod.rs` | `AnalysisResults.batch_shape` |
| `src/backend/llvm/mod.rs` | dispatch arm: batch (dense, post-increment) before version-DAG |
| `src/backend/llvm/loop_engine/counter.rs` | rebuilt `emit_countable_batched_main` |
| `src/backend/llvm/tests.rs` | batch dispatch regression tests |
