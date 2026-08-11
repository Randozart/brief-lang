# Phase 2b — Component mounting (SPEC 21.3, first slice: view-fragment mounts)

**Date:** 2026-08-11
**Status:** slice 1 (view-fragment mounting) implemented. Slice 2 (per-instance
state) open.

## Slice 1 implementation summary (2026-08-11)

- `compile_view` collects `render Name { ... }` blocks into a name→html map;
  the ViewCompiler's `inject_ids_inner` splices `<Name />` (self-closing and
  `<Name>...</Name>` paired forms) with the fragment's HTML, processed
  recursively (shared id_counter → unique element IDs per mount).
- Cycle detection (A→B→A) is a compile-time error; unknown PascalCase tags
  still warn.
- Verified E2E: `examples/counter.rbv` mounts `<Counter />` → fragment HTML
  live with injected IDs, buttons trigger `_txn("increment"/"decrement"/"reset")`,
  the span binds `count`; two mounts produce two unique-ID spans that both
  react; `<Outer><Inner/></Outer>` nesting works.
- Tests: 5 new (view_compiler). Suite 1757 lib + 14 bin green.

## Problem

`<Counter />` tags currently compile with a warning and render inert. The
`render Counter { ... }` block is parsed but never consumed by the view
pipeline: when a `<view>` block exists, `compile_view` uses only the `<view>`
HTML and ignores the render blocks entirely. SPEC 21.3 says custom component
tags create first-class reactive instances; the flagship `examples/counter.rbv`
uses a `render Counter {}` fragment whose directives reference the GLOBAL state
(`let count`, `txn increment`), mounted via `<Counter />`.

## Semantics (slice 1)

- A `render Name { <html> }` block is a reusable view fragment.
- `<Name />` in the view MOUNTS the fragment's HTML at that position — the
  fragment is inlined at view-compile time, its elements get injected IDs, and
  its directives (b-text/b-trigger/b-when/b-show/b-class/…) become ordinary
  view bindings wired to whatever state the fragment's expressions name.
- Multiple `<Name />` mounts inline the fragment multiple times; each mount's
  elements get unique IDs, and each reacts to the same referenced (global)
  state — matching counter.rbv's current semantics. Mounting = the fragment's
  DOM is live; `b-when` inside the fragment mounts/unmounts it structurally.
- **Per-instance state (each mount owns its own field slots; component
  transactions route per instance) is slice 2** — the WASM runtime has no
  mount registry and state is one global `%State`. Explicitly out of scope;
  a component whose directives reference state the design intends to be
  per-instance keeps the current (global) semantics until slice 2.

## Design decisions

1. **Compile-time inlining, no runtime registry.** `compile_view` collects
   `render Name` blocks into `name → html`. The ViewCompiler replaces each
   `<Name />` tag with the fragment's HTML and re-processes it (shared ID
   counter → unique element IDs). This is the "compiler teaches, stdlib
   learns" shape — no WASM mount registry needed for slice 1.
2. **Nesting + cycle guard.** A fragment may itself contain `<Name2 />`
   (recursion via the same path). A cyclic render chain (A→B→A) is a
   compile-time error, never infinite inlining.
3. **Unknown PascalCase tags still warn** (Phase 2b2 / user error) — a known
   render block mounts, an unknown one warns, never silent dead DOM.
4. **Element ID uniqueness across mounts.** The shared `id_counter` in
   `inject_ids` already guarantees unique `rbv-*` ids; the spliced fragment
   flows through the same code path.
5. **Bindings fan out per mount** — two `<Counter />` mounts produce two
   bindings on the same state handle; the Phase 2a `_registerViewEffect`
   per-handle lists make both react (already landed).

## Implementation order

1. `compile_view`: collect `RenderBlock`s into a name→html map; pass to the
   ViewCompiler.
2. `ViewCompiler`: `render_blocks` field + `set_render_blocks`; `inject_ids`
   splices known `<Name />` tags (recursion + cycle guard); `warn_component_tags`
   only for unknown tags.
3. Tests: fragment inlining (single mount), multiple mounts (unique IDs, both
   bind), nesting, cycle error, unknown-tag warning preserved.
4. Verify: `cargo test --lib`, `cargo test --bin brivc`, Praetor, E2E
   `examples/counter.rbv` (mounts `<Counter />` → fragment HTML live, buttons
   call the txns, span binds `count`).

## Documentation updates

- `spec/SPEC.md` §21.3: note that a `render Name` block is a reusable view
  fragment and `<Name />` mounts it (slice 1); per-instance state is a
  follow-up.
- `src/view_compiler.rs` + `src/compile.rs` doc comments: remove the "rendered
  inert" note for known components.
- `BUGS.md`: new entry for "component tags render inert" (resolved as this
  lands; per-instance state remains open).
- `docs/plans/2026-08-11-phase2-component-model.md`: mark 2b slice 1 done.

## Baselines

Feature work (webstack-only) — no benchmark baseline required. x86_64 codegen
untouched.
