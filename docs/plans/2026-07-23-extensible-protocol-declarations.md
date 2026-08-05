# Extensible Protocol Declarations — Operation-First Compilation

**Date:** 2026-07-23
**Status:** Plan
**Executive Sponsor:** User

## Summary

Add user-declarable protocol variants (`proto ASCII: #String { CastTo(#String<UTF8>); };`) so programmers can define custom bit-layout assumptions and their compatibility edges — without modifying the compiler. Protocols remain a frontend-only abstraction; the compiler resolves them to concrete types before LLVM ever sees them.

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

Briv does not dilute. The protocol graph keeps the source experience pure wine and lets the compiler become perfect water at the boundary — zero runtime translation, zero compromise on either side. The architecture's job is to keep these two domains separate: the programmer writes pure wine, the compiler emits perfect water.

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

A type does not "declare protocol membership." A type has operations mapped to certain types and protocols. The backend figures out the best layout for how the type should behave. Protocol declarations only define semantic compatibility edges — they don't know or care about concrete types.

### Fewer hops = less conversion

Declaring a variant closer to the target means fewer BFS hops:
- `#String<UTF16>` on Windows → identity path, zero conversion code
- `#String` → resolves through `UTF8 → UTF16` → one conversion edge
- Both compile correctly; the pinned variant gives the optimizer less work

### Primordial defaults vs prelude variants

Defaults (UTF8, IEEE754, unicode) are hardcoded in the parser — they work with `--no-stdlib`. Non-default variants (ASCII, Posit32, UTF16) are provided by the prelude plugin. This matches the intrinsics-vs-stdlib dividing line.

## Design

### Syntax — minimal by design

```briv
// Minimal — just edges, no contract
proto ASCII: #String {
    CastTo(#String<UTF8>);
    CastFrom(#String<UTF8>);
};

// With optional contract (recommended) using #Self hashword
proto ASCII: #String [forall(i in 0..#Self:>Size, #Self[i] < 128)] {
    CastTo(#String<UTF8>);
    CastFrom(#String<UTF8>);
};

// With an optional cross-variant op override
proto ASCII: #String {
    CastTo(#String<UTF8>);
    CastFrom(#String<UTF8>);
    op Add(#String<UTF8>) = add_UTF8_to_ASCII(#L, #R);
};
```

A protocol declaration is primarily about **edges**. Self-referencing ops (e.g. adding `ASCII` to `ASCII`) are never declared — they fall through to the default protocol via CastTo. Ops are only declared for **cross-variant overrides**: `op Add(#String<UTF8>) = fn(#L, #R)` means "if you ever need to add UTF8 directly to me, no need to walk the graph — this is the way." The self-variant is implicit (the protocol's own variant).

- `proto variant: #Category { ... }` — new top-level form using `proto` keyword. Follows the same `: #Category` pattern as `type Name: #Protocol { ... }`. The `proto` keyword is contextual (like `type`, `fn`, `node`), not reserved.
- `CastTo(#Other<Variant>)` / `CastFrom(#Other<Variant>)` — edges in the protocol graph
- `op Name(#TargetCategory<target_variant>) = fn(#L, #R)` — *optional* cross-variant override. Add maps to `+`, so `#L` is the left operand (the protocol's own variant) and `#R` is the right operand (the target variant). No self-referencing ops — those delegate to default.
- `[contract]` before the body — *optional* contract, recommended. Uses Briv expression syntax with `#Self` as a hashword referencing the value at the protocol boundary. `#Self` follows the PascalCase hashword convention (`#Int`, `#String`, `#Self`). Contracts are not required — a protocol without one trusts the programmer.
- No `layout` field — layout is implicit. Types that participate in a protocol declare their own layout. The protocol only defines *semantic compatibility*.

### Three tiers of effort

| Tier | What you write | What the compiler does |
|:---:|---|---|
| 0 — Default delegation | Just the type definition, no protocol declaration | Everything uses the primordial default (`UTF8`, `IEEE754`) |
| 1 — Edges only | `#String latin15 { CastTo(#String<UTF8>); ... };` | Ops fall through to default via CastTo. No custom op code needed. |
| 2 — Edges + contract (recommended) | Tier 1 + `[forall(...)]` before the body | Proof engine checks boundary crossings. Denies compilation on unprovable paths. |
| 3 — Edges + contract + custom ops | Tier 2 + `op Add(#String<UTF8>) = fn(#L, #R);` | Cross-variant ops use explicit binding. Contract still enforced. |

Tier 1 is the common case — most variants differ only in encoding/layout, not in operation semantics. Tier 2 adds safety with a compile-time enforceable invariant. Tier 3 is the escape hatch for genuinely different operation semantics.

### Graph semantics

- A variant "defined against" another (has `CastTo`/`CastFrom` edges) is automatically compatible
- Sharing a base category (`#String`) does NOT imply interoperability — edges must be explicitly declared
- `#Bits` remains universally reachable (existing invariant)
- BFS finds the shortest path through variant nodes
- Undefined self-ops (e.g. `Length` for `ASCII`) resolve by: CastTo default → run default op → CastFrom back. The compiler handles this transparently.
- If a protocol has a contract (`[expr]`), the proof engine checks it at every boundary crossing — data entering via CastTo and data exiting via CastFrom. If the contract cannot be proven at compile time for a given call site, compilation is denied. Without a contract, no boundary check is performed (trusts the programmer).

### Compiler proof — boundary enforcement

When a protocol has an optional contract `[expr]`, the proof engine checks it at every boundary crossing:

- **Entering** via CastTo: the incoming value must satisfy the contract
- **Exiting** via CastFrom: the outgoing value must satisfy the contract
- If unprovable at compile time for a given call site → **compilation denied**
- Without a contract, no boundary check — the cast is trusted

The `#Self` hashword in the contract refers to the value at the boundary crossing. For example:

```briv
proto ASCII: #String [forall(i in 0..#Self:>Size, #Self[i] < 128)] { ... }
```

The compiler does not need to know what "ASCII" means. It only knows: "data in this protocol must satisfy `forall(... < 128)`."

### Compiler proof of custom paths (aspirational)

When a user declares a cross-variant override like:

```briv
op Add(#String<UTF8>) = add_UTF8_to_ASCII(#L, #R);
```

The compiler should ideally verify equivalence:
- **Graph path**: `CastTo(#String<UTF8>)` on self → `Add(UTF8, UTF8)` → `CastFrom(#String<UTF8>)`
- **Custom path**: `add_UTF8_to_ASCII(#L, #R)`

If the symbolic proof engine (`symbolic.rs`) can determine these produce the same result, the custom declaration is validated. If they diverge, the compiler warns. This is an aspirational feature — the initial implementation accepts the user's declaration without proof.

## Current Architecture Gaps (Pre-Implementation)

Before the protocol declarations can be wired in, several gaps in the existing pipeline must be addressed. These are **prerequisites** that any implementation must handle.

### Gap 1: `Cast.#` properties are never injected into the type universe

Operator definitions (`op CastTo(#Category) = fn(...)`) on type bodies are collected into `operator_defs` at `src/compile.rs:576` and passed to the backend. But they are **never converted to `Cast.#` properties** on `ResolvedType` in `TypeUniverse`. This means:

- `find_cast_path()` in both `type_universe/operators.rs:131` and `layout_optimizer.rs:276` finds NO edges (only the implicit `#Bits` fallback)
- `find_protocol_category()` at `layout_optimizer.rs:192` returns None for all types
- `lookup_foreign_type()` at `frgn_dispatch.rs:226` always falls through to name-based matching
- Only `try_cast_protocol_path()` at `intrinsics.rs:1064` works — because it reads `operator_defs` directly

**Fix needed:** A pipeline step (or enhancement to the normalize stage) that injects `Cast.#` properties from `operator_defs` into the universe, so the BFS can find them.

### Gap 2: Proof engine is a stub

`prove_contract()` at `src/proof_engine/mod.rs:14` always returns `Ok(())`. The SMT solver integration at `src/proof_engine/smt.rs` exists but is never called. Contracts are **never enforced at compile time**.

**Fix needed:** Wire the proof engine for real — at minimum, evaluate symbolic contracts against known values at boundary crossings. The `symbolic.rs` executor works; it just needs to be connected via a non-stubbed path and given authority to deny compilation.

### Gap 3: Duplicate BFS implementations

Two nearly identical `find_cast_path` functions:
- `type_universe/operators.rs:131` — checks both `Cast.*` properties and `"op.Cast"` property
- `layout_optimizer.rs:276` — checks only `Cast.*` properties

**Fix needed:** Consolidate into one, ideally in `layout_optimizer.rs` or the new `protocol_graph.rs`.

### Gap 4: Normalizer strips non-LLVM properties

The LLVM normalizer at `src/backend/llvm/normalizer.rs:116-118` retains only `llvm_type` and `disamb` properties from the universe. Even if `Cast.#` properties were set before normalization, they would be stripped.

**Fix needed:** The normalizer must preserve `Cast.#` properties (and any other protocol-graph-relevant properties) through normalization.

## Implementation Phases

### Phase 0: Prerequisites (~200 lines across multiple files)

Before protocol declarations can work, fix the four gaps above:

1. **Inject `Cast.#` properties into the universe** — After operator_defs are collected (`compile.rs:576`), walk each type's operators. For each `op CastTo(#Category)` or `op CastFrom(#Category)`, set `rt.properties.insert(format!("Cast.#{}", cat), PropertyValue::Bool(true))` on the type's `ResolvedType` in the universe. Additionally, `TypeDef.protocol: Some("#Int")` implicitly implies `CastTo(#Int)` — register this edge too.

2. **Wire the proof engine** — In `prove_contract()` (`proof_engine/mod.rs:14`), replace the stub with actual symbolic evaluation via `symbolic.rs`. For simple cases (known values, simple predicates), use `eval_symbolic_expr()`. For complex cases, call `prove_smt_formula()`. Give it authority to return `Err("contract unprovable")`.

3. **Consolidate BFS** — Standardize on the `layout_optimizer.rs` version. Remove duplicate in `type_universe/operators.rs`. The single BFS will later accept an optional `ProtocolGraph` parameter.

4. **Preserve `Cast.#` in normalizer** — At `normalizer.rs:116-118`, change the property filter to retain keys starting with `"Cast."` alongside `llvm_type` and `disamb`.

### Phase 1: AST + Parser (~100 lines)

**AST additions (`src/ast/top.rs`):**

```rust
pub struct ProtocolDef {
    pub category: String,       // "String"
    pub variant: String,        // "ASCII"
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

**Lexer (`src/lexer.rs`):**

Add `#Self` as a recognized multi-character hash token before the general identifier regex `[a-zA-Z_#$][a-zA-Z0-9_#$]*`. Lexed as `Token::HashSelf` — it was reserved for this use case. At protocol-boundary checking time, `#Self` resolves to "the value at the boundary crossing." This follows the pattern of `#L`, `#R`, `#T` as positional markers.

**Parser (`src/parser/definitions.rs`):**

New arm in `parse_top_level()`: when `peek()` is `Token::Identifier("proto")`, consume `proto`, expect `identifier` for variant name, expect `Token::Colon`, parse optional category hashword `#Category`, parse optional contract `[expr]` using the existing contract parser (`parse_contract`), then parse `{ CastTo(...); CastFrom(...); op Name(#Target<Variant>) = fn(#L, #R); }` body. Return `TopLevel::ProtocolDef(...)`.

The `proto` keyword is contextual (like `type`, `fn`, `node`) — it's only special in top-level position. No reserved word needed.

Default primordial resolution in `src/parser/types.rs:40-48` is unchanged:
- `#String` → `HashWordVariant("#String", "UTF8")`
- `#Float` → `HashWordVariant("#Float", "IEEE754")`
- `#Char` → `HashWordVariant("#Char", "unicode")`
- All others → `HashWord(name)` (no variant)

### Phase 2: Protocol Graph (~180 lines)

**New file: `src/analysis/protocol_graph.rs`**

```rust
pub struct ProtocolGraph {
    /// (category, variant) → outgoing Cast edges
    edges: HashMap<(String, String), Vec<CastEdge>>,
    /// Pre-registered primordial defaults
    defaults: HashMap<String, String>,
    /// Cross-variant op overrides: (self_variant, op_name, target_variant) → function name
    cross_ops: HashMap<(String, String, String), String>,
    /// Optional contracts per variant: (category, variant) → Contract
    contracts: HashMap<(String, String), Contract>,
}
```

Methods:
- `new()` — register primordial defaults
- `build_from(items: &[TopLevel])` — scan for `ProtocolDef` items, insert edges, cross-variant ops, and contracts
- `find_protocol_path(source_cat, source_var, target_cat, target_var) -> Option<Vec<CastEdge>>` — BFS through variant-aware graph
- `find_default_delegation(cat, var) -> Option<Vec<CastEdge>>` — BFS from variant to default (for implicit self-op resolution)
- `get_contract(cat, var) -> Option<&Contract>` — retrieve contract for boundary enforcement
- `inject_edges(universe: &mut TypeUniverse)` — inject graph edges as `Cast.#` properties so the existing BFS can find them alongside operator_defs edges from Phase 0

**Integration (`src/analysis/layout_optimizer.rs`):**

`find_cast_path` gets an optional `&ProtocolGraph` parameter. When searching for neighbors:
1. Check universe `Cast.#` properties (existing, now populated from Phase 0 + Phase 2 injection)
2. If protocol graph present, query it for variant-aware edges
3. `#Bits` fallback (existing)

No existing arms modified. Protocol graph is additional data for the same BFS.

### Phase 3: Prelude Declarations (~15 lines)

In `plugins/parsed/prelude.bv` (or a companion file discovered by the same plugin system):

```briv
// Non-default protocol variants.
// Defaults (UTF8, IEEE754, unicode) are primordial — no declaration needed.
// These only declare Cast edges; self-ops implicitly delegate to defaults.

proto ASCII: #String {
    CastTo(#String<UTF8>);
    CastFrom(#String<UTF8>);
};

proto UTF16: #String {
    CastTo(#String<UTF8>);
    CastFrom(#String<UTF8>);
};

proto Posit32: #Float {
    CastTo(#Float<IEEE754>);
    CastFrom(#Float<IEEE754>);
};
```

`--no-stdlib` disables these. Defaults work regardless.

### Phase 4: Pipeline Integration — Boundary Enforcement (~100 lines)

**Pipeline placement:**

Boundary enforcement lives in the **frgn dispatch stage** (`compile.rs:333-350`), which is where the compiler already computes protocol paths for FFI and resolves cross-type conversions. This is the natural home: protocol boundaries and FFI boundaries are the same concept — data crossing from one protocol variant to another.

**Mechanism:**

After the protocol graph is built from Phase 2 and its edges are injected into the universe via Phase 2's `inject_edges()`, add a new sub-pass in `compile.rs` between the frgn dispatch loop and codegen:

1. Walk all resolved protocol crossings (CastTo/CastFrom) in the AST
2. For each crossing, look up the target variant's contract via `ProtocolGraph::get_contract()`
3. If a contract exists, call `prove_contract()` (now real from Phase 0) on the contract expression with `#Self` bound to the value at the boundary
4. If the proof fails or is unknown → **deny compilation** with a diagnostic showing the crossing site and the contract that was violated
5. If no contract exists → allow the crossing (trust the programmer)

**GLUE bridge (`src/analysis/frgn_dispatch.rs`, `src/glue/bridge.rs`):**

- `compute_protocol_path()` — try protocol graph if universe BFS fails, before bitcast fallback
- GLUE export — protocol graph supplements `lib/glue.toml`. TOML config wins when present

**Self-op resolution (implicit delegation):**

When the compiler encounters `#String<ASCII>` used with a self-referencing op (e.g. `Length`, `Add` to self) and no explicit binding exists on the variant:
1. Query `ProtocolGraph::find_default_delegation` for path to default variant
2. If the source variant has a contract, prove it at the CastTo boundary
3. Emit: `CastTo(default) → default_op → CastFrom(variant)`
4. LLVM optimizes the round-trip

This is transparent — no source-level declaration needed. The protocol graph resolves it.

**Cross-variant op overrides:**

When `op Add(#String<UTF8>) = add_UTF8_to_ASCII(#L, #R)` exists on `#String<ASCII>`:
- `#L` = left operand (the protocol's own variant, ASCII); `#R` = right operand (the target, UTF8). Add maps to `+`, so this follows the natural `left + right` ordering.
- Store on `ProtocolGraph.cross_ops` as `(ASCII, Add, UTF8) → add_UTF8_to_ASCII`
- When dispatching `Add(#String<ASCII>, #String<UTF8>)`, check for cross-op before walking graph
- The override is a direct declaration: "for this pair, use this path"
- If the source variant has a contract, it is still enforced at the boundary — the override bypasses the graph walk but not the invariant check

**Cast resolution pipeline (`src/backend/llvm/intrinsics.rs:945`):**

Existing 4-step pipeline, unchanged in structure. Step 2 (protocol path via `try_cast_protocol_path`) is updated to also consult the protocol graph when `operator_defs` has no entry for the pair:

1. Direct `op Cast(Target)` — existing
2. Protocol path — try `operator_defs` first (existing), then `ProtocolGraph` edges (new)
3. Meld shuffle — existing
4. Implicit `Cast(#Bits)` — existing

## Backward Compatibility

| Existing behavior | Impact |
|---|---|
| Parser defaults (`#String` → `UTF8`) | Unchanged |
| Type bodies with `op CastTo(#String<UTF8>)` | Unchanged — still sets `Cast.#` properties (now injected into universe via Phase 0) |
| `lib/glue.toml` protocol mappings | Unchanged — TOML still wins over graph |
| BFS path finding | Unchanged — graph is additional data |
| `--no-stdlib` | Primordial defaults still work |
| Existing tests | Phase 0 fixes (proof engine wiring, Cast.# injection) may cause previously-silent failures to surface — update tests accordingly |

## Testing

### Phase 0 (Prerequisites)
- `test_cast_properties_injected`: verify `Cast.#` properties appear in universe after operator_defs processing
- `test_proof_engine_denies`: verify `prove_contract()` returns Err for unprovable contract
- `test_proof_engine_accepts`: verify `prove_contract()` returns Ok for trivially true contract
- `test_normalizer_preserves_cast`: verify `Cast.#` properties survive normalization
- `test_bfs_consolidated`: verify single BFS finds edges from both universe and protocol graph

### Phase 1 (Parser)
- `test_protocol_def_edges_only`: `proto ASCII: #String { CastTo(#String<UTF8>); };` parses with empty cross_ops
- `test_protocol_def_cross_op`: `proto ASCII: #String { CastTo(#String<UTF8>); op Add(#String<UTF8>) = fn(#L, #R); };`
- `test_protocol_def_with_contract`: `proto ASCII: #String [forall(...)] { CastTo(...); };`
- `test_protocol_def_empty_body`: `proto ASCII: #String {};` — valid, just no edges
- `test_protocol_def_no_self_ops`: verify self-referencing ops like `op Length(#String<ASCII>)` are rejected (self-ops delegate to default, never declared)
- `test_hash_self_token`: verify `#Self` is lexed as `Token::HashSelf`, not `Identifier("#Self")`

### Phase 2 (Protocol Graph)
- `test_graph_single_edge`: UTF8 → ASCII path found
- `test_graph_multi_hop`: UTF8 → ASCII → UTF16 path found
- `test_graph_default_delegation`: ASCII → UTF8 delegation path found for implicit self-ops
- `test_graph_cross_op_lookup`: cross-variant override found for Add(ASCII, UTF8)
- `test_graph_contract_retrieval`: contract stored and retrievable for a variant
- `test_graph_no_path`: unrelated variants return None
- `test_graph_fallback_to_bits`: every variant reaches `#Bits`

### Phase 4 (Boundary Enforcement)
- `test_implicit_self_op_delegation`: `#String<ASCII>` uses Length without declaration — resolves through CastTo → default Length → CastFrom
- `test_cross_op_override`: Add between ASCII and UTF8 uses explicit binding instead of graph walk
- `test_boundary_contract_enforced`: protocol with contract denies compilation at boundary crossing when contract is unprovable
- `test_boundary_contract_allowed`: protocol with contract allows crossing when contract is provable
- `test_no_contract_allows_crossing`: protocol without contract allows crossing unconditionally
- `test_prelude_variants_available`: ASCII/UTF16 present with stdlib, absent with `--no-stdlib`
- `test_existing_cast_paths_unchanged`: old `Cast.#` properties still resolve

## Summary

| Phase | What | Files | Lines |
|:---:|---|:---|---:|
| 0 | Prerequisites (Cast.# injection, proof engine, BFS consolidation, normalizer fix) | Multiple | ~200 |
| 1 | AST + parser + `#Self` lexer token | `src/ast/top.rs`, `src/parser/definitions.rs`, `src/lexer.rs` | ~100 |
| 2 | Protocol graph + BFS integration | `src/analysis/protocol_graph.rs` (new), `layout_optimizer.rs` | ~180 |
| 3 | Prelude declarations | `plugins/parsed/prelude.bv` | ~15 |
| 4 | Pipeline integration — boundary enforcement | `compile.rs`, `intrinsics.rs`, `frgn_dispatch.rs`, `bridge.rs` | ~100 |
| — | Tests | Various test files | ~150 |
| | **Total** | | **~745** |

Zero new runtime intrinsics. Zero existing optimization paths modified. Zero TOML config changes. The protocol graph is just more data for the same BFS. LLVM never sees protocols — only concrete types and operations.