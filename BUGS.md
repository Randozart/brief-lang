# Bugs & Mistakes Log

## Format
- **Date**: YYYY-MM-DD
- **Issue**: What happened
- **Root Cause**: Why it happened
- **Fix**: How it was resolved
- **Lesson**: How to avoid next time

## 2026-05-28 — Overriding `from "..."` location in typechecker

**Issue**: `__read_file` FFI call failed with `location: <profile:__read_file>` instead of `"std::fs::read_to_string"`.

**Root Cause**: Typechecker at `src/typechecker.rs:993` unconditionally overwrote `signature.location` with `<profile:{name}>` when `toml_path` was empty, even when the `from "..."` clause had already set a correct location. The parser correctly parsed `from "std::fs::read_to_string"` but the typechecker replaced it with a profile placeholder.

**Fix**: Only set the profile placeholder when `signature.location` is empty:
```diff
- signature.location = format!("<profile:{}>", name);
+ if signature.location.is_empty() {
+     signature.location = format!("<profile:{}>", name);
+ }
```

**Lesson**: Always check before overwriting. The "new FFI syntax" (direct `from "..."`) and "old profile FFI" coexist — the typechecker assumed no `from` clause existed.

## 2026-05-28 — Adding built-in string matches for stdlib functions

**Issue**: Added `is_digit`, `is_alpha`, `is_alphanumeric`, `is_upper`, `is_lower`, `is_space`, `char_to_string` as Rust string-match built-ins in the interpreter.

**Root Cause**: When `UndefinedForeignFunction("is_digit")` appeared, instead of adding `import char from "std/char.bv"` to the calling `.bv` file, I added a Rust string match in `Expr::Call` handler. Also pre-populated `None`/`Some` enum constants in `Interpreter::new()`.

**Fix**: Reverted all built-in hacks. Added `import char from "std/char.bv"` to `lib/compiler/lexer.bv`.

**Lesson**: When the interpreter can't find a function, check if it's in the standard library first. The standard library IS the dependency source. Never add Rust string-match built-ins for things the standard library provides.

## 2026-05-28 — Typechecker overwrites `from` location

**Issue**: `from "std::fs::read_to_string"` in `lib/std/io.bv` was being parsed correctly but then overwritten by typechecker.

**Root Cause**: The typechecker's `load_binding_for_frgn` function set `location = format!("<profile:{}>", name)` unconditionally when `toml_path` was empty, discarding the parser-provided location from the `from "..."` clause.

**Fix**: Added `if signature.location.is_empty()` guard before overwriting.

**Lesson**: Multiple FFI resolution paths coexist (profile-based + direct `from`). Don't assume one path overrides the other.

## 2026-05-28 — Contract-after-arrow parser bug

**Issue**: `-> Type [pre][post]` syntax caused both pre and post conditions to parse as `Expr::Bool(true)`.

**Root Cause**: The while loop in `parse_contract` (line 2879) correctly consumed `[`, but `parse_expression()` returned `BoolTrue` instead of the bracket contents, indicating a lexer state issue specific to the contract-after-arrow code path.

**Workaround**: Use contract-before-arrow syntax `[pre][post] -> Type`.

**Lesson**: The contract-after-arrow path has a subtle lexer/parser interaction bug that's not yet fully understood. Always prefer contract-before-arrow.

## 2026-05-28 — Keyword tokens can't appear in any variable position

**Issue**: `txn`, `reg`, `from` used as variable names caused parse failures across multiple `.bv` files.

**Root Cause**: 44 keyword tokens in the lexer were not accepted by `expect_identifier()` or `parse_primary_expr()`, causing keywords to fail as parameter names, `let` bindings, and `uni` pattern variables.

**Fix**: Route B: Added all 44 missing keywords to `expect_identifier()` and the `parse_primary_expr()` fallback path. Also fixed 3 `while let Some(Ok(Token::Identifier(_)))` loops to use `expect_identifier()` instead.

**Lesson**: The lexer defines ~60 keyword tokens but only 22 were handled as identifiers. When dealing with keyword-as-identifier issues, fix the parser, not the `.bv` files.

## 2026-05-28 — \u{D800} surrogate fails char::from_u32

**Issue**: `'\u{D800}'` in `lib/std/char.bv` caused parse error because logos `regex` callback returned `None` for surrogates, interpreted as a lex error.

**Root Cause**: UTF-16 surrogates (0xD800-0xDFFF) aren't valid Unicode scalar values. `char::from_u32(cp)` returns `None` for them. Logos interprets a `None` return from a regex callback as "token didn't match" (the `?` operator propagates `None` as a failed match).

**Fix**: Changed `char::from_u32(cp)` to `char::from_u32(cp).unwrap_or('?')` in the `\u{...}` escape handler.

**Lesson**: Logos regex callbacks must never return `None` for valid lexer input. Handle all failure modes of `from_u32`, including surrogates.

## 2026-05-28 — Unevaluated enum constructors (None, Some, Ok, Err)

**Issue**: Interpreter returned `UndefinedVariable("None")` and `UndefinedForeignFunction("Err")`.

**Root Cause**: After removing magic constants from `Interpreter::new()`, `load_program` didn't register enum variant names from declarations. `Result` is also an intrinsic type — no `.bv` `enum Result { Ok, Err }` declaration exists to load from.

**Fix**: 
1. `load_program` now iterates `TopLevel::Enum` and registers each variant as `Value::Enum` in state
2. `Ok`/`Err` handled as special intrinsic enum constructors in `Expr::Call` handler
3. `is_ok`, `is_err`, `unwrap`, `unwrap_err` added as Result methods in interpreter

**Lesson**: Enum constructors must be loaded from actual declarations. Intrinsic types (Result, Option) need explicit handling since they lack `.bv` enum declarations.

## 2026-05-29 — Method-call `x.foo(y)` drops all arguments except receiver

**Issue**: `UndefinedVariable("s")` when calling `output.append_str("...")`.

**Root Cause**: The parser's `parse_postfix` function (lines 4314, 4385) parsed arguments from `x.foo(y, z)` into `args` but then constructed `Expr::Call(member_name, vec![expr])` — dropping all parsed arguments and passing only the receiver `x`. The receiver was passed as `expr` which is the preceding expression. The `let mut args = Vec::new()` on the line above was parsed but the populated `args` vector was never used in the `Expr::Call` construction.

```rust
// Line 4385 (BUGGY):
expr = Expr::Call(member_name, vec![expr]);
// Should be:
let mut call_args = vec![expr];
call_args.extend(args);
expr = Expr::Call(member_name, call_args);
```

**Fix**: Changed both locations to prepend receiver `expr` to the parsed `args` vector before constructing `Expr::Call`.

**Lesson**: Mental model matched the intent ("prepend receiver to args") but code used `vec![expr]` which discarded args. Always verify that all populated variables are actually consumed, especially when refactoring from a simpler implementation.

## 2026-05-29 — Term statement inside nested blocks doesn't propagate return value

**Issue**: `compile_file` returned `Value::Void` even though `term Ok(output)` was reached inside a `uni` block.

**Root Cause**: The `call_defn` function (line ~302) handled `Statement::Term` at the top level by capturing the result, but for statements inside nested blocks (unifications, guarded blocks, etc.), `exec_stmt` processed the `Term` and **discarded the value**:

```rust
// exec_stmt (line ~462):
Statement::Term { values: outputs, .. } => {
    if let Some(first) = outputs.first() {
        if let Some(expr) = first {
            let value = self.eval_expr(expr)?;
            if value != Value::Bool(true) {}  // <-- NO-OP, value discarded
        }
    }
}
```

While `call_defn`'s top-level handler correctly stored `result`, it never checked `exec_stmt`'s nested result. After `exec_stmt` returned, `call_defn` continued to the next statement without capturing any value set by `term` inside nested scopes.

**Fix**: 
1. Added `return_value: Option<Value>` to `Interpreter` struct
2. Modified `exec_stmt`'s `Term` handler to store the value in `self.return_value`
3. Modified `call_defn` to save/restore `return_value` and break when it's set after any statement (top-level or nested)

**Lesson**: `term` in Brief is not merely a "function return" — it's a value-capture mechanism that can appear inside any nested scope (guards, unifications, blocks). The interpreter must capture ALL `term` values, not just top-level ones.

## 2026-05-29 — Result field key mismatch between constructor and consumer

**Issue**: `run_selfhost` in `main.rs` looked for field key `"result"` (from the specific Ok/Err path at line 878) but the generic enum constructor path at line 869 used field key `"value"`.

**Root Cause**: Two code paths create `Ok`/`Err` enum values:
1. Generic enum constructor path (line 867-876): used when `Ok` variant is in state (registered from `Result` enum declaration). Creates fields with key `"value"`.
2. Specific Ok/Err path (line 878-882): used when `Ok`/`Err` are NOT in state. Creates fields with key `"result"`/`"error"`.

Since `std.result` is imported, `Ok` IS in state, so path 1 always applies. But `run_selfhost` at lines 643 and 651 looked for `"result"`/`"error"` keys from path 2.

**Fix**: Changed `fields.get("result")` to `fields.get("value")` and `fields.get("error")` to `fields.get("value")` in `run_selfhost`.

**Lesson**: When multiple construction paths exist for the same enum type, their field key conventions must be consistent. The generic path always uses `"value"` for single-field enum construction, but the specific Result path had its own convention. Prefer a single consistent convention.

## 2026-05-29 — Brief-written lexer rejects all input with "Unexpected character"

**Issue**: Self-host pipeline tokenizes files via `lib/compiler/lexer.bv` (running inside the interpreter) but fails with `Lex error: Unexpected character: ` on all inputs.

**Root Cause**: The lexer's `next_token` function (line 321-489) checks for EOF at line 325-328 using `is_none(current_char(state))`. If this check somehow fails to detect `None`, line 333 unconditionally calls `unwrap(current_char(state))` — which returns `Void` for `None` input. The guards then compare `Void` against various chars (all false), eventually hitting the "Unknown character" fallthrough at line 488. `char_to_string(Void)` produces an empty string, hence the blank error message.

**Status**: Not yet fixed. Potential causes:
- `is_none` generic function may not dispatch correctly to `Option<Char>` specialization
- `current_char` may return wrong `None` representation
- The `None` variant of `Option` may not exist in the interpreter's state

**Lesson**: (pending investigation)