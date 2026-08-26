# Async Phase D — Compiled Port Events

**Date:** 2026-08-26
**Builds on:** Phase C segmented tasks (compiled scheduler) + Phase B
interpreter wake/block semantics. Closes Phase C's disclosed gap: blocking
port reads were interpreter-only.

## Insight that makes this small

An event wire IS an i64 — a handle to a runtime event-slot record. That
slides straight into every ABI we already have:

- **Task args** are i64 argv slots → pass the wire id, no new ABI.
- **Obj pool columns** are per-instance slots → an OUT port column stores
  its slot id (`Type::int()`), allocated at spawn.
- **Struct payloads** are boxed i64 handles → payload projection is
  `read → inttoptr → GEP`, the tuple/struct path that already exists.
- **Presplit segments** guarantee a port read heads its segment → a blocked
  read just re-runs its segment after wake (no mid-segment resume state).

## Design

### 1. C runtime (`briev_rt.c`)

```c
static struct { int ready; long long payload;
                int waiters[8]; int nwaiters; } briev_events[BRIEV_EVENT_MAX];
long long briev_event_alloc(void);
void briev_event_fire(long long slot, long long payload);   // wake waiters→READY
int  briev_event_read(long long slot, long long* out);      // 1 ready; 0 = BLOCKED
int  briev_event_ready(long long slot);
```

- New task status WAITING (4). `briev_event_read` on an unready slot with
  `__briev_current_task >= 0` registers the waiter, marks WAITING, returns 0.
- `BrievSegOut.finished` gains state 2 = BLOCKED: the await driver leaves
  the cursor untouched (presplit guarantees the read heads the segment) and
  skips the task until a fire re-marks READY.
- `__briev_current_task` global set by the await loop around each segment
  call — the compiled twin of the interpreter's CURRENT_TASK thread-local.

### 2. Backend emission

- **Registration:** obj OUT and IN ports append to `struct_types[base]` as
  `Int` fields → `register_pool_columns` creates `{base}.{port}` columns
  automatically; unpacked top-level instances get slots too.
- **Spawn:** after the Init member runs, each OUT-port column gets
  `call @briev_event_alloc()` stored into it; IN-port columns store the
  adapted ctor arg (the caller's wire id) positionally.
- **Segment params:** bind REAL declared types (`ctx.task_segments`
  already carries them) instead of blanket `Type::int()` — downstream arms
  need `Event(_)` to recognize wires.
- **Fire** (`w <- value;` where w binds an `Event<_>`):
  `load wire id → adapt value → call @briev_event_fire`. Tracked via the
  existing handle-tracking hook points (Let/Assign arms + countable walker),
  keyed on declared `Event` type rather than spawn provenance.
- **Payload projection** (`d.field` where d binds `Event<P>`): emit
  `%ready = call @briev_event_read(id, &out)`; branch ready→project
  (`inttoptr/GEP` for product payloads, direct for scalars), not-ready→the
  BLOCKED continuation. Inside segment fns the not-ready branch returns
  `{0, 2}`; outside any task it traps with a diagnostic message
  (`.^Ready` gates top-level reads — parity with the interpreter's strict
  error).
- **`.^Ready` reflect** on an Event-typed receiver → `@briev_event_ready`.

### 3. Capability

LLVM `obj_ports` flips TRUE (cells stay false — B2 was interpreter-side).

## Tests

1. rt.c unit (C harness): alloc/fire/read/wake transitions incl. blocked
   task resurrection and waiter draining.
2. e2e `examples/async-events-compiled.bv`: consumer task blocks on an obj
   out port; producer task drives a member txn whose fire wakes it — ONE
   await completes both. Deterministic output asserted; interpreter run of
   the SAME file must agree (parity across backends).
3. `^Ready` gating e2e: top-level gated read observes false→true.
4. Suite + sweep green; capability rejection removed for obj programs.

## Non-goals

- Cells compiled (stays off).
- Multiple payloads per fire / broadcast semantics beyond one-slot
  level-triggered (SPEC §9.5 unchanged).
- Float payloads (same i64-ABI gate as Phase C).

## Milestone log — slice 2/3 complete (2026-08-26, commit 340c43c9)

`examples/async-events.bv` compiles + links + runs; wake path verified
end-to-end with a printable twin (`17` = consume(7) + produce(1)·10 —
result reachable only if fire woke a blocked consumer through the
round-robin). Suite 1960 green. Four defects found and fixed:

1. **Empty-slot vacuous truth** — early TypeDef walk guard
   `!slots.is_empty() && !slots.iter().all(variant)` rejected ports-only
   objs: on zero slots `all()` is vacuously true. Guard now
   `(empty || !all_variant)`; the arm appends port columns to
   struct_types and seeds obj_port_wiring.
2. **obj_members-only pool gate** — the spawn-pool registration loop in
   build_field_index skipped bases without members; widened to accept
   obj_port_wiring bases. Same widening in `instance_prefix_for`
   (emit_expr.rs), else `bus.evt` fell to the boxed inttoptr path.
3. **Base-store wiring** — emit_spawn_init stored event-slot ids at the
   column BASE while reads indexed by row; wiring now goes through
   emit_instance_column_row so ids land in THIS spawn's row. Duplicate
   pre-Init wiring block removed (would double-allocate for Init'd objs).
4. **Fold-path type pin** — the main-fold Let recorded the column-read
   register type (Int), losing `Event<Damage>`; the spawn-site wrapper
   then re-wrapped `wire` into PRIVATE slots (alloc+fire per arg) and
   payload projections derefed address 0. A declared Event<T> pin now
   wins over reg.ty in loop_engine/counter.rs.

Also flipped LLVM CAPABILITIES.obj_ports = true (capability gate was
checked before context existed).

Remaining for 3/3: none functional — SPEC §9.5/§12.2 notes landed this
slice; final sweep/praetor pass stands.

## Undo

Revert rt.c block + backend arms + capability flip; interpreter untouched.
