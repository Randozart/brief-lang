# Remaining Work: Arrow Operator + SSA Dispatch Selection

**Date:** 2026-06-17
**Session:** Backend Completeness — Arrow Operator + Dispatch Selection

## Work Item 1: Implement `<-` Arrow Operator for List<T> (Push)

### Current State
The LLVM backend emits `i64 0` for all arrow mutations:
```
LLVM backend stub: arrow operator (collect/discard/transfer) returns 0
```

Every `&history <- rec`, `&result <- item`, `&dst <- src[i]` silently drops
the item. Officina's history, windows, and rule updates are all non-functional.

### Target State
`<-` on a `List<T>` reads the list header, extends the data buffer (via
`realloc`), stores the new element, and commits the updated header back
to the state field.

List memory layout:
```
List header (i64*):
  [0]: data_ptr — pointer past header to element area
  [1]: len — number of elements
  [2..]: elements stored as i64
```

On `&list <- value`:
1. Load header `{ data_ptr, len }` from state field
2. Compute new size: `(len + 1) * 8`
3. Call `realloc(data_ptr - 16, new_size + 16)` to extend
4. Store value at `data_ptr[len]`
5. Write back `{ new_data_ptr, len + 1 }`

### Scope
- **Push only** (`ArrowDir::Push`) for now
- **List<T> only** (most common, covers officina's usage)
- Pop, Discard, Transfer, HashMap/HashSet/Stack/Queue deferred

## Work Item 2: SSA Dispatch Selection (Investigated — No Fix Needed)

### Finding
The dispatch selection in `mod.rs:1501-1515` correctly identifies that
programs with non-enumerable wake triggers (like `@stdin#`) should use
the SSA loop with `__rt_wait()`. The path at line 1515 passes
`has_wake_triggers = true` to `emit_ssa_main`, which emits
`call void @__rt_wait()` between ticks.

The `__rt_wait()` call is verified present in the generated officina.ll.
The program blocks on stdin rather than spinning — correct behavior.

The "warning: program has wake triggers but no exit path" is about the
missing `#!exit <condition>;` pragma, not about dispatch mode. Adding an
exit condition is a user-facing feature (Ctrl+C handler), not a compiler bug.

### No Fix Needed
This item is resolved — the compiler makes the correct dispatch choice.

## Verification
1. `cargo test --lib` — all tests pass
2. Officina boots, `<-` operations no longer return 0
3. Generated IR shows proper list mutation code
4. `__rt_wait()` present in generated officina.ll
