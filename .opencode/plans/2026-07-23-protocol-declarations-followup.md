# Protocol Declarations — Definitive Follow-Up

**Date:** 2026-07-23
**Status:** Execution plan
**Depends on:** `docs/plans/2026-07-23-extensible-protocol-declarations.md`

## The Final Model

A protocol declaration defines **both** compatibility (edges) and transformation (functions).

```brief
proto ASCII: #String {
    // REQUIRED: Edge with binding — defines HOW layouts differ
    CastTo(#String<UTF8>) = ASCII_to_UTF8(#L);
    CastFrom(#String<UTF8>) = UTF8_to_ASCII(#L);

    // OPTIONAL: Skip the CastTo→op→CastFrom round-trip
    // Must be proven equivalent to the round-trip
    op Add(#String<UTF8>) = ASCII_add_with_UTF8(#L, #R);
};
```

### Rules

| Item | Requirement | What happens if violated |
|---|---|---|
| `CastTo`/`CastFrom` | MUST have a binding `= fn(#L)` | Compile error |
| Round-trip parity | Compiler proves `inverse(forward(x)) == x` via symbolic/SMT | Compilation denied |
| Cross-op equivalence | Compiler proves custom op == CastTo→default→CastFrom | Compilation denied |
| `#L`, `#R` | `#L` = self (protocol's own variant), `#R` = target variant | Convention, enforced by type checking |

### Transformations Are `defn`

The binding functions are `defn` declarations with bodies the compiler can inline:

```brief
defn ASCII_to_UTF8(x: #String<ASCII>) -> #String<UTF8> {
    // Bitwise ops — defines how the ASCII layout maps to UTF8
    // The compiler inlines this body for proof purposes
};

defn UTF8_to_ASCII(x: #String<UTF8>) -> #String<ASCII> {
    // Inverse — compiler proves round-trip identity
};

proto ASCII: #String {
    CastTo(#String<UTF8>) = ASCII_to_UTF8(#L);
    CastFrom(#String<UTF8>) = UTF8_to_ASCII(#L);
};
```

The compiler finds the `defn` body by searching items for a matching function name. If the body is available, it inlines forward + inverse, builds the composition, and proves via symbolic eval or SMT. If the body is not available (`frgn` or external), the proof is skipped with a warning.

The `#String<ASCII>` parameter types in the `defn` are elegant — they declare the protocol variant as the parameter type, making the relationship explicit and typed.

### What gets proven

```brief
// Given this protocol:
CastTo(#String<UTF8>) = ASCII_to_UTF8(#L);
CastFrom(#String<UTF8>) = UTF8_to_ASCII(#L);

// Proof 1: Round-trip identity
UTF8_to_ASCII(ASCII_to_UTF8(x)) == x

// Given this cross-op:
op Add(#String<UTF8>) = ASCII_add_with_UTF8(#L, #R);

// Proof 2: Equivalence to default-round-trip path
ASCII_add_with_UTF8(ASCII_value, UTF8_value)
    == UTF8_to_ASCII(ASCII_to_UTF8(ASCII_value) + UTF8_value)
```

---

## Implementation Changes

### Part A: Parser — CastTo/CastFrom with bindings

**Current state:** Parser handles `CastTo(#target);` (edge only). `op` supports `= fn(#L, #R)` bindings.

**Change:** In `parse_protocol_def()` (`src/parser/definitions.rs`), after parsing `CastTo(#target)` or `CastFrom(#target)`, check for `=` followed by a binding expression. If present, store it as `impl_args` on the edge.

```rust
// New AST for CastEdge:
pub struct CastEdge {
    pub direction: CastDirection,
    pub target_category: String,
    pub target_variant: String,
    pub binding: Option<CastBinding>,  // NEW: optional binding
}

pub struct CastBinding {
    pub param: String,  // "L" for #L
    pub expr: String,   // function name or expression
}
```

### Part B: Validation — Reject bare edges

In `compile.rs`, after protocol graph building, validate that every `ProtocolDef` has bindings on all CastTo/CastFrom edges:

```rust
for item in &items {
    if let TopLevel::ProtocolDef(pd) = item {
        for edge in &pd.cast_edges {
            if edge.binding.is_none() {
                return Err("CastTo/CastFrom must have a binding".into());
            }
        }
    }
}
```

### Part C: Round-trip proof

**Existing infrastructure:** `src/analysis/meld_validation.rs` — Layers 4 (symbolic) and 5 (SMT) prove
`inverse(forward(x)) == x`. Reuse the same pipeline.

**New function:** `verify_protocol_roundtrip(pd, items) → Result`
- Find matching CastTo/CastFrom pair with the same target
- Find `defn` body for each binding function via `find_defn_body()`
- If both bodies available:
  - Build composition: `CastFrom(CastTo(x))`
  - Run symbolic evaluation via `eval_symbolic_expr()`
  - If symbolic passes → return Ok
  - If symbolic inconclusive → build SMT formula, call Z3
- If bodies unavailable → warn and skip (functions are external/frgn)

**Helper:** `find_defn_body(name, items) → Option<Expr>` — scans `TopLevel::Definition` items for matching function name, returns its body expression.

### Part D: Cross-op equivalence proof

**New function:** `verify_crossop_equivalence(pd, items) → Result`
- For each cross-op (e.g., `op Add(#String<UTF8>) = ASCII_add_with_UTF8(#L, #R)`)
- Build default round-trip path: `CastFrom(CastTo(x) + y)` — inlines both binding defns and the default Add
- Build custom path: `ASCII_add_with_UTF8(x, y)` — inlines the cross-op defn
- Compare both paths via symbolic/SMT
- If not equivalent → compilation denied

### Part E: Follow-up plan update

**Prelude (`lib/std/protocols.bv`):**
```brief
// Non-default protocol variants with bindings.
// The bindings define actual transformations between layouts.

proto ASCII: #String {
    CastTo(#String<UTF8>) = ASCII_to_UTF8(#L);
    CastFrom(#String<UTF8>) = UTF8_to_ASCII(#L);
};
```

**Test files (`test_protocols/`):**
- `t1_basic.bv`: minimal protocol with CastTo/CastFrom + bindings
- `t2_roundtrip.bv`: round-trip proof passes
- `t3_crossop.bv`: cross-op with equivalence proof
- `t4_fail_contradiction.bv`: contradictory CastTo/CastFrom pair (proof fails → deny)

---

## Summary of Remaining Work

| Part | What | Files | Lines |
|---|---|---|---|
| A | CastEdge.binding field + parser | `src/ast/top.rs`, `src/parser/definitions.rs` | ~30 |
| B | Validate bindings exist | `src/compile.rs` | ~15 |
| C | Round-trip proof | `src/analysis/protocol_graph.rs` (new `verify*`) | ~60 |
| D | Cross-op equivalence | `src/analysis/protocol_graph.rs` | ~40 |
| E | Prelude + tests update | `lib/std/protocols.bv`, `test_protocols/*.bv` | ~25 |
| **Total** | | | **~170** |
