# Fix: i64 Boxing Type Confusion — Phase 0

**Date**: 2026-06-16
**Bug**: LLVM backend generates invalid IR (`ptrtoint i8* %i64`, `trunc i64 %i1 to i8`,
`and i1 %i64, %i1`) due to `TypedRegister.ty` being out of sync with the actual LLVM
register type.

## Root Cause

The backend boxes all native types to `i64` for a uniform ABI (Bool → zext i1 to i64,
Char → zext i32 to i64, String → ptrtoint i8* to i64). The `TypedRegister.ty` field
tracks the **Briv-level type** (Bool, Char, String, etc.), NOT the LLVM type (i64).
When `adapt_to_i64` (or equivalent) is called on a value with `ty == Type::String` but
whose SSA register is already `i64`, it emits `ptrtoint i8* %i64_reg to i64` — invalid.

This manifests in ~18 distinct code paths across 4 files.

## Fix Summary

Four files changed across 18 sites. Each fix ensures that when a value is already
boxed to `i64`, its `TypedRegister.ty` is `Type::Int`, so `adapt_to_i64` passes
through instead of generating a bogus cast.

### 1. `src/backend/llvm/emit_expr.rs` (11 sites)

| Site | Path | Fix |
|------|------|-----|
| SSA old-int-regs `i8*` field | Lines 65-70 | `ptrtoint i8* old_reg to i64` (was `add i64 0, old_reg`) → `Type::Int` |
| SSA extractvalue `i8*` field | Lines 104-108 | `ptrtoint i8* ev to i64` (was `add i64 0, ev`) → `Type::Int` |
| Non-SSA field load (`%fdp`) | Lines 226-229 | `load i8*` then `ptrtoint` to i64 → `Type::Int` |
| `Expr::String`/`Expr::RegexLiteral` | Lines 29-33 | `bitcast` to i8* with `Type::String` (native ptr; `adapt_to_i64` correctly boxes) |
| `Expr::Cast` to String/Data | Lines 1393-1397 | Return `Type::Int` instead of `Type::String`/`Type::Data` |
| `emit_fcmp` | Lines 1836-1842 | `adapt_to_i64` both operands before `icmp` |
| `emit_binop` | Lines 1762-1764 | `adapt_to_i64` both operands before `i64` ops |
| `ListLiteral` element stores | Lines 977-980 | `adapt_to_i64` each element before `store i64` |
| `Tuple` element stores | Lines 1001-1004 | Same as ListLiteral |
| `ReadFile` intrinsic | Lines 484-499 | Box result via `ptrtoint` → `Type::Int` |
| `Spawn`/`SpawnWithOutput` intrinsics | Lines 534-555 | Same as ReadFile |
| `InlineConcat` | Lines 1674-1684 | `inttoptr i64` (was `bitcast i8*`) to handle i64 (boxed) operands |

### 2. `src/features/literal.rs` (2 sites)

| Site | Path | Fix |
|------|------|-----|
| `LiteralExpr::String` | Lines 106-113 | Return `Type::Int` (value is already `ptrtoint`'d to i64) |
| `LiteralExpr::Char` | Lines 117-122 | Return `Type::Int` (value is already `zext`'d to i64) |

### 3. `src/backend/llvm/emit_toplevel.rs` (3 sites)

| Site | Path | Fix |
|------|------|-----|
| `emit_definition` param binding | Lines 516-520 | Store `Type::Int` for Bool/Char/String/Data params |
| `emit_callable_txn` loop param load | Lines 684-686 | Same as emit_definition |
| `emit_init_state` field store | Lines 430-456 | `adapt_to_i64` before `trunc`/`inttoptr` |

### 4. `src/backend/llvm/emit_stmt.rs` (3 sites)

| Site | Path | Fix |
|------|------|-----|
| `param_slots` store | Lines 354-359 | `adapt_to_i64` + `let_binding_types = Type::Int` |
| Guarded field store (`i8`) | Lines 389-396 | `adapt_to_i64` before `trunc i64 to i8` |
| SSA `insertvalue` store | Lines 301-318 | `adapt_to_i64` before `trunc`/`inttoptr` |

## Safety

- **Interpreter**: Unaffected — all changes are in the LLVM backend only
- **Webstack/CIRCT backends**: Unaffected —no changes to shared code
- **Performance**: Zero net change for already-i64 values (the `adapt_to_i64` pass-through is a string clone). For native i1/i32/i8* values, adds one cast instruction per site (the same cast that was previously emitted at a different point in the pipeline)

## Regression Tests

See `src/backend/llvm/tests.rs` for the following test cases added alongside this fix:

- `test_bool_field_store_adapt` — Bool assignment to i8 state field
- `test_bool_param_guard` — Bool param used in guard condition (`[p] { ... }`)
- `test_string_field_load` — String state field load → boxed i64
- `test_string_concat_boxed` — String concatenation with function-returned strings
- `test_string_cast_to_int` — String→Int cast calls `__str_to_int` (regression)
- `test_char_literal_boxed` — Char literal returns Type::Int
- `test_bool_in_tuple` — Bool in Tuple literal boxes to i64
- `test_callable_txn_bool_return` — Callable txn returning Bool boxes correctly
- `test_bool_and_string_guard` — Guard with mixed i1 + i64 Bool operands
