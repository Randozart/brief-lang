# `:= ref_fn` — Reference Function via Reused Token

Date: 2026-07-29
Status: Plan → Implementation

## Reasoning

The `verifying` keyword approach had a fundamental problem: it required a new
lexer token, and the `.derive.bv` re-parse failed because `verifying` appeared
as a top-level token after function definitions.

Using `:= ref_fn` solves this because:
1. `:=` is already a valid token — no lexer changes needed
2. `parse_derivation_block` already consumes `:=` — the second `:= ref_fn`
   is consumed inside the same function, never appearing at top level
3. The disambiguation is clean: `:= {` is examples, `:= <ident>` is ref

## Syntax

Three forms, all using `:=`, no new keywords:

| Form | Example |
|------|---------|
| Examples only | `:= { 0 -> 0; 1 -> 1; }` |
| Reference only | `:= popcount_ref` |
| Both (either order) | `:= { 0 -> 0; } := popcount_ref` or `:= popcount_ref := { 0 -> 0; }` |

The `:= ref_fn` clause uses the existing `DerivationBlock.ref_name` and
`DerivationBlock.ref_tolerance` fields — no AST changes needed.

## Implementation

### 1. Parser — `parse_derivation_block` (definitions.rs)

Current flow:
```
eat(ColonEq) → expect(LBrace) → parse examples → expect(RBrace) → [;] → [contract] → [verifying]
```

New flow:
```
// First :=
eat(ColonEq)
if check(LBrace):
    parse examples (existing)
    eat optional ;
else:
    parse reference name from identifier
    parse optional tolerance [tol: N]

// Optional second :=
if eat(ColonEq):
    if check(LBrace):
        parse examples
    else:
        parse reference name + tolerance

eat optional ;
return DerivationBlock { examples, ref_name, ref_tolerance }
```

### 2. Reference-only derivation (no examples)

When `ref_name` is set but `examples` is empty:
- Skip synthesis entirely
- Use the reference function's body as the synthesized body (clone)

This is handled in `cli.rs::handle_derive_command` — check if the derivation
block has `ref_name` but empty `examples`, and if so, copy the reference body
instead of calling `synthesize`.

### 3. Clean up — remove `verifying`

- Remove `Token::Verifying` from lexer
- Remove `Token::Verifying` Display impl
- No more `verifying` handling in `parse_derivation_block`

## Files

| File | Change |
|------|--------|
| `src/lexer.rs` | Remove `Token::Verifying` |
| `src/parser/definitions.rs` | Rewrite `parse_derivation_block` for dual `:=` |
| `src/derive/cli.rs` | Handle reference-only (skip synthesis, copy ref body) |

All other files (`doppelganger.rs`, `assert.rs`, `helpers.rs`, `mod.rs`,
`verify.rs`, `verify_smt.rs`) already have `ref_name`/`ref_tolerance` fields
and `ref_fn` parameters — no changes needed there.
