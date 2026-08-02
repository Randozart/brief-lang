# Arrow Syntax (`<-`) and Arena Allocation

## Arrow Syntax

The `<-` operator provides push/pop/discard operations on collections. The
`&` fake-pointer marker is REMOVED (2026-08-01, Phase 3) — the dispatch finds
the collection by the op binding on each side, and the arrow is the unified
`Statement::ArrowAssign { target, value, consume }`:

| Pattern | Operation | AST |
|---------|-----------|-----|
| `queue <- value` | Insert (push) into `queue` | `ArrowAssign { target: Some(queue), value, consume: false }` |
| `dest <- queue` | Read (copy) an element out | `ArrowAssign { target: Some(dest), value: queue, consume: false }` |
| `dest ~<- queue` | Destructive extract | `ArrowAssign { target: Some(dest), value: queue, consume: true }` |
| `<- queue` | Pop, discard result | `ArrowAssign { target: None, value: queue, consume: false }` |
| `~<- queue` | Pop, discard + destroy | `ArrowAssign { target: None, value: queue, consume: true }` |

The target is the collection for an insert (InsertAt binding on the lhs); the
value is the collection for a read/extract (ExtractFrom/CopyFrom binding on the
rhs). The element/return types are generic-substituted (`List<Int>` push's `T`
→ `Int`).

### Type-Based Dispatch

The codegen uses `find_insert_strategy` / `find_extract_strategy` to determine
whether a variable is a collection type (RingBuffer, List, etc.):

- **RingBuffer with inline fields** — Direct `%State` GEP+load+store via the
  4 tracked indices (data, head, tail, mask). No `inttoptr` handle indirection.
- **Heap-allocated List** — Arena-backed allocation via `emit_arena_alloc()`,
  with `@malloc` fallthrough when the arena is exhausted.

### Implementation

- **AST**: `Statement::ArrowAssign` in `src/ast/top.rs`; `Expr::Consume` in
  `src/ast/expr.rs` (the destructive marker).
- **Parser**: `Token::ArrowLeft` / `Token::TildeArrowLeft` handling in
  `src/parser/statements.rs`.
- **Codegen**: Strategy dispatch via property values (`#L`, `#R`, `#T`) in
  `src/backend/llvm/emit_stmt.rs`. Emits `call @strategy_fn(handle, value)`
  which LLVM -O3 inlines. See `docs/architecture/hash-words.md`.
- **Field metadata**: `RingbufInlineFields` in `src/backend/llvm/context.rs`
- **Inline field registration**: `mod.rs:3206-3239` — 4 extra %State fields
  for RingBuffer variables (data, head, tail, mask)

## Arena Allocation

The arena allocator provides bump allocation within a transaction tick,
eliminating per-operation free calls:

| Function | File | Purpose |
|----------|------|---------|
| `emit_arena_init` | `mod.rs:1181` | Allocate 64KB scratch buffer |
| `emit_arena_alloc` | `mod.rs:1116` | Bump allocate (fallthrough to malloc) |
| `emit_arena_reset` | `mod.rs:1204` | Rewind bump pointer to base |
| `emit_arena_fini` | `mod.rs:1217` | Free arena, set slots to None |
| `arena_slots` | `context.rs:310` | `Option<(ptr, end, base)>` |

The arena is initialized before each tick body and finalised after. List<T>
push/pop operations use `emit_arena_alloc` for allocation, avoiding per-op
`@free` when the arena is active (the arena owns all memory).
