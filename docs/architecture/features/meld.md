# Meld Feature

**Purpose:** Declare two types as mutually lens-compatible, enabling zero-cost casting and shared memory representation.

**Date added:** 2026-06-22  
**Phase:** 0 (Foundation) — AST, parser, TypeUniverse, typechecker  
**Status:** Complete

## Syntax

```brief
// Tier 1 — Fully inferred: routes derived from @/ bit-range matching
meld Float <:> CFloat;

// Tier 2 — Partially inferred: user provides explicit routes
meld String <:> CString {
    Ptr -> CString.ptr;
    Size -> CString :> Size;
};

// Tier 3 — Fully explicit with router body
meld A <:> B {
    Ptr -> B.ptr;
    Size -> B.len;
};
```

Two forms:
- `meld A <:> B;` — no explicit routes, compiler infers from `@/` bit-range matching
- `meld A <:> B { ... };` — explicit routes override/customize inference

## Phase 0 — Implementation Details

### AST (`src/ast.rs`)

```rust
pub struct MeldDeclaration {
    pub name_a: String,
    pub name_b: String,
    pub routes: Vec<MeldRouteDef>,
    pub span: Option<Span>,
}

pub struct MeldRouteDef {
    pub accessor: String,
    pub dest_expr: Expr,
}
```

Added `TopLevel::Meld(MeldDeclaration)` variant.

### Lexer (`src/lexer.rs`)

New token `LtColonGt` for `<:>` syntax. New keyword `Meld` for `meld`.

### Parser (`src/parser.rs`)

`parse_meld_decl()` handles:
- `meld A <:> B;`
- `meld A <:> B { Ptr -> B.ptr; Size -> B :> Size; };`

Routes use `accessor -> dest_expr;` syntax within `{ }`.

### TypeUniverse (`src/type_universe.rs`)

New field `melds: HashMap<(String, String), MeldDeclaration>` in `TypeUniverse`. Collected in `build()` Phase 3. Keyed by sorted `(name_a, name_b)` pair for bidirectional lookup.

`find_meld(a, b) -> Option<&MeldDeclaration>` — no transitive resolution.

### Typechecker (`src/typechecker.rs`)

`is_cast_valid()` extended: when both types are `Type::Custom`, checks `TypeUniverse::find_meld()`. Primitive cast pairs unchanged.

### Anti-Patterns

- **No transitive inference**: `A <:> B` + `B <:> C` does NOT imply `A <:> C`. E006 error.
- **No implicit coercions**: All meld crossings require `as` cast.
- **No runtime type tags**: The type is statically known from `TypedRegister.ty`.

### Tests

- 3 parser tests: simple, with routes, empty braces
- 1 TypeUniverse test: registration and bidirectional lookup
- 1 typechecker test: `is_cast_valid` for melded/non-melded custom types

## Evaluation

Not yet implemented. See Phase 2 of the meld implementation plan.

## Codegen

Not yet implemented. See Phases 1-3 of the meld implementation plan.
