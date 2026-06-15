# `name#()` — Compiler Intrinsic System (The Airlock)

**Date added:** 2026-06-15
**Status:** Phase B — Raw File I/O complete (2026-06-15). Phase A also complete. Supersedes `as-intrinsic.md`.

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
|---|---|---|---|---|
| **Shim** | Complex multi-step ops | C function in `brief_rt.c`, compiler emits `declare`+`call` | `tty_raw_mode#`, `spawn_with_output#` | Rewrite `brief_rt.c` function |
| **Direct** | Thin syscall wrappers | Inline asm in compiler's LLVM codegen | `open#`, `read#`, `fork#`, `mmap#` | Change codegen match arm |
| **Native** | LLVM-native instructions | Atomic LLVM IR directly | `atomic_load#`, `fence#` | Already native |

**Phase A** uses **Shim** exclusively (wraps libc in `brief_rt.c`). Direct and
Native are for later phases as each intrinsic is migrated to pure assembly.

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

### D12 — Random / Entropy

| Intrinsic | Signature | Safety | Airlock |
|---|---|---|---|
| `getrandom#` | `(buf: Int, len: Int, flags: Int) -> Int` | returns -1 on error | buf writable, len > 0 |

Irreducible: `/dev/urandom` via `read#` works but `getrandom` is the
modern kernel interface. Needed for crypto, UUIDs, hash randomization.

### D13 — System Info

| Intrinsic | Signature | Safety | Airlock |
|---|---|---|---|
| `uname#` | `() -> String` | returns empty on error | — |
| `sysinfo#` | `() -> Int` | returns 0 on error | — |
| `pagesize#` | `() -> Int` | always succeeds | — |
| `cpu_count#` | `() -> Int` | returns 1 on error | — |
| `hostname#` | `() -> String` | returns empty on error | — |
| `errno#` | `() -> Int` | always succeeds | — |

`errno#` is needed after any failing intrinsic to diagnose the error.

### D14 — Debugging

| Intrinsic | Signature | Safety | Airlock |
|---|---|---|---|
| `abort#` | `() -> Void` | never returns | — |
| `backtrace#` | `() -> List<Int>` | returns empty list if unavailable | — |

### D15 — Scheduling

| Intrinsic | Signature | Safety | Airlock |
|---|---|---|---|
| `sched_yield#` | `() -> Int` | always succeeds | — |
| `getpriority#` | `(which: Int, who: Int) -> Int` | returns -1 on error | — |
| `setpriority#` | `(which: Int, who: Int, prio: Int) -> Int` | returns -1 on error | — |

### D16 — User / Group

| Intrinsic | Signature | Safety | Airlock |
|---|---|---|---|
| `getuid#` | `() -> Int` | always succeeds | — |
| `geteuid#` | `() -> Int` | always succeeds | — |
| `getgid#` | `() -> Int` | always succeeds | — |
| `getegid#` | `() -> Int` | always succeeds | — |

### D17 — Threading

| Intrinsic | Signature | Safety | Airlock |
|---|---|---|---|
| `thread_create#` | `(fn: Int, arg: Int) -> Int` | returns -1 on error | fn is function pointer |
| `thread_join#` | `(thread: Int) -> Int` | returns -1 on error | thread from `thread_create#` |
| `thread_exit#` | `(code: Int) -> Void` | never returns | — |
| `mutex_lock#` | `(mutex: Int) -> Int` | returns -1 on error | mutex initialized |
| `mutex_unlock#` | `(mutex: Int) -> Int` | returns -1 on error | mutex locked by this thread |
| `condvar_wait#` | `(cond: Int, mutex: Int) -> Int` | returns -1 on error | both initialized |
| `condvar_signal#` | `(cond: Int) -> Int` | returns -1 on error | cond initialized |

Replaces the `pthread` dependency. The thread pool in `brief_rt.c`
currently uses pthread_barrier/pthread_create — moving to intrinsics
eliminates the libpthread link dependency.

### D18 — Resource Limits

| Intrinsic | Signature | Safety | Airlock |
|---|---|---|---|
| `getrlimit#` | `(resource: Int) -> Int` | returns -1 on error | — |
| `setrlimit#` | `(resource: Int, rlim: Int) -> Int` | returns -1 on error | — |

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

### Phase F — Signals (5 intrinsics)

**New Intrinsic variants:** D8: `SigAction`, `SigProcMask`, `Kill`,
`SignalFd`, `TimerFdCreate`

**LLVM backend:** Category **Shim**→**Direct**.

**Files deleted:** `lib/std/ffi/system.bv` frgn trigger declarations

### Phase G — Networking (13 intrinsics)

**New Intrinsic variants:** D10: `Socket`, `Bind`, `Listen`, `Accept`,
`Connect`, `Send`, `Recv`, `SendTo`, `RecvFrom`, `SetSockOpt`,
`GetSockOpt`, `Shutdown`, `GetAddrInfo`

**LLVM backend:** Category **Shim**→**Direct**.

**Files deleted:** `lib/std/ffi/http.bv`, the stubs in `emit_expr.rs` for
Socket/Bind/Listen/Accept

### Phase H — Everything Else (~20 intrinsics)

**New Intrinsic variants:**
- D6: `GetEnv`, `SetEnv`, `UnsetEnv`, `GetPid`, `GetPPid`
- D7: `ClockGetTime`, `NanoSleep`
- D12: `GetRandom`
- D13: `Uname`, `SysInfo`, `PageSize`, `CpuCount`, `HostName`, `ErrNo`
- D14: `Abort`, `BackTrace`
- D15: `SchedYield`, `GetPriority`, `SetPriority`
- D16: `GetUid`, `GetEUid`, `GetGid`, `GetEGid`
- D17: `ThreadCreate`, `ThreadJoin`, `ThreadExit`, `MutexLock`,
       `MutexUnlock`, `CondVarWait`, `CondVarSignal`
- D18: `GetRLimit`, `SetRLimit`

**LLVM backend:** Category **Shim**→**Direct** or **Native** as appropriate.

**Files deleted:**
- `lib/std/ffi/time.bv` frgn declarations (~20 entries → intrinsic + pure Brief)
- `lib/std/ffi/env.bv`
- `lib/std/ffi/process.bv` frgn declarations

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

## Reference

- Interpreter: `src/interpreter.rs` — `Expr::IntrinsicCall` match (line ~1423)
- LLVM backend: `src/backend/llvm/emit_expr.rs` — `Expr::IntrinsicCall` match (line ~355)
- AST enum: `src/ast.rs` — `Intrinsic` enum (line ~449), `from_name` (line ~484)
- Parser: `src/parser.rs` — `name#(` detection and resolution (line ~5676)
- Status: replaces `docs/architecture/features/as-intrinsic.md`
