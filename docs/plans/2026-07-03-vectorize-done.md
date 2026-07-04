# Vectorization Enablement: Break the Phi→Use Chain in done:

## Problem

The per-field phi registers (`%phi_bx0`, `%phi_vz0`, etc.) are defined at
`loop_hdr` and currently referenced by `emit_hoisted_post_loop_prints`
via `phi_regs_to_ssa_old()`.  This creates SSA uses of loop-carried values
OUTSIDE the loop (in `done:`), which LLVM's vectorizer detects and blocks:

```
loop not vectorized: value that could not be identified as reduction
is used outside the loop
```

## Fix

Replace `phi_regs_to_ssa_old()` with `pre_load_all_fields(out, "%state")`
in `emit_hoisted_post_loop_prints`.  This populates `ssa_old_*_regs` from
GEP+load of `%State` instead of from phi registers.  The phi values become
unused in `done:` — the vectorizer sees no loop-carried values escaping.

## Why GEP+load in done: doesn't block the vectorizer

The vectorizer analyzes the **loop body** (loop_hdr → body → latch), not the
exit block (done:).  GEP+load in done: is outside the analysis scope.

The loop body's stores use constant-index GEPs:
```llvm
store float %val, ptr %gep_i32_2    ; field 2
store float %val, ptr %gep_i32_5    ; field 5
```
LLVM's LoopAccessAnalysis can analyze these directly from the GEP indices —
no SROA decomposition required.  Constant indices make each field a distinct
memory location that the vectorizer can handle.

SROA not decomposing `%State` (because done: has GEPs) is acceptable.
The vectorizer doesn't need SROA when GEP indices are constant and the
struct is a single alloca with known offsets.

## Risk

Low.  This reverts a decision made in Phase 0 that prioritized SROA over
vectorization.  The trade-off was wrong — vectorization wins by 4-8×,
SROA wins by 1-2×.  All benchmarks should remain correct (same values
loaded from %State, just through GEP+load instead of phi registers).
