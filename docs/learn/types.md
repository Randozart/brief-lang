# Types in Brief — Learning Guide

**Last updated:** 2026-06-09

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

### Syntax Gates

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

### InsertAt / ExtractFrom

These two properties define where elements go when pushing and where they come from when popping:

| Expression | Example use |
|---|---|
| `0` | Front (Queue pop) |
| `:> Size` | Append (push to end) |
| `:> Size - 1` | Last (Stack pop) |
| `<: { MAX(.k) }` | Max-heap ordered |
| `<: { MIN(.k) }` | Min-heap ordered |

### How it works (Two-Pass Pipeline)

1. **Pass 1 (Type-Universe)**: The compiler collects all `Type` declarations, resolves derivation chains, inherits properties, and freezes the type map.
2. **Pass 2 (Executable)**: Uses the frozen type map for type checking, literal encoding, and code generation.

### The Brief philosophy

Most languages hardcode type rules inside the compiler's Rust/C++ source. Brief hardcodes about 13 properties. **That's it.** Everything — `String`, `Stack`, `Queue`, `HashMap`, even `Int` — is defined in Brief source files, using the same syntax you use to define your own types.
