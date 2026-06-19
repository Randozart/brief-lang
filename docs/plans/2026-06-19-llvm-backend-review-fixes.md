# LLVM Backend Review Fixes

**Date**: 2026-06-19  
**Motivation**: External architectural review of the LLVM backend identified
several real issues. This plan addresses the three most impactful.

## P0 — String Concat Memory Leak

**File**: `src/backend/llvm/emit_expr.rs:3664-3732`

### Problem
`emit_inline_concat` allocates a new buffer via `malloc` for the concatenation
result but never frees the operand strings. The comment at lines 3721-3725
documents this as intentional because the existing bit-0 tag (static vs heap)
cannot distinguish state-aliased strings (don't free) from temporary concat
results (should free). For long-running reactor programs, each tick leaks
intermediate string allocations.

### Solution
Use **bit 1 as a "temporary" tag**:

| Source | Bit 0 | Bit 1 | Meaning |
|--------|-------|-------|---------|
| String constant | 1 | 0 | Static data, never free |
| State-loaded string | 0 | 0 | Heap-allocated, state-owned, don't free |
| Concat result | 0 | 1 | Heap-allocated, temporary, safe to free |

Changes:
1. Tag concat result with `or i64 %result, 2` (set bit 1)
2. Before copying operands, check bit 1 and emit conditional `@free` if set
3. Widen tag mask from `and i64 ..., -2` to `and i64 ..., -4` (mask both bits)
4. No change to state loads (already bit 1 = 0) or string constants (already bit 1 = 0)

## P1 — GPU Doc Comments vs Reality

**File**: `src/backend/llvm/gpu.rs:44-51`

### Problem
Doc comment claims criteria #2 ("No loop-carried dependencies — parallelizable")
and #3 ("Contiguous memory access patterns — coalesced reads/writes") are
checked by `check_eligibility`, but they are not implemented — only documented
as intent.

### Solution
Update the doc comment to match current implementation. List only the
checks that are actually performed, and note the missing ones as deferred work.

## P2 — Silent Cycle Fallback in Reorder

**File**: `src/backend/llvm/reorder.rs:205-213`

### Problem
When Kahn's topological sort detects a dependency cycle, the cycle fallback
silently appends unscheduled statements in original order. No warning or
diagnostic is emitted, potentially masking subtle mutable state aliasing bugs.

### Solution
1. Return a `bool` flag from `topological_sort` indicating cycle detection
2. Propagate through `reorder_body_statements` as a return tuple
3. At the two call sites in `emit_toplevel.rs:746,776`, push a warning to
   `self.warnings` when a cycle is detected
4. Update existing tests for the new return type

## Commit Order

1. P0 (concat leak) + tests + architecture doc update
2. P1 (GPU docs) + architecture doc update
3. P2 (cycle warning) + tests update + architecture doc update
