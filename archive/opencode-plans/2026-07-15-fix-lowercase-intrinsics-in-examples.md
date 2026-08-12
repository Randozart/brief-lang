# Fix Lowercase `#` Intrinsics — Audit & Replace

**Date:** 2026-07-15
**Status:** Plan — not yet implemented
**Branch:** `main`

## Table of Contents

1. [Summary](#1-summary)
2. [Scope](#2-scope)
3. [Documentation](#3-documentation)
4. [Group A: Simple Print# Replacements (8 files)](#4-group-a-simple-print-replacements-8-files)
5. [Group B: Case Corrections (3 files)](#5-group-b-case-corrections-3-files)
6. [Group C: Networking/MMIO with AddressOf# (2 files)](#6-group-c-networkingmmio-with-addressof-2-files)
7. [Group D: Macro Demo Removal (1 file)](#7-group-d-macro-demo-removal-1-file)
8. [Verification Gates](#8-verification-gates)

---

## 1. Summary

The AGENTS.md convention states: **"All intrinsic names are PascalCase + `#` suffix."**
An audit found 37 `#`-suffixed identifiers in examples that violate this rule. They
fall into four categories, each with a different fix strategy.

---

## 2. Scope

**Included:** 14 example `.bv` files with lowercase `#` calls.
**Not included:** Benchmarks (already PascalCase), stdlib (uses `import#` as a compiler
directive, `stdin#` as a trigger name — these are not function-call intrinsics).

---

## 3. Documentation

### 3.1 Rationale comments

Each changed file gets a header comment update noting the `Print#` unification
(`// 2026-07-15: Print# dispatches on argument type`).

### 3.2 Architecture docs

None — no structural changes, just example fixes.

---

## 4. Group A: Simple `Print#` Replacements (8 files)

### 4.1 Rationale

`Print#` is the single print intrinsic. It dispatches on argument type at compile
time. `println#` was never a registered intrinsic; `print_int#` does not match
PascalCase convention and won't resolve.

### 4.2 Files and changes

| File | Old (line) | New |
|------|-----------|-----|
| `examples/error-handling.bv:23` | `println#("Error: " + msg)` | `Print#("Error: " + msg)` |
| `examples/swan-song.bv:16` | `println#("x = " + str)` | `Print#("x = " + str)` |
| `examples/hello-world/src/main.bv:7` | `println#("Hello, World!")` | `Print#("Hello, World!")` |
| `examples/test_ffi.bv:15,55,59` | `println#(...)` (3x) | `Print#(...)` |
| `examples/test_ffi_minimal.bv:7` | `println#("hello from ffi")` | `Print#("hello from ffi")` |
| `examples/pipe-skip.bv:24` | `print_int#(result)` | `Print#(result)` |
| `examples/pipe-chain.bv:15` | `print_int#(result)` | `Print#(result)` |
| `examples/ptr-arithmetic.bv:89` | `print_int#(result)` | `Print#(result)` |

All are one-line replacements. `Print#(...)` takes any printable value
and emits the correct LLVM `__print_<type>` call.

---

## 5. Group B: Case Corrections (3 files)

### 5.1 Rationale

Several examples use `sqrt#`, `sin#`, `pow#` — lowercase forms that don't match
the registered PascalCase intrinsics `Sqrt#`, `Sin#`, `Pow#`. Same for `putchar#`
→ `PutChar#` and OS calls in `process-spawn.bv`.

### 5.2 `examples/test_ffi.bv`

| Line | Old | New |
|------|-----|-----|
| 19 | `sqrt#(x)` | `Sqrt#(x)` |
| 20 | `sqrt#(y)` | `Sqrt#(y)` |
| 24 | `sin#(x)` | `Sin#(x)` |
| 25 | `sin#(y)` | `Sin#(y)` |
| 29 | `pow#(x, y)` | `Pow#(x, y)` |
| 30 | `pow#(y, x)` | `Pow#(y, x)` |

### 5.3 `examples/cell-demo.bv`

| Line | Old | New |
|------|-----|-----|
| 21 | `putchar#(c)` | `PutChar#(c)` |

### 5.4 `examples/process-spawn.bv`

| Line | Old | New |
|------|-----|-----|
| 6 | `argv#(0)` | Remove `#` suffix — inline access via `GetEnv#` |
| 10 | `getpid#()` | `GetPid#()` if registered, else `frgn` |
| 14 | `spawn#(...)` | `Spawn#(...)` |
| 21 | `spawn_with_output#(...)` | `SpawnWithOutput#(...)` |
| 28 | `getenv#(...)` | `GetEnv#(...)` |
| 34 | `setenv#(...)` | `SetEnv#(...)` |
| 40 | `getcwd#()` | `GetCwd#()` |
| 43 | `chdir#(...)` | `ChDir#(...)` |
| 50 | `exit#(0)` | `Exit#(0)` |

### 5.5 `examples/sync-domain.bv`

| Line | Old | New |
|------|-----|-----|
| 7 | `read#(file)` | Remove `#` — not an intrinsic; use `frgn` or `AddressOf#` |

---

## 6. Group C: Networking/MMIO with `AddressOf#` (2 files)

### 6.1 Rationale

`networking.bv` and `mmap-demo.bv` use lowercase `#` for OS system calls
(`socket#`, `connect#`, `mmap#`, etc.) that have never been compiler intrinsics.
These should use `AddressOf#` to resolve the hardware/OS interface, then `frgn`
declarations for the actual syscall wrappers.

### 6.2 `examples/networking.bv`

Current (broken): `socket#(AF_INET, SOCK_STREAM, 0)` — calls non-existent intrinsic.

New approach — typed `AddressOf#` handle + `frgn` syscalls:

```briev
// 2026-07-15: AddressOf# resolves a typed pointer to the networking interface
let net: Ptr<NetIf> = AddressOf#("sys:net");

frgn sys_socket(domain: Int, type: Int, protocol: Int) -> Int from "c";
frgn sys_connect(fd: Int, addr: Ptr<SockAddr>, addrlen: Int) -> Int from "c";
frgn sys_send(fd: Int, buf: Data, len: Int, flags: Int) -> Int from "c";
frgn sys_recv(fd: Int, buf: Data, len: Int, flags: Int) -> Int from "c";
frgn sys_close(fd: Int) -> Int from "c";
```

The `AddressOf#` call provides a typed handle to the platform's networking
subsystem; `frgn` declarations provide the raw syscall surface.

### 6.3 `examples/mmap-demo.bv`

Replace `mmap#(...)` with:

```briev
// 2026-07-15: AddressOf# resolves the memory subsystem handle
let mem: Ptr<MemRegion> = AddressOf#("sys:mmap");

frgn sys_mmap(addr: Int, len: Int, prot: Int, flags: Int, fd: Int, offset: Int) -> Int from "c";
frgn sys_mprotect(addr: Int, len: Int, prot: Int) -> Int from "c";
frgn sys_munmap(addr: Int, len: Int) -> Int from "c";
```

---

## 7. Group D: Macro Demo Removal (1 file)

### 7.1 Rationale

`examples/macro-demo.bv` uses `compile#`, `error#`, `gensym#`, `int_to_str#`,
`warn#` — these belong to the old macro system that was replaced by `$(Stage)`
blocks and `$` intrinsics in Phases 5-6. The file is obsolete.

### 7.2 Action

**Delete** `examples/macro-demo.bv`.

---

## 8. Verification Gates

1. `cargo test --lib` — all tests pass (currently 860)
2. All changed example files pass `briev check`
3. No remaining lowercase `#` calls in `examples/` (grep for `[a-z]+#\(`)
4. `cargo build --release` — no warnings
