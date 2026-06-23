# LLVM Backend Memory Management

**Date:** 2026-06-23
**Status:** Current

## Principle

The LLVM backend's memory strategy is: **%State lives on the stack; heap
allocation is the exception, not the rule.** By keeping the entire program
state in a single `alloca %State` at `main()` entry, LLVM's `mem2reg` +
SROA can promote every field to an SSA virtual register, eliminating
memory traffic entirely for the common case. Heap allocation (`malloc`)
is reserved exclusively for runtime-sized dynamic structures (collections,
strings, enum variants).

## 1. Stack-Allocated State

Every `main()` entry allocates `%State` via `alloca`:

```
%state = alloca %State, align 8
```

Sources:
- `src/backend/llvm/loop_engine.rs:553` (folded main)
- `src/backend/llvm/loop_engine.rs:606` (folded memory main)
- `src/backend/llvm/loop_engine.rs:678` (SSA main)
- `src/backend/llvm/loop_engine.rs:1285` (pure counter)
- `src/backend/llvm/emit_toplevel.rs:1374` (init_state body)

Because `%State` is stack-allocated, LLVM's `mem2reg` can promote fields
to SSA virtual registers. The `noalias nocapture` attribute on `%State*`
parameters (`mod.rs:419`) ensures LLVM sees no aliasing, enabling GVN
and LICM.

### Three SROA Paths

The backend selects a codegen strategy based on body structure:

| Path | Condition | Strategy |
|------|-----------|----------|
| **A005a** (SSA insertvalue) | Straight-line or provably linear body | `extractvalue`/`insertvalue` chain on a single `%State` SSA register. LLVM's SROA decomposes to scalars. `loop_engine.rs:362,412-461` |
| **A005b** (Memory GEP) | Non-linear body with branching guards | Per-field `GEP` + `load`/`store`. Counter uses phi induction variable. `loop_engine.rs:571-659` |
| **A005c** (Pure counter) | Compile-time constant bound, pure body | Single `store i64 <total_value>` into counter, then `ret`. O(1) store. `loop_engine.rs:1282-1292` |

### Pre-extraction

Before the body loop, all float fields are extracted into native float
registers (`pre_extract_float_fields`, `loop_engine.rs:212-226`) and
all Int/Bool/Char/String fields into old-value maps (`pre_extract_int_fields`,
`loop_engine.rs:232-246`). This eliminates the per-reference `extractvalue`
chain that inflated IR by ~5x.

### Per-Field GEP Loading (Memory Path)

`pre_load_all_fields()` (`loop_engine.rs:252-270`) loads ALL state fields
at tick entry via per-field `GEP`:

```
%gep_X = getelementptr inbounds %State, ptr %state, i32 0, i32 <field_idx>
%X_old = load <ty>, <ty>* %gep_X, align <N>, !tbaa !<N>
```

Identifier expressions read from these pre-loaded registers directly
(`emit_expr.rs:64-98`) — zero memory traffic.

## 2. Heap Allocation (Dynamic Structures Only)

`malloc` is used only for runtime-sized data:

| Use | Location | Pattern |
|-----|----------|---------|
| List headers (slice results) | `emit_expr.rs:3314` | `@malloc((len+2) * 8)` — 2-slot header + elements |
| Map/Set literals | `emit_expr.rs:3500,3525` | `@malloc((n+2) * 8)` — header + key-value pairs |
| `<-` arrow push | `emit_expr.rs:3572` | `@free(old)`, `@malloc((len+3) * 8)`, `@llvm.memcpy`, store |
| `<-` arrow pop | `emit_expr.rs:3664` | `@free(old)`, `@malloc((len+1) * 8)`, memcpy before/after |
| `<-` arrow discard | `emit_expr.rs:3740` | Same as pop, element not loaded |
| `<-` arrow transfer | `emit_expr.rs:3825` | Combined buffer `(dest_len+src_len+2)*8` |
| Enum variant construction | `emit_expr.rs:563` | Tagged union via `@malloc` |
| String concat | `emit_expr.rs:4764` | `@malloc(header + total_chars + 1)`, memcpy A + B |

### 2-Slot Header Format

All heap-allocated collections use the same layout:

```
slot 0: data_ptr (i64) — pointer to first element
slot 1: length (i64)
slot 2..N: elements (i64 each)
```

String constants mirror this format: emitted as `<{ i64, i64, [N x i8] }>`
structs with `data_ptr` pointing to the chars field, making static strings
indistinguishable from heap strings at the pointer level (`mod.rs:1641-1649`).

### Embedded Mode Ban

Embedded targets (`.ebv`/`.sebv`) ban all heap allocation. `check_embedded_restrictions()`
(`mod.rs:1059-1099`) warns if any state, let-binding, or expression uses
`Type::String`, `Type::Data`, or any collection type.

## 3. String Concat Optimization

### Detection (`is_string_chain`, `emit_expr.rs:4986-5018`)

Recursively detects if a `+`/Concat expression chain produces a string,
checking: literal strings, identifiers (against type bindings), `Call` results
(against `defn_return_types`), and `Cast` to String/Data.

### Inline Expansion (`emit_inline_concat`, `emit_expr.rs:4728-4836`)

Emits **no runtime library calls**:
1. Mask tag bits (bit 0 = static constant, bit 1 = temporary) from operands
2. Load lengths from header slot 1
3. `@malloc(header_size + total_chars + 1)` — tight packing
4. Write data_ptr, total length into result header
5. `@llvm.memcpy` operand A chars, then operand B chars at offset len_A
6. Null-terminate
7. Check bit 1 of each operand — if set (temporary), `@free` it. Static
   constants (bit 0) and state-owned strings (both bits clear) preserved.
8. Tag result with bit 1 set

## 4. TBAA Metadata

A 6-node TBAA type tree (`mod.rs:448-457`):

```
!0 = !{!"Brief"}        — root
!1 = !{!"Int", !0}      — i64-stored values
!2 = !{!"Bool", !0}     — i1/i8-stored Bool
!3 = !{!"Char", !0}     — i32-stored Char
!4 = !{!"String", !0}   — i8*-stored String
!5 = !{!"Float", !0}    — float-stored Float
```

Annotated on every state field load/store and collection element access
(~80 sites across `emit_expr.rs`, `emit_stmt.rs`, `loop_engine.rs`). Even
though all boxed types are stored as `i64` in `%State`, TBAA lets LLVM
disambiguate accesses by logical type for GVN and load elimination.

## 5. `!range` Metadata

For simple `[x < N]` precondition patterns (`emit_toplevel.rs:1093-1119`),
emits a re-load of field `x` with `!range !{ 0, N }` instead of
`@llvm.assume`:

```
%prl = load i64, i64* %gep, align 8, !tbaa !1, !range !{ 0, 100 }
```

LLVM uses this to infer `nuw`/`nsw` on arithmetic. Complex patterns fall
back to `@llvm.assume`.

## 6. Dead-Field Elimination

`apply_field_modes()` (`mod.rs:2622-2696`) runs after the transition graph
is built:

1. **Assign modes**: each field is `Always`, `LazyCached`, or `Never`
2. **Always**: triggers, cell fields, param slots — unconditionally kept
3. **Never**: physically removed from `field_index_map` and `field_types` —
   `%State` struct shrinks
4. **LazyCached**: appended cache slots (one `i64` for cached value + one `i8`
   valid flag per projection target). Computed lazily via `try_cached_projection()`
   (`emit_expr.rs:5213-5272`) — load valid flag → branch → hit loads cache,
   miss computes, stores, sets valid, phi merge.

Driven by `live_fields` from the transition graph (`mod.rs:1344-1346`).

## 7. Projection Fast-Path

`try_projection_fast_path()` (`emit_expr.rs:5023-5208`) emits native LLVM IR
for 45+ `UserDefinedWithArg` operator/type pairs (`Add`, `Sub`, `Mul`, `Div`,
`Eq`, `Ne`, `Lt`, `Le`, `Gt`, `Ge`, `BitAnd`, `BitOr`, etc.):

- **`Type::Int`**: `add`/`sub`/`mul`/`sdiv`/`icmp` + `zext`
- **`Type::Float`**: `fadd`/`fsub`/`fmul`/`fdiv`/`fcmp` + `zext`
- **`Type::Bool`**: `and`/`or`/`icmp eq`/`icmp ne`

No boxing through i64. Called from the projection dispatch at
`emit_expr.rs:2768`.

## 8. No LLVM Struct Types

User-defined structs are never emitted as LLVM `%MyStruct = type { ... }`.
`struct_types` (`mod.rs:661`) stores field metadata for offset arithmetic only.
`FieldAccess` uses raw `getelementptr i64, i64* %base, i64 <offset>`
(`emit_expr.rs:2700-2722`). `StructInstance` allocates `alloca i64, i64 N` and
stores at computed GEP offsets. This avoids LLVM struct-type rigidity and keeps
SROA decomposition trivial.

## 9. Instruction Reordering

`reorder.rs` builds a dependency DAG (RAW/WAW/WAR dependencies from statement
read/write sets) and applies Kahn's topological sort to group independent
statements for maximum ILP. Terminators are always placed last. Bodies with
< 3 statements skip reordering. Cycle detection falls back to original order.

## 10. SLP Hazard Analysis

`hazard.rs` prevents SLP vectorization when register pressure would exceed
hardware capacity:

- `compute_peak_live_floats()` — interval analysis for peak register demand
- `target_hardware()` — maps target to (register_count, vector_width):
  AVX512 = 32/16, AVX2 = 16/8, NEON = 32/4, SSE = 16/4
- Disables SLP when peak demand ≥ available registers, or when
  ops-per-field ratio < 1.5 (too many shuffles for too few ops)
- `optimal_unroll_factor()` selects 1, 4, or 8 based on pressure

## 11. Native Type Mapping

`TypedRegister::llvm()` (`mod.rs:179-188`) maps each Brief type to its
native LLVM type:

| Brief | LLVM |
|-------|------|
| `Bool` | `i1` |
| `Char` | `i32` |
| `Int` | `i64` |
| `Float` | `float` |
| `String` | `i8*` |

This avoids boxing everything to `i64`, enabling native register operations.
Float register caching (`emit_toplevel.rs:166-189`) prevents redundant
`trunc`+`bitcast` sequences for boxed→native float conversion.

## 12. Constant Deduplication

Constant globals are deduplicated by value (`mod.rs:1538-1627`). Identical
constants map to the same global via `@alias`, reducing cache line pressure.

## Summary

| Technique | File:Line |
|-----------|-----------|
| `%State` alloca (stack) | `loop_engine.rs:553,606,678,1285`; `emit_toplevel.rs:1374` |
| Pre-extraction (float/int fields) | `loop_engine.rs:212-246` |
| Pre-load all fields (GEP) | `loop_engine.rs:252-270` |
| SSA insertvalue chain (A005a) | `loop_engine.rs:362,412-461` |
| Memory GEP path (A005b) | `loop_engine.rs:571-659` |
| Pure counter fold (A005c) | `loop_engine.rs:1282-1292` |
| `malloc` for collections | `emit_expr.rs:3314,3500,3525` |
| `<-` arrow push/pop/discard/transfer | `emit_expr.rs:3546-3890` |
| Enum malloc | `emit_expr.rs:563` |
| String concat inline | `emit_expr.rs:4728-4836` |
| `is_string_chain()` detection | `emit_expr.rs:4986-5018` |
| TBAA tree | `mod.rs:448-457` |
| `!range` metadata | `emit_toplevel.rs:1093-1119` |
| Dead-field elimination | `mod.rs:2622-2696` |
| Cache slots (Hot Dual) | `emit_expr.rs:5213-5272` |
| Projection fast-path (45+ pairs) | `emit_expr.rs:5023-5208` |
| No LLVM struct types (raw GEP) | `emit_expr.rs:2700-2722` |
| Instruction reordering (ILP) | `reorder.rs` |
| SLP hazard analysis | `hazard.rs` |
| Native type mapping | `mod.rs:179-188` |
| Float register caching | `emit_toplevel.rs:166-189` |
| Constant deduplication | `mod.rs:1538-1627` |
