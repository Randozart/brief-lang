# Plan: Fair C Benchmarks + Input Fuzzing

**Date:** 2026-06-01
**Status:** In Progress

## Motivation

The current C reference benchmarks are actively hobbled by `volatile` qualifiers
that prevent clang from applying the same optimizations Briv proves safe. Briv
gets O(1) `store i64 N` while C does O(N) `while (volatile ops < N) ops++`.

Additionally, all benchmarks test only one hardcoded input. Real-world programs
have variable inputs. Fuzzing across input ranges tests both languages under
uncertain conditions and reveals Briv's performance cliff between compile-time
optimization and runtime dispatch.

## Phase 1: Fair C Benchmarks

Remove `volatile` where it prevents legitimate compiler optimizations. Keep only
where needed to prevent dead-code elimination of the entire program:

| Benchmark | Current Issue | Fix |
|-----------|--------------|-----|
| `ring_buffer_c.c` | `volatile long ops` → 50M store-load per iter | `ops = N;` — empty body, O(1) |
| `async_counters_c.c` | Two pthreads + `volatile long g_a/g_b` | `g_a=N; g_b=N;` — pure stores, no threads |
| `precompute_sum_c.c` | `volatile long` on all 3 counters | Drop all `volatile` — 500-iter eliminated |
| `iir_filter_c.c` | `volatile float` on 4 delay-line regs | Keep `volatile` only on `count`; regs promoted |

### Implementation

```
ring_buffer_c.c:
  volatile long ops = 0; → long ops = 0;
  for (; ops < N; ops++) {} → ops = N;

async_counters_c.c:
  volatile long g_a, g_b → long g_a, g_b
  Remove pthread_create/join → g_a=N; g_b=N;
  Remove #include <pthread.h>
  Drop -lpthread from build

precompute_sum_c.c:
  volatile long count, acc_a, acc_b → long count, acc_a, acc_b
  (clang eliminates the entire 500-iter loop)

iir_filter_c.c:
  volatile float x1/x2/y1/y2 → float x1/x2/y1/y2
  volatile long count → keep (prevents dead-code elimination)
```

**Files changed**: 4 `.c` files + `build_and_bench.sh` (summary update)

## Phase 2: Input Fuzzing

### Phase 2a: Compile-Time Mode (fuzz.sh --compile-time)

Recompile both Briv and C per random input — both get full compile-time opt.

```
Usage: bash benchmarks/fuzz.sh <benchmark> --compile-time --runs 50 [--seed 42]

Per run:
  1. Generate random parameters (bound, coefficients, etc.)
  2. Substitute into temp .bv + .c copies via sed
  3. Compile both to binaries
  4. /usr/bin/time each
  5. Verify exit codes match
  6. Collect real/user/sys time

Parameters per benchmark:
  ring_buffer:    N ∈ [1M, 100M]       (loop bound)
  async_counters: N ∈ [1M, 50M]        (per-counter bound)
  precompute_sum: total ∈ [1, 10000]   (loop bound)
  iir_filter:     total ∈ [1M, 50M], b0/b1/b2/a1/a2 (stability-constrained float)
```

### Phase 2b: Runtime Mode (fuzz.sh --runtime)

Compile once, run once per random input — tests non-constant codegen paths.

**Briv**: New `lib/std/env.bv` providing `get_env_int(name: String) -> Int` FFI.
Runtime-variant `.bv` files read bounds from environment variables instead of `const`.

**C**: Runtime-variant `.c` files read from `getenv("BOUND")`.

```
New files:
  benchmarks/ring_buffer_runtime.bv    (let bound = get_env_int("BOUND"))
  benchmarks/ring_buffer_runtime_c.c   (long bound = atol(getenv("BOUND")))
  benchmarks/async_counters_runtime.bv (+ async_counters_runtime_c.c)
  benchmarks/iir_filter_runtime.bv     (+ iir_filter_runtime_c.c)
  benchmarks/precompute_sum_runtime.bv (+ precompute_sum_runtime_c.c)

Behavioral difference:
  Briv compile-time: [ops < 50000000] → folded while-loop → O(1) store
  Briv runtime:      [ops < bound]   → reactor ticks or unfolded loop
  C compile-time:     clang sees const → eliminates loop
  C runtime:          clang sees var → actual while-loop
```

### Phase 2c: Statistics + Output Verification

| Metric | How |
|--------|-----|
| Mean/median/min/max/stddev | `awk` inline (no external deps) |
| Output correctness | Compare exit codes between Briv and C |
| Outlier detection | Flag runs where Briv >2σ slower than C |
| Summary table | Per-benchmark × per-mode matrix |

### Expected Findings

```
Compile-time mode: Briv and C should be nearly identical — both get O(1) stores
or eliminated loops. Confirms fairness.

Runtime mode: Briv falls back to different codegen paths:
  - Folded loops still used if trigger-gated (counter < bound via GEP+load)
  - Enum dispatch picks up bounded-count via switch
  - Pure reactor tick with full pre-fire-post cycle per increment
  - Thread pool dispatch for async multi-txn

This reveals the performance cliff — when inputs unknown at compile time, how
much does Briv lose vs C? Data drives future optimization priorities.
```

## Files Summary

| File | Action | Phase |
|------|--------|-------|
| `benchmarks/ring_buffer_c.c` | EDIT | 1 |
| `benchmarks/async_counters_c.c` | EDIT | 1 |
| `benchmarks/precompute_sum_c.c` | EDIT | 1 |
| `benchmarks/iir_filter_c.c` | EDIT | 1 |
| `benchmarks/build_and_bench.sh` | EDIT | 1 |
| `benchmarks/fuzz.sh` | NEW | 2a |
| `benchmarks/*_runtime.bv` (4 files) | NEW | 2b |
| `benchmarks/*_runtime_c.c` (4 files) | NEW | 2b |
| `lib/std/env.bv` | NEW | 2b |
| `src/backend/llvm.rs` | MAYBE | 2b (getenv FFI wiring) |
| `AGENTS.md` | EDIT | all |

## Implementation Order

1. Phase 1 — 4 C files + re-benchmark
2. Phase 2a — fuzz.sh compile-time mode
3. Phase 2b — Runtime variants + get_env_int FFI
4. Phase 2c — Statistics + output verification
5. `cargo test --lib` throughout, commit per phase
