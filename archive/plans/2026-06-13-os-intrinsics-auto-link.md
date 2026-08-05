# Plan: OS Intrinsic Surface + Auto-Link from `from` Paths

**Date:** 2026-06-13
**Status:** Active

## Design Principle

**Intrinsics = OS syscalls. C FFI (`from`) = libraries.**

LLVM already knows how to emit every syscall across every target. Briv
intrinsics are thin wrappers that emit direct libc calls — no C runtime
needed for OS operations. Everything else (string ops, JSON, compression)
lives in the stdlib and routes through `from "lib/runtime/file.c"` paths
that auto-compile+link.

## Intrinsic Surface (19 total)

### Terminal (3)
| Intrinsic | LLVM IR | Briv sig |
|-----------|---------|-----------|
| `tty_raw_mode#` | `tcsetattr(0, TCSANOW, &raw)` | `(enable: Int) -> Int` |
| `tty_size#` | `ioctl(1, TIOCGWINSZ, &ws)` → pack `cols*100000+rows` | `() -> Int` |
| `tty_read_key#` | `read(0, &buf, 1)` → return buf as i64 | `() -> Int` |

### Process (1 + 1 existing)
| Intrinsic | LLVM IR | Briv sig |
|-----------|---------|-----------|
| `exit#` | `exit(code)` | `(code: Int)` — EXISTS |
| `spawn#` | `popen(cmd)` → read output → return as Briv string | `(cmd: String) -> String` |

### Filesystem (5 + 2 existing)
| Intrinsic | LLVM IR | Briv sig |
|-----------|---------|-----------|
| `read_file#` | `fopen+fread+fclose` → Briv string | `(path: String) -> String` — EXISTS |
| `write_file#` | `fopen+fwrite+fclose` | `(path, data: String) -> Int` — EXISTS |
| `list_dir#` | `opendir+readdir+closedir` → newline-sep Briv string | `(path: String) -> String` |
| `file_exists#` | `access(path, F_OK)` | `(path: String) -> Int` |
| `delete_file#` | `unlink(path)` | `(path: String) -> Int` |
| `create_dir#` | `mkdir(path, 0755)` | `(path: String) -> Int` |
| `file_size#` | `stat(path, &st)` → return `st.st_size` | `(path: String) -> Int` |

### Environment (2)
| Intrinsic | LLVM IR | Briv sig |
|-----------|---------|-----------|
| `env_get#` | `getenv(name)` → return as Briv string | `(name: String) -> String` |
| `env_set#` | `setenv(name, value, 1)` | `(name, value: String) -> Int` |

### Time (1 + 1 existing)
| Intrinsic | LLVM IR | Briv sig |
|-----------|---------|-----------|
| `time#` | `time(null)` | `() -> Int` — EXISTS |
| `sleep#` | `usleep(ms * 1000)` or `nanosleep` | `(ms: Int)` |

### Console (2)
| Intrinsic | LLVM IR | Briv sig |
|-----------|---------|-----------|
| `print#` | `fputs(msg, stdout)` | `(text: String) -> Int` |
| `read_stdin#` | `fread(stdin, buf, len)` → return as Briv string | `() -> String` |

### Network (5)
| Intrinsic | LLVM IR | Briv sig |
|-----------|---------|-----------|
| `dns_lookup#` | `getaddrinfo(host, ...)` → pack IP as string | `(host: String) -> String` |
| `tcp_connect#` | `socket+connect` → return fd | `(host: String, port: Int) -> Int` |
| `tcp_send#` | `send(fd, data, len, 0)` → return bytes written | `(fd: Int, data: String) -> Int` |
| `tcp_recv#` | `recv(fd, buf, len, 0)` → return data as Briv string | `(fd: Int) -> String` |
| `tcp_close#` | `close(fd)` | `(fd: Int) -> Int` |

## Implementation Order

### Phase 1: Intrinsic enum + match arms (emit_expr.rs)
1. Add all 16 new variants to `Intrinsic` enum in `ast.rs`
2. Add match arms in `emit_expr.rs:Intrinsic` handler — each emits direct libc call
3. Briv string helper: for intrinsics returning strings, emit `alloca` + data pointer + length + chars, return `ptrtoint`

### Phase 2: Auto-link from `from` paths (main.rs)
1. In `run_llvm_compile`, scan `program.items` for `ForeignBinding` entries
2. For each `from` path ending in `.c`, add to the LTO foreign modules list
3. Auto-compile to bitcode via `compile_to_bitcode` (already exists — handles `.c` → `.bc`)
4. Feed into `link_llvm_modules`

### Phase 3: Stdlib string/JSON implementations (briv_rt.c)
1. Implement `__trim`, `__to_lower`, `__split`, `__substring`, `__starts_with`, `__contains`
2. Implement `__parse` (JSON), `__is_array`, `__len`, `__get`, `__at`

### Phase 4: Officina-cli migration
1. Replace 14 `frgn` declarations with `import "std/..."` calls
2. Keep 3 TTY intrinsics as direct `frgn` (no `from`)
3. Build + test

## Files Changed

| File | What |
|------|------|
| `src/ast.rs` | Add 16 new `Intrinsic` variants |
| `src/backend/llvm/emit_expr.rs` | 16 new match arms + Briv string helper |
| `src/main.rs` | Auto-link from `from` paths in LTO pipeline |
| `lib/runtime/briv_rt.c` | Implement __trim, __to_lower, __split, etc. |
| officina-cli/officina.bv | Replace frgn with import std/... |
