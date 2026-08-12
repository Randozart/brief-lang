# A006: Direct SSA Loop Dispatch

**Date added:** 2026-06-10
**Phase:** Optimization Sprint

## Problem

The `@reactor_tick` wrapper function received `%State*` as a function argument,
causing the `%State` alloca in `main()` to escape. LLVM's SROA pass can only
promote allocas that never escape, so all state field accesses remained as
GEP + load/store — 100-500× more memory operations than Clang's phi-node
approach.

## Solution

For programs where runtime dispatch is fully determined at compile time
(no async dispatch, no MMIO), inline all txn bodies directly in `main()`
using `emit_ssa_main` instead of going through `@reactor_tick`.

## Condition

```rust
!txns.is_empty()
    && self.async_txn_names.is_empty()
    && self.mmio_fields.is_empty()
```

## What each case uses

| Case | Path | Why |
|---|---|---|
| Single txn, no triggers | `emit_ssa_main` | Fully determined at compile time |
| Multi-txn, no triggers | `emit_ssa_main` | emit_ssa_main iterates txns in order |
| Wake triggers | `emit_ssa_main` | Trigger sampling inline via lazy emit_trg_load |
| Enumerable triggers | `emit_folded_multi_main` (existing) | Switch dispatch in main() |
| Async/parallel | reactor loop (`emit_main`) | Thread pool needs independent state copies |
| MMIO fields | reactor loop (`emit_main`) | Volatile semantics prevent SROA |

## Trigger sampling

Triggers are sampled lazily on first reference in a precondition expression
via `emit_trg_load` (volatile load from trigger address). The result is cached
in `sampled_triggers` for subsequent references in the same tick.

## Wake path

For `has_wake_triggers` programs, `emit_ssa_main` emits `call void @__rt_wait()`
between ticks when no exit condition fires. This matches the behavior of the
previous `emit_main` path.

## Impact

Eliminated all GEP load/store from the hot loop for benchmarks without
async/MMIO. LLVM's SROA promotes every state field to a phi node, matching
Clang's zero-memory-overhead pattern.

### Results

| Benchmark | Before (reactor_tick) | After (direct SSA) | C | Ratio |
|---|---|---|---|---|
| nbody_sqrt | 2.81× | 3.72s | 3.17s | 1.17× |
| float_math_nonzero | 2.43× | 0.194s | 0.169s | 1.14× |
| knucleotide | 1.21× | 0.111s | 0.199s | **0.56× Briev wins** |
| kalman_filter_runtime | 3.62× | 0.178s | 0.184s | **0.96× Briev wins** |
| nbody_newton | 1.08× | precomputed | 9.1s | No hot-loop FFI |
| fannkuch_redux | 5.06× | precomputed | 0.07s | No hot-loop FFI |
