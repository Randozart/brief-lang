# Strategy Op Integration + Hash-Prefixed Compiler Words

## Overview

Integrate `InsertAt` / `ExtractFrom` strategy dispatch into the OP_CONFIG
template system using `#L`, `#R`, `#T` as positional marker tokens.

---

## Phase 1: Parser — Accept `#L`, `#R`, `#T` Tokens

**Files:** `src/lexer.rs`, `src/parser/definitions.rs`

### 1a. Lexer tokens

Add three new tokens to the lexer:

```rust
// src/lexer.rs
#[token("#L")] HashL,
#[token("#R")] HashR,
#[token("#T")] HashT,
```

### 1b. Property value parsing

Currently `parse_bracket_decl_body` at `definitions.rs:425` expects an identifier
after `<~`. Extend to also accept `HashL`, `HashR`, `HashT` as property values:

```rust
Some(Token::Identifier(id)) => { /* existing */ }
Some(Token::HashL) => { lhs.push(("insert".to_string(), PropertyValue::HashL)); }
Some(Token::HashR) => { lhs.push(("insert".to_string(), PropertyValue::HashR)); }
```

### 1c. PropertyValue enum

Add `HashL`, `HashR`, `HashT` variants to `PropertyValue` in `src/ast/top.rs`.

---

## Phase 2: Property Resolution + Substitution

**Files:** `src/backend/llvm/emit_stmt.rs`, `src/backend/llvm/emit_toplevel.rs`

### 2a. Strategy dispatch

Replace the hardcoded `match strat.as_deref() { Some("ring_push") => ... }` with:

```rust
// Look up the strategy from the type property
let strat = match backend.check_insert_strategy(target) {
    Some(PropertyValue::HashL) => /* #L = collection handle */,
    Some(PropertyValue::HashR) => /* #R = value */,
    None => { /* regular store */ },
};
```

### 2b. Substitution logic

For `InsertAt <~ ring_push(#L, #R)`:
- `#L` → `emit_addr_of(target)` → handle register
- `#R` → the value register (already computed)

For `ExtractFrom <~ ring_pop(#R)`:
- `#R` → `emit_addr_of(source)` → handle register

For `as #T`:
- Resolves to the concrete element type from the collection's type parameter
- Used in generic strategies that need element size/width

---

## Phase 3: Deprecate `emit_ring_push`/`emit_ring_pop` + Remove Intrinsics

**Files:** `src/backend/llvm/emit_stmt.rs`, `src/backend/llvm/intrinsics.rs`

### 3a. Remove inline GEP functions

Delete `emit_ring_push` and `emit_ring_pop` from `emit_stmt.rs`. The strategy
dispatch now calls the Briv function definition directly.

### 3b. Remove RingPush/RingPop intrinsics

Delete `Intrinsic::RingPush` and `Intrinsic::RingPop` from `intrinsics.rs`.
These are no longer needed — the strategy dispatch emits `call @ring_push(...)`
which LLVM -O3 inlines.

---

## Phase 4: Documentation

### 4a. Architecture note — Hash-prefixed compiler words (`#words`)

New file: `docs/architecture/hash-words.md`

Document that `#` prefix denotes compiler-internal words:
- `#L` — left operand
- `#R` — right operand  
- `#T` — type parameter
- Future: `#Self` reserved

### 4b. Update strategy docs

Update `docs/architecture/arrow-syntax-and-arena.md` with the new op-based
strategy dispatch.

---

## Files Changed

| File | Change |
|------|--------|
| `src/lexer.rs` | Add `HashL`, `HashR`, `HashT` tokens |
| `src/ast/top.rs` | Add `PropertyValue::HashL/R/T` variants |
| `src/parser/definitions.rs` | Parse `#L`/`#R`/`#T` in property values |
| `src/backend/llvm/emit_stmt.rs` | Strategy dispatch via property values |
| `src/backend/llvm/emit_toplevel.rs` | `check_insert_strategy` returns PropertyValue |
| `src/backend/llvm/intrinsics.rs` | Remove RingPush/RingPop |
| `docs/architecture/hash-words.md` | NEW — hash-prefixed compiler words |
| `docs/architecture/arrow-syntax-and-arena.md` | Update for op-based dispatch |
