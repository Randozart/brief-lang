# Phase 0 — Baseline Results (frontend-driven dispatch effort)

**Date:** 2026-07-31
**Commit:** `5fed0573` (FDD worktree at `../briv-compiler-fdd`, branch `feat/frontend-driven-dispatch`)
**Compiler code equivalent to:** `666fb502` (baseline worktree `../briv-compiler-baseline`)
**Harness:** `bash benchmarks/build_and_bench.sh --runtime`, BOUND=50000000
**Toolchain:** `clang 18.1.3`, `llc 18.1.3`
**Raw output:** `/tmp/opencode/phase0-runtime.txt`

## Runtime ratios (Briv vs C, ratio < 1 = Briv faster)

| Benchmark | Briv time | Ratio | Winner | Correct |
|-----------|-----------:|:-----:|:------:|:-------:|
| ring_buffer | 0.0543s | 1.13× | C | MATCH |
| float_math | 0.0721s | 0.97× | Briv | MATCH |
| float_math_nonzero | 0.2082s | 1.24× | C | MATCH |
| sparse_dispatch | 0.0527s | 0.84× | Briv | MATCH |
| print_loop | 0.0620s | 1.01× | C | MATCH |
| nbody_newton | 6.8833s | 0.83× | Briv | MATCH |
| nbody_sqrt | 2.1812s | 0.77× | Briv | MATCH |
| nbody_sqrt_idio | 2.7124s | 0.75× | Briv | MATCH |
| fasta | 0.2040s | 0.97× | Briv | MATCH |
| fannkuch_redux | 0.0621s | 0.97× | Briv | MATCH |
| mandelbrot | 0.6769s | 1.02× | C | MATCH |
| kalman_filter_runtime | 0.2191s | 1.22× | C | MATCH |
| knucleotide | 0.1874s | 0.99× | Briv | MATCH |
| cancel_math | 0.0519s | 0.85× | Briv | MATCH |
| bit_clear | 0.0001s | ~tie | ~tie | MATCH |
| queue_drain | 0.0540s | 0.86× | Briv | MATCH |
| queue_drain_sym | 0.0553s | 0.89× | Briv | MATCH |
| queue_drain_idio | 0.0567s | 0.94× | Briv | MATCH |
| interval_step | 0.0632s | 1.01× | C | MATCH |

**Zero MISMATCH.**

Notes:
- `bridge_glue`/`bridge_multi` failed at the end (missing `koffi` node package) — unrelated to this effort; they are not in the runtime ratio table.
- Baseline comparison: this table tracks the recorded `666fb502` baseline within run-to-run noise (ring_buffer 1.13 vs 1.15, float_math_nonzero 1.24 vs 1.21, mandelbrot 1.02 vs 1.03, bit_clear ~tie).

## Dispatch decision log (per txn, from `info:` lines)

Captured in `benchmarks/*.ll` + compiler `info:` warnings during this run; the dispatch-sensitive benchmarks for A/B are:
`nbody_newton`, `nbody_sqrt`, `nbody_sqrt_idio`, `kalman_filter_runtime`,
`ring_buffer`, `float_math`, `float_math_nonzero`, `sparse_dispatch`,
`fannkuch_redux`, `knucleotide`, `mandelbrot`, `fasta`.
