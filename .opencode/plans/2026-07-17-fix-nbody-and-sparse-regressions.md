# Fix nbody_newton and sparse_dispatch Regressions

## Fix 1: nbody_newton — Floating Point Dominance Bug

### Root Cause
`ensure_float_reg` caches `fpext` results in `reg_float_cache`. The periodic
print's `Print#(energy)` emits `%t2341 = fpext float %t2325 to double`
inside the periodic print's conditional block. The hoisted swan song
`term! -> Print#(last_energy)` resolves last_energy to the same SSA register
`%t2325`, hits the cache, reuses `%t2341` — but the conditional block
doesn't dominate the exit block.

For BOUND=5 (harness default), the periodic block never fires, `%t2341` is
undefined, LLVM replaces it with `0.0`.

### Fix
Clear `reg_float_cache` before emitting hoisted post-loop prints.

**File**: `src/backend/llvm/loop_engine/counter.rs` (line ~352)
```rust
// Before emit_hoisted_post_loop_prints:
self.fun.reg_float_cache.clear();
```

## Fix 2: sparse_dispatch — SSA Alloca Pointer Poisoning

### Root Cause A
`emit_ssa_mt_prealloc` inserts alloca pointers into `last_val_temps`.
When `emit_expr` looks up a field, it finds a `ptr`-typed alloca register
and tries to use it as `i64` — LLVM type error.

### Fix A
Remove `last_val_temps.insert(name.clone(), alloca)` from 
`emit_ssa_mt_prealloc`. Field reads fall through to GEP + load from `%State`.

**File**: `src/backend/llvm/loop_engine/ssa.rs`

### Root Cause B
Commit `fb3c335` removed the `try_modulo_switch_dispatch` early-return
from `emit_ssa_main`. sparse_dispatch now checks 8 preconditions per
iteration instead of 1 modulo check + direct branch.

### Fix B
Re-instate the `try_modulo_switch_dispatch` check.

**File**: `src/backend/llvm/loop_engine/ssa.rs`

## Regression Guard
- `cargo test --lib` — 913+ tests pass
- `nbody_newton BOUND=5` prints `-0.169...` (matches C)
- `sparse_dispatch` compiles and runs
