# Progress Report: Optimization Framework & Pragmas Elimination

**Date:** 2026-05-31
**Commits:** `993a1d1`, `808c1c8`, `1472235`, `bf46fd2`
**Tests:** 347 pass, 0 fail

## Summary

Completed the entire optimization framework across 4 commits. All 5 optimization paths are implemented and tested. Three programmer-supplied pragmas have been eliminated — the compiler now auto-infers what it already proves.

## What Was Built

### Commit `993a1d1` — Optimization Completion (Phases A/B/C)

- **Phase A**: Wake-trigger/enum dispatch soundness fix. Wake reactors with enumerable triggers now take the switch-dispatch path with `@__rt_wait()` loop-back, instead of exiting after one tick.
- **Phase B**: Compile-time complete evaluation. When state space ≤ budget, all results are precomputed at compile time via `collect_final_values()` + `emit_precomputed_main()`. Runtime becomes a single `store` instruction (O(1)).
- **Phase C**: IIR filter benchmark regression test. Guards against optimization regressions on the canonical folded-path benchmark.

### Commits `808c1c8` + `1472235` + `bf46fd2` — Eliminate Redundant Pragmas (Steps 1-5)

- **Step 1**: Auto-select `Parallel` dispatch when all reactive txns are conflict-free. No `#pragma dispatch(parallel)` needed — the compiler already proves conflict freedom.
- **Step 2**: `@ link` triggers default to wake. No `#wake` pragma needed — all `@ link` triggers monitor volatile globals that change during sleep.
- **Step 3**: Lifted wake+enum mutual exclusion. Wake reactors with bounded trigger value sets now enter enum switch-dispatch with `@__rt_init()`/`@__rt_wait()` hybrid mode.
- **Step 4**: `suggest_async_promotion()` lint. Emits A001 warning for conflict-free `rct` txns that could be marked `async`.
- **Step 5**: Thread pool + auto async/enum inference.
  - Portable thread pool in `runtime/briev_rt.c` (mutex+cond+counter barrier, works on macOS)
  - Auto-categorization: enum candidates (trigger-gated) beat async candidates (conflict-free)
  - `emit_async_body` functions for worker threads
  - Hybrid `@main` with thread pool init, barrier phases, and `__rt_wait` integration
  - Link step detection of `@llvm.thread_pool` metadata

### Optimization Path Coverage

| Path | Description | Status |
|------|------------|--------|
| Path 1 | No optimization (standard reactor tick) | Baseline |
| Path 2 | Folded while-loop (counter convergence) | IIR filter — 1.53× faster than C |
| Path 3 | Compile-time precompute (state space ≤ budget) | Done — `emit_precomputed_main` |
| Path 4 | Enum switch-dispatch (bounded trigger values) | Done — wake+enum hybrid |
| Path 5 | Thread pool async dispatch (conflict-free txns) | Done — auto-inference |

### Key Design Properties

- **No atomics on state fields** — the proof engine guarantees disjoint field access per txn group. Plain loads/stores are data-race-free per C11 5.1.2.4p25. The barrier provides tick-to-tick happens-before.
- **Enum beats async** — when a txn qualifies for both, enum dispatch wins (O(1) folded loops vs per-tick precondition evaluation).
- **Pragmas gone** — `#pragma dispatch(parallel)`, `#wake`, and `async` marking are all handled by compiler auto-inference. The compiler already proves these properties; now it acts on them.

## Test Coverage

| Category | Count | Notes |
|----------|-------|-------|
| Total tests | 347 | All pass |
| Async dispatch | 4 new | `test_async_body_functions_emitted`, `test_thread_pool_metadata_emitted`, `test_async_barrier_calls_in_main`, `test_no_thread_pool_without_async_txns` |
| Enum+wake hybrid | 1 new | `test_enum_with_wake_triggers_hybrid` |
| Precompute | 2 new | `test_precompute_pure_counter`, `test_precompute_budget_exceeded_fallback` |
| IIR regression | 1 new | `test_iir_filter_folded_path_regression` |

## Next Steps

- 3 new benchmarks to validate each optimization path against C equivalents
- Benchmark infrastructure: monotonic clock FFI, `--link-rt` support in build script
