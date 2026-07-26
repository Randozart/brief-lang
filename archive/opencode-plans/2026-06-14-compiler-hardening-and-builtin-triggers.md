# Phase 17: Compiler Hardening + Built-in Trigger Sources

**Date**: 2026-06-14  
**Status**: Plan (not yet implemented)

---

## Part A — Compiler Errors (Reject Invalid Programs)

Six issues from officina-cli that should have been caught at compile time.

### A1. String + Int in `Expr::Add`

**Root cause**: `infer_expression` for `Expr::Add` doesn't check operand type compatibility. `"hello" + 42` passes typechecking; the backend emits `add i64, i64` (integer addition) which is meaningless for a string.

**Fix**: In `typechecker.rs`, `infer_expression` for `Expr::Add` — check that both operands have the same numeric type (Int, UInt, Float). If one is String, emit a type error.

**Error**: `Type mismatch: cannot add Int to String`

**Risk**: None — this is a pure hardening change.

### A2. Assignment to trigger variables

**Root cause**: `officina.bv` contains `&keypress = "";` which tries to write to the trigger variable `keypress`. The backend's `emit_stmt` for `Assignment` updates `let_bindings` but never emits a `store` to the linked C global. The assignment is silently dropped.

**Fix**: In `typechecker.rs` or `emit_stmt.rs`, detect when the LHS of an assignment is a trigger name. If it's a `@ link` trigger, emit a check: either store the value to the linked global, or emit an error.

**Option A (preferred)**: Reject at the typechecker level — `Cannot assign to trigger variable 'X'`. Triggers are read-only from the Brief side; their values come from the linked C global.

**Option B**: Allow the assignment and have the backend emit `store volatile` to the linked global. This is more permissive but enables use cases like resetting a trigger after reading it. However, this is semantically dangerous — the C runtime may be writing to the same global concurrently via epoll handler.

**Recommendation**: Option A — reject at typecheck time. If users need to track trigger state, use a separate `let` variable:
```brief
let prev_keypress: Char = '\0';
txn track_key [keypress != '\0'] {
    &prev_keypress = keypress;
    term;
};
```

### A3. `@ link` to a function symbol

**Root cause**: The backend emits `@tty_read_key = external global i8, align 1` for `@ link tty_read_key`. If `tty_read_key` is a C function, the linker resolves the symbol to the function's code section, not a data global. The `load volatile i8, i8* @tty_read_key` loads machine code bytes instead of data.

**Fix**: In `emit_declares()` (`mod.rs:730-779`), after collecting linked symbols, check `self.frgn_map` for name conflicts. If a linked symbol is also declared as a `frgn` function, emit a warning:
```
warning: 'tty_read_key' is declared as a frgn function but used as a @ link trigger.
         Use a volatile C variable or @ stdin# for keyboard input.
```

Better: check at parse/analysis time in `cross_reference.rs` or the typechecker.

### A4. `Expr::Block` fallthrough in `emit_expr`

**Root cause**: `emit_expr.rs` line 829 has a `_ => {}` catch-all that silently returns an undefined register for unhandled `Expr` variants. `Expr::Block` (used by unification `x := { ... }`) falls through here, producing an undefined register that LLVM optimizes to `%t0 = add i64 0, 0` — hence the `%t0`/`%t6` bug.

**Fix**: Add an explicit match arm for `Expr::Block` in `emit_expr`:
```rust
Expr::Block(stmts, val) => {
    for s in stmts { self.emit_stmt(out, s, indent); }
    if let Some(v) = val { return self.emit_expr(out, v, indent); }
    // No trailing value — return 0
    writeln!(out, "{}{} = add i64 0, 0", indent, v).ok();
}
```

And change the `_ => {}` catch-all to `_ => { unreachable!("unhandled Expr variant in emit_expr"); }` to catch future gaps.

### A5. Unification codegen uses RHS body as match value

**Root cause**: `emit_stmt.rs` for `Unification` (line ~446) emits `emit_expr` on `expr` (the RHS block body `{ term true; }`) instead of on the LHS variable being unified from. The `Expr::Block` fallthrough (A4) causes it to silently return 0.

**Fix**: When the RHS is a `Block` with a terminal terminator, the unification value is the LHS variable's current value, not the RHS. Emit the LHS as the value, and only evaluate the RHS for side effects (statement execution without value).

This is the actual cause of the `%t0`/`%t6` bug in officina.

### A6. `_ => {}` in `emit_expr` catch-all

**Root cause**: `emit_expr.rs` line 812: the `_ => {}` arm silently ignores any `Expr` variant not explicitly handled. This masks bugs (new Expr variants added without updating emit_expr) and produces undefined registers.

**Fix**: Change to `_ => { unreachable!("emit_expr: unhandled Expr variant") }`. This turns silent wrong-code bugs into compile-time panics that crash the compiler with a clear message.

---

## Part B — Built-in Trigger Sources

### B1. AST

Add three variants to `LinkRef` in `src/ast.rs`:

```rust
pub enum LinkRef {
    Explicit(u64),
    Linked(String),
    /// @stdin# — read one byte from stdin per tick (0 = no data)
    Stdin,
    /// @ timer#(Hz) — periodic timer at given frequency
    Timer(u64),
    /// @ signal#(Name) — POSIX signal delivery
    Signal(String),
}
```

The `#` is part of the source keyword (like `import#`, `intrinsic#`),
not a suffix on parameters. Parameters go in parentheses after the keyword.

### B2. Parser

In `parse_trigger()` (`src/parser.rs:3179`), add before the fallthrough
`Identifier` branch at line 3207. The `#` is consumed as part of the
source keyword name, before any parenthesized parameters:

```
@stdin#          → expect identifier "stdin" followed immediately by Token::Hash. No params → LinkRef::Stdin
@ timer#(Hz)     → expect "timer" with space after @, then Token::Hash, then (integer) → LinkRef::Timer(N)
@ signal#(Name)  → expect "signal" with space after @, then Token::Hash, then (identifier) → LinkRef::Signal(name)
```

The parser flow after `@`:
1. Check for `Integer` (MMIO address) — existing
2. Check for `Link` keyword — existing
3. Check for `Identifier` — if `stdin`, `timer`, or `signal`, expect `Token::Hash`
   immediately after the identifier, then handle parameters:
   - `stdin#` — done, advance past `#`
   - `timer#` — after `#`, expect `(`, integer, `)`
   - `signal#` — after `#`, expect `(`, identifier, `)`
4. Otherwise, fall through to existing backward compat `@ identifier` handling

The `#wake` check at line 3250 must gate on built-in sources (skip the
check — they have their own `#` syntax already consumed as part of the
source keyword).

### B3. Backend emit

**Declares**: Skip global emission for non-`Linked` variants (already handled
by the match on `Linked` in `emit_declares()`).

**Init**: Add `emit_trg_open()` to `LlvmBackend` — for each built-in trigger,
emit initialization code:
- `@stdin#` — no init needed (stdin is always open)
- `@ timer#(Hz)` — `call @timerfd_create`, `call @timerfd_settime`
- `@ signal#(Name)` — `sigemptyset + sigaddset + sigprocmask + signalfd`

**Load**: Update `emit_trg_load()` for the new variants — emit `read()` calls
that return 0 (no data) or non-zero (data available).

### B4. C Runtime

Add thin wrappers for syscalls not available via `frgn`:
```c
int32_t __sys_timerfd_create(int32_t clockid, int32_t flags);
int32_t __sys_timerfd_settime(int32_t fd, int32_t flags,
                               const struct itimerspec* new_value, void* old_value);
int32_t __sys_signalfd(int32_t fd, const sigset_t* mask, int32_t flags);
```

Eventually remove `__io_pending`, `__stdin_ready`, `__tty_read_key`,
`__timer_1hz`, `__timer_100hz`, `__sigint_flag`, `__sigterm_flag`,
`__sighup_flag` once all users migrate.

---

## Part C — Remaining is/from/like Work

### C1. Interpreter/typechecker tests not compiled

16 tests exist in source (`interpreter.rs:5644-5776`, `typechecker.rs:2610-2643`)
inside `mod tests { }` but don't appear in the test binary (794 vs expected 810).

**Investigation needed**: Add `cargo clean && cargo test --lib` — if that doesn't
resolve, inspect `nm` output for the specific function symbols. Likely a cargo
fingerprinting issue where the dependency files are out of date.

---

## Execution Order

```
Step │ Part │ Work                     │ Verification
─────┼──────┼──────────────────────────┼──────────────────────────────
1    │ A4   │ Expr::Block + catch-all  │ cargo test --lib (catches gaps)
2    │ A5   │ Unification codegen fix  │ officina no longer needs sed
3    │ A1   │ String+Int type error    │ "hello" + 42 → compile error
4    │ A2   │ Trigger assignment error │ &keypress = "" → compile error
5    │ A3   │ @ link function warning  │ @ link tty_read_key → warning
6    │ A6   │ unreachable catch-all    │ cargo test --lib
7    │ B1-4 │ Built-in triggers        │ officina uses @ stdin#
8    │ C1   │ Test investigation       │ 810 tests vs 794
9    │      │ Build + final verify     │ cargo test --lib, cargo build
```
