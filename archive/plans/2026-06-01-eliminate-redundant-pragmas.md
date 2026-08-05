# Plan: Eliminate Redundant Pragmas via Auto-Inference

**Date:** 2026-06-01
**Status:** Steps 1-4 Complete, Step 5 Future

## Motivation

The Briv compiler's analysis pipeline proves everything that the following programmer-supplied pragmas/annotations currently require:

| Pragma/Keyword | What the compiler already proves |
|---|---|
| `#pragma dispatch(parallel)` | `check_mutual_exclusion` proves no write conflicts; `build_write_masks` computes per-txn write masks |
| `#wake` on `@ link` triggers | All `@ link` triggers are volatile globals that change during sleep — wake is the natural default |
| `async` keyword | `find_read_write_conflicts` proves disjoint access; `preconditions_overlap` checks safety |

The pragmas exist because the backend doesn't consume its own analysis results. The compiler knows more about the program than the programmer does about conflict analysis — making the programmer declare what's already proven is noise that obscures real issues.

## Background: The SPSC Ring Buffer Diagnostic

A proposed SPSC ring buffer benchmark revealed three implementation gaps:

1. **`async` is a compile-time annotation that does nothing at runtime.** The proof engine proves two transactions can run concurrently, then the LLVM backend fires them sequentially. `is_async` has zero codegen impact.

2. **Enum dispatch kills wake reactors.** A program with `@ link` triggers and bounded convergence transactions silently degrades from O(1) switch dispatch to the slow polling reactor loop because `has_wake_triggers` gates `enumerable` to `None` in the decision cascade (llvm.rs:447-453). Wake reactors are forced into the hot-polling standard reactor even when trigger value sets are fully known.

3. **Parallel dispatch fires sequentially in one thread.** Write-mask analysis correctly identifies non-conflicting transactions, but `emit_parallel_reactor` fires them one after another in the same single-threaded tick. No threads, no barriers, no actual concurrency.

## Implementation Plan

### Step 1: Auto-select `DispatchMode::Parallel` when conflict-free

**Root cause:** `process_dispatch_attribute` (parser.rs:571) defaults to `Sequential`. The `#pragma dispatch(parallel)` gate is the only way to enable `Parallel`, even when the compiler can prove all txns are conflict-free.

**Fix:**
1. Move dispatch mode selection into the LLVM backend's `generate()` method, after the proof engine has run.
2. Add `all_conflict_free(program)` — a boolean query built from the same logic as `check_mutual_exclusion` (proof_engine.rs:2538) that returns `true` when zero reactive txn pairs have read/write conflicts.
3. If all are conflict-free, auto-set `dispatch_mode = Parallel`.
4. Keep `#pragma dispatch(sequential)` as explicit override (some programs need deterministic ordering).
5. Remove the `#pragma dispatch(parallel)` pragma handling.

**Files:** `src/parser.rs:570-582`, `src/backend/llvm.rs:526-612`, `src/proof_engine.rs` (new method)

**Estimated:** ~50 lines

### Step 2: Make `@ link` triggers default to wake

**Root cause:** `parse_trigger` (parser.rs:2494-2504) only sets `is_wake = true` when the `#wake` hashtag is present. Without it, `@ link` programs hot-poll at 100% CPU.

**Fix:**
1. In `parse_trigger()`, set `td.is_wake = true` for all `@ link` triggers by default.
2. Remove the `#wake` hashtag handling (deprecate with a soft warning if still used).
3. Keep `#nowake` as an explicit opt-out for the rare busy-polling case (MMIO triggers remain wake-capable by nature of being MMIO).
4. Update the `#io` declaration parser — it already sets `is_wake = true`, so no change needed there.

**Files:** `src/parser.rs:2494-2525`

**Estimated:** ~20 lines

### Step 3: Lift wake+enum mutual exclusion

**Root cause:** `emit_enum_main` (llvm.rs:1904-2011) emits `ret i32 0` in every switch arm — the program exits after one tick. Wake reactors need an infinite loop with `@__rt_wait()` between cycles. The gate at `has_wake_triggers` → `enumerable = None` (llvm.rs:447-453) prevents enum dispatch for wake programs.

**Fix:**
1. Remove the `&& !has_wake_triggers` gate from the `enumerable` computation (llvm.rs:453).
2. Add a `has_wake: bool` parameter to `emit_enum_main`.
3. When `has_wake` is true:
   - Use `#3` attribute (no `willreturn`, no `mustprogress`) instead of `#0`.
   - Call `@__rt_init()` once at entry.
   - Replace every `ret i32 0` with `br label %tick` (loop back to trigger re-sample + switch dispatch).
   - After each arm completes (or the residual reactor tick), call `@__rt_wait()` then loop back.
4. When `has_wake` is false: preserve current one-tick behavior.
5. The residual path already calls `reactor_tick()` — extend it to also call `@__rt_wait()` between cycles when wake is active.

**Structure of the hybrid `@main`:**
```
define i32 @main() #3 {
entry:
  call void @init_state()
  call void @__rt_init()          ; NEW: one-time setup
  br label %tick
tick:
  ; Sample triggers (volatile loads) — RE-SAMPLED each cycle
  %sz_btn = load volatile i8, i8* @__btn
  switch i8 %sz_btn, label %reactor_dispatch [
    i8 0, label %case_0
    i8 1, label %case_1
  ]
case_0:
  ; Folded while-loop for trigger=0
  call void @emit_folded_loop(...)
  br label %do_wait             ; WAS: ret i32 0
case_1:
  call void @emit_folded_loop(...)
  br label %do_wait             ; WAS: ret i32 0
reactor_dispatch:
  call void @reactor_tick()
  br label %do_wait
do_wait:
  call void @__rt_wait()        ; NEW: block until next event
  br label %tick                 ; NEW: re-sample triggers
}
```

**Files:** `src/backend/llvm.rs:447-453` (gate removal), `src/backend/llvm.rs:571-581` (call site), `src/backend/llvm.rs:1904-2011` (function restructure)

**Estimated:** ~100 lines

### Step 4: Auto-promote conflict-free transactions to `async`

**Root cause:** `check_mutual_exclusion` (proof_engine.rs:2538) only fires when txns already have `is_async = true`. Non-async txns that are perfectly conflict-free are never examined. This is a one-direction check — "if async, verify safety" — instead of a two-direction inference — "if safe, suggest async."

**Fix:**
1. Add `suggest_async_promotion()` to the proof engine: iterate ALL `rct` txn pairs (not just `rct async`), run `find_read_write_conflicts`. For any pair with zero conflicts, emit a lint: "transactions X and Y are conflict-free; consider adding 'async' for concurrent dispatch."
2. Phase 2 (future): auto-promote with a compiler flag `--auto-async`.

**Note:** Step 4 gives us the lint/proof plumbing. Actual concurrent thread dispatch is Step 5.

**Files:** `src/proof_engine.rs` (new method)

**Estimated:** ~40 lines

### Step 5: True multi-threaded async dispatch (future)

**Prerequisites:** Steps 1-4 complete. This is where `async` generates actual concurrent code.

**Fix:**
1. In `emit_main`, detect async txn groups (conflict-free sets). Emit per-group thread body functions.
2. Emit `briv_spawn_threads()` / `briv_barrier_wait()` calls in `@main` via the C runtime bridge.
3. Add `briv_thread_entry(void(*body)(void))` and `briv_barrier_wait()` to `runtime/briv_rt.c` using `pthread_create`/`pthread_join`.
4. State struct remains shared — disjoint GEP offsets guarantee no data races (C11 5.1.2.4p25).
5. Add `-lpthread` to the link step.

**No atomics are needed** in the minimal implementation because the proof engine guarantees disjoint field access. The only synchronization is the tick-start barrier (trigger snapshot load) and tick-end barrier (`pthread_join`).

**Files:** `src/backend/llvm.rs` (~200 lines), `runtime/briv_rt.c` (~50 lines), `src/backend/mod.rs` (builtins list)

## Validation: The SPSC Ring Buffer Benchmark

| Step | What the benchmark tests | Expected behavior |
|------|------------------------|-------------------|
| 1+2+3 | `trg cmd: Bool @ link __cmd` (auto-wake, 2-value enum) with bounded push/pop txns | 2-case switch dispatch + `@__rt_wait()` between cycles. No `#pragma`, no `#wake` needed. |
| 4 | Two `rct` txns (push/pop) auto-detected as async | Lint fires: "txns are conflict-free; consider adding async" |
| 5 | Push and pop firing on separate threads | True SPSC throughput vs C `pthread` equivalent |

## Implementation Order

1. **Step 2** — `@ link` default to wake (smallest, unblocks Step 3)
2. **Step 3** — Lift wake+enum exclusion (enables enum+wake hybrid, the main optimization gap)
3. **Step 1** — Auto-parallel dispatch (eliminates the most redundant pragma)
4. **Step 4** — Async suggestion lint (prepares for Step 5)
5. **Step 5** — True concurrent dispatch (the big feature)

## Testing Strategy

- Each step: add integration tests in `src/backend/llvm.rs` (following existing `test_enum_*` patterns)
- Step 2: `test_link_trigger_defaults_to_wake` — verify `is_wake = true` without `#wake`
- Step 3: `test_enum_with_wake_triggers` — verify switch dispatch + `@__rt_wait()` hybrid output
- Step 1: `test_auto_parallel_when_conflict_free` — verify `Parallel` selected without pragma
- Step 4: `test_suggests_async_for_conflict_free_txns` — verify lint output
- Full pipeline: `cargo test --lib` (currently 334 tests pass)
