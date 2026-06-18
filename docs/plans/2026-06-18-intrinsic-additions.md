# Intrinsic Additions: `sin#`, `cos#`, `pow#`, `float_to_str#`, `to_str#`

**Date:** 2026-06-18  
**Status:** Planned  

## Motivation

The `docs/learn/macros.md` tutorial used example intrinsics that don't exist
(`math#.sin()`, `float_to_str()`, `sys#()`). This plan adds the useful ones
and fixes the tutorial.

## Additions

| Intrinsic | Signature | Purity | Priority |
|-----------|-----------|--------|----------|
| `Sin` | `sin#(Float) -> Float` | Pure | Medium |
| `Cos` | `cos#(Float) -> Float` | Pure | Medium |
| `Pow` | `pow#(Float, Float) -> Float` | Pure | Medium |
| `FloatToStr` | `float_to_str#(Float) -> String` | Pure | High |
| `ToStr` | `to_str#(Int\|Float\|Char\|Bool) -> String` | Pure | Medium |

## Implementation Steps

### Step 1: AST (`src/ast.rs`)
- Add `Sin`, `Cos`, `Pow`, `FloatToStr`, `ToStr` to the `Intrinsic` enum
- Add to `has_side_effects()` — all pure (return `false`)
- Add to `from_name()` — `"sin"`, `"cos"`, `"pow"`, `"float_to_str"`, `"to_str"`
- Add to `name()` — same strings

### Step 2: Typechecker (`src/typechecker.rs`)
- Add return type entries: `Sin`/`Cos`/`Pow` → `Type::Float`, `FloatToStr` → `Type::String`, `ToStr` → `Type::String`
- For `ToStr`, accept `Int | Float | Char | Bool` and return `String`

### Step 3: Interpreter (`src/interpreter.rs`)
- `Sin`: extract Float, call `.sin()`, return `Value::Float`
- `Cos`: extract Float, call `.cos()`, return `Value::Float`
- `Pow`: extract two Floats, call `.powf()`, return `Value::Float`
- `FloatToStr`: extract Float, format via `format!("{:.9}", n)`, return `Value::String`
- `ToStr`: match input type, convert each variant to String

### Step 4: LLVM Backend (`src/backend/llvm/emit_expr.rs`)
- `Sin`: call `llvm.sin.f32` or `sin` from libm
- `Cos`: call `llvm.cos.f32`
- `Pow`: call `powf`
- `FloatToStr` / `ToStr`: call `snprintf` or similar

### Step 5: Documentation
- Fix `docs/learn/macros.md` — replace bad examples with working intrinsics
- Update `docs/architecture/features/macro.md` with full intrinsic list
