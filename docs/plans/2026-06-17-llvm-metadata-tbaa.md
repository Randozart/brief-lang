# Optimization: Replace `llvm.assume` with Metadata + Add TBAA

**Date:** 2026-06-17
**Session:** Backend Optimization — LLVM Metadata for Better Alias Analysis

## Overview

Two interrelated optimizations to feed contract-proven information into
LLVM in a form the optimizer can use without the overhead of explicit
instructions.

---

## Work Item 1: Replace `@llvm.assume` with Instruction-Level Metadata

### Current State

`src/backend/llvm/emit_toplevel.rs:835` emits `call void @llvm.assume(i1 %cond)`
after every proven precondition. `@llvm.assume` is a real instruction that:

- Inflates LLVM's inline cost estimate (it counts as an instruction)
- Flows through the entire optimization pipeline as an instruction
- Can block early passes (GVN, LICM, SROA) if interleaved with their
  target instructions
- Is only consumed late (by `InferAlignmentPass` and a few others)

### Target State

Emit **instruction-level metadata** instead, matching the proven fact to
the cheapest LLVM annotation:

| Contract Pattern | Current | Target |
|---|---|---|
| `[x < N]` (bounded integer) | `@llvm.assume(i1 %cmp)` | `!range` on the `load`/`trunc`/`zext` that produces `x` |
| `[ptr != null]` | `@llvm.assume(i1 %nonnull)` | `!nonnull` metadata on the `load` of `ptr` |
| `[ptr + offset < end]` (bounded pointer) | `@llvm.assume(i1 %cmp)` | `dereferenceable(N)` parameter attribute **or** `!noundef` on pointer |
| `[x != 0]` (non-zero integer) | `@llvm.assume(i1 %nonzero)` | `icmp ne` + `assume` → replace with `!range` excluding 0 |

### Implementation Plan

1. **Add metadata emission helpers** in `impl LlvmBackend`:
   - `emit_range_metadata(reg, lo, hi)` — appends `, !range !{ i64 lo, i64 hi }`
   - `emit_nonnull_metadata(reg)` — appends `, !nonnull`
   - `emit_dereferenceable(reg, bytes)` — appends `, !dereferenceable !{ i64 bytes }`

2. **Remove `@llvm.assume` from `emit_precondition_check`** (line 835):
   - Instead of emitting `call void @llvm.assume(i1 %cond)`, inspect the
     precondition expression's structure:
     - `Expr::Lt(id, Int(N))` → `!range` on the identifier's load
     - `Expr::Ne(id, Int(0))` → `!range { 1, 0 }` on the identifier's load
     - `Expr::Ne(ptr, Expr::Null)` → `!nonnull` on the ptr load
   - For complex preconditions (conjunctions, compound guards), fall back
     to `@llvm.assume` — metadata wins are for simple patterns.

3. **Update `emit_stmt.rs` store/load helpers** to carry optional metadata
   parameters.

4. **Benchmark**: Run `fasta`, `fannkuch-redux`, `spectral-norm` before and
   after, comparing `opt -O3 -pass-remarks-missed=gvn` output.

### Risk

- `!range` that contradicts program behavior causes `poison`, not `UB`.
  Briev's contract checker already proves the guard, so this is safe.
- Removing `@llvm.assume` entirely before all patterns are covered could
  regress optimizations that depend on `assume`. Keep the fallback for
  unsupported patterns.

---

## Work Item 2: TBAA Metadata for `i64`-Boxed Types

### Current State

Briev stores all values in `%State` as `i64` (or `i8*` for strings).
LLVM's Type-Based Alias Analysis sees every field access as a generic
`i64` load/store. Without type distinctions, LLVM conservatively assumes
every store may alias every load.

### Why This Matters

LLVM's GVN, LICM, and load-store forwarding all depend on alias analysis.
When all fields are `i64`:

```
; Two different fields — but LLVM can't prove no-alias
store i64 %v1, i64* %field_a
%v2 = load i64, i64* %field_b   ; LLVM: may alias, reload
```

With TBAA metadata:

```
store i64 %v1, i64* %field_a, !tbaa !3   ; !3 = "Int"
%v2 = load i64, i64* %field_b, !tbaa !4  ; !4 = "String"
; LLVM: different TBAA trees → no alias, forward!
```

### Implementation Plan

1. **Define TBAA metadata tree** in `mod.rs`, emitted once per module:

   ```llvm
   !0 = !{!"Briev"}
   !1 = !{!"Int", !0}
   !2 = !{!"Bool", !0}
   !3 = !{!"Char", !0}
   !4 = !{!"String", !0}
   !5 = !{!"Float", !0}
   !6 = !{!"List", !0}
   !7 = !{!"HashMap", !0}
   ```

2. **Map `Type` → TBAA node index**:

   | Type | TBAA Tag |
   |------|----------|
   | `Int` / `UInt` | `!1` |
   | `Bool` | `!2` |
   | `Char` | `!3` |
   | `String` / `Data` | `!4` |
   | `Float` | `!5` |
   | `List<T>` | `!6` |
   | `HashMap` | `!7` |

3. **Annotate every `store` and `load`** in `emit_stmt.rs` with the
   corresponding `!tbaa` metadata, using the `TypedRegister` type or the
   `let_binding_types` map to determine the type tag.

4. **Annotate `GEP` results** in `emit_expr.rs` — when a `getelementptr`
   accesses a state field, the result's type determines the TBAA tag.
   Store/load through that pointer carries the tag.

5. **Handle `inttoptr`/`ptrtoint` casts**: When a typed value (String,
   List) is boxed to `i64` for storage, the TBAA tag should reflect the
   ORIGINAL type, not `Int`. This prevents LLVM from conflating a
   boxed-String with an actual integer field.

### Ordering

TBAA (Work Item 2) is **data-parallel** with the `llvm.assume` → metadata
migration (Work Item 1). They touch separate emission paths:

- Work Item 1: primarily `emit_toplevel.rs:emit_precondition_check`
- Work Item 2: primarily `emit_stmt.rs` load/store helpers, `mod.rs` metadata
  emission, `emit_expr.rs` field access

Both can be developed independently.

### Benchmarking Strategy

Run with `opt -O3 -pass-remarks-missed=gvn` before and after each change:

```
# Count GVN missed optimizations due to unknown alias
opt -O3 -pass-remarks=gvn program.ll -disable-output 2>&1 | grep "load.*may alias" | wc -l
```

After TBAA, this count should drop significantly for programs with
mixed-type state fields.

---

## Verification

1. `cargo test --lib` — all tests pass
2. Compile officina-cli, run `opt -O3 -verify-module` on the IR
3. Benchmarks: `bash benchmarks/build_and_bench.sh --runtime`
4. `kani` — no regression in proof harnesses
