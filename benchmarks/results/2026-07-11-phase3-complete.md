# Benchmark Results — Post Phase 3 (String Migration + Factory Methods)

> **Note:** Superseded by baseline at commit `be6583bc` (2026-07-21, post-SLP anchor).
> All benchmarks improved — see the comprehensive plan document for current results.

**Commit:** `8a827db1bb600d64740daab52b4613ab7f5cedae`
**Date:** 2026-07-11
**Build:** `cargo build --release`

## Runtime Benchmarks

5 iterations per benchmark, avg wall clock via `CLOCK_MONOTONIC`.
`BOUND=50000000`. Nanosecond-precision fork+exec timing harness.

| Benchmark | Briev | C | Ratio | Winner | Correct |
|-----------|-------|---|-------|--------|---------|
| ring_buffer | 0.0686s | 0.0676s | 1.01x | C | MATCH |
| float_math | 0.0631s | 0.0771s | 0.81x | Briev | MATCH |
| float_math_nonzero | 0.1920s | 0.1727s | 1.11x | C | MATCH |
| sparse_dispatch | 0.0060s | 0.0657s | 0.09x | Briev | MATCH |
| print_loop | 0.0639s | 0.0670s | 0.95x | Briev | MATCH |
| nbody_newton | 7.4132s | 9.8522s | 0.75x | Briev | MATCH |
| nbody_sqrt | 3.0046s | 3.5218s | 0.85x | Briev | MATCH |
| nbody_sqrt_idio | 2.9578s | 4.3184s | 0.68x | Briev | MATCH |
| fasta | 0.2695s | 0.2636s | 1.02x | C | MATCH |
| fannkuch_redux | 0.0763s | 0.0789s | 0.96x | Briev | MATCH |
| mandelbrot | 0.7514s | 0.7538s | 0.99x | Briev | MATCH |
| kalman_filter_runtime | 0.1876s | 0.1887s | 0.99x | Briev | MATCH |
| knucleotide | 0.2093s | 0.2060s | 1.01x | C | MATCH |
| cancel_math | 0.0682s | 0.0672s | 1.01x | C | MATCH |
| bit_clear | 0.0010s | 0.0009s | 1.11x | C | MATCH |
| queue_drain | 0.0007s | 0.0632s | 0.01x | Briev | MATCH |
| queue_drain_sym | 0.0639s | 0.0672s | 0.95x | Briev | MATCH |
| queue_drain_idio | precomputed | — | — | — | SKIP |
| interval_step | 0.0009s | 0.0669s | 0.01x | Briev | MATCH |

## Optimizer Benchmarks

| Benchmark | Status | Correct |
|-----------|--------|---------|
| iir_filter | precomputed | MATCH |
| precompute_sum | precomputed | MATCH |
| const_heavy | precomputed | MATCH |
| async_counters_idio | precomputed | SKIP |

## Summary

- **All runtime benchmarks:** MATCH (no regressions)
- **All optimizer benchmarks:** MATCH / SKIP (no regressions)
- **Performance range:** 0.01x (sparse dispatch) — 1.11x (float_math_nonzero)
- **Briev beat C on:** nbody benchmarks, sparse_dispatch, queue_drain, interval_step, float_math, mandelbrot, print_loop, fannkuch_redux
- **C beat Briev on:** fasta, cancel_math, bit_clear, knucleotide, ring_buffer, float_math_nonzero
