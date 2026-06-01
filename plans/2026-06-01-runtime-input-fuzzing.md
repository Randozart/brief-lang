# Plan: Runtime Input Fuzzing (Phase 2b)

**Date:** 2026-06-01
**Status:** In Progress

## Motivation

All benchmarks currently use compile-time constants for loop bounds and inputs.
Brief exploits this to fold loops into O(1) stores, proving counter convergence
at compile time. This is legitimate optimization but leaves a blind spot: **how
does Brief perform when inputs are genuinely runtime-variable?**

With runtime-variable inputs:
- Brief cannot fold loops (no constant bound to store)
- Brief cannot precompute final values (no compile-time state space)
- Brief falls through to its runtime dispatch: reactor ticks, pre/fire/post, 
  trigger-gated enum/async paths
- C loses clang's constant propagation too — fair comparison

This tests the "prediction cliff" — how much does performance degrade when the
compiler can't know the inputs ahead of time?

## Mechanism: Trigger + C Global + Constructor

Avoids modifying `emit_init_state()` (which silently drops complex expressions).
Instead, uses the existing trigger (`@ link`) mechanism:

1. `runtime/brief_rt.c` adds `volatile int64_t __runtime_bound` — a C global
2. Constructor function `__brief_read_env_bound()` reads `BOUND` from environment
   before `main()` via `__attribute__((constructor))`
3. `lib/std/env.bv` declares `trg runtime_bound: Int @ link __runtime_bound;`
4. Runtime `.bv` benchmarks `import { runtime_bound } from "std/env.bv"` and use
   `runtime_bound` in place of `const N: Int = <value>`

Because `runtime_bound` is a trigger (volatile external global, not a state field
or compile-time constant), the compiler's analysis is agnostic:
- `extract_bounded_pre` finds `[ops < runtime_bound]` — but `runtime_bound` is 
  not in `field_index_map` or `constants`, so `bounded_pre` is recognized but
  the LLVM backend cannot fold it (no index for bound_var)
- Program falls through to: trigger-gated reactor path
- Dead-field elimination classifies the txn as NOT effectively pure (counter is
  not in `field_index_map` — wait, the counter IS; but the bound isn't)
- Result: standard reactor tick + precheck + body + postcheck per iteration

## Implementation Steps

### Step 1: `runtime/brief_rt.c` — Add environment global + constructor

```c
#include <stdlib.h>     // for getenv, strtol

volatile int64_t __runtime_bound = 0;

__attribute__((constructor)) static void __brief_read_env_bound(void) {
    const char* val = getenv("BOUND");
    if (val) {
        char* end = NULL;
        long v = strtol(val, &end, 10);
        if (end != val) __runtime_bound = (int64_t)v;
    }
}
```

### Step 2: `lib/std/env.bv` — Trigger declaration

```brief
trg runtime_bound: Int @ link __runtime_bound;
```

### Step 3: Runtime-variant `.bv` benchmarks (4 new files)

Each replaces `const N: Int = <value>` with:
```
import { runtime_bound } from "std/env.bv";
```

And uses `runtime_bound` in preconditions and exit conditions.

Files:
- `benchmarks/ring_buffer_runtime.bv`
- `benchmarks/async_counters_runtime.bv`
- `benchmarks/precompute_sum_runtime.bv`
- `benchmarks/iir_filter_runtime.bv`

### Step 4: Runtime-variant `.c` benchmarks (4 new files)

Each replaces `long bound = <value>` with:
```c
long bound = atol(getenv("BOUND") ? : "50000000");
```

Files:
- `benchmarks/ring_buffer_runtime_c.c`
- `benchmarks/async_counters_runtime_c.c`
- `benchmarks/precompute_sum_runtime_c.c`
- `benchmarks/iir_filter_runtime_c.c`

### Step 5: `benchmarks/fuzz.sh` — Fuzzing runner script

Two modes:

```
Usage: bash benchmarks/fuzz.sh --mode <compile-time|runtime> --runs N [--seed S]

Mode A — compile-time (--mode compile-time):
  For each run:
    1. Generate random bounds (BOUND ∈ [1M, 100M])
    2. Substitute into template .bv + .c
    3. Compile both to temp binaries
    4. /usr/bin/time each
    5. Verify exit codes match
    6. Collect real time

Mode B — runtime (--mode runtime):
  1. Compile runtime-variant .bv + .c once
  2. For each run:
     a. Export BOUND=<random>
     b. /usr/bin/time ./benchmark_runtime [brief]
     c. /usr/bin/time ./benchmark_runtime_c [C]
     d. Store both times
  3. Compute statistics

Output:
  benchmark: ring_buffer (runtime, n=50)
    brief: mean=0.045s median=0.044s min=0.042s max=0.052s σ=0.0023s
    c:     mean=0.042s median=0.041s min=0.040s max=0.048s σ=0.0018s
    brief/c ratio: 1.07×  (Brief is 7% slower)
    correctness: 50/50 exit codes match
```

### Step 6: `benchmarks/build_and_bench.sh` — Add `--fuzz N` flag

New flag: `bash benchmarks/build_and_bench.sh --fuzz 50` runs both fuzz modes.

### Step 7: Cross-platform note

`__attribute__((constructor))` is GCC/Clang. Not available on MSVC — but our
runtime targets Linux/macOS/ARM via Clang/GCC anyway.

## Files Summary

| File | Action | Lines |
|------|--------|-------|
| `runtime/brief_rt.c` | EDIT | +5 (global) +12 (constructor) |
| `lib/std/env.bv` | NEW | 2 |
| `benchmarks/ring_buffer_runtime.bv` | NEW | ~25 |
| `benchmarks/async_counters_runtime.bv` | NEW | ~30 |
| `benchmarks/precompute_sum_runtime.bv` | NEW | ~30 |
| `benchmarks/iir_filter_runtime.bv` | NEW | ~65 |
| `benchmarks/ring_buffer_runtime_c.c` | NEW | ~20 |
| `benchmarks/async_counters_runtime_c.c` | NEW | ~35 |
| `benchmarks/precompute_sum_runtime_c.c` | NEW | ~25 |
| `benchmarks/iir_filter_runtime_c.c` | NEW | ~50 |
| `benchmarks/fuzz.sh` | NEW | ~150 |
| `benchmarks/build_and_bench.sh` | EDIT | +10 |

## Expected Findings

| Benchmark | Compile-time (both) | Runtime (neither) | Gap analysis |
|-----------|--------------------|--------------------|--------------|
| iir_filter | Field + Brief: 0.00s C: 0.10s | Both: actual loop | Brief's runtime dispatch overhead |
| ring_buffer | Both: 0.00s | Both: actual loop | Brief's trigger/reactor tick overhead |
| async_counters | Both: 0.00s | Both: actual loop | Thread pool + dispatch overhead |
| precompute_sum | Both: 0.00s | Both: actual loop | Brief's chain execution overhead |
