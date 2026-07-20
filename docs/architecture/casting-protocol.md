# Casting Protocol — Type Conversion via Hashword Categories

**Date:** 2026-07-20
**Status:** Foundational
**Applies to:** TypeUniverse, normalizer, all backends, typechecker

---

## Overview

Brief has no built-in `as` cast operator. Type conversion is handled by the
protocol system: every hashword category defines required `Cast` ops, and
the compiler finds the shortest path through the protocol graph at compile
time.

Types never "belong to" categories. They *interact with* categories by
declaring `op Cast(#Category)` in their signature. The presence or absence
of these declarations determines what conversions are available.

---

## Protocol Parameterization

Each hashword category has one or more **protocol variants** — concrete
representations of the same semantic concept:

| Hashword | Protocol variants | Default (`.bv`) | Default (`.ebv`) |
|---|---|---|---|
| `#String` | `utf8`, `ascii`, `hex`, `base64` | `utf8` | `ascii` |
| `#Float` | `ieee754`, `bin32`, `bin64` | `ieee754` (backends choose width) | *(same)* |
| `#Int` | (no variants — width is target-dependent) | (intrinsic) | (intrinsic) |
| `#Bool` | (no variants) | (intrinsic) | (intrinsic) |
| `#Char` | `unicode`, `ascii` | `unicode` | `ascii` |
| `#Bits` | (no variants — raw bits) | (intrinsic) | (intrinsic) |

**The file extension determines the default protocol:**

```brief
// foo.bv — #String<utf8> by default
op Add(#String, #String);   // resolves to #String<utf8>

// bar.ebv — #String<ascii> by default
op Add(#String, #String);   // resolves to #String<ascii>
```

**Cross-variant calls require explicit protocol:**

```brief
fn cross(a: #String<utf8>, b: #String<ascii>) { ... };
                               ^^^^^ explicit
```

Without the angle bracket, the compiler uses the source file's default.
If a function from a different file extension (e.g., `.ebv`) is called
from `.bv` where the protocol differs, the compiler errors:

```
error: protocol declaration for #String not explicitly defined.
  Called from .bv (default: utf8) into .ebv (default: ascii).
  Use #String<utf8> or #String<ascii> to disambiguate.
```

### Why explicit protocols matter

A bit-shifting function written against `#String` bytes produces different
results depending on the encoding. UTF-8 has multi-byte sequences where
the high bit is set. ASCII rejects bytes ≥ 0x80. If the protocol variant
changed silently between file extensions, the same function would produce
different results on different targets.

Explicit protocols at crossing boundaries prevent this. The programmer
must acknowledge the encoding difference and write (or accept) the
appropriate `Cast` between variants.

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
    op Cast(#Int) = float_to_int(#L);     // edge: Float → #Int
    op Cast(#Bits) = float_to_bits(#L);   // edge: Float → #Bits (explicit)
};

type ASCIIString <: String {
    op Cast(#String<utf8>) = ascii_to_utf8(#L);  // edge: ASCIIString → #String<utf8>
};
```

Each `op Cast(#Category<variant>)` adds an edge from the declaring type to
the specific protocol variant.

---

## Protocol Shapes (Backend Contract)

Every hashword category variant has a concrete representation that ALL
backends must be able to produce and consume:

| Hashword | Variant | Representation | Required ops |
|---|---|---|---|
| `#Int` | *(none)* | `i{target_width}` | Add, Sub, Mul, Div, And, Or, Xor, Not, Shl, Shr |
| `#Float` | `ieee754` | binary32 or binary64 | Add, Sub, Mul, Div, Sqrt, FMA, `Cast(Float64)` |
| `#Float` | `bin32` | binary32 | Same as ieee754, fixed at 32-bit |
| `#Float` | `bin64` | binary64 | Same as ieee754, fixed at 64-bit |
| `#Bool` | *(none)* | `i1` (stored i8) | And, Or, Not |
| `#Char` | `unicode` | `i32` code point 0–0x10FFFF | `Cast(#Int)`, Eq, Lt |
| `#Char` | `ascii` | `i8` code point 0–127 | `Cast(#Int)`, Eq, Lt |
| `#String` | `utf8` | UTF-8 byte sequence | Extract(`#Char`), InsertAt(`#Char`), Concat(`#String`), `:> Size`, `Cast(#Bits)` |
| `#String` | `ascii` | ASCII byte sequence (0–127 per byte) | Same as utf8 — protocol ops are encoding-agnostic |
| `#String` | `hex` | Hex-encoded bytes (`0-9a-f` pairs) | Same as utf8 |
| `#String` | `base64` | Base64-encoded bytes | Same as utf8 |
| `#Bits` | *(none)* | Raw `iN` | And, Or, Xor, Not, Shl, Shr |

The protocol ops (`Extract(#Char)`, `InsertAt(#Char)`, etc.) are the same
regardless of variant. The backend's protocol handler translates between
the variant's internal representation and the protocol currency (`Char`).

---

## Conversion Path Resolution

When the compiler encounters a cross-type operation (e.g., `ascii_str + string`),
it resolves the conversion path:

1. **Exact match**: Type defines `op Add(String)` → use it.
2. **Direct cast**: Type A defines `op Cast(#Category<variant>)` → use it once.
3. **Protocol chain**: Find shortest path through the protocol graph.
4. **Implicit fallback**: `#Bits → #Bits` (raw bytes).

### BFS Search

```
Input:  source_type, target_category<variant>
Output: sequence of Cast ops, or error

1. If source_type has op Cast(target_category<variant>):
       return [source_type → target_category<variant>]
2. BFS over the protocol graph from source_type to target_category<variant>:
   - Each node is a category variant (e.g. #String<utf8>)
   - Each edge is a Cast(Category<variant>) declaration
   - #Bits is always reachable from every type
3. If path found: return sequence of Cast ops
4. If no path: compiler error with available alternatives
```

### Example: `#String<ascii> → #String<utf8>`

If no direct `Cast(#String<utf8>)` exists, the path is:
`#String<ascii> → #Bits → #String<utf8>`

1. `#String<ascii> :> Cast(#Bits)` → raw bytes
2. `#Bits :> Cast(#String<utf8>)` → construct UTF-8 from raw bytes

The backend implements step 2 in its protocol handler. An optimizing
backend that uses ASCII internally for both would skip both casts.

---

## Backend Protocol Handlers

Each backend declares which hashword categories and protocol variants it
supports in `config/targets.toml`:

```toml
[target.desktop]
backend = "llvm"
protocols = [
    "#String<utf8>",
    "#String<ascii>",
    "#Float<ieee754>",
    "#Int",
    "#Bool",
    "#Char<unicode>",
    "#Char<ascii>",
    "#Bits",
]

[target.embedded-riscv]
backend = "llvm"
protocols = [
    "#String<ascii>",
    "#Int",
    "#Bool",
    "#Char<ascii>",
    "#Bits",
]
```

The backend implements a protocol handler — a `match` on the variant:

```rust
impl LlvmBackend {
    fn emit_string_concat(&mut self, a: &TypedRegister, b: &TypedRegister, protocol: &str) {
        match protocol {
            "utf8" => {
                writeln!(out, "%r = call {{ i64, i64 }} @__utf8_concat({}, {})",
                    a, b);
            }
            "ascii" => {
                writeln!(out, "%r = call {{ i64, i64 }} @__ascii_concat({}, {})",
                    a, b);
            }
            "hex" | "base64" => {
                // No hardware support — stdlib fallback
                writeln!(out, "%r = call {{ i64, i64 }} @__string_transform({}, {}, \"{}\")",
                    a, b, protocol);
            }
            _ => compile_error!("protocol '{}' not supported by LLVM backend", protocol),
        }
    }
}
```

A function requiring a protocol the backend does not implement:

```
error: target 'embedded-riscv' does not support protocol '#String<utf8>'.
  Required by function 'generic_concat' in foo.bv.
  Available protocols on this target: #String<ascii>, #Int, #Bool, ...
```

### Cross-variant detection

The typechecker treats `#String<utf8>` and `#String<ascii>` as distinct types.
Passing one where the other is expected produces:

```
type mismatch: expected #String<ascii> for parameter 1, found #String<utf8>
```

The file extension determines the default variant at parse time:
- `.bv` files: bare `#String` → `#String<utf8>`
- `.ebv` files: bare `#String` → `#String<ascii>`

When a `.bv` file calls an `.ebv` function using `#String`, the default
variants differ (`utf8` vs `ascii`), and the typechecker's existing
mismatch detection catches it automatically. The programmer adds the
explicit variant at the call site:

```brief
fn cross(a: #String<utf8>, b: #String<ascii>) { ... };
```

### Adding new protocols

Adding a new protocol variant is additive:
1. Add it to the protocol list in `config/targets.toml`
2. Add a match arm in the backend's protocol handler
3. Add the conversion function in stdlib (or inline defn)

No changes to the type system, typechecker, or normalizer.

---

## `disamb` — Disambiguation Hint

The `disamb <~ "value"` metadata property disambiguates representations that
structure + bytes + protocol ops cannot distinguish. Currently only needed
for 2-byte floats (`half` vs `bfloat`):

```brief
type Bfloat16 {
    data: Bits<16>;
    disamb <~ "bfloat";
    op Add(#Float, #Float);
};
```

The normalizer reads `disamb` when deriving `llvm_type` for `#Float`-category
types at 2 bytes:

| `disamb` absent | `disamb <~ "bfloat"` |
|---|---|
| `llvm_type = "half"` (IEEE 754) | `llvm_type = "bfloat"` |

`disamb` is a **hint, not a directive** — the normalizer ignores it when
structure alone is sufficient (e.g., 4-byte `#Float` is always `"float"`,
8-byte is always `"double"`). It only matters when the combinatorics of
bytes + category ops produce multiple valid representations.

---

## Protocol Ops (Required Per Category)

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

```brief
inline defn any_string_to_ascii(source: #String) -> #String<ascii> {
    let len = source :> Size;
    let result = #String<ascii>::alloc(len);
    let mut i = 0;
    do {
        let c: Char = source :> Extract(i);
        result :> InsertAt(i, c);
        i = i + 1;
    } while i < len;
    result
};
```

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
`Float64` at the protocol boundary.

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
    buckets: Bits<64>;
    len: Bits<64>;
    capacity: Bits<64>;
    op Insert(V) = hashmap_insert(#L, #R1, #R2);
    op Get(K) -> V = hashmap_get(#L, #R);
};
```

`K: #String` is a **protocol satisfaction check**. The compiler verifies:
does `K` implement the `#String` protocol ops (`Extract(#Char)`,
`InsertAt(#Char)`, `Concat(#String)`, `:> Size`, `Cast(#Bits)`)?
If yes, `K` satisfies `#String` regardless of its concrete name or layout.

Protocol variant constraints are also valid:
```brief
type AscHashMap<K: #String<ascii>, V> { ... };
```

---

## Implementation Phases

### Phase 1: Protocol Variant Syntax

- Parse `#Category<variant>` in op signatures
- File extension → default protocol mapping (.bv → utf8, .ebv → ascii)
- Cross-variant call detection and error reporting

### Phase 2: Protocol Graph Skeleton

- `Cast(#Category<variant>)` as a valid op signature
- Implicit `Cast(#Bits)` on every type
- BFS path resolution in the typechecker

### Phase 3: Protocol Shape Validation

- Per-category variant validation in typechecker
- `K: #String` constraint satisfaction checking
- Missing protocol ops → compile error with available alternatives

### Phase 4: Backend Protocol Handlers

- `config/targets.toml` → protocol list per target
- LLVM backend protocol handler match arms
- Unsupported protocol → compile error listing available protocols
