# Protocol Types — How Brief Types Speak Across Languages

**Date:** 2026-07-22
**Status:** Architecture documentation

---

## The Core Idea

A protocol category (written as `#String`, `#Int`, `#Float`, `#Bits`) is not
a type. It's a **contract** — a promise that "I support the operations of this
category."

A concrete Brief type like `Custom("String")` is an opaque `{i64, i64}` struct
with SSO inline storage. A Python `str` is a C `PyObject*` with UCS-4 encoding.
Neither has anything in common at the byte level. But both can declare:

```brief
// Brief String says: I know how to act like a #String
op CastTo(#String);   // my data is already UTF-8, just expose it

// Python str says: I know how to accept a #String
op CastFrom(#String); // convert to UCS-4, I'll handle the detail
```

The protocol category `#String` IS the shared abstraction. The concrete types
never need to know about each other — they only need to know how to relate
to the protocol.

## Protocol Categories Are Fictional, Not Abstract

A protocol category has **no implementation**. There is no "string struct"
for `#String`. It's a hypothetical — "this is what a thing WOULD look like
if it were a string, but I don't care what it needs to be."

The compiler uses the TypeUniverse to discover CastTo/CastFrom relationships:

```
TypeUniverse:
  "String"  → properties: { "Cast.#String": "", "bytes": 16 }
  "str"     → properties: { "Cast.#String": "", "bytes": 16 }
  "#String" → properties: { "bytes": 0 }  // fictional, no size
```

Both `"String"` and `"str"` have `Cast.#String` — they both speak the protocol.
The BFS (breadth-first search) in `find_cast_path()` finds:

```
Path: [String → #String → str]
  Step 1: String.CastTo(#String) — cost 0 (identity — SSO inline is already UTF-8)
  Step 2: str.CastFrom(#String) — cost 0 (identity — both are {ptr, len} UTF-8)
  
Total cost: 0 → zero instructions at the boundary.
```

If the path has non-zero cost, the transforms are emitted as real instructions:
- `Bitcast`: LLVM `bitcast` instruction (same byte width, different type)
- `MeldShuffle`: `extractvalue`/`insertvalue` for field reordering
- `ProtocolTransform(#category)`: call `_CastTo_#category` intrinsic

## Protocol Resolution at the Boundary

When a frgn or export crosses a language boundary, the GLUE bridge:

1. **Queries the universe** — for the Brief type, finds which protocol categories
   it participates in via `Cast.#Category` properties
2. **Looks up the protocol** in the target language's TOML config —
   `lib/glue.toml` has `protocols` sections like:
   ```toml
   [rust.protocols]
   "#String" = { native = "str", c_abi = "i64" }
   "#Int" = { native = "i64", c_abi = "i64" }
   ```
3. **Computes the path** — BFS finds the cheapest transform chain between
   the Brief type's representation and the target language's representation
4. **Emits the transforms** — `emit_protocol_chain()` in `src/glue/bridge.rs`
   generates the LLVM IR for each step in the path
5. **If the path is empty or identity, the boundary compiles to zero instructions**
   at LTO time

## The TOML Config Sees Only Protocols, Not Types

The `lib/glue.toml` file maps **protocol categories**, not Brief types:

```toml
# This is all the TOML knows. No Brief type names, no language-specific logic.
[rust.protocols]
"#String" = { native = "str", c_abi = "i64" }
"#Int" = { native = "i64", c_abi = "i64" }
"#Float" = { native = "f64", c_abi = "double" }
```

A Brief `String`, Rust's `str`, a Python `str`, and a Node `string` all speak
`#String`. The TOML only needs to say "if you encounter `#String`, use this
native type and this C ABI type." Zero knowledge of Brief-internal types leaks
into the config.

## Why `#Bits` Always Works

Every type in Brief IS bits. `#Bits` is the universal protocol — any type can
declare `op CastTo(#Bits)` at zero cost because the type's underlying memory
IS just bits.

The BFS always has `#Bits` as a fallback path. If no higher protocol path
exists between two languages, the bridge emits a plain bitcast:

```
Path: [Custom("MyOpaqueStruct") → #Bits → Ptr<u8>]
  Cost: 2 bitcasts (one to #Bits, one from #Bits)
```

This works for any pair of types with the same byte width. The protocol
system's job is to find SHORTER paths that preserve meaning — but `#Bits`
means the bridge never fails purely due to type layout differences.

## Relationship to `op` Declarations

In `bootstrap.bv`, concrete types declare their protocol participation:

```brief
type String <: Bits {
    op CastTo(#String);
    op Add(#String);        // ++ operator on String
    op Eq(#String);         // == comparison
};
```

The TypeUniverse picks up these declarations and populates the `Cast.*`
properties that the BFS walks. A custom type can participate in any protocol
by declaring `op CastTo(#Category)` and optionally `op CastFrom(#Category)`:

```brief
type MyString <: Bits {
    op CastTo(#String);    // MyString can be used wherever #String is expected
};
```

See `learn-brief/15-custom-types.md` for a tutorial on this.
