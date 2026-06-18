# Optimization Remarks

**Date added:** 2026-06-18
**Phase:** 3 (planned)

---

## Purpose

Speculative directives (`#?`) produce structured diagnostic feedback —
**optimization remarks** — that explain the compiler's decision in
actionable terms. Remarks teach the developer about hardware constraints
without requiring them to read assembly or LLVM IR.

---

## Output Format

```
remark: #?vectorize on line 42 did not vectorize
  analysis:
    - loop-carried dependency: data[i] -> data[i-1] at line 44
    - LLVM cannot safely execute iterations in parallel
  help:
    - Restructure to remove the backward data dependency
    - Use #vectorize (imperative) to force with runtime checks
```

---

## Remark Types

| Directive | Decision | Example remark |
|-----------|----------|----------------|
| `#?inline` | Applied | `inlined: function size 14 ≤ threshold 25` |
| `#?inline` | Skipped | `not inlined: call graph has cycles` |
| `#?unroll` | Applied | `unrolled: factor 4, trip count 16` |
| `#?unroll` | Skipped | `not unrolled: trip count 3 below minimum 8` |
| `#?vectorize` | Applied | `vectorized: 8-lane SIMD fadd` |
| `#?vectorize` | Failed | `not vectorized: loop-carried dependency at line 44` |
| `#?gpu` | Applied | `offloaded to GPU: kernel block size 256` |
| `#?gpu` | Skipped | `CPU retained: arithmetic intensity 1.2 ops/byte < 8.0` |

---

## CLI Control

| Flag | Effect |
|------|--------|
| `--remarks` | Emit optimization remarks for all `#?` directives |
| `--verbose` / `-v` | Implies `--remarks` |
| Default | No remarks (zero overhead) |

---

## Implementation Plan

1. Define `OptimizationRemark` struct in `src/backend/llvm/remark.rs`
2. Store `remarks: Vec<OptimizationRemark>` on `LlvmBackend`
3. Emit remarks at each directive resolution site
4. Print remarks after compilation when `--remarks` is set

## Concat Memory Leak Fix — Tagged Pointer Approach (2026-06-18)

The `s1 + s2` string concatenation previously leaked both operand buffers.
The fix uses a tagged-pointer scheme to distinguish heap-allocated strings
from static string constants:

### Tagging

- **Static strings** (`@str.N` globals): tagged with bit 0 = 1
  (`or i64 %ptr, 1`) at the `Expr::String` emission site in
  `emit_expr.rs:31-33`.
- **Heap strings** (`malloc` results): bit 0 naturally clear due to 8-byte
  alignment.

### Freedom

In `emit_inline_concat`:
1. Bit 0 is masked off (`and i64, -2`) before reading string headers.
2. After copying both operands' data into the new buffer, bit 0 is tested.
3. If clear → heap → emit `free`.
4. If set → static constant → skip free.

This is safe because `malloc` guarantees at least 8-byte alignment on all
platforms Brief targets, so bit 0 is always usable as a tag.

### Files
- `emit_expr.rs` — `Expr::String` handler (tagging) and `emit_inline_concat`
  (mask + conditional free).
