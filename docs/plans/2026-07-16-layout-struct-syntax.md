# Layout Struct Syntax — `layout <~ { field: Type }`

**Date:** 2026-07-16
**Status:** Implementation
**Applies to:** Parser, normalizer

---

## Motivation

The angle-bracket layout syntax `<le: [ptr: 64, size: 64]>` gives
bit-level control but is verbose for the common case: fields aligned
to their natural type boundaries, tight-packed, native endianness.

The struct syntax `{ field: Type }` lets the programmer write field
names and Brief types, and the compiler computes bit widths from the
TypeUniverse.

The two paths converge at `LayoutPattern` and produce identical
`field.name.offset` / `field.name.width` metadata.

---

## Syntax

```brief
type MyType : Bits {
    bytes <~ 16;
    layout <~ { ptr: Int, size: Int };
};
```

Each field is `name: Type` separated by `;` or `,`. The body is delimited
by `{ }` and terminated by `;` after the closing brace.

---

## Parser Changes

In `parse_type_body` (definitions.rs ~line 873), the current `layout <~`
handler calls `read_layout_body()` which consumes `<...>` tokens. After
consuming `<~`:

1. Peek for `{` → parse struct-format fields
2. Peek for `<` → existing angle-bracket path

The struct body is parsed as a sequence of `Identifier Colon Type`
triples, terminated by `}`. The result is stored in `metadata` under
key `"layout_struct"` as `PropertyValue::List(Vec<PropertyValue::Tuple> )`.

---

## Normalizer Changes

In `register_typedefs` (normalizer.rs ~line 180), after the existing
angle-bracket layout handling, add:

```
if layout_struct property exists:
    for each (name, type_string) tuple:
        look up type_string in universe
        bits = bytes * 8
        push LayoutField { name, bits, mutable: false, structural: false }
    build LayoutPattern::Slice(fields)
    attach_layout_fields(&mut rt, &pat)
```

---

## Files Touched

- `src/parser/definitions.rs` — add `{` branch in layout handler
- `src/backend/llvm/normalizer.rs` — struct-format → LayoutPattern conversion
