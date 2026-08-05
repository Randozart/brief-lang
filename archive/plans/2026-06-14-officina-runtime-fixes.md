# officina Runtime Fixes + Remaining is/from/like Work

**Date**: 2026-06-14  
**Status**: Implemented and committed (`c3c2d5b`)  
**Priority**: High (officina bugs) / Medium (is/from/like remaining work)

---

## Phase A — officina Runtime Bugs

Three bugs preventing officina from working correctly.

### A1. `__print` doesn't flush stdout

**File**: `lib/runtime/briv_rt.c:379-381`  
**Root cause**: ANSI escape sequences contain no `\n`; line-buffered stdout never flushes.  
**Fix**: Add `fflush(stdout)` after `fputs`.  
**Risk**: None.

### A2. Loop exits after one cycle

**File**: `src/backend/llvm/loop_engine.rs:622`  
**Root cause**: `done_{txn}: br label %done` terminates main() when the first reactive txn's precondition is false. Should continue to `s_{txn}` (skip to next txn).  
**Fix**: Change `br label %done` to `br label %s_{name}`.  
**Risk**: Low — only changes one branch target.

### A3. `@ link String` loads i8* pointer, not content

**Files**: `mod.rs:318` + `emit_toplevel.rs:99-103` + `emit_expr.rs:1062` + `briv_rt.c`  
**Root cause**: `@ link` for String declares `external global i8*` and loads a pointer address. For C functions (like `tty_read_key`), the GOT holds the function entry address — comparing this against the empty string literal's address always produces "not empty."  
**Design**: Change `@ link String` to load a single byte from the linked address, consistent with how `@ link Int` loads i64 and `@ link Bool` loads i8. Add a special case in `emit_fcmp` to compare linked String triggers against string literals by first-byte value, not pointer address.

#### Fixes:
- **A3a** (`mod.rs:318`): Change `String` storage type from `"i8*"` to `"i8"`  
- **A3b** (`emit_toplevel.rs:99-103`): Change load from `load volatile i8*; ptrtoint` to `load volatile i8; zext`  
- **A3c** (`emit_expr.rs`): Add special case in `emit_fcmp` — when comparing a linked String trigger against a string literal `"X"`, compare against `X`'s first byte value (0 for `""`)  
- **A3d** (`briv_rt.c`): Replace `int64_t tty_read_key(void)` (blocking function) with `volatile char __tty_read_key = 0` global; wire epoll/kqueue stdin handlers to read into `__tty_read_key`

---

## Phase B — Remaining is/from/like Work

### B1. Interpreter + typechecker tests not compiled

16 tests exist in source (`interpreter.rs:5644-5776`, `typechecker.rs:2610-2643`) inside `mod tests { }` but don't appear in test binary. Need investigation.

### B2. `Some`/`None` lexer capitalization

`Ok` and `Err` recognize capitalized forms (`Ok`/`OK`, `Err`/`ERR`) but `Some`/`None` only recognize lowercase (`some`/`SOME`, `none`/`NONE`). Need to add `#[token("Some")]` and `#[token("None")]` for consistency.

---

## Execution Order

```
Step │ Work  │ Files                │ Verification
─────┼───────┼──────────────────────┼─────────────────────────
1    │ A1    │ briv_rt.c           │ Review output reaches terminal
2    │ A3d   │ briv_rt.c           │ volatile char + epoll read
3    │ A2    │ loop_engine.rs       │ cargo test --lib
4    │ A3a   │ mod.rs               │ cargo build
5    │ A3b   │ emit_toplevel.rs     │ cargo build
6    │ A3c   │ emit_expr.rs         │ cargo build + inspect IR
7    │ B2    │ lexer.rs             │ cargo test --lib
8    │ B1    │ investigate          │ cargo test --lib (810 tests)
9    │       │ Documentation        │ Update all affected docs
10   │       │ Final verify         │ cargo test --lib, cargo build
```
