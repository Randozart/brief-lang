# Phase 6 Briv: SIMD Vector Alignment + Loop Metadata

**Date:** 2026-05-29  
**Spec Reference:** `09-SIMD.md`  
**Prerequisite:** Phase 2 (contract analysis for bounds)  
**Estimated Effort:** 1 day  

## Goal

Emit `!llvm.loop.vectorize.enable` metadata on loop branch instructions and correct alignment on array loads/stores. The backend does not auto-vectorize — it annotates so LLVM's vectorizer can.

## Deliverables

### 1. `!llvm.loop.vectorize.enable` on Loop Branches

When a `guarded` block's condition is a bounded range check (`[i < N]`) with a field assignment inside, annotate the back-edge branch:

```llvm
; Before:
;   br i1 %cond, label %loop, label %exit

; After:
  br i1 %cond, label %loop, label %exit, !llvm.loop !0
```

With metadata node at module footer:
```llvm
!0 = !{!0, !1, !2}
!1 = !{!"llvm.loop.vectorize.enable", i1 true}
!2 = !{!"llvm.loop.interleave.count", i32 4}
```

### 2. Alignment Metadata for Array Types

| Element Count | Vector Width | Alignment |
|--------------|--------------|-----------|
| 2-4 | `<4 x float>` / `<4 x i32>` | 16 |
| 5-8 | `<8 x float>` / `<8 x i32>` | 32 |
| 9-16 | `<16 x float>` / `<16 x i32>` | 64 |

This is emitted via proper `align N` on loads/stores in `Expr::ListLiteral` / `Expr::ListIndex`.

## Test Fixture

| Fixture | Tests |
|---------|-------|
| `loop_vectorize.bv` | `[i < 16] { &result[i] = input[i] * 2; &i = i + 1; }` |

## Acceptance Criteria

```bash
briv-compiler llvm tests/fixtures/phase6/loop_vectorize.bv --out /tmp/p6/
llc /tmp/p6/loop_vectorize.ll -o /dev/null  # Must succeed
grep "llvm.loop.vectorize.enable" /tmp/p6/loop_vectorize.ll  # Metadata present
grep "align" /tmp/p6/loop_vectorize.ll  # Alignment on loads/stores
```

## Implementation Checklist

- [ ] Emit `!llvm.loop.vectorize.enable` metadata on guarded block back-edges
- [ ] Add metadata nodes to module footer
- [ ] Compute alignment from array element count
- [ ] Regression: all 17 existing fixtures still pass