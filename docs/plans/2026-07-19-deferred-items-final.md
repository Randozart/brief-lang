# Deferred Items: Implementation Plan

**Date:** 2026-07-19
**Status:** Build — ready to implement
**Prerequisites:** DRY consolidation (Rule 16), baseline worktree (Rule 11b), all prior stabilization

---

## Overview

Three remaining gaps. Implementation order is critical — each builds on the next.

| Step | Gap | Fix Type | Impact | Risk |
|------|-----|----------|--------|------|
| 1 | Memory loop dispatch | 2 lines | nbody_newton 13.5s → ~6.1s | Low — same fix as `emit_countable_main` |
| 2 | Native float types in %State | ~80 lines across 7 files | Eliminates all float boxing/unboxing from state access | Medium — `memcpy` size bug at mod.rs:241 |
| 3 | DRY consolidation | Inline with Step 2 | Remaining 39 hand-rolled sites | Low — mechanical replacement |

---

## Step 1: Memory Loop Dispatch (2 lines, 5 minutes)

### The bug

`emit_folded_memory_main` at counter.rs:208 is missing `last_val_temps.clear()` and `last_val_types.clear()` before the hoisted prints. During the loop body, `last_val_temps` accumulates entries for ALL variables (state fields AND local let bindings like `vx01`, `energy_step`). After the loop, `.fm_body` does NOT dominate `.fm_end`. Any stale entry in `last_val_temps` that gets resolved by the hoisted body produces `undef` → LLVM `-O3` folds to `~0.0`.

`emit_countable_main` has the exact same pattern at line 391 with the clear. The memory loop was simply missing it.

### File: `src/backend/llvm/loop_engine/counter.rs`

**Line 208 — add two clear() calls:**
```rust
// Before (broken):
self.fun.reg_float_cache.clear();
let hoisted = self.fun.pending_post_hoist.clone();

// After (fixed):
self.fun.reg_float_cache.clear();
self.fun.last_val_temps.clear();
self.fun.last_val_types.clear();
let hoisted = self.fun.pending_post_hoist.clone();
```

### File: `src/backend/llvm/mod.rs` — re-enable dispatch

Remove the `post_hoist.is_empty()` guard (line 2640 area):
```rust
// Change:
} else if post_hoist.is_empty() && write_density >= 0.8 && total_fields >= 8 {

// To:
} else if write_density >= 0.8 && total_fields >= 8 {
```

### Verification

```bash
cargo build --release
BOUND=50000000 ./target/release/briv-compiler build benchmarks/nbody_newton.bv --out benchmarks
# Verify output matches C reference
diff <(BOUND=50000000 timeout 30 ./benchmarks/nbody_newton) <(timeout 30 ./benchmarks/nbody_newton_c)
# Verify timing
BOUND=50000000 bash -c "TIMEFORMAT='%3R'; time ./benchmarks/nbody_newton"
# Expect ~6-8 seconds (was 13.5s)
```

---

## Step 2: Native Float Types in %State

### Design

**Key insight:** Cell state types (`cell_state_types` in `push_field_type`) ALREADY use native LLVM types:
```rust
cs_tys.push(self.llvm_type(&field.ty).to_string());  // mod.rs:3546
```

The main `%State` type is the ONLY place where `"i64"` is hardcoded. The fix makes it consistent with cell state.

**Memory stores** — the `State::Assign` and `Statement::Let` handlers at emit_stmt.rs, emit_toplevel.rs, and counter.rs all call `adapt_to_i64` to box float values before storing. With native types, this boxing is skipped — the native float value is stored directly.

**Memory loads** — identifier resolution at emit_expr.rs currently does:
```
load i64 → trunc i64 to i32 → bitcast i32 to float
```
With native types, this becomes:
```
load float → (no unboxing needed)
```

**Phi registers** — stay `i64` (phi registers are fundamentally i64 in SSA). The phi register path at emit_expr.rs:91-109 KEEPS its float unboxing — no change.

### File changes

| # | File | Lines | What |
|---|------|-------|------|
| 2a | `mod.rs:932` | 1 | Change `push_field_type` from `"i64"` to `self.llvm_type(&ty)` |
| 2b | `loop_engine/mod.rs:241` | 1 | Fix `memcpy` size: `field_types.len() * 8` → `compute_state_size_bytes()` |
| 2c | `helpers.rs:2038-2077` | 20 | Make 4 load/store helpers type-aware (read `field_types[idx]`) |
| 2d | `emit_expr.rs:110-132` | 15 | Dynamic load type, remove float unboxing branch for state loads |
| 2e | `emit_stmt.rs:95-99` | 8 | Dynamic store type, skip `adapt_to_i64` for native fields |
| 2f | `emit_toplevel.rs:800-878` | 25 | Init stores use native type, skip boxing |
| 2g | `gpu.rs:640` | 5 | GPU store path — dynamic type dispatch |

### Detailed changes

#### 2a. `push_field_type` (mod.rs:932)

```rust
// Current:
self.ctx.field_types.push("i64".to_string());

// New:
let native = self.llvm_type(ty);
self.ctx.field_types.push(native);
```

#### 2b. `memcpy` size (loop_engine/mod.rs:241)

```rust
// Current:
let state_bytes = (self.ctx.field_types.len() * 8) as i64;

// New:
let state_bytes = self.compute_state_size_bytes();
```

#### 2c. Helpers (helpers.rs:2038-2077)

Each of the 4 helpers needs to emit the type from `field_types[idx]` instead of hardcoded `i64`.

**`emit_state_load_i64`** (line 2045):
```rust
// Current:
writeln!(out, "{}{} = load i64, ptr {}, align 8", indent, val, gep).ok();

// New:
let llvm_ty = &self.ctx.field_types[idx];
writeln!(out, "{}{} = load {}, ptr {}, align {}", indent, val, llvm_ty, gep, self.align_of(llvm_ty)).ok();
```

**`emit_state_store_i64`** (line 2056):
```rust
// Current:
writeln!(out, "{}store i64 {}, ptr {}, align 8", indent, val, gep).ok();

// New:
let llvm_ty = &self.ctx.field_types[idx];
writeln!(out, "{}store {} {}, ptr {}, align {}", indent, llvm_ty, val, gep, self.align_of(llvm_ty)).ok();
```

The `_by_idx` variants follow the same pattern.

#### 2d. Identifier resolution (emit_expr.rs:110-132)

The `field_index_map` path (memory state load):
```rust
// Current — hardcoded load i64 + unbox:
let briv_ty = ...;
writeln!(out, "{}{} = load i64, ptr {}", indent, v, gep).ok();
if briv_ty == Type::float64() {
    let dbl = self.fun.gen_reg();
    writeln!(out, "{}{} = bitcast i64 {} to double", indent, dbl, v).ok();
    TypedRegister { name: dbl, ty: Type::float64() }
} else if briv_ty == Type::float() {
    let tr = self.fun.gen_reg();
    let fl = self.fun.gen_reg();
    writeln!(out, "{}{} = trunc i64 {} to i32", indent, tr, v).ok();
    writeln!(out, "{}{} = bitcast i32 {} to float", indent, fl, tr).ok();
    TypedRegister { name: fl, ty: Type::float() }
} else {
    TypedRegister { name: v.to_string(), ty: briv_ty }
}

// New — dynamic load, no unboxing:
let llvm_ty = &self.ctx.field_types[idx];
writeln!(out, "{}{} = load {}, ptr {}, align {}", indent, v, llvm_ty, gep, self.align_of(llvm_ty)).ok();
let briv_ty = ...;
TypedRegister { name: v.to_string(), ty: briv_ty }
```

The `phi_field_regs` path (lines 91-109) stays the same — phi registers remain `i64`.

#### 2e. Assign to state field (emit_stmt.rs:95-99)

```rust
// Current:
let boxed = backend.adapt_to_i64(out, indent, &val);
writeln!(out, "{}store i64 {}, ptr {}", indent, boxed, ptr).ok();

// New:
let llvm_ty = &backend.ctx.field_types[idx];
let store_val = if backend.llvm_type(&val.ty) == *llvm_ty {
    val.name.clone()  // native type matches — store directly
} else {
    backend.adapt_to_i64(out, indent, &val)  // box
};
writeln!(out, "{}store {} {}, ptr {}", indent, llvm_ty, store_val, ptr).ok();
```

#### 2f. Init stores (emit_toplevel.rs:800-878)

Float literal init (line ~826):
```rust
// Current (boxes float to i64):
let hex = float_to_llvm_str(*f, llvm_ty);
writeln!(out, "{}{} = bitcast i32 {} to float", indent, val, hex).ok();
writeln!(out, "{}{} = bitcast float {} to i32", indent, bo, val).ok();
writeln!(out, "{}{} = zext i32 {} to i64", indent, z, bo).ok();
writeln!(out, "{}store i64 {}, ptr {}, align {}", indent, z, gep, align_of("i64")).ok();

// New (stores float directly when field is "float"):
if llvm_ty == "float" {
    let bytes = float_to_llvm_str(*f, "float");
    writeln!(out, "{}store float {}, ptr {}, align 4", indent, bytes, gep).ok();
} else {
    // existing boxing path for i64 state
    ...
}
```

The catch-all at line 882:
```rust
// Current:
let boxed = self.adapt_to_i64(out, indent, &val_reg);
writeln!(out, "{}store i64 {}, ptr {}, align {}", indent, boxed, gep, self.align_of("i64")).ok();

// New:
if backend.llvm_type(&val_reg.ty) == llvm_ty {
    writeln!(out, "{}store {} {}, ptr {}, align {}", indent, llvm_ty, val_reg.name, gep, self.align_of(&llvm_ty)).ok();
} else {
    let boxed = self.adapt_to_i64(out, indent, &val_reg);
    writeln!(out, "{}store i64 {}, ptr {}, align {}", indent, boxed, gep, self.align_of("i64")).ok();
}
```

#### 2g. GPU store (gpu.rs:640)

```rust
// Current:
ir.push_str(&format!("{}store i64 {}, i8* {}, align 8\n", indent, val, gep));

// New:
let ft = field_types.get(&name).cloned().unwrap_or_else(|| "i64".to_string());
match ft.as_str() {
    "float" => ir.push_str(&format!("{}store float {}, float* {}, align 4\n", indent, val, gep)),
    "double" => ir.push_str(&format!("{}store double {}, double* {}, align 8\n", indent, val, gep)),
    _ => ir.push_str(&format!("{}store i64 {}, i8* {}, align 8\n", indent, val, gep)),
}
```

### Verification

```bash
cargo build --release
# Build every benchmark — LLVM verifier catches any type mismatch
for b in benchmarks/*.bv; do
    BOUND=50000000 timeout 30 ./target/release/briv-compiler build "$b" --out benchmarks
done
# Run correctness check
BOUND=50000000 timeout 300 bash benchmarks/build_and_bench.sh --correctness
# Compare nbody_newton timing
bash benchmarks/compare_baseline.sh nbody_newton
```

---

## Step 3: DRY Consolidation

No separate implementation — migrate remaining sites as each file is touched in Step 2. The centralized helpers already exist:
- `emit_state_load_i64_by_idx` / `emit_state_store_i64_by_idx` (helpers.rs)
- `ensure_typed_value` (helpers.rs) — float/double unboxing
- `adapt_to_i64` (helpers.rs) — float/double boxing

---

## Implementation Order

```
1. cargo build — verify clean state
2. Step 1: counter.rs + mod.rs (memory loop fix)
3. Build nbody_newton, verify timing + correctness
4. Step 2a: mod.rs (push_field_type)
5. Build — expect LLVM errors from type mismatches
6. Steps 2b-2f: Fix each error as it appears (LLVM verifier guides)
7. Step 2g: gpu.rs
8. Full correctness check
9. Commit all changes
```
