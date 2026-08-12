# Briev FFI Architecture — Zero-Cost Multi-Language Interop

## Core Insight

`frgn` is just a `call` instruction. Nothing more. No marshaling, no context switch, no runtime boundary. The exact same LLVM `call` that Briev uses for its own `defn` functions is used for foreign functions.

## How It Works

```
Briev source:    frgn __print_int(n: Int) -> Result<Bool, Error>;
                         ↓
Parser:          stores name="__print_int", params=[(n, Int)], return=Result<Bool,Error>
                         ↓
LLVM codegen:    declare i64 @__print_int(i64)     ← just a symbol declaration
                         ↓
LLVM call site:  %result = call i64 @__print_int(i64 %n)   ← same as any function call
                         ↓
LTO link:        llvm-link program.bc briev_rt.bc          ← merges IR modules
                         ↓
Inlining:        opt -O3                                    ← inlines across language boundaries
```

## `import "link/..."` — The Bridge

`import "link/briev_rt.c"` tells the compiler:

| Step | Command | What happens |
|------|---------|-------------|
| 1 | `clang -c -emit-llvm -O2 briev_rt.c` | Compile C to LLVM bitcode |
| 2 | `llvm-as program.ll` | Compile Briev to LLVM bitcode |
| 3 | `llvm-link program.bc briev_rt.bc` | Merge both into one module |
| 4 | `opt -O3 program_merged.bc` | Inline across language boundary |
| 5 | `llc -filetype=obj -O3` | Generate native code |

The file extension tells the compiler which toolchain to use:

| Extension | Language | Compiler | Convention |
|-----------|----------|----------|------------|
| `.c` | C | `clang -emit-llvm` | C ABI |
| `.cpp` | C++ | `clang++ -emit-llvm` | C++ ABI |
| `.rs` | Rust | `rustc --emit=llvm-ir` | Rust ABI |
| `.zig` | Zig | `zig build-obj --emit-llvm-ir` | C ABI |
| `.bc` | LLVM IR (any) | (already bitcode) | Inferred |

## `from "lang"` — Disambiguation Only

`from` is **not required** on `frgn` declarations. The compiler resolves symbols by scanning all `import "link/..."` targets:

```
frgn __print_int(n: Int) -> Result<Bool, Error>;  
  // ^ found in briev_rt.c → uses C convention
```

Only needed when two link targets export the same symbol:

```
import "link/posix.c";
import "link/windows.c";
frgn write(fd: Int, buf: Data, len: Int) -> Result<Int, Error> from "posix";
frgn write(fd: Int, buf: Data, len: Int) -> Result<Int, Error> from "windows";
```

## Zero-Cost Inlining

Before `opt -O3`:
```llvm
define i64 @main() { call i64 @__print_int(i64 42) }
define i64 @__print_int(i64 %n) { call i64 @fprintf(stderr, "%lld\n", %n) }
```

After `opt -O3`:
```llvm
define i64 @main() { call i64 @fprintf(stderr, "%lld\n", i64 42) }
```

The call to `@__print_int` is inlined. The C function body is pasted directly into `main`. **Zero overhead.**

## Error Handling

| Syntax | Semantics |
|--------|-----------|
| `frgn foo() -> Result<T, Error>` | Returns `Result<T, Error>` — caller MUST handle both Ok and Err |
| `frgn! foo()` | Fire-and-forget — return discarded, error panics |

No `-> Void` syntax. `frgn!` IS the void case.

## Languages Without LLVM Backends

Python, JavaScript, Java, etc. cannot produce LLVM IR. They are **interpreter-only** — compiled backends emit an error: *"`from "python"` has no LLVM backend — can only be called via interpreter."*

## Prelude Auto-Import (std/os/)

The compiler auto-imports 20 modules from `lib/std/os/` as a prelude, replacing 127 former compiler intrinsics. These modules declare `inop` functions that call `briev_rt.c` wrappers:

```briev
// No explicit import needed — these are auto-loaded:
let fd = open#("/path/file", 0, 0);       // from std/os/fs.bv
let pid = getpid#();                        // from std/os/process.bv
let page = mmap#(0, 4096, 3, 0x22, -1, 0); // from std/os/mem.bv
```

Available modules:

| Module | Contents |
|--------|----------|
| `std/os/fs.bv` | open, close, read, write, lseek, pread, pwrite, stat, ftruncate, fsync, dup, fcntl |
| `std/os/net.bv` | socket, bind, listen, accept, connect, send, recv, setsockopt, getaddrinfo |
| `std/os/dir.bv` | mkdir, rmdir, unlink, rename, symlink, readlink, getcwd, readdir, chmod, chown, access |
| `std/os/thread.bv` | thread_create, thread_join, mutex_lock, mutex_unlock, condvar_wait, condvar_signal |
| `std/os/atomic.bv` | atomic_load, atomic_store, atomic_cas, atomic_xchg, atomic_add, fence |
| `std/os/mem.bv` | mmap, munmap, mprotect, brk, mlock |
| `std/os/process.bv` | spawn, getpid, getppid, exit, abort, sleep |
| `std/os/time.bv` | clock_gettime, nanosleep, time |
| `std/os/signal.bv` | sigaction, sigprocmask, kill, signal_fd, timerfd_create |
| `std/os/ipc.bv` | pipe, shm_open, shm_unlink, sem_open, sem_wait, sem_post |
| `std/os/io.bv` | print, println, readln, get_env, set_env |
| `std/os/tty.bv` | tty_raw_mode, tty_size, tty_read_key, ioctl, isatty |
| `std/os/user.bv` | getuid, geteuid, getgid, getegid |
| `std/os/sched.bv` | sched_yield, getpriority, setpriority |
| `std/os/resource.bv` | getrlimit, setrlimit |
| `std/os/sysinfo.bv` | uname, hostname, realpath, pagesize, cpu_count |
| `std/os/dynlib.bv` | dlopen, dlsym, dlclose |
| `std/os/debug.bv` | backtrace, halt, abort |
| `std/os/temp.bv` | mkstemp, mkdtemp |
| `std/os/ring.bv` | ring_push, ring_pop |
| `std/os/rand.bv` | getrandom, errno |

Use `--no-std` to disable prelude auto-import.

## ABI Bridge (briev_rt.c)

Briev's native integer type is `i64` for all scalar values (Int, pointers, etc.).
libc functions often take/return `i32` or `uid_t` (different widths). The
`briev_rt.c` runtime provides wrapper functions that bridge between the two:

```c
// libc: uid_t getuid(void);  (returns i32 on most platforms)
// briev_rt.c wrapper:
int64_t briev_getuid(void) { return (int64_t)getuid(); }
```

In the generated LLVM IR, the wrapper is declared then called from the inop:

```
declare i64 @briev_getuid()     // preamble
define internal i64 @__sys_getuid(i64 %s) {
  %r = call i64 @briev_getuid();
  ret i64 %r;
}
```

The `internal` linkage on the inop function prevents symbol conflicts with
libc — `define internal i64 @read(...)` coexists with `declare i64 @read(...)`
from the C library preamble.

53 wrapper functions currently exist in `lib/runtime/briev_rt.c`, covering
all prelude module requirements.

## No Magic

| Bad (old) | Good (new) |
|-----------|------------|
| `from "libruntime"` (parsed and discarded) | `import "link/briev_rt.c"` + optional `frgn name()` |
| Hardcoded `emit_declares("__rt_init")` | `frgn __rt_init()` declared in `std/rt.bv`, imported explicitly |
| Interpreter match on `"insert"` string | Type-based dispatch on `Value::HashMap` — same native code |
| `"None"`/`"Err"` => discriminant 0 | Enum declaration drives discriminant |
| 127 compiler intrinsics (Socket, Open, MkDir, ...) | 20 `std/os/*.bv` prelude modules with `inop` declarations |
| Hardcoded `Intrinsic::from_name()` dispatches | Universe-resolved `inop` + `frgn` calls through `briev_rt.c` |
| Type dispatch on `Type::Int8`, `Type::Float64`, etc. | TypeUniverse query: `universe.get(name).ops["add"]` |

The FFI is transparent. Every function name you see is the actual symbol the linker resolves. No hidden name mapping, no string matching, no magic destinations.
