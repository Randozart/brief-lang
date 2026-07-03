# Float-i64 Roundtrip Elimination Plan

## Problem

Every float store in the loop body emits 4 redundant instructions:

```llvm
; box (adapt_to_i64):
bitcast float %val to i32
zext i32 %boxed to i64          ; widen to uniform 64-bit slot

; unbox (ensure_typed_value):
trunc i64 %wval to i32
bitcast i32 %rval to float       ; same value as %val!
store float %rval, ptr %gep
```

30 stores per iteration × 50M iterations = 1.5B roundtrips (6B IR instructions)
that are entirely wasted — the source value is already a native `float`.

## Fix

In `emit_memory_field_store` (`emit_stmt.rs:37-51`), split the store path:

- **Float/double types**: store `val.name` (the native float register) directly,
  skipping `adapt_to_i64` and `ensure_typed_value`. Insert a dummy into
  `pending_phi_backedge` (only the key matters; the latch uses
  `pending_phi_native_backedge` for the actual value).

- **Integer/pointer types**: keep the existing box→store path unchanged.

## Impact

| Benchmark | Current | After | Savings |
|-----------|---------|-------|---------|
| nbody_sqrt | 1.23x | ~1.1x | 120 fewer ops/iteration |
| nbody_sqrt_idio | 1.0x | ~0.95x | Same |
| float_math_nonzero | 2.3x | ~1.8x | High float ratio |
| nbody_newton | 1.5x | ~1.4x | More memory-bound |

## Risk

Low. The native float register is bit-identical to the boxed→unboxed result.
The change only affects the non-volatile float/double store path.  Integer
types, volatile stores, and edge cases (boxed floats from globals) are
unchanged because `val.ty` would be `Type::Int` for boxed values (the float
branch won't trigger).

## Implementation

One function: `emit_memory_field_store` in `emit_stmt.rs`. Add an `if` guard
before the existing `let val_boxed = self.adapt_to_i64(...)` to check for
native float types and take the direct store path.
