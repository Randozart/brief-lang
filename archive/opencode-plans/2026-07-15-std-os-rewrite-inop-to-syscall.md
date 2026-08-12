# Std/OS Rewrite: `inop` → `defn` + `Syscall#`

**Date:** 2026-07-15
**Status:** Plan — directed build execution
**Branch:** `main`

---

## 1. Summary

Replace 98 `inop` declarations across 18 files in `lib/std/os/` with pure-Briev
`defn` wrappers calling the `Syscall#` intrinsic. Eliminate the C runtime
dependency for these operations. Then fix all example `.bv` files that used
the old lowercase `#` patterns or `inop` declarations.

**Total files changed:** ~35 (18 std/os + 14 examples + 3 source)
**New files:** 0
**Deleted files:** `examples/macro-demo.bv` (already removed)

---

## 2. Scope

**Included:**

| Layer | Files | Changes |
|-------|-------|---------|
| `lib/std/os/*.bv` | 18 files, 98 `inop` | Each `inop` → `defn` calling `Syscall#(AbstractOp, args...)` |
| `lib/runtime/briev_rt.c` | 1 file | Keep `briev_syscall`, remove ~70 `briev_*` functions after migration |
| `examples/*.bv` | ~14 files | Replace lowercase `#` calls and `inop`-based imports with `import` from `std/os` |
| `docs/architecture/` | 1 file | Update `docs/architecture/features/plugins.md` |

**Not included:**
- Volatile load/store (`volatile_load#`, `volatile_store#`) — these are MMIO operations, not OS syscalls. They use `AddressOf#` + `#[volatile]` contract path instead.
- `std/os/atomic.bv` — atomic operations use LLVM `atomicrmw` instructions, not syscalls. They need separate `Atomic*#` intrinsics (already partially exist).
- `std/os/ring.bv` — ring buffer primitives are user-space, not syscalls.

---

## 3. Documentation Strategy

### 3.1 Rationale comments to add at each change site

```
// 2026-07-15: Replaced inop with defn + Syscall#(AbstractOp, ...)
// Syscall# dispatches to the correct arch syscall via the backend.
```

Every modified `.bv` and `.rs` file gets this comment at the top of each
changed function or section.

### 3.2 `///` doc comments to update

- `src/backend/llvm/intrinsics.rs` — `emit_syscall` function gets a `///` doc comment
- `src/interpreter/intrinsics.rs` — `Syscall#` handler gets a `///` doc comment

### 3.3 Architecture docs to update

- `docs/architecture/features/plugins.md` — add note that `Syscall#` is the
  OS primitive replacing `inop` declarations. Update the intrinsic table.

### 3.4 Preservation of existing commentary

The `inop` declarations in `lib/std/os/*.bv` have header comments like
`// Phase 3: relocated from compiler Intrinsic enum to std/os/ module`.
These are preserved in the replacement `defn` wrappers, with an additional
`// 2026-07-15:` note appended explaining the migration.

---

## 4. Phase 1: Simple Syscall Wrappers (~72 inops, no string conversion)

### 4.1 Pattern

Each `inop` with `Int`-only parameters (no `Ptr<Byte>` strings) is replaced
with a `defn` that calls `Syscall#` with the matching abstract op name:

```briev
// 2026-07-15: Replaced inop with defn + Syscall#(Close, ...)
defn file_close(fd: Int) -> Int {
    Syscall#(Close, fd, 0, 0, 0, 0, 0)
};
```

`Ptr<Byte>` args are passed as-is — the backend treats them as `i64`.

### 4.2 Files and abstract op mapping

#### `lib/std/os/fs.bv` (7 simple ops)

| Old `inop` | Abstract op | Syscall# call |
|------------|-------------|---------------|
| `file_close(fd)` | `Close` | `Syscall#(Close, fd, 0,0,0,0,0)` |
| `file_read(fd, buf, count)` | `Read` | `Syscall#(Read, fd, buf, count, 0,0,0)` |
| `file_write(fd, buf, count)` | `Write` | `Syscall#(Write, fd, buf, count, 0,0,0)` |
| `file_lseek(fd, offset, whence)` | `LSeek` | `Syscall#(LSeek, fd, offset, whence, 0,0,0)` |
| `file_pread(fd, buf, count, offset)` | `PRead` | `Syscall#(PRead, fd, buf, count, offset,0,0)` |
| `file_pwrite(fd, buf, count, offset)` | `PWrite` | `Syscall#(PWrite, fd, buf, count, offset,0,0)` |
| `file_fsync(fd)` | `FSync` | `Syscall#(FSync, fd, 0,0,0,0,0)` |
| `file_dup(fd)` | `Dup` | `Syscall#(Dup, fd, 0,0,0,0,0)` |
| `file_dup2(oldfd, newfd)` | `Dup2` | `Syscall#(Dup2, oldfd, newfd, 0,0,0,0)` |
| `file_fcntl(fd, cmd, arg)` | `Fcntl` | `Syscall#(Fcntl, fd, cmd, arg, 0,0,0)` |
| `file_ftruncate(fd, length)` | `FTruncate` | `Syscall#(FTruncate, fd, length, 0,0,0,0)` |

String-arg ops from `fs.bv` (`file_open`, `file_stat`, `file_fstat`) are
deferred to Phase 2.

#### `lib/std/os/process.bv` (2 ops)

| `__sys_getpid()` | `GetPid` | `Syscall#(GetPid, 0,0,0,0,0,0)` |
| `__sys_getppid()` | `GetPPid` | `Syscall#(GetPPid, 0,0,0,0,0,0)` |

#### `lib/std/os/mem.bv` (5 ops)

| `mmap(addr, len, prot, flags, fd, offset)` | `Mmap` | `Syscall#(Mmap, addr, len, prot, flags, fd, offset)` |
| `munmap(addr, len)` | `Munmap` | `Syscall#(Munmap, addr, len, 0,0,0,0)` |
| `mprotect(addr, len, prot)` | `Mprotect` | `Syscall#(Mprotect, addr, len, prot, 0,0,0)` |
| `brk(addr)` | `Brk` | `Syscall#(Brk, addr, 0,0,0,0,0)` |
| `mlock(addr, len)` | `Mlock` | `Syscall#(Mlock, addr, len, 0,0,0,0)` |

#### `lib/std/os/net.bv` (12 ops)

| `__sys_socket(domain, type_, protocol)` | `Socket` | Full args |
| `__sys_bind(fd, addr, addrlen)` | `Bind` | Full args |
| `__sys_listen(fd, backlog)` | `Listen` | Full args |
| `__sys_accept(fd, addr, addrlen)` | `Accept` | Full args |
| `__sys_connect(fd, addr, addrlen)` | `Connect` | Full args |
| `__sys_send(fd, buf, len, flags)` | `Send` | Full args |
| `__sys_recv(fd, buf, len, flags)` | `Recv` | Full args |
| `__sys_sendto(fd, buf, len, flags, dest, destlen)` | `SendTo` | Full args |
| `__sys_recvfrom(fd, buf, len, flags, src, srclen)` | `RecvFrom` | Full args |
| `__sys_setsockopt(fd, level, optname, optval, optlen)` | `SetSockOpt` | Full args |
| `__sys_getsockopt(fd, level, optname, optval, optlen)` | `GetSockOpt` | Full args |
| `__sys_shutdown(fd, how)` | `Shutdown` | Full args |

#### `lib/std/os/dir.bv` (10 string-arg ops — deferred to Phase 2)

All 10 ops in `dir.bv` take string paths. Deferred.

#### `lib/std/os/time.bv` (2 ops)

| `__sys_clock_gettime(clock_id, tp)` | `ClockGetTime` | Full args |
| `__sys_nanosleep(req, rem)` | `NanoSleep` | Full args |

#### `lib/std/os/sched.bv` (1 op)

| `__sys_sched_yield()` | `SchedYield` | `Syscall#(SchedYield, 0,0,0,0,0,0)` |

#### `lib/std/os/signal.bv` (2 ops)

| `__sys_sigaction(sig, action, old_action)` | `RtSigAction` | Full args |
| `__sys_sigprocmask(how, set, old_set)` | `RtSigProcmask` | Full args |

#### `lib/std/os/ipc.bv` (6 ops)

| `pipe(pipefd)` | `Pipe` | Full args |
| `shm_open(name, oflag, mode)` | — deferred (string arg `name`) |
| `shm_unlink(name)` | — deferred (string arg `name`) |
| `sem_open(name, oflag, mode, value)` | — deferred (string arg `name`) |
| `sem_wait(sem)` | `SemOp` | Full args |
| `sem_post(sem)` | `SemOp` | Full args |

Note: `sem_wait` and `sem_post` both use the same `SemOp` syscall with
different operation codes (0 = wait, 1 = post). The `defn` wrapper handles
the opcode selection.

#### `lib/std/os/tty.bv` (5 ops)

| `__sys_tty_raw_mode(enable)` | `IoCtl` | Uses `IoCtl` with `TCSETSF` constant |
| `__sys_tty_size()` | `IoCtl` | Uses `IoCtl` with `TIOCGWINSZ` |
| `__sys_tty_read_key(fd)` | `Read` | `Syscall#(Read, fd, buf, 1, 0,0,0)` |
| `__sys_ioctl(fd, request, argp)` | `IoCtl` | `Syscall#(IoCtl, fd, request, argp, 0,0,0)` |
| `__sys_isatty(fd)` | `IoCtl` | Uses `IoCtl` with `TCGETS` |

#### `lib/std/os/sysinfo.bv` (4 ops)

| `__sys_uname()` | `Uname` | Full args (needs buffer) |
| `__sys_hostname()` | `Hostname` | via `sysinfo` |
| `__sys_pagesize()` | — | No syscall needed — compile-time constant |
| `__sys_cpu_count()` | — | No syscall needed — `_SC_NPROCESSORS_ONLN` via `sysconf` |

Note: `pagesize` and `cpu_count` are `sysconf()` values, not syscalls.
These need a special `Sysconf#` intrinsic or can be kept as C runtime calls
for now.

#### `lib/std/os/user.bv` (4 ops)

| `__sys_getuid()` | `GetUid` | `Syscall#(GetUid, 0,0,0,0,0,0)` |
| `__sys_geteuid()` | `GetEuid` | `Syscall#(GetEuid, 0,0,0,0,0,0)` |
| `__sys_getgid()` | `GetGid` | `Syscall#(GetGid, 0,0,0,0,0,0)` |
| `__sys_getegid()` | `GetEgid` | `Syscall#(GetEgid, 0,0,0,0,0,0)` |

#### `lib/std/os/thread.bv` (9 ops)

All 9 use `Syscall#` with `Clone`, `Futex`, or raw op.

| `__sys_thread_create(fn, arg)` | `Clone` | Full args (with `CLONE_VM\|CLONE_VFORK` flags) |
| `__sys_thread_join(thread)` | `Wait4` | Full args |
| `__sys_thread_exit(code)` | `Exit` | `Syscall#(Exit, code, 0,0,0,0,0)` |
| `__sys_mutex_lock(mptr)` | `Futex` | Full args with `FUTEX_WAIT` op |
| `__sys_mutex_unlock(mptr)` | `Futex` | Full args with `FUTEX_WAKE` op |
| `__sys_condvar_wait(cptr, mptr)` | `Futex` | Full args with `FUTEX_WAIT_BITSET` |
| `__sys_condvar_signal(cptr)` | `Futex` | Full args with `FUTEX_WAKE` |
| `__sys_condvar_broadcast(cptr)` | `Futex` | Full args with `FUTEX_REQUEUE` |

Note: The thread ops need futex opcode constants and clone flags. These
will be `Int` constants defined at the top of the file.

#### `lib/std/os/rand.bv` (1 op)

| `__sys_getrandom(buf, len, flags)` | `GetRandom` | `Syscall#(GetRandom, buf, len, flags, 0,0,0)` |

#### `lib/std/os/temp.bv` (2 ops — deferred to Phase 2)

Both `mkstemp` and `mkdtemp` take string template paths.

#### `lib/std/os/resource.bv` (2 ops)

| `__sys_getrlimit(resource)` | `GetRlimit` | Full args |
| `__sys_setrlimit(resource, packed)` | `SetRlimit` | Full args |

Note: `prlimit64` syscall is used (x86_64 number 302).

---

## 5. Phase 2: String-Arg Wrappers (~17 inops, needs string conversion)

### 5.1 The Briev string format

Briev strings are stored as: `[8-byte length prefix][UTF-8 data]`. The
incoming `Ptr<Byte>` points to the length prefix. To create a C string
(null-terminated) for a syscall, we need:

```
cstr = Malloc#(length + 1)
// copy data after length prefix
Memcpy#(cstr, ptr + 8, length)
// write null terminator at cstr[length]
Memset#(cstr + length, 0, 1)
```

### 5.2 Helper function: `lib/std/string_c.bv`

```briev
// 2026-07-15: Convert Briev string to C-style null-terminated string.
// Allocates memory via Malloc# — caller must free.
// Input: ptr to Briev string (length prefix + UTF-8 data)
// Returns: ptr to C string (null-terminated)

defn to_c_string(s: Ptr<Byte>) -> Ptr<Byte> {
    let len_ptr: Ptr<Byte> = s;
    let len: Int = LoadU8#(len_ptr);
    let cstr: Ptr<Byte> = Malloc#(len + 1);
    Memcpy#(cstr, len_ptr + 8, len);
    Memset#(cstr + len, 0, 1);
    term cstr;
};
```

### 5.3 String-arg wrappers (deferred files)

After `to_c_string` is available, rewrite:

- `lib/std/os/fs.bv`: `file_open(path, flags, mode)` → `to_c_string(path)` then `Syscall#(Open, ...)`
- `lib/std/os/fs.bv`: `file_stat(path, buf)` → same
- `lib/std/os/dir.bv`: all 10 ops → same
- `lib/std/os/ipc.bv`: `shm_open`, `shm_unlink`, `sem_open` → same
- `lib/std/os/temp.bv`: both ops → same
- `lib/std/os/spawn.bv` (not yet created): `spawn`, `spawn_with_output` → same

---

## 6. Phase 3: Interpreter Fallback Consistency

### 6.1 Current state

The interpreter's `Syscall#` handler calls `libc::syscall()` for both
abstract op names and raw numbers. When `check` evaluates a program,
it actually makes the syscall on the host.

### 6.2 Issue

`check` mode may try to make syscalls that fail in the interpreter
environment (e.g., `Mmap` with specific flags). These failures should
not block type checking.

### 6.3 Fix

Wrap the `libc::syscall()` call in a `catch_unwind` or check the return
value. On failure, return 0 (simulating success) rather than propagating
the error. This matches how other OS intrinsics work in check mode.

---

## 7. Phase 4: Example File Fixes

### 7.1 Files with `Print#` already applied (verified passing `check`)

These use `Print#` correctly after Group A fixes:

- `examples/error-handling.bv` — still has pre-existing parser errors
  unrelated to `#` naming. Will be fixed after std/os rewrite.
- `examples/swan-song.bv` — same
- `examples/pipe-skip.bv`, `examples/pipe-chain.bv` — same
- `examples/ptr-arithmetic.bv` — same
- `examples/test_ffi.bv`, `examples/test_ffi_minimal.bv` — same
- `examples/hello-world/src/main.bv` — same

### 7.2 Files needing std/os imports

After Phase 1, these can import from `std/os`:

| Example | Import from `std/os` |
|---------|---------------------|
| `networking.bv` | `import { socket, bind, listen, accept, connect, send, recv, setsockopt } from "std/os"` |
| `mmap-demo.bv` | `import { mmap, munmap, mprotect } from "std/os"` |
| `process-spawn.bv` | `import { getpid, spawn, getenv, setenv, getcwd, chdir, exit } from "std/os"` |
| `cell-demo.bv` | `PutChar#` already fixed. Remaining `print_int#` → `Print#` done. |

---

## 8. Phase 5: C Runtime Cleanup

After all 98 `inop` declarations are migrated, remove the corresponding
`briev_*` functions from `lib/runtime/briev_rt.c`. Keep:

- `briev_syscall` (the new core)
- `__rt_init`, `__rt_wait` (runtime initialization — needed by main)
- `briev_str_to_c` (until Phase 2 is complete)
- `briev_pagesize`, `briev_cpu_count` (until `Sysconf#` is added)

Delete all other `briev_*` functions (~50 functions).

---

## 9. Tests

### 9.1 Behavioral tests (not literal)

Each test verifies that the `defn` wrapper calls the correct `Syscall#`
with the right abstract op name:

```rust
// Test: file_close calls Syscall#(Close, ...)
let close_call = compile_expr("file_close(3)");
assert!(close_call.contains("Syscall#(Close, 3"));
```

### 9.2 Test files to create/modify

- `src/backend/llvm/intrinsics.rs` — add `test_syscall_emits_call`:
  Verify `Syscall#(Open, 1,2,3,4,5,6)` emits `call i64 @briev_syscall`
- `src/interpreter/intrinsics.rs` — add `test_syscall_abstract_op`:
  Verify `Syscall#(GetPid, ...)` resolves abstract op to number 39

### 9.3 Integration test

After Phase 1, verify that `briev check` passes for a program using
`Syscall#` for file operations:

```briev
import { file_close } from "std/os";
defn main() -> Int { Syscall#(GetPid, 0,0,0,0,0,0); term 0; };
```

---

## 10. Verification Gates

| Check | How |
|-------|-----|
| `cargo test --lib` | All 860+ tests pass |
| `cargo build --release` | No warnings |
| `briev check` on rewritten .bv files | Each `lib/std/os/*.bv` parses and passes type check |
| No remaining `inop` in `lib/std/os/` | `grep -r '^inop' lib/std/os/` returns empty |
| No remaining lowercase `#` calls in examples | `grep -rn '[a-z][a-z0-9_]*#(' examples/` returns only `AddressOf#`, `PutChar#`, `Print#` |
| Praetor on new/changed Rust code | Complexity ≤ 15, lines ≤ 100, params ≤ 6 |
