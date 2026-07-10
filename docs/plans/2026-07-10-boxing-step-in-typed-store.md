# Fannkuch Remaining Gap: i64 Boxing Step in emit_typed_store

## Root Cause

`emit_typed_store` skips the `adapt_to_i64` boxing step that
`emit_memory_field_store` always performs for non-float fields. This
changes LLVM's SSA phi placement and register allocation, producing a
~35% performance difference vs the old codegen path.

## The Boxing Step

`emit_memory_field_store` for integer fields:

```llvm
%tN = add i64 0, %val       ; adapt_to_i64 — SSA register copy
%tM = trunc i64 %tN to i32  ; ensure_typed_value — type adaptation
store i32 %tM, ptr %gep     ; store to state field
```

`emit_typed_store` (current):

```llvm
%tM = trunc i64 %val to i32  ; ensure_typed_value — type adaptation
store i32 %tM, ptr %gep     ; store to state field
```

Missing the `add i64 0, %val` step. This extra register copy changes
LLVM's phi node placement, register allocation pressure, and ultimately
loop optimization. The effect is a ~35% slowdown on multi-field loops
like fannkuch_redux.

## Why It's Safe

The boxing step for integer fields is purely cosmetic — a fresh SSA
register copy that affects LLVM's internal optimization decisions
(phi placement, coalescing, register allocation) without changing
the instruction semantics.

For float/double fields, the boxing step IS skipped (as fixed in
Phase 7), because boxing floats through i64 reproduces the
`float → i32 → i64 → i32 → float` bitcast chain that LLVM handles
poorly. Native float store is faster.

For bool/char fields, `adapt_to_i64` handles the i1→i64 zext, then
`ensure_typed_value` does the i64→i1 trunc for the store. This is the
same path `emit_memory_field_store` uses.

## Fix

In `emit_typed_store`, for non-float types, call `adapt_to_i64` on the
value before calling `ensure_typed_value`:

```rust
let is_native_float = ty_str == "float" || ty_str == "double";
let val_for_store = if is_native_float {
    val.name.clone()
} else {
    self.adapt_to_i64(out, indent, val)
};
let tv = self.ensure_typed_value(out, indent, &ty, &val_for_store, brief_ty, Some(&val.ty));
```

This produces IR structurally identical to `emit_memory_field_store`,
closing the remaining performance gap.

## Why This Doesn't Reintroduce the Float Boxing Bug

In Phase 7, I fixed `native_float_or_box` to check the source type
(TypedRegister.ty). When the source register is already native float,
skip the `trunc i64 → bitcast` dance. The `adapt_to_i64` call here
is gated on `!is_native_float`, so floats skip boxing entirely.
