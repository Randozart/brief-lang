# Phase 2b3 — obj-backed components: instantiate in Briev, mount in HTML

**2026-08-12.** Supersedes the globals-based component model (2b2, SPEC 21.3) and
the direct-store reset export (2b2 slice 2c). Replaces the frontend-seeded
instance model with Briev-driven instantiation.

## Problem

The 2b2 component model seeds instance state from the FRONTEND:

- `<Counter count="7" />` — Rust string-parses the HTML attr
  (`collect_mount_props` / `parse_prop_value`, `component_instances.rs:104,288`),
  invents an `Expr::Decimal(7)` in Rust, and injects a direct `store i32 7`
  into `init_state` via `field_initializers`. The VALUE originates in Rust, not
  Briev source — no contract, no reactive machinery.
- The lifecycle reset (`__instance_reset_<Name>_<i>`, `emit_toplevel.rs:1376`)
  is a Rust-emitted DIRECT store called by the shim on b-when unmount — no
  contract proof, no flush. The missing flush leaves the DOM stale after a
  reset (remount shows authored content, not the reset value).
- The component state is declared as fragment-referenced GLOBALS
  (`let count: Int = 0` + top-level `txn increment`), not an object.

Rule (user, 2026-08-12): **if something in the frontend can change backend
state, it must be bound to a trg.** The frontend (Rust) invents no state
values; all state changes flow through the transaction machinery (contract
proof + flush). Initial state is declared in Briev source.

## Target model

Components ARE objects.

```briev
obj Counter {
    count: Int;
    txn increment [count < 100][true] {
        count = count + 1;
        term;
    };
};

render Counter {
    <span b-text="count">0</span>
    <button b-trigger:click="increment">+</button>
};

// Briev-side instance — the PROGRAM owns it (named, seedable, contractable):
let c1: Counter = Counter { count: 5 };
let c2: Counter = Counter { count: 7 };

render Root {
    <c1 />            // mounts c1's fragment, routes count -> c1.count
    <c2 />            // routes count -> c2.count
    <Counter />       // HTML-side anonymous spawn — the REACTOR owns it,
                      // zero-init, pool-indexed, not referenceable by code
};
```

Ownership split (user decision, 2026-08-12):

- **Briev-side** (`let c1: Counter = ...` + `<c1 />`): the program owns the
  instance. Named, code-controllable, seeded in Briev. Seed values are parsed
  by the BRIEV parser, never Rust.
- **HTML-side** (`<Counter />`): the reactor owns it. Anonymous, per-mount
  pool (`Counter.<i>.*`), zero-init defaults, only its txn variants touch it.
  Not referenceable by code.
- **No HTML props.** `<Counter count="5" />` is dropped entirely. All seeding
  is Briev.

### Tag namespace resolution (deterministic)

For a tag `<Foo ...>`:

1. `Foo` is a declared instance var (`let c1: Counter`) → instance mount, routes
   fragment refs to `c1.*` slots, txn variants `increment_c1`.
2. `Foo` is a component type (`obj Foo` + `render Foo`) → anonymous spawn,
   routes to `Foo.<i>.*` slots, txn variants `increment_<i>`.
3. `Foo` is a lowercase HTML element → normal element.
4. else → unknown-tag warning (existing).
5. **Compile error** if a var name collides with an HTML element name
   (`let div = ...`) — keeps the namespaces separated. Compile error if a var
   name collides with a component type name.

### render↔obj pairing

`render Name` REQUIRES `obj Name` (compile error otherwise). The fragment's
field refs must be ⊆ the obj's slots; the fragment's txn refs must be ⊆ the
obj's member txns. Slot types come from the obj (kills the `Type::int()`
fallback in `replace_or_add_state_decl`). The globals-based form
(`let count: Int = 0` + top-level `txn increment`) is dropped; existing
demos/tests migrate to obj form.

### Obj member txns (working form)

`txn Counter.increment`, `&count = ...`, and `@count` do NOT parse today
(`tests/instances_test.bv` is stale/aspirational — verified 2026-08-12). The
working member form is bare:

```briev
obj Counter {
    count: Int;
    txn increment [count < 100][true] { count = count + 1; term; };
};
```

Per-mount variants source from the OBJ MEMBER bodies via the existing
`build_txn_variants` rewrite machinery (identifier qualification), not from
top-level txns. The member body's bare slot names (`count`) are the rewrite
keys (fragment refs ⊆ obj slots guarantees the overlap).

## The trg rule, mapped

| Write | Mechanism | Compliance |
|---|---|---|
| Seed (`count: 5`) | Briev StructLiteral `Counter { count: 5 }` → `c1.count` init → `init_state` store | ✅ value is Briev source |
| Per-mount write (`increment`) | callable txn variant, contract proven | ✅ |
| Reset on unmount | callable reset TXN `txn __reset_c1 [true][c1.count == 5] { c1.count = 5; term; }`, contract + flush | ✅ fixes stale-DOM |
| HTML-side default | zero-init (type-defined) | ✅ no Rust-invented value |

Callable txns (shim-invoked, like `b-trigger` today) never become persistent
ticked nodes, so a `[true]`-pre reset txn cannot livelock.

## Slice plan (green + committed after each)

### Slice 1 — obj-backed render foundation

- `render Name` requires `obj Name`; fragment field refs ⊆ obj slots, txn refs
  ⊆ obj member txns (compile errors otherwise).
- `build_txn_variants` sources from obj member bodies (bare form) instead of
  top-level txns; slot types from the obj's slots (drop `Type::int()` fallback).
- Per-mount slots = fragment refs ∪ member-txn write sets, typed by the obj.
- HTML-side `<Counter />` preserved: zero-init, props DROPPED (delete
  `collect_mount_props` / `parse_prop_value` / `parse_tag_attrs` seeding).
- Migrate `component_instances.rs` tests + `examples/*.s.rbv` counter demos to
  obj form.

### Slice 2 — Briev-side instances

- Collect top-level `let <name>: <Obj> = <StructLiteral>` where the type has a
  `render` block → instance registry.
- Decompose the StructLiteral fields → dotted `name.<field>` StateDecls (typed
  by the obj) + `field_initializers` seeds (Briev values) → `init_state`.
- Suppress the obj-unpack path (`build_field_index` unpack) for
  component-backed lets — avoid a double `Counter.count` column conflict.
- `<c1 />` tag resolution (var → type → render block → mount spec routing to
  `name.*`, variant `increment_c1`, `data-instance="c1"`).
- Tag namespace resolution + collision guards.

### Slice 3 — trg-based reset (replaces 2c direct-store reset)

- Emit callable reset txns per instance: `__reset_c1` (re-applies the Briev
  seed) / `__reset_Counter_<i>` (zero). Contract proven, flush runs.
- Delete `emit_component_reset_exports` + `ctx.component_instances` + the
  `instances` field on the plan; shim reset hook calls
  `_txn('__reset_' + inst.replace('.', '_'))`.
- The reset txn body is the Briev seed expression; for the HTML-side spawn it is
  the type default (zero).

### Slice 4 — docs, spec, migration

- Rewrite SPEC §21.3 (tags no longer "create instances"; ownership split;
  namespace rule; no HTML props).
- Update `docs/architecture/` (agent-reference §webstack / component model),
  `BUGS.md` (record: stale instances_test.bv, reset-no-flush bug, StructLiteral
  seed loss).
- Test migration to obj form; E2E `.s.rbv` demos.

## Known risks / verification

- Callable-txn flush: verify a shim-invoked reset txn flushes its write set to
  the DOM (the `increment` click path already proves the mechanism).
- Obj unpack conflict: component-backed `let c1: Counter = ...` must not also
  create the `Counter.count` unpack column.
- Post-condition proof for reset txns: `[true][c1.count == 5]` after
  `c1.count = 5` — non-trivial posts already proven (see `toggle` test).
  Complex seeds (expressions) either constant-fold or become a compile error —
  never silently wrong.
