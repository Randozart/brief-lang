# Phase 6: Rewrite Stdlib I/O Using Syscalls

**Date:** 2026-06-25
**Status:** Planned
**Previous:** `docs/plans/2026-06-25-native-briv-io.md` (Phases 1–2, Ext A done)
**Dependencies:** Phase 4 (`#!cfg`), Phase 3 (`lib/std/syscall.bv`)

---

## Goal

Replace C runtime calls for I/O intrinsics with direct syscall-based `inop!`
declarations guarded by `#!cfg(target_os == "linux")`, then shrink `briv_rt.c`
by removing dead shim code.

## Key Finding

The LLVM backend already emits **direct libc calls** for most intrinsics
(fprintf, open, read, write, exit, nanosleep, etc.). These do NOT go through
`briv_rt.c` shims. The remaining intrinsics that use C shims are:

| Intrinsic | Current C shim | Replace with |
|---|---|---|
| `ReadFile` | `__read_file__` | Direct file I/O (`open`+`read`+`close`) |
| `WriteFile` | `__write_file__` | Direct file I/O (`open`+`write`+`close`) |
| `TtyReadKey` | `__tty_read_key__` | Already direct `read(STDIN, buf, 1)` |
| `TtyRawMode` | `__tty_raw_mode__` | tcgetattr/cfmakeraw/tcsetattr |
| `SpawnWithOutput` | `__spawn_with_output__` | `fork`+`exec`+`pipe` (popen equivalent) |
| `Spawn` | `__spawn__` (via `system()`) | `fork`+`exec`+`waitpid` |
| String ops | `__readln__` | Direct `getline`/`fgets` |
| ReadDir | `__readdir__` | `opendir`+`readdir` |
| ReadLink | `__readlink__` | Direct `readlink` |
| GetCwd | `__getcwd__` | Direct `getcwd` |
| GetAddrInfo | `__getaddrinfo__` | Direct `getaddrinfo` |
| GetEnv | `__get_env_int__` | Direct `getenv` |
| SortList | `__sort_list__` | `qsort` |
| ReverseList | `__reverse_list__` | Direct `qsort` |

However, all of these can be done via **direct libc calls** in the LLVM codegen
(which is what most intrinsics already do). The C shims are fully redundant.

## Implementation

### Step 1: Convert C shim intrinsics to direct libc calls (30 min)

For each intrinsic that currently calls a C shim, change the LLVM codegen in
`emit_expr.rs` to emit a direct libc call instead. Examples:

| Current | Change to |
|---|---|
| `call i64 @__read_file__(i64)` | `call ptr @fopen(ptr, ptr)` + `call i64 @fread(ptr, i64, i64, ptr)` |
| `call i64 @__spawn_with_output__(i64)` | `fork` + `exec` + `pipe` (can simplify to `popen`) |
| `call i64 @__tty_raw_mode__(i64)` | `call i32 @tcgetattr(i32, ptr)` + `call i32 @tcsetattr(i32, i32, ptr)` |

**Files modified:** `src/backend/llvm/emit_expr.rs`
**Tests:** Existing LLVM codegen tests should pass; no behavior change.

### Step 2: Add `#!cfg`-guarded inop! syscall wrappers for benchmarks (30 min)

The benchmark intrinsics (`PrintInt`, `PutChar`, `PrintFloat`) already use
direct libc. Add `#!cfg(target_os == "linux")` alternatives using syscalls:

```briv
// lib/std/bench/print.bv
#!cfg(target_os == "linux") {
    inop! print_int(n: Int) -> Bool [true][true] {
        %buf = alloca i8, i64 32;
        %len = call i64 @snprintf(ptr %buf, i64 32, ptr @FMT_INT, i64 %n);
        // syscall SYS_write = 1 on x86_64
        %res = syscall3(1, 1, %buf as Int, %len);
        term %res == %len;
    } fallback false;
};
```

These are guarded by `#!cfg` — on non-Linux targets, the existing C-calling
versions remain. The `flatten_cfg` pass removes inactive arms before codegen.

**Files created:** `lib/std/bench/` directory
**Tests:** Benchmark harness uses `--correctness` to verify output matches C.

### Step 3: Remove dead C shims from briv_rt.c (30 min)

Remove the following sections from `briv_rt.c`:
- Section 1.5b (`__print`, `__print_int`, `__exit`) — already handled by direct libc in LLVM codegen
- Section 1.9 (`__trg_timerfd_open/read`, `__trg_signalfd_open/read`) — handled by direct libc `timerfd_create`, `timerfd_read`, etc.
- Phase A: `__readln__`, `__sort_list__`, `__reverse_list__`, `__range__` — can use direct libc
- Phase A: `__stack_top__`, `__queue_front__`, `__hashmap_get__`, `__hashset_elements__` — handled by LLVM codegen or can be migrated
- Phase A: `__ioctl__`, `__isatty__` — direct libc already used
- Phase A: `__spawn_with_output__`, `__spawn__` — convert to direct libc
- Phase A: `__trim_left__`, `__trim_right__`, `__to_lower__`, `__contains_at__`, `__find_from__`, `__int_to_str__`, `__float_to_str__`, `__to_str`, `__splitn__` — string ops that can use direct libc
- Phase B: all `briv_open` through `briv_fcntl` — direct libc already used in codegen
- Phase C: all filesystem shims — direct libc already used in codegen
- Phase D–H: all memory/IPC/signal/networking shims — direct libc already used in codegen
- D12–D18: all shims — direct libc already used in codegen

**Files modified:** `lib/runtime/briv_rt.c` (1744 lines → ~200 lines)
**Kept:** Signal handlers, timer setup, `__rt_init`, thread pool, `@ link` globals

### Step 4: Verify (10 min)

- `cargo test --lib` — all tests pass
- `cargo build` — no warnings
- `bash benchmarks/build_and_bench.sh --correctness` — all benchmarks pass

---

## Per-commit checklist

- `cargo test --lib` — all tests pass
- `cargo build` — no warnings
- `_ => return None;` fallthrough unchanged in all optimization passes
- No weakening of existing optimization paths
- Briv benchmarks pass correctness check
- briv_rt.c still links (no removed symbols that are referenced)
