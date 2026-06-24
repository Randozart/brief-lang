# Arrow (`<-`) — Collection Mutation Syntax

**Date:** 2026-06-24
**Status:** Fully implemented in interpreter; LLVM backend is a stub

## Overview

The arrow operator `<-` provides concise mutation syntax for collections. It dispatches on the value type to perform the appropriate operation: push, pop, discard, or transfer elements.

## Operations

### Push (`&collection <- value`)

Appends a value to the collection. Dispatch by type:

| Type | Behavior |
|------|----------|
| `List<T>` | Append to end (or insert at index with `&list[idx] <- val`) |
| `HashMap<K,V>` | Insert key-value pair: `&map <- ("key", value)` |
| `HashSet<T>` | Insert element: `&set <- "element"` |
| `Stack<T>` | Push onto top |
| `Queue<T>` | Enqueue to back |

```brief
&items <- 42;
&map <- ("name", "alice");
&set <- "tag";
&stack <- 100;
&queue <- 7;
```

### Pop (`let val = &collection <-`)

Removes and returns an element. Dispatch by type:

| Type | Behavior |
|------|----------|
| `List<T>` | Pop from end (or at index with `list[idx]`) |
| `HashMap<K,V>` | Remove by key: `&map <- "key"` |
| `HashSet<T>` | Remove and return element |
| `Stack<T>` | Pop from top |
| `Queue<T>` | Dequeue from front |

```brief
let last = &items <- ;        // pop list
let val = &map <- "name";     // remove key from map
let top = &stack <- ;         // pop stack
let front = &queue <- ;       // dequeue queue
```

### Discard (`&collection <- index ! `)

Removes an element without returning it:

```brief
&list <- 0 !;           // discard index 0
&map <- "key" !;        // discard entry by key
&set <- "elem" !;       // discard element
&stack <- !;            // discard top
```

### Transfer (`&dest <- &source`)

Moves all elements from source to destination:

```brief
&dest <- &source;         // transfer all
&dest <- &source { FILTER(active); };  // transfer with filter
```

The transfer with filter uses a subtype projection (`<:` syntax) as the filter — only matching elements are moved.

## Insert/Extract Strategies

The `TypeUniverse` can configure custom strategies:

```brief
// Prepend strategy (compile-time)
// type_universe defines InsertStrategy for this type

// Sorted insert strategy
// type_universe defines InsertStrategy::Sorted for ordered types
```

When configured, arrow operations respect the strategy instead of the default behavior.

## Backend Status

| Backend | Status |
|---------|--------|
| Interpreter | ✅ Full evaluation for all collection types |
| LLVM | ⚠️ Stub — emits `%arr: Void` |
| Webstack | ✅ ArrowMut, ArrowDiscard, ArrowTransfer return `JsValue::TRUE` |

## Related

- `examples/arrow-mutation.bv` — Complete demo of all arrow operations
- `docs/architecture/features/collection.md` — Collection types and type dispatch
