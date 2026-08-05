# Accelerated Briv (`.abv`) — Native GPU Compilation

**Date added:** 2026-06-18 (renamed from Graphic Briv 2026-06-19)  
**Phase:** 1 (StrictMode, type validation, SPIR-V output)  
**Status:** Active — canonical GPU file format  
**Also known as:** "Briv Accel"

---

## Purpose

Accelerated Briv is a dedicated file extension that always compiles to GPU
(SPIR-V). Unlike `.bv` with `#gpu` or `--gpu-offload`, `.abv` is implicit —
no flags or directives needed. It enforces type/intrinsic restrictions at
compile time so every valid `.abv` file is guaranteed to produce valid SPIR-V.

## Syntax

`.abv` uses the same syntax as `.bv`. Key differences are enforced by the
typechecker, not the parser:

- **No `frgn`** — `frgn` declarations are banned. Only well-known GPU
  intrinsics (sin, cos, pow, sqrt, fabs, ceil, floor, get_global_id,
  get_local_id, get_group_id, get_num_groups, barrier) are allowed.
- **Types** — Only `Int`, `UInt`, `Float`, `Bool`, `Char`, `String`
  (const-only), and fixed-size arrays `[T; N]` are allowed.
- **Contracts** — Optional, same sugar as `.bv` (`[pre][post]`, `[[post]`,
  `[pre]]`). Missing contracts produce a warning.
- **Implicit GPU** — No `#gpu` directive needed. All transactions are
  compiled to SPIR-V.

## Type Enforcement

The typechecker (`validate_gpu_program` in `src/typechecker.rs`) checks:

| Error code | Condition | Severity |
|------------|-----------|----------|
| G001 | FFI usage (`frgn` or `Expr::Call`) | Error |
| G002 | Transaction with no contracts | Warning |
| G003 | Disallowed type (struct, enum, HashMap, etc.) | Error |
| G004 | Disallowed intrinsic | Error |

## Compilation Pipeline

```
file.abv → parser (StrictMode::Gpu) → import resolver →
           typechecker (GPU validation) → LlvmBackend.generate() →
           CPU IR + embedded SPIR-V blob + standalone .spv file
```

The compilation is triggered automatically by `briv-compiler build file.abv`.
The `is_gpu_extension()` check in `main.rs` detects `.abv` and:
1. Sets `StrictMode::Gpu` in the parser
2. Enables `gpu_offload = true` for the LLVM backend
3. Writes `file.spv` (standalone SPIR-V binary) alongside the native binary

## Detection Points

| Entry point | `.abv` support |
|-------------|----------------|
| `briv-compiler build file.abv` | ✅ Auto-detected, GPU path |
| `briv-compiler llvm file.abv` | ✅ Auto-detected (via `is_gpu_extension`) |
| `briv-compiler check file.abv` | ✅ Auto-detected (via `is_gpu_extension`) |

## CLI

```text
$ briv-compiler build file.abv
Building Accelerated Briv (.abv) file: compiling via GPU...

$ briv-compiler llvm file.abv --gpu-backend opencl
Compiling to LLVM IR: file.abv (GPU — .abv)
```

## Related Files

| File | Purpose |
|------|---------|
| `src/ast.rs` | `StrictMode::Gpu` variant |
| `src/parser.rs` | `.with_gpu_mode()` builder method |
| `src/typechecker.rs` | `validate_gpu_program()` — G001–G004 checks |
| `src/main.rs` | `is_gpu_extension()`, `run_build` `.abv` arm |
| `src/backend/llvm/gpu.rs` | SPIR-V kernel extraction and emission |
