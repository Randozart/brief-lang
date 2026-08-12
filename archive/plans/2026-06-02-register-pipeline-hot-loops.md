# Plan: Register-Pipeline Hot Loops (Phi-Node Optimization)

**Date:** 2026-06-02
**Status:** Plan — ready for implementation

## Motivation

Briev's folded-path codegen currently emits a GEP → load → add → GEP → store
round-trip through `@global_state` on every loop iteration:

```
loop:
  %gp = gep @global_state, counter_idx    ; address arithmetic
  %lp = load i64, i64* %gp                ; load from memory
  %inc = add i64 %lp, 1                   ; increment
  store i64 %inc, i64* %gp                ; store to memory
  %cmp = icmp slt i64 %inc, %bound        ; compare
  br i1 %cmp, label %loop, label %done
```

This is unnecessary when the txn body is a pure counter increment (`&a = a + 1`).
The proof engine already proves:
- The txn only writes to the counter (`node.is_pure_body` or `node.is_effectively_pure`)
- The counter converges (`bounded_pre` + `increments`)
- No other txn reads or writes the counter concurrently

Therefore the counter can live in an SSA phi node (register) for the duration of
the loop, with a single load at entry and a single store at exit:

```
entry:
  %counter_ptr = gep @global_state, counter_idx
  %bound_ptr   = gep @global_state, bound_idx
  %init = load i64, i64* %counter_ptr       ; load once
  %bound_val = load i64, i64* %bound_ptr    ; load once (LICM)
  br label %loop
loop:
  %counter = phi i64 [%init, %entry], [%inc, %body]
  %cmp = icmp slt i64 %counter, %bound_val
  br i1 %cmp, label %body, label %done
body:
  %inc = add i64 %counter, 1
  br label %loop
done:
  store i64 %counter, i64* %counter_ptr    ; store once
  ret i32 0
```

The register allocator keeps `%counter` in a register. Zero memory traffic per
iteration. clang -O3 then sees a pure-increment loop with loop-invariant bound
and folds it to O(1) `store counter, bound_val`.

## Two Parts

### Part A — Single-txn register pipeline

Modify `emit_folded_loop` (called by both `emit_folded_main` and
`emit_case_folded_loops`) to accept a `use_phi: bool` parameter.

When `use_phi = true`:
- Emit two GEPs in `entry:` (counter + bound)
- One `load i64` per pointer
- `br label %hdr`
- `hdr:` phi node for counter
- `icmp slt` against the pre-loaded bound value
- `br i1` to body or done
- `body:` add 1, `br` back to hdr
- `done:` store counter, fall through

Passed from the folding decision at line 702-722: when
`node.is_pure_body || node.is_effectively_pure` is true AND `total_val` is None
(= runtime-variable bound), call with `use_phi = true`.

### Part B — Multi-txn register pipeline main

New function `emit_folded_multi_main(out, fold_params, ...)`.

Takes a list of `(counter_idx, total_idx, total_const_name)` tuples (one per
txn) and emits a single `@main()` that:

1. Loads all counters and the shared bound once
2. Allocates phi nodes for each counter
3. Single loop body that increments all counters
4. Stores all counters once after loop

Check added in `generate()` between existing checks:

```
1. Single-txn foldable check → emit_folded_main / emit_folded_pure_counter
2. Precompute check → emit_precomputed_main
3. Enum dispatch check → emit_enum_main
4. NEW: Multi-txn all-pure check → emit_folded_multi_main
5. Sequential/Parallel dispatch → emit_reactor + emit_main
```

Gate: `txns.len() > 1` AND all reactive txns have `bounded_pre + increments`
AND no triggers.

## Effect on Benchmarks

| Benchmark | Before | After |
|-----------|--------|-------|
| ring_buffer_runtime (1 txn, folded) | while-loop with `call @txn` | phi-node register pipeline → clang O(1) fold |
| iir_filter_runtime (1 txn, dead-field-eliminated) | while-loop with `call @txn` | phi-node → clang O(1) fold (0.26s → 0.00s) |
| async_counters_runtime (2 txns, both pure) | sequential `reactor_tick` (0.44s) | `emit_folded_multi_main` → clang O(1) fold (0.44s → 0.00s) |
| precompute_sum_runtime (2 txns, NOT pure) | sequential `reactor_tick` | No change (cannot fold) |

## Risk Assessment

- **Compiler tests**: 362 existing tests pass unchanged. The single-txn folding
  path tests (`test_folded_main_emitted`, `test_iir_filter_folded_path`) should
  continue to pass because the phi-node produces equivalent behavior (counter
  reaches bound, main returns 0).
- **Enum dispatch**: `emit_case_folded_loops` handles one txn at a time. The
  phi-node optimization applies per-txn within each case arm. The all-internal
  pure-counter shortcut (O(1) store) takes priority when bound is a
  compile-time constant.
- **No wake/trigger interaction**: Register pipeline is only for foldable
  (one-shot) programs, never for wake programs. No `__rt_wait()` or trigger
  sampling interaction.
- **Composability**: Multi-txn pipeline is checked AFTER enum dispatch, so
  trigger-gated programs still use their existing enum path. No conflict.

## File Changes

- `src/backend/llvm.rs` — `emit_folded_loop` gains `use_phi` parameter;
  `generate()` gains multi-txn all-pure check; new `emit_folded_multi_main`.
  ~60 lines net.
