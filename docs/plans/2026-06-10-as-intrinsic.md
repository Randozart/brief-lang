# `as intrinsic` — Verbatim LLVM Intrinsic Support

**Date:** 2026-06-10T18:09:38+02:00  
**Status:** Planned → In Progress  
**Files changed:** ~110 lines across 9 files

## The Problem

Briev's `frgn` system routes FFI calls through the dynamic FFI registry (interpreter) or
direct function calls (LLVM backend). For LLVM intrinsics like `llvm.sqrt.f32`:

- Interpreter: dynamic FFI calls `sqrtf()` from libc — correct
- LLVM backend: emits `call @__sqrtf(...)` — goes through PLT, not hardware

The fix must respect the NO MAGIC rule: no hardcoded Rust string matches mapping
`__sqrtf` → `llvm.sqrt.f32`.

## Solution: `as intrinsic "llvm.symbol.name"`

```briev
frgn sqrt_f32(x: Float) -> Float as intrinsic "llvm.sqrt.f32" ;
```

The `as intrinsic "..."` clause explicitly declares the LLVM symbol name. The backend
emits it verbatim at both declare and call sites. No prefix matching, no whitelist,
no per-function magic — the programmer writes the exact LLVM intrinsic name.

## Changes

| # | File | Change | Lines |
|---|------|--------|-------|
| 1 | `src/ast.rs` | Add `intrinsic_name: Option<String>` to `ForeignSignature` | +1 |
| 2 | `src/ffi/validator.rs` | Add `intrinsic_name: None` to 2 construction sites | +2 |
| 3 | `src/typechecker.rs` | Add `intrinsic_name: None` to 1 construction site | +1 |
| 4 | `src/hardware_validator.rs` | Add `intrinsic_name: None` to 1 construction site | +1 |
| 5 | `src/parser.rs` | Parse `as intrinsic "..."` clause after `from` | +25 |
| 6 | `src/backend/llvm/mod.rs` | Use `sig.intrinsic_name` in declare loop | +1 changed line |
| 7 | `src/backend/llvm/emit_expr.rs` | Use `intrinsic_name` in call emission | +3 changed lines |
| 8 | `lib/std/llvm.bv` | New file — standard library for LLVM intrinsics | ~15 |
| 9 | `benchmarks/nbody_sqrt.bv` | Import `sqrt_f32` from `std/llvm` | ~2 changed lines |
| — | Tests | Parser + LLVM backend tests for intrinsic handling | ~60 |

## Design Decisions

### `as intrinsic` vs `from` vs pragma

- `from "..."` = TOML profile path for interpreter registry (existing meaning)
- `as intrinsic "..."` = verbatim LLVM symbol name for backend emission (new)
- Both can coexist: `frgn foo(x: Float) -> Float from "std::f64::sqrt" as intrinsic "llvm.sqrt.f32"`
- The `as intrinsic` signal is visually distinct and tells the programmer "this is compiler-level"

### Backend handling

- The string is emitted verbatim — no `@` prefix needed, no `starts_with("llvm.")` check
- The `declare` and `call` instructions use `intrinsic_name` if present, `name` otherwise
- All other backends (interpreter, C, WASM, VHDL, etc.) ignore `intrinsic_name`
- The interpreter resolves calls via `from` (TOML path) as normal

### Existing hardcoded declares

The LLVM backend's `emit_declares()` (emit_toplevel.rs:13-24) hardcodes declarations for
`@llvm.assume`, `@llvm.ctpop.i64`, etc. These should eventually move to `std/llvm.bv`
as `as intrinsic` declarations. Deferred to keep this change minimal.

## Open Items

- `references_triggers_or_ffi` in `transition_graph.rs` flags ALL `Expr::Call` as FFI,
  including intrinsic calls. This means intrinsic calls in hot loop bodies prevent
  pure-counter folding. Not addressed here — separate analysis fix needed.
