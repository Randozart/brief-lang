# Casting Protocol — Type Conversion via Hashword Categories

**Date:** 2026-07-20
**Status:** Foundational
**Applies to:** TypeUniverse, normalizer, all backends, typechecker

---

## Overview

Brief has no built-in `as` cast operator. Type conversion is handled by the
protocol system: every hashword category defines required `Cast` ops, and
the compiler finds the shortest path through the protocol graph at compile time.

Types never "belong to" categories. They *interact with* categories by
declaring `op Cast(#Category)` in their signature. The presence or absence
of these declarations determines what conversions are available.

---

## The Protocol Graph

Every `Cast(#Category)` declaration is a directed edge. The compiler finds
the shortest path from source to target at compile time.

### Root: `#Bits`

`Cast(#Bits)` is **implicit on every type**. Because `Bits` is the implicit
base of all types (the Bits thesis), every type can reinterpret itself as
raw bytes. This guarantees that the protocol graph is always connected.

```
SourceType --[implicit Cast(#Bits)]--> #Bits --[implicit Cast(TargetType)]--> TargetType
```

The implicit `Cast(#Bits)` produces raw bytes of the source type's width.
The implicit `Cast(TargetType) from #Bits` constructs the target type from
raw bytes. These are the "last resort" path — always correct, always available.

### Edges from declarations

```brief
type Float {
    data: Bits<32>;
    op Cast(#Int) = float_to_int(#L);   // edge: Float → #Int
    op Cast(#Bits) = float_to_bits(#L); // edge: Float → #Bits (explicit override)
};

type ASCIIString <: String {
    op Cast(#String) = ascii_to_utf8(#L); // edge: ASCIIString → #String
};
```

Each `op Cast(#Category)` adds an edge from the declaring type to the category.

---

## Protocol Shapes (Backend Contract)

Every hashword category has a well-defined protocol shape — a concrete
representation that ALL backends must be able to work with:

| Hashword | Protocol shape | Required Cast ops | Backend contract |
|---|---|---|---|
| `#Int` | `i64` | None (intrinsic) | Add, Sub, Mul, Div, And, Or, Xor, Not, Shl, Shr on i64 |
| `#Float` | IEEE 754 binary32/64 | `Cast(Float64)` | Add, Sub, Mul, Div, Sqrt, FMA on IEEE 754 |
| `#Bool` | `i1` (stored as i8) | None (intrinsic) | And, Or, Not |
| `#Char` | Unicode scalar (`i32`) | `Cast(#Int)` | Eq, Lt; code point 0x00–0x10FFFF |
| `#String` | UTF-8 byte sequence | `Cast(#Bits)`, `Extract(#Char)`, `InsertAt(#Char)`, `:> Size`, `Concat(#String)` | UTF-8 encoded text |
| `#Bits` | Raw `iN` (width varies) | None (base) | And, Or, Xor, Not, Shl, Shr |

The protocol shape is the **universal currency** for that category. Any backend
that claims to understand `#String` must be able to produce/consume UTF-8 at
the protocol boundary. An optimizing backend may store strings internally as
Latin-1, but must translate to UTF-8 at every `#String` protocol op.

---

## Conversion Path Resolution

When the compiler encounters a cross-type operation (e.g., `ascii_str + string`),
it resolves the conversion path:

1. **Exact match**: Type defines `op Add(String)` → use it.
2. **Direct cast**: Type A defines `op Cast(#Category)` → use it once.
3. **Protocol chain**: Find shortest path through the protocol graph.
4. **Implicit fallback**: `#Bits → #Bits` (raw bytes).

### BFS Search

```
Input:  source_type, target_category
Output: sequence of Cast ops, or error

1. If source_type has op Cast(target_category): return [source_type → target_category]
2. BFS over the protocol graph from source_type to target_category:
   - Each node is a category (or concrete type)
   - Each edge is a Cast(Category) declaration
   - #Bits is always reachable from every type
3. If path found: return sequence of Cast ops
4. If no path: compiler error with available alternatives
```

### Example: `Posit32 → #Float`

```brief
type Posit32 {
    data: Bits<32>;
    op Cast(#Bits) = posit_to_bits(#L);       // Posit32 → Bits<32>
    // No direct Cast(#Float)
};
```

Protocol path: `Posit32 → #Bits → #Float`

1. `Posit32 :> Cast(#Bits)` → raw 32-bit integer
2. `#Bits :> Cast(#Float)` → the backend's i32-to-float conversion

Step 2 is backend-intrinsic — since `#Float` is a hashword category, the
backend knows how to construct one from raw bits. If the programmer wants
a better path, they declare `op Cast(#Float) = posit_to_float(#L)`.

### Example: `ASCIIString → #String` (explicit)

```brief
type ASCIIString {
    data: Bits<64>;  // pointer
    len: Bits<64>;
    op Cast(#String) = ascii_to_utf8(#L);  // direct edge
};
```

Protocol path: `ASCIIString → #String` (1 hop — optimal).

---

## Protocol Ops (Required Per Category)

Each hashword category defines a set of ops that ALL types interacting with
that category must implement. These form the "common language" for conversions.

### `#String` protocol

```
op Extract(#Char) = fn(#L, #R);     // extract char at index
op InsertAt(#Char) = fn(#L, #R);    // insert char at index
op Concat(#String) = fn(#L, #R);    // append another string-type
:> Size                              // get length in characters
Cast(#Bits)                          // raw bytes
```

These ops let ANY two `#String` types communicate via `Char` — the universal
text currency. A conversion function between two opaque string representations
uses `Extract(#Char)` and `InsertAt(#Char)` without knowing the encoding.

### `#Float` protocol

```
Add(#Float)
Mul(#Float)
Sub(#Float)
Div(#Float)
Sqrt(#Float)
Cast(Float64)     // IEEE 754 double — universal currency
Cast(#Bits)       // raw bits
```

`Float64` is the universal float currency. A Posit32 backend converts to
`Float64` at the protocol boundary. An optimizer that can keep Posit32 in
its native format eliminates the conversion.

### `#Int` protocol

```
Add(#Int)
Sub(#Int)
Mul(#Int)
Div(#Int)
And(#Bits)
Or(#Bits)
Xor(#Bits)
Not(#Bits)
Shl(#Bits)
Shr(#Bits)
```

Int ops are backend-intrinsic — every backend knows how to add integers.
The universal currency is `#Bits` (the raw integer representation).

---

## Type Parameter Constraints

```brief
type HashMap<K: #String, V> {
    data: Bits<64>;
    len: Bits<64>;
    cap: Bits<64>;
    op Insert(V) = hashmap_insert(#L, #R);
    op Get(K) -> V = hashmap_get(#L, #R);
};
```

`K: #String` is a **protocol satisfaction check**, not a category membership
test. The compiler verifies: does `K` implement the `#String` protocol ops
(`Extract(#Char)`, `InsertAt(#Char)`, `Concat(#String)`, `:> Size`, `Cast(#Bits)`)?
If yes, `K` satisfies `#String`. The concrete type of `K` can be `String`,
`ASCIIString`, `Utf8View`, or any type that implements the required ops.

---

## Implementation Phases

### Phase 1: Protocol Graph Skeleton

- Define `Cast(#Category)` as a valid op signature
- Add implicit `Cast(#Bits)` to every type (compiler-generated)
- Build BFS path resolution in the typechecker

### Phase 2: Protocol Shape Validation

- Each category declares its protocol shape
- A type that declares `op Add(#Float)` must also implement `Cast(Float64)`
- Error: "Type X implements op Add(#Float) but not Cast(Float64) — required by #Float protocol"

### Phase 3: Backend Protocol Handlers

- LLVM backend maps `#String` protocol ops to LLVM IR
- SPIR-V backend maps them to SPIR-V instructions
- Each backend decides its internal representation; protocol ops are the bridge
