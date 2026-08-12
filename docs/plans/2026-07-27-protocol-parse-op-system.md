# Phase 2.8 — Protocol Cleanup + Parse Discriminator System

**Date:** 2026-07-27

## 1. Overview

Complete the `op Parse` discriminator system with `pre:`, `suf:`, and `reg:` fields,
clean up the 8 core protocols, and fix the remaining 2 benchmark SKIPs + 1 MISMATCH.

## 2. Workstream A — Protocol Cleanup

Define the 8 core protocols in `lib/std/types/bootstrap.bv`:

| Protocol | Parses | Canonical form | Bindings |
|----------|--------|---------------|----------|
| `#Int` | `Int`, `UInt`, `Int8`–`Int64` | `Decimal` | `Add`, `Sub`, `Mul`, `Div`, `Eq`, `Lt`, `Gt`, `Parse` |
| `#UInt` | `UInt` variants | `Decimal` | (inherits from Int via `type UInt: Int`) |
| `#Float` | `Float`, `Double` | `Decimal` | `Add`, `Sub`, `Mul`, `Div`, `Eq`, `Parse` |
| `#Bool` | `Bool` | `Bare` | `Eq`, `And`, `Or`, `Not`, `Parse` |
| `#String` | `String` | `Quoted` | `CastTo`, `CastFrom`, `Parse`, `prop Size`, `prop Bytes` |
| `#Char` | `Char` | `Quoted` (single) | `Eq`, `Parse` |
| `#Ptr` | `Ptr<T>` | — | — |
| `#Void` | `Void` | — | — |

No `#Bits` protocol. Bits is the primitive — the `#Int` protocol is the default for bit-based types.

### Protocol inheritance for parse ops

Resolution walks UP the type chain (child → parent). Each type's parse ops shadow parent ops with the same `(pre, suf, reg)` signature. Sibling ambiguity = error.

### Files (3)

| File | Change |
|------|--------|
| `lib/std/types/bootstrap.bv` | 8 core protocols with `op Parse(Decimal/Quoted/Bare)` |
| `lib/std/types/float.bv` | Subtypes get their unique `suf:"h"`, `suf:"bf"` rules |
| `src/typechecker/mod.rs` | Update `matches_parse_identity` for 8 core protocols |

## 3. Workstream B — Parse Discriminator System

### 3a. AST — Add fields to `OperatorBinding`

`src/ast/top.rs`:
```rust
pub struct OperatorBinding {
    pub name: String,
    pub protocol_variant: Option<String>,
    pub pre: Option<String>,      // "0x", "sql", etc.
    pub suf: Option<String>,      // "f", "h", "bf", "km", etc.
    pub reg: Option<String>,      // regex for literal matching
    pub expr: Expr,
    pub span: Option<Span>,
}
```

### 3b. Parser — `parse_op_definition` with discriminators

`src/parser/definitions.rs`:

After the op name `Add`, before `:`, parse optional key-value pairs:
- `pre:"0x"` → `pre = Some("0x")`
- `suf:"f"` → `suf = Some("f")`
- `reg:"[0-9a-fA-F]+"` → `reg = Some("...")`

Syntax:
```briev
type Int: #Int {
    op Parse(Decimal): parse_dec(#L);
};
type Half: Float {
    op Parse(Decimal, suf:"h"): to_f16(#L);
};
type HexInt: #Int {
    op Parse(Decimal, pre:"0x", reg:"[0-9a-fA-F]+"): parse_hex(#L);
};
```

### 3c. Expression parser — suffix peek-ahead

`src/parser/expressions.rs`:

After parsing a literal (`Decimal`, `Float`, `Quoted`, `Identifier`), check:
1. Is the next token an `Identifier` that looks like a suffix (lowercase, short)?
2. Was there no whitespace between the literal and the identifier?
3. If yes → construct `Expr::TaggedLiteral(val, suf_str)` instead of returning early

### 3d. Type checker — parse op resolution

`src/typechecker/mod.rs`:

**Registration:** On `TypeDef` items, call `register_parse_ops` which filters `op_bindings` for `name == "Parse"` and stores them in `parse_ops` map.

**Resolution (`find_parse_op`):**
1. Walk type hierarchy (type → parent → grandparent)
2. For each type, collect all `op Parse` entries matching the literal form
3. Filter by `pre`: literal starts with prefix
4. Filter by `suf`: literal ends with suffix
5. Filter by `reg` (if specified): `regex.is_match(&literal_text)` at compile time
6. If zero match → return `None` (fallthrough)
7. If one match → return it
8. If multiple match → error: "ambiguous literal, add explicit type annotation"

**`matches_parse_identity`:** Map `#Int`/`#UInt` → `Decimal`, `#Float` → `Decimal`, `#String` → `Quoted`, `#Bool` → `Bare`.

### 3e. Files (5)

| File | Change |
|------|--------|
| `src/ast/top.rs` | Add `pre`, `suf`, `reg` to `OperatorBinding` |
| `src/ast/expr.rs` | (no change needed — `TaggedLiteral` exists) |
| `src/parser/definitions.rs` | `parse_op_definition` parses pre/suf/reg key-value pairs |
| `src/parser/expressions.rs` | Suffix peek-ahead after literals |
| `src/typechecker/mod.rs` | Wire `register_parse_ops`, update `find_parse_op`, inheritance walk, ambiguity detection |

## 4. Workstream C — Fix Remaining Benchmarks

| File | Change |
|------|--------|
| `benchmarks/build_and_bench.sh` | One-step clang linking with `lib/runtime/briev_rt.c` |
| `src/backend/llvm/loop_engine/counter.rs` | phi type `"i64"` for all counter fields |

## 5. Coding Standards

Per AGENTS.md Plan Directives:
- **FLAT CONTROL FLOW:** Max 2 nesting levels, guard clauses, early returns
- **COMMENT THE CODE:** `// YYYY-MM-DD: <why>` on every modified site
- **UPDATE ALL EXAMPLES:** All `.bv` files using `op Parse` updated
- **DOCUMENTATION IS CODE:** Update docs in same commit
- **BEHAVIORAL TESTS:** Test parse op resolution outcomes, not IR snapshots

## 6. Risks

| Risk | Mitigation |
|------|------------|
| `regex` crate not in dependencies | `cargo add regex` if needed (check first) |
| Suffix peek-ahead consumes identifiers incorrectly | Only consume if no whitespace between literal and suffix token |
| Inheritance chain walk misses some types | Test with `Half: Float -> type Half: Float { op Parse(Decimal, suf:"h"): ... }` |
| Ambiguity false positive for sibling types | Error is correct — user adds explicit type annotation to disambiguate |

## 7. Implementation Order

1. **B1:** Add `pre`/`suf`/`reg` to `OperatorBinding` in AST
2. **B3:** Update `parse_op_definition` to parse discriminator key-value pairs
3. **B4:** Add suffix peek-ahead in expression parser
4. **A1–A3:** Clean up protocol declarations
5. **B5–B6:** Wire type checker resolution
6. **C1–C2:** Fix remaining benchmarks
7. **Commit + test**
