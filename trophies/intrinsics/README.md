## 167 Compiler Intrinsics: name#() Syntax

**What**: Added 14 new compiler intrinsics (11 system I/O + 3 data operations)
and established the `name#()` calling convention as the uniform syntax for all
intrinsics, replacing the old `as intrinsic "llvm.xxx"` string-matching
approach. The initial 29 grew to 167 as more system, filesystem, IPC, networking,
and synchronization operations were migrated from `frgn` to intrinsics.

**Why it matters**: The new syntax is target-independent — `sqrt#(x)` compiles
to `llvm.sqrt.f32` on x86, but could compile to a hardware sqrt unit on FPGA
without changing source code. The intrinsic set now covers math, terminal I/O,
process spawning, raw file I/O, filesystem operations, memory mapping,
synchronization, IPC, signals, networking, environment, timing, and
benchmark-specific operations.

**How**: `Expr::IntrinsicCall { intrinsic: Intrinsic, args }` dispatches through
the interpreter (real system calls) and LLVM backend (libc/syscall emission).
`emit_declares` auto-generates `declare float @llvm.sqrt.f32(float)` declarations
for used intrinsics. The `name#()` parser rule is a postfix operator: identifier
followed by `#` and `(...)`.

**Before/After**:
| Aspect | Before | After |
|--------|--------|-------|
| Syntax | `as intrinsic "llvm.xxx"` | `name#(args)` |
| IO ops | `frgn __print_float(...)` | `println#(x)` |
| Total intrinsics | 15 | 167 |
