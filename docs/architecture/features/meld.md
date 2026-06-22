# Meld Feature

**Purpose:** Declare two types as mutually lens-compatible, enabling zero-cost casting and shared memory representation.

**Date added:** 2026-06-22  
**Phase:** 1 (Adaptive Layout Engine) — FieldMode, projection usage, cache slots  
**Status:** Core infrastructure complete; field elimination disabled (conservative)

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

## Phase 1 — Adaptive Layout Engine

### FieldMode Enum (`src/analysis/mod.rs`)

```rust
pub enum FieldMode {
    Always,                              // Always present in %State
    LazyCached { cache_index: usize },   // Cache slot + valid flag appended
    Never,                               // Eliminated from %State
}
```

### Projection Usage Analysis (`src/analysis/transition_graph.rs`)

`compute_projection_usage()` scans all transaction bodies for `Expr::Projection` where the source is a state field identifier. Returns a map: field name → set of projection target strings.

`assign_field_modes()` uses projection usage to assign `LazyCached` when a field has ≥2 distinct projection targets (dual-lens access). Single-lens fields get `Always`.

### LLVM Backend Integration (`src/backend/llvm/mod.rs`)

New fields on `LlvmBackend`:
- `field_modes: HashMap<String, FieldMode>`
- `cache_slots: HashMap<String, (usize, usize)>` — maps field name to `(cache_idx, valid_idx)`

`apply_field_modes()` called after `build_field_index()`:
1. Iterates all fields, keeps all fields (Never elimination disabled — conservative)
2. For `LazyCached` fields: appends `{ i64, i8 }` to `field_types` (cache value + valid flag)
3. Stores cache slot indices in `cache_slots`

### emit_inline_init_stores() (`src/backend/llvm/emit_toplevel.rs`)

Cache slots initialized to `{ 0, 0 }` (no value, not valid) after regular field initialization.

### Key Design Decisions

- **Never elimination disabled** by default because `live_fields` is too conservative (only seeds from FFI, exit conditions, preconditions — not direct field reads). When re-enabled, fields not in `live_fields` with no projection usage entry would be eliminated.
- **Dual-lens detection** today is based on projection target count (≥2 targets = LazyCached). Future refinement: check if accesses occur in a hot loop body.
- **Cache slots are NOT in `field_index_map`** — they're appended to `field_types` only. This keeps `pre_load_all_fields()` and field iteration loops from loading/storing cache values as if they were user fields.

### Tests (Phase 1)

- `test_compute_projection_usage_none` — no projections → empty usage
- `test_compute_projection_usage_single` — single projection on state field detected
- `test_assign_field_modes_single_lens` — single target → Always
- `test_assign_field_modes_dual_lens` — two targets → LazyCached
- `test_assign_field_modes_no_usage` — no usage → no mode assigned
- `test_adaptive_layout_cache_slots_in_state` — %State output contains expected types

## Evaluation

Not yet implemented. See Phase 2 of the meld implementation plan.

## Codegen

Phase 1: Cache slots are appended to `%State` and initialized to zero. Phase 2 (Chimera Projection Dispatch) will emit cache load/store logic.
