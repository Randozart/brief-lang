# Kalman Filter 2× Regression: Root Cause Analysis

## Date: 2026-06-02

### The Regression

The old conversation showed **0.716s Briv vs 0.781s C** (50M iterations, Briv 9%
faster). Our current implementation runs the same benchmark at **0.28s at 10M**.
Scaling: 0.716s / 5 = **0.143s expected** vs **0.28s actual** = **exactly 2× slower**.

### What Changed

The struct-SSA optimization (dead-field elimination + pure-counter fold, Step 7)
introduced two codegen paths in `emit_folded_main`:

- **`use_phi=true`**: Counter uses a phi node, pure/effectively-pure bodies get
  O(1) store. Optimal for dead-field-eliminated programs (IIR, ring_buffer, etc.).
  
- **`use_phi=false, body=Some(stmts)`**: Loads entire `%State` struct from memory,
  executes body via `extractvalue`/`insertvalue` chains, stores entire struct.
  **Used when float fields are live** (referenced in preconditions or `#!exit`).

The Kalman filter takes the `use_phi=false, body=Some(stmts)` path because the
precondition references all 12 float fields (`x0 == x0 && x1 == x1 && ...`), making
them live. This path emits:

```llvm
%ssaN = load %State, %State* @global_state      ; 64-byte struct load
; ... 13 chained insertvalue instructions ...
store %State %finalReg, %State* @global_state     ; 64-byte struct store
```

This struct-SSA requires **SROA** (Scalar Replacement of Aggregates) to decompose
into per-field scalar operations — and **`llc -O2` does NOT run SROA**. Only
`opt -O2` does.

The old (pre-struct-SSA) codegen used per-field `GEP + load/store` throughout,
which LLVM's backend handles naturally without SROA.

### The Fix: Option A

Run `opt -O2 -vectorize-slp=false` before `llc` in the compilation pipeline:

```
briv-compiler → .ll → opt -O2 -vectorize-slp=false → .opt.ll → llc -O2 → .o
```

This:
1. **SROA**: Breaks `%State` into individual field allocas, then `mem2reg`
   promotes them to scalar phi nodes
2. **GVN**: Eliminates redundant float→i64→float round trips
3. **SLP disabled**: No `<2 x float>` packed phis → no shufflevector → no spills
4. **Zero user intervention**: Automatic in the compiler pipeline

Compile-time cost: ~50ms (separate process invocation).

### Expected Result

Without SLP, 12 scalar float phis fit cleanly in 16 XMM registers (zero spills).
With SROA, the per-field scalar operations are recovered from struct-SSA. Together
this should restore **0.716s at 50M** (14.32 ns/iter, beating C by 9%).
