# Eliminate `frgn` From officina — Pure Briv, Direct libc Intrinsics

**Datetime**: 2026-06-17T10:30:00-06:00
**Status**: In progress (build mode)

## Guiding Principle

Briv is a systems language. Its computational primitive (the reactive
transaction) compiles to native LLVM IR. There is no reason to insert
`frgn` as an intermediary between Briv and the OS — libc and LLVM
are the only bridges needed.

Every `frgn` in the stdlib or in officina is either:
1. A pure Briv function that was never written (string ops, encoding, JSON)
2. An intrinsic that emits `call @briv_*` instead of directly calling libc
3. A Rust-interpreter-only function with no LLVM backend path

All three categories are eliminable.

---

## Phase 1: Pure-Briv JSON Parser (`lib/std/json.bv`) — ✅ DONE

Wrote a full recursive-descent JSON parser in pure Briv.
Replaces 28 `frgn __*` declarations backed by interpreter-only Rust FFI.
The parser handles all JSON grammar (objects, arrays, strings with escapes,
numbers as Int/Float, keywords). Error types use `String` because the
type checker can't infer enum variant types as the parent enum type.

Built-in `Result<T,E>` and `Option<T>` (injected by the compiler) enable
clean error handling. LLVM backend has two pre-existing bugs that prevent
binary emission for code using JsonValue:
1. Char `zext i32 %i64_reg to i64` — fixed Int→Char cast but string indexing
   still produces mismatched types
2. Float parameter marshaling — defn functions receive `float` but internals
   convert through i32→i64, causing type mismatch

## Phase 2: Migrate All `briv_*` Intrinsics to Direct libc — ✅ DONE

**75/86** intrinsics now emit direct libc calls. **11** remain as C shims
in `briv_rt.c` that are auto-linked for all native builds.

## Phase 3: Replace `frgn` in Stdlib With Pure Briv — 🟡 In Progress

### Step 3d: JSON (28 `frgn __*`) — ✅ DONE
Replaced entirely by Phase 1's pure-Briv parser.

## Phase 4: Rework Officina — 🟡 In Progress

### Changes to officina/persistence.bv — ✅ DONE
1. Removed 5 `frgn json_*` declarations
2. Added `import# "std/json"`
3. Updated `parse_rules_from_json` to use `JsonValue` type
4. Updated `parse_json_rules` to handle `Result<JsonValue, String>` properly

### Changes to officina/core.bv — ✅ DONE
1. Removed duplicate `Result<T,E>` enum definition (now injected by compiler)
2. All `Result` usage resolves to the built-in type

### Remaining
- `system/understands.dbv` — DBriv format, not yet converted to JSON
- `serialize_rules` still produces DBriv format
- LLVM backend Char/Float bugs block binary emission for code using JSON parsing

---

## Phase 2: Migrate All `briv_*` Intrinsics to Direct libc

### Current state

74 intrinsics in `emit_expr.rs` emit `call @briv_*(...)` instead of calling
libc directly. 67 have direct libc equivalents. 7 are Linux-specific but
still callable via libc or `syscall()`.

### Migration table

For each intrinsic in `emit_expr.rs`, change the `writeln!(out, "call @briv_*...")`
to `writeln!(out, "call @libc_name...")` and update the `declare` in
`emit_toplevel.rs`.

#### Group A: File I/O (13 intrinsics) — trivial rename

| Intrinsic | Current `@briv_*` | Target libc | Header |
|-----------|-------------------|-------------|--------|
| `open#` | `briv_open` | `open` | `fcntl.h` |
| `close#` | `briv_close` | `close` | `unistd.h` |
| `read#` | `briv_read` | `read` | `unistd.h` |
| `write#` | `briv_write` | `write` | `unistd.h` |
| `lseek#` | `briv_lseek` | `lseek` | `unistd.h` |
| `pread#` | `briv_pread` | `pread` | `unistd.h` |
| `pwrite#` | `briv_pwrite` | `pwrite` | `unistd.h` |
| `stat#` | `briv_stat` | `stat` | `sys/stat.h` |
| `fstat#` | `briv_fstat` | `fstat` | `sys/stat.h` |
| `truncate#` | `briv_truncate` | `truncate` | `unistd.h` |
| `ftruncate#` | `briv_ftruncate` | `ftruncate` | `unistd.h` |
| `fsync#` | `briv_fsync` | `fsync` | `unistd.h` |
| `dup#` / `dup2#` | `briv_dup` / `briv_dup2` | `dup` / `dup2` | `unistd.h` |

#### Group B: Filesystem (15 intrinsics)

| `mkdir#` | `briv_mkdir` | `mkdir` | `sys/stat.h` |
| `rmdir#` | `briv_rmdir` | `rmdir` | `unistd.h` |
| `unlink#` | `briv_unlink` | `unlink` | `unistd.h` |
| `rename#` | `briv_rename` | `rename` | `stdio.h` |
| `symlink#` | `briv_symlink` | `symlink` | `unistd.h` |
| `readlink#` | `briv_readlink` | `readlink` | `unistd.h` |
| `link#` | `briv_link` | `link` | `unistd.h` |
| `getcwd#` | `briv_getcwd` | `getcwd` | `unistd.h` |
| `chdir#` | `briv_chdir` | `chdir` | `unistd.h` |
| `readdir#` | `briv_readdir` | `opendir`+`readdir` | `dirent.h` |
| `chmod#` | `briv_chmod` | `chmod` | `sys/stat.h` |
| `chown#` | `briv_chown` | `chown` | `unistd.h` |
| `umask#` | `briv_umask` | `umask` | `sys/stat.h` |
| `access#` | `briv_access` | `access` | `unistd.h` |
| `fcntl#` | `briv_fcntl` | `fcntl` | `fcntl.h` |

#### Group C: Memory (5 intrinsics)

| `mmap#` | `briv_mmap` | `mmap` | `sys/mman.h` |
| `munmap#` | `briv_munmap` | `munmap` | `sys/mman.h` |
| `mprotect#` | `briv_mprotect` | `mprotect` | `sys/mman.h` |
| `brk#` | `briv_brk` | `brk` | `unistd.h` |
| `mlock#` | `briv_mlock` | `mlock` | `sys/mman.h` |

#### Group D: Process + Environment (8 intrinsics)

| `spawn_with_output#` | `briv_spawn_with_output` | `popen` | `stdio.h` |
| `spawn#` | `briv_spawn` | `system` | `stdlib.h` |
| `getenv#` | `briv_getenv` | `getenv` | `stdlib.h` |
| `setenv#` | `briv_setenv` | `setenv` | `stdlib.h` |
| `unsetenv#` | `briv_unsetenv` | `unsetenv` | `stdlib.h` |
| `getpid#` | `briv_getpid` | `getpid` | `unistd.h` |
| `getppid#` | `briv_getppid` | `getppid` | `unistd.h` |
| `nanosleep#` | `briv_nanosleep` | `nanosleep` | `time.h` |

#### Group E: Terminal (5 intrinsics)

| `tty_raw_mode#` | `briv_tty_raw_mode` | `tcsetattr` | Complex: needs tcgetattr+cfmakeraw+tcsetattr |
| `tty_size#` | `briv_tty_size` | `ioctl(TIOCGWINSZ)` | Single ioctl call |
| `tty_read_key#` | `briv_tty_read_key` | `read(STDIN_FILENO)` | Single read call |
| `ioctl#` | `briv_ioctl` | `ioctl` | `sys/ioctl.h` |
| `isatty#` | `briv_isatty` | `isatty` | `unistd.h` |

#### Group F: Networking (13 intrinsics)

`socket#`, `bind#`, `listen#`, `accept#`, `connect#`, `send#`, `recv#`,
`sendto#`, `recvfrom#`, `setsockopt#`, `getsockopt#`, `shutdown#`,
`getaddrinfo#` → all map 1:1 to libc functions with same names.

#### Group G: Signals + IPC (10 intrinsics)

| `sigaction#` | `briv_sigaction` | `sigaction` | `signal.h` |
| `sigprocmask#` | `briv_sigprocmask` | `sigprocmask` | `signal.h` |
| `kill#` | `briv_kill` | `kill` | `signal.h` |
| `pipe#` | `briv_pipe` | `pipe` | `unistd.h` |
| `shm_open#` | `briv_shm_open` | `shm_open` | `sys/mman.h` |
| `shm_unlink#` | `briv_shm_unlink` | `shm_unlink` | `sys/mman.h` |
| `sem_open#` | `briv_sem_open` | `sem_open` | `semaphore.h` |
| `sem_wait#` | `briv_sem_wait` | `sem_wait` | `semaphore.h` |
| `sem_post#` | `briv_sem_post` | `sem_post` | `semaphore.h` |
| `clock_gettime#` | `briv_clock_gettime` | `clock_gettime` | `time.h` |

#### Group H: Special cases (5 intrinsics) — Linux-specific

| `read_file#` | `briv_read_file` | `fopen`+`fread`+`fclose` | Needs sequence of calls |
| `futex#` | `briv_futex` | `syscall(SYS_futex,...)` | Linux-only via `sys/syscall.h` |
| `signalfd#` | `briv_signalfd` | `signalfd` | Linux-only via `sys/signalfd.h` |
| `timerfd_create#` | `briv_timerfd_create` | `timerfd_create` | Linux-only via `sys/timerfd.h` |

#### Group I: Thread pool barriers (3 intrinsics) — keep as C bridge

`briv_barrier_release`, `briv_barrier_wait`, `briv_thread_pool_init`:
Async runtime infrastructure. Keep in a small C file.

---

## Phase 3: Replace `frgn` in Stdlib With Pure Briv

### Step 3a: String helpers (24 `frgn __*`)

Implementable in pure Briv using string slicing, `s .#Size`, and character
comparison. `__to_lower`, `__is_alpha`, `__replace_all`, `__splitn`, etc.

### Step 3b: Encoding helpers (27 `frgn __*`)

Base64, hex, URL, HTML, UTF-8, SHA2, UUID — all character/bit manipulation.
Implementable in pure Briv.

### Step 3c: Time helpers (24 `frgn __*`)

`__now` → `clock_gettime` intrinsic. `__year`/`__month`/`__day` → pure Briv
Gregorian calendar algorithm. `__seconds_per_*` → consts.

### Step 3d: JSON (28 `frgn __*`)

Replaced entirely by Phase 1's pure-Briv parser. Delete opaque `Data` type.

### Step 3e: Metro/SHM/Atomic (26 `frgn __*`)

Convert 1:1 wrappers to direct libc or LLVM atomic instructions.

### Step 3f: I/O + Process (8 `frgn __*`)

Already have intrinsics (`read_file#`, `write_file#`, `spawn#`, `unlink#`,
`mkdir#`, etc.). Remove `frgn __*` wrappers.

### Step 3g: HTTP (2 `frgn __*`)

Pure-Briv HTTP client using socket intrinsics (socket#, connect#, send#, recv#).

---

## Phase 4: Rework Officina

### Changes to `officina/persistence.bv`

1. Remove 5 `frgn json_*` declarations
2. Add `import json from "std/json"`
3. Update `parse_rules_from_json` to use `JsonValue` type
4. Fix `json_get` Result handling

### Changes to `system/understands.dbv`

Convert from DBriv to JSON format.

### Changes to `serialize_rules`

Produce JSON instead of DBriv format.

---

## Phase 5: Documentation

Update `docs/architecture/` — intrinsics migration, backend strategy,
glossary. Deprecate `frgn` in stdlib. Remove `lib/std/ffi/` mirrors.

---

## Work Order

| Phase | Items | Effort | Depends on |
|-------|-------|--------|------------|
| **2a** | 67 trivial intrinsics (Groups A–G) | ~2 hours | Nothing |
| **2b** | 5 Linux-specific + special (Groups H–I) | ~30 min | 2a |
| **1** | Pure-Briv JSON parser | ~3 hours | Nothing |
| **3a** | 24 string helpers → pure Briv | ~2 hours | Nothing |
| **3b** | 27 encoding → pure Briv | ~3 hours | Nothing |
| **3c** | 24 time → pure Briv + intrinsics | ~2 hours | 2a |
| **3d** | Delete old json.bv (replaced by Phase 1) | ~5 min | 1 |
| **3e** | Metro/SHM/Atomic cleanup | ~1 hour | 2a |
| **3f** | I/O + Process cleanup | ~30 min | 2a |
| **3g** | HTTP: pure-Briv client | ~1 hour | 2a |
| **4** | Rework officina | ~1 hour | 1, 2a |
| **5** | Documentation | ~1 hour | All |
