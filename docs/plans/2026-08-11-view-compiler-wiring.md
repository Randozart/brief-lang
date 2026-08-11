# Wire the ViewCompiler into compile_source (Phase 1 root-view wiring)

**Date:** 2026-08-11
**Status:** Phase 1 implemented (root-view wiring). Phase 2 component model open.

## Phase 1 implementation summary (2026-08-11)

- `compile_source` calls `compile_view()` before codegen — ViewCompiler over the
  preprocessed `.rbv` `<view>` block, falling back to concatenated
  `RenderBlock.view_html` (`.bv`). Result cached in `CompiledView`
  (`modified_view_html`/`view_bindings`/`view_warnings`).
- `view_root_signals()` derefs `.^Size`/`.^Len`/property projections to the root
  field; threaded into `codegen(.., view_signals)`; webstack live arm sets
  `b.ctx.view_bound_fields` → `apply_field_modes` marks them `FieldMode::Always`
  (observability-as-liveness: dead-field elimination was pruning read-only view
  fields, leaving dangling handles).
- `verify_srbv` runs only under `conformance::is_strict(path)` — the `.s` strict
  profile (`ui.s.rbv`), per SPEC §3.2. Plain `.rbv`/`.bv` surface diagnostics as
  warnings. SRBV001 = undefined signal reference, hard error in strict mode.
- `b-if` rejected with a hard error naming SPEC 21.4 (`b-when` is the way).
- `RbvFile::parse` rejects `<script` only at line start — a `//` comment
  mentioning `<script>` must not be treated as markup.
- `extract_trigger_value_from_tag` quote-bound panic fixed.
- web_generator `field_handle_for_signal` resolves the root field handle; Text
  applyFn projects reflections inline (`.^Size`→`.length`, `.^Len`→`.length`,
  other `.^X` → `[".<lowercase>"]` property access).
- Component tags (`<Counter />`) → `warn_component_tags` warning, inert render.
- Tests: 6 in compile.rs + 1 in rbv.rs. Full suite 1740 lib + 12 bin green.

## Problem

The `ViewCompiler` (src/view_compiler.rs) and `verify_srbv` have zero non-test
callers. `BuildOptions.view_bindings` is always `vec![]` and `opts.view_html` is
never populated — only the `.rbv` `<view>` block reaches `index.html`. As a
result, a `--backend webstack` build of `todo.rbv` writes `index.html` but **no
`dom-shim.mjs`** (the emit gate `!frgn_decls.is_empty() || !view_bindings.is_empty()`
fails without `#Web` frgn declarations). Even when the shim is emitted (e.g. via
`#Web` imports), its `getElementById` calls are dead because `index.html` carries
the raw view markup without the ViewCompiler's injected element IDs.

This plan wires the phase-by-basics root-view path end-to-end: `.rbv`/`.bv` view
markup → ViewCompiler (IDs injected, bindings extracted, `b-*` directives
validated per SPEC 21.4) → SRBV reference verification → dom-shim emission.

**Full component model (SPEC 21.3: mountable `<Name />` instances, b-when
structural mount/unmount, b-each+b-key reconciliation, b-bind:value,
per-instance state) is Phase 2 — the WASM runtime has no mount registry and the
state model is a single global `%State`. Explicitly out of scope here; unknown
component tags produce a compile-time warning, not silence or dead DOM.**

## Design decisions

1. **View HTML precedence** (inside the webstack output block, compile.rs:818):
   `opts.view_html` (CLI) → `.rbv` `<view>` (`preprocessed.view_html`) →
   concatenated `TopLevel::RenderBlock.view_html` from `items` (`.bv`).
2. **Signals/transactions registration**: walk `items` for
   `StateDecl`/`Cell`/`Object` field names (signals) and
   `Transaction`/`Definition` names (transactions) so
   `validate_user_triggered_preconditions` can lint user-triggered txns.
3. **Surface diagnostics — never silent (SPEC 21.4)**. `compile()` returns
   validation_errors followed by info diagnostics in one Vec; split them by
   `validation_errors.len()`:
   - validation_errors (rejected directive) → **hard error**.
   - remaining diagnostics → stderr warnings.
   - `verify_srbv` SRBV001/002 → **hard error** (undefined signal / trivial
     contract referenced by the view).
4. **ID injection is load-bearing**: write `compile()`'s `modified_html` into
   `index.html` AND pass it to `ssr::render_ssr`. Without the injected IDs every
   `document.getElementById(el)` in the shim returns null.
5. **Projection deref**: view expr `items.^Size` normalizes to its root field
   (`items`) for `field_handle_for_signal` lookup; the `b-text` applyFn projects
   inline in JS (`.^Size` → `value.length`; `.^Len`/`.^Ptr` already exist as
   reflection). Precisely: strip `.<refl>` suffix chains down to the path head.
6. **Unknown `<Name />` component tags** inside the view: left as inert DOM in
   the output but emit a compile-time warning naming the tag and stating mount
   wiring arrives with the Phase 2 component plan (SPEC 21.3).
7. **Dom-shim gate**: emit the shim whenever bindings are non-empty; drop the
   dead-fallback stub `StateLayout` (real layout is always captured by the
   webstack codegen arm via `web_layout`).

## Work items

1. Plan doc (this file).
2. `compile.rs` webstack block: collect view html, register signals/txns, run
   ViewCompiler, split+surrender diagnostics, verify_srbv, thread
   `bindings`/`modified_html` into `opts.view_bindings` + `index.html` + SSR.
3. `web_generator.rs`: root-field deref in `field_handle_for_signal` +
   projection-aware inline JS for the Text applyFn.
4. `view_compiler.rs`: unknown-component-tag detection → diagnostic.
5. Tests: compile-level `.rbv` (todo shape), `.bv` render-block shape,
   SRBV001 hard error, b-if validation hard error, `.^Size` deref, unknown tag
   warning, SSR uses modified_html.
6. Full suite (`cargo test --lib`), Praetor on changed files, docs
   (`spec/SPEC.md` §21 wiring note), BUGS.md Phase-1 entry.

## Verification

- `brivc build examples/todo.rbv --backend webstack --out /tmp/w1`:
  `index.html` contains injected `id="..."`, `todo-bindings.mjs` (dom-shim)
  written, `_bindingTable` binds `items` (b-each row is Phase 2) and the
  `add_item`/`clear_all` triggers.
- `brivc build examples/counter.rbv --backend webstack --out /tmp/w2`:
  view directives compile (b-text/b-trigger on the root instance) and a
  warning names `<Counter />` as a Phase 2 component.
- A `.bv` with a bare `render` block (no `<view>`): index.html derives from the
  render block HTML.
- Error paths: `b-text="nope"` → SRBV001 hard error naming `nope`;
  `b-if="x"` → SPEC 21.4 validation hard error.
- `cargo test --lib` fully green (no change to x86_64 codegen).

## Rationale

- The ViewCompiler already implements ID injection, directive extraction,
  validation, and SRBV — the missing work is entirely integration + the
  layout/projection glue. No new passes, no new heuristics.
- Rules satisfied: never silently ignore rejected directives (SPEC 21.4);
  undefined signals are compile errors not runtime nulls; the `.^` reflection
  syntax is canonical Briv (parser/expressions.rs:307), so deref is not a
  mini-language.