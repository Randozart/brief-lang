# Plan: Thread Pool + Automatic Async/Enum Inference

**Date:** 2026-06-01
**Status:** In Progress

## Motivation

Steps 1-4 eliminated redundant pragmas by having the compiler consume its own analysis results. Step 5 makes `async` actually concurrent by implementing a thread pool in the runtime and automatic txn categorization in the backend. No programmer annotation needed — the compiler already proves conflict freedom, knows trigger value sets, and can auto-categorize every txn into enum, async, or sequential dispatch.

## Thread Pool Design

**Model**: Workers spawned once at init, synchronized via barrier each tick. No per-tick `pthread_create`/`pthread_join` overhead.

```
__rt_init():
  set up signal handlers, timers  (existing)
  spawn N worker threads          (NEW)
  each worker: while(!shutdown) { barrier_wait; fire_body; barrier_release; }

main loop:
  sample triggers
  enum dispatch (main thread, for sync trigger-gated txns)
  barrier_release → workers run async bodies
  sequential phase (main thread, any remaining sync txns)
  barrier_wait → all workers done  (opposing barrier direction)
  __rt_wait()
  br tick
```

### Platform Plan

| Platform | Barrier mechanism | Threads? |
|----------|------------------|----------|
| Linux, FreeBSD, NetBSD | `pthread_barrier_t` | Yes |
| macOS | `pthread_mutex_t` + `pthread_cond_t` + counter (Apple omits POSIX barriers) | Yes |
| WASM/Emscripten | `pthread_barrier_t` with `-s USE_PTHREADS=1` | Yes |
| Bare metal ARM/x86 | N/A | No — no OS scheduler |

Gated behind `#if defined(BRIV_THREAD_POOL)` in `briv_rt.c`, activated by `@llvm.thread_pool` metadata in generated IR.

## Compiler Auto-Inference

The compiler already has all the facts. No programmer annotation needed:

```
For each txn:
  ├─ Is trigger-gated with known value set? → enum candidate
  ├─ Is conflict-free with ALL other async-candidates? → async candidate
  ├─ BOTH? → pick enum (more optimized, O(1) folded loops)
  └─ NEITHER? → sequential fallback
```

The decision is made in `generate()` before code emission. The split is automatic:

- `enum_txns`: trigger-gated, bounded value sets, produces switch dispatch
- `async_txns`: conflict-free pairwise, runs in worker threads
- `sequential_txns`: everything else, runs after enum+before barrier

## Codegen Structure

```llvm
define i32 @main() #3 {
entry:
  call void @init_state()
  call void @__rt_init()          ; spawns workers internally
  br label %tick

tick:
  ; Phase 1: Sample triggers once
  %sz_trg = load volatile i8, i8* @__trigger

  ; Phase 2: Enum dispatch (sync txns only, main thread)
  switch i8 %sz_trg, label %async_phase [
    i8 0, label %case_0
    i8 1, label %case_1
  ]
case_0:  ;; folded loop / pure counter for sync txn at value 0
  br label %async_phase
case_1:  ;; folded loop / pure counter for sync txn at value 1
  br label %async_phase

async_phase:
  ; Phase 3: Release workers (async bodies run in parallel)
  call void @briv_barrier_release()

  ; Phase 3b (concurrent): main does sequential non-enum txns
  call void @reactor_tick_seq()

  ; Phase 4: Wait for all workers
  call void @briv_barrier_wait()

  ; Phase 5: Wait for next event
  call void @__rt_wait()
  br label %tick
}
```

Worker thread bodies:
```llvm
define void @async_body_txn_a(%State* %state) #4 {
  %pr = call i1 @pre_txn_a(%State* %state)
  br i1 %pr, label %fire, label %done
fire:
  call void @txn_a(%State* %state)
  br label %done
done:
  ret void
}
```

## Files to Change

| File | Lines | Change |
|------|-------|--------|
| `runtime/briv_rt.c` | ~80 | `briv_thread_pool_init(N, fn_ptrs)`, `briv_barrier_release()`, `briv_barrier_wait()`, `briv_thread_pool_shutdown()`. Platform-specific barrier under `#if`. |
| `src/backend/llvm.rs` | ~50 | `emit_async_body` — per-txn worker function |
| `src/backend/llvm.rs` | ~100 | Auto-split txns into enum/async/seq groups in `generate()`; emit hybrid `@main` |
| `src/backend/llvm.rs` | ~30 | `emit_enum_main` extended for async phase after switch arms |
| `src/backend/mod.rs` | ~10 | Builtins: `briv_thread_pool_init`, `briv_barrier_release`, `briv_barrier_wait` |
| `src/main.rs` | ~10 | Detect `@llvm.thread_pool` metadata, add `-DBRIV_THREAD_POOL -lpthread` to link step |
| `src/proof_engine.rs` | 0 | Already done — `suggest_async_promotion` from Step 4 |

## Key Design Decisions

### Why enum beats async when a txn qualifies for both?
Enum dispatch produces O(1) folded while-loops with no precondition checking in the hot path. Async dispatch still evaluates preconditions each tick. For a trigger-gated txn with a known value set, enum is strictly better.

### Why two opposing barriers?
The thread pool initial barrier pattern is: `worker: wait → fire → release → main: release → work → wait`. This is essentially a tick-level rendezvous. Workers block on `barrier_enter` until the main thread calls `briv_barrier_release()`. After firing, workers block on `barrier_exit` until the main thread calls `briv_barrier_wait()`. Two barriers prevent the main thread from advancing to the next tick before all workers complete.

### No atomics on state fields
The proof engine guarantees:
- Async txn A writes only to field indices {2, 5}
- Async txn B writes only to field indices {1, 3}
- No overlap → no data races per C11 5.1.2.4p25 (distinct memory locations)
- The barrier provides the happens-before edge for tick-to-tick visibility
- No `atomicrmw`, no fences, no `seq_cst` — just plain loads and stores

### No locking, no CAS
The proof engine's compile-time guarantees eliminate the need for runtime synchronization on state. This is the key advantage over C/C++ implementations that must use atomics or locks because the compiler can't prove disjoint access.

## Downstream Implications

- `resolve_fusable_pairs` already excludes async txns (line 2078) — correct, keep this
- `build_write_masks` — async txns don't participate in write-mask tracking because they run in isolation. The proof engine guarantees disjoint sets.
- Interaction with `--optimize-budget` and `--optimize-report` — the report should show which txns went to which phase
- The `@llvm.wake_triggers` metadata and `@llvm.thread_pool` metadata coexist — a program can have both
- `-lpthread` is already conditionally linked when wake triggers exist; extend to also link when thread pool metadata exists

## Test Plan

1. Two `rct async` txns, disjoint writes → verify `@async_body_*` functions emitted, `briv_barrier_*` calls in main
2. Two `rct async` + one `rct` sync (non-enum) → verify sequential phase after async
3. Two `rct async` + trigger-gated sync (enum candidate) → verify enum dispatch before async
4. End-to-end: compile `.bv` → verify `pthread_create`/barrier in native binary via `nm`
