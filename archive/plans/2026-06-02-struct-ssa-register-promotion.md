# Plan: Struct-SSA Register Promotion (Full Optimization)

**Date:** 2026-06-02
**Status:** Plan — ready for implementation

## Motivation

Briev's folded-loop codegen currently accesses every state field through
individual `gep @global_state → load` or `gep @global_state → store`
instructions. This creates 20+ memory instructions per loop iteration
for the IIR biquad (5 fields × 2 GEP/load + 5 fields × 2 GEP/store).

C keeps the same state in CPU registers via local variables. clang's SROA
promotes local variables to SSA registers — zero memory traffic.

Briev can match this by loading the entire `%State` struct once per loop
iteration and operating on it via `extractvalue`/`insertvalue` chains.
LLVM's SROA pass promotes these to pure registers, identical to C.

## Core Mechanism: `ssa_state_reg` flag

Add `ssa_state_reg: Option<String>` to `LlvmBackend`. When set to
`Some("reg_name")`:

- **`emit_expr`**: field identifier → `extractvalue %State %reg_name, field_idx`
  (instead of `gep %State, %State* @global_state, idx → load`)
- **`emit_stmt`**: `&field = expr` → compute value, then
  `insertvalue %State %reg_name, value, field_idx → %new_reg_name`,
  update `ssa_state_reg = Some("new_reg_name")`
  (instead of `gep → store`)

When `ssa_state_reg` is `None` (default): unchanged — existing GEP/load/store.

## Three Dispatch Paths

### Path A — Single-txn folded loop (`emit_folded_loop`)

Current:
- `use_phi = true`: phi-node for pure counters (register pipeline)
- `use_phi = false`: `call void @txn_name(%State* @global_state)` inside while-loop

With SSA mode (when `use_phi = false`):
```
body:
  %state = load %State, %State* @global_state      ; load once
  ; ssa_state_reg = "state"
  ; emit txn body inline (extract/insert on %state)
  ; ssa_state_reg updated through insertvalue chain
  store %State %final_state, %State* @global_state   ; store once
  br label %hdr
```

### Path B — Multi-txn async pipeline (`emit_folded_multi_main`)

Current gate: ALL async txns must be `is_pure_body || is_effectively_pure`.

New gate: ALL async txns must have `bounded_pre + increments` (convergence).
Remove the `is_pure_body` requirement. The struct-ssa mode handles non-pure
bodies by keeping all fields in registers.

```
loop:
  %state = load %State                                   ; load once
  for each txn (inline, SSA mode):
    check precondition (extractvalue)
    fire body (insertvalue chain)
  store %State %state, %State* @global_state              ; store once
```

### Path C — Sequential multi-txn main (`emit_ssa_main`, new function)

For programs where ALL reactive txns have `bounded_pre + increments`
but are NOT single-txn-foldable, NOT precomputed, NOT enum, NOT async.

This catches programs like `precompute_sum_runtime.bv` with live
accumulators (two conflicting txns, both convergent).

```
tick:
  %state = load %State, %State* @global_state      ; load once
  for each txn (inline, SSA mode):
    check precondition (extractvalue)
    fire body (insertvalue chain)
  store %State %state, %State* @global_state        ; store once
  exit check
  br i1 %exit, done, tick
```

## Gate Logic in `generate()`

```
1. Single-txn foldable → emit_folded_main
   - pure/effectively-pure + const bound → O(1) store
   - pure/effectively-pure + runtime bound → phi-node (use_phi=true)
   - non-pure → SSA mode (use_phi=false, ssa_state_reg set)

2. Precompute check → emit_precomputed_main (unchanged)

3. Multi-txn async (all convergent + async) → emit_folded_multi_main
   - NOW: relaxed gate — requires only bounded_pre + increments
   - Pure/effectively-pure → phi-nodes (as before)
   - Non-pure → SSA mode (new, via struct-ssa)

4. NEW: Sequential convergent (all convergent, NOT async)
   → emit_ssa_main (load state, run bodies, store state)

5. Enum dispatch → emit_enum_main (unchanged)

6. Sequential/Parallel fallback → emit_reactor + emit_main (unchanged)
```

## Precondition Tautologies

Make float/accumulator fields live so dead-field elimination retains them:

**`benchmarks/iir_filter_runtime.bv`:**
```
node process [bound > 0 && count < bound
  && x1 == x1 && x2 == x2 && y1 == y1 && y2 == y2][count == bound]
```

**`benchmarks/precompute_sum_runtime.bv`:**
```
node step_a [bound > 0 && count < bound && acc_a >= 0][count == bound]
node step_b [bound > 0 && count < bound && acc_b >= 0][count == bound]
```

## Expected Outcome (BOUND=50000000)

| Benchmark | Before (Briev) | After (Briev) | C | Notes |
|-----------|---------------|--------------|---|-------|
| ring_buffer | 0.01s (O(1)) | 0.01s (O(1)) | 0.00s | Tie |
| async_counters | 0.01s (O(1)) | 0.01s (O(1)) | 0.00s | Tie |
| iir_filter | 0.01s (dead-field) | ~0.12s (SSA) | ~0.12s | **Parity** |
| precompute_sum | 0.00s (dead-field) | ~0.01s (SSA) | ~0.01s | **Parity** |

## File Changes

| File | Change | Lines |
|------|--------|-------|
| `src/backend/llvm.rs` | `ssa_state_reg` field + init | +6 |
| `src/backend/llvm.rs` | `emit_expr` SSA mode (identifier) | +15 |
| `src/backend/llvm.rs` | `emit_stmt` SSA mode (store) | +15 |
| `src/backend/llvm.rs` | `emit_folded_loop` SSA path | +30 |
| `src/backend/llvm.rs` | `emit_folded_multi_main` gate change | +5 |
| `src/backend/llvm.rs` | `emit_ssa_main` (new function) | +60 |
| `src/backend/llvm.rs` | `generate()` gate logic | +20 |
| `benchmarks/iir_filter_runtime.bv` | precondition tautologies | +1 |
| `benchmarks/precompute_sum_runtime.bv` | precondition tautologies | +2 |
| **Total** | | **~154** |

Zero changes to parser, AST, proof engine, or analysis passes.
