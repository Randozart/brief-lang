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

## Work Item 2: Fix SSA Dispatch Selection for Trigger-Based Programs

### Current State
Officina declares `reactor @30Hz;` and `trg keypress: Char @stdin#;`, yet
the compiler selects SSA loop dispatch. The program spins in a busy loop
with no `epoll_wait`, wasting CPU.

### Target State
When a program has external triggers (`@stdin#`, `@linked`), the dispatch
should select wake/reactor mode, which enters `epoll_wait` between ticks.

### Investigation Needed
The dispatch selection logic is in `loop_engine.rs` or `mod.rs`. The flag
`has_wake_triggers` is computed somewhere — check if trigger detection
happens before or after the `emit_ssa_main` vs `emit_enum_main` decision.

## Verification
1. `cargo test --lib` — all tests pass
2. Officina boots, `<-` operations no longer return 0
3. Generated IR shows proper list mutation code
