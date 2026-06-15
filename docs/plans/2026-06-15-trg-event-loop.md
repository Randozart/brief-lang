# trg Event Loop — Epoll-Based Main Loop Integration

**Date:** 2026-06-15  
**Status:** Planned  
**Context:** Phases 1–6 (trg dirty-flag architecture) completed the dependency graph, bitmask, `@step()` function, and CIRCT/Webstack backends, but the **main loop never calls `@step()`**. The old `emit_trg_init()`/`emit_trg_load()` still emit polling calls to `@__trg_stdin_read()` which was removed from the runtime in Phase 6.

## Problem

- `@step(%State*, i64)` exists as a standalone function but nothing invokes it
- `emit_trg_init()` in `emit_toplevel.rs:156-254` emits old timerfd/signalfd setup
- `emit_trg_load()` emits `@__trg_stdin_read()` calls — symbol no longer exists
- `emit_main()` in `loop_engine.rs` calls `@__rt_wait()` (sleep 1) instead of blocking on events
- Result: programs with `trg` declarations fail to link

## Design

The compiler emits epoll setup and event loop as **bare LLVM IR** — no intrinsics, no FFI, no runtime helpers beyond libc. The programmer declares the trigger and its source; the compiler generates the fd setup, epoll registration, data reading, and state field stores:

```brief
trg keypress: Char @stdin#;     // stdin input
trg tick: Int @timer#(60);      // 60 Hz timer
trg resize: Int @signal#(SIG);  // terminal resize signal
```

The `@source#` syntax is the programmer's interface. The `#` suffix marks it as a compiler built-in — the compiler emits the fd creation and event plumbing automatically. Sources without `#` (e.g. `@ 0x1000`, `@link "name"`) are handled directly by the backends (MMIO address, C global volatile load).

### Event Loop Structure

```
tick:
  process triggers, call @step(%state, dirty)
  check exit condition (natural death / term! / #!exit)
  br label %epoll_wait_site

epoll_wait_site:
  %n = call i32 @epoll_wait(i32 %epfd, i8* %events, i32 1, i32 -1)
  // for each registered trigger fd:
  //   read(fd, &buf, n) → store to trg's state field via GEP
  //   set dirty bit for this trg
  br label %tick
```

The program stays alive in a loop. Between ticks it blocks in `epoll_wait` at the kernel level — 0% CPU.

### Source-specific setup

Each built-in source type maps to specific fd creation + epoll registration:

| Source | Setup |
|--------|-------|
| `@stdin#` | `fcntl(STDIN_FILENO, F_SETFL, O_NONBLOCK)` + `epoll_ctl(epfd, EPOLL_CTL_ADD, fd, &ev)` |
| `@timer#(N)` | `timerfd_create(CLOCK_MONOTONIC, TFD_NONBLOCK)` + `timerfd_settime(tfd, 0, &its, NULL)` + `epoll_ctl(...)` |
| `@signal#(S)` | `sigemptyset(&mask); sigaddset(&mask, S); sigprocmask(...); signalfd(sfd, &mask, SFD_NONBLOCK)` + `epoll_ctl(...)` |

On wake, each trigger reads its data and stores it to the corresponding state field:
- `@stdin#`: `read(STDIN_FILENO, &ch, 1)` → store to `keypress` state field
- `@timer#(N)`: `read(tfd, &exp, 8)` → store tick count to `tick` state field
- `@signal#(S)`: `read(sfd, &siginfo, sizeof(siginfo))` → store signal number to `resize` state field

## Work Items

### Item 1 — Rewrite `emit_trg_init()` → `emit_trg_epoll_setup()`

**File:** `src/backend/llvm/emit_toplevel.rs:156-254`

Current: emits timerfd/signalfd setup + stores tfd/sfd to state.

Replace: emit `epoll_create1(0)` → store epfd to a synthetic state field. For each trigger source:

- `@stdin#`: emit `fcntl(STDIN_FILENO, F_SETFL, O_NONBLOCK)` + `epoll_ctl(epfd, EPOLL_CTL_ADD, 0, &ev)`
- `@timer#(N)`: emit `timerfd_create(...)` + `timerfd_settime(...)` + `epoll_ctl(epfd, EPOLL_CTL_ADD, tfd, &ev)`
- `@signal#(S)`: emit `sigemptyset`/`sigaddset`/`sigprocmask`/`signalfd(...)` + `epoll_ctl(epfd, EPOLL_CTL_ADD, sfd, &ev)`

Output: epfd stored as a state field, all trigger fds registered in epoll.

### Item 2 — Replace declares in `mod.rs`

**File:** `src/backend/llvm/mod.rs (declare section)`

Remove:
- `declare i32 @__trg_stdin_read()`
- `declare i32 @__trg_timerfd_open(i64)`
- `declare i32 @__trg_timerfd_read(i32)`
- `declare i32 @__trg_signalfd_open(i8*)`
- `declare i32 @__trg_signalfd_read(i32)`

Add:
- `declare i32 @epoll_create1(i32)`
- `declare i32 @epoll_ctl(i32, i32, i32, %struct.epoll_event*)`
- `declare i32 @epoll_wait(i32, %struct.epoll_event*, i32, i32)`
- `declare i64 @read(i32, i8*, i64)`
- `declare i32 @fcntl(i32, i32, ...)`
- `declare i32 @timerfd_create(i32, i32)`
- `declare i32 @timerfd_settime(i32, i32, %struct.itimerspec*, i8*)`
- `declare i32 @signalfd(i32, %struct.sigset_t*, i32)`
- `declare i32 @sigemptyset(%struct.sigset_t*)`
- `declare i32 @sigaddset(%struct.sigset_t*, i32)`
- `declare i32 @sigprocmask(i32, %struct.sigset_t*, i8*)`

### Item 3 — Modify `emit_main()` to use epoll

**File:** `src/backend/llvm/loop_engine.rs (emit_main, emit_ssa_main, etc.)`

In the tick loop, replace:
```llvm
call void @__rt_wait()
```
with:
```llvm
; block until a trigger fires
%n = call i32 @epoll_wait(i32 %epfd, %struct.epoll_event* %events, i32 1, i32 -1)
; for each trigger with a built-in source:
;   read data from fd → store to state field
;   set dirty bit for this trg
; branch back to tick body
```

This applies to all main-loop dispatch variants that have `has_wake_triggers == true`. For variants that don't (`emit_folded_pure_counter`), no change needed.

### Item 4 — Remove `emit_trg_load()`

**File:** `src/backend/llvm/emit_toplevel.rs`

`emit_trg_load()` emitted `@__trg_stdin_read()` calls. With the event loop, triggers are read from the epoll wake path instead. The function can be deleted entirely (or reduced to a no-op if still referenced).

### Item 5 — Update `state_decl` for epfd

**File:** `src/backend/llvm/mod.rs:build_field_index`

The synthetic epfd needs a slot in `%State`. Add a synthetic state declaration for `__trg_epfd: Int` that's injected during `emit_trg_epoll_setup()` but not visible to the Brief program.

### Item 6 — Compile `officina.bv` end-to-end

**Files:** `officina.bv` + generated `officina.ll`

After all changes, compile officina-cli:
```
./target/release/brief-compiler llvm officina.bv -o officina.ll
llc -O3 officina.ll -o officina.s
clang -O3 officina.s lib/runtime/brief_rt.c -o officina_bin
```

Verify: binary boots, reads stdin, renders TUI, exits on Ctrl+C.

## Files Changed

| File | Change |
|------|--------|
| `src/backend/llvm/emit_toplevel.rs` | Rewrite `emit_trg_init()` → `emit_trg_epoll_setup()`; remove `emit_trg_load()` |
| `src/backend/llvm/mod.rs` | Replace declares; add epfd to `build_field_index` |
| `src/backend/llvm/loop_engine.rs` | Modify `emit_main()` etc. to use epoll_wait + per-trg read + dirty-bit-set |
| `lib/runtime/brief_rt.c` | No changes needed (old `__trg_*` already removed in Phase 6) |

## Test Impact

| Test | What it verifies |
|------|-----------------|
| Existing 902 tests | No regressions |
| `test_trg_event_loop_emits_epoll` | Generated .ll contains `epoll_create1` and `epoll_wait` |
| `test_trg_program_links_and_runs` | End-to-end: compile, link, execute a trg-based program |
