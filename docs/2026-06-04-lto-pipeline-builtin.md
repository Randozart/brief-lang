# LTO Pipeline: `-fno-builtin` Removal & `alwaysinline` FFI Functions

**Date:** 2026-06-04

## Discussion

### The problem

`frgn __sqrtf(d: Float) -> Float` generates `call float @__sqrtf(float)` in LLVM IR.
C with `-ffast-math` converts `sqrtf()` to `vrsqrtps` — hardware reciprocal sqrt, single instruction.
Brief's `__sqrtf` goes through PLT → libc `sqrtf()` — ~21 calls per tick @ nbody_sqrt, 2.15× slower.

### The root cause

`try_lto_pipeline()` in `src/main.rs` passes `-fno-builtin` to `clang -c -emit-llvm` when
compiling `brief_rt.c` to LLVM bitcode. This flag prevents clang from recognizing `sqrtf()`
as a builtin and emitting the `llvm.sqrt.f32` intrinsic. The resulting `.bc` has a raw
`call float @sqrtf(float)` that even LTO can't optimize.

### Why `-fno-builtin` was there

It arrived as boilerplate alongside `-ffreestanding` and `-fno-stack-protector` in the initial
LTO pipeline commit. Standard bare-metal/embedded flags. But `brief_rt.c` links against libc
and libm — there's no bare-metal target for this code path. The flag was never a deliberate
policy choice; it was copy-pasted from standard embedded build scripts.

### The only function affected

`sqrtf` is the **only** function in `brief_rt.c` that clang would recognize as a builtin.
No other function (`getenv`, `strtol`, `fprintf`, `fwrite`, `fread`, `putchar`, `exit`,
`pthread_*`, `epoll_*`) maps to an LLVM intrinsic.

### Policy: magic in LLVM, not in Brief

Brief's compiler must remain transparent — no hardcoded string matches for `__sqrtf`,
no `llvm.sqrt.f32` intrinsic emission in `emit_expr`, no special-case handling of any
FFI function name. The No Magic principle applies to Brief's compiler source code.

The LTO pipeline is *not* Brief's compiler. It's the same LLVM toolchain every compiled
language uses. C gets `sqrtf` → `fsqrt` because clang recognizes `sqrtf` as a builtin.
Brief should get the same treatment for the same reason — both pipe through the same
`opt -O3`. Removing `-fno-builtin` just removes an artificial handicap.

### When `-fno-builtin` IS useful

**Bare-metal targets without an FPU** — where `sqrtf` as a hardware instruction would
crash. But Brief's `.ebv` (Embedded Brief) targets already use different backends
(Verilog, AArch64, WASM) with separate compilation pipelines. The x86-64 LTO path is
always targeting systems with libc, libm, and FPU. If a bare-metal x86-64 target is
added in the future, `-fno-builtin` can be conditionally re-enabled for that target.

### Conclusion

1. Remove `-fno-builtin` from the LTO `clang -c -emit-llvm` command (line 1906)
2. Mark leaf FFI functions in `brief_rt.c` as `__attribute__((alwaysinline)) static inline`:
   `__sqrtf`, `__putchar`, `__print_str_len`, `__write_bytes` — single-call wrappers
   that benefit from LTO inlining
3. Add `-ffast-math` to `opt -O3` (line 1949) — matches C's `-ffast-math`, enables `vrsqrtps`
4. Keep the `cc` fallback path unchanged — `-fno-builtin` serves as documentation there
   (makes no practical difference since `cc` doesn't emit intrinsics)


## Verification (2026-06-04)

Step 1 failed (`-fno-builtin` removal alone didn't help because `-ffreestanding` implicitly adds `"no-builtins"` attribute to all functions).

Additional discovery during implementation: `-ffreestanding` is the real blocker.
When `clang -c -emit-llvm -ffreestanding` compiles any `.c` file, it attaches
`"no-builtins"` to every function attribute group. `-fno-builtin` is redundant —
`-ffreestanding` already disables all builtin recognition.

Fix: removed BOTH `-ffreestanding` and `-fno-builtin` from the LTO `clang -c -emit-llvm`
command. `brief_rt.c` is NOT a freestanding program — it links against libc/libm
and calls `fprintf`, `fwrite`, `malloc`, `sqrtf`, etc.

Verified: regenerated `brief_rt.bc` has no `"no-builtins"` attribute. Tests pass (437).
nbody_sqrt improved from 2.15× to 1.88× despite all physics fields being dead
(DFE eliminates everything — separate issue). Full benefit when fields are live.

## Actual changes

1. `src/main.rs:1906` — Removed `-ffreestanding` and `-fno-builtin`:
   `-c -emit-llvm -O2 -fno-stack-protector`

2. `src/main.rs:1949` — Added `-ffast-math` to `opt -O3`

3. `runtime/brief_rt.c` — Marked `__putchar`, `__print_str_len`, `__write_bytes`
   as `__attribute__((always_inline)) static inline` (leaf FFI wrappers).
   `__sqrtf` reverted to non-static (collides with glibc `__sqrtf` —
   not needed since intrinsic conversion is now handled by clang without
   `-ffreestanding`).
