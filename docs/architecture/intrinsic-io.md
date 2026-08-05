# Intrinsic I/O — Print family + stream symbols

**2026-08-01 status:** this page's "C-independent inline I/O" design was
SUPERSEDED. The actual implementation (Phase 2 audit + Phase 4) consolidated the
four special-cased print intrinsics into ONE generic `Print#` that dispatches by
the argument's protocol category (String/Char/Bool/Float/else Int) and routes
through the runtime `__print_*` family (`briv_rt.c`), and added the stream
symbols `#StdOut <- value` / `#StdErr <- <String>` / `#StdIn`. The C-surface
reduction is now the `.bv`/`.ebv` split (`docs/architecture/c-surface-inventory.md`),
not inline IR. The sections below are the archived pre-2026-08-01 design.

## Motivation (archived)

`PrintInt#` currently routes through `briv_rt.c`:

```
PrintInt#(42) → call @__print_int → briv_rt.c → printf → libc → syscall write
```

This has three problems:
1. **C dependency**: Briv cannot self-host I/O without a C toolchain.
2. **Portability coupling**: `briv_rt.c` must exist for every target, even though the actual
   I/O mechanism is platform-specific and the arithmetic is universal.
3. **Backend lock-in**: Webstack and CIRCT cannot share `briv_rt.c` — they must duplicate.

The new design inlines ALL behavior into the backend, emitting zero external calls for I/O.

## Design: Two Concerns, One Intrinsic

Every `PrintInt#` invocation decomposes into **two concerns**:

| Concern | What | Where | Platform |
|---------|------|-------|----------|
| **Arithmetic** | Int → decimal characters | Inline IR (all backends share the pattern) | Portable |
| **I/O** | characters → hardware | LLVM inline asm, WASM `fd_write`, etc. | Per-backend |

### Arithmetic: Decimal Conversion Pattern

```llvm
; Input:  %n = i64 <value>
; Output: writes decimal representation to %buf, returns %len

%buf = alloca [20 x i8]         ; max 20 digits for i64
%buf_ptr = getelementptr inbounds [20 x i8], ptr %buf, i32 0, i32 19
store i8 0, ptr %buf_ptr        ; sentinel
%tmp_ptr = ptrtoint ptr %buf_ptr to i64

loop:                           ; [tmp_ptr > buf_ptr] → invariant
  %tmp = phi i64 [%tmp_ptr, %entry], [%next_ptr, %loop]
  %n = phi i64 [%val, %entry], [%quot, %loop]
  %rem = srem i64 %n, 10
  %quot = sdiv i64 %n, 10
  %char = add i64 %rem, 48      ; '0' = 48 ASCII
  %next_ptr = sub i64 %tmp, 1
  %char_ptr = inttoptr i64 %next_ptr to ptr
  store i8 %char, ptr %char_ptr
  %done = icmp eq i64 %quot, 0
  br i1 %done, label %done, label %loop

done:
  %len = sub i64 %tmp_ptr, %next_ptr  ; exclude sentinel
  %buf_start = inttoptr i64 %next_ptr to ptr
```

This algorithm is **identical** across all three active backends. The only thing that
changes between targets is the final I/O instruction.

### I/O: Per-Backend Implementation

#### LLVM (Linux x86_64) — Inline Syscall

```llvm
; write(rdi=1=stdout, rsi=buf, rdx=len)
%rax_val = call i64 asm sideeffect "syscall",
    "{rax},${0:q},{1:q},{2:q},{3:q},{4:q},{5:q}"
    (i64 1, i64 %buf_addr, i64 %len, i64 0, i64 0, i64 0)
    : "rax", "rcx", "r11", "memory"
    : "intel"
```

Uses LLVM `asm sideeffect` — no C dependency. The `syscall` instruction is x86_64-native.
The `sideeffect` tag prevents DCE from removing the instruction, and it carries the
implied `observable` property (backend cannot reorder past it).

ARM64 equivalent uses `svc #0` with different register mapping (x8=1 for write,
x0=1 for fd, x1=buf, x2=len).

WASM uses `call $fd_write (i32 1, i32 %buf, i32 %len) → i32`.

#### Webstack — WASI fd_write

```llvm
; Emitted as call to WASI fd_write
call i32 @__wasi_fd_write(i32 1, ptr %iov, i32 1, ptr %nwritten)
; %iov = <{ ptr %buf, i32 %len }>
```

Webstack already compiles to WASM. WASI provides `fd_write` for stdout.
No `briv_rt.c` needed — WASI is the platform.

#### CIRCT — Simulation Print

```mlir
// Emitted as system verilog $write or simulation-only display
%0 = "circt.print"(%buf) : (!seq.vector<20 x i8>) -> ()
```

Hardware backends cannot use syscalls. CIRCT emits `$write` for simulation;
real hardware routes through a UART peripheral.

### Float I/O

`PrintFloat#` shares the same structure:

```
PrintFloat#(3.14):
  ┌─ Arithmetic: Float → sign + integer digits + fraction digits + "e±NN" if scientific
  │  (harder than Int — needs frexp / multiplication by powers of 10)
  │
  └─ I/O: same syscall / fd_write / $write call
```

**Implement after PrintInt#.** Float-to-decimal is a separate concern; the I/O path is shared.

### StdOut#<T> / StdIn#<T> — REALIZED as stream symbols (Phase 4, 2026-08-01)

The polymorphic write is realized as the `#StdOut` / `#StdErr` stream symbols
(not a generic `StdOut#<T>` intrinsic): `#StdOut <- value` lowers to the generic
`Print#` (type-dispatched by protocol category), `#StdErr <- <String>` lowers to
`__eprint_str` (stderr), and `#StdIn` is a `Ptr<Int>` stream-handle value.
Read-side parsing (stdin → Int/Float) is still future work.

## Backend Contract

Every active backend must implement:

| Intrinsic | Signature | Status |
|-----------|-----------|--------|
| `PrintInt#` | `PrintInt#(val: Int)` → `Void` | **Step 7 — implementing now** |
| `PrintFloat#` | `PrintFloat#(val: Float)` → `Void` | Deferred |
| `PrintChar#` | `PrintChar#(val: Char)` → `Void` | Deferred |
| `ReadInt#` | `ReadInt#()` → `Int` | Deferred |

Dead backends (verilog.rs, vhdl.rs, c.rs, rust.rs, cobol.rs, x86_64.rs, aarch64.rs,
wasm.rs) are NOT required to implement these. Ignore with `todo!()` if mechanically
touched by the shared API.

## Observability

Every I/O intrinsic MUST set `observable <~ true` in its metadata. This prevents DCE
from eliminating the call when the result is unused. Without this, the compiler is
correct to optimize `PrintInt#(42);` to `ret void` (no observable effect according to
the IR).

The `observable` property is respected by all three backends:
- LLVM: `sideeffect` on inline asm, or `nounwind` + `readnone` → not readnone
- Webstack: WASI calls are side-effecting by definition
- CIRCT: `$write` is simulation-side-effecting

## Architecture Doc Updates

When implementing PrintInt#:
1. Add `PrintInt#` to `get_intrinsic_signature()` — `observable: true`
2. Add `PrintInt#` handler in `emit_intrinsic_call()` — emits decimal conversion + syscall
3. Remove `@__print_int` declaration from `rt_init` / `emit_declares`
4. All three active backends: LLVM gets inline syscall, Webstack gets WASI fd_write,
   CIRCT gets simulation print stub
5. Dead backends: `match _ => todo!()`

## Rationale Comments

Every modified code site must carry:

```
// 2026-07-28: C-independent I/O. PrintInt# inlines decimal conversion + syscall.
// No briv_rt.c dependency. observable<~true> prevents DCE of side effects.
// Replaces @__print_int from briv_rt.c.
```
