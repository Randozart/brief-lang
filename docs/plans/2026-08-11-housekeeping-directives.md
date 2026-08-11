# Housekeeping + global class/style/attr directives

**Date:** 2026-08-11
**Status:** active — plan for clearing webstack debt and completing the SPEC
21.4 directive set, then the 2b2 instance-state plan.

## Part 1 — Housekeeping

### 1a. Remove duplicate Webstack arms
`src/compile.rs` codegen match has THREE `BackendKind::Webstack =>` arms; only
the first (line ~1597, sets `web_layout` + `bind_routes` + `view_bound_fields`)
is live — the other two are unreachable (Rust warns "unreachable pattern").
Dead code from historical merges; open in BUGS.md. Remove the two dupes.
Verify: no new "unreachable pattern" warnings; suite green; counter E2E still
builds.

### 1b. Migrate `examples/todo.rbv`
Stale syntax blocked the flagship example:
- `items :> Size` → `items.^Size` (and `@items :> Size` → dropped; `@`
  prior-state references are staged/unimplemented per SPEC, so the
  postconditions use the counter.rbv bounded-pre + `[true]`-post shape).
- The `b-each` lacked the now-mandatory `b-key` → added `b-key="item"`.
- `List<String>` push: `import <std/collections>` (List's `op InsertAt` lives
  in the stdlib), `&items <- v` (AddrOf target now resolves via
  `push_element_type`/`extract_element_type` unwrap), and empty-list
  assignment (`items = []`) coerces via `try_coerce_via_parse`.

Status: **compiles + links for x86_64** (`brivc build examples/todo.rbv
--backend llvm`). The webstack build is blocked by a separate pre-existing
bug — wasm32 obj-member bodies hardcode i64 slot widths (BUGS.md, OPEN) —
because the List's `len`/handle slots are i32 on wasm32. A compile-time
WARNING for non-vector `b-each` iterables was added (the generator skips them,
never a wrong render); the todo example's List each warns + skips until the
list-rendering slice lands.

Honest limitation: `items` is `List<String>`; the 2a3 b-each renderer handles
static `Int[N]`/`Bool[N]` vectors only, and `.^Size` on a heap List isn't
decoded. Add a WARNING when an Each iterable is not a vector layout row
(size % element_size, element_size from the layout) so the example compiles
with a documented list-rendering gap (a future list-support slice). Never a
silent wrong render — the warning names the gap.

## Part 2 — Global `b-class`/`b-style`/`b-attr` emission

Currently DEAD: `binding_to_js` matches Text/Show/Hide/When/Trigger/Bind/Each
and falls to `_ => ""` for Class/Attr/Style — top-level (non-each) instances
never update the DOM. Completes SPEC 21.4's directive list.

Design (bounded, consistent with the b-each item expressions):

- **Class** `b-class="{ 'cls': <expr> }"` — emit on the expr's ROOT field
  handle: `el.classList.toggle(cls, eval(value))`. Bounded expr: bare field
  (truthy), `field <op> <literal>` (==, !=, <, <=, >, >= against number/bool/
  string literal). Complex (ternary, call, multi-field) → compile-time error
  (SPEC 21.4: never silent).
- **Style** `b-style="name: value"` — `el.style[<name>] = value`; bounded
  value: literal or single field (string). Complex → error.
- **Attr** `b-attr="name: value"` — `el.setAttribute(name, value)`; bounded
  same as Style.
- The flush-driven applyFn evaluates the expr with the flushed root field's
  value; `view_root_signals` already protects these fields (observability).
- `_registerViewEffect` fans multi-binding fields out (already landed).

Migrate `examples/view-directives.rbv`'s complex `b-style` (string-concat
ternary) to supported forms so the example compiles.

Tests: view_compiler (extraction + validation errors), web_generator
(Class/Style/Attr emission + expr translation), E2E `.rbv`.

## Part 3 — 2b2 (after Parts 1–2)

Per-instance component state: mount registry in the WASM runtime, per-instance
field slots, instance-routed transactions, `b-when` structural component
mounts. Separate plan doc `docs/plans/2026-08-11-phase2b2-instance-state.md`.

## Documentation updates

- `BUGS.md`: duplicate-Webstack-arms entry → resolved; add Class/Attr/Style
  dead-emission entry → resolved as Part 2 lands.
- `spec/SPEC.md` §21.4: note the bounded expression forms + that complex
  global view expressions are compile-time errors (per directive).
- Plan docs updated at each part.

## Verification per commit

`cargo test --lib` + `cargo test --bin brivc`; Praetor on changed files (no
new diagnostics); E2E `brivc build` on the touched examples + a directive
test file; x86_64 codegen untouched (webstack-only).
