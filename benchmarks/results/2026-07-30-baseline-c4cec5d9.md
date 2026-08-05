# Baseline Benchmark Results — Commit `c4cec5d9`

**Date:** 2026-07-30
**Commit:** c4cec5d9a488646b34cfa67e034aed50988e297a
**Tag:** Batch-loop guard hoisting fix — nbody_newton at 0.83× C
**Toolchain:** `clang 18.1.3`, `llc 18.1.3`

## Runtime Benchmarks

5 iterations per benchmark, BOUND=50000000, nanosecond-precision fork+exec timing.

| Benchmark | Briv | C | Ratio | Winner | Correct |
|-----------|:-----:|:--:|:-----:|:------:|:-------:|
| ring_buffer | .0541s | .0458s | **1.18×** | C | MATCH |
| float_math | .0065s | .0733s | .08× | Briv | MATCH |
| float_math_nonzero | .2137s | .1670s | 1.27× | C | MATCH |
| sparse_dispatch | .0513s | .0612s | .83× | Briv | MATCH |
| print_loop | .0001s | .0586s | 0× | Briv | MATCH |
| **nbody_newton** | **6.9434s** | **8.2763s** | **.83×** | **Briv** | **MATCH** |
| nbody_sqrt | 2.1625s | 2.8141s | .76× | Briv | MATCH |
| nbody_sqrt_idio | 2.6355s | 3.6471s | .72× | Briv | MATCH |
| fasta | .2094s | .2100s | .99× | Briv | MATCH |
| fannkuch_redux | .0621s | .0639s | .97× | Briv | MATCH |
| mandelbrot | .6952s | .6638s | 1.04× | C | MISMATCH |
| kalman_filter_runtime | .1817s | .1793s | 1.01× | C | MATCH |
| knucleotide | .2290s | .1905s | 1.20× | C | MISMATCH |
| cancel_math | .0004s | .0641s | 0× | Briv | MATCH |
| bit_clear | .0002s | .0002s | 1.00× | ~tie | MATCH |
| queue_drain | .0030s | .0623s | .04× | Briv | MATCH |
| queue_drain_sym | .0004s | .0608s | 0× | Briv | MATCH |
| queue_drain_idio | 0s | .0604s | 0× | Briv | MATCH |
| interval_step | .0629s | .0640s | .98× | Briv | MATCH |
| bridge_glue | done | — | — | — | SKIP |
| bridge_multi | done | — | — | — | PASS |

## Summary

**Primary achievement:** nbody_newton improved from 1.22× C to **0.83× C** (Briv beats C by 17%) by fixing the `split_hoistable` safety check to:
1. Process the `let_to_field` remapping map (energy → last_energy) so the periodic print guard is correctly identified as safe to hoist
2. Handle `Expr::Block` in the safety check (PrintLn! resolves to Block after the print plugin runs)

**Remaining MISMATCH benchmarks (pre-existing, predate today's work):**
- **mandelbrot** (1.04× C, briv output differs from C)
- **knucleotide** (1.20× C, briv output differs from C)

**Remaining Ring Buffer** (1.18× C) — pointer boxing overhead via `inttoptr` on Ptr<Int> state fields. The `!invariant.load` metadata helps but doesn't eliminate the round-trip cost entirely.

## Changes Since Previous Baseline (`4fa1641e`)

- Batch-loop optimization (inner/outer loop structure)
- Float constant emission: single `bitcast i32` instead of `add+bitcast+fadd`
- AoS→SoA field reorder pass (front-end, before `build_field_index`)
- `!invariant.load` on Ptr<T> state fields
- Briv-level LICM (hoist loop-invariant let-bindings)
- `declare_struct_types` deduplication (fixed `%String` redefinition)
- Guard emission in `.inner_exit_124` with `let_to_field` remapping
- `split_hoistable` safety check with `Expr::Block` and `let_to_field` support
