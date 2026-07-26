# C Independence Roadmap

## Goal

Eliminate C as a source-level dependency of the Brief compiler and stdlib.
The compiler should produce binaries by invoking `llc` + `lld`, not `clang`.
The stdlib should implement all runtime functions in Brief, calling OS syscalls
through `frgn from #System` and `#Link` directives instead of compiling `.c` files.

## Current C Dependencies

### Source-level (`.c` files)

| File | Used by | Functions |
|------|---------|-----------|
| `lib/runtime/brief_rt.c` | 8 stdlib ffi modules | ~100 fn: print, env, time, string ops, encoding (base64/hex/URL/HTML/md5/sha/uuid), HTTP, SHM, mmap |
| `lib/runtime/brief_gpu_rt.c` | GPU backend | GPU runtime |
| `lib/std/c/xxhash/xxhash.c` | `ffi/xxhash.bv` | XXH64, XXH32 |
| `lib/std/c/lz4/lz4.c` | (unused) | LZ4 compression |
| `lib/std/c/stb_image/stb_image.c` | (unused) | Image loading |

### Tool-level

- **`clang`**: Compiles `.ll` → binary, compiles `.c` sources → `.o`
- **`libm`**: `-lm` linked for math functions
- **LLVM**: Written in C++, unavoidable

## Roadmap

### Phase 1: Remove hardcoded C references (this commit)

- Remove hardcoded `brief_rt.c` from `compile_ll_to_binary` — it's already carried
  by `extra_objects` via frgn declarations
- Deduplicate `extra_objects` at the merge point

### Phase 2: Replace `clang` with `llc` + `lld`

**Problem:** `compile_ll_to_binary` invokes `clang` to compile `.ll` → binary.
This requires a C compiler toolchain to be installed.

**Solution:**
- Use `llc` to compile `.ll` → `.o` (already done for WASM in `compile_wasm`)
- Use `lld` (or wasm-ld) to link `.o` → binary
- This eliminates the C compiler tool dependency entirely

**Dependencies replaced:**
- `clang -O3 -flto` → `llc -O3` + `lld -O3`
- `-lm` → no linker flag needed (Brief implements math in Brief or calls `#System` libm)

### Phase 3: Rewrite `brief_rt.c` in Brief

Each function group can be migrated independently:

| Group | Approach | Difficulty |
|-------|----------|------------|
| Pure string ops (`to_upper`, `is_alpha`, `rfind`, etc.) | `defn` in Brief | Low — pure computation |
| Encoding (base64, hex, URL, HTML, UTF8) | `defn` + `txn` in Brief | Low — pure computation |
| Hashing (md5, sha1, sha256, sha512) | `defn` in Brief | Medium — algorithmic |
| UUID generation | Brief with `frgn from #System` `/dev/urandom` | Low |
| Time functions (`now`, `year`, `format_timestamp`, etc.) | `frgn from #System` `clock_gettime`, `localtime_r` etc. | Medium — POSIX API |
| I/O (`print_int`, `print_float`, `print_char`, `print_str`) | `frgn from #System` `write` syscall | Low |
| HTTP (`http_get`, `http_post`) | `frgn from #System` socket APIs | Medium |
| SHM/mmap (`shm_open`, `mmap`, `munmap`) | `frgn from #System` POSIX APIs | Medium |
| XXHash | Rewrite in Brief or `frgn from #System xxhash` | Medium |

A migrated function looks like:
```brief
// Instead of:  frgn __to_upper(s: String) -> String from "lib/runtime/brief_rt.c"
// Write:
frgn __to_upper(s: String) -> String from #System;
```
With `#Link<xxx>` if a system library is needed.

### Phase 4: Move `brief_rt.c` out of the repo

Once all functions are migrated:
- Delete `lib/runtime/brief_rt.c` and `lib/runtime/brief_gpu_rt.c`
- Remove C compilation from `collect_extra_objects` (no more `.c` → `.o` step)
- All runtime functions are either Brief `defn`/`txn` or `frgn from #System`

## Measure of Success

```bash
# No .c files in the source tree (except third-party in lib/std/c/)
find . -name '*.c' -not -path './lib/std/c/*' | wc -l
# Should print 0

# No clang invocation during build
# compile_ll_to_binary uses llc + lld

# cargo build --release succeeds with --no-stdlib
# A "hello world" program compiles with --no-stdlib and only #System frgns
```

## Non-goals

- Eliminating LLVM (C++). That would be a completely different backend strategy.
- Eliminating `libm` / system libraries at the OS level. `-lm` is fine when
  declared explicitly via `from #System`.
- Eliminating `clang` for C compilation of third-party C libs (xxhash, lz4,
  stb_image) — those are optional extensions, not core dependencies.
