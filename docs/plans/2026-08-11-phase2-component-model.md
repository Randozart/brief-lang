# Phase 2 — Component model + structural directives (SPEC 21.3/21.4)

**Date:** 2026-08-11
**Status:** active — plan for the SPEC 21.3 component model, decomposed into
independently-complete sub-phases. **2a1 `b-when`, 2a2 `b-bind:value`, 2a3
`b-each`+`b-key` implemented (committed).** Phase 2b pending.

## 2a3 implementation summary (2026-08-11)

- `Directive::Each` carries `item_bindings` + `key_expr`; item-scoped directives
  (b-text/b-class/b-show/b-when/b-trigger referencing `item`) are captured from
  the each element AND its inner template (data-itm markers), NOT leaked as
  global bindings. Unsupported item expressions (state-field comparisons, calls)
  are compile-time errors — never silent dead DOM.
- Shim renderer: on iterable flush, reads `count = size/element_size` i64-or-i32
  slots from WASM (width-aware — vector slots are `i{int_bits}`), builds a fresh
  tagName clone per item, applies item directives, reconciles by key
  (insert/remove/reorder).
- **Two critical runtime gaps fixed** (never exercised because the page never
  ran): (1) wasm-ld exported NOTHING (`state_layout`/txn exports undefined) —
  `compile_wasm` now exports every txn (both `@<name>` and `@txn_<name>` forms)
  + `state_layout`; (2) the reactive txn exports as `txn_<name>` while callable
  txns export as `<name>` — the shim's `_txn()` resolves either.
- `web_vector_element_size` + `FieldLayout.element_size`: the b-each count is
  layout-derived (type-driven, no name matching).
- `find_each_inner_html` quote-aware `>` scan (same bug class as parse_tag).
- Tests: 3 new (view_compiler ×2, web_generator ×1). Suite 1752 lib + 14 bin
  green.

## 2a2 implementation summary (2026-08-11)

- `Directive::Bind { target }` extracted; SRBV011 checks the target is a state
  field (strict profile).
- Build-time writer resolution (`resolve_bind_routes`): a field's input route is
  the UNIQUE transaction whose `write_set` (the SAME transition-graph source the
  flush batch covers) contains it. Zero writers / ambiguous writers / wrong
  arity are hard errors (SPEC 21.4: never an inert input). `codegen` resolves
  from the graph; compile_source validates every `Directive::Bind`.
- Shim emits type-driven input wiring: String → `_writeString(el.value)` then
  `exports[txn](ptr)`; Int/Float → `Number(el.value)`; Bool → checkbox
  `.checked`. Marshalling derived from the txn param type via
  protocol_category → TypeTag → ParamKind (no name matching).
- Three webstack parameterized-txn codegen bugs fixed (BUGS.md): void-return
  `alloca void`; boxed params typed `int()` (wrong on wasm32 i32); narrow Int
  params stored un-widened.
- Tests: 5 new (view_compiler ×2, web_generator ×1, compile ×2). Suite 1749
  lib + 14 bin green.

## 2a1 implementation summary (2026-08-11)

- `Directive::When { expr }` extracted (KNOWN_DIRECTIVES + inject_ids
  `has_directive` + extract_directives), SRBV-checked by leading-identifier
  (`condition_root_signal`, shared with the generator + frontend) so compound
  `b-when="count > 0"` conditions don't false-positive SRBV003.
- Shim emits a per-node mount/unmount effect: element starts in the DOM as
  authored; first falsy flush detaches it (comment anchor + template snapshot);
  truthy flushes re-insert a fresh clone (identity NOT preserved — the
  `b-show` distinction). Initial-authored-DOM is consistent with every other
  binding.
- Four bugs fixed en route (BUGS.md): applyFn clobbering → `_registerViewEffect`
  per-handle effect lists; conditional writes now in the txn write_set
  (`extract_write_set` recurses into `If`/`Guarded`/`Block`/`Foreach` — the
  webstack flush batch was missing them, so b-when/b-show on a conditionally-
  written field stayed dead); `parse_tag` quote-aware `>` scan; self-closing
  tags with directives get IDs; dead-field diagnostics filter out
  `view_bound_fields`.
- Tests: 6 new (view_compiler ×3, web_generator ×2, transition_graph ×1).
  Suite 1746 lib + 12 bin green.

## Problem

Phase 1 wired the single-root view: one `<view>`/`render Root {}` block, one
global `%State`, one flat binding table. SPEC 21.3/21.4 demand more:

- `b-when="expr"` — structural mount/unmount of a subtree (vs `b-show`, which
  preserves identity/state). Currently `b-when` is **not** a known directive:
  the ViewCompiler flags it `unknown directive 'b-when'` (RBV001).
- `b-bind:value="field"` — input → assignable field write. Currently validated
  structurally but never wired: no binding is extracted and no WASM path can
  mutate a field from JS.
- `b-each:item="items"` + `b-key` — dynamic repetition with stable-key
  reconciliation. Currently an `Each` binding is extracted but **emits no JS**
  (`binding_to_js` returns `""` for it) — the loop never renders.
- Custom component tags (`<Counter />`) — first-class reactive instances with
  mount/unmount lifecycle and per-instance state. Currently a warning + inert
  render.

Per SPEC 21.3: *"Custom component tags create first-class reactive instances.
The rendered parent owns each mounted component handle. Mounting creates the
handle; unmounting releases state and subscriptions."*

## Scope decomposition

**Phase 2a — structural + input directives, single-root state.** No instance
registry. Each directive is a complete, testable feature on the existing one-
global-`%State` model. Components remain a warning until Phase 2b.

- 2a1 `b-when`: state-driven DOM mount/unmount.
- 2a2 `b-bind:value`: input → transaction write, type-marshalled, with the
  write-contract proof (SPEC 21.4) enforced at codegen time.
- 2a3 `b-each` + `b-key`: vector-field iteration with keyed reconciliation.

**Phase 2b — component instances.** The mount registry: per-instance state in
%State, `mount`/`unmount` exports, component-view extraction, subscription
release on unmount. Builds on 2a1 (a component inside a `b-when` subtree
mounts/unmounts with it). Phase 1's `warn_component_tags` diagnostic is
replaced by real wiring.

Phase 2b is the architectural core (the WASM runtime has no mount registry and
state is a single global `%State`) — planned here, implemented in a follow-up.

## Design decisions — Phase 2a

### 2a1 `b-when="expr"` (SPEC 21.4: mounts/unmounts a subtree)

- ViewCompiler: add `Directive::When { expr }`; register `b-when` in
  `KNOWN_DIRECTIVES`; extract like `b-show`. `validate_directives` already
  purity-checks it.
- Semantics: the element (and subtree) is **present in the DOM iff `expr` is
  truthy**. Identity is NOT preserved across toggles (structural, unlike
  `b-show`).
- `view_root_signals`: `When` contributes its root signal, so the field stays
  live (observability-as-liveness, same as Text/Show).
- Shim JS (web_generator): at init, cache the element as a detached template,
  insert a comment anchor in its place, and emit an applyFn on the root field's
  handle that mounts (clone template after the anchor) or unmounts (remove the
  clone) based on the decoded value's truthiness. No `x == x` liveness hacks —
  the flush drives it.
- Re-mount of a `b-when` subtree re-runs the subtree's bindings because the
  clone is a fresh DOM node with its own listeners.

### 2a2 `b-bind:value="field"` (SPEC 21.4: assignable field, proven write contract)

- ViewCompiler: add `Directive::Bind { target }`; register `b-bind` in
  `KNOWN_DIRECTIVES`; keep the structural validation (identifier-only target).
- **Writer resolution at codegen time** — the transition graph (the write-set
  source of truth, built during `generate()`) resolves the unique transaction
  whose `write_set` contains `field`. Rules (SPEC 21.4 discipline, no silent
  guesses):
  - exactly one such transaction **with exactly one parameter** → wire `input`
    events to `exports[txn](marshalledValue)`;
  - zero → hard error "no transaction writes 'field'" ;
  - more than one → hard error "ambiguous — multiple transactions write
    'field'".
- Value marshalling is **type-driven** (no name matching): the txn param type
  (from the Briv signature in `items`) selects `_writeString` (#String),
  `Number(...)` (#Int/#Float), checkbox `.checked` (#Bool).
- The resolution map `field → (txn, param_ty)` is extracted in `codegen` from
  `b.ctx.transition_graph` + `items`, returned alongside `web_layout`, and
  passed into `GlueWebGenerator`. If the unique writer has zero params → hard
  error (an input writes a value; a param-less writer cannot accept it).

### 2a3 `b-each:item="items"` + `b-key` (SPEC 21.4: stable-key reconciliation)

- Scope: `items` is a **vector state field** — a static `Int[N]`/`Bool[N]`
  `[N x i64]` slot array in %State (the interpreter-verified object kind;
  emit_expr.rs:1357). Item = the slot scalar. The template's item-relative
  bindings (`item`, `item.^Size`, …) render per-item.
- The `Each` binding already carries `iterable`, `item_name`, `template_html`,
  `container_id`. `b-key` becomes `Directive::Key { expr }`-adjacent metadata
  on the Each binding (extract `b-key` value; a missing key stays a validation
  error — already enforced).
- Shim JS: on flush of the iterable field, read `count = layout_size /
  slot_width(tag)` (Int/Bool vectors are i64 slots → 8), read each slot from
  WASM, and reconcile the container's children by key (`item` value):
  insert new keys, remove gone keys, update changed keys, preserve order.
- Item-scoped rendering: the shim keeps a per-template item renderer emitted by
  the generator for the template's item-relative directives (text + projections
  + classes), applied to a fresh clone per key. Global-binding resolution is
  skipped for signals inside an `Each` template (they are not root fields).

## Contracts / invariants (Phase 2a)

- SPEC 21.4: rejected or unresolvable directives are **hard errors**, never
  silent stubs. (`b-when` unknown-directive RBV001 warning disappears; b-bind
  ambiguity/none errors; b-each without key already errors.)
- SPEC 21.5: all directive expressions stay pure/read-only; b-bind's write is
  an explicit input event, not a view expression.
- No new compiler knowledge of specific types: b-bind marshalling is param-type
  driven; b-each width comes from the state layout row (size/tag), never from
  Briv type names.
- Phase 1 behavior preserved: Text/Show/Hide/Trigger/Class/Attr/Style and the
  `.s` strict SRBV verification unchanged. Full suite stays green.
- `b-each`+`b-when` on the same subtree: unsupported in 2a (a hard diagnostic,
  not a silent conflict) — reconciled in 2b via the mount registry.

## Implementation order

1. 2a1 `b-when` (view_compiler + web_generator + root signals + tests).
2. 2a2 `b-bind:value` (Directive::Bind + codegen write-map + shim input wiring
   + marshalling + error tests).
3. 2a3 `b-each`+`b-key` (Each JS emission + reconciliation + item renderer +
   tests).
4. Verify: `cargo test --lib`, `cargo test --bin brivc`, Praetor changed files,
   E2E `brivc build` on a `.rbv` exercising all three + the dom-shim output.
5. Phase 2b plan doc (mount registry) follows as its own plan.

## Documentation updates

- `spec/SPEC.md` §21.4: unchanged (already specifies the directives); add a
  sentence that b-bind resolves its writer transaction at build time and errors
  on ambiguity.
- `src/view_compiler.rs` + `src/glue/web_generator.rs` doc comments: update the
  directive list; remove the "Each emits no JS" and "b-when unknown" stale
  notes as they land.
- `BUGS.md`: new entry for b-when/b-bind/b-each not wired (resolved as each
  lands); keep the Phase 2b component entry open.

## Baselines

Feature work, not performance work — no benchmark baseline required (the
benchmark harness covers x86_64; these changes are webstack-only). The
x86_64 codegen is untouched.
