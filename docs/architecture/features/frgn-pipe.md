# Pipe-Separated Fallback Syntax for `frgn` Declarations

**Date:** 2026-06-18  
**Phase:** 10.5  
**Status:** Implemented (interpreter + tests), LLVM backend partial (raw pass-through)

## Syntax

```briev
frgn name(args) -> T | fallback_expr ;
frgn name(args) -> T | fallback_expr from "lib.so" ;
```

The pipe `|` separates the expected return type T from a compile-time-evaluable
fallback expression. Coexists with `Result<T, E>` syntax — both are valid.

## Semantics

The pipe syntax declares a raw C ABI foreign function. On call:

1. The raw FFI function is invoked via native call or `dlopen`
2. The return value is validated against type T using sentinel conventions:
   - `String`/`Data`: null pointer → invalid
   - `Float`: NaN/Inf → invalid
   - `Int`/`UInt`/`Bool`/`Char`: always valid
3. If valid → return `Ok(value)`
4. If invalid → evaluate the fallback expression, return `Err(fallback_value)`
5. Caller sees `Result<T, typeof(fallback_expr)>` — a sum type

No TOML binding is required. The fallback replaces the error value entirely.

## Typechecking

- Pipe frgns skip TOML binding validation (`check_frgn_binding` returns early)
- Fallback expression is validated as compile-time evaluable by
  `is_compile_time_expr`:
  - Allowed: literals (`""`, `0`, `false`), constructor calls with literal
    args (`CustomError("msg")`), tuples and lists of same
  - Rejected: variable/function references (`Identifier`, `OwnedRef`,
    `PriorState`), operator expressions, blocks
- The diagnostic `T001` ("FFI call returns Result type") is emitted for
  pipe frgn call sites, hinting `Ok(val) = func()` pattern

## Evaluation

### Interpreter

Two dispatch paths:

1. **Native FFI** (`ffi_name_to_location`): `CallExpr::evaluate` detects
   `sig.is_pipe` and calls `call_pipe_frgn(fn_name, raw_value)` directly
   after the FFI function returns.

2. **Dynamic FFI** (`frgn_registry`): Before `frgn_registry.call()`, the
   pipe interceptor unwraps the registry's `Ok(raw)` wrapping, passes the
   raw value through `call_pipe_frgn`, and re-wraps in `Ok`/`Err` with
   the fallback.

`call_pipe_frgn` performs sentinel validation via `is_valid_ffi_return`:
- Float: `f.is_finite()` — NaN and Inf rejected
- String/Data: always valid at interpreter level (null never reaches here)
- Int/Bool/Char: any value valid
- Complex types (List, Instance, Enum, etc.): always valid

On success: `Value::Enum("Result", "Ok", {"value": raw})`  
On failure: `Value::Enum("Result", "Err", {"value": fallback})`

### LLVM Backend

Currently a raw pass-through — the call result is returned as-is without
sentinel checking or Result enum construction. Full implementation requires:

1. Null-pointer check for `i8*` returns (String/Data)
2. NaN check (`fcmp uno`) for float returns
3. Branch to fallback evaluation + phi merge
4. Result enum (`{discriminant, payload}`) construction

## Files

| File | Role |
|------|------|
| `src/ast.rs` | `is_pipe: bool`, `fallback: Option<Expr>` on `ForeignSignature` |
| `src/parser.rs` | Pipe parsing after return type in `parse_frgn_binding` |
| `src/typechecker.rs` | Skip TOML check, `is_compile_time_expr` validation, T001 diagnostic |
| `src/features/call.rs` | Pipe dispatch in native and dynamic FFI paths |
| `src/interpreter.rs` | `call_pipe_frgn()`, `is_valid_ffi_return()` |
| `src/ffi/validator.rs` | Early return for `is_pipe` |
| `src/backend/llvm/emit_expr.rs` | Raw pass-through (TODO: full implementation) |

## Tests

| Test | File | Coverage |
|------|------|----------|
| `test_parse_frgn_pipe_literal` | `parser.rs` | `frgn f() -> String \| ""` |
| `test_parse_frgn_pipe_constructor` | `parser.rs` | `frgn f() -> String \| Error("msg")` |
| `test_parse_frgn_pipe_with_from` | `parser.rs` | `frgn f() -> Int \| 0 from "lib.so"` |
| `test_parse_frgn_pipe_does_not_break_plain` | `parser.rs` | Plain `-> String` still works |
| `test_parse_frgn_pipe_does_not_break_result` | `parser.rs` | `Result<T,E>` still works |
| `test_frgn_pipe_registers_signature` | `typechecker.rs` | Pipe sig stored/retrieved |
| `test_frgn_pipe_skips_toml_validation` | `typechecker.rs` | No error for pipe without TOML |
| `test_is_compile_time_expr_*` (9 tests) | `typechecker.rs` | Compile-time expr validation (literals, constructors, identifiers rejected) |
| `test_is_valid_ffi_return_*` (6 tests) | `interpreter.rs` | Per-type sentinel validation |
| `test_call_pipe_frgn_*` (4 tests) | `interpreter.rs` | Ok/Err wrapping for valid/invalid |
| `test_pipe_frgn_integration_*` (2 tests) | `interpreter.rs` | Full dispatch through `Expr::Call` with mock FFI |
| `test_llvm_pipe_frgn_*` (4 tests) | `backend/llvm/tests.rs` | LLVM IR: declare, null check, NaN check, no-sentinel for Int |

## Future Work

- LLVM backend: full sentinel check + Result enum construction
- Contract-bounds as sentinel (e.g., `frgn f() -> Int | 0 [result > 0]`)
