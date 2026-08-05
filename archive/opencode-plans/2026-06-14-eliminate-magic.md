# Eliminate Magic: Plan for Phase 20

**Date**: 2026-06-14  
**Status**: Plan  
**Goal**: Remove two No-Magic violations introduced in Phases 18–19

---

## Item 1 — `__chr_to_str` inline codegen (Phase 19)

**Problem**: The LLVM backend emits `call i8* @__chr_to_str(i32)` for
`Char → String` casts, creating a hidden dependency on `briv_rt.c`.

**Fix**: Replace the `call` + `ptrtoint` with inline LLVM IR that allocates
a 2-byte buffer, stores the character, and null-terminates it.

Current code (emit_expr.rs:1046–1051):
```llvm
%tr = trunc i64 %src to i32
%ip = call i8* @__chr_to_str(i32 %tr)
%dst = ptrtoint i8* %ip to i64
```

Replace with:
```llvm
%tr = trunc i64 %src to i8         ; extract low byte
%buf = alloca i8, i64 2, align 1
store i8 %tr, i8* %buf, align 1
%gp = getelementptr inbounds i8, i8* %buf, i64 1
store i8 0, i8* %gp, align 1
%dst = ptrtoint i8* %buf to i64
```

**What to remove**:
- `__chr_to_str` from `lib/runtime/briv_rt.c` (9 lines)
- `declare i8* @__chr_to_str(i32)` from `src/backend/llvm/mod.rs` (1 line)

**Risk**: None — the inline IR is simpler and has no allocation
(alloca on stack, freed at function return).

---

## Item 2 — Built-in triggers via `frgn` instead of hardcoded syscalls

**Problem**: `@stdin#`, `@ timer#(Hz)`, `@ signal#(Name)` cause the compiler
to generate epoll/timerfd/signalfd code with no `frgn` declarations the user
can inspect.

**Fix**: Define a set of **compiler-known `frgn` signatures** that the emitter
treats as available without explicit declaration. These are declared once in a
`lib/std/` module, not hardcoded in the backend.

### Design

The backend currently emits:
```llvm
%raw = call i32 @__trg_stdin_read()
```

Instead, emit calls to **standard POSIX functions declared via `frgn`** that
a standard library module provides:

**New file: `lib/std/core/io.bv`**:
```briv
frgn read(fd: Int, buf: Ptr<Byte>, count: Int) -> Int;
frgn write(fd: Int, buf: Ptr<Byte>, count: Int) -> Int;
frgn epoll_create1(flags: Int) -> Int;
frgn epoll_ctl(epfd: Int, op: Int, fd: Int, event: Ptr<Byte>) -> Int;
frgn epoll_wait(epfd: Int, events: Ptr<Byte>, maxevents: Int, timeout: Int) -> Int;
frgn timerfd_create(clockid: Int, flags: Int) -> Int;
frgn timerfd_settime(fd: Int, flags: Int, new: Ptr<Byte>, old: Ptr<Byte>) -> Int;
frgn signalfd(fd: Int, mask: Ptr<Byte>, flags: Int) -> Int;
frgn sigprocmask(how: Int, set: Ptr<Byte>, old: Ptr<Byte>) -> Int;
```

But wait — this just moves the magic from the C runtime to a `frgn` list. The
user still can't see what syscalls the compiler generates. The `@stdin#`
keyword still triggers epoll setup invisibly.

### Better approach — eliminate built-in trigger sources

Instead of `@stdin#`, the user writes a reactive txn that calls `read()` via
`frgn` directly:

```briv
import# "core/io.bv" provides read, epoll_*, timerfd_*, signalfd_*;

// Manual stdin poll:
txn poll_stdin [booted] {
    let buf: Ptr<Byte> = ...;
    let n = read(0, buf, 1);
    if n > 0 {
        set_trigger(keypress, buf[0] as Char);
        // ...or use a let binding to track the key
    };
};
```

But this loses the reactive model — the trigger doesn't fire automatically.
The user would need to manually track the key state.

### Compromise — `@ link` to a `frgn` return value

The most transparent approach: allow `@ link` to bind to a `frgn` function's
return value instead of a C global:

```briv
frgn tty_read_key() -> Char;

trg keypress: Char @ link tty_read_key;   // calls the frgn each tick
```

This is NOT new magic — it's extending the existing `@ link` semantics:
- Today: `@ link symbol` loads `volatile i8` from a C global
- Proposed: `@ link frgn_name` calls the `frgn` function and uses its return value

The compiler knows `tty_read_key` is a `frgn` because it's declared in the
source. No hidden syscalls. No invisible C globals.

**Implementation**:
1. **AST**: Already works — `LinkRef::Linked(name)` stores the symbol name.
   The parser already handles `@ link symbol`.
2. **Typechecker**: When `@ link frgn_name` is used and `frgn_name` is declared,
   validate the trigger type matches the `frgn` return type.
3. **Backend**: In `emit_trg_load`, check if the link target is a `frgn`. If so,
   emit `call %result = call i64 %frgn_fn(...)` instead of `load volatile i8`.
4. **Remove hardcoded sources**: `emit_trg_init` (timerfd/signalfd setup) and
   the C runtime wrappers (`__trg_stdin_read`, `__trg_timerfd_*`, etc.) are
   replaced by `frgn` declarations the user writes.

The user would then write:
```briv
// briv_rt.c provides these C functions:
frgn __trg_stdin_read() -> Char;
frgn __trg_timerfd_open(hz: Int) -> Int;
frgn __trg_timerfd_read(fd: Int) -> Int;

trg keypress: Char @ link __trg_stdin_read;  // no #, no magic
trg tick @ link __trg_timerfd_read;          // explicit frgn call
trg sigint @ link __trg_signalfd_read;        // user's own code
```

**What to remove**:
- `@stdin#`, `@ timer#(Hz)`, `@ signal#(Name)` parser support
- `LinkRef::Stdin`, `LinkRef::Timer`, `LinkRef::Signal` AST variants
- `emit_trg_init()` method
- All `__trg_*` declares in `mod.rs`
- Non-blocking stdin setup in `__rt_init()`
- All `__trg_*` C wrappers from `briv_rt.c`

**Risk**: Medium — this is a non-trivial refactor. The `@ link` extension
needs careful implementation to handle function calls with different signatures
from global loads. But it eliminates all the magic in one shot.

---

## Execution Order

| Step | Item | What | Verification |
|------|------|------|-------------|
| 1 | Item 1 | Inline `__chr_to_str` in `emit_cast_convert` | 801 tests pass, no `call @__chr_to_str` in emitted IR |
| 2 | Item 2 | Extend `@ link` to call `frgn` functions | officina.bv compiles with `@ link __trg_stdin_read` |
| 3 | Item 2 | Remove `@stdin#`/`@ timer#`/`@ signal#` | Parser + AST + LinkRef all cleaned up |
| 4 | Item 2 | Remove `emit_trg_init()` and all `__trg_*` C wrappers | No magic OS dependencies |
