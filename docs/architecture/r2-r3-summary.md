# R2 + R3: Float Boxing Elimination & Per-Field GEP Loops

**Date added:** 2026-06-10
**Phase:** Optimization Sprint

## R2: Float Boxing Elimination

### Problem

Every float value traveled through `i64` in the LLVM backend:
```
unbox: trunc i64 → bitcast i32 → float
op:    fadd fast float
box:   bitcast float → i32 → zext i64
```

5 extra instructions per float operation. For nbody_newton's ~200 float ops
per tick, this was ~1000 wasted instructions.

### Solution

Emit float values as native `float` registers through the entire backend:
- Float literals emit `bitcast i32 N to float` directly (no i64 boxing)
- Float binops (`fadd`, `fmul`, etc.) operate on native float registers
- State field loads return native float TypedRegisters
- FFI boundaries keep boxing (must remain for C ABI)

### Key changes

| File | Change |
|---|---|
| `emit_expr.rs` | Float literal, Neg, binop, fcmp return native float TypedRegisters |
| `loop_engine.rs` | Pre-extraction loads float fields as native float |
| `emit_toplevel.rs` | `ensure_float_reg` helper handles both native float and boxed i64 |
| `mod.rs` | Float state type returns "float" not "i8" |

## R3: Per-Field GEP Loop Codegen

### Problem

The folded SSA path emitted:
```llvm
%ss = load %State           ; wide load
%p0 = extractvalue %State %ss, 0  ; tear apart
...body...
%ss1 = insertvalue %State %ss, %new_p0, 0  ; rebuild
store %State %ss1            ; wide store
```

The wide load/store prevented LLVM's SROA from promoting individual fields.

### Solution

Replace with per-field GEP loads/stores:
```llvm
%p0 = load i64, i64* %gep_0  ; direct scalar
...body...
store i64 %new_p0, i64* %gep_0  ; direct scalar
```

This lets LLVM's SROA promote each field independently. After `opt -O3`, every
field becomes a phi node — matching Clang's output pattern.

### Key changes

| File | Change |
|---|---|
| `loop_engine.rs` | `pre_load_all_fields` loads via GEP; `emit_ssa_main` uses per-field GEP |
| `emit_expr.rs` | Non-SSA field loads load directly into destination register (no add i64 0, %il) |
| `mod.rs` | Removed ssa_state_reg for A006 path |

## Copy Elimination

### Problem

Every non-SSA field load emitted:
```llvm
%ilN = load i64, i64* %gep  ; load into intermediate
%tM  = add i64 0, %ilN       ; copy to destination
```

This added 10-18% redundant copy instructions.

### Solution

Emit the load directly into the destination register when no type conversion is
needed:
```llvm
%tM = load i64, i64* %gep  ; load directly into destination
```

### Impact

| Benchmark | Before copies | After copies | Reduction |
|---|---|---|---|
| fannkuch_redux | 31 (18%) | 3 (2%) | 90% |
| knucleotide | 22 (18%) | 4 (3%) | 82% |
| nbody_newton | 7 (0%) | 0 (0%) | 100% |
