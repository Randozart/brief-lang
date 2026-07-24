# Extensible Protocol Declarations — Operation-First Compilation

**Date:** 2026-07-23
**Status:** Plan
**Executive Sponsor:** User

## Summary

Add user-declarable protocol variants (`#String ascii { CastTo(#String<utf8>); };`) so programmers can define custom bit-layout assumptions and their compatibility edges — without modifying the compiler. Protocols remain a frontend-only abstraction; the compiler resolves them to concrete types before LLVM ever sees them.

## Context

Two of three discussion topics are **already implemented**:

| Feature | Status |
|---------|--------|
| Beast naming with `.priority` extension (`file.Parsed.999.beast`) | Done |
| `--emit-beast stage.before` / `stage.after` CLI + pipeline | Done |

This plan covers the third topic: user-declarable protocol variants.

## Cultural Note

> *"Water bij de wijn doen"* — Dutch proverb for diluting wine with water.

Every other language dilutes at the FFI boundary. C++ diluted OOP and template safety for C compat. Java, C#, JS locked into UTF-16. Rust pays a runtime translation tax at every FFI call. All of them mix water into their wine.

Brief does not dilute. The protocol graph keeps the source experience pure wine and lets the compiler become perfect water at the boundary — zero runtime translation, zero compromise on either side. The architecture's job is to keep these two domains separate: the programmer writes pure wine, the compiler emits perfect water.

Proost.

## Philosophy

### Protocols are a frontend abstraction — LLVM never sees them

```
Source:  #String, op Length, op Add
             │
             ▼
    Protocol Graph ─── BFS CastTo/CastFrom edges
    (frontend only)    (fewer hops = less conversion code)
             │
             ▼
    Concrete types + ops chosen per target
    (backend: struct { ptr, i64 }, native add)
             │
             ▼
    LLVM IR: concrete types, concrete ops, never knows protocols
```

LLVM already handles target adaptation (Windows UTF-16, calling conventions, register widths). The compiler should never compete with LLVM. Protocols are a way for the programmer to write *operation-based* code instead of *layout-based* code. The compiler translates abstract intent to concrete layouts, LLVM optimizes the concrete result.

### Fewer hops = less conversion

Declaring a variant closer to the target means fewer BFS hops:
- `#String<utf16>` on Windows → identity path, zero conversion code
- `#String` → resolves through `utf8 → utf16` → one conversion edge
- Both compile correctly; the pinned variant gives the optimizer less work

### Primordial defaults vs prelude variants

Defaults (utf8, ieee754, unicode) are hardcoded in the parser — they work with `--no-stdlib`. Non-default variants (ascii, posit32, utf16) are provided by the prelude plugin. This matches the intrinsics-vs-stdlib dividing line.

## Design

### Syntax

```brief
// Protocol declaration — defines a variant and its compatibility edges
#String ascii {
    CastTo(#String<utf8>);
    CastFrom(#String<utf8>);
    // Optional: backend-directive ops
    op Length(#String<ascii>) -> Int;
};
```

- `#Category variant { ... }` is a new top-level form
- Disambiguated from `type` by the initial `#`
- `CastTo(#Other<Variant>)` / `CastFrom(#Other<Variant>)` — edges in the protocol graph
- `op Name(#Params)` — backend-directive (not a binding); tells the backend "when you see types in this protocol being operated on with this name, apply your default semantics"
- No `layout` field — layout is implicit. Types that participate in a protocol declare their own layout. The protocol only defines *semantic compatibility*.

### Graph semantics

- A variant "defined against" another (has `CastTo`/`CastFrom` edges) is automatically compatible
- Sharing a base category (`#String`) does NOT imply interoperability — edges must be explicitly declared
- `#Bits` remains universally reachable (existing invariant)
- BFS finds the shortest path through variant nodes

## Implementation Phases

### Phase 1: AST + Parser (~110 lines)

**AST additions (`src/ast/top.rs`):**

```rust
pub struct ProtocolDef {
    pub category: String,       // "String"
    pub variant: String,        // "ascii"
    pub cast_edges: Vec<CastEdge>,
    pub operators: Vec<OperatorDef>,
    pub span: Option<Span>,
}

pub struct CastEdge {
    pub direction: CastDirection,
    pub target_category: String,
    pub target_variant: String,
}

pub enum CastDirection { CastTo, CastFrom }
```

Add `ProtocolDef(ProtocolDef)` variant to `TopLevel` enum.

**Parser (`src/parser/definitions.rs`):**

New arm in `parse_top_level()`: when `peek()` is `Identifier(name)` where `name` starts with `#` (and isn't one of the reserved multi-char tokens `#[`, `#!`, `#?`, `#L`, `#R`, `#T`):
1. Consume `#Category` identifier
2. Expect `identifier` for variant name
3. Parse `{ CastTo(...); CastFrom(...); op ...; }` body
4. Return `TopLevel::ProtocolDef(...)`

Default primordial resolution in `src/parser/types.rs:40-48` is unchanged:
- `#String` → `HashWordVariant("#String", "utf8")`
- `#Float` → `HashWordVariant("#Float", "ieee754")`
- `#Char` → `HashWordVariant("#Char", "unicode")`
- All others → `HashWord(name)` (no variant)

### Phase 2: Protocol Graph (~150 lines)

**New file: `src/analysis/protocol_graph.rs`**

```rust
pub struct ProtocolGraph {
    /// (category, variant) → outgoing Cast edges
    edges: HashMap<(String, String), Vec<CastEdge>>,
    /// Pre-registered primordial defaults
    defaults: HashMap<String, String>,
}
```

Methods:
- `new()` — register primordial defaults
- `build_from(items: &[TopLevel])` — scan for `ProtocolDef` items, insert edges
- `find_protocol_path(source_cat, source_var, target_cat, target_var) -> Option<Vec<CastEdge>>` — BFS through variant-aware graph
- `register_edges(universe: &mut TypeUniverse)` — inject graph edges as `Cast.#` properties for existing BFS to find

**Integration (`src/analysis/layout_optimizer.rs`):**

`find_cast_path` gets an optional `&ProtocolGraph` parameter. When searching for neighbors:
1. Check universe `Cast.#` properties (existing)
2. If protocol graph present, query it for variant-aware edges
3. `#Bits` fallback (existing)

No existing arms modified. Protocol graph is additional data for the same BFS.

### Phase 3: Prelude Declarations (~15 lines)

In `plugins/parsed/prelude.bv` (or a companion file discovered by the same plugin system):

```brief
// Non-default protocol variants.
// Defaults (utf8, ieee754, unicode) are primordial — no declaration needed.

#String ascii {
    CastTo(#String<utf8>);
    CastFrom(#String<utf8>);
};

#String utf16 {
    CastTo(#String<utf8>);
    CastFrom(#String<utf8>);
};

#Float posit32 {
    CastTo(#Float<ieee754>);
    CastFrom(#Float<ieee754>);
};
```

`--no-stdlib` disables these. Defaults work regardless.

### Phase 4: Wire into Backend + GLUE (~30 lines)

**Cast resolution pipeline (`src/backend/llvm/intrinsics.rs:945`):**

Existing 4-step pipeline, unchanged:
1. Direct `op Cast(Target)` — existing
2. Protocol path via `try_cast_protocol_path` — updated to also consult `ProtocolGraph`
3. Meld shuffle — existing
4. Implicit `Cast(#Bits)` — existing

**GLUE bridge (`src/analysis/frgn_dispatch.rs`, `src/glue/bridge.rs`):**

- `compute_protocol_path()` — try protocol graph if universe BFS fails, before bitcast fallback
- GLUE export — protocol graph supplements `lib/glue.toml`. TOML config wins when present

**Protocol-level `op` semantics:**

`op Add(#String<ascii>, #String<ascii>)` inside a protocol declaration is a backend directive. It tells the backend: "when types in this protocol use `Add`, apply your default semantics for the concrete type." No binding code, no `= fn(#L, #R)`. If the backend knows the concrete type, it emits native ops; if not, it falls through to function call.

## Backward Compatibility

| Existing behavior | Impact |
|---|---|
| Parser defaults (`#String` → `utf8`) | Unchanged |
| Type bodies with `op CastTo(#String<utf8>)` | Unchanged — still sets `Cast.#` properties |
| `lib/glue.toml` protocol mappings | Unchanged — TOML still wins over graph |
| BFS path finding | Unchanged — graph is additional data |
| `--no-stdlib` | Primordial defaults still work |

## Testing

### Parser
- `test_protocol_def_simple`: `#String ascii { CastTo(#String<utf8>); };` parses correctly
- `test_protocol_def_with_ops`: protocol def with `op Length(#String<ascii>) -> Int`
- `test_protocol_def_empty_body`: valid with empty braces
- `test_protocol_def_invalid_category`: `#123` rejected

### Protocol Graph
- `test_graph_single_edge`: utf8 → ascii path found
- `test_graph_multi_hop`: utf8 → ascii → utf16 path found
- `test_graph_no_path`: unrelated variants return None
- `test_graph_fallback_to_bits`: every variant reaches `#Bits`
- `test_graph_primordial_defaults`: defaults always registered

### Integration
- `test_protocol_path_in_cast_resolution`: frgn dispatch finds path through protocol graph
- `test_prelude_variants_available`: ascii/utf16 present with stdlib, absent with `--no-stdlib`
- `test_existing_cast_paths_unchanged`: old `Cast.#` properties still resolve

## Summary

| Phase | What | Files | Lines |
|:---:|---|:---|---:|
| 1 | AST + parser | `src/ast/top.rs`, `src/parser/definitions.rs` | ~110 |
| 2 | Protocol graph + BFS | `src/analysis/protocol_graph.rs` (new), `layout_optimizer.rs` | ~150 |
| 3 | Prelude declarations | `plugins/parsed/prelude.bv` | ~15 |
| 4 | Wire into backend + GLUE | `intrinsics.rs`, `frgn_dispatch.rs`, `bridge.rs` | ~30 |
| — | Tests | Various test files | ~120 |
| | **Total** | | **~450** |

Zero new intrinsics. Zero existing paths modified. Zero TOML config changes. The protocol graph is just more data for the same BFS LLVM never sees protocols — only concrete types and operations.
