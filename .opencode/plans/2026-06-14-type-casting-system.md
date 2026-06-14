# Type Casting System: `as` and `(Type)expr`

**Date**: 2026-06-14  
**Status**: Plan (to be implemented)

## Syntax

Two casting forms:

| Form | Example | Parser | Status |
|------|---------|--------|--------|
| Postfix `as` | `k as String` | ✅ at parser.rs:5787–5790 | Parser done |
| Prefix `(Type)` | `(String)k` | ❌ new `parse_primary` arm | Needs implementation |

## Conversions

| From | To | Codegen | C helper |
|------|-----|---------|----------|
| `Char`→`String` | Allocate 2-byte buf `[ch, 0]` | LLVM inline | — |
| `String`→`Char` | Load first byte, trunc to 8 bits | LLVM inline | — |
| `Int`→`String` | Format decimal | call `__int_to_str` | new |
| `String`→`Int` | Parse decimal | call `__str_to_int` | new |
| `Int`→`Float` | `sitofp i64 to float` | LLVM inline | — |
| `Float`→`Int` | `fptosi float to i64` | LLVM inline | — |
| `Char`→`Int` | `zext i8 to i64` | LLVM inline | — |
| `Int`→`Char` | `trunc i64 to i8` | LLVM inline | — |
| `String`→`String` | identity (no-op) | — | — |
| `Char`→`Char` | identity (no-op) | — | — |
| Others | type error | typechecker rejects | — |

## Implementation Order

| Step | Layer | Files | What |
|------|-------|-------|------|
| 1 | Parser | `parser.rs` | Detect `(Type)expr` in `parse_primary`: `LParen` + built-in type token + `RParen` → parse inner expr → emit `Expr::Cast` |
| 2 | Typechecker | `typechecker.rs` | Add `Expr::Cast(inner, target_ty)` in `infer_expression`: infer `inner` type, validate conversion compatibility, return `target_ty` |
| 3 | Interpreter | `interpreter.rs` | Add `Expr::Cast(inner, _)` in `eval_expr`: match on (inner_type, target_type) pairs, implement actual conversion |
| 4 | C runtime | `brief_rt.c` | Add `__int_to_str(i64) → i8*`, `__str_to_int(i8*) → i64`, `__chr_to_str(i32) → i8*` |
| 5 | LLVM backend | `emit_expr.rs` | Add `Expr::Cast` arm + update `emit_cast_convert` for all conversion pairs |
| 6 | Tests | `tests.rs`, parser tests | Add tests for both syntaxes and all type pairs |
| 7 | LLVM declares | `mod.rs` | Declare C helper functions |
