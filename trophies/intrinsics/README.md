## 29 Compiler Intrinsics: name#() Syntax

**What**: Added 14 new compiler intrinsics (11 system I/O + 3 data operations)
and established the `name#()` calling convention as the uniform syntax for all
intrinsics, replacing the old `as intrinsic "llvm.xxx"` string-matching
approach.

**Why it matters**: The new syntax is target-independent — `sqrt#(x)` compiles
to `llvm.sqrt.f32` on x86, but could compile to a hardware sqrt unit on FPGA
without changing source code. The 14 new intrinsics (println, readln, exit,
time, read_file, write_file, sleep, socket, bind, listen, accept, sort, reverse,
range) cover the most common FFI patterns that benefit from compiler knowledge
of types and control flow.

**How**: `Expr::IntrinsicCall { intrinsic: Intrinsic, args }` dispatches through
the interpreter (real system calls) and LLVM backend (libc/syscall emission).
`emit_declares` auto-generates `declare float @llvm.sqrt.f32(float)` declarations
for used intrinsics. The `name#()` parser rule is a postfix operator: identifier
followed by `#` and `(...)`.

**Before/After**:
| Aspect | Before | After |
|--------|--------|-------|
| Syntax | `as intrinsic "llvm.sqrt.f32"` | `sqrt#(x)` |
| IO ops | `frgn __print_float(...)` | `println#(x)` |
| Total intrinsics | 15 | 29 |
