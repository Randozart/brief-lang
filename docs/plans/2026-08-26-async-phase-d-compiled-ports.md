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

## Undo

Revert rt.c block + backend arms + capability flip; interpreter untouched.
