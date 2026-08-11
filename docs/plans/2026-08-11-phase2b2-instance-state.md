# Phase 2b2 — Per-instance component state (SPEC 21.3, slice 2)

**Date:** 2026-08-11
**Status:** plan — design for per-instance component state, the second slice
of the SPEC 21.3 component model. 2b1 (fragment mounting) landed; this gives
each mount its OWN state.

## Problem

2b1 mounts a `render Name { ... }` fragment whose directives bind the GLOBAL
`%State` — two `<Counter />` mounts share one `count`. SPEC 21.3 says custom
component tags create **first-class reactive instances**: "The rendered parent
owns each mounted component handle. Mounting creates the handle; unmounting
releases state and subscriptions."

Per-instance state needs:
- Each mount of `Name` to own its own copy of the fields its fragment
  references (`count` for the counter fragment).
- Component transactions to route to the RIGHT instance (`increment` bumps
  THIS counter, not all of them).
- Mount/unmount to allocate/release instance state (`b-when` inside the
  fragment, or the component tag inside a `b-when` subtree).
- The WASM runtime's single-global-`%State` model to carry N instances.

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
- `cargo test --lib`, `cargo test --bin brivc`, Praetor, per commit.

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
