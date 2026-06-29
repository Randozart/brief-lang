# String Initialization Fix: Null → Empty String Sentinel

Date: 2026-06-29

## Root Cause

State fields of type `String` declared without an initializer (e.g., `let inp: String;`) get `store i8* null` instead of a pointer to the empty string sentinel `@str.0`.

When a reactive tick loop's guard check determines `inp_id != last_id`, the runtime tries to process the input string. It calls `trim(inp)` which compares the input against `@str.0` (the empty string sentinel at address `0x40a048`). Since the input is null (0), not the sentinel address, the check `null == @str.0 ?` fails, and the code proceeds to dereference the null pointer → SIGSEGV.

## Root Cause Details

The `None` branch in both `emit_init_state` and `emit_inline_init_stores` at `src/backend/llvm/emit_toplevel.rs` generates:

```rust
let default = if ty == "i8*" { "null".to_string() } else { "0".to_string() };
```

This does `store i8* null` for uninitialized `i8*` (String) fields. The fix changes this to store `@str.0` (the empty string sentinel) for `i8*` fields.

Additionally, `loop_engine.rs` has a bug where even `Some(Expr::String(...))` initializers produce `i8* null` instead of the actual string constant.

## Changes

### 1. `emit_toplevel.rs:595-598` — `emit_init_state` None branch

Replace `i8* null` with `bitcast @str.0 to i8*` for uninitialized String fields.

### 2. `emit_toplevel.rs:789-791` — `emit_inline_init_stores` None branch

Replace `i8* null` with tagged `@str.0` (with `OR 1` tag bit) for uninitialized String fields, matching the pattern used by `Expr::String("")` initializers.

### 3. `loop_engine.rs:600-604` — `Expr::String` initializer

Replace hardcoded `i8* null` with a `bitcast` of the actual string constant `@str.N`, looked up via `self.string_constants`.

## Testing

- `cargo test --lib` must pass
- The officina binary should no longer crash on startup
- Existing tests for `@str.0` and `@ll_empty_list` must continue to pass
