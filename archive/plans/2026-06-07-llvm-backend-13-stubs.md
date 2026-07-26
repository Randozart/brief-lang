# LLVM Backend: Implement 13 Stub Match Arms

**Date**: 2026-06-07
**Status**: Implementation in progress
**Problem**: 13 LLVM backend tests fail because `emit_expr` has no match arms for these `Expr` variants — all fall to `_ => {}`.

## Implementation Order

| # | Group | Tests | Approach |
|---|-------|-------|----------|
| 1 | **ListLiteral / Tuple** | 2 | 2-slot header: `alloca i64, i64 N+2`, store data_ptr (ptrtoint of alloca) to slot 0, length to slot 1, elements to slots 2+ |
| 2 | **ListIndex** | 1 | Load data_ptr from slot 0, GEP + load |
| 3 | **Projection::Size** | 1 | Load length from slot 1 |
| 4 | **StructInstance / ObjectLiteral** | 2 | `alloca i64, i64 N`, store each field, `ptrtoint` |
| 5 | **FieldAccess** | 2 | GEP at field offset in struct, fallback `add i64 0, 0 ; field` |
| 6 | **PatternMatch** | 1 | Load discriminant from slot 0, `icmp eq` against expected disc |
| 7 | **MultiSlice** | 1 | Load data_ptr, GEP at coordinate index |
| 8 | **TupleDestructure** | 1 | Special-case in `emit_statement`: emit tuple, extract elements via GEP+load, bind each name |
| 9 | **Match** | 1 | `switch i64` on discriminant, GEP for field binding, `let_bindings` |
| 10 | **Slice** | 1 | Copy loop with `phi` + `icmp slt` + `br`, allocate new header |

## Key Patterns

- **List/Tuple layout**: `[data_ptr, len, elem0, elem1, ...]` — `data_ptr` = ptrtoint of alloca. Slots = N + 2.
- **Struct/Object layout**: `alloca i64, i64 N`, field offsets match order, ptrtoint returns pointer.
- **Enum discriminant**: Slot 0 = discriminant u64, slots 1..N = payload.
- **TupleDestructure**: Handled in `emit_statement` before generic `Statement::Let`.
- **Additive only**: New match arms before `_ => {}`, no existing paths touched.
