# Conditional FFI — `frgn?`, `frgn!`, `frgn?!`, `fn?`, `term?`, and Hashword Sources
## 2026-07-25

## Motivation

Brief currently has a single FFI declaration — `frgn` — which **must** link or
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

```brief
frgn shm_open(name: String, oflag: Int, mode: Int) -> Int from #POSIX fallback -1;
```

- Must link against the target. If the symbol is not found, compilation fails.
- `fn?` always evaluates to `true` at compile time.
- Callable without any guard. The compiler guarantees it exists.

### `frgn?` — Optional Foreign Function

```brief
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

```brief
frgn! emit_metric(name: String, value: Int) -> Void from #POSIX fallback;
```

- Tries to link. Returns `Void` immediately — no wait for a response.
- If the symbol doesn't link, the call is a no-op (silently skipped).
- The narrowing pass eliminates the entire call site when `fn?` is false.
- No existence guard required — the call is already a "best effort."

### `frgn?!` — Fire-and-Forget with Delivery Check

```brief
frgn?! send_message(addr: Int, data: Ptr<Int>) -> Bool from #POSIX fallback false;
```

- Tries to link. Returns `Bool(true)` if the call was dispatched,
  `Bool(false)` if the symbol didn't link or the dispatch failed.
- Returns immediately — no wait for processing, just dispatch acknowledgment.
- The narrowing pass const-folds to `Bool(false)` when `fn?` is false,
  eliminating the call entirely and leaving the `false` return.

## The `fn?` Expression

The `?` suffix on a function name evaluates to a `Bool`:

```brief
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

```brief
export defn handle(x: Int) -> Int {
    term posix_fn(x)?;
    term fallback(x);
};
```

This is syntactic sugar for:

```brief
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

```brief
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

```brief
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
`Call(name)`, look up the `ForeignBinding` with `brief_name == name`. If the
binding is `frgn?`/`frgn!`/`frgn?!`, check that `name?` appears as a guard
in every path leading to the call. If not, emit an error.

**Const-folding the existence check:** The narrowing pass sees
`Expr::Exists("optional_fn")`. For `frgn?` bindings, the compiler knows at
compile time whether the symbol linked (from the linker resolution). The
narrowing pass sets the range to exactly `[0,0]` (false) or `[1,1]` (true).
The guard `when optional_fn? { ... }` is const-folded: if false, the branch
is eliminated; if true, the guard is eliminated.

## Implementation Order

| Step | What | Files |
|------|------|-------|
| 1 | Lexer: add `?` suffix handling for `fn?` expressions | `src/lexer.rs` |
| 2 | AST: `Expr::Exists(String)`, update `ForeignBinding` flags | `src/ast/expr.rs`, `src/ast/top.rs` |
| 3 | AST: `ProtoSource` enum for `from #Protocol` | `src/ast/top.rs` |
| 4 | Parser: `frgn?`/`frgn!`/`frgn?!`, `fn?`, `term?` | `src/parser/definitions.rs`, `src/parser/statements.rs`, `src/parser/expressions.rs` |
| 5 | Parser: `from #POSIX` → `ProtoSource::Posix` | `src/parser/definitions.rs` |
| 6 | Eval: `Expr::Exists` handler in `eval_nav_chain` | `src/macros/eval.rs` |
| 7 | Eval: `term expr?` handler in `evaluate_stage_stmt` | `src/macros/eval.rs` |
| 8 | Safety pass: check `frgn?` is guarded by `fn?` | New file: `src/analysis/frgn_guard.rs` |
| 9 | Narrowing: const-fold `fn?` for `frgn?` bindings | `src/optimizer/narrow_int.rs` |
| 10 | Migrate `from "c"` → `from #POSIX` across all `.bv` files | `lib/std/*.bv`, `lib/glue/*.bv` |

Steps 1-5 (lexer, AST, parser) must be done first. Steps 6-10 depend on them.

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
