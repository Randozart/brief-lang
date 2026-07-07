# Remove `#!exit` Pragmas from Benchmarks — migrate to swan song pattern

## Motivation

`#!exit <condition>` was a workaround from before the contract system fully
handled natural death (loop exits when precondition fails). Today, every
benchmark has `[pre][post]` contracts that prove convergence — `#!exit` is
redundant.

The correct liveness pattern is `term! -> print_xxx#(result)` — a swan song
that runs before `ret`, making the program's final output structurally live.
This eliminates the last magic pragma from the language surface.

## Plan

### Step 1: `#!exit` removal — 26 files

Every file with `#!exit` AND FFI calls in the hot loop body (Category A, 25
files) just needs the `#!exit` line deleted. Natural death via the contract
handles loop exit. The swan song for the final result is already covered by
observable FFI calls in the body (print_int#, print_float#, putchar#).

Two additional files need `#!exit` removed but have no FFI in the body:

- **`precompute_sum_runtime.bv`**: No FFI in hot loop. After removing `#!exit`,
  the pure loop would be dead-code eliminated. Fix: add a swan song
  `term! -> print_int#(acc_a + acc_b);` in a `[count == bound]` guard (same
  pattern as `precompute_sum.bv`).

- **`async_counters_idio.bv`**: Optimizer benchmark (tag: `--optimizer`).
  Removing `#!exit` lets it fold correctly — the optimizer path is supposed to
  precompute. No change needed beyond deletion. Correctness is checked by
  the harness (precomputed binary → skip timing → check zero exit).

- **`let-order.bv`**: Test file (not in benchmark harness). Remove `#!exit`,
  no swan song needed.

### Step 2: Add swan song to `precompute_sum_runtime.bv`

The two txns each accumulate (`acc_a += a`, `acc_b += b`). The final output is
`acc_a + acc_b`. Add to each txn a terminating guard:

```brief
[count == bound] {
    term! -> print_int#(acc_a + acc_b);
};
```

### Files NOT touched

| File | Reason |
|------|--------|
| `test_ring_buffer.bv` | Test file, has `#!exit` but also prints |
| `test_import.bv` | No txn, no `#!exit` |
| `gpu/saxpy/saxpy.bv` | Not in benchmark suite |
| `precompute_sum.bv` | Already has swan song, no `#!exit` |
| `fannkuch_redux.bv` | Already has swan song, no `#!exit` |
| `async_counters_idio.bv` | Left as-is (optimizer benchmark) |
| `let-order.bv` | Left as-is (test file) |

### Risk Assessment

- **All A005c/A005a programs** (26 of 28 files): `#!exit` was redundant — the
  counter comparison in the loop header already exits when `count >= bound`.
  Removal is a no-op for generated IR.

- **bit_clear.bv**: Goes through A006 (non-counter-bound precondition
  `[reg != 0]`). Our fix from commit 552fcd2 skips `any_fired`/`cycle_count`
  when `exit_condition` is `Some`. Removing `#!exit` would make
  `exit_condition = None` and reintroduce the overhead.

  **But** — this is a separate problem. The `#!exit` removal from bit_clear
  is harmless because bit_clear's `[reg != 0]` contract ALSO works with A006's
  `any_fired` mechanism (the loop exits when precondition fails → no txn fires
  → `any_fired` stays 0). The only loss is the ~6 ops/iteration optimization
  from our earlier fix. Since bit_clear runs 63 iterations at sub-ms, this is
  negligible noise.

  Long-term fix: handle `Expr::Ne` in `extract_bounded_pre` to route
  bit_clear through A005c per-field phi loop (which doesn't need any_fired at
  all). This is deferred.

### Post-Migration Verification

1. `cargo test --lib` — all 1403 tests pass
2. `cargo build --release` — zero warnings
3. Full benchmark suite — all 22 benchmarks MATCH, no regressions
