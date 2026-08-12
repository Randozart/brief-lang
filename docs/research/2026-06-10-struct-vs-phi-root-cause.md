# Root Cause: %State Struct vs Phi Nodes

**Date:** 2026-06-10T22:45:00+02:00
**Author:** Benchmark Investigation
**Status:** Confirmed — fix implementing

## Problem

Briev's runtime benchmarks are 1.08×–4.36× slower than C. All share the same root
cause: the `%State` struct forces GEP + load/store for every field access, while
Clang uses phi nodes with zero memory overhead.

## Method

Compiled C reference for each benchmark with:
```
clang -O3 -ffast-math -march=native -S -emit-llvm
```

Compared against Briev's IR compiled with:
```
opt -O3 -ffast-math -mtriple=x86_64-pc-linux-gnu
llc -O3 --mcpu=native
```

## IR Comparison

### Instruction Counts in Hot Loop

| Operation | Clang (phi) | Briev (struct) | Ratio |
|---|---|---|---|
| **Load/store (kalman)** | 1 (stderr) | 132 + 29 | Briev 161× worse |
| **Load/store (fannkuch)** | 1 (stderr) | 36 + 38 | Briev 74× worse |
| **Load/store (nbody_sqrt)** | 1 (stderr) | 433 + 110 | Briev 543× worse |
| **Load/store (float_math)** | 1 (stderr) | 40 + 17 | Briev 57× worse |
| **Load/store (knucleotide)** | 1 (stderr) | 26 + 14 | Briev 40× worse |
| **Load/store (nbody_newton)** | 1 (stderr) | 372 + 110 | Briev 482× worse |
| **GEP** | 0 | 69–419 | Briev ∞ worse |
| **Phi nodes** | 6–35 | 0 | Clang phi, Briev struct |
| **Arithmetic** | ~equal | ~equal | Same |

### Key Observation

The ONLY memory operation in Clang's hot loop is the `load @stderr` for `fprintf`.
Everything else — state variables (x0, p00, count, etc.) — exists as **SSA phi
nodes** at the loop back-edge. Zero GEP, zero load, zero store.

Briev's `%State` struct forces:
1. `getelementptr %State, %State* %state, i32 0, i32 N` — address computation
2. `load/store type, type* %gep` — memory access

Every read or write is TWO instructions (GEP + load/store). For kalman's 132 reads,
that's 264 extra instructions in the hot loop vs Clang's 0.

### Why LLVM Can't Promote to Phi Nodes

The `%State` alloca in `main()` is passed to `@reactor_tick(%State* %state)`.
This is a function call that "escapes" the alloca. LLVM's SROA can only promote
allocas that never escape. Adding `@reactor_tick` as a wrapper prevents promotion.

Current call chain:
```
main:
  %state = alloca %State, align 8
  call @init_state(%state)
  tick:
    call @reactor_tick(%state)  ← escapes the alloca
    br exit_check
```

What `opt -O3` could do if `%state` didn't escape:
```
main:
  tick:
    %count.phi = phi i64 [ 0, entry ], [ %count.next, tick ]
    %p00.phi = phi float [ 0.1, entry ], [ %p00.next, tick ]
    ...
    ; body operates on phi registers directly
    br exit_check
```

## Root Cause

The `@reactor_tick` wrapper function is unnecessary for single-txn, no-trigger
programs. It was designed for multi-txn reactive dispatch with triggers, wake
events, and thread pools. But ALL six runtime benchmarks have a single reactive
txn with no triggers — the wrapper adds only overhead.

## Implementation Note: Heuristic, Not Ceiling

The current fix uses `txns.len() == 1` as a heuristic for "dispatch fully determined
at compile time." This is intentionally conservative — single-txn programs trivially
have no dispatch decisions at runtime.

Future expansion should replace this with a general predicate:
```rust
fn dispatch_statically_deterministic(&self) -> bool {
    self.async_txn_names.is_empty()        // no async dispatch
    && self.mmio_fields.is_empty()          // no volatile memory
    && self.trigger_names.is_empty()        // no runtime events
    && !has_wake_triggers                   // no wake-trigger polling
    && (enumerable.is_some()                // all triggers bounded, OR
        || txns.len() == 1)                 // single txn, no triggers
    // Future: add multi-txn convergence-order-proven
}
```

This would cover enumerable-trigger programs (switch dispatch) and single-txn
programs equally. Currently blocked on `enumerable` analysis not being available
at the dispatch decision point (line 1279).

## Fix (current heuristic)

Add a new dispatch path (A006) in `src/backend/llvm/mod.rs` that detects
single-txn, no-trigger programs and emits directly into `main()` using
`emit_ssa_main` instead of going through `@reactor_tick`.

Condition:
```rust
txns.len() == 1
    && !has_wake_triggers
    && enumerable.is_none()
    && self.async_txn_names.is_empty()
    && self.mmio_fields.is_empty()
```

This catches ALL six runtime benchmarks. After inlining in `main()` via
`emit_ssa_main`, the `%state` alloca never escapes, and LLVM's SROA promotes
every field to a phi node — eliminating all GEP, load, and store instructions
from the hot loop.

## Expected Impact

| Benchmark | Before | Expected after | Cause closure |
|---|---|---|---|
| nbody_newton | 1.08× | ~1.0× | 372 loads → 0 |
| nbody_sqrt | 2.41× | ~1.0× | 433 loads + 110 stores → 0 |
| fannkuch_redux | 4.36× | ~1.0× | 36 loads + 38 stores → 0 |
| float_math_nonzero | 2.42× | ~1.0× | 40 loads + 17 stores → 0 |
| knucleotide | 1.24× | ~1.0× | 26 loads + 14 stores → 0 |
| kalman_filter_runtime | 3.48× | ~1.0× | 132 loads + 29 stores → 0 |

All six benchmarks should reach parity (or near-parity) because the memory
overhead is eliminated. Remaining gaps would be from instruction-level
differences (e.g., `vsqrtps` vs scalar `sqrt`), not memory access.
