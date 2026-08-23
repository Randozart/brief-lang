# Obj/Cell Ports — Sub-Plan (Phase 7)

**Date:** 2026-08-22
**Parent:** `docs/plans/2026-08-22-spec-conformance.md` §Phase 7
**Strategy (owner):** interp-first slice; **interp-complete cells**. LLVM + rbv staged behind a BUGS entry.
**SPEC:** §9.5, §9.6 — exact grammar and semantics below are taken verbatim from the spec examples.

## SPEC-pinned surface

```briev
obj Enemy(damage: Event<Damage>) -> died: Event<EnemyId> {
    health: Int;

    node apply_damage()[damage.Ready][health >= 0] {
        health = health - damage.amount;
        term;
    };
};

cell Timer(period: Duration) -> tick: Event { /* owned state + internal nodes */ };
```

Normative facts:
1. Cells and objects share input `(name: Type, …)` and named output
   `-> name: Type, …` header syntax. Both sides optional.
2. Multiple outputs form a complete named product on every target.
3. Input ports appear in CONTRACTS (`[damage.Ready]`) and bodies
   (`damage.amount`) — an input port exposes `.Ready` (a pending event is
   observable) and projects the payload's members directly.
4. Cells: communication ONLY through declared ports; internal state not
   externally visible.

## Design decisions (binding for v1)

| Question | Decision |
|---|---|
| Event<T> representation | A one-slot-deep event queue per port instance: `{ ready: bool, payload: Value }`. `.Ready` → ready flag. Member access on an Event falls through to the payload's fields (`damage.amount`). Writing a port sets payload+ready. Reading consumes nothing (peek semantics); a `trg`-style consumer or explicit clear resets Ready. |
| Output firing | Assignment to the port name inside the obj/cell body: `died <- value;` (ArrowAssign — the existing stream-write form). Deterministic delivery order = scheduler order; no implicit concurrency (XOR gate unchanged). |
| Wiring | Input ports bind at construction: `spawn Enemy(damage: timer.tick)` where `timer.tick` reads ANOTHER instance's output port — the wire shares the producer's slot handle. Plain values also legal (`spawn T(x: Event<Int>)`? NO — inputs must receive Event-typed sources; direct payloads wrap automatically). |
| Sealing | Cell internals (fields/members) reject external name resolution; ports are the only externally visible names. Objs keep current open-member behavior. |
| Duration | `Duration` is not yet a fundamental; the cell example's `period: Duration` types as any declared stdlib/custom type in fixtures (use Int in tests until Duration exists). |

## Sub-phases

### 7a — Header parse + AST + typecheck
- `parse_obj_like`: after `Name<T>`, optional `(params)` via the standard
  parameter list, then optional `-> out1: Type, out2: Type` (named pairs).
  Store on TypeDef as `ports_in: Vec<(String, Type)>`,
  `ports_out: Vec<(String, Type)>`.
- `parse_cell`: REPLACE the token-skip skeleton — real body walk reusing the
  obj-like member parser (slots, metadata, txn/node/defn/trg), populating
  CellDef.parameters / output_type-as-named-ports / fields. Add
  `ports_in/ports_out` mirroring TypeDef.
- Typechecker: ports join the type_slots/type_members namespaces of the
  type; `[port.Ready]` contract references resolve; sealing check for cells
  (external references to cell members error naming the port rule).

### 7b — Interpreter event delivery
- Value::EventQ variant; construction wiring at spawn sites; `.Ready`
  reflect/member resolution; payload fallthrough; ArrowAssign-to-port fires;
  cross-instance wiring shares the queue handle.
- Fixture `examples/object_ports.bv` (Enemy/damage/died shape) runs in the
  interpreter with pinned output.

### 7c — LLVM + rbv (STAGED)
- Explicit rejection when a program instantiates port-bearing objs or any
  cell, until the backend grows port columns + event queues + rbv mount
  wiring. BUGS.md entry tracks it. Non-port programs unaffected.

## Acceptance
- Spec example shapes parse/typecheck/run (interp).
- Sealing negative test.
- All pinned fixtures still byte-correct; suite green per commit.
