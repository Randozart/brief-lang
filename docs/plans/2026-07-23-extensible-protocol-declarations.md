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

### Syntax — minimal by design

```brief
// Minimal — just edges, no contract
#String ascii {
    CastTo(#String<utf8>);
    CastFrom(#String<utf8>);
};

// With optional contract (recommended) using #Self hashword
#String ascii [forall(i in 0..#Self:>Size, #Self[i] < 128)] {
    CastTo(#String<utf8>);
    CastFrom(#String<utf8>);
};

// With an optional cross-variant op override
#String ascii {
    CastTo(#String<utf8>);
    CastFrom(#String<utf8>);
    op Add(#String<utf8>) = add_utf8_to_ascii(#L, #R);
};
```

A protocol declaration is primarily about **edges**. Self-referencing ops (e.g. adding `ascii` to `ascii`) are never declared — they fall through to the default protocol via CastTo. Ops are only declared for **cross-variant overrides**: `op Add(#String<utf8>) = fn(#L, #R)` means "if you ever need to add utf8 directly to me, no need to walk the graph — this is the way." The self-variant is implicit (the protocol's own variant).

- `#Category variant { ... }` — new top-level form, disambiguated from `type` by the initial `#`
- `CastTo(#Other<Variant>)` / `CastFrom(#Other<Variant>)` — edges in the protocol graph
- `op Name(#TargetCategory<target_variant>) = fn(#L, #R)` — *optional* cross-variant override. Declares how this variant handles an operation against a *different* variant of the same category. No self-referencing ops — those delegate to default.
- `[contract]` before the body — *optional* contract, recommended. Uses Brief expression syntax with `#Self` as a hashword referencing the value at the protocol boundary. `#Self` follows the PascalCase hashword convention (`#Int`, `#String`, `#Self`). Contracts are not required — a protocol without one trusts the programmer.
- No `layout` field — layout is implicit. Types that participate in a protocol declare their own layout. The protocol only defines *semantic compatibility*.

### Three tiers of effort

| Tier | What you write | What the compiler does |
|:---:|---|---|
| 0 — Default delegation | Just the type definition, no protocol declaration | Everything uses the primordial default (`utf8`, `ieee754`) |
| 1 — Edges only | `#String latin15 { CastTo(#String<utf8>); ... };` | Ops fall through to default via CastTo. No custom op code needed. |
| 2 — Edges + contract (recommended) | Tier 1 + `[forall(...)]` before the body | Proof engine checks boundary crossings. Denies compilation on unprovable paths. |
| 3 — Edges + contract + custom ops | Tier 2 + `op Add(#String<utf8>) = fn(#L, #R);` | Cross-variant ops use explicit binding. Contract still enforced. |

Tier 1 is the common case — most variants differ only in encoding/layout, not in operation semantics. Tier 2 is the escape hatch for when a genuinely different operation is needed.

### Graph semantics

- A variant "defined against" another (has `CastTo`/`CastFrom` edges) is automatically compatible
- Sharing a base category (`#String`) does NOT imply interoperability — edges must be explicitly declared
- `#Bits` remains universally reachable (existing invariant)
- BFS finds the shortest path through variant nodes
- Undefined self-ops (e.g. `Length` for `ascii`) resolve by: CastTo default → run default op → CastFrom back. The compiler handles this transparently.
- If a protocol has a contract (`[expr]`), the proof engine checks it at every boundary crossing — data entering via CastTo and data exiting via CastFrom. If the contract cannot be proven at compile time for a given call site, compilation is denied. The programmer acknowledges with `Info#()`, `Warning#()`, or `Error#()`. Without a contract, no boundary check is performed (trusts the programmer).

### Compiler proof — boundary enforcement

When a protocol has an optional contract `[expr]`, the proof engine checks it at every boundary crossing:

- **Entering** via CastTo: the incoming value must satisfy the contract
- **Exiting** via CastFrom: the outgoing value must satisfy the contract
- If unprovable at compile time for a given call site → **compilation denied**
- The programmer acknowledges with `Info#()`, `Warning#()`, or `Error#()` at the call site
- Without a contract, no boundary check — the cast is trusted

The `#Self` hashword in the contract refers to the value at the boundary crossing. For example:

```brief
#String ascii [forall(i in 0..#Self:>Size, #Self[i] < 128)] { ... }
```

The compiler does not need to know what "ascii" means. It only knows: "data in this protocol must satisfy `forall(... < 128)`."

### Compiler proof of custom paths (aspirational)

When a user declares a cross-variant override like:

```brief
op Add(#String<utf8>) = add_utf8_to_ascii(#L, #R);
```

The compiler should ideally verify equivalence:
- **Graph path**: `CastTo(#String<utf8>)` on self → `Add(utf8, utf8)` → `CastFrom(#String<utf8>)`
- **Custom path**: `add_utf8_to_ascii(#L, #R)`

If the symbolic proof engine (`symbolic.rs`) can determine these produce the same result, the custom declaration is validated. If they diverge, the compiler warns. This is an aspirational feature — the initial implementation accepts the user's declaration without proof.

## Implementation Phases

### Phase 1: AST + Parser (~100 lines)

**AST additions (`src/ast/top.rs`):**

```rust
pub struct ProtocolDef {
    pub category: String,       // "String"
    pub variant: String,        // "ascii"
    pub contract: Option<Contract>,  // optional, recommended — enforced at boundaries
    pub cast_edges: Vec<CastEdge>,
    /// Optional cross-variant op overrides.
    /// Never self-referencing — those delegate to the default protocol.
    pub cross_ops: Vec<OperatorDef>,
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
3. Parse optional contract `[expr]` using the existing contract parser (`parse_contract`)
4. Parse `{ CastTo(...); CastFrom(...); op Name(#Target<Variant>) = fn(#L, #R); }` body
5. Return `TopLevel::ProtocolDef(...)`

Default primordial resolution in `src/parser/types.rs:40-48` is unchanged:
- `#String` → `HashWordVariant("#String", "utf8")`
- `#Float` → `HashWordVariant("#Float", "ieee754")`
- `#Char` → `HashWordVariant("#Char", "unicode")`
- All others → `HashWord(name)` (no variant)

**Lexer addition (`src/lexer.rs`):**

Add `#Self` as a recognized multi-character hash token (like `#L`, `#R`, `#T`). It's lexed as `Token::HashSelf` (or equivalent) and resolved at protocol-boundary checking time to "the value at the boundary crossing." Unlike `#L`/`#R` which are positional markers for op bindings, `#Self` is a self-reference for protocol contracts.

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
- `build_from(items: &[TopLevel])` — scan for `ProtocolDef` items, insert edges and cross-variant ops
- `find_protocol_path(source_cat, source_var, target_cat, target_var) -> Option<Vec<CastEdge>>` — BFS through variant-aware graph
- `find_default_delegation(cat, var) -> Option<Vec<CastEdge>>` — BFS from variant to default (for implicit self-op resolution)
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
// These only declare Cast edges; self-ops implicitly delegate to defaults.

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

**Self-op resolution (implicit delegation):**

When the compiler encounters `#String<ascii>` used with a self-referencing op (e.g. `Length`, `Add` to self) and no explicit binding exists on the variant:
1. Query `ProtocolGraph::find_default_delegation` for path to default variant
2. Emit: `CastTo(default) → default_op → CastFrom(variant)`
3. LLVM optimizes the round-trip

This is transparent — no source-level declaration needed. The protocol graph resolves it.

**Cross-variant op overrides:**

When `op Add(#String<utf8>) = add_utf8_to_ascii(#L, #R)` exists on `#String<ascii>`:
- Store on `ProtocolGraph` as a cross-op edge: `(ascii, Add, utf8) → add_utf8_to_ascii`
- When dispatching `Add(#String<ascii>, #String<utf8>)`, check for cross-op before walking graph
- The override is a direct declaration: "for this pair, use this path"

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
- `test_protocol_def_edges_only`: `#String ascii { CastTo(#String<utf8>); };` parses with empty cross_ops
- `test_protocol_def_cross_op`: `#String ascii { CastTo(#String<utf8>); op Add(#String<utf8>) = fn(#L, #R); };`
- `test_protocol_def_empty_body`: `#String ascii {};` — valid, just no edges
- `test_protocol_def_no_self_ops`: verify self-referencing ops like `op Length(#String<ascii>)` are rejected (self-ops delegate to default, never declared)

### Protocol Graph
- `test_graph_single_edge`: utf8 → ascii path found
- `test_graph_multi_hop`: utf8 → ascii → utf16 path found
- `test_graph_default_delegation`: ascii → utf8 delegation path found for implicit self-ops
- `test_graph_cross_op_lookup`: cross-variant override found for Add(ascii, utf8)
- `test_graph_no_path`: unrelated variants return None
- `test_graph_fallback_to_bits`: every variant reaches `#Bits`

### Integration
- `test_implicit_self_op_delegation`: `#String<ascii>` uses Length without declaration — resolves through CastTo → default Length → CastFrom
- `test_cross_op_override`: Add between ascii and utf8 uses explicit binding instead of graph walk
- `test_prelude_variants_available`: ascii/utf16 present with stdlib, absent with `--no-stdlib`
- `test_existing_cast_paths_unchanged`: old `Cast.#` properties still resolve

## Summary

| Phase | What | Files | Lines |
|:---:|---|:---|---:|
| 1 | AST + parser | `src/ast/top.rs`, `src/parser/definitions.rs` | ~100 |
| 2 | Protocol graph + BFS | `src/analysis/protocol_graph.rs` (new), `layout_optimizer.rs` | ~150 |
| 3 | Prelude declarations | `plugins/parsed/prelude.bv` | ~15 |
| 4 | Wire into backend + GLUE | `intrinsics.rs`, `frgn_dispatch.rs`, `bridge.rs` | ~30 |
| — | Tests | Various test files | ~120 |
| | **Total** | | **~415** |

Zero new intrinsics. Zero existing paths modified. Zero TOML config changes. The protocol graph is just more data for the same BFS. LLVM never sees protocols — only concrete types and operations.