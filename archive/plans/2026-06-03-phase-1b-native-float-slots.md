# Phase 1b: Native Float State Slots

**Date:** 2026-06-03
**Status:** Complete — all 8 implementation steps done

## Problem

Briev's `%State` struct currently uses all-`i64` slots. Every float field goes through:

```
Write: i64 → trunc to i32 → bitcast to float → store float
Read:  GEP → load float → bitcast to i32 → zext to i64 → i64 register
```

This is 4 boxing instructions per float access. At 50M iterations × 30 float fields × 2 accesses (read + write) = 12B unnecessary instructions. FFI float calls compound this: `i64 → trunc → bitcast → call → bitcast → zext → i64` adds 6 more per call.

Other LLVM-based languages (Rust, Zig, Swift, Julia) keep native types in state/struct slots. Briev's boxing was an early-development simplification that is now the bottleneck.

## What Already Exists

The infrastructure for native float registers is ~80% complete:
- `TypedRegister { name, ty }` tracks expression types through the AST
- `emit_binop` already emits `fadd`/`fsub`/`fmul`/`fdiv` when both ops are `Type::Float`
- `pre_extract_float_fields` extracts native `float` SSA values from state
- `reg_float_cache: HashMap<String, String>` maps `i64` reg names to native `float` regs
- `let_binding_types: HashMap<String, Type>` tracks let-binding types

What's missing: the store and load boundaries between Briev's type system and LLVM's `%State` type.

## Changes Required

### 1. `declare_state_type` — per-slot types

**File:** `src/backend/llvm.rs`, function `declare_state_type` (line ~1735)

**Current:**
```rust
let mut fields = Vec::new();
for i in 0..self.field_index_map.len() {
    fields.push("i64".to_string());
}
writeln!(out, "%State = type {{ {} }}", fields.join(", "));
```

**New:**
```rust
for ty_str in &self.field_types {
    let ll_ty = match ty_str.as_str() {
        "float" => "float",
        "i8" => "i8",
        "i8*" => "i8*",
        _ => "i64",
    };
    fields.push(ll_ty.to_string());
}
```

The `field_types: Vec<String>` already contains `"i64"` or `"float"` per slot, populated by `build_field_index()` which reads from `StateDecl` via `Type` → LLVM type string. This is a single-line change.

### 2. `emit_expr → Expr::Identifier` — native float load

**File:** `src/backend/llvm.rs`, line 2557-2561

**Current:**
```rust
"float" => {
    let i = bitcast float → i32;
    zext i32 → i64;
}
```

The load already produces a native `float` register (`%ld`). The boxing chain converts it back to `i64`. We need to:
- Return the native float register directly
- Track it in `reg_float_cache`
- Return `TypedRegister { name: v, ty: Type::Float }` where `v` is the boxing result for backward compat, but also cache the native register

**New approach:** Two-register strategy. The identifier produces an `i64` register (for existing i64 consumers) AND caches a native `float` register (for typed consumers):
```rust
"float" => {
    let i = bitcast float %ld to i32;
    zext i32 %i to i64 → %v ;
    self.reg_float_cache.insert(v.clone(), ld.clone()); // cache native float
    return TypedRegister { name: v, ty: Type::Float };
}
```
This is already done (line 2474-2475 in SSA path). The gap is the non-SSA path (line 2559) — it never inserts into `reg_float_cache`.

### 3. `emit_stmt → Statement::Assignment` — native float store

**File:** `src/backend/llvm.rs`, function `emit_stmt`, `Statement::Assignment` arm

**Current:**
```rust
// For all types including float:
let av = emit_expr(out, expr, indent);  // → i64 register
// Then: trunc i64 → i32 → bitcast i32 → float → store float
```

**New:** Check `av.ty` and the slot's type. If both are `Type::Float`, check `reg_float_cache` for the native float register and store it directly:
```rust
if av.ty == Type::Float && field_type == "float" {
    if let Some(float_reg) = self.reg_float_cache.get(&av.name) {
        writeln!(out, "store float {}, float* %gep, align 4", float_reg);
        return;
    }
}
// Fall through to existing boxing path
```

### 3b. SSA state merge paths (the hard part)

SSA mode uses `insertvalue %State` chains. Currently:
```llvm
%ss2 = insertvalue %State %ss1, i64 %counter, 0
```
With typed slots, element index 0 maps to type `i64`, index 2 maps to type `float`. LLVM's `insertvalue` requires:
```
insertvalue <aggregate type>, <element value>, <index>
```
where element value type MUST match the aggregate element type. If slot 2 is `float`, we must pass a `float` register.

**Affected functions:**
- `emit_folded_loop` (line 3412+): SSA state construction at loop header
- `emit_ssa_main` (line 3598+): SSA state construction at tick entry
- `emit_reactor` (line ~3100+): post-body SSA state construction
- Various case-body paths in `emit_folded_multi_main`

**Fix:** Before each `insertvalue`, check `field_types[idx]` and convert the value:
```rust
let slot_ty = &self.field_types[idx];
match slot_ty.as_str() {
    "float" => {
        if let Some(float_reg) = self.reg_float_cache.get(&val_reg) {
            writeln!(out, "%ss{} = insertvalue %State %ss{}, float {}, {}",
                idx, prev_idx, float_reg, idx);
        } else {
            // Boxing path: trunc+bitcast, then insertvalue float
        }
    }
    _ => {
        writeln!(out, "%ss{} = insertvalue %State %ss{}, i64 {}, {}",
            idx, prev_idx, val_reg, idx);
    }
}
```

### 4. `emit_init_state` — native float initialization

**File:** `src/backend/llvm.rs`, `emit_init_state`

**Current:**
All fields: `bitcast i32 0 to float → store float` (for float), `store i64` (for int).

**New:**
Check `field_types[idx]` and emit direct stores without boxing:
```rust
"float" => { store float <val>, float* %gep, align 4 }
"i64"   => { store i64 <val>, i64* %gep, align 8 }
```

### 5. `emit_precomputed_main` — precomputed float values

Same change as init_state — direct native float stores for precomputed values.

### 6. FFI Fast Path (Level 1)

In `emit_expr → Expr::Call`, the Float argument marshal already has the `trunc → bitcast` path. Add fast path:
```rust
Type::Float => {
    if let Some(cached) = self.reg_float_cache.get(&raw.name) {
        marshaled.push(format!("float {}", cached));
    } else {
        // existing trunc+bitcast path
    }
}
```

FFI return marshaling: after `call float @__sqrtf(...)`, cache the result as a native float.

## Non-Regression Guarantee

- All changes add new match arms or fast paths before existing code. When the cache misses, the existing slow path executes unchanged.
- Integer operations never interact with `reg_float_cache` — no change.
- Existing float benchmarks (kalman_filter, float_math, float_math_nonzero) verify correctness through SROA + SLP hazard.
- `emit_binop` already correctly handles float vs int — no type confusion possible.

## Impact Estimate

| Benchmark | Current Boxing Ops | After Native Slots | Expected Improvement |
|-----------|-------------------|-------------------|---------------------|
| nbody_newton | 6 per float op × 200 float ops/tick × 50M = 60B | 1 per float op = 10B | ~2-3× faster |
| nbody_sqrt | 6 per float op + 6 FFI per call × 20 calls/tick | 1 per float op + 2 FFI per call | ~4× faster (est 6.8s → 1.7s) |
| float_math | 6 per float op × 8 ops/tick | 1 per float op | ~3× faster |
| kalman_filter | 6 per float op × 126 ops/tick | 1 per float op | ~3× faster |

## Implementation Order

1. `declare_state_type` — emit typed %State (1 line change)
2. `emit_identifier` — cache native float in non-SSA path (1 line)
3. `emit_stmt → Assignment` — fast-path native float store (5 lines)
4. `emit_init_state` — native float init (3 lines)
5. `emit_precomputed_main` — native float precompute (3 lines)
6. SSA insertvalue chains — float-aware slot typing (most complex, ~20-30 lines per site)
7. FFI fast path — float arg marshal + return cache (10 lines)
8. Test suite + benchmark verification

## Dependencies

- Phase 1 (struct codegen) — complete. `struct_types` and `let_binding_types` already exist.
- No dependency on Phase 2 (enum codegen) or Phase 3 (collections).
- Can be implemented independently and in parallel with Phase 2.

---

## Completion Summary (2026-06-03)

**Status: Done.** 410 tests pass, 11 benchmarks zero regression.

### What Was Implemented

| Step | Description | Result |
|------|-------------|--------|
| `declare_state_type` | Already correct — uses `field_types` which has `"float"` for float slots | No change |
| `emit_identifier` (non-SSA) | Cache native float register after field load (line 2581) | Boxing skipped on re-read |
| `emit_stmt → Assignment` (SSA + non-SSA) | `native_float_or_box` at 3 store sites | 405/412 insertvalue ops use native `float` |
| `emit_init_state` | `native_float_or_box` for non-literal float init | Eliminates ~2000 trunc+bitcast ops |
| `emit_precomputed_main` | Per-slot type-aware store (float/i8/i64) | Correct for typed `%State` |
| SSA insertvalue chains | Cache hit via `native_float_or_box` | Already working via cache propagation |
| FFI fast path | `native_float_or_box` for FFI float args + return cache | 0.5s improvement on nbody_sqrt |
| Float literal emission | Cache native float in `Expr::Float` | Eliminates re-boxing for literals |

### Key Design

The `native_float_or_box` helper checks `reg_float_cache` before emitting the `trunc i64 → i32 → bitcast i32 → float` chain. The cache is populated by:
- `Expr::Float` literal emission
- `emit_binop` float result
- `emit_identifier` float field load
- `Expr::Neg` float result
- FFI float return demarshal
- SSA float extract from `pre_extract_float_fields`

The cache is cleared at every function/body boundary alongside `let_bindings`.

### Audit Results (nbody_newton IR)
- `%State = type { i64, i64, float×30 }` — typed slots working
- 405/412 insertvalue ops use `float` (98.3%)
- Remaining boxing: ONLY in `native_float_or_box` fallback (cache-miss path) and `emit_cast_convert` type conversions

### Benchmark Progression
| Benchmark | Before Phase 1b | After Cache | After L2 init_state fix |
|-----------|----------------|-------------|------------------------|
| nbody_newton | 3.18s | 3.05s (-4%) | **2.99s** (-6%) |
| nbody_sqrt | 6.81s | 6.33s (-7%) | **6.19s** (-9%) |
| float_math_nonzero | 0.1619s | 0.1605s | **0.1589s** (-2%) |

