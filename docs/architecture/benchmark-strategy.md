<!-- 2026-06-09 -->

# Benchmark Strategy

## Philosophy

Benchmarks exist to find flaws in Brief, not to produce flattering numbers.

A benchmark that reports `0.0002s vs 0.088s` (440x Brief win) is a diagnostic
signal — it tells you the compiler folded the loop. The compiler is correct.
The benchmark is measuring the wrong thing.

## The Two Categories

Every benchmark belongs to exactly one category:

| Category | Tag | What it measures | Criteria |
|----------|-----|------------------|----------|
| **Runtime** | `--runtime` | Throughput of compiled code | FFI call in the hot loop body. LLVM cannot eliminate the loop. |
| **Optimizer** | `--optimizer` | Compile-time folding power | All `const` inputs + no FFI in the hot loop. LLVM may eliminate the loop. Both Brief and C must produce identical outputs. |

A benchmark cannot be both. If it has no observable side effects in its hot
loop, it is an optimizer benchmark — runtime timing is meaningless.

### Runtime Benchmarks (14)

| Benchmark | What it tests | Loop structure |
|-----------|--------------|----------------|
| `fasta` | FFI `putchar` in hot loop, convergent loop | `__putchar(seed % 26 + 97)` each iteration |
| `fannkuch_redux` | 12-field rotation, SROA scalar decomposition | Pure integer math, FFI only at exit |
| `mandelbrot` | Complex arithmetic + escape tracking | FFI `__print_int` every 5M iters |
| `knucleotide` | 64-field guarded dispatch, compiler switch-gen | FFI `__print_int` every 5M iters |
| `float_math` | Float arrays at contract-proven scale | FFI `__print_float` every 5M iters |
| `float_math_nonzero` | Float with nonzero preconditions | FFI `__print_float` every 5M iters |
| `kalman_filter_runtime` | Kalman filter with runtime bound | FFI `__print_float` every 5M iters |
| `nbody_newton` | Newton-iteration sqrt pipeline | FFI `__print_float` at exit only (runtime bound prevents fold) |
| `nbody_sqrt` | Hardware sqrt via FFI | FFI `__sqrtf` in hot loop |
| `sparse_dispatch` | Sparse conditional dispatch | FFI `__print_int` every 5M iters |
| `cancel_math` | Expression cancellation patterns | FFI `__print_int` every 5M iters |
| `bit_clear` | Bitwise ops + popcount | FFI `__print_int` conditionally (prevents fold) |
| `queue_drain` | Collection push/pop + dispatch | FFI `__print_int` every 5M iters |
| `interval_step` | Interval arithmetic stepping | FFI `__print_int` every 5M iters |
| `ring_buffer` | Folded while-loop with periodic output | FFI `__print_int` every 5M iters |

Sources: `async_counters`, `async_counters_runtime`, `iir_filter_runtime`,
`precompute_sum_runtime`, `ring_buffer_runtime` also runtime-bound
(use `__get_env_int` for loop bound).

### Optimizer Benchmarks (3)

| Benchmark | What it tests | Why foldable |
|-----------|--------------|--------------|
| `iir_filter` | Folded while-loop counter convergence | All `const` inputs + pure float math |
| `precompute_sum` | Compile-time complete evaluation | All `const` inputs, no FFI |
| `const_heavy` | Many constant operands folded | All `const` inputs, no FFI |

These emit correct LLVM IR and produce correct results. The timing is
irrelevant — they are `precompute_ok`.

## How the Harness Decides

After compilation, the harness (`build_and_bench.sh`) inspects the emitted
IR or binary for evidence of an observable loop:

1. **Binary size check**: If Brief `.text` < 25% of C `.text`, flag as
   `precompute_ok` (compiler folded the loop).
2. **IR check**: If the emitted `main()` contains `ret i32 0` and no loop
   back-edge, flag as `precompute_ok`.
3. **Runtime timing**: Only performed for non-precompute_ok benchmarks.

```
=== iir_filter ===
  brief:     1 KB  (precompute_ok — skip runtime)
  c:        36 KB
  brief out: <correctness: match>
  c out:     <correctness: match>
```

## Correctness Verification

Every benchmark, regardless of category, must produce the same output as
its C reference when given the same input. The harness runs both with
`BOUND=5` and compares stdout.

## New Benchmarks

New benchmarks should follow the `fasta` pattern: an FFI call in the hot loop
body that produces observable output. The canonical set from AGENTS.md:

| Benchmark | Pattern | Status |
|-----------|---------|--------|
| `fasta` | FFI output in hot loop | ✅ Done |
| `fannkuch-redux` | 12-field rotation SROA | ✅ Done |
| `mandelbrot` | Complex arithmetic | ✅ Done |
| `knucleotide` | 64-field guarded dispatch | ✅ Done |
| `spectral-norm` | Float arrays, contract-proven scale | 📝 Planned |
| `binary-trees` | Index-based tree walk, struct pool | 📝 Planned |

## CLI Flags

```
bash benchmarks/build_and_bench.sh               # all (current behavior)
bash benchmarks/build_and_bench.sh --runtime     # runtime only
bash benchmarks/build_and_bench.sh --optimizer   # optimizer only
bash benchmarks/build_and_bench.sh --correctness # output verification only
```
