# Conditional FFI — `frgn?`, `frgn!`, `frgn?!`, `fn?`, `term?`, and Hashword Sources
## 2026-07-25

## Motivation

Briv currently has a single FFI declaration — `frgn` — which **must** link or
compilation fails. This breaks in three scenarios:

1. **Platform-adaptive code** — a function exists on Linux (`#POSIX`) but not
   on Windows (`#Win32`). The source should compile on both without wrapping
   everything in `$let platform = SysQuery$` boilerplate.

2. **Optional features** — an optional GPU backend, DSP accelerator, or audio
   driver. The frgn exists if the hardware/driver is present at link time.

3. **Fire-and-forget calls** — telemetry, logging, metrics. Send the data, don't
   wait for a response. If the target doesn't support it, skip silently.

The solution extends `frgn` with three new variants, an existence expression
(`fn?`), a conditional term (`term expr?`), and hashword-based source
specifications replacing `from "c"`.

## The Four FFI Declarations

| Declaration | Must link? | Return | Blocking? | Existence check |
|-------------|-----------|--------|-----------|-----------------|
| `frgn` | Yes | Normal | Yes | `fn?` always true |
| `frgn?` | No | Normal | Yes | `fn?` target-dependent |
| `frgn!` | No | Void | **No** | `fn?` may be checked |
| `frgn?!` | No | Bool(delivered) | **No** | `fn?` may be checked |

### `frgn` — Required Foreign Function

```briv
frgn shm_open(name: String, oflag: Int, mode: Int) -> Int from #POSIX fallback -1;
```

- Must link against the target. If the symbol is not found, compilation fails.
- `fn?` always evaluates to `true` at compile time.
- Callable without any guard. The compiler guarantees it exists.

### `frgn?` — Optional Foreign Function

```briv
frgn? glCompileShader(shader: Int) -> Void from #OpenGL fallback;

export defn compile_shader(s: Int) -> Int {
    when glCompileShader? {
        term glCompileShader(s);
    };
    term -1;
};
```

- Tries to link. If the symbol is not found, `fn?` evaluates to `false`.
- The compiler **requires** a `fn?` guard before any call to a `frgn?` symbol.
  If a function body calls `frgn? name` without first checking `name?`,
  the compiler emits an error.
- The narrowing pass proves `name?` at compile time per target and const-folds
  the guard, eliminating the dead branch. Zero runtime cost.

### `frgn!` — Fire-and-Forget

```briv
frgn! emit_metric(name: String, value: Int) -> Void from #POSIX fallback;
```

- Tries to link. Returns `Void` immediately — no wait for a response.
- If the symbol doesn't link, the call is a no-op (silently skipped).
- The narrowing pass eliminates the entire call site when `fn?` is false.
- No existence guard required — the call is already a "best effort."

### `frgn?!` — Fire-and-Forget with Delivery Check

```briv
frgn?! send_message(addr: Int, data: Ptr<Int>) -> Bool from #POSIX fallback false;
```

- Tries to link. Returns `Bool(true)` if the call was dispatched,
  `Bool(false)` if the symbol didn't link or the dispatch failed.
- Returns immediately — no wait for processing, just dispatch acknowledgment.
- The narrowing pass const-folds to `Bool(false)` when `fn?` is false,
  eliminating the call entirely and leaving the `false` return.

## The `fn?` Expression

The `?` suffix on a function name evaluates to a `Bool`:

```briv
when glCompileShader? { term glCompileShader(s); };
```

`glCompileShader?` is parsed as `Expr::Exists("glCompileShader")`.

**Compile-time behavior:**
- For `frgn` and regular `defn`: always `Bool(true)` — they always exist.
- For `frgn?`, `frgn!`, `frgn?!`: `Bool(true)` if the symbol links on the
  target, `Bool(false)` otherwise. Proven at compile time by the linker
  resolution pass.

**Runtime behavior (for `frgn?!` at call time):**
- `send_message?` at the call site checks whether the dispatch succeeded,
  returning the delivery status.

## The `term expr?` Statement

`term expr?` is a conditional term: if `expr` can be resolved (the function
exists), evaluate and return it. If it can't be resolved, continue to the
next statement.

```briv
export defn handle(x: Int) -> Int {
    term posix_fn(x)?;
    term fallback(x);
};
```

This is syntactic sugar for:

```briv
export defn handle(x: Int) -> Int {
    when posix_fn? {
        term posix_fn(x);
    };
    term fallback(x);
};
```

The narrowing pass const-folds the existence check. On a POSIX target,
`posix_fn?` is true, so the function returns `posix_fn(x)`. On a non-POSIX
target, `posix_fn?` is false, the "try" term is eliminated, and the fallback
runs.

## Hashword Sources — `from #POSIX` instead of `from "c"`

Replace `from "c"` with hashword-based platform protocols:

```briv
// Before:
frgn putchar(c: Int) -> Int from "c" fallback -1;

// After:
frgn putchar(c: Int) -> Int from #POSIX fallback -1;
frgn MessageBox(h: Int, t: String, c: String, t: Int) -> Int from #Win32 fallback -1;
frgn random_get(b: Ptr<Int>, l: Int) -> Int from #WASI fallback -1;
```

The `from` field in `ForeignBinding` currently stores a `String` ("c", a
file path, or a library name). With hashword sources, it stores a
`ProtoSource` enum:

```rust
pub enum ProtoSource {
    /// from #POSIX — POSIX libc
    Posix,
    /// from #Win32 — Windows API (kernel32, user32, etc.)
    Win32,
    /// from #WASI — WebAssembly System Interface
    Wasi,
    /// from "path" — custom source file or library
    Path(String),
}
```

The backend maps:

| Protocol | Linux | macOS | Windows | WASM/WASI |
|----------|-------|-------|---------|-----------|
| `#POSIX` | libc | libc | emulated via CRT | `#if WASI` |
| `#Win32` | error | error | kernel32 | error |
| `#WASI` | error | error | error | wasi-emulated |

If a `frgn` (required) uses `#Win32` on Linux, compilation fails at link
resolution — the symbol can't be found and there's no fallback. If a `frgn?`
uses `#Win32` on Linux, `fn?` evaluates to `false` and the guarded path is
dead-code eliminated.

### Migration

All `from "c"` declarations across stdlib become `from #POSIX`:

```bash
# In lib/std/*.bv, lib/glue/*.bv:
frgn puts(s: String) -> Int from "c" fallback -1
# becomes:
frgn puts(s: String) -> Int from #POSIX fallback -1
```

## Compile-Time Safety: `frgn?` Guards

If a function body calls a `frgn?` symbol without first checking `fn?`,
the compiler emits an error:

```briv
frgn? optional_fn(x: Int) -> Int from #POSIX fallback 0;

export defn bad() -> Int {
    term optional_fn(5);  // ERROR: frgn? 'optional_fn' not guarded by optional_fn?
};

export defn good() -> Int {
    when optional_fn? {
        term optional_fn(5);  // OK
    };
    term 0;
};
```

**How the check works:** In the typechecker (or a dedicated pass after
typechecking), for each function body, walk all `Expr::Call` sites. For each
`Call(name)`, look up the `ForeignBinding` with `briv_name == name`. If the
binding is `frgn?`/`frgn!`/`frgn?!`, check that `name?` appears as a guard
in every path leading to the call. If not, emit an error.

**Const-folding the existence check:** The narrowing pass sees
`Expr::Exists("optional_fn")`. For `frgn?` bindings, the compiler knows at
compile time whether the symbol linked (from the linker resolution). The
narrowing pass sets the range to exactly `[0,0]` (false) or `[1,1]` (true).
The guard `when optional_fn? { ... }` is const-folded: if false, the branch
is eliminated; if true, the guard is eliminated.

## Lexer Details

**`?` and `!`** are standalone tokens, already defined in the lexer:

```rust
// src/lexer.rs — existing:
#[token("?")]
Question,

#[token("!")]
Exclamation,
```

These are NOT absorbed into identifiers. `fn?` is lexed as
`Identifier("fn")` + `Question`. The parser detects the `?` after an
identifier and combines them.

**`from #POSIX`:** `#POSIX` is lexed as `Token::Identifier("#POSIX")`.
The identifier regex `[a-zA-Z_#$][a-zA-Z0-9_#$]*` allows `#` as a starting
character. The parser checks the identifier's string for the `#` prefix.

## Parser Grammar

### `frgn` Declaration Variants

```rust
// In parse_frgn_decl:
fn parse_frgn_decl(&mut self) -> Result<TopLevel, SyntaxError> {
    self.pos += 1; // consume 'frgn'
    let is_optional = self.eat(&Token::Question);   // frgn? — optional
    let is_fire_forget = self.eat(&Token::Exclamation); // frgn! — fire-forget
    let needs_delivery = if is_fire_forget && self.eat(&Token::Question) {
        true  // frgn?! — fire-forget with delivery
    } else {
        false // frgn! — just fire-forget
    };
    // Error: frgn?? is invalid (can't have optional + fire-forget both)
    if is_optional && is_fire_forget {
        return self.error("cannot combine frgn? with frgn! or frgn?!");
    }
    // ...rest of parsing...
}
```

The `ForeignBinding` struct gains three bool flags:

```rust
pub struct ForeignBinding {
    // ... existing fields ...
    pub is_optional: bool,      // frgn? — check fn? before calling
    pub is_fire_forget: bool,   // frgn!/frgn?! — non-blocking, void or Bool return
    pub is_delivery: bool,      // frgn?! — frgn! + delivery status returned
}
```

### `fn?` Expression

```rust
// In parse_primary or parse_postfix:
// After parsing an identifier as an expression (e.g., "glCompileShader"),
// check if the next token is ? — if so, wrap in Expr::Exists.
fn parse_primary(&mut self) -> Result<Expr, SyntaxError> {
    // ...existing code...
    if let Some(name) = self.expect_identifier_opt() {
        if self.eat(&Token::Question) {
            return Ok(Expr::Exists(name));
        }
        // ...rest of identifier handling...
    }
    // ...rest of primary...
}
```

`Expr::Exists("glCompileShader")` evaluates to `NavValue::Bool(true)` if
the function linked, `NavValue::Bool(false)` if it didn't.

### `term expr?` — Conditional Term

```rust
// In parse_term_statement:
fn parse_term_statement(&mut self) -> Result<Statement, SyntaxError> {
    self.pos += 1; // consume 'term'
    let expr = self.parse_expression()?;
    let is_conditional = self.eat(&Token::Question);  // term expr?
    self.expect(Token::Semicolon)?;
    // ...store is_conditional on the Term statement...
}
```

Desugared in the stage evaluator to:

```rust
// When is_conditional is true:
// term expr? → when expr? { term expr; };
```

The `Statement::Term` variant gains an `is_conditional: bool` flag.

### `from #POSIX` — Hashword Source

```rust
// In parse_frgn_decl, after parsing the from keyword:
fn parse_frgn_source(&mut self) -> Result<ProtoSource, SyntaxError> {
    match self.peek() {
        // from #POSIX, from #Win32, from #WASI
        Some(Token::Identifier(s)) if s.starts_with('#') => {
            let name = self.expect_identifier()?;
            match name.as_str() {
                "#POSIX" => Ok(ProtoSource::Posix),
                "#Win32" => Ok(ProtoSource::Win32),
                "#WASI" => Ok(ProtoSource::Wasi),
                _ => self.error(&format!("unknown protocol source '{}'", name))
            }
        }
        // from "path/to/file.c"
        Some(Token::String(_)) => {
            let path = self.expect(Token::String)?;
            Ok(ProtoSource::Path(path))
        }
        _ => self.error("expected protocol source (#POSIX, #Win32, #WASI) or string path")
    }
}
```

## Implementation Order

| Step | What | Files |
|------|------|-------|
| 1 | AST: `Expr::Exists(String)`, `ProtoSource` enum, `ForeignBinding` flags | `src/ast/expr.rs`, `src/ast/top.rs` |
| 2 | Parser: `frgn?`/`frgn!`/`frgn?!` modifiers in `parse_frgn_decl` | `src/parser/definitions.rs` |
| 3 | Parser: `fn?` expression in `parse_primary` | `src/parser/expressions.rs` |
| 4 | Parser: `from #POSIX` → `ProtoSource::Posix` | `src/parser/definitions.rs` |
| 5 | Parser: `term expr?` in `parse_term_statement` | `src/parser/statements.rs` |
| 6 | Eval: `Expr::Exists` handler | `src/macros/eval.rs` |
| 7 | Eval: `term?` conditional skip | `src/macros/eval.rs` |
| 8 | Safety pass: `frgn?` guard check | New: `src/analysis/frgn_guard.rs` |
| 9 | Narrowing: const-fold `fn?` | `src/optimizer/narrow_int.rs` |
| 10 | Migrate `from "c"` → `from #POSIX` | `lib/std/*.bv`, `lib/glue/*.bv` |

## Files Changed

| File | Change |
|------|--------|
| `src/lexer.rs` | `?` handling after identifiers (or `Question` token) |
| `src/ast/expr.rs` | `Expr::Exists(String)` variant |
| `src/ast/top.rs` | `ForeignBinding.is_optional`, `is_fire_forget`, `is_delivery` flags; `ProtoSource` enum |
| `src/parser/definitions.rs` | `parse_frgn_decl`: handle `?`/`!`/`?!` modifiers, hashword sources |
| `src/parser/expressions.rs` | `parse_fn_exists`: `Identifier("foo")` + `?` → `Expr::Exists("foo")` |
| `src/parser/statements.rs` | `parse_term`: handle `expr?` |
| `src/macros/eval.rs` | `Expr::Exists` → `NavValue::Bool`; `Statement::Term(Some(Expr::Exists(_)))` → skip |
| `src/analysis/frgn_guard.rs` | Safety pass: verify `frgn?` guarded by `fn?` |
| `src/optimizer/narrow_int.rs` | Const-fold `fn?` for `frgn?` bindings |
| `src/type_universe/resolve.rs` | Resolve `from #POSIX` → `ProtoSource::Posix` |
| `src/backend/llvm/mod.rs` | Map `ProtoSource::Posix` → libc linking |
| `lib/std/*.bv` | `from "c"` → `from #POSIX` |
| `lib/glue/*.bv` | Same migration |
