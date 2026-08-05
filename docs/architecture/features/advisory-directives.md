# Advisory Directives (`#?`)

**Date added:** 2026-06-18
**Phase:** 1–2 — Lexer/Parser/AST infrastructure + LLVM metadata mapping

---

## Purpose

The `#?` prefix on hashtag directives transforms a compiler command into
a **speculative hint**. The developer expresses intent ("I think this
should be inlined / vectorized / offloaded to GPU"), and the compiler
evaluates mechanical feasibility and cost-benefit tradeoffs before
deciding.

---

## Syntax

Briv has three directive modes:

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

## LLVM Metadata Mapping (Phase 2)

The `directive.rs` module (`src/backend/llvm/directive.rs`) centralizes the
mapping from directive hashtags to LLVM IR annotations.

### `DirectiveCtx`

```rust
pub enum DirectiveCtx {
    Transaction,    // reactive txn → function attr
    CallableTxn,    // callable txn/defn → function attr  
    Loop,           // foreach or counted loop → loop metadata
    Body,           // general guarded body
}
```

### `DirectiveEffect`

```rust
pub enum DirectiveEffect {
    FunctionAttribute(String),  // e.g. "alwaysinline"
    LoopMetadata(String, String),  // e.g. ("llvm.loop.unroll.full", "")
    None,
}
```

### Integration Points

| Location | Context | Directives |
|----------|---------|------------|
| `emit_toplevel.rs:emit_transaction()` | Transaction | `#inline` → `alwaysinline`, `#?inline` → `inlinehint` |
| `emit_toplevel.rs:emit_callable_txn()` | CallableTxn | `#inline` → `alwaysinline`, `#?inline` → `inlinehint` |
| `foreach.rs:emit_llvm()` | Loop | `#unroll` → `!llvm.loop.unroll.full`, `#?unroll` → `!llvm.loop.unroll.enable` |
| `foreach.rs:emit_llvm()` | Loop | `#vectorize`/`#?vectorize` → `!llvm.loop.vectorize.enable = true` |

### Foreach Modifiers

`Statement::Foreach` now carries `modifiers: Vec<Hashtag>` (added to the
AST, parser, and all match sites). The `ForeachStmt` codegen struct
includes the modifiers and resolves them via `resolve_directives()` in
the `Loop` context.

### Default Behavior

When no vectorize directive is present on a foreach loop, the codegen
still emits `!llvm.loop.vectorize.enable = true` (matching pre-existing
behavior). Loop directives are additive — they do not disable the
default vectorization unless explicitly overridden.

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
