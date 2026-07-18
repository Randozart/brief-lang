# Arrow Syntax (`<-`) and Arena Allocation

## Arrow Syntax

The `<-` operator provides push/pop/discard operations on collections:

| Pattern | Operation | AST |
|---------|-----------|-----|
| `queue <- value` | Push `value` into `queue` | `Statement::Assign(Ident("queue"), value)` |
| `&queue <- value` | Explicit push (same as above) | `Statement::Assign(AddrOf("queue"), value)` |
| `x <- &queue` | Pop from `queue` into `x` | `Statement::Assign(Ident("x"), AddrOf("queue"))` |
| `<- &queue` | Pop from `queue`, discard result | `Statement::Expression(AddrOf("queue"))` |

The LHS is always the target. For push, the target is a collection; for pop, the
target is a plain variable receiving the popped value. The `&` prefix on the RHS
marks the source as a collection to extract from.

### Type-Based Dispatch

The codegen uses `check_insert_strategy` / `check_extract_strategy` to determine
whether a variable is a collection type (RingBuffer, List, etc.):

- **RingBuffer with inline fields** — Direct `%State` GEP+load+store via the
  4 tracked indices (data, head, tail, mask). No `inttoptr` handle indirection.
- **Heap-allocated List** — Arena-backed allocation via `emit_arena_alloc()`,
  with `@malloc` fallthrough when the arena is exhausted.

### Implementation

- **AST**: `Expr::AddrOf(Box<Expr>)` in `src/ast/expr.rs`
- **Parser**: `Token::ArrowLeft` handling in `src/parser/statements.rs`
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
