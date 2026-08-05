# 10 New `#` Intrinsics — Atomic, DlOpen, Backtrace

**Date:** 2026-07-15
**Status:** Active — implementation in progress
**Branch:** `main`

## 1. Summary

Add 10 new `#` intrinsics to replace stubs in `lib/std/os/atomic.bv`,
`lib/std/os/ring.bv`, `lib/std/os/dynlib.bv`, and `lib/std/os/debug.bv`.

## 2. Why These Must Be `#` Intrinsics

Per AGENTS.md Golden Rule #3: **"INTRINSICS BEFORE FRGN"** — before writing
`frgn`, check if an intrinsic exists. The `#` suffix mechanism is for
operations the compiler must know about to emit correct LLVM IR.

| Category | Why `#` intrinsic, not `defn` | Why not `Syscall#` |
|----------|-------------------------------|-------------------|
| **Atomic** (6) | LLVM `atomicrmw` / `load atomic` / `fence` instructions have no Briv operator. The compiler must emit the `seq_cst` ordering flag. | Not syscalls — these are CPU instructions emitted inline. |
| **dlopen/dlsym/dlclose** (3) | Platform dynamic linker ABI. The compiler must emit the correct calling convention for `@dlopen`. | Not syscalls — these are C library functions resolved by the system linker. |
| **backtrace** (1) | Stack walking requires LLVM `@llvm.frameaddress` or DWARF unwind. The compiler controls frame layout. | Not a syscall — it's a debugging primitive like `Print#`. |

## 3. Documentation Strategy

### 3.1 Rationale comments

```
// 2026-07-15: $name — $reason (see docs/plans/2026-07-15-atomic-dlopen-backtrace-intrinsics.md)
```

Every dispatch arm, emit handler, and interpreter entry gets this comment.

### 3.2 Architecture docs to update

`docs/architecture/features/plugins.md` — add the new intrinsics to the
intrinsic table.

## 4. Intrinsic Specifications

### 4.1 Atomic Operations (LLVM `atomicrmw`)

| # | Intrinsic | Args | Returns | LLVM IR |
|---|----------|------|---------|---------|
| 1 | `AtomicLoad#` | `(ptr: Ptr<Byte>)` | `Int` | `%v = load atomic i64, ptr %ptr, seq_cst` |
| 2 | `AtomicStore#` | `(ptr: Ptr<Byte>, val: Int)` | `Int` | `store atomic i64 %val, ptr %ptr, seq_cst` |
| 3 | `AtomicCas#` | `(ptr: Ptr<Byte>, expected: Int, desired: Int)` | `Int` | `%v = cmpxchg ptr %ptr, i64 %exp, i64 %des seq_cst seq_cst` |
| 4 | `AtomicXchg#` | `(ptr: Ptr<Byte>, val: Int)` | `Int` | `%v = atomicrmw xchg ptr %ptr, i64 %val seq_cst` |
| 5 | `AtomicAdd#` | `(ptr: Ptr<Byte>, val: Int)` | `Int` | `%v = atomicrmw add ptr %ptr, i64 %val seq_cst` |
| 6 | `Fence#` | `()` | `Void` | `fence seq_cst` |

Interpreter: non-atomic heap load/store (correct for single-threaded check mode).

### 4.2 Dynamic Linker (platform library functions)

| # | Intrinsic | Args | Returns | LLVM IR |
|---|----------|------|---------|---------|
| 7 | `DlOpen#` | `(path: Ptr<Byte>, flags: Int)` | `Ptr<Byte>` | `%h = call ptr @dlopen(ptr %path, i32 %flags)` |
| 8 | `DlSym#` | `(handle: Ptr<Byte>, symbol: Ptr<Byte>)` | `Ptr<Byte>` | `%s = call ptr @dlsym(ptr %handle, ptr %symbol)` |
| 9 | `DlClose#` | `(handle: Ptr<Byte>)` | `Int` | `%r = call i32 @dlclose(ptr %handle)` |

Interpreter: actual `libc::dlopen`/`dlsym`/`dlclose` calls on the host.

### 4.3 Debugging (stack trace)

| # | Intrinsic | Args | Returns | LLVM IR |
|---|----------|------|---------|---------|
| 10 | `Backtrace#` | `()` | `Int` | `%r = call i64 @briv_backtrace()` |

Interpreter: stub returning 0.

## 5. Implementation Steps

### Step 1: `src/intrinsic_signatures.rs`

Add all 10 signatures + update test list.

### Step 2: `src/backend/llvm/intrinsics.rs`

Add 10 emit handlers and 10 dispatch arms. Each handler is ~5-10 lines.

### Step 3: `src/backend/llvm/emit_toplevel.rs`

Add declares for `@dlopen`, `@dlsym`, `@dlclose`.

### Step 4: `src/interpreter/intrinsics.rs`

Add 10 interpreter entries:
- Atomic ops: simple heap operations (not atomic in check mode)
- DlOpen/DlSym/DlClose: `libc::dlopen`/`dlsym`/`dlclose`
- Backtrace: stub

### Step 5: Update `.bv` files

- `lib/std/os/atomic.bv` — replace stubs with `AtomicLoad#`/`AtomicStore#`/etc.
- `lib/std/os/ring.bv` — implement `ring_push`/`ring_pop` using `AtomicCas#`
- `lib/std/os/dynlib.bv` — `to_c_string` + `DlOpen#`/`DlSym#`/`DlClose#`
- `lib/std/os/debug.bv` — replace stub with `Backtrace#`

## 6. Verification

- `cargo test --lib` — all 860+ tests pass
- All 21 std/os files pass `briv check`
- `cargo build --release` — no warnings
