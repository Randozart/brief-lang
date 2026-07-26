# LLVM Backend — Struct Instance + FieldAccess + Intrinsic Fixes

**Date:** 2026-06-13
**Status:** In Progress

## Goal

Make the LLVM backend compile officina-cli (a multi-module Brief application with structs,
lists, FFI, match, collections, and imports). Three specific fixes needed.

## Background

Contrary to the AGENTS.md "Backend Gaps" table, the LLVM backend already has working
implementations for:
- Tuple / TupleDestructure (alloca + GEP + load for N+2-slotted tuple header)
- StructInstance (alloca + GEP + store + ptrtoint for flat i64 array)
- FieldAccess on Expr::Identifier objects (GEP + load into struct field index)

The three stubs that actually block officina-cli are:

1. **StructInstance returns Type::Int**, not Type::Custom(struct_name). The ptrtoint value
   is correct, but the returned TypedRegister has `ty: Type::Int`. This means any code that
   depends on the return type — like FieldAccess trying to identify the object as a struct —
   won't match.

2. **FieldAccess only checks Expr::Identifier**. When the object is an expression like
   `records[i]` (a ListIndex) or `get_struct()` (a Call), the only check is
   `if let Expr::Identifier(name) = obj`. Non-Identifier objects fall through to
   `add i64 0, 0 ; field`.

3. **Intrinsic::ReadFile returns add i64 0, 0**. The officina-cli reads `system/understands.dbv`
   at boot via `read_file#(...)`. Without real file I/O, all NLU rules are empty.

4. **Intrinsic::Time returns add i64 0, 0**. Timestamps in history records are wrong.

## Changes

### Fix 1 — StructInstance return type (1 line)
File: `src/backend/llvm/emit_expr.rs`, after line 512
Add `return TypedRegister { name: v, ty: Type::Custom(name.clone()) };`

### Fix 2 — FieldAccess TypedRegister fallback (~10 lines)
File: `src/backend/llvm/emit_expr.rs`, lines 534-546
After the Expr::Identifier check fails, fall back to checking obj_val.ty:
```rust
if !found_offset {
    if let Type::Custom(struct_name) = &obj_val.ty {
        if let Some(fields) = self.struct_types.get(struct_name) {
            for (fi, (fn_, _)) in fields.iter().enumerate() {
                if fn_ == field { offset = fi as i64; found_offset = true; break; }
            }
        }
    }
}
```

### Fix 3 — read_file# intrinsic via brief_rt.c
File: `lib/runtime/brief_rt.c` — add `brief_read_file` function
File: `src/backend/llvm/emit_expr.rs` — emit declare + call instead of stub

The read_file#(path: String) → String intrinsic:
- Declare `declare i64 @brief_read_file(i64)` in LLVM IR header
- Call it with the path argument
- Return the result as a Brief String pointer

### Fix 4 — time# intrinsic via libc time(null)
File: `src/backend/llvm/emit_expr.rs` — emit declare + call to libc time

## Verification

1. `cargo test --lib` — all existing tests pass
2. `brief check officina.bv` — 0 errors
3. `brief build officina.bv` — produces binary
4. Binary runs without crash
