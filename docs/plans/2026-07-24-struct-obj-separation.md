# `struct` / `obj` Separation

**Date:** 2026-07-24
**Status:** Plan → Implementation

## Goal

Rename current `struct` (dynamic, methods, instantiable) to `obj`.
Introduce new `struct` (static, fixed layout, C-compatible, pure data).

## Changes

### Phase 1: Lexer

Add `Obj` token for keyword `obj`. `Token::Struct` stays for the new `struct`.

### Phase 2: AST

New `TopLevel::StaticStruct(StructDef)` — static fixed-layout struct.

```rust
pub struct StructDef {
    pub name: String,
    pub fields: Vec<(String, Type)>,
    pub metadata: HashMap<String, PropertyValue>,
    pub span: Option<Span>,
}
```

Current `TopLevel::TypeDef(TypeDefBody::Struct(...))` is renamed to `...::Obj(...)`.

### Phase 3: Parser

- `Token::Struct` → `parse_struct_def()` → `TopLevel::StaticStruct`
- `Token::Obj` → `parse_obj_like()` → same as old `parse_struct_like`
- Rename `parse_struct_like` → `parse_obj_like`

### Phase 4: Migration

All `.bv` files: `s/\bstruct\b/obj/g` (lib/std, examples, benchmarks, test fixtures).
All Rust match arms matching `TypeDefBody::Struct` → `TypeDefBody::Obj`.

### Phase 5: Codegen

New `struct` emits LLVM struct type with known field offsets, populates
type universe via the same mechanism as `InjectTypeLayout$`. Protocol
graph BFS uses this for cross-language layout matching.
