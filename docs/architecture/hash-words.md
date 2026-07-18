# Hash-Prefixed Compiler Words (`#words`)

Compiler-internal tokens prefixed with `#` that carry special meaning.
They are lexed as distinct tokens, never as identifiers.

## Current Words

| Token | Meaning | Used In |
|-------|---------|---------|
| `#L` | Left operand of `<-` | Strategy property bindings: `InsertAt <~ fn(#L, #R)` |
| `#R` | Right operand of `<-` | Strategy property bindings: `ExtractFrom <~ fn(#R)` |
| `#T` | Type parameter of generic collection | Strategy property bindings: `pop as #T` |

## Semantics

### In strategy property bindings (`<~`)

Resolved at codegen time by substituting the concrete operand:

| Marker | `queue <- value` (InsertAt) | `x <- &queue` (ExtractFrom) | `<- &queue` (Discard) |
|--------|-----------------------------|-----------------------------|----------------------|
| `#L` | handle register for `queue` | pop target register for `x` | void (no target) |
| `#R` | value register for `value` | handle register for `queue` | handle register for `queue` |
| `#T` | element type of collection | element type of collection | element type of collection |

The handle register is computed via `emit_addr_of` on the collection variable,
which produces a GEP into `%State` (for state fields) or an alloca address
(for let bindings). The `#R`/`#L` substitution is a register name pass-through
— the compiler resolves the expression to a register first, then substitutes.

### Rule

No `#`-prefixed word is ever a user-defined identifier. They are reserved
compiler vocabulary. Adding a new `#word` requires:
1. A new token in `src/lexer.rs`
2. Parser handling in the relevant context
3. Codegen resolution in the backend
4. Entry in this document

### Reserved

- `#Self` — reserved for future use (self-reference to the type definition)

## Non-word Hash Tokens

These `#` tokens exist but are NOT compiler words — they are syntax:

| Token | Purpose |
|-------|---------|
| `#` suffix on identifiers | Intrinsic marker: `Malloc#`, `Print#`, `Sqrt#` |
