# RingBuffer Bracket Initialization — `[0]` for `RingBuffer<T>`

**Date**: 2026-07-01
**Status**: Implemented
**Feature**: Bracket syntax `[elem, ...]` produces RingBuf heap layout instead of List layout when the target type is `RingBuffer<T>`.

## Problem

`let queue: RingBuffer<Int> = [0]` fails at two levels:

1. **Type checker** rejects the assignment because `Applied("RingBuffer", [Int])` ≠ `Applied("List", [Int])` at strict name equality.
2. **LLVM codegen** `emit_list_or_tuple_body` always produces the 3-slot List layout (data_ptr, length, elements), never the 4-slot RingBuf layout (data_ptr, head, tail, mask).

## Root Cause

The bracket syntax `[elem, ...]` is hardcoded to produce `List<T>` at every level:
- **Parser**: `[e]` → `Expr::ListLiteral`
- **Type checker**: `Expr::ListLiteral` → `Type::Applied("List", [elem_type])`
- **Codegen**: `emit_list_or_tuple_body` → `malloc((n+2)*8)` with List header

No mechanism exists for TypeDefs to redefine what bracket syntax produces.

## Solution

### Part 1: Type Checker — TypeDef Inheritance in `types_compatible`

**File**: `src/typechecker.rs:3314-3316`

Add an OR branch to the `(Applied(an, _), Applied(bn, _))` match arm: when `an != bn`, look up `an` in the type universe. If `an`'s resolved base type equals `bn`, accept the assignment (with type parameter matching).

This makes `RingBuffer<Int>` ← `List<Int>` valid because `RingBuffer : List`.

### Part 2: Codegen — `emit_ringbuf_init` Helper

**File**: `src/backend/llvm/emit_toplevel.rs` (new function + callsites)

Add a helper `emit_ringbuf_init()` that emits LLVM IR for a RingBuf heap allocation, then call it from both `emit_init_state` and `emit_inline_init_stores` BEFORE the generic `emit_expr` fallthrough.

The helper:
1. `malloc(4 * 8)` for RingBuf struct (data_ptr, head, tail, mask)
2. `malloc(capacity * 8)` for element buffer, where capacity = `next_power_of_2(max(4, n))`
3. Store each literal element into buffer slots via `emit_expr`
4. Write struct fields: data_ptr = ptrtoint(buffer), head = 0, tail = n, mask = capacity - 1
5. Store ptrtoint(struct) to the state field via the GEP

Detection: At both callsites, before `Some(expr) => { self.emit_expr(...) }`, add:
```rust
Some(Expr::ListLiteral(items)) => {
    let briv_ty = &self.ctx.field_briv_types[field_idx];
    if let Type::Applied(type_name, _) = briv_ty {
        if let Some(rt) = self.ctx.type_universe.get(type_name) {
            if rt.insert_at.as_deref() == Some("ring_push") {
                emit_ringbuf_init(self, out, &items, field_idx, indent);
                continue;
            }
        }
    }
    // fall through to generic emit_expr
}
```

### Part 3: queue_drain Benchmark

**File**: `benchmarks/queue_drain.bv`

Change `let queue: List<Int> = [0]` to `let queue: RingBuffer<Int> = [0]` and add the import `import "std/core/ring_buffer.bv"`.

## RingBuf Struct Layout

```
RingBuf (32 bytes, 4 × i64 slots):
  offset 0: data_ptr (i64)  — ptrtoint of element buffer
  offset 1: head    (i64)  — read index (next element to pop)
  offset 2: tail    (i64)  — write index (next empty slot)
  offset 3: mask    (i64)  — capacity - 1 (power of 2)
```

This matches the layout expected by `RingPush`/`RingPop` in `intrinsics.rs`.

## Capacity Calculation

For `[e0, e1, ..., e_{n-1}]`:
- raw_capacity = max(4, n)
- capacity = next power of 2 ≥ raw_capacity
- mask = capacity - 1
- tail = n (items are pre-loaded, starting from head=0)

Minimum capacity is 4 (mask ≥ 3) because RingBuf operations require at least one empty slot to distinguish full from empty.

## Files Changed

| File | Lines | Change |
|------|-------|--------|
| `docs/plans/2026-07-01-ringbuffer-bracket-init.md` | all | This plan document |
| `src/typechecker.rs` | ~3314-3316 | Add TypeDef inheritance check |
| `src/backend/llvm/emit_toplevel.rs` | ~596, ~798 | Add ListLiteral match arm in both init functions |
| `src/backend/llvm/emit_toplevel.rs` | new | `emit_ringbuf_init` helper function |
| `lib/std/core/ring_buffer.bv` | ~13-16 | Verify strategy names (no `#` suffix) |
| `benchmarks/queue_drain.bv` | ~22-24 | Use `RingBuffer<Int>` + import |
| `benchmarks/test_ring.bv` | deleted | Replaced by queue_drain |

## Verification

1. `cargo test --lib` — all 1363 tests pass
2. `cargo build` — no warnings
3. Compile `queue_drain.bv` with RingBuffer, inspect IR for RingBuf init
4. Run `queue_drain` benchmark (optional, requires C reference)