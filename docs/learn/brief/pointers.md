# Pointers in Brief

Brief has a unified `&` operator that always means **address-of**. When you
write `&x` you get a pointer (`Ptr<T>` or `PtrConst<T>`) to `x`.

## The short rule for state field writes

```brief
field = value;      // direct state field write — use this
&field = value;     // writes through a transient pointer — redundant
```

The `&` is unnecessary for direct state field writes. It existed because the
legacy `&` was a syntactic marker (`OwnedRef`), not a real pointer. Now that
`&` is a proper address-of operator, `&field = value` means:

```
*(&field) = value   // create a transient Ptr<T>, store through it
```

This is semantically correct but unnecessarily verbose. Write directly instead.

## When `&` IS meaningful

```brief
let p = &field;     // creates a persistent Ptr<T> — the pointer lives
*p = value;         // writes through p — genuine pointer store
let v = *p;         // reads through p — genuine pointer load
```

For collection arrow operations, `&` on the RHS of `<-` marks **consumption**
(take elements, don't copy). On the LHS of `<-`, `&` is still required by the
parser for now (it's a syntactic requirement, not a semantic one):

```brief
&list <- value;     // push — parser requires & on LHS
value <- &list;     // pop — & on RHS means consumption (remove from list)
value <- list;      // peek — no & means no consumption (future)
<- &list;           // drain — & on target means consumption
```

## Dereferencing

`*ptr` reads or writes the value that `ptr` points to:

```brief
let p = &counter;   // p: Ptr<Int>
let v = *p;         // v: Int (reads counter)
*p = v + 1;         // writes through p (updates counter)
```

A dereference of an address-of `*(&counter)` is a no-op — it takes the
address of `counter` then immediately reads through it. Use `counter`
directly.

## Why both forms exist

They go through different codegen paths:

| Form | AST node | Codegen path |
|------|----------|-------------|
| `field = value` | `Identifier("field")` | `emit_memory_field_store` (original) |
| `&field = value` | `AddrOf(Identifier("field"))` | `emit_typed_store` (new) |

Both produce correct code. The bare `field = value` path is the simpler,
original path used for the benchmark baseline.

## History

- **Before Ptr Level 3 (pre-2026-07-09)**: `&` was parsed as `Expr::OwnedRef`,
  a syntactic marker with no type-level effect. `&field = value` and
  `field = value` both produced the same AST.
- **Phase 0 (2026-07-09)**: `OwnedRef` removed. `&` now produces
  `Expr::AddrOf(Box<Expr>)` with type `Ptr<T>` or `PtrConst<T>`.
- **Phase 1-2**: Full pipeline support: typechecker infers `Ptr<T>`,
  interpreter wraps in `Value::Ref`, LLVM codegen emits GEP+store.
- **Post-cleanup (2026-07-10)**: `&` removed from simple state field writes
  in all benchmarks and examples. Kept only where pointer semantics are
  actually used (dereference, persistent pointer creation, type annotations).
