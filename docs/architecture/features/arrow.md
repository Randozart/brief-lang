# Arrow (`<-`) — Collection Mutation & Consumption Syntax

**Date:** 2026-06-24
**Status:** Fully implemented in interpreter; LLVM backend is a stub

## Overview

The arrow operator `<-` provides concise mutation syntax for collections. It dispatches on the value type to perform the appropriate operation: push, pop, discard, or transfer elements.

## Operations

### Push (`collection <- value`)

Appends a value to the collection. Dispatch by type:

| Type | Behavior |
|------|----------|
| `List<T>` | Append to end (or insert at index with `&list[idx] <- val`) |
| `HashMap<K,V>` | Insert key-value pair: `map <- ("key", value)` |
| `HashSet<T>` | Insert element: `set <- "element"` |
| `Stack<T>` | Push onto top |
| `Queue<T>` | Enqueue to back |

```brief
items <- 42;
map <- ("name", "alice");
set <- "tag";
stack <- 100;
queue <- 7;
```

### Pop (`let val = collection <-`)

Removes and returns an element. Dispatch by type:

| Type | Behavior |
|------|----------|
| `List<T>` | Pop from end (or at index with `list[idx]`) |
| `HashMap<K,V>` | Remove by key: `map <- "key"` |
| `HashSet<T>` | Remove and return element |
| `Stack<T>` | Pop from top |
| `Queue<T>` | Dequeue from front |

```brief
let last = items <- ;    // pop list
let val = map <- "name";   // remove key from map
let top = stack <- ;     // pop stack
let front = queue <- ;    // dequeue queue
```

### Discard (`collection <- index ! `)

Removes an element without returning it:

```brief
list <- 0 !;      // discard index 0
map <- "key" !;    // discard entry by key
set <- "elem" !;    // discard element
stack <- !;      // discard top
```

### Transfer (`dest <- &source`)

Moves all elements from source to destination:

```brief
dest <- &source;     // transfer all (& on RHS = consumption)
dest <- &source { FILTER(active); }; // transfer with filter
```

The transfer with filter uses a subtype projection (`<:` syntax) as the filter — only matching elements are moved.

## Insert/Extract Strategies

The `TypeUniverse` can configure custom strategies via `type ... <: List { InsertAt = ... }`.
The strategy system resolves the binding string to an `InsertStrategy` or `ExtractStrategy`
variant:

```brief
// Built-in strategies:
type Fifo <: List { InsertAt = prepend; ExtractFrom = shift; };
type Mapped <: List { InsertAt = hash; };

// Custom function strategy:
type SkipList<T> <: List<T> {
  InsertAt = sl_insert;  // dispatches to sl_insert#(list, val)
  ExtractFrom = sl_remove; // dispatches to sl_remove#(list)
};
```

When a Custom strategy is configured, arrow operations dispatch to the named
function instead of using the default behavior:

- **Push**: calls `fn_name(collection, value)` → returns updated collection
- **Pop**: calls `fn_name(collection)` → returns `(popped_value, updated_collection)`

### Strategy resolution

| Strategy string | Result |
|----------------|--------|
| `append` | `InsertStrategy::Append` (default) |
| `prepend` | `InsertStrategy::Prepend` |
| `sorted` | `InsertStrategy::Sorted` |
| `hash` | `InsertStrategy::Hash` |
| anything else | `InsertStrategy::Custom(name)` |

Same for `ExtractFrom`: `pop`, `shift`, `head`, `tail`, `hash` are built-in;
anything else becomes `ExtractStrategy::Custom(name)`.

### Interpreter

`lookup_insert_strategy` uses the variable's declared type annotation
(`let_types`) to resolve the type name, then looks up the strategy in
the TypeUniverse. This fixes the variable-name vs type-name mismatch
that previously prevented strategies from firing in the interpreter.

### LLVM backend

- Push Custom: emits `call i64 @fn_name(i64 %list, i64 %elem)`
- Pop Custom: emits `call { i64, i64 } @fn_name(i64 %list)` then extracts results
- Shift strategy: pops from front (index 0) instead of end

## Backend Status

| Backend | Status |
|---------|--------|
| Interpreter | ✅ Full evaluation for all collection types + Custom strategy dispatch |
| LLVM | ✅ Custom strategy for Push/Pop, Shift strategy, check_extract_strategy |

## Related

- `docs/architecture/features/typedef.md` — InsertAt/ExtractFrom binding syntax
- `docs/architecture/features/inop.md` — Writing inops for Custom strategy dispatch
- `lib/std/skiplist.bv` — Full example: SkipList with Custom strategy
- `examples/inop-skiplist-dispatch.bv` — Demo of `<-` on SkipList
- `examples/arrow-mutation.bv` — Demo of all built-in arrow operations
