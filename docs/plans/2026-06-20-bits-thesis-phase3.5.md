# Phase 3.5 — Backend Fast-Path Registry + TypeUniverse Wiring

**Date:** 2026-06-20  
**Phase:** 3.5 of 6  
**Status:** Complete (1087 tests pass, release build passes)

## Summary

Implemented the backend fast-path registry for well-known operator projections
and wired `TypeUniverse` into the real compiler pipeline. After this phase,
`UserDefinedWithArg("Add", rhs)` and similar projection-based operators resolve
to native code in all three layers (interpreter, typechecker, LLVM backend).

## Changes

### 1. TypeUniverse Wired Into Pipeline (`main.rs`)

`TypeUniverse::build(&program)` is constructed after desugaring/macro expansion
and before the typechecker. Passed to both `TypeChecker` and `LlvmBackend` via
`.with_type_universe(tu)`.

**Files:**
- `src/main.rs:2403-2404` — build TypeUniverse
- `src/main.rs:2415` — pass to TypeChecker
- `src/main.rs:2446` — pass to LlvmBackend

### 2. LLVM Backend Fast-Path (`emit_expr.rs`)

`try_projection_fast_path()` recognizes 45+ well-known (type, operator) pairs
and emits native LLVM IR directly instead of the `add i64 0, 0` stub.

| Type | Operators | LLVM Instrs |
|------|-----------|-------------|
| `Int` | Add, Sub, Mul, Div, Mod | `add/sub/mul/sdiv/srem i64` |
| `Int` | Eq, Ne, Lt, Le, Gt, Ge | `icmp` + `zext i1 to i64` |
| `Int` | BitAnd, BitOr, BitXor, Shl, Shr | `and/or/xor/shl/lshr i64` |
| `Int` | And, Or | `and/or i64` (bitwise, treated as logical in Briv) |
| `Float` | Add, Sub, Mul, Div | `fadd/fsub/fmul/fdiv float` |
| `Float` | Eq, Ne, Lt, Le, Gt, Ge | `fcmp oeq/one/olt/ole/ogt/oge` + `zext` + `sitofp` |
| `Bool` | And, Or | `and/or i1` |
| `Bool` | Eq, Ne | `icmp eq/ne i1` |

**Files:**
- `src/backend/llvm/mod.rs:615` — `type_universe: Option<TypeUniverse>` field
- `src/backend/llvm/mod.rs:798-802` — `.with_type_universe()` builder
- `src/backend/llvm/emit_expr.rs:2601-2607` — split UserDefined/UserDefinedWithArg arms
- `src/backend/llvm/emit_expr.rs:3944-4135` — `try_projection_fast_path()` function

### 3. TypeChecker Projection Resolution (`typechecker.rs`)

`resolve_user_projection_type()` resolves the return type of user-defined
projections by checking well-known operator names first, then falling back
to `TypeUniverse` lookup for user-defined types.

**Well-known type projections:**

| Type | Operator(s) | Return Type |
|------|-------------|-------------|
| `Int` | Add/Sub/Mul/Div/Mod/BitAnd/BitOr/BitXor/Shl/Shr/Neg/BitNot | `Int` |
| `Int` | Eq/Ne/Lt/Le/Gt/Ge/And/Or/Not | `Bool` |
| `Float` | Add/Sub/Mul/Div/Neg | `Float` |
| `Float` | Eq/Ne/Lt/Le/Gt/Ge | `Bool` |
| `Bool` | And/Or/Eq/Ne/Not | `Bool` |
| `Char` | Eq/Ne/Lt/Le/Gt/Ge | `Bool` |

**Files:**
- `src/typechecker.rs:65` — `type_universe: Option<TypeUniverse>` field
- `src/typechecker.rs:118-122` — `.with_type_universe()` builder
- `src/typechecker.rs:2012-2020` — UserDefined/UserDefinedWithArg → `resolve_user_projection_type()`
- `src/typechecker.rs:2480-2520` — `resolve_user_projection_type()` method

### 4. Interpreter Fast-Path (`projection.rs`)

`eval_user_projection_fast_path()` handles well-known operator projections
in the interpreter. Same 45+ (value, operator) pairs as the LLVM backend,
matching on `Value::Int`, `Value::Float`, `Value::Bool` variants.

**Files:**
- `src/features/projection.rs:194-213` — UserDefined arm (handles Neg/Not/BitNot)
- `src/features/projection.rs:215-223` — UserDefinedWithArg fast-path entry
- `src/features/projection.rs:241-322` — `eval_user_projection_fast_path()` function

### Remaining Gaps (Deferred)

| Item | Why deferred |
|------|-------------|
| Webstack UserDefined handler | Falls through to `_ => src` (source unchanged) — acceptable |
| TypeUniverse generic projection evaluation (interpreter) | Requires binding expression evaluation with `_` = source — complex |
| TypeUniverse generic projection codegen (LLVM) | Requires compiling binding expressions inline — complex |

## Testing

- `cargo test --lib` — 1087 passed, 0 failed
- `cargo build --release` — builds without errors
- No regression in existing benchmarks (no benchmark code uses UserDefined projections yet)
