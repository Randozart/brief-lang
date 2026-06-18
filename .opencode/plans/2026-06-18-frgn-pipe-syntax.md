# Frgn Pipe-Separated Fallback Syntax

**Date**: 2026-06-18
**Status**: Planned

## Summary

Add a pipe-separated fallback syntax to `frgn` declarations as an alternative
to `Result<T, E>` with TOML bindings:

```brief
frgn name(args) -> T | fallback_expr ;
```

The pipe syntax provides a fallback VALUE (evaluated at compile time) that the
system returns when the raw FFI return does not constitute a valid value of
type T (null pointer, type mismatch, out-of-contract-bounds). No TOML binding
is required.

## Semantics

```
frgn read_file(path: String) -> String | "" ;
```

1. The FFI function is called as a raw C ABI call (no TOML binding lookup)
2. The raw return value is checked for validity against type T:
   - `String`/`Data`/`Ptr<T>`: null pointer -> invalid
   - `Int`/`UInt`: always valid (no sentinel), unless contract bounds constrain
   - `Bool`: any i64 value is valid (0/1 are normal, others are truthy)
   - `Float`: NaN/Inf -> invalid
   - Struct/union: discriminant check against expected variant
3. If the raw value is valid for type T -> `Ok(value as T)`
4. If invalid -> `Err(fallback_value)` where `fallback_value` is the evaluated expression
5. The caller sees a `Result<T, typeof(fallback_expr)>` — a sum type they must `match` on

**Key differences from `Result<T, E>` with TOML:**

| Aspect | `Result<T, E>` (TOML) | `T \| fallback` (pipe) |
|--------|----------------------|----------------------|
| Error type | TOML binding defines error_fields | fallback expression's type |
| Error value | Structured from FFI response | Fixed at compile time |
| TOML required | Yes | No |
| Error detection | error_fields non-empty | Null ptr / NaN / type mismatch / contract bounds |
| Caller sees | `Result<T, E>` | `Result<T, typeof(fallback)>` |

## Syntax

```ebnf
frgn_decl ::= ("frgn" | "syscall") name params "->" type ("|" fallback_expr)? ("from" location)? ";" ;
```

Both pipe and `from` are optional:
- `frgn f(x: Int) -> String | "" ;` — pipe with fallback, no from
- `frgn f(x: Int) -> String | "" from "libc.so" ;` — pipe with from
- `frgn f(x: Int) -> Result<String, IoError> from "std::fs::read_to_string" ;` — traditional
- `frgn f(x: Int) -> String ;` — plain return (no error handling)

The fallback expression must be compile-time evaluable: literals (`""`, `0`,
`false`, `null`) or constructor calls with literal args (`CustomError("msg")`).

## Implementation

### 1. AST (`src/ast.rs`)

Add to `ForeignSignature`:

```rust
pub struct ForeignSignature {
    // ... existing fields ...
    pub fallback: Option<Expr>,        // NEW: fallback expression
    pub is_pipe: bool,                 // NEW: true if pipe syntax used
}
```

When `is_pipe` is true, `error_type_name` and `error_fields` are empty — the
fallback supplies the error value directly. `result_type` stores the success
type T; the fallback expression's type is inferred.

### 2. Parser (`src/parser.rs`)

In `parse_frgn_binding()`, after parsing the return type (the `Result<T,E>` or
plain type path), check for `|`:

```rust
// After parsing return type -> success_output
let mut fallback = None;
let mut is_pipe = false;
if let Some(Ok(Token::Pipe)) = self.current_token() {
    self.advance();
    is_pipe = true;
    fallback = Some(self.parse_expression()?);
}
```

The `|` token already exists in the lexer. It's unambiguous in `frgn` context
because it follows a type (never a binary-op position). Store on the signature.

When `is_pipe` is true, the success_output still stores the success type (T),
but the function is recorded as pipe-style.

### 3. Typechecker (`src/typechecker.rs`)

**Inference**: When a call references a pipe frgn, the return type is
`Result<T, fallback_type>` where:
- T = `success_output`'s type
- fallback_type = inferred from `fallback` expression

**Validation**:
- Fallback expression must be compile-time evaluable (reject function calls,
  variable references, runtime-dependent expressions)
- If fallback is a constructor call, the constructor must be in scope
- No TOML binding lookup — skip `check_frgn_binding` for pipe frgns

**In `check_frgn_binding`**: Skip validation if `is_pipe` is true:
```rust
if signature.is_pipe {
    return; // No TOML validation needed
}
```

### 4. Interpreter (`src/features/call.rs` + `src/interpreter.rs`)

**New dispatch path** in `CallExpr::evaluate`:

```rust
// After checking definitions, callable_txns, etc.
if let Some(sig) = ctx.ffi_bindings.get(fn_name) {
    if sig.is_pipe {
        return ctx.call_pipe_frgn(fn_name, sig, &arg_values);
    }
}
```

**`call_pipe_frgn` method** (new, in `interpreter.rs`):

```rust
fn call_pipe_frgn(
    &mut self,
    fn_name: &str,
    sig: &ForeignSignature,
    args: &[Value],
) -> Result<Value, RuntimeError> {
    // 1. Call the FFI function (raw C ABI)
    let raw_result = self.call_raw_frgn(fn_name, args)?;

    // 2. Validate the raw result against the expected type T
    let success_type = sig.success_output.first()
        .map(|(_, t)| t)
        .unwrap_or(&Type::Void);

    if self.is_valid_ffi_return(&raw_result, success_type) {
        // Valid -> Ok(raw_result)
        Ok(ctx.wrap_ok(raw_result))
    } else {
        // Invalid -> evaluate fallback expression -> Err(fallback)
        let fallback_val = ctx.eval_expr(sig.fallback.as_ref().unwrap())?;
        Ok(ctx.wrap_err(fallback_val))
    }
}
```

**`is_valid_ffi_return`** — sentinel-based validation:

| Type T | Valid if | Invalid if |
|--------|----------|------------|
| `String` / `Data` | `Value::String(s)` where ptr non-null | `Value::String(s)` where ptr null |
| `Int` / `UInt` | Always valid (any i64) | Never (or if contract bounds exist and are violated) |
| `Bool` | Always valid | Never |
| `Float` | `Value::Float(f)` where `f.is_finite()` | `NaN` or `Inf` |
| `Ptr<T>` | `Value::Ptr(p)` where p non-null | `Value::Ptr(null)` |
| `List<T>` | `Value::List(_)` | Any non-list |

### 5. LLVM Backend (`src/backend/llvm/emit_expr.rs`)

**New code path** for pipe frgn calls:

1. Emit the raw C ABI call
2. Emit a null/sentinel check on the return value
3. Branch:
   - Valid -> construct `Ok` enum with the value
   - Invalid -> evaluate fallback expression (constant), construct `Err` enum with it
4. Use existing `emit_enum_constructor` for `Ok`/`Err` wrapping

For pointer types (String/Data):
```llvm
%raw = call i8* @some_fn(i64 %arg)
%is_null = icmp eq i8* %raw, null
br i1 %is_null, label %err, label %ok

ok:
  ; construct Ok(%raw)
  ...

err:
  ; construct Err("") — evaluate fallback, wrap in Err
  ...
```

For float:
```llvm
%raw = call float @some_fn(i64 %arg)
%is_nan = fcmp uno float %raw, %raw
br i1 %is_nan, label %err, label %ok
```

For contract-bounded Ints (with `[result > 0]`):
```llvm
%raw = call i64 @some_fn(i64 %arg)
%in_bounds = icmp sgt i64 %raw, 0
br i1 %in_bounds, label %ok, label %err
```

### 6. TOML Validation (`src/ffi/validator.rs`)

Skip validation entirely when `signature.is_pipe == true`:

```rust
pub fn validate_frgn_against_binding(
    signature: &ForeignSignature,
    binding: &ForeignBinding,
) -> Result<(), FfiError> {
    if signature.is_pipe {
        return Ok(()); // Pipe frgns don't use TOML bindings
    }
    // ... existing validation ...
}
```

### 7. Backend `declare` emission (`src/backend/llvm/mod.rs`)

For pipe frgns, the LLVM `declare` uses the success type T as the C return
type (not wrapped in Result — the wrapping happens in Brief-level emitted
code). No change needed to the declare emission; the `result_type` field
still holds `Projection([T])`.

## Order of Implementation

1. **AST** — add `fallback: Option<Expr>`, `is_pipe: bool` to `ForeignSignature`
2. **Parser** — parse `|` + expression, store on signature
3. **Typechecker** — pipe frgns skip TOML validation; return type is `Result<T, fallback_type>`
4. **Interpreter** — new `call_pipe_frgn` dispatch path with sentinel-based validation
5. **LLVM Backend** — null/sentinel check + conditional branch to fallback
6. **Tests** — unit tests for parser, typechecker, interpreter, LLVM

## Tests

| Test | File | What it covers |
|------|------|---------------|
| `test_parse_frgn_pipe` | `parser.rs` | `frgn f(x: Int) -> String \| ""` parses correctly |
| `test_parse_frgn_pipe_constructor` | `parser.rs` | `frgn f() -> String \| Error("msg")` parses |
| `test_parse_frgn_pipe_no_fallback` | `parser.rs` | Plain `-> String` still works |
| `test_frgn_pipe_inference` | `typechecker.rs` | Return type is `Result<String, String>` |
| `test_frgn_pipe_no_toml` | `typechecker.rs` | Pipe frgn skips TOML check |
| `test_frgn_pipe_interp_success` | `interpreter.rs` | Valid FFI return -> `Ok(value)` |
| `test_frgn_pipe_interp_fail_null` | `interpreter.rs` | Null string -> `Err("")` |
| `test_frgn_pipe_interp_fail_nan` | `interpreter.rs` | NaN float -> `Err(0.0)` |
| `test_frgn_pipe_interp_callable` | `interpreter.rs` | `match` on pipe frgn return works |

## Open Questions (Runtime)

1. **Int sentinel**: Should non-null-terminated Int ever be considered
   "invalid"? Currently: no — any i64 value is a valid Int. Contract
   bounds `[result > 0]` can constrain it if present.

2. **Bool sentinel**: Any i64 is valid as Bool (0 = false, non-zero = true).
   No invalid case.

3. **String null check details**: At the FFI boundary, a C function returning
   `char*` returns null on error. The Brief runtime wraps this in
   `Value::String`. We must distinguish "null pointer" from "empty string"
   (which is `""` — a valid pointer to a null terminator).

4. **Interaction with `from`**: For pipe frgns, `from` specifies a shared
   library name (e.g., `from "libm.so"`), not a TOML path. The resolver must
   distinguish these cases.
