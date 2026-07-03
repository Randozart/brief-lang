# Types in Brief — Learning Guide

**Last updated:** 2026-06-26

## Type Derivation (`<:`)

Brief's type system is built on a small primitive kernel (~13 properties) that the compiler understands natively. Everything else is defined in user-space Brief.

The syntax for defining a type:

```brief
Type Name <: Base {
    Property = Value;
    [ Constraint ];
};
```

### Scalars

```brief
Type U8  <: Bits { Bytes = 1; Alignment = 1; };
Type U32 <: Bits { Bytes = 4; Alignment = 4; };
Type Int <: U64;
```

`Bits` is the only truly built-in type. `Bytes` and `Alignment` describe physical layout.

### Collections

Collections are defined with element type and access pattern metadata:

```brief
Type List<T> <: Bits {
    ElementType = T;
    FixedSize = false;
    InsertAt = :> Size;
    ExtractFrom = :> Size - 1;
};
```

`ElementType = T` unlocks `[]` brackets. `FixedSize = false` unlocks `<-`/`->` arrows.

### Tuples

Tuples are fixed-size heterogeneous collections. Unlike `List<T>` (which holds zero or more elements of a single type), a Tuple's length and element types are part of its type signature:

```brief
defn pair() -> (Int, String) {
    term (42, "hello");
};
```

### Bracket Indexing

Tuples support the same `[index]` bracket syntax as Lists:

```brief
let t = (10, 20, 30);
let x = t[1];   // 20
let y = t[0];   // 10
```

Indices are zero-based and bounds-checked at runtime. Use bracket syntax (`pair[0]`) instead of the deprecated `:> 0` projection.

### Memory Layout

In the LLVM backend, Tuples share the same memory layout as Lists: `[data_ptr, len, elem0, elem1, ...]`. This means all existing GEP-based indexing code works for both List and Tuple without modification.

## Syntax Gates

Override to restrict access:

```brief
Type Stack<T> <: List<T> { AllowIndex = false; };
Type Queue<T> <: List<T> { ExtractFrom = 0; AllowIndex = false; };
```

These say: "Stack is like List but you can't index into it." The compiler synthesizes the correct memory operations based on the metadata.

### Codecs

Codecs define how literals are translated to bytes at compile time:

```brief
import { Utf8 } from "std/utf8.bv";
Type String <: List<U8> { Codec = Utf8; };
```

When you write `"Hello"`, the compiler calls `Utf8::encode("Hello")` during compilation and embeds the result directly in the binary.

### Refinement Constraints

```brief
Type PositiveInt <: Int {
    [ > 0 ]
};
```

The implicit subject is `_` (the value itself). Constraints are validated against literals at compile time; runtime guards are synthesized for dynamic values.

### Metadata Queries (`:>`)

The projection operator `:>` extracts metadata from any value without mutation:

| Expression | Returns | Works on |
|---|---|---|
| `val :> Size` | `Int` — number of elements/bytes | List, Tuple, String, HashMap, HashSet |
| `val :> IsEmpty` | `Bool` — `true` if zero elements | List, Tuple, String, HashMap, HashSet |
| `val :> Type` | `Int` — type discriminant | Any value |
| `val :> Keys` | `List<K>` — all keys | HashMap |
| `val :> Values` | `List<V>` — all values | HashMap |
| `val :> Contains(k)` | `Bool` — key membership | HashMap, HashSet |

Tuple element access via `:> N` (integer index) is **deprecated** — use `val[N]` bracket syntax instead.

### InsertAt / ExtractFrom

These two properties define where elements go when pushing and where they come from when popping:

| Expression | Example use |
|---|---|
| `0` | Front (Queue pop) |
| `:> Size` | Append (push to end) |
| `:> Size - 1` | Last (Stack pop) |
| `<: { MAX(.k) }` | Max-heap ordered |
| `<: { MIN(.k) }` | Min-heap ordered |

#### Custom strategy dispatch

When `InsertAt` or `ExtractFrom` is set to a name that doesn't match any
built-in strategy string, the compiler treats it as a reference to a
user-defined function (typically an `inop` or `defn`). The `<-` arrow
operator dispatches to that function instead of using the default behavior:

```brief
type SkipList<T> <: List<T> {
    InsertAt = sl_insert;     // &sl <- val → sl_insert#(sl, val)
    ExtractFrom = sl_remove;  // val <- &sl → sl_remove#(sl)
};

inop sl_insert<T>(list: SkipList<T>, val: T) -> SkipList<T> {
    ... BILD (malloc/memcpy/free) ...
} fallback sl_append(list, val);
```

The function is resolved first as an `inop` (uses fallback for interpreter,
BILD for LLVM), then as a `defn` (executes body). The interpreter tracks
declared types via `let_types` so that `let sl: SkipList<Int> = []` correctly
maps `sl` to `SkipList<Int>` for strategy resolution.

Built-in strategy names:
- `InsertAt`: `append`, `prepend`, `sorted`, `hash`
- `ExtractFrom`: `pop`, `shift`, `head`, `tail`, `hash`

Any other string becomes `Custom(fn_name)`.

### How it works (Two-Pass Pipeline)

1. **Pass 1 (Type-Universe)**: The compiler collects all `Type` declarations, resolves derivation chains, inherits properties, and freezes the type map.
2. **Pass 2 (Executable)**: Uses the frozen type map for type checking, literal encoding, and code generation.

### The Brief philosophy

Most languages hardcode type rules inside the compiler's Rust/C++ source. Brief hardcodes about 13 properties. **That's it.** Everything — `String`, `Stack`, `Queue`, `HashMap`, even `Int` — is defined in Brief source files, using the same syntax you use to define your own types.

### Pointers (2026-07-03)

Brief has four forms of pointer, all sharing the same machine representation
(`i64`/`u64` in the backend):

| Form | Example | What it says |
|------|---------|--------------|
| Typed | `Ptr<Int>` | Points to an Int (full nominal type) |
| Bare | `Ptr` | Points to 8 bytes (safe void\*) |
| Fixed | `Ptr32` / `Ptr64` / `Ptr128` | Points to N bytes (known layout) |
| Bits | `Ptr<Bits @/0..63>` | Points to exact bit range |

`Ptr<T>` **cannot be dereferenced directly** — use `volatile_load#` / `volatile_store#`:

```brief
let reg: Ptr<Int> = 0x40011000 as Ptr<Int>;
let val = volatile_load#(reg);
volatile_store#(reg, val + 1);
```

Layout-compatible casts between pointer types are allowed when the pointee
types have the same bytes and compatible alignment:

```brief
let f: Ptr<Float> = 0x4000 as Ptr<Float>;
let i: Ptr<Int32> = f as Ptr<Int32>;  // both 4 bytes, align 4
```

Spatial operations use `lib/std/spatial.bv`:

```brief
import { block_copy } from "std/spatial.bv";
let dst: Ptr64 = malloc(8);
block_copy(dst, src, 8);
```

Function pointers via `:> Ptr`:

```brief
defn cmp(a: Int, b: Int) -> Bool { term a == b; };
let fn_ptr = cmp :> Ptr;
let eq = fn_ptr(3, 5);
```

No `reinterpret_cast` exists. If the compiler can't prove layout compatibility,
use explicit byte-level operations or a `meld` declaration.
