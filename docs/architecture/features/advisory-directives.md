# Advisory Directives (`#?`)

**Date added:** 2026-06-18
**Phase:** 1 — Lexer/Parser/AST infrastructure

---

## Purpose

The `#?` prefix on hashtag directives transforms a compiler command into
a **speculative hint**. The developer expresses intent ("I think this
should be inlined / vectorized / offloaded to GPU"), and the compiler
evaluates mechanical feasibility and cost-benefit tradeoffs before
deciding.

---

## Syntax

Brief has three directive modes:

| Prefix | Name | Semantics |
|--------|------|-----------|
| `#tag` | Advisory | "Do this if supported, warn if not" |
| `#!tag` | Mandatory | "Do this or fail compilation" |
| `#?tag` | Speculative | "Consider this, evaluate cost, explain your decision" |

The `#?` prefix is lexed as a single `HashQuestion` token (not `Hash` +
`Question`). It is handled in `parse_hashtag_modifiers()` before the
`Hash` arm to ensure greedy matching.

---

## AST Representation

In `src/ast.rs`, the `Hashtag` struct gains a `speculative: bool` field:

```rust
pub struct Hashtag {
    pub name: String,
    pub value: Option<String>,
    pub mandatory: bool,
    pub speculative: bool,   // true for #?tag
    pub fallback: Vec<String>,
    pub scoped: Option<String>,
}
```

Constructor helpers:
- `Hashtag::new(name)` → `speculative: false`
- `Hashtag::mandatory(name)` → `speculative: false`
- `Hashtag::speculative(name)` → `speculative: true`, `mandatory: false`

---

## Validation

In `src/backend/mod.rs`, `validate_single_hashtag()` handles speculative
tags as always-advisory: even if the tag name is unrecognized, it maps to
`UnsupportedAdvisory` rather than `UnsupportedMandatory`. This means a
`#?gpu` on a CPU-only backend emits a warning, not an error.

The `supported_hashtags()` registry was extended for the `llvm` backend
with the new directive names: `inline`, `unroll`, `vectorize`, `gpu`.

---

## Evaluation

Speculative directives produce **optimization remarks** (see
`optimization-remarks.md`) explaining the compiler's decision:
- `#?inline` → "inlined (function size 14 ≤ threshold 25)" or "not inlined (cycles detected)"
- `#?vectorize` → "vectorized (8-lane SIMD)" or "not vectorized (loop-carried dependency)"
- `#?unroll` → "unrolled (factor 4)" or "not unrolled (trip count too small)"
- `#?gpu` → "offloaded to GPU" or "CPU retained (arithmetic intensity too low)"

---

## Per-backend notes

| Backend | `#?` support | Notes |
|---------|-------------|-------|
| LLVM | Full | Phase 2 maps directives to LLVM metadata |
| Webstack | Speculative tags ignored with warning | No inline/vectorize/unroll in WASM |
| CIRCT | Speculative tags ignored with warning | Hardware backend — no GPU offload |
| Dead backends | Speculative tags silently ignored | No fixes needed |

---

## Testing

- Lexer: `#?inline` → `[HashQuestion, Identifier("inline")]`
- Parser: `#?gpu(1024)` → speculative Hashtag with value
- Validation: `#?unknown` on C backend → `UnsupportedAdvisory` (not error)
