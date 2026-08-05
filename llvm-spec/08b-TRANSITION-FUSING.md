# Transition Fusing: Symbolic State Composition

## Overview

If `Txn_A` transitions the state to `S'`, and the guard for `Txn_B` is guaranteed true at `S'`, the compiler fuses their bodies into a single atomic transition. This eliminates an entire reactor tick for the intermediate state.

## When Fusing Applies

**Precondition:** `post(Txn_A)` logically implies `pre(Txn_B)` — proven by the proof engine's symbolic analysis.

**Inhibition rules** (fusion is refused if ANY apply):
- `Txn_B`'s precondition references a volatile `trg` (see `08a-TRIGGERS.md`)
- `Txn_A` writes to a field that `Txn_B` reads AND writes (write-after-write would hide a bug)
- `Txn_A` or `Txn_B` is async (`is_async == true`) — external preemption breaks atomicity
- The fused state would exceed LLVM's per-function complexity budget (configurable, default 1000 instructions)

## Implementation

### Analysis: `detect_fusable_pairs`

Already exists in `src/backend/mod.rs` at line 291. Returns `Vec<(String, String)>` of `(Txn_A, Txn_B)` pairs where `post(A)` intersects `pre(B)`.

The lowering pass consumes this list and applies the inhibition rules to produce the final fusable set.

### Body Fusion

```briv
; Before fusion — two sequential ticks:
txn StateX [true] {
    &gpuBar0 = reservedMem;
    term;
}

txn StateY [gpuBar0 == reservedMem] {
    &gpuBar1 = signalY;
    term;
}

; After fusion — single tick:
txn StateX_Y [true] {
    &gpuBar0 = reservedMem;
    &gpuBar1 = signalY;  ; Fused into same execution step
    term;
}
```

### LLVM IR Result

Without fusion (2 ticks, 2 loads + 2 stores of `gpuBar0`):

```llvm
; Tick 1 — StateX
store i64 %reservedMem, i64* %gpuBar0_ptr
; Tick 2 — StateY
%gpuBar0_val = load i64, i64* %gpuBar0_ptr
store i64 %signalY, i64* %gpuBar1_ptr
```

With fusion (1 tick, 0 intermediate memory round-trips):

```llvm
; Single fused transition
%new_bar0 = add i64 0, %reservedMem
%new_bar1 = add i64 0, %signalY
store i64 %new_bar0, i64* %gpuBar0_ptr
store i64 %new_bar1, i64* %gpuBar1_ptr
```

Both stores happen in the same basic block — LLVM can schedule them optimally, issue them to the store buffer together, or hoist them past a single memory barrier.

## Integration with Reactor Loop

In the acyclic reactor loop (`08-REACTOR-LOOP.md`), fused transactions don't appear as separate `br`/`phi` paths. Instead, the fused body is emitted as a single block reachable from a single precondition check. The `detect_fusable_pairs` output tells the reactor emitter which preconditions to evaluate jointly and which bodies to inline as one unit.

## Fusing with Trigger Sampling

The sampling pass (`08a-TRIGGERS.md`) runs before fusion evaluation. If `Txn_B` references `trg_button`, the trigger is sampled at tick entry into `%button_sampled`. Since `Txn_B`'s guard uses `%button_sampled` (not the raw pointer), fusing would cause `Txn_B` to execute with a stale sample from `Txn_A`'s tick.

**Solution:** The inhibition rule automatically rejects fusion when `Txn_B`'s precondition references any `trg`. This is checked during the lowering pass by scanning the precondition's identifier references against the program's trigger declarations.
