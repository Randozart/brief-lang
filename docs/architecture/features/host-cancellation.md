# Host Cancellation — cancel a long-running Briv call

**Date:** 2026-08-03
**Status:** Implemented (explicit polling; process-global flag)
**Request origin:** RamKumar Revanur — "we need a cancellation token if it gets
stuck so we can cancel the request as it is taking too long."

## What was built

A host thread raises a process-global atomic flag; a long-running Briv loop
polls it explicitly and stops early with a partial result.

```briv
txn sum_loop(acc: Int, i: Int, count: Int)
    [i < count && !CancelRequested#()][i == count] -> Int
{
    let na: Int = acc + (i * 3);
    acc = na;
    i = i + 1;
    term acc;
};

export defn cancellable_sum(count: Int) -> Int {
    term sum_loop(0, 0, count);
};
```

Host side (C):
```c
__briv_set_cancel(st, 1);   // from another thread
int64_t r = cancellable_sum(st, 2000000000);  // returns early
__briv_clear_cancel(st);
```

## Pieces

1. **Intrinsics:** `CancelRequested#() -> Bool` loads `@__briv_cancel_flag`
   (seq_cst); `ClearCancel#()` stores 0. Interpreter returns `false` (no
   host in-process).
2. **Shim exports:** `__briv_set_cancel(ptr %state, i32)` /
   `__briv_clear_cancel(ptr %state)` in `emit_library_shim`; declared in the
   C bindings header.
3. **Explicit polling only** (rule 2): the loop's precondition is
   `[i < count && !CancelRequested#()]` — the compiler never injects checks.
   Composes with the existing watchdog `?[c] within N ms` deadline.
4. **Demo:** `examples/glue-host/cancel.bv`.
5. **Test:** `tests/c_driver_cancel.rs` (toolchain-guarded) — a pthread raises
   the flag after 20ms; a 2e9-count run stops far before the 50M full run.

## Design notes

- **Process-global flag** (`@__briv_cancel_flag`): one Briv instance per
  process. The `ptr %state` parameter is accepted for ABI stability but
  unused. Per-state flags (independent concurrent instances) would move the
  flag into the `%State` layout — deferred.
- **Partial results are correct by construction:** the txn's committed state
  at cancellation is returned; contracts still hold (the post may not, but
  the pre became false — the loop's normal exit semantics).
- The `.ll` warning "emitted runtime loop has no observable side effects"
  is expected for library builds — the exported result IS observable to the
  host caller.

## Undo

- Remove the `CancelRequested#`/`ClearCancel#` signatures + intrinsics arms,
  `@__briv_cancel_flag` global, and the shim `__briv_set_cancel`/
  `__briv_clear_cancel` functions.
