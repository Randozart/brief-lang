# `as intrinsic` — Verbatim LLVM Intrinsic Support

**Date added:** 2026-06-10
**Status:** Active (will be replaced by `name#()` syntax — see `intrinsics.md`)

## Purpose

Allow `frgn` declarations to emit LLVM intrinsic calls instead of normal FFI
function calls. Used for `llvm.sqrt.f32`, `llvm.ceil.f32`, etc.

## Syntax

```brief
frgn sqrt_f32(x: Float) -> Float as intrinsic "llvm.sqrt.f32" ;
```

The `as intrinsic "..."` clause declares the LLVM symbol name verbatim. The
backend emits it at both declare and call sites.

## How it works

| Layer | What happens |
|---|---|
| **Parser** | After `from` clause, parses `as intrinsic STRING` → stores in `ForeignSignature.intrinsic_name` |
| **LLVM backend declare** | Uses `sig.intrinsic_name.unwrap_or(name)` as the LLVM symbol |
| **LLVM backend call** | Same — uses `intrinsic_name` as call target |
| **Interpreter** | Ignores `intrinsic_name` — resolves via TOML path as normal |

## Standard library usage

```brief
// lib/std/llvm.bv
frgn sqrt_f32(x: Float) -> Float  as intrinsic "llvm.sqrt.f32" ;
frgn sqrt_f64(x: Float) -> Float  as intrinsic "llvm.sqrt.f64" ;
```

## Limitations

- Only supports LLVM intrinsics whose signature matches the Brief `frgn`
  declaration exactly. Intrinsics with extra parameters (like `llvm.abs.i64`
  with its `i1` zero-undef flag) can't be expressed and remain hardcoded in
  `emit_declares()`.

## Replacement

This mechanism is deprecated. The `name#()` intrinsic syntax (see `intrinsics.md`)
will replace it entirely.
