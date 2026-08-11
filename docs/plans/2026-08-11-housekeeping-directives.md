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

Status: **compiles for x86_64 AND webstack** (the wasm32 obj-member bug that
blocked the webstack build was fixed — see BUGS.md FIXED). The List b-each
warns + skips (list rendering is a separate slice).

Honest limitation: `items` is `List<String>`; the 2a3 b-each renderer handles
static `Int[N]`/`Bool[N]` vectors only, and `.^Size` on a heap List isn't
decoded. Add a WARNING when an Each iterable is not a vector layout row
(size % element_size, element_size from the layout) so the example compiles
with a documented list-rendering gap (a future list-support slice). Never a
silent wrong render — the warning names the gap.

## Part 2 — Global `b-class`/`b-style`/`b-attr` emission — DONE

- **Class** `b-class="{ 'cls': <expr> }"`: `el.classList.toggle(cls, eval)`,
  registered on the expr's root field handle (flush-driven). Bounded expr:
  bare field (truthy) or `field <op> <literal>`; literal-only pairs apply once
  at init.
- **Style** `b-style="name: value"`: `el.style[name] = value` (field ref,
  flush-driven) or a literal (init).
- **Attr** `b-attr="name: value"`: `el.setAttribute(name, value)` (same).
- `parse_attr_raw` preserves value quotes (quoted = literal, unquoted = field);
  the literal check requires the closing quote at the exact end. Complex
  expressions are compile-time errors (SPEC 21.4: never silent dead DOM).
- Fixed the HTML-comment validation bug (`<!-- b-each: ... -->` triggered the
  b-key error — the comment skip now checks the raw `<!--`/`</`).
- `examples/view-directives.rbv` migrated to supported forms (vector each,
  bounded class/style/attr exprs) — compiles + builds clean.
- Tests: 4 new. Suite 1762 lib + 14 bin green.

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
