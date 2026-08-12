# C-Surface Reduction — Runtime Inventory + `.bv`/`.ebv` Split

**Date:** 2026-08-01
**Status:** Active (Phase 6 of the master plan)
**Plan map:** `2026-08-01-consumptive-operators-lifetime-and-c-surface.md`

## Goal

Reduce the C surface (`lib/runtime/briev_rt.c`, 710 lines) so a freestanding
`.ebv` build needs no libc. Classify every function as *syscall shim* (stays C,
the OS boundary) or *logic* (movable to pure Briev `.ebv`). The `.bv` variants
keep the OS/libc calls; the `.ebv` variants hold the pure-Briev logic; the
import resolver prefers `.bv` on OS targets and `.ebv` on freestanding.

## Classification (2026-08-01 audit)

### Syscall / OS shim — stays C (the boundary)

| Function | OS surface | Notes |
|---|---|---|
| `briev_syscall` | raw `syscall(2)` | the universal shim; no libc needed |
| `briev_sysconf` | `sysconf(3)` | replaceable via `briev_syscall` |
| `__getenv_*` (`__getenv_briev`/`__getenv_int`/`__get_environ`) | `getenv`/`environ` | |
| `__argv_*` (`__argv_count`/`__argv_get`/`__argv_has`/`__argv_value`/`__argv_command`) | `argv`/`main` | |
| `__trg_timerfd_*`, `__trg_signalfd_*` | `timerfd`/`signalfd` (Linux) | trigger sources |
| `ShellCmd` | `popen`/`system` | |
| `__read_file__`/`__write_file__` | `open`/`read`/`write` | via syscall |
| `tty_*`/`briev_ttyname`/`__tty_raw_mode__`/`__tty_size__`/`__tty_read_key__` | `tcgetattr`/`tcsetattr`/`ioctl`/`ttyname` | |
| `__readln__` | `read`/stdin | |
| `__rt_init`/`__rt_wait`/`__rt_poll`/`__wait_for_trigger__` | `poll`/`epoll` | reactor loop |
| `__watchdog_fail` | `fprintf(stderr)` | |
| `__exit` | `_exit` | |
| `__briev_now` | `clock_gettime` | watchdog deadlines |
| `__briev_free`/`__briev_free_count` | `free` | the counting free |
| `worker_thread`/`__thread_pool_init__`/`__barrier_*`/`__set_async_state__` | pthreads | async/sync groups |

### Logic — movable to pure Briev `.ebv`

| Function | Logic | Move target |
|---|---|---|
| `briev_str_to_c` / `briev_cstr_to_briev` | the `[len][bytes]` ↔ C string marshalling | `.ebv` string module |
| `briev_free_briev_str` | string buffer free | `.ebv` |
| `briev_bits_to_str` | bits → string | `.ebv` |
| `briev_char_len` | char length | `.ebv` |
| `briev_str_eq` | content equality | `.ebv` |
| `briev_str_band`/`bor`/`bxor`/`bnot` + `briev_str_bitop` | string bitwise | `.ebv` |
| `__print_*` (`__print`/`__print_int`/`__print_bool`/`__print_float`/`__print_float64`/`__print_char`/`__print_str`/`__eprint_str`) | formatting | `.ebv` via a `WriteByte`/`WriteStr` syscall shim |
| `__sort_list__`/`__reverse_list__` | sorting/reversal | `.ebv` collections |

### The split

- **`.bv` (OS):** the `import "link/briev_rt.c"` shims + `std/ffi/*.bv` keep the
  syscall boundary. The print family stays C here (fast path, `fputs`/`printf`).
- **`.ebv` (freestanding):** the logic modules are reimplemented in pure Briev
  over a minimal shim: `syscall(num, args...)` (the single raw syscall),
  `_start`/`_exit`, and a `Write` syscall for output. The string marshalling,
  formatting, equality, and list ops move into `.ebv` files.

## The no-libc target sketch

1. `_start` (assembly) calls `briev_main` with the stack-aligned `argc`/`argv`,
   then `_exit` with the return code.
2. `briev_syscall` is the ONLY C function (or inline assembly) — `syscall(2)`
   numbers for `read`/`write`/`exit`/`brk`/`clock_gettime`.
3. The `.ebv` stdlib: string layout + formatting + collections in pure Briev;
   output via `write(1, buf, len)`.
4. The allocator: a bump/`brk` allocator in `.ebv` (or C), with
   `__briev_free` counting removed (no free on freestanding).

## Next steps — blocking prerequisites (2026-08-01 audit)

The `.ebv` string/formatting modules cannot be written and verified yet:

1. **String construction from bytes** — pure-Briev int→string formatting needs a
   `bytes → String` primitive (build a `[len][bytes]` buffer). Today the buffer
   is only READ via `StrBytes#` (String → List<Int>); the inverse does not exist
   (the C runtime does all formatting). A `CharStr#`/`StrFromBytes#` intrinsic
   (or a stdlib op) is the prerequisite.
2. **A verified write-syscall shim** — `briev_syscall` exists (the raw
   `syscall(2)`), but there is no Briev-level `write(fd, ptr, len)` wrapper that
   the `.ebv` print family can call. The `.bv` print path uses `fputs`/`printf`
   (libc); the `.ebv` path needs the raw-syscall write.
3. **The no-libc build flow** — a freestanding target in `config/targets.toml` +
   `_start` (assembly) + the import-resolver `.ebv` preference wiring. Without a
   build flow, `.ebv` modules cannot be compiled to a runnable binary, and
   unverified stdlib code violates the operating contract.

Until these land, the `.ebv` split remains DESIGNED (this inventory + the
master plan) with the C surface unchanged.
