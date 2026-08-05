# Final Push: Close Three Remaining Gaps

**Date:** 2026-07-19
**Status:** Plan — ready to implement
**Prerequisite:** All earlier stabilization + DRY work complete (23/24 MATCH, Rule 16 established)

---

## Overview

Three gaps remain after the intrinsic migration and stabilization:

| Gap | Problem | Current | Target |
|-----|---------|---------|--------|
| 1. Memory loop dispatch | Stale `reg_float_cache` in `emit_folded_memory_main` causes hoisted swan song to use non-dominating float registers → wrong output | nbody_newton 13.5s | **~6.1s** |
| 2. Native float types | `push_field_type` hardcodes `"i64"` for all state fields. Float32 boxing/unboxing adds 3 extra instructions per field access | ~4× instruction count for float state access | **1×** (native `load float`, `store float`) |
| 3. DRY consolidation | 39 hand-rolled GEP+load/store sites across 8 files — each a potential bug vector | Bug fixed in 1 place, missed in N others | **Consolidated** |

---

## Gap 1: Memory Loop Dispatch — Stale `reg_float_cache`

### Root Cause

`emit_folded_memory_main` (counter.rs:206) loads final state values via `load_last_val_temps` and emits hoisted prints. But it does NOT clear `reg_float_cache` before the hoisted prints.

During the loop body, `reg_float_cache` accumulates entries mapping boxed i64 register names to their unboxed float counterparts. These float registers are defined inside `.fm_body` (and inside conditional guards like `[count % 5000000 == 0]` which may never fire for small BOUND values). The cache entries point to registers in non-dominating basic blocks.

When the hoisted swan song prints `last_energy` (a Float32), the expression emission calls `ensure_float_reg` which consults `reg_float_cache` before generating new instructions. It finds a stale cache entry → returns a register from inside the loop body that doesn't dominate the exit block → LLVM produces `undef` → output is `~3.47e-29`.

The same bug existed in `emit_countable_main` and was fixed at line 383:
```rust
self.fun.reg_float_cache.clear();    // Add this line — the entire fix
self.fun.last_val_temps.clear();
self.fun.last_val_types.clear();
```

### Fix

**File:** `src/backend/llvm/loop_engine/counter.rs`, ~line 206

Add `self.fun.reg_float_cache.clear()` before the hoisted prints:

```rust
// 2026-07-19: Missing reg_float_cache.clear() — same nbody_newton bug
// as was fixed in emit_countable_main (line 383). Stale cache entries
// point to float registers inside the loop body that don't dominate
// the exit block, producing undef in LLVM.
self.fun.reg_float_cache.clear();
self.fun.last_val_temps.clear();
self.fun.last_val_types.clear();
let hoisted = self.fun.pending_post_hoist.clone();
if !hoisted.is_empty() {
    self.load_last_val_temps(out);
    self.emit_hoisted_post_loop_prints(out, &hoisted);
}
```

After the fix, verify the memory counter loop dispatch is still active in mod.rs (lines 2635-2648). The condition `write_density >= 0.8 && total_fields >= 8` should already be there from the earlier change.

**Verification:** nbody_newton builds, runs, matches C output, and times between 6-8s.

---

## Gap 2: Native Float Types in %State

### Current Behavior

`push_field_type` (mod.rs:902) pushes `"i64".to_string()` for ALL state fields, regardless of their Briv type. Float32 values take 4 instructions per access:
```
GEP → load i64 → trunc i64 to i32 → bitcast i32 to float
```

### Change

**Step 2a — push_field_type (mod.rs:902):**

```rust
// Current:
self.ctx.field_types.push("i64".to_string());
// New:
self.ctx.field_types.push(self.llvm_type(ty));
```

This propagates the correct type:
- `"float"` for Float32
- `"double"` for Float64
- `"i64"` for Int
- `"i8"` for Bool

**Step 2b — Fix all load paths** (remove manual float unboxing):

| Location | File:Line | Change |
|----------|-----------|--------|
| Identifier resolution (state memory load) | `emit_expr.rs:127-132` | Load with `field_types[idx]`. Remove the `briv_ty == Type::float32()` unboxing block. The load already returns the correct type. |
| `load_last_val_temps` | `ssa.rs:513` | Load with `field_types[idx]`. Remove manual unboxing. |
| Identifier resolution (phi register) | `emit_expr.rs:108-126` | **Keep unboxing** — phi registers are always i64. Only memory loads change. |

**Step 2c — Fix all store paths** (remove manual float boxing):

| Location | File:Line | Change |
|----------|-----------|--------|
| `emit_field_init_value` catch-all | `emit_toplevel.rs:882` | Store with `field_types[idx]`. When target is float and value is float, store directly. |
| `emit_stmt.rs` state field store | `emit_stmt.rs:98` | Same — check `field_types[idx]`, store directly for native types. |
| `counter.rs` phi backedge store | `counter.rs:520` | Same — check `field_types[idx]`, store directly for native types. |

The boxing helper `adapt_to_i64` is still needed for phi registers (they always use i64). The change only affects the final store to `%State` via GEP.

**Safety:** LLVM verifier catches any type mismatch at compile time. A `load float` from a `float*` GEP that is actually an `i64` field in `%State` would be rejected. So any missed store path produces a clear build error, not a silent wrong output.

---

## Gap 3: DRY Consolidation (Inline with Gap 2)

While touching each file for Gap 2, replace hand-rolled GEP+load/store with the centralized helpers:

| File | Sites | Replacement |
|------|-------|-------------|
| `emit_expr.rs` | 2 (state field load) | `emit_state_load_i64_by_idx` + `ensure_typed_value` |
| `emit_toplevel.rs` | 5 (field init, marshaling) | `emit_state_store_i64_by_idx` |
| `emit_stmt.rs` | 1 (state field store) | `emit_state_store_i64_by_idx` |
| `loop_engine/counter.rs` | 6 (counter/bound load/store) | `emit_state_load_i64_by_idx` / `emit_state_store_i64_by_idx` |
| `loop_engine/ssa.rs` | 9 (counter/bound load) | `emit_state_load_i64_by_idx` |
| `mod.rs` | 1 (prealloc store) | `emit_state_store_i64_by_idx` |

This is mechanical — each replacement removes 3-4 lines and replaces with 1 line.

---

## Implementation Order

```
Step 1: Gap 1 — add reg_float_cache.clear(), verify nbody_newton timing
Step 2: Gap 2a — change push_field_type, build, observe LLVM errors
Step 3: Gap 2b — fix each load/store path that fails LLVM verification
Step 4: Gap 3 — inline DRY consolidation while touching each file
Step 5: Full correctness check + timing comparison
```

**Expected outcome:**
- nbody_newton: 6-8s (Phase 3 parity)
- All benchmarks: 23/24 MATCH, no regressions
- LLVM verifier: clean
- DRY: all common state field access patterns use centralized helpers
