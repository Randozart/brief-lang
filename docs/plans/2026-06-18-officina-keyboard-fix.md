# Officina Keyboard Input Fix

**Date**: 2026-06-18
**Component**: LLVM Backend — SSA dispatch loop (`loop_engine.rs`)
**Status**: Implementation complete

## Problem

Officina CLI (`officina-cli`) cannot read keyboard input. The `@stdin#` trigger
is initialized (epoll_create, fcntl nonblock, epoll_ctl ADD) but `epoll_wait` is
never called in the main loop, so the `keypress` trigger value stays at 0 forever.
The precondition `[booted && keypress != '\0']` never becomes true → input
processing never fires.

## Root Cause

`emit_ssa_main` in `src/backend/llvm/loop_engine.rs` uses `__rt_wait()` (a no-op
setter for `__io_pending`) instead of `emit_trg_event_epoll_wait()` (which
actually calls `epoll_wait` and reads trigger data). Two code paths affected:

1. **Exit condition + wake triggers** (line 851):
   `call void @__rt_wait()` should be `emit_trg_event_epoll_wait(self, out)`

2. **Wake triggers only, no exit condition** (line 858):
   `call void @__rt_wait()` should be `emit_trg_event_epoll_wait(self, out)`

The non-SSA path (`emit_main`) already uses the correct call at line 181.
The SSA path was simply missed.

## Secondary Issues (already fixed in current code)

- `\0` char escape in lexer: `'\0'` was parsed as backslash (92) instead of
  null (0). Fixed at `src/lexer.rs:422-423`.
- `done_{name} → %done` early exit: `done_boot` branched to `%done` (program
  exit) instead of the next txn's skip label. Fixed at `loop_engine.rs:812`.

## Fix

Two-line change in `src/backend/llvm/loop_engine.rs`:

### Line 851 (exit_condition + has_wake_triggers path)

Before:
```rust
writeln!(out, "  call void @__rt_wait()").ok();
```

After:
```rust
emit_trg_event_epoll_wait(self, out);
```

### Line 858 (has_wake_triggers only path)

Before:
```rust
writeln!(out, "  call void @__rt_wait()").ok();
```

After:
```rust
emit_trg_event_epoll_wait(self, out);
```

## Verification

1. `cargo test --lib` — all existing tests pass
2. `cargo build` — no warnings
3. Recompile officina: `./target/release/brief-compiler rbv officina.bv`
4. Run officina and verify keyboard input is processed
