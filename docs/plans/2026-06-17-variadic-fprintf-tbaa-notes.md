# Fix: Variadic `fprintf` Calling Syntax + Optimization Notes

**Date:** 2026-06-17
**Session:** Backend Correctness — LLVM IR Verification Compliance

## Bug: Variadic `fprintf` Missing `(ptr, ptr, ...)` Prototype

### Symptom

Three `fprintf` call sites in the LLVM backend omit the explicit variadic
function type `(ptr, ptr, ...)` from the `call` instruction:

```llvm
; BUG — mismatch with declare i32 @fprintf(ptr, ptr, ...)
call i32 @fprintf(ptr %out, ptr %fmt, i64 %val)
```

LLVM requires that the function type signature in a `call` instruction
match the declared type of the callee. The `declare` for `fprintf` is:

```llvm
declare i32 @fprintf(ptr, ptr, ...)
```

A `call i32 @fprintf(ptr, ptr, i64)` would be a type mismatch because
the call site describes the function as `i32 (ptr, ptr, i64)` while the
declaration describes it as `i32 (ptr, ptr, ...)`. The LLVM verifier
rejects this mismatch.

### Fix

Add `(ptr, ptr, ...)` to the call type:

```llvm
call i32 (ptr, ptr, ...) @fprintf(ptr %out, ptr %fmt, i64 %val)
```

### Affected Sites

| File | Line | Intrinsic | Current | Fix |
|------|------|-----------|---------|-----|
| `src/backend/llvm/loop_engine.rs` | 872 | `print_int` in `emit_post_print` | `call i32 @fprintf(...)` | Add `(ptr, ptr, ...)` |
| `src/backend/llvm/emit_expr.rs` | 1747 | `Intrinsic::PrintInt` | `call i32 @fprintf(...)` | Add `(ptr, ptr, ...)` |
| `src/backend/llvm/emit_expr.rs` | 1769 | `Intrinsic::PrintFloat` | `call i32 @fprintf(...)` | Add `(ptr, ptr, ...)` |

### Why These Haven't Triggered

These intrinsics (`PrintInt`, `PrintFloat`, `print_int` as hoisted
post-loop) are not used by officina-cli, which uses `println#` for all
output. The `println#` intrinsic's `fprintf` call (emit_expr.rs:546)
already uses the correct `(ptr, ptr, ...)` syntax. The bugs are latent
and would only surface when `__print_int` or `__print_float` intrinsics
are exercised.

---

## Optimization Notes (Documentation Only)

These findings from the external audit are worth recording but do not
require code changes in this session:

### 1. `llvm.assume` Can Block Optimizations

The compiler emits `@llvm.assume(i1 %cond)` for proven preconditions.
While `assume` feeds facts to the optimizer, it is a real instruction
that flows through IR and can:
- Inflate the inline cost estimate
- Block early GVN/LICM if interleaved with other instructions
- Prevent SROA from recognizing an alloca as dead

**Better approach**: Prefer **instruction-level metadata** over explicit
`assume` calls where possible:
- `!range` on integer loads/stores
- `!nonnull` on pointer loads
- `!align` on pointer parameters
- `dereferenceable` parameter attributes
- `!alias.scope` / `!noalias` for proven non-aliasing

Metadata is naturally discarded when unused; `assume` lingers.

### 2. TBAA Metadata for `i64`-Boxed Types

Brief's `i64`-centric model (all values boxed to i64 for storage)
disables LLVM's Type-Based Alias Analysis (TBAA), because LLVM sees
all fields as `i64` and assumes they may alias.

**Fix**: Generate `!tbaa` metadata on load/store instructions that
reconstructs the original type boundaries. This would let LLVM prove
that `struct_a.x` and `struct_b.y` cannot alias even though both are
`i64` at the IR level.

Not a quick fix — requires a TBAA metadata tree and per-field
annotation in every `emit_store`/`emit_load` path.

### 3. Graceful Degradation for Unprovable Contracts

The philosophy document states "you do not drop to unsafe — you prove."
But there will always be valid invariants the compiler cannot prove
at compile time. The document should specify:
- **Compile-time rejection** when contracts are unprovable (strict mode)
- **Runtime assertion fallback** when contracts are unprovable (dev mode)
- **No fallback** — the optimizer assumes the contract holds, risky

This is a design policy decision, not a code change.
