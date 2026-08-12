# Syscall# Intrinsic + Std/os Rewrite + Example Fixes

**Date:** 2026-07-15
**Status:** Active

## Summary

Replace 98 `inop` declarations in `lib/std/os/` and 37 lowercase `#` calls in
examples with a single `Syscall#` intrinsic + pure-Briev stdlib wrappers.

## 1. Syscall# Intrinsic

### Signature
```
Syscall#(op, arg1?, arg2?, arg3?, arg4?, arg5?, arg6?) -> Int
```

`op` is either:
- A PascalCase identifier (`Open`, `Read`, `Write`, `Close`, `Socket`, etc.)
- A raw integer (`2`, `41`, etc.)

When `op` is PascalCase, the backend maps it to the target arch's syscall number.
When `op` is a raw integer, the backend passes it through unchanged.

### Implementation

| File | Change |
|------|--------|
| `src/intrinsic_signatures.rs` | Add `Syscall#` with variable params |
| `src/backend/llvm/intrinsics.rs` | Emit `syscall` instruction (x86_64) or `svc #0` (aarch64) |
| `src/interpreter/intrinsics.rs` | Dispatch via host OS `libc::syscall()` |

### Abstract Op → Syscall Number mapping (x86_64)

| Abstract | Number | Source |
|----------|--------|--------|
| `Read` | 0 | `sys_read` |
| `Write` | 1 | `sys_write` |
| `Open` | 2 | `sys_open` |
| `Close` | 3 | `sys_close` |
| `Mmap` | 9 | `sys_mmap` |
| `Exit` | 60 | `sys_exit` |
| `GetPid` | 39 | `sys_getpid` |
| `Socket` | 41 | `sys_socket` |
| `Connect` | 42 | `sys_connect` |
| `Send` | 44 | `sys_sendto` (simplified) |
| `Recv` | 45 | `sys_recvfrom` (simplified) |
| ... | ... | ... |

## 2. Std/os Rewrite

Each file in `lib/std/os/` replaces `inop` declarations with `defn` wrappers:

```
// Old (inop):
inop file_open(path: Ptr<Byte>, flags: Int, mode: Int) -> Int { ... };

// New (defn + Syscall#):
defn file_open(path: Ptr<Byte>, flags: Int, mode: Int) -> Int {
    Syscall#(Open, path as Int, flags, mode, 0, 0, 0)
};
```

Files to rewrite: fs.bv, net.bv, mem.bv, thread.bv, time.bv, dir.bv, etc.

## 3. Example Fixes

| File | Fix |
|------|-----|
| `error-handling.bv` | `println#` → `Print#` |
| `swan-song.bv` | `println#` → `Print#` |
| `hello-world/src/main.bv` | `println#` → `Print#` |
| `test_ffi.bv` | `println#` → `Print#`, `sqrt#` → `Sqrt#`, etc. |
| `test_ffi_minimal.bv` | `println#` → `Print#` |
| `pipe-skip.bv` | `print_int#` → `Print#` |
| `pipe-chain.bv` | `print_int#` → `Print#` |
| `ptr-arithmetic.bv` | `print_int#` → `Print#` |
| `cell-demo.bv` | `putchar#` → `PutChar#` |
| `process-spawn.bv` | `import` from std/os |
| `networking.bv` | `import` from std/os |
| `mmap-demo.bv` | `import` from std/os |
| `sync-domain.bv` | `read#` → `import` from std/os |
| `macro-demo.bv` | **Delete** (obsolete macro system) |

## 4. Verification

- `cargo test --lib` — all pass
- All changed examples pass `briev check`
- `grep '[a-z]+#(' examples/` — zero lowercase # calls remain
