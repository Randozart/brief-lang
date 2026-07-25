# Protocol Declarations — Definitive Follow-Up

**Date:** 2026-07-23
**Status:** Execution plan
**Depends on:** `docs/plans/2026-07-23-extensible-protocol-declarations.md`

## The Final Model

A protocol declaration defines **both** compatibility (edges) and transformation (functions).

```brief
proto ascii: #String {
    // REQUIRED: Edge with binding — defines HOW layouts differ
    CastTo(#String<utf8>) = ascii_to_utf8(#L);
    CastFrom(#String<utf8>) = utf8_to_ascii(#L);

    // OPTIONAL: Skip the CastTo→op→CastFrom round-trip
    // Must be proven equivalent to the round-trip
    op Add(#String<utf8>) = ascii_add_with_utf8(#L, #R);
};
```

### Rules

| Item | Requirement | What happens if violated |
|---|---|---|
| `CastTo`/`CastFrom` | MUST have a binding `= fn(#L)` | Compile error |
| Round-trip parity | Compiler proves `inverse(forward(x)) == x` via symbolic/SMT | Compilation denied |
| Cross-op equivalence | Compiler proves custom op == CastTo→default→CastFrom | Compilation denied |
| `#L`, `#R` | `#L` = self (protocol's own variant), `#R` = target variant | Convention, enforced by type checking |

### What gets proven

```brief
// Given this protocol:
CastTo(#String<utf8>) = ascii_to_utf8(#L);
CastFrom(#String<utf8>) = utf8_to_ascii(#L);

// Proof 1: Round-trip identity
utf8_to_ascii(ascii_to_utf8(x)) == x

// Given this cross-op:
op Add(#String<utf8>) = ascii_add_with_utf8(#L, #R);

// Proof 2: Equivalence to default-round-trip path
ascii_add_with_utf8(ascii_value, utf8_value)
    == utf8_to_ascii(ascii_to_utf8(ascii_value) + utf8_value)
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

**Existing infrastructure:** `src/analysis/meld_validation.rs` already proves `inverse(forward(x)) == x` via Layer 4 (symbolic) and Layer 5 (SMT). The same pipeline applies here:

1. Register each `CastTo` + `CastFrom` pair as a meld-like round-trip
2. Call existing `validate_symbolic()` and `validate_smt()` on the pair
3. Deny compilation if proof fails

**New function:** `verify_protocol_roundtrip(pd: &ProtocolDef) -> Result<(), String>`
- Extract matching `CastTo`/`CastFrom` pairs by target
- For each pair, run `validate_symbolic()` then `validate_smt()`
- Return Err if either proof fails

### Part D: Cross-op equivalence proof

**New function:** `verify_crossop_equivalence(pd: &ProtocolDef) -> Result<(), String>`
- For each cross-op (e.g., `op Add(#String<utf8>) = ascii_add_with_utf8(#L, #R)`)
- Build the round-trip path: `CastFrom(ascii_to_utf8(x) + y)`
- Compare with the custom implementation: `ascii_add_with_utf8(x, y)`
- Run symbolic evaluation to check equality

### Part E: Follow-up plan update

**Prelude (`lib/std/protocols.bv`):**
```brief
// Non-default protocol variants with bindings.
// The bindings define actual transformations between layouts.

proto ascii: #String {
    CastTo(#String<utf8>) = ascii_to_utf8(#L);
    CastFrom(#String<utf8>) = utf8_to_ascii(#L);
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
