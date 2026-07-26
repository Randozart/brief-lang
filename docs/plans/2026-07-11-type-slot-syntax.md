# Type Slot Syntax — Plan

## Summary

Add field/slot syntax to `type` declarations so type definitions can declare
how their bits are partitioned — not just metadata properties. This makes
`type` the universal mechanism for bit interpretation, and removes the last
hardcoded type hacks (String's `struct_layout`).

## Motivation

Currently there are two separate systems:

| Construct | Purpose | Registry |
|-----------|---------|----------|
| `type` | Bit interpretation (metadata, operators, constraints) | TypeUniverse |
| `struct` | Organized data bundles (named fields) | `struct_types` |

A `type` can declare `bytes <~ 8;` and `codec <~ "UTF8"` but cannot declare
that its bits are partitioned into `{ptr, len, codec}`. That requires either
a hardcoded `struct_layout` hack (String at `type_universe.rs:488`) or a
separate `struct` declaration.

But partitioning bits IS part of the interpretation. A type's slot layout
answers: "how do I read these N bytes as this type?" Adding slot syntax to
`type` makes the type declaration self-contained.

## What changes

### Parser

Inside a `type` body, alongside property bindings (`name <~ expr;`),
operators (`op Rune(Param) -> Ret = intrinsic;`), and constraints
(`[expr];`), allow slot declarations:

```brief
type String {
    ptr: Ptr<UInt8>;      // slot: bits 0..63 → Ptr<UInt8>
    len: Int;             // slot: bits 64..127 → Int
    codec: UInt8;         // slot: bits 128..135 → UInt8
    codec <~ "UTF8";      // property: default codec
    alignment <~ 8;       // property: alignment
    op Concat(String) -> String = __string_concat#;
    [len >= 0];
};
```

The parser distinguishes slots from properties by the separator:
- `name: Type;` → slot declaration (colon, type, semicolon)
- `name <~ expr;` → property binding (tilde-arrow, expression, semicolon)

Slot declarations are stored in a new `slots: Vec<TypeSlot>` field on
`TypeDefBody`:

```rust
pub struct TypeSlot {
    pub name: String,
    pub ty: Type,
    pub span: Option<Span>,
}
```

### TypeUniverse

When `resolve_type_def()` processes a `TypeDef` with slots, it populates
`ResolvedType.struct_layout` from the slot list. Offsets are computed
sequentially — each slot starts at the byte after the previous one, with
alignment padding. This replaces the hardcoded String hack at
`type_universe.rs:488-500`.

```rust
if !td.body.slots.is_empty() {
    let mut offset_bits = 0u64;
    let mut fields = Vec::new();
    for slot in &td.body.slots {
        let slot_bits = resolve_bytes(&slot.ty, universe) * 8;
        fields.push(StructField {
            name: slot.name.clone(),
            ty: slot.ty.clone(),
            offset_bits,
            size_bits: slot_bits,
        });
        offset_bits += slot_bits;
    }
    rt.struct_layout = Some(StructLayout {
        fields,
        packed: false,
        total_bytes: offset_bits / 8,
        alignment: rt.alignment,
    });
}
```

### `struct_types` population

The `struct_types` HashMap is currently populated ONLY from
`TopLevel::Struct` items (mod.rs line 1574). After this change, it is also
populated from `TopLevel::TypeDef` items whose resolved type has a
non-`None` `struct_layout`.

The population happens in `generate()`'s first scan pass (same place where
`TopLevel::Struct` is handled):

```rust
// After struct_types has been populated from TopLevel::Struct items:
if let Some(ref universe) = self.ctx.type_universe {
    for (name, rt) in &universe.types {
        if let Some(ref layout) = rt.struct_layout {
            let fields: Vec<(String, Type)> = layout.fields.iter()
                .map(|f| (f.name.clone(), f.ty.clone()))
                .collect();
            self.ctx.struct_types.entry(name.clone()).or_insert(fields);
        }
    }
}
```

This means:
- `struct_types` has entries from BOTH `struct` declarations AND `type` declarations with slots
- `declare_struct_types()` emits LLVM named types for both
- `llvm_type()` returns `"ptr"` for both (in export signatures)
- Field access codegen in `rest.rs` and `projection.rs` works for both

### No changes to `struct`

`struct` stays exactly as it is today — organized data bundles with
`StructDefinition` fields (defaults, visibility, inline transactions,
variants, view_html). The `struct` keyword still populates `struct_types`
via the existing code path.

The only difference is that `struct` now also auto-generates a minimal
`ResolvedType` entry in the TypeUniverse (see below).

### Struct auto-registration in TypeUniverse

Since a struct IS a type (product type), `TopLevel::Struct` items also
generate a `ResolvedType` entry:

```rust
TopLevel::Struct(s) => {
    // Existing: populate struct_types
    let fields: Vec<(String, Type)> = s.fields.iter()
        .map(|f| (f.name.clone(), f.ty.clone()))
        .collect();
    self.ctx.struct_types.insert(s.name.clone(), fields.clone());

    // NEW: generate minimal TypeUniverse entry
    if let Some(ref mut universe) = self.ctx.type_universe {
        if !universe.types.contains_key(&s.name) {
            let byte_size = fields.iter().map(|(_, t)| t.to_bits().unwrap_or(64) / 8).sum();
            universe.types.insert(s.name.clone(), ResolvedType {
                name: s.name.clone(),
                type_params: s.type_params.clone(),
                base: "Bits".to_string(),
                bytes: byte_size,
                alignment: 8,
                llvm_type: format!("%{}", s.name),  // will be used by declare_struct_types
                storage: "Native".to_string(),
                ..ResolvedType::default()
            });
        }
    }
}
```

This means structs participate in meld lookups (by string name, same as
types), without needing any special-case meld code for structs.

### String migration

The hardcoded `struct_layout` hack at `type_universe.rs:488-500` is
removed. String's bootstrap declaration in
`lib/std/types/bootstrap.bv` gains slot declarations:

```brief
type String {
    ptr: Ptr<UInt8>;
    len: Int;
    codec: UInt8;
    bytes <~ 8;
    alignment <~ 8;
    llvm <~ "i8*";
    storage <~ "Boxed";
    tbaa <~ "String";
    box <~ "ptrtoint#";
    unbox <~ "inttoptr#";
    default_width <~ 64;
    default_codec <~ 0;
};
```

The offset computation in `resolve_type_def` handles the slot layout:
- `ptr: Ptr<UInt8>` → 64 bits at offset 0
- `len: Int` → 64 bits at offset 64
- `codec: UInt8` → 8 bits at offset 128
- Total: 136 bits = 17 bytes → 24 bytes after alignment padding

## What remains unchanged

- `struct` keyword, syntax, and semantics (organized data bundles)
- `declare_struct_types()` function (still iterates `struct_types`)
- Field access codegen (`rest.rs`, `projection.rs`, `helpers.rs`)
- `llvm_type()` struct check (still checks `struct_types`)
- Export wrapper emission
- GLUE bridge phases

## Implementation order

1. Add `slots: Vec<TypeSlot>` to `TypeDefBody` in `ast.rs`
2. Update `parse_type_def()` in `parser.rs` to parse `name: Type;` slots
3. Update `resolve_type_def()` in `type_universe.rs` to compute `struct_layout` from slots
4. Remove hardcoded String `struct_layout` at `type_universe.rs:488-500`
5. Update String's bootstrap declaration with slot syntax
6. Add TypeUniverse → `struct_types` population in `generate()` scan pass
7. Add struct auto-registration in TypeUniverse from `TopLevel::Struct`
8. Update bootstrap.bv with String's new declaration
9. Tests, tests, tests
