# Eliminate `frgn` From officina — Pure Brief, Direct libc Intrinsics

**Datetime**: 2026-06-17T10:30:00-06:00
**Status**: In progress (build mode)

## Guiding Principle

Brief is a systems language. Its computational primitive (the reactive
transaction) compiles to native LLVM IR. There is no reason to insert
`frgn` as an intermediary between Brief and the OS — libc and LLVM
are the only bridges needed.

Every `frgn` in the stdlib or in officina is either:
1. A pure Brief function that was never written (string ops, encoding, JSON)
2. An intrinsic that emits `call @brief_*` instead of directly calling libc
3. A Rust-interpreter-only function with no LLVM backend path

All three categories are eliminable.

---

## Phase 1: Pure-Brief JSON Parser (`lib/std/json.bv`)

### Problem

Officina declares 5 `frgn` JSON functions backed by the interpreter's Rust FFI.
The LLVM backend emits `call @json_parse` which has no linkable definition.
Additionally, `json_get` is a **stub** that ignores its key argument — it's
broken even in the interpreter.

### Solution

Write a pure-Brief JSON parser that replaces ALL `frgn __*` declarations in
`lib/std/json.bv`. The API surface officina needs is exactly 5 functions:

| Function | Signature | Implementation |
|----------|-----------|----------------|
| `json_parse(s: String) -> Result<Value, JsonError>` | String → JSON tree | Character-by-character recursive descent parser in pure Brief |
| `json_is_array(v: Value) -> Result<Bool, JsonError>` | Check if value is array | Match on `Value::Array` |
| `json_length(v: Value) -> Result<Int, JsonError>` | Get array length | Match `Value::Array`, return `list :> Size` |
| `json_get(v: Value, key: String) -> Result<String, JsonError>` | Get string field by key | Match `Value::Object(map)`, lookup in HashMap |
| `json_get_by_index(v: Value, i: Int) -> Result<Value, JsonError>` | Index into array | Match `Value::Array`, index into list |

### Value type (define in `lib/std/json.bv`)

```brief
enum JsonValue {
    Null,
    Bool(Bool),
    Number(Int),
    Float(Float),
    String(String),
    Array(List<JsonValue>),
    Object(HashMap<String, JsonValue>)
};
```

This replaces the opaque foreign `Data` type.

### Compatibility break

The existing `frgn __*` functions (__parse, __stringify, __is_null, etc.)
will be removed from `lib/std/json.bv`. The public API uses `JsonValue` and
clean function names. Old code using `import json from "std/ffi/json"` with
`__parse` etc. will need migration.

### DBrief config files

Officina's `system/understands.dbv` is NOT JSON — it's DBrief format.
Convert to JSON.

---

## Phase 2: Migrate All `brief_*` Intrinsics to Direct libc

### Current state

74 intrinsics in `emit_expr.rs` emit `call @brief_*(...)` instead of calling
libc directly. 67 have direct libc equivalents. 7 are Linux-specific but
still callable via libc or `syscall()`.

### Migration table

For each intrinsic in `emit_expr.rs`, change the `writeln!(out, "call @brief_*...")`
to `writeln!(out, "call @libc_name...")` and update the `declare` in
`emit_toplevel.rs`.

#### Group A: File I/O (13 intrinsics) — trivial rename

| Intrinsic | Current `@brief_*` | Target libc | Header |
|-----------|-------------------|-------------|--------|
| `open#` | `brief_open` | `open` | `fcntl.h` |
| `close#` | `brief_close` | `close` | `unistd.h` |
| `read#` | `brief_read` | `read` | `unistd.h` |
| `write#` | `brief_write` | `write` | `unistd.h` |
| `lseek#` | `brief_lseek` | `lseek` | `unistd.h` |
| `pread#` | `brief_pread` | `pread` | `unistd.h` |
| `pwrite#` | `brief_pwrite` | `pwrite` | `unistd.h` |
| `stat#` | `brief_stat` | `stat` | `sys/stat.h` |
| `fstat#` | `brief_fstat` | `fstat` | `sys/stat.h` |
| `truncate#` | `brief_truncate` | `truncate` | `unistd.h` |
| `ftruncate#` | `brief_ftruncate` | `ftruncate` | `unistd.h` |
| `fsync#` | `brief_fsync` | `fsync` | `unistd.h` |
| `dup#` / `dup2#` | `brief_dup` / `brief_dup2` | `dup` / `dup2` | `unistd.h` |

#### Group B: Filesystem (15 intrinsics)

| `mkdir#` | `brief_mkdir` | `mkdir` | `sys/stat.h` |
| `rmdir#` | `brief_rmdir` | `rmdir` | `unistd.h` |
| `unlink#` | `brief_unlink` | `unlink` | `unistd.h` |
| `rename#` | `brief_rename` | `rename` | `stdio.h` |
| `symlink#` | `brief_symlink` | `symlink` | `unistd.h` |
| `readlink#` | `brief_readlink` | `readlink` | `unistd.h` |
| `link#` | `brief_link` | `link` | `unistd.h` |
| `getcwd#` | `brief_getcwd` | `getcwd` | `unistd.h` |
| `chdir#` | `brief_chdir` | `chdir` | `unistd.h` |
| `readdir#` | `brief_readdir` | `opendir`+`readdir` | `dirent.h` |
| `chmod#` | `brief_chmod` | `chmod` | `sys/stat.h` |
| `chown#` | `brief_chown` | `chown` | `unistd.h` |
| `umask#` | `brief_umask` | `umask` | `sys/stat.h` |
| `access#` | `brief_access` | `access` | `unistd.h` |
| `fcntl#` | `brief_fcntl` | `fcntl` | `fcntl.h` |

#### Group C: Memory (5 intrinsics)

| `mmap#` | `brief_mmap` | `mmap` | `sys/mman.h` |
| `munmap#` | `brief_munmap` | `munmap` | `sys/mman.h` |
| `mprotect#` | `brief_mprotect` | `mprotect` | `sys/mman.h` |
| `brk#` | `brief_brk` | `brk` | `unistd.h` |
| `mlock#` | `brief_mlock` | `mlock` | `sys/mman.h` |

#### Group D: Process + Environment (8 intrinsics)

| `spawn_with_output#` | `brief_spawn_with_output` | `popen` | `stdio.h` |
| `spawn#` | `brief_spawn` | `system` | `stdlib.h` |
| `getenv#` | `brief_getenv` | `getenv` | `stdlib.h` |
| `setenv#` | `brief_setenv` | `setenv` | `stdlib.h` |
| `unsetenv#` | `brief_unsetenv` | `unsetenv` | `stdlib.h` |
| `getpid#` | `brief_getpid` | `getpid` | `unistd.h` |
| `getppid#` | `brief_getppid` | `getppid` | `unistd.h` |
| `nanosleep#` | `brief_nanosleep` | `nanosleep` | `time.h` |

#### Group E: Terminal (5 intrinsics)

| `tty_raw_mode#` | `brief_tty_raw_mode` | `tcsetattr` | Complex: needs tcgetattr+cfmakeraw+tcsetattr |
| `tty_size#` | `brief_tty_size` | `ioctl(TIOCGWINSZ)` | Single ioctl call |
| `tty_read_key#` | `brief_tty_read_key` | `read(STDIN_FILENO)` | Single read call |
| `ioctl#` | `brief_ioctl` | `ioctl` | `sys/ioctl.h` |
| `isatty#` | `brief_isatty` | `isatty` | `unistd.h` |

#### Group F: Networking (13 intrinsics)

`socket#`, `bind#`, `listen#`, `accept#`, `connect#`, `send#`, `recv#`,
`sendto#`, `recvfrom#`, `setsockopt#`, `getsockopt#`, `shutdown#`,
`getaddrinfo#` → all map 1:1 to libc functions with same names.

#### Group G: Signals + IPC (10 intrinsics)

| `sigaction#` | `brief_sigaction` | `sigaction` | `signal.h` |
| `sigprocmask#` | `brief_sigprocmask` | `sigprocmask` | `signal.h` |
| `kill#` | `brief_kill` | `kill` | `signal.h` |
| `pipe#` | `brief_pipe` | `pipe` | `unistd.h` |
| `shm_open#` | `brief_shm_open` | `shm_open` | `sys/mman.h` |
| `shm_unlink#` | `brief_shm_unlink` | `shm_unlink` | `sys/mman.h` |
| `sem_open#` | `brief_sem_open` | `sem_open` | `semaphore.h` |
| `sem_wait#` | `brief_sem_wait` | `sem_wait` | `semaphore.h` |
| `sem_post#` | `brief_sem_post` | `sem_post` | `semaphore.h` |
| `clock_gettime#` | `brief_clock_gettime` | `clock_gettime` | `time.h` |

#### Group H: Special cases (5 intrinsics) — Linux-specific

| `read_file#` | `brief_read_file` | `fopen`+`fread`+`fclose` | Needs sequence of calls |
| `futex#` | `brief_futex` | `syscall(SYS_futex,...)` | Linux-only via `sys/syscall.h` |
| `signalfd#` | `brief_signalfd` | `signalfd` | Linux-only via `sys/signalfd.h` |
| `timerfd_create#` | `brief_timerfd_create` | `timerfd_create` | Linux-only via `sys/timerfd.h` |

#### Group I: Thread pool barriers (3 intrinsics) — keep as C bridge

`brief_barrier_release`, `brief_barrier_wait`, `brief_thread_pool_init`:
Async runtime infrastructure. Keep in a small C file.

---

## Phase 3: Replace `frgn` in Stdlib With Pure Brief

### Step 3a: String helpers (24 `frgn __*`)

Implementable in pure Brief using string slicing, `s :> Size`, and character
comparison. `__to_lower`, `__is_alpha`, `__replace_all`, `__splitn`, etc.

### Step 3b: Encoding helpers (27 `frgn __*`)

Base64, hex, URL, HTML, UTF-8, SHA2, UUID — all character/bit manipulation.
Implementable in pure Brief.

### Step 3c: Time helpers (24 `frgn __*`)

`__now` → `clock_gettime` intrinsic. `__year`/`__month`/`__day` → pure Brief
Gregorian calendar algorithm. `__seconds_per_*` → consts.

### Step 3d: JSON (28 `frgn __*`)

Replaced entirely by Phase 1's pure-Brief parser. Delete opaque `Data` type.

### Step 3e: Metro/SHM/Atomic (26 `frgn __*`)

Convert 1:1 wrappers to direct libc or LLVM atomic instructions.

### Step 3f: I/O + Process (8 `frgn __*`)

Already have intrinsics (`read_file#`, `write_file#`, `spawn#`, `unlink#`,
`mkdir#`, etc.). Remove `frgn __*` wrappers.

### Step 3g: HTTP (2 `frgn __*`)

Pure-Brief HTTP client using socket intrinsics (socket#, connect#, send#, recv#).

---

## Phase 4: Rework Officina

### Changes to `officina/persistence.bv`

1. Remove 5 `frgn json_*` declarations
2. Add `import json from "std/json"`
3. Update `parse_rules_from_json` to use `JsonValue` type
4. Fix `json_get` Result handling

### Changes to `system/understands.dbv`

Convert from DBrief to JSON format.

### Changes to `serialize_rules`

Produce JSON instead of DBrief format.

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
| **1** | Pure-Brief JSON parser | ~3 hours | Nothing |
| **3a** | 24 string helpers → pure Brief | ~2 hours | Nothing |
| **3b** | 27 encoding → pure Brief | ~3 hours | Nothing |
| **3c** | 24 time → pure Brief + intrinsics | ~2 hours | 2a |
| **3d** | Delete old json.bv (replaced by Phase 1) | ~5 min | 1 |
| **3e** | Metro/SHM/Atomic cleanup | ~1 hour | 2a |
| **3f** | I/O + Process cleanup | ~30 min | 2a |
| **3g** | HTTP: pure-Brief client | ~1 hour | 2a |
| **4** | Rework officina | ~1 hour | 1, 2a |
| **5** | Documentation | ~1 hour | All |
