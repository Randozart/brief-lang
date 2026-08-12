# Phase 2b2 — Per-instance component state (SPEC 21.3, slice 2)

**Date:** 2026-08-11
**Status:** slice 2a (fixed instance pools) and slice 2b props implemented.
Dynamic component counts remain a follow-up.

## Slice 2b — props (2026-08-11)

`<Name attr="value" />` seeds the mount's instance slot for the
fragment-referenced field `attr`. `collect_mount_props` parses the view's
mount tags in order; `parse_prop_value` turns the raw value into a literal
(Decimal / Bool / Quoted). The plan's `initializers` map
(`Counter.0.count` → 5) flows through codegen via
`LlvmBackend::with_component_initializers` and build_field_index merges them
into `field_initializers`, so `init_state` seeds each mount's slot.

Verified E2E: `<Counter count="5" />` + `<Counter count="7" />` → init_state
stores 5 into field 0 and 7 into field 1; each mount binds its own slot and
its trigger fires its own variant.

## Slice 2a implementation summary (2026-08-11)

- `analysis/component_instances::expand_component_instances` runs in
  compile_source after typecheck: counts `<Name />` mounts per component,
  and for each mount registers instance-qualified state slots
  (`Counter.0.count`, `Counter.1.count` — dotted StateDecls, scalar %State
  rows) and per-mount txn variants (`increment_0`, `increment_1` — the txn
  body/contract identifiers rewritten to the instance slots). Consumed
  globals are removed.
- The ViewCompiler's `render_blocks` became `HashMap<String, Vec<String>>`
  (per-mount fragments); mount k splices `fragments[k % len]`, binding each
  mount's DOM to its pool handles. `b-trigger` fires the mount's variant.
- Field types are preserved on instance slots (a `let show: Bool` keeps Bool);
  txns consumed by write-set (not just trigger name) are variant-ized + the
  originals removed.
- Verified E2E: two `<Counter />` mounts get independent `count` state and
  `increment_0`/`increment_1` variants; the wasm exports both variants; the
  shim binds each mount to its own handle; a `b-when` inside a component
  toggles one instance independently. `examples/counter.rbv` (single mount)
  binds `Counter.0.count`. Strict `.s.rbv` SRBV passes with the
  instance-qualified signals.
- Tests: 2 new (component_instances). Suite 1765 lib + 14 bin green; no new
  Praetor diagnostics in the new module.

## Design: fixed instance pools (slice 2a)

The runtime has ONE global `%State` and a flat field-index map — no dynamic
allocation, no mount registry. The honest first slice that fits the model:

- **Compile-time instance slots.** The view compiler counts `<Name />` mounts
  per component type. For each component `Name`, `%State` gains a pool of
  `M × instance_size` slots, where `M` = the mount count of `Name` and
  `instance_size` = the fields the fragment references (the component's
  "instance state"). Each mount is a fixed pool row — a static index, decided
  at compile time, not a runtime allocation.
- **Instance-qualified field handles.** The parent's `%State` fields for a
  component instance are `name.0.count`, `name.1.count`, … (the instance-pool
  prefix mechanism already exists for object instance pools — emit_expr.rs
  `instance_slots`/`self_prefix`). The view compiler's bindings for a mount
  resolve to that mount's handles.
- **Instance-routed transactions.** A component txn's `write_set` fields become
  instance-qualified. With fixed pools and compile-time mount indices, a
  transaction firing on a mount's handles writes exactly that instance. The
  transition graph's nodes get one entry per (txn, instance) when the txn
  writes component state, or the write_set carries the instance prefix. The
  flush batch covers instance-qualified fields (the `view_bound_fields`
  observability machinery generalizes: instance-qualified names are protected).
- **Mount/unmount = slot active flag.** Each pool row carries an `active: Bool`
  slot. A `b-when` inside the fragment (or the tag inside a `b-when` subtree)
  toggles it structurally — the fragment's DOM mounts/unmounts with 2b1's
  mount logic, and the instance's subscriptions (its flush effects) only apply
  while active. Unmounting releases the row's state (re-zero) — no GC, no
  registry: the fixed pool IS the handle.

### What this handles
- `render Counter { count + buttons }` mounted twice → two independent counts,
  two independent increment paths.
- A `b-when` inside the fragment mounts/unmounts ONE instance (its DOM +
  state) without touching the other.
- No runtime mount registry, no heap instance allocation — fits the
  single-`%State` model and the existing instance-pool machinery.

### What it does NOT (slice 2b, follow-up)
- **Dynamic instance counts** (`b-each` of components, variable mounts).
  Fixed pools need a compile-time max per type; a `b-each` of components
  needs a runtime index + a real registry. Documented, not designed here.
- **Props** (`<Counter initial="5" />`) — attribute → instance-state seeding.
  A follow-up.

## Frontend changes

1. `compile_view`/ViewCompiler: count `<Name />` mounts per render block;
   assign each mount a pool index; emit bindings with instance-qualified
   signals (`name.<i>.<field>`), already supported by `root_signal` +
   `field_handle_for_signal` for dotted prefixes.
2. The component fragment's directives rewrite their field references to the
   instance-qualified names (a per-fragment field map). The fragment's
   `render Name` block becomes a template that re-renders per instance with
   the instance's handles.
3. The transition graph: component txns route per instance — the write_set
   fields are instance-qualified; dispatch/fusion treat `name.<i>.*` as a
   unit.

## Backend changes

4. `%State` layout: instance pools appended (the existing
   `build_field_index`/`instance_slots` machinery already lays out
   `{prefix}.{field}` slots — the pools reuse it with a per-mount prefix).
5. The webstack `state_layout` table exposes instance-qualified rows; the shim
   binds a mount's DOM to its pool handles. `web_vector_element_size`-style
   layout derivation generalizes to pool rows.
6. Flush: instance-qualified write_sets → flush records for the instance's
   handles (already the mechanism — the names just carry the prefix).

## Tests

- view_compiler: two mounts → distinct handles; the fragment's `b-text`
  resolves per-instance; `b-when` inside toggles one instance.
- codegen: `%State` has `M × instance_size` per component; the txn's write
  lands on the right pool row.
- E2E: a two-`<Counter />` page — incrementing one doesn't move the other;
  `b-when` unmounts one.
- `cargo test --lib`, `cargo test --bin brievc`, Praetor, per commit.

## Documentation

- `spec/SPEC.md` §21.3: per-instance state, fixed pools, instance-routed
  transactions.
- `BUGS.md`: the "per-instance state is slice 2" note → resolved as this
  lands.
- `src/view_compiler.rs`/`compile.rs`/`emit_expr.rs` doc comments updated in
  the same commits.

## Baselines

Feature work (webstack + the instance-pool layout touches x86_64 %State layout
only behind the component path) — no benchmark baseline required. If the
instance-pool layout affects existing programs, the full suite guards it.
