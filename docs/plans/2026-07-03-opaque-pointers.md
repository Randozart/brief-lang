# Opaque Pointer Migration Plan

## Motivation

The backend currently emits typed pointers (`float*`, `i64*`, `i8*`, `double*`).
LLVM 18 requires opaque pointers (`ptr`). The `opt` auto-upgrade pass converts
typed→opaque, but mixing both styles in the same `.ll` file confuses SROA:

- Builder methods emit `ptr` (opaque)
- `writeln!` calls emit `float*` etc. (typed)
- `%State` struct has typed GEPs (`i64* %gep`) and opaque GEPs (`ptr %gep`)
  in the same function
- SROA sees `%State` referenced by both pointer styles and conservatively
  refuses to decompose it

After the migration, ALL pointers use `ptr`. SROA sees uniform `ptr` references
and can decompose `%State` into scalar float/integer phis. The loop vectorizer
then sees individual `float` values instead of one big struct — enabling
vectorization for nbody and similar benchmarks.

## Scope

7 files, ~7000 lines total. Every `{type}*` in IR emission must become `ptr`.

| File | Lines | Pointer uses |
|------|-------|--------------|
| `loop_engine.rs` | 2700 | load, store, GEP, alloca |
| `emit_stmt.rs` | 1040 | load, store, GEP, alloca |
| `emit_toplevel.rs` | 2152 | load, store, GEP, alloca |
| `mod.rs` | 740 | load, store, GEP, alloca |
| `emit_expr.rs` | 500 | load, store, GEP, alloca |
| `helpers.rs` | 1833 | load, store, GEP, alloca |
| `builder.rs` | 738 | ALREADY uses `ptr` (no changes needed) |

## Pattern to change

Before (typed pointer):
```
store float %val, float* %ptr, align 4
  → remove "float* " → "ptr "
load i64, i64* %ptr, align 8
  → remove "i64* " → "ptr "
getelementptr inbounds %State, %State* %state, i32 0, i32 1
  → remove "%State* " → "ptr "
alloca %State, align 8
  → already uses ptr (alloca_typed in builder, alloca in writeln)
```

After (opaque pointer):
```
store float %val, ptr %ptr, align 4
load i64, ptr %ptr, align 8
getelementptr inbounds %State, ptr %state, i32 0, i32 1
```

The key insight: **the type before the pointer is duplicated in the instruction**.
`store float %val, float* %ptr` — the `float` before `%val` already tells LLVM
the type. The `float*` before `%ptr` is redundant. Removing it to `ptr` loses
no information.

## Mechanical replacement table

| Match | Replace | Notes |
|-------|---------|-------|
| `, float*` | `, ptr` | Load/store operand pointer |
| `, i64*` | `, ptr` | Load/store operand pointer |
| `, i8*` | `, ptr` | Load/store operand pointer |
| `, i32*` | `, ptr` | Load/store operand pointer |
| `, double*` | `, ptr` | Load/store operand pointer |
| `, i16*` | `, ptr` | Load/store operand pointer |
| `%, i8**` | `%, ptr` | Store pointer-to-pointer |
| `%State*` | `ptr` | GEP base type |

## Verification

```bash
cargo test --lib                                 # all tests pass
opt -O3 -pass-remarks-missed=sroa nbody_sqrt.ll  # no SROA remarks
```