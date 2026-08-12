# Report: C Barrier Bug + Lightweight Async Optimization

**Date:** 2026-06-02
**Status:** Pending Implementation

## The Finding

Runtime input fuzzing exposed a latent bug in Briev's thread pool barrier and an
incidental optimization opportunity. Both stem from the same root cause: **the
thread pool path was never actually exercised at runtime.**

### Bug: Portable Barrier Spurious Wakeup Crash

The compile-time async benchmarks (e.g., `async_counters.bv` with `const N`)
all hit the **pure-counter O(1) store path** in the LLVM backend, completely
bypassing thread creation, barrier synchronization, and worker dispatch.

The actual thread pool barrier in `runtime/briev_rt.c` is a custom portable
implementation for macOS (which lacks `pthread_barrier_t` pre-10.12). It uses
`pthread_mutex` + `pthread_cond` directly:

```c
// CUSTOM barrier — broken at scale
static void briev_barrier_wait_impl(briev_barrier_t *b) {
    pthread_mutex_lock(&b->mutex);
    b->count++;
    if (b->count >= b->target) {
        b->count = 0;
        pthread_cond_broadcast(&b->cond);
    } else {
        pthread_cond_wait(&b->cond, &b->mutex);  // spurious wakeup not handled
    }
    pthread_mutex_unlock(&b->mutex);
}
```

**Problem:** No `while` loop around `pthread_cond_wait`. Spurious wakeups cause
`count` to drift upward across multiple ticks. Eventually `count >= target`
fires with the wrong number of participants, threads exit the barrier in the
wrong phase, and the next `pthread_mutex_lock` fires glibc's deadlock assertion:

```
Fatal glibc error: pthread_mutex_lock.c:94 (___pthread_mutex_lock):
    assertion failed: mutex->__data.__owner == 0
```

This is glibc's protection against recursive locking on a non-recursive mutex.
The thread that enters the next tick's barrier still holds the mutex from the
previous tick's corrupted exit.

**This is fundamentally a C runtime bug, not a compiler bug.** Briev's LLVM IR
is correct — the generated `call void @briev_barrier_release()` and
`call void @briev_barrier_wait()` are exactly right. The C implementation of
those functions is wrong.

### Optimization: Lightweight Async Dispatch

The barrier bug exposed a larger design question: **should async dispatch with
lightweight bodies even use threads and barriers?**

For the runtime-variable bound benchmarks:
- The txn body is pure (just `a = a + 1`)
- The bound is runtime-variable (can't fold to O(1) store)
- The body is a single `add` + `store` instruction (~2 ns)
- The barrier costs ~1 µs per wait
- Thread pool init costs ~50 µs
- **Threading overhead dwarfs the actual work by 500×**

Current dispatch for `async` txns with runtime bounds:
```
main:
  init_state()
  briev_thread_pool_init(2, ...)   // 50µs overhead
  tick:
    barrier_release()               // 1µs barrier wait
    reactor_tick()                  // 2ns of actual work
    barrier_wait()                  // 1µs barrier wait
    exit_check → tick or done
```

With 50M iterations: ~50 seconds in barrier overhead, ~0.1s in actual work.

Proposed dispatch for "lightweight async" txns:
```
main:
  init_state()
  // No thread pool, no barriers
  while (exit_condition) {
    reactor_tick()                  // 2ns of actual work
  }
```

With 50M iterations: ~0.1s total — same as the sequential C equivalent.

## Lightweight Async Classification

A txn is "lightweight async" when ALL of these hold:

| Condition | Check | Source |
|-----------|-------|--------|
| `async` keyword or promoted to async | `t.is_async` | Analyzer |
| Body is effectively pure | `node.is_effectively_pure` | `transition_graph.rs` |
| Has bounded convergence | `node.bounded_pre.is_some()` | Graph analysis |
| Has counter increment | `node.increments.is_some()` | Graph analysis |
| Bound is runtime-variable | `field_initializers.get(bound_var) ∉ Expr::Integer` | LLVM backend |

When all conditions are met, the compiler:
1. Skips `briev_thread_pool_init()` in `emit_main()` / `emit_enum_main()`
2. Skips `briev_barrier_release()` / `briev_barrier_wait()` in the main loop
3. Skips `@llvm.thread_pool` + `@thread_pool_fns` metadata
4. Uses the existing sequential `reactor_tick()` dispatch

The existing sequential path already handles multi-txn programs correctly —
`emit_reactor()` checks each txn's precondition and fires bodies in sequence.

## The Irony

Briev's compiler is so good at optimization that it never exercised its own
runtime thread pool. The pure-counter fold eliminates the loop entirely, so the
barrier code lay dormant — compiled, linked, but never called. Only runtime-
variable bounds (which defeat constant folding) revealed the C bug.

The fix is in two layers:
1. **Immediate — Lightweight async classification (Approach B):** Skip thread
   pool when it doesn't help. No C runtime changes needed. Fix in LLVM backend
   only (~15 lines).
2. **Deferred — Barrier fix (Approach A):** Use `pthread_barrier_t` on Linux,
   fix the custom barrier on macOS with generation-based spurious wakeup
   handling. Needed for heavyweight async work where threading is beneficial.

## Files Changed

| File | Change | Purpose |
|------|--------|---------|
| `src/backend/llvm.rs` | +~15 | Lightweight async classification + dispatch gating |
| `src/parser.rs` | ~10 | Accept plain return types for `frgn` (e.g., `-> Int`) |
| `runtime/briev_rt.c` | ~20 | `__get_env_int()` function + barrier generation fix |
| `lib/std/env.bv` | NEW | `frgn __get_env_int(name: String) -> Int` |
| `benchmarks/*_runtime.bv` | 4 NEW | Runtime-variant Briev benchmarks |
| `benchmarks/*_runtime_c.c` | 4 NEW | Runtime-variant C reference benchmarks |
| `benchmarks/fuzz.sh` | NEW | Fuzzing runner script |
| `benchmarks/build_and_bench.sh` | EDIT | `--fuzz N` flag support |
| `plans/2026-06-01-fair-c-benchmarks-fuzzing.md` | PREV | Phase 1 (fair C) + Phase 2 plan |
| `plans/2026-06-01-dead-field-elimination.md` | PREV | Dead-field elimination plan |
| `plans/2026-06-01-runtime-input-fuzzing.md` | PREV | Runtime fuzzing plan |
