# `name#()` — Compiler Intrinsic System (The Airlock)

**Date added:** 2026-06-15
**Updated:** 2026-06-19 — D12–D18 + 9 extras implemented (35 new intrinsics, 166 total)
**Status:** All 18 domains complete. **166** intrinsic variants. **131** emit direct libc or inline LLVM IR, **35** use brief_rt.c shims.

## Migration to Direct Libc (2026-06-16 — 2026-06-17)

The initial intrinsic implementation used **Shim** (C functions in `brief_rt.c`).
Over two sessions, ~60 of the 74 intrinsics were migrated to **Direct** (inline
libc calls in LLVM IR), eliminating the C dependency for the common case.

### Migration summary

| Category | Total | Migrated | Remaining | Remaining reason |
|----------|-------|----------|-----------|-----------------|
| D1 Memory | 5 | 5 | 0 | — |
| D2 File I/O | 15 | 15 | 0 | — |
| D3 Filesystem | 14 | 11 | 3 | `readlink`, `getcwd`, `readdir` — Brief string/list boxing |
| D4 Terminal | 5 | 4 | 1 | `tty_raw_mode` — `cfmakeraw` macro, 3-termios struct (~15 fields) |
| D5 Process | 2 | 1 | 1 | `spawn` ✅ — `system()`+`WEXITSTATUS` | `spawn_with_output` — popen+fread+pclose+string boxing |
| D6 Environment | 5 | 5 | 0 | — |
| D7 Timing | 2 | 2 | 0 | — |
| D8 Signals | 5 | 3 | 2 | `signalfd` ✅, `timerfd_create` ✅ | `sigaction`/`sigprocmask` — struct sigaction/sigset_t, Linux-specific |
| D9 Sync (atomic) | 6 | 6 | 0 | — (all Native LLVM IR) |
| D9 Sync (futex) | 1 | 1 | 0 | `futex` ✅ — stub (was already a stub in C, now `add i64 0, -1`) |
| D10 Networking | 13 | 12 | 1 | `getaddrinfo` — linked list walk + struct construction |
| D11 IPC | 6 | 6 | 0 | — |
| Thread pool | 3 | 0 | 3 | `barrier_release`, `barrier_wait`, `thread_pool_init` — global state in C |
| D19 Benchmarks | 4 | 4 | 0 | — |
| **Total** | **86** | **71** | **15** | |

## Purpose

Define the complete set of ~80 compiler-owned `#`-intrinsics that bridge between
safe Brief space and the host OS. These replace the current `frgn`+FFI-registry
pattern for all OS-level operations, making the compiler the sole owner of the
system-call boundary.

## The Airlock Model

```
┌──────────────────────────────┐     ┌──────────┐     ┌─────────────────────────┐
│      Safe Brief Space        │ ──> │ Airlock  │ ──> │      Host OS Void       │
│                              │     │  (std #) │     │                         │
│ - Contracts everywhere       │     │          │     │ - raw pointers, errno   │
│ - Borrow-checked references  │     │ compiler │     │ - raw file descriptors  │
│ - Reactive transactions      │     │  owns it │     │ - C ABI, no guarantees  │
│ - No raw memory access       │     │          │     │ - signals, syscalls     │
│ - No undefined behavior      │     │  + doc   │     │ - can fail silently     │
└──────────────────────────────┘     └──────────┘     └─────────────────────────┘
```

Brief code never crosses into Host OS Void directly. It always passes through
the Airlock: `name#(args)`. The Airlock is:

- **Compiler-owned**: dispatch via `Intrinsic::from_name()` hash, not FFI string match
- **Documented**: every intrinsic has a spec (Description / Safety / Airlock)
- **Swappable**: the codegen for each intrinsic can change (libc → asm) without
  touching user code
- **Uniform**: same syntax in every backend (interpreter, LLVM, VHDL, Webstack)

## Syntax

```
name#(arg1, arg2, ...)
```

Where `name` is a known intrinsic identifier from `Intrinsic::from_name()`.
The `#` suffix is the dispatch marker — the parser sees `#(` and creates
`Expr::IntrinsicCall { intrinsic, args }`. No import, no `frgn` declaration,
no binding files.

```brief
// Before (frgn):
frgn tty_raw_mode(enable: Bool) -> Result<void, IoError>;
tty_raw_mode(true);
let encoded = tty_size();

// After (intrinsic):
tty_raw_mode#(true);
let encoded = tty_size#();
```

## The Three Codegen Categories

| Category | What | How | Examples | Autarky effort |
|---|---|---|---|---|---|
| **Shim** | Complex multi-step ops | C function in `brief_rt.c`, compiler emits `declare`+`call` | `tty_raw_mode#`, `spawn_with_output#`, `readdir#` | Rewrite `brief_rt.c` function |
| **Direct** | Thin syscall wrappers | Compiler emits inline libc calls in LLVM IR | `open#`, `read#`, `mmap#`, `socket#` | Change codegen match arm |
| **Native** | LLVM-native instructions | Atomic LLVM IR directly | `atomic_load#`, `fence#` | Already native |

**Status (2026-06-17):** ~60/74 intrinsics migrated from Shim → Direct.
The remaining 14 Shim intrinsics require multi-call sequences, heap string boxing,
or C macros (`cfmakeraw`, `WEXITSTATUS`).` brief_rt.c` is auto-linked for all native builds.

## Complete Intrinsic Catalog (18 Domains, ~80 Intrinsics)

Each intrinsic must specify three things:

- **Description**: what it does, parameters, return value
- **Safety**: what can go wrong (errno, null, overflow, blocking)
- **Airlock precondition**: what the Brief code must ensure before calling

### D1 — Memory

| Intrinsic | Signature | Safety | Airlock |
|---|---|---|---|
| `mmap#` | `(addr: Int, len: Int, prot: Int, flags: Int, fd: Int, off: Int) -> Int` | returns -1 on failure, sets errno | `len > 0`, addr page-aligned or 0 |
| `munmap#` | `(addr: Int, len: Int) -> Int` | returns -1 on failure | addr must be from `mmap#` |
| `mprotect#` | `(addr: Int, len: Int, prot: Int) -> Int` | returns -1 on failure | addr page-aligned |
| `brk#` | `(addr: Int) -> Int` | returns old brk on failure | — |
| `mlock#` | `(addr: Int, len: Int) -> Int` | returns -1 on failure | addr page-aligned |

Irreducible because: `mmap` is the fundamental allocation primitive.
Every allocator (malloc, GC, pool) builds on it.

### D2 — Raw File I/O

| Intrinsic | Signature | Safety | Airlock |
|---|---|---|---|
| `open#` | `(path: String, flags: Int, mode: Int) -> Int` | returns -1 on error, sets errno | `path != ""` |
| `close#` | `(fd: Int) -> Int` | returns -1 on error | fd ≥ 0 |
| `read#` | `(fd: Int, buf: Int, count: Int) -> Int` | returns -1 on error, < count possible | fd ≥ 0, buf writable, count > 0 |
| `write#` | `(fd: Int, buf: Int, count: Int) -> Int` | returns -1 on error, < count possible | fd ≥ 0, buf readable |
| `lseek#` | `(fd: Int, offset: Int, whence: Int) -> Int` | returns -1 on error | fd ≥ 0 |
| `pread#` | `(fd: Int, buf: Int, count: Int, offset: Int) -> Int` | returns -1 on error | fd ≥ 0, count > 0 |
| `pwrite#` | `(fd: Int, buf: Int, count: Int, offset: Int) -> Int` | returns -1 on error | fd ≥ 0 |
| `stat#` | `(path: String) -> Int` | returns 0 on success, -1 on error | `path != ""`, buf ≥ 144 bytes |
| `fstat#` | `(fd: Int) -> Int` | returns 0 on success, -1 on error | fd ≥ 0 |
| `truncate#` | `(path: String, len: Int) -> Int` | returns -1 on error | len ≥ 0 |
| `ftruncate#` | `(fd: Int, len: Int) -> Int` | returns -1 on error | fd ≥ 0, len ≥ 0 |
| `fsync#` | `(fd: Int) -> Int` | returns -1 on error | fd ≥ 0 |
| `dup#` | `(fd: Int) -> Int` | returns -1 on error | fd ≥ 0 |
| `dup2#` | `(old: Int, new: Int) -> Int` | returns -1 on error | old ≥ 0, new ≥ 0 |
| `fcntl#` | `(fd: Int, cmd: Int, arg: Int) -> Int` | returns -1 on error | fd ≥ 0 |

Irreducible because: These are the 15 POSIX file operations.
`read_file#()` and `write_file#()` stay as convenience bundles (open+read+close).

### D3 — Filesystem

| Intrinsic | Signature | Safety | Airlock |
|---|---|---|---|
| `mkdir#` | `(path: String, mode: Int) -> Int` | returns -1 on error | `path != ""` |
| `rmdir#` | `(path: String) -> Int` | returns -1 on error | `path != ""` |
| `unlink#` | `(path: String) -> Int` | returns -1 on error | `path != ""` |
| `rename#` | `(old: String, new: String) -> Int` | returns -1 on error | both non-empty |
| `symlink#` | `(target: String, link: String) -> Int` | returns -1 on error | both non-empty |
| `readlink#` | `(path: String) -> String` | returns empty on error | `path != ""` |
| `link#` | `(old: String, new: String) -> Int` | returns -1 on error | both non-empty |
| `getcwd#` | `() -> String` | returns empty on error | — |
| `chdir#` | `(path: String) -> Int` | returns -1 on error | `path != ""` |
| `readdir#` | `(path: String) -> List<String>` | returns empty list on error | `path != ""` |
| `chmod#` | `(path: String, mode: Int) -> Int` | returns -1 on error | `path != ""` |
| `chown#` | `(path: String, uid: Int, gid: Int) -> Int` | returns -1 on error | `path != ""` |
| `umask#` | `(mask: Int) -> Int` | always succeeds (returns old mask) | — |
| `access#` | `(path: String, mode: Int) -> Int` | returns -1 on error | `path != ""` |

Irreducible because: Each is a separate Linux syscall. `list_directory()` builds on `open#`+`readdir#`+`close#`.

### D4 — Terminal / TTY

| Intrinsic | Signature | Safety | Airlock |
|---|---|---|---|
| `tty_raw_mode#` | `(enable: Bool) -> Bool` | false if not a TTY | caller must restore false on exit |
| `tty_size#` | `() -> Int` | returns 80x10000+24 fallback | — |
| `tty_read_key#` | `() -> Int` | returns -1 if no key | stdin should be raw mode |
| `ioctl#` | `(fd: Int, req: Int, arg: Int) -> Int` | returns -1 on error | fd ≥ 0 |
| `isatty#` | `(fd: Int) -> Bool` | always succeeds | fd ≥ 0 |

`tty_raw_mode#` bundles `tcgetattr` + flag-munging + `tcsetattr`.
`tty_size#` packs as `width * 10000 + height` (decode: `width = val / 10000`, `height = val % 10000`).
`ioctl#` is the universal escape hatch for device control.

### D5 — Process

| Intrinsic | Signature | Safety | Airlock |
|---|---|---|---|
| `fork#` | `() -> Int` | returns -1 on error | — |
| `execve#` | `(path: String, argv: List<String>, envp: List<String>) -> Int` | only returns on error (-1) | path must exist |
| `waitpid#` | `(pid: Int, options: Int) -> Int` | returns -1 on error | — |
| `exit#` | `(code: Int) -> Void` | never returns (same as existing) | — |
| `spawn#` | `(cmd: String, args: List<String>) -> Int` | returns -1 on error | convenience: fork+execve+waitpid |
| `spawn_with_output#` | `(cmd: String) -> String` | returns empty on error | convenience: pipe+fork+execve+read+waitpid |

`spawn_with_output#` and `spawn#` are convenience bundles over `fork#`+`execve#`+`pipe#`+`read#`+`waitpid#`.
The fine-grained primitives allow custom process management (redirection, env, chroot).

### D6 — Environment

| Intrinsic | Signature | Safety | Airlock |
|---|---|---|---|
| `getenv#` | `(name: String) -> String` | returns empty if unset | `name != ""` |
| `setenv#` | `(name: String, value: String) -> Int` | returns -1 on error | `name != ""` |
| `unsetenv#` | `(name: String) -> Int` | returns -1 on error | `name != ""` |
| `getpid#` | `() -> Int` | always succeeds | — |
| `getppid#` | `() -> Int` | always succeeds | — |

### D7 — Timing

| Intrinsic | Signature | Safety | Airlock |
|---|---|---|---|
| `clock_gettime#` | `(clock_id: Int) -> Int` | returns 0 on error | `clock_id` is CLOCK_REALTIME (0) or CLOCK_MONOTONIC (1) |
| `nanosleep#` | `(ns: Int) -> Int` | returns -1 if interrupted | `ns > 0` |

These two replace the entire `lib/std/ffi/time.bv` FFI surface (~20 frgn declarations).
Everything in that module (year/month/day extraction, formatting, parsing) is
implementable in pure Brief from `clock_gettime#()` — no intrinsic needed.

### D8 — Signals

| Intrinsic | Signature | Safety | Airlock |
|---|---|---|---|
| `sigaction#` | `(signum: Int, handler: Int) -> Int` | returns -1 on error | signum > 0 |
| `sigprocmask#` | `(how: Int, mask: Int) -> Int` | returns -1 on error | — |
| `kill#` | `(pid: Int, sig: Int) -> Int` | returns -1 on error | pid ≥ -1 |
| `signalfd#` | `(mask: Int) -> Int` | returns -1 on error | — |
| `timerfd_create#` | `(hz: Int) -> Int` | returns -1 on error | `hz > 0` |

`signalfd#` and `timerfd_create#` replace the entire C-level signal handler
infrastructure in `brief_rt.c` (the `__trg_*` functions and `signal()` calls).

### D9 — Synchronization

| Intrinsic | Signature | Safety | Airlock |
|---|---|---|---|
| `atomic_load#` | `(addr: Int, order: Int) -> Int` | always succeeds | addr ≥ 4096 (userspace) |
| `atomic_store#` | `(addr: Int, val: Int, order: Int) -> Void` | always succeeds | addr ≥ 4096 |
| `atomic_cas#` | `(addr: Int, expected: Int, new: Int, order: Int) -> Int` | always succeeds | addr ≥ 4096 |
| `atomic_xchg#` | `(addr: Int, val: Int, order: Int) -> Int` | always succeeds | addr ≥ 4096 |
| `atomic_add#` | `(addr: Int, val: Int, order: Int) -> Int` | always succeeds | addr ≥ 4096 |
| `fence#` | `(order: Int) -> Void` | always succeeds | — |
| `futex#` | `(uaddr: Int, op: Int, val: Int, timeout: Int, uaddr2: Int, val3: Int) -> Int` | returns -1 on error | uaddr ≥ 4096 |

These map to LLVM atomic instructions directly (Category: **Native**). No syscall
needed (except `futex#`). Replaces the entire Metro atomic FFI.

### D10 — Networking

| Intrinsic | Signature | Safety | Airlock |
|---|---|---|---|
| `socket#` | `(domain: Int, type: Int, protocol: Int) -> Int` | returns -1 on error | — |
| `bind#` | `(fd: Int, addr: Int, addrlen: Int) -> Int` | returns -1 on error | fd ≥ 0 |
| `listen#` | `(fd: Int, backlog: Int) -> Int` | returns -1 on error | fd ≥ 0 |
| `accept#` | `(fd: Int, addr: Int, addrlen: Int) -> Int` | returns -1 on error | fd ≥ 0 |
| `connect#` | `(fd: Int, addr: Int, addrlen: Int) -> Int` | returns -1 on error | fd ≥ 0 |
| `send#` | `(fd: Int, buf: Int, len: Int, flags: Int) -> Int` | returns -1 on error | fd ≥ 0, buf readable |
| `recv#` | `(fd: Int, buf: Int, len: Int, flags: Int) -> Int` | returns -1 on error | fd ≥ 0, buf writable |
| `sendto#` | `(fd: Int, buf: Int, len: Int, flags: Int, addr: Int, addrlen: Int) -> Int` | returns -1 on error | fd ≥ 0 |
| `recvfrom#` | `(fd: Int, buf: Int, len: Int, flags: Int, addr: Int, addrlen: Int) -> Int` | returns -1 on error | fd ≥ 0 |
| `setsockopt#` | `(fd: Int, level: Int, opt: Int, val: Int, len: Int) -> Int` | returns -1 on error | fd ≥ 0 |
| `getsockopt#` | `(fd: Int, level: Int, opt: Int, val: Int, len: Int) -> Int` | returns -1 on error | fd ≥ 0 |
| `shutdown#` | `(fd: Int, how: Int) -> Int` | returns -1 on error | fd ≥ 0 |
| `getaddrinfo#` | `(node: String, service: String) -> List<String>` | returns empty on error | — |

Each is a separate syscall. The current stubs (Socket, Bind, Listen, Accept)
are the first four — the full set adds client-side connections, data transfer,
and DNS resolution.

### D11 — IPC

| Intrinsic | Signature | Safety | Airlock |
|---|---|---|---|
| `pipe#` | `(fds: Int) -> Int` | returns -1 on error | — |
| `shm_open#` | `(name: String, flags: Int, mode: Int) -> Int` | returns -1 on error | `name != ""` |
| `shm_unlink#` | `(name: String) -> Int` | returns -1 on error | `name != ""` |
| `sem_open#` | `(name: String, flags: Int, mode: Int, value: Int) -> Int` | returns -1 on error | `name != ""` |
| `sem_wait#` | `(sem: Int) -> Int` | returns -1 on error | sem ≥ 0 |
| `sem_post#` | `(sem: Int) -> Int` | returns -1 on error | sem ≥ 0 |

`pipe#` is fundamental for process I/O (used by `spawn_with_output#`).
`shm_open#` + `mmap#` replace the entire Metro SHM FFI.

### D12 — Random / Entropy ✅ (2026-06-19)

| Intrinsic | Signature | Safety | Airlock | Codegen |
|---|---|---|---|---|
| `errno#` | `() -> Int` | always succeeds | — | Shim (`__errno__`) |
| `getrandom#` | `(buf: Int, len: Int, flags: Int) -> Int` | returns -1 on error | buf writable, len > 0 | Shim (`__getrandom__`) |

`errno#` is needed after any failing intrinsic to diagnose the error.
`getrandom#` wraps the Linux `getrandom()` syscall (or `/dev/urandom` fallback).

### D13 — System Info ✅ (2026-06-19)

| Intrinsic | Signature | Safety | Airlock | Codegen |
|---|---|---|---|---|
| `pagesize#` | `() -> Int` | always succeeds | — | Direct (`sysconf`) |
| `cpu_count#` | `() -> Int` | returns 1 on error | — | Direct (`sysconf`) |
| `hostname#` | `() -> String` | returns empty on error | — | Shim (`__hostname__`) |
| `uname#` | `() -> String` | returns empty on error | — | Shim (`__uname__`) |
| `strerror#` | `(errnum: Int) -> String` | always succeeds | — | Shim (`__strerror__`) |
| `strsignal#` | `(signum: Int) -> String` | always succeeds | — | Shim (`__strsignal__`) |

`pagesize#` and `cpu_count#` are pure (no side effects — constant for process lifetime).
`strerror#` provides human-readable error messages for `errno#` results.
`strsignal#` provides human-readable signal names for signal numbers.

### D14 — Debugging ✅ (2026-06-19)

| Intrinsic | Signature | Safety | Airlock | Codegen |
|---|---|---|---|---|
| `abort#` | `() -> Void` | never returns | — | Direct (`abort()`) |
| `backtrace#` | `() -> List<Int>` | returns empty list if unavailable | — | Shim (`__backtrace__`) |

`abort#` triggers SIGABRT + core dump. `backtrace#` returns program counter addresses
(use with `dladdr` or `addr2line` for symbol resolution).

### D15 — Scheduling ✅ (2026-06-19)

| Intrinsic | Signature | Safety | Airlock | Codegen |
|---|---|---|---|---|
| `sched_yield#` | `() -> Int` | always succeeds | — | Direct (`sched_yield`) |
| `getpriority#` | `(which: Int, who: Int) -> Int` | returns -1 on error | — | Direct (`getpriority`) |
| `setpriority#` | `(which: Int, who: Int, prio: Int) -> Int` | returns -1 on error | — | Direct (`setpriority`) |

`getpriority#`/`setpriority#` use `PRIO_PROCESS` (0), `PRIO_PGRP` (1), or `PRIO_USER` (2) for `which`.

### D16 — User / Group ✅ (2026-06-19)

| Intrinsic | Signature | Safety | Airlock | Codegen |
|---|---|---|---|---|
| `getuid#` | `() -> Int` | always succeeds | — | Direct (`getuid`) |
| `geteuid#` | `() -> Int` | always succeeds | — | Direct (`geteuid`) |
| `getgid#` | `() -> Int` | always succeeds | — | Direct (`getgid`) |
| `getegid#` | `() -> Int` | always succeeds | — | Direct (`getegid`) |
| `getpwuid#` | `(uid: Int) -> String` | returns empty on error | uid ≥ 0 | Shim (`__getpwuid__`) |
| `getgrgid#` | `(gid: Int) -> String` | returns empty on error | gid ≥ 0 | Shim (`__getgrgid__`) |

`getpwuid#` returns `"name:dir:shell"` colon-separated. Use pure-Brief parsing to extract fields.

### D17 — Threading ✅ (2026-06-19)

| Intrinsic | Signature | Safety | Airlock | Codegen |
|---|---|---|---|---|
| `thread_create#` | `(fn_ptr: Int, arg: Int) -> Int` | returns -1 on error | fn_ptr is callable | Shim (`__thread_create__`) |
| `thread_join#` | `(thread: Int) -> Int` | returns -1 on error | thread from `thread_create#` | Shim (`__thread_join__`) |
| `thread_exit#` | `(code: Int) -> Void` | never returns | — | Shim (`__thread_exit__`) |
| `mutex_lock#` | `(mptr: Int) -> Int` | returns -1 on error | mptr is initialized mutex | Shim (`__mutex_lock__`) |
| `mutex_unlock#` | `(mptr: Int) -> Int` | returns -1 on error | mptr locked by this thread | Shim (`__mutex_unlock__`) |
| `condvar_wait#` | `(cptr: Int, mptr: Int) -> Int` | returns -1 on error | both initialized | Shim (`__condvar_wait__`) |
| `condvar_signal#` | `(cptr: Int) -> Int` | returns -1 on error | cptr initialized | Shim (`__condvar_signal__`) |
| `condvar_broadcast#` | `(cptr: Int) -> Int` | returns -1 on error | cptr initialized | Shim (`__condvar_broadcast__`) |

All threading intrinsics wrap `pthread_*` functions. Thread handles are `pthread_t` packed as `i64`.

### D18 — Resource Limits ✅ (2026-06-19)

| Intrinsic | Signature | Safety | Airlock | Codegen |
|---|---|---|---|---|
| `getrlimit#` | `(resource: Int) -> Int` | returns -1 on error | — | Shim (`__getrlimit__`) |
| `setrlimit#` | `(resource: Int, packed: Int) -> Int` | returns -1 on error | — | Shim (`__setrlimit__`) |

Limits are packed as `(cur << 32) | max`. Decode: `cur = packed >> 32`, `max = packed & 0xFFFFFFFF`.
Resource constants: `RLIMIT_CPU=0`, `RLIMIT_FSIZE=1`, `RLIMIT_DATA=2`, `RLIMIT_STACK=3`,
`RLIMIT_CORE=4`, `RLIMIT_NOFILE=7`, `RLIMIT_AS=9`, `RLIMIT_NPROC=6`, `RLIMIT_MEMLOCK=8`.

### Extra — Cross-platform Utilities ✅ (2026-06-19)

| Intrinsic | Signature | Safety | Airlock | Codegen |
|---|---|---|---|---|
| `realpath#` | `(path: String) -> String` | returns empty on error | path != "" | Shim (`__realpath__`) |
| `mkstemp#` | `(template: String) -> Int` | returns -1 on error | template ends with XXXXXX | Shim (`__mkstemp__`) |
| `mkdtemp#` | `(template: String) -> String` | returns empty on error | template ends with XXXXXX | Shim (`__mkdtemp__`) |
| `dlopen#` | `(filename: String) -> Int` | returns 0 on error | filename != "" | Shim (`__dlopen__`) |
| `dlsym#` | `(handle: Int, symbol: String) -> Int` | returns 0 on error | handle from dlopen# | Shim (`__dlsym__`) |
| `dlclose#` | `(handle: Int) -> Int` | returns -1 on error | handle from dlopen# | Shim (`__dlclose__`) |
| `ttyname#` | `(fd: Int) -> String` | returns empty on error | fd is a terminal | Shim (`__ttyname__`) |
| `strlen#` | `(ptr: Ptr<Byte>) -> Int` | C string length, no bounds check | ptr != null | Direct (`@strlen`) |

`strlen#` is the pure-read strlen(3) wrap. Used by the CString lazy lens pattern:
`Size = _ :> Ptr :> strlen#;`. Zero-cost: `strlen` runs only when `Size`
is explicitly queried; character access via `At(i)` never calls strlen.

### Phase I–VI — D12–D18 + Extras (completed 2026-06-19)

**35 new intrinsics** across 7 domains + 9 extras:

| Phase | Domain | Intrinsics | Count | Codegen |
|---|---|---|---|---|
| I | D12 Random + D13 System Info (subset) | `errno`, `getrandom`, `pagesize`, `cpu_count`, `hostname`, `abort` | 6 | 2 Direct + 4 Shim |
| II | D13 System Info (remainder) + Extras | `uname`, `strerror`, `strsignal`, `realpath` | 4 | 4 Shim |
| III | D16 User/Group | `getuid`, `geteuid`, `getgid`, `getegid`, `getpwuid`, `getgrgid` | 6 | 4 Direct + 2 Shim |
| IV | D15 Scheduling + D18 Resource Limits | `sched_yield`, `getpriority`, `setpriority`, `getrlimit`, `setrlimit` | 5 | 3 Direct + 2 Shim |
| V | D17 Threading | `thread_create`, `thread_join`, `thread_exit`, `mutex_lock`, `mutex_unlock`, `condvar_wait`, `condvar_signal`, `condvar_broadcast` | 8 | 8 Shim |
| VI | Extras | `mkstemp`, `mkdtemp`, `dlopen`, `dlsym`, `dlclose`, `ttyname` | 6 | 6 Shim |

**Total after all phases:** 166 intrinsic variants (131 Phase A–H + 35 Phase I–VI).

## Implementation Phases

### Phase A — Terminal + Process (completed 2026-06-15)

**New Intrinsic variants:**
- D4: `TtyRawMode`, `TtySize`, `TtyReadKey`, `IoCtl`, `IsTty`
- D5: `SpawnWithOutput`, `Spawn`

**LLVM backend:** Category **Shim** — emit `declare i64 @brief_*(i64, ...)`
and `call`, implemented in `brief_rt.c` via libc.

**Interpreter:** Rust `std::process::Command`, `libc` crate, direct impl.

**Files changed:**
- `src/ast.rs` — add variants + `from_name` + `name`
- `src/interpreter.rs` — add eval match arms + helper functions (`set_tty_raw_mode`, `get_terminal_size`, `read_key_nonblocking`)
- `src/backend/llvm/emit_expr.rs` — add LLVM codegen match arms
- `src/backend/llvm/emit_toplevel.rs` — add `declare` stubs in `emit_declares`
- `src/typechecker.rs` — add return type dispatch match arms
- `lib/runtime/brief_rt.c` — add C function implementations (`brief_tty_raw_mode`, `brief_tty_size`, `brief_tty_read_key`, `brief_ioctl`, `brief_isatty`, `brief_spawn_with_output`, `brief_spawn`)

**Files deleted:** `lib/std/ffi/tty.bv` (replaced by intrinsics)

**Tests added:**
- `src/ast.rs` — roundtrip test for `Intrinsic::from_name` + `name()` for all Phase A entries
- `src/interpreter.rs` — eval tests for `tty_raw_mode#`, `tty_size#`, `tty_read_key#`, `ioctl#`, `isatty#`, `spawn_with_output#`, `spawn#`
- `src/typechecker.rs` — return type inference tests for all Phase A intrinsics

**Test result:** 809 pass, 0 fail (up from 713)

### Phase B — Raw File I/O (completed 2026-06-15)

**New Intrinsic variants:** D2: `Open`, `Close`, `Read`, `Write`, `LSeek`,
`PRead`, `PWrite`, `Stat`, `FStat`, `Truncate`, `FTruncate`, `FSync`,
`FDup`, `FDup2`, `FCntl`

**LLVM backend:** Category **Shim** — emit `declare i64 @brief_*(i64, ...)`
and `call`, implemented in `brief_rt.c` via POSIX.

**Interpreter:** Rust `libc` crate for raw fd operations. `read#`/`pread#`
allocate temporary buffers (caller's pointer is opaque in interpreter).
`write#`/`pwrite#` return `-1` (can't dereference opaque pointer).

**Files changed:**
- `src/ast.rs` — add 15 variants + `from_name` + `name`
- `src/interpreter.rs` — add eval match arms for all 15
- `src/backend/llvm/emit_expr.rs` — add LLVM codegen match arms
- `src/backend/llvm/emit_toplevel.rs` — add 15 `declare` stubs
- `src/typechecker.rs` — add return type (all `Type::Int`)
- `lib/runtime/brief_rt.c` — add C function implementations

**Tests added:**
- `src/ast.rs` — 15 roundtrip tests (from_name + name)
- `src/interpreter.rs` — 22 eval tests (type errors + edge cases)
- `src/typechecker.rs` — 15 inference tests (one per variant)

**Test result:** after Phase A+B

### Phase C — Filesystem (completed 2026-06-15)

**New Intrinsic variants:** D3: `MkDir`, `RmDir`, `Unlink`, `Rename`,
`SymLink`, `ReadLink`, `Link`, `GetCwd`, `ChDir`, `ReadDir`, `ChMod`,
`ChOwn`, `UMask`, `Access`

**LLVM backend:** Category **Shim** — emit `declare i64 @brief_*(i64, ...)`
and `call`, implemented in `brief_rt.c` via POSIX.

**Interpreter:** Rust `libc` crate for filesystem ops.
`readdir#` uses `std::fs::read_dir`, `readlink#`/`getcwd#` use libc.

**Return types:** `readlink#`/`getcwd#` → String; `readdir#` → List;
all others → Int.

**Tests added:**
- `src/ast.rs` — 14 roundtrip tests
- `src/interpreter.rs` — 20 eval tests (type errors + edge cases)
- `src/typechecker.rs` — 14 inference tests

### Phase D — Memory + Synchronization (completed 2026-06-15)

**New Intrinsic variants:** D1: `Mmap`, `MUnmap`, `MProtect`, `Brk`, `MLock`
+ D9: `AtomicLoad`, `AtomicStore`, `AtomicCas`, `AtomicXchg`, `AtomicAdd`,
`Fence`, `Futex`

**LLVM backend:** D1 → **Shim** (call i64 @brief_*). D9 atomic ops →
**Native** (LLVM atomic IR: load atomic, store atomic, cmpxchg, atomicrmw,
fence). Futex → **Shim** (call i64 @brief_futex).

**Interpreter:** D1 uses libc::mmap/munmap/mprotect/sbrk/mlock. D9 atomic
ops are stubs (opaque pointers can't be dereferenced). Brk returns current
break via sbrk(0). Futex returns -1.

**Tests added:**
- `src/ast.rs` — 12 roundtrip tests
- `src/interpreter.rs` — 22 eval tests (type errors + stub values)
- `src/typechecker.rs` — 12 inference tests

### Phase E — IPC (completed 2026-06-15)

**New Intrinsic variants:** D11: `Pipe`, `ShmOpen`, `ShmUnlink`,
`SemOpen`, `SemWait`, `SemPost`

**LLVM backend:** Category **Shim** — emit `call i64 @brief_*`, implemented
in `brief_rt.c` via POSIX (pipe, shm_open, shm_unlink, sem_open, sem_wait,
sem_post).

**Interpreter:** Uses `libc::pipe` (writes fds through opaque pointer),
`libc::shm_open`, `libc::shm_unlink`, `libc::sem_open`, `libc::sem_wait`,
`libc::sem_post`. All return Int.

**Tests added:**
- `src/ast.rs` — 6 roundtrip tests
- `src/interpreter.rs` — 6 eval tests (type errors)
- `src/typechecker.rs` — 6 inference tests

### Phase B — Raw File I/O (15 intrinsics)

**New Intrinsic variants:** D2: `Open`, `Close`, `Read`, `Write`, `LSeek`,
`PRead`, `PWrite`, `Stat`, `FStat`, `Truncate`, `FTruncate`, `FSync`,
`FDup`, `FDup2`, `FCntl`

**LLVM backend:** Category **Shim** → eventually **Direct** (inline asm syscall).

**Interpreter:** Rust `std::fs` and `std::os::unix` for raw fd operations.

**Files deleted:** `lib/std/ffi/io.bv` frgn declarations (path ops stay in stdlib as pure Brief)

### Phase C — Filesystem (14 intrinsics)

**New Intrinsic variants:** D3: `MkDir`, `RmDir`, `Unlink`, `Rename`,
`SymLink`, `ReadLink`, `Link`, `GetCwd`, `ChDir`, `ReadDir`, `ChMod`,
`ChOwn`, `UMask`, `Access`

**LLVM backend:** Category **Shim** → **Direct**.

Interpreter: Rust `std::fs`.

### Phase D — Memory + Synchronization (10 intrinsics)

**New Intrinsic variants:** D1: `MMap`, `MUnmap`, `MProtect`, `Brk`, `MLock`
+ D9: `AtomicLoad`, `AtomicStore`, `AtomicCas`, `AtomicXchg`, `AtomicAdd`,
`Fence`, `Futex`

**LLVM backend:** D1 → **Shim**→**Direct**. D9 → **Native** (LLVM atomic IR directly).

**Files deleted:** `lib/std/ffi/shm.bv` (replaced by intrinsics)

### Phase E — IPC (6 intrinsics)

**New Intrinsic variants:** D11: `Pipe`, `ShmOpen`, `ShmUnlink`,
`SemOpen`, `SemWait`, `SemPost`

**LLVM backend:** Category **Shim**→**Direct**.

### Phase F — Signals (completed 2026-06-15)

**New Intrinsic variants:** D8: `SigAction`, `SigProcMask`, `Kill`,
`SignalFd`, `TimerFdCreate`

**LLVM backend:** Category **Shim** — emit `call i64 @brief_*`, implemented
in `brief_rt.c` via POSIX (sigaction, sigprocmask, kill, signalfd, timerfd_create).

**Interpreter:** Uses `libc::sigaction`, `libc::sigprocmask`, `libc::kill`,
`libc::signalfd`, `libc::timerfd_create`. All return Int.

**Tests added:**
- `src/ast.rs` — 5 roundtrip tests
- `src/interpreter.rs` — 5 eval tests (type errors)
- `src/typechecker.rs` — 5 inference tests

### Phase G — Networking (completed 2026-06-15)

**New Intrinsic variants:** D10: `Socket`, `Bind`, `Listen`, `Accept`,
`Connect`, `Send`, `Recv`, `SendTo`, `RecvFrom`, `SetSockOpt`,
`GetSockOpt`, `Shutdown`, `GetAddrInfo`

**LLVM backend:** Category **Shim** — emit `call i64 @brief_*`, implemented
in `brief_rt.c` via POSIX socket API.

**Interpreter:** Uses `libc::socket`, `libc::bind`, `libc::listen`,
`libc::accept`, `libc::connect`, `libc::send`, `libc::recv`, `libc::sendto`,
`libc::recvfrom`, `libc::setsockopt`, `libc::getsockopt`, `libc::shutdown`,
`libc::getaddrinfo`. All return Int.

**Tests added:**
- `src/ast.rs` — 13 roundtrip tests
- `src/interpreter.rs` — 13 eval tests (type errors)
- `src/typechecker.rs` — 13 inference tests

### Phase H — Everything Else (completed 2026-06-15)

**New Intrinsic variants:** D6: `GetEnv`, `SetEnv`, `UnsetEnv`, `GetPid`, `GetPPid`
+ D7: `ClockGetTime`, `NanoSleep`

**LLVM backend:** Category **Shim** — emit `call i64 @brief_*`, implemented
in `brief_rt.c` via POSIX (getenv, setenv, unsetenv, getpid, getppid,
clock_gettime, nanosleep).

**Interpreter:** Uses `std::env` for env vars, `libc::getpid`, `libc::getppid`,
`libc::clock_gettime`, `libc::nanosleep`. `getenv#` returns String (empty on
missing), `clock_gettime#` returns Int as nanoseconds since epoch.

**Tests added:**
- `src/ast.rs` — 7 roundtrip tests
- `src/interpreter.rs` — 5 eval tests (type errors; GetPid/GetPPid take no args)
- `src/typechecker.rs` — 7 inference tests

## LLVM Backend Type Policy

The expression system currently boxes everything in `i64` (except Float,
which is native `float` in SSA). This creates ~130 marshaling instructions
(trunc/zext/ptrtoint/inttoptr/bitcast) per transaction body.

**Phase A policy:** Keep `i64` everywhere. The marshaling overhead is
real but LLVM's InstCombine+SROA passes eliminate most of it. The
intrinsic calls themselves are `call i64 @brief_*(i64, ...)` — they
fit the i64 pattern naturally.

**Post-intrinsics cleanup:** After all intrinsic phases are stable,
do a backend refactor pass that makes emission produce natively-typed
SSA registers:
- Char → `i32` (no zext to i64)
- Bool → `i1` for comparisons, `i8` for storage (no zext)
- String → `i8*` directly (no ptrtoint boxing)

This is purely an LLVM backend change. It doesn't affect Brief semantics,
the interpreter, stdlib, or intrinsics. Do it as a standalone cleanup when
ready. See `docs/architecture/features/expr-eqsat.md` for the expression
simplification pass that already handles some of this.

## Files to Delete Across All Phases

| File | Phase | Reason |
|---|---|---|
| `lib/std/ffi/tty.bv` | A | All 3 frgn + defn wrappers → intrinsics |
| `lib/std/ffi/io.bv` (frgn lines) | B+C | `__read_file` etc. → `read#`+`write#`+`open#` etc. |
| `lib/std/ffi/process.bv` (frgn lines) | A+H | `__spawn`, `__env_var` etc. → intrinsics |
| `lib/std/ffi/shm.bv` | D+E | `__mmap*`, `__atomic*`, `__shm*` → intrinsics |
| `lib/std/ffi/system.bv` (frgn lines) | F | `__sig*`, `__timer*` → intrinsics |
| `lib/std/ffi/time.bv` (frgn lines) | H | `__now*`, `__year*`, `__format*` → `clock_gettime#` + pure Brief |
| `lib/std/ffi/env.bv` | H | `__get_env_int` → `getenv#` |
| `lib/std/ffi/http.bv` | G | `__http_get`, `__http_post` → socket intrinsics + pure Brief |

After all phases, the remaining `frgn` declarations should be for **pure
computation** only (encoding, XXHASH, JSON) — operations that don't
cross the OS boundary.

## Existing Intrinsic Cleanup

The current `Intrinsic` enum has the following stubs that must be
replaced by the new system:

| Current stub | Replaced by |
|---|---|
| `Socket`, `Bind`, `Listen`, `Accept` | D10 full networking set |
| `Sort`, `Reverse`, `Range` | Pure Brief stdlib (not intrinsics) |
| `ReadFile`, `WriteFile` | Keep as convenience, add `open#`+`read#`+`write#`+`close#` |
| `Println` | Keep as convenience for `write#(1, msg, len)` |
| `Readln` | `read#(0, buf, 1)` in a txn loop |
| `Sleep` | `nanosleep#` |
| `Time` | `clock_gettime#(CLOCK_REALTIME)` |
| `Exit` | Keep as `exit#(code)` → wraps D5 |

### Phase I — Benchmark Intrinsics (Direct Libc, completed 2026-06-16)

**New Intrinsic variants:** D19: `PrintInt`, `PutChar`, `PrintFloat`, `GetEnvInt`

Same syntax as existing `#` intrinsics, but with a fundamentally different codegen
category: **Direct Libc** — no `brief_rt.c` shim, no LTO linking. The compiler
emits inline calls to libc functions (`fprintf`, `fputc`, `getenv`, `atol`).

| Intrinsic | Signature | LLVM IR emitted |
|---|---|---|
| `print_int#` | `(n: Int) -> Bool` | `fprintf(stdout, "%ld\n", n)` + `fflush(stdout)` |
| `putchar#` | `(c: Char) -> Bool` | `fputc(c, stdout)` + `fflush(stdout)` |
| `print_float#` | `(d: Float) -> Bool` | `fprintf(stdout, "%.9f\n", d)` + `fflush(stdout)` |
| `getenv_int#` | `(name: String) -> Int` | `getenv(name)` → `atol(result)` or 0 if null |

**Format string pool** (shared globals at module level):
```llvm
@FMT_INT = private unnamed_addr constant [5 x i8] c"%ld\0A\00"
@FMT_FLOAT = private unnamed_addr constant [6 x i8] c"%.9f\0A\00"
@FMT_STR = private unnamed_addr constant [4 x i8] c"%s\0A\00"
```

**String marshaling:** Brief's string struct has layout `{ ptr_to_data: i64, length: i64, data: [N x i8] }`.
The first field contains the address of the actual data bytes. The LLVM codegen
loads this field and passes it to libc:
```llvm
%sptr = inttoptr i64 %name to ptr
%sp   = bitcast ptr %sptr to i64*
%data = load i64, i64* %sp
%str  = inttoptr i64 %data to ptr
%res  = call ptr @getenv(ptr %str)
```

**Why Direct Libc?** These intrinsics are the toolchain's canonical output path.
No C runtime dependency means the resulting `.ll` files are `clang`-compilable
without any additional link step (besides `-lm`). The benchmarks that use these
intrinsics no longer need `import "link/brief_rt.c"` or `frgn` declarations.

**Side-effect annotation:** `has_side_effects()` returns `true` for all four,
preventing the optimizer from folding them away even when their return values
are unused. This is critical for benchmarks — `print_int#(n)` must remain
observable regardless of optimization level.

**Status:** 26 benchmark files migrated. `benchmarks/brief_rt.c` and
`runtime/brief_rt.c` deleted (no longer referenced).

## Side-Effect Metadata

Intrinsics now carry `has_side_effects()` metadata (added 2026-06-16):

```rust
impl Intrinsic {
    fn has_side_effects(&self) -> bool {
        match self {
            // Pure/mathematical — can fold safely
            Intrinsic::Sqrt | Intrinsic::Fabs | Intrinsic::Ceil
            | Intrinsic::Floor
            | Intrinsic::Ctpop | Intrinsic::Ctlz | Intrinsic::Cttz
            | Intrinsic::Abs | Intrinsic::Bitreverse
            | Intrinsic::Bytes | Intrinsic::Size
            => false,
            // Everything else is observable — cannot fold
            _ => true,
        }
    }
}
```

The `references_triggers_or_ffi` function in `transition_graph.rs` now uses
this method instead of treating all `IntrinsicCall` as impure:

```rust
// Before:
Expr::IntrinsicCall { .. } => true,  // all are impure

// After:
Expr::IntrinsicCall { intrinsic, .. } => intrinsic.has_side_effects(),
```

This enables folding of pure intrinsics like `sqrt#(9.0)` → `3.0` at compile
time while preserving observable I/O intrinsics like `print_int#(n)`.

## Reference

- Interpreter: `src/interpreter.rs` — `Expr::IntrinsicCall` match (line ~1475)
- LLVM backend: `src/backend/llvm/emit_expr.rs` — `Expr::IntrinsicCall` match (line ~427)
- AST enum: `src/ast.rs` — `Intrinsic` enum (line ~449), `from_name` (line ~577), `has_side_effects` (line ~576)
- Parser: `src/parser.rs` — `name#(` detection and resolution (line ~5676)
- Reorder pass: `src/backend/llvm/reorder.rs` — `collect_reads_from_expr` now handles `IntrinsicCall`
- Status: replaces `docs/architecture/features/as-intrinsic.md`
