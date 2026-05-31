# Benchmarks Plan

**Date:** 2026-05-31
**Status:** Planned

## Overview

Add 3 new benchmarks to validate every optimization path against C equivalents, plus regression guard on the existing IIR filter. Each benchmark targets a specific path in the compilation pipeline.

## Benchmark 1: Enum-Driven Ring Buffer (Path 4 + Wake Hybrid)

**Targets:** Enum switch-dispatch, wake+enum hybrid, auto-wake, `@ link` triggers

**Program structure:**
- `trg cmd: Bool @ link __cmd` — selects push (0) vs pop (1)
- State: `head: Int`, `tail: Int`, `count: Int`
- Constants: `CAP: Int = 1024`, `N: Int = 50_000_000`
- `rct txn push [cmd == 0][count == @count + 1]` — bounded convergence push loop
- `rct txn pop [cmd == 1 && count > 0][count == @count - 1]` — bounded convergence pop loop
- Init txn: captures `t0` via monotonic clock
- Report txn: prints elapsed ns after N operations

**Expected LLVM output:**
- 2-case `switch i8 %sz_cmd [i8 0, %case_0], [i8 1, %case_1]`
- Each arm: folded `while (count < N)` loop
- `do_wait:` label with `@__rt_wait()` → `br tick` (wake hybrid)
- No `@reactor_tick` in hot path (enum dispatch wins)

**C comparison:** `switch(cmd) { case 0: while(pushes < N) push(); break; case 1: while(pops < N) pop(); break; }`

**Requires:** `--link-rt` (for `@link` trigger + wake reactor)

---

## Benchmark 2: Async Parallel Accumulators (Path 5 — Thread Pool)

**Targets:** Thread pool dispatch, auto-async categorization, dual-barrier synchronization

**Program structure:**
- Two `rct async txn` with disjoint writes to `a: Int` and `b: Int`
- Bounded convergence: `[a < N][a == N]` and `[b < N][b == N]`
- Both fire concurrently via thread pool
- Init/report transactions for timing

**Expected LLVM output:**
- `@async_body_inc_a` and `@async_body_inc_b` worker functions
- `@llvm.thread_pool` metadata
- `@main`: `brief_thread_pool_init → loop(tick: barrier_release → reactor_tick → barrier_wait → __rt_wait → br tick)`
- Zero annotation needed — compiler auto-categorizes both as async

**C comparison:** Two `pthread_create` threads each running `for (i=0; i<N; i++) a++` / `for (i=0; i<N; i++) b++`

**Requires:** `--link-rt` (for thread pool runtime)

---

## Benchmark 3: Precomputed Sum (Path 3 — Compile-Time Evaluation)

**Targets:** Compile-time complete evaluation, zero-instruction hot path

**Program structure:**
- Single `rct txn sum [count < 1000][count == 1000]` computing `result = result + count`
- State space: {count: 0..1000, result: 0..499500} ≤ budget (256)
- Compiler evaluates all 1000 steps at compile time

**Expected LLVM output:**
- `@main`: `init_state() → store i64 499500, i64* %gp_result → ret i32 0`
- No `while` loops, no `switch`, no `@reactor_tick`
- O(1) runtime — single store instruction

**C comparison:** `for (i=0; i<1000; i++) sum += i` — O(N) at runtime, O(1) at compile time

**Requires:** No `--link-rt` (standalone binary)

---

## Benchmark 4: IIR Filter (Path 2 — Regression Guard)

**Targets:** Ensure folded while-loop path is not regressed by any of the above changes

**Status:** Already exists at `benchmarks/iir_filter.bv` + `benchmarks/iir_filter_c.c` + `benchmarks/build_and_bench.sh`

**Expected:** Maintain 0.15s vs C 0.23s (1.53× faster). Must pass `test_iir_filter_folded_path_regression`.

**Requires:** No `--link-rt`

---

## Infrastructure Needs

### Monotonic Clock FFI

Add to benchmarks (shared file `benchmarks/timing_bridge.c`):
```c
#include <time.h>
#include <stdint.h>

uint64_t __monotonic_ns(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (uint64_t)ts.tv_sec * 1000000000ULL + (uint64_t)ts.tv_nsec;
}

void __run_benchmark(void (*body)(void), uint64_t iters) {
    for (uint64_t i = 0; i < iters; i++) body();
}
```

Brief side:
```brief
frgn __monotonic_ns() -> Result<Int, TimeError>;
```

### build_and_bench.sh Extension

```bash
# New flags:
#   --link-rt     Compile with brief_rt.c runtime (for @link triggers + thread pool)
#   BENCH_DIR      Directory containing .bv and .c files
#   BENCH_NAME     Base name (e.g., "ring_buffer")

# For --link-rt benchmarks:
cargo run --bin brief-compiler -- llvm $BENCH_DIR/${BENCH_NAME}.bv --out $BENCH_DIR --link-rt
clang -O3 -march=native -o $BENCH_DIR/${BENCH_NAME} $BENCH_DIR/${BENCH_NAME}.o $BENCH_DIR/brief_rt.o -lpthread
```

### Timing Pattern (in-benchmark)

```brief
import io from "std/io.bv";

frgn __monotonic_ns() -> Result<Int, TimeError>;

let _t0: Int = 0;
let _t1: Int = 0;
let _started: Bool = false;

txn _init [_started == false][_started == true] {
    &_t0 = __monotonic_ns().unwrap();
    term;
};

// ... benchmark transactions ...

txn _report [_started == true && /* convergence condition */][true] {
    &_t1 = __monotonic_ns().unwrap();
    let elapsed: Int = _t1 - _t0;
    let msg: String = "Elapsed: " + String(elapsed) + " ns";
    io.println(msg);
    term;
};
```

## Files to Create

| File | Purpose |
|------|---------|
| `benchmarks/ring_buffer.bv` | Path 4 + wake hybrid benchmark |
| `benchmarks/ring_buffer_c.c` | C equivalent (switch dispatch) |
| `benchmarks/async_counters.bv` | Path 5 thread pool benchmark |
| `benchmarks/async_counters_c.c` | C equivalent (pthread) |
| `benchmarks/precompute_sum.bv` | Path 3 precompute benchmark |
| `benchmarks/precompute_sum_c.c` | C equivalent (for loop) |
| `benchmarks/timing_bridge.c` | Shared monotonic clock FFI |

## Expected Results (Target)

| Benchmark | Path | Brief expected | C expected | Ratio target |
|-----------|------|---------------|-----------|-------------|
| IIR filter | 2 | 0.15s | 0.23s | 1.53× faster |
| Ring buffer | 4 | TBD | TBD | ≥1.0× (parity or better) |
| Async counters | 5 | TBD | TBD | ~N× for N threads |
| Precompute sum | 3 | ~0.000s | ~0.001s | ~∞ (O(1) vs O(N)) |