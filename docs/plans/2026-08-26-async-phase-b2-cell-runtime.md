# Async Phase B2 — Cell Runtime

**Date:** 2026-08-26
**Builds on:** `docs/plans/2026-08-26-async-phase-b.md` (Phase B core landed)
**Deferral closed:** Phase B disclosed that the interpreter had NO cell
runtime — `TopLevel::Cell` was parsed and contract-checked but never
instantiated, its members never typechecked, its internal triggers dropped
by the parser. This phase makes cells first-class runtime citizens that
consume Phase B wake semantics.

## Problem

SPEC §9.6 promises "a sealed state machine with an independent convergence
membrane" sharing the obj port grammar. Today:

1. **Members unchecked** — a type error inside a `cell` txn/defn body passes
   silently (`TopLevel::Cell` appears only at contract collection,
   typechecker.rs:3258, and port registration, :3885).
2. **No instantiation** — `spawn Timer(...)` falls through spawn inference as
   `Custom("Timer")` but nothing registers a shape; interpreter spawn misses.
3. **Internal triggers vanish** — parse_cell pushes `TopLevel::Trigger` into
   `members`, whose filter_maps then drop it; `internal_triggers` is
   hardwired `vec![]`.

## Design

Cells are objs with a sealing rule. Every mechanism below REUSES the obj
machinery verbatim — no parallel cell code path (DRY; the only new
decision is *which* names seal).

### 1. Parser keeps internal triggers

`parse_cell` routes `trg` items into `internal_triggers: Vec<Trigger>`
(the field already exists) instead of `members`. Data preserved;
trigger SCHEDULING semantics stay staged — SPEC §13 declares typed event
ports "the staged replacement for a typed trigger surface", so inventing
trigger firing rules now would be speculative language design.

### 2. Typechecker: members register + bodies check

- Cell transactions/defns join `all_type_members[c.name]` → `instance.txn()`
  method dispatch resolves exactly like obj members.
- The obj member-body loop (:4103) gains a `TopLevel::Cell` branch with
  identical context construction: `self: Custom(name)`, field slots +
  ports_in/out bound as member-scope bindings, params/output per member.
  A type error inside a cell body is now a compile error.

### 3. Interpreter: cells construct like objs

`load_program` gains a `TopLevel::Cell` branch registering:
- `ObjShape { ports_in, ports_out, fields }` into `self.objs` under the
  cell name — spawn's existing shape path then wires input ports to shared
  EventQ handles, creates fresh unready out slots, defaults fields.
- Members under `{Cell}::{member}` — the SAME key format method calls
  resolve (`{type}::{name}`, eval.rs:834), so `timer.tick()` dispatches.

Sealing stays compile-time (cell_ports/type_slots): the interpreter serves
any declared field, as for objs — the reference trusts the checked program.

### 4. Integration: cells schedule through Phase B

The acceptance flow proves the composition: a cell txn fires an out port;
a spawned task blocked reading that port wakes and completes through the
ordinary round-robin. No scheduler code changes — cells feed the same
EventSlot machinery.

## Changes

| File | Change |
|---|---|
| `src/parser/definitions.rs` | `parse_cell`: trg items → `internal_triggers`; members no longer swallow them |
| `src/typechecker/mod.rs` | cell members → `all_type_members`; Cell branch in member-body check loop |
| `src/interpreter/mod.rs` | `load_program`: Cell → objs shape + member functions |
| `spec/SPEC.md` | §9.6: instantiation + member-call sentence; internal triggers noted parsed/staged |
| tests | parser trigger preservation; typecheck catches bad cell body; end-to-end wake-through-cell |

## Tests

1. `cell_internal_triggers_preserved` (parser): `trg t @ src;` inside a cell
   lands in `internal_triggers`, not members.
2. `cell_member_body_type_errors_are_caught` (typechecker): bad body → type
   error naming the cell; good body clean. Also `instance.txn()` call
   resolves.
3. `cell_spawn_wires_ports_and_dispatches_members` (interpreter):
   instantiate, mutate state via txn, fire out port, observe payload.
4. `phase_b_consumer_wakes_through_cell_port` (interpreter): consumer task
   blocks on cell out port; cell txn fire revives it mid-await — the B2
   acceptance.

## Non-goals

- Trigger scheduling semantics (staged per SPEC §13).
- LLVM lowering of cells (backend capability flags keep cells off the LLVM
  surface — unchanged; Phase C territory).
- Cell convergence membrane / independent reactor loops (unspecified beyond
  §9.6's one-line mention; needs its own plan when scheduled).

## Undo

Each change is additive beside its obj twin: revert the four files; no
existing arm changes meaning.
