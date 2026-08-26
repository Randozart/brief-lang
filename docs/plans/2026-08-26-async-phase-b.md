# Async Phase B — Port Event Integration

**Date:** 2026-08-26
**Builds on:** `docs/plans/2026-08-23-async-scheduler.md` (Phases A1–A3 complete)
**Scope:** Interpreter reference scheduler. LLVM lowering stays Phase C.

## Problem

Phase A3 delivered the task table, lazy spawn, yield-split segments, and the
round-robin scheduler in the `Await` handler. What it cannot do: a consumer
task that reads an **unready input port** fails outright (`TypeError: no
pending event`, eval.rs Field-on-EventQ arm) instead of suspending. There is
no mechanism by which firing a port (`out <- value;`) makes a waiting
consumer runnable. Cooperative scheduling without event wake-up cannot run
the SPEC §9.5 producer/conductor patterns.

## Design

Zero syntax changes. `<~` is NOT involved — port send already exists as
plain ArrowAssign (`died <- value;`, SPEC §9.5); `~<-` keeps its destructive-
extract meaning untouched. All four mechanics below are runtime semantics.

### 1. EventSlot gains waiters

```rust
pub struct EventSlot {
    pub ready: bool,
    pub payload: Option<Value>,
    pub waiters: Vec<u64>,   // NEW: task ids blocked on this slot
}
```

### 2. TaskStatus gains Waiting

```
Ready ──(await drives)──▶ Yielded/Done
Ready/Yielded ──(unready port read)──▶ Waiting ──(fire wakes)──▶ Ready
any ──(free)──▶ Cancelled (already existed; now enforced at runtime)
```

A `Waiting` task is invisible to `collect_runnable()` until a fire flips it
back to `Ready`.

### 3. Blocking reads

Reading a payload member off an unready `EventQ`:

- **Inside a task** (thread-local `CURRENT_TASK` set by the segment
  executors): register the task id in the slot's waiter list, set status
  `Waiting`, abort the segment with the new `RuntimeError::TaskBlocked`.
  The executor catches it: NO segment advance, NO result store — post-wake
  the SAME segment re-runs from its first statement.
- **Outside a task**: the existing strict `TypeError` stands (`.^Ready`
  gates top-level reads; behavior preserved for all current programs).

### 4. Segment pre-splitting at port reads

Whole-segment re-run after wake would duplicate side effects that precede
the blocking read inside the same segment. Fix at registration time:
`register_pending_task` additionally cuts a segment boundary BEFORE any
statement whose expressions contain `<param>.<field>` for any parameter
name (conservative syntactic receiver check — FunctionDef carries names,
not types; over-splitting is safe since finer interleave preserves
statement order exactly). The blocking read then HEADS its own segment:
prior statements ran once in the completed prior segment; the re-run
re-executes only the read-headed segment.

Known limitation (documented in SPEC prose): a blocking read inside a
`while` body still re-runs the whole loop after wake — gate loops with
`.^Ready` checks (level-triggered semantics: slot values are stable once
written, so gated re-entry converges).

### 5. Fire wakes waiters

The ArrowAssign EventQ arm (`out <- value;`) sets ready + payload, then
drains `waiters`: every id whose status is `Waiting` flips to `Ready`.
Cancelled/Done ids are skipped (no resurrection).

### 6. `free` cancels at runtime

`FreeHint` today removes the local binding only — the table entry stayed
`Ready` and OTHER awaits' round-robins would happily run it. Now: if the
binding holds a task-id atom, mark that entry `Cancelled` before removing.

## Deadlock posture

Cooperative scheduler, no preemption: if every remaining task is `Waiting`,
the `Await` round-robin finds `runnable` empty and returns the handle value
(await does not hang). Cycles between blocked producers/consumers are a
program error the reference reports by returning; Phase C may add a
diagnostic. Documented, not hidden.

## Cells — deferred to B2 (disclosed)

The original Phase B bullet included "cell internal nodes schedule as tasks".
Investigation found the interpreter has NO cell runtime at all: `TopLevel::Cell`
is parsed (definitions.rs:918) and contract-checked (typechecker:3258) but
never instantiated — `load_program` registers obj shapes only, spawn has no
cell path, nothing evaluates cell transactions or internal triggers. Cell
scheduling presumes cell instantiation; building that runtime is its own
first-class phase (B2), not a bolt-on here. This phase lands the event core
cells will consume.

## Changes

| File | Change |
|---|---|
| `src/errors.rs` | `RuntimeError::TaskBlocked` variant + Display arm |
| `src/interpreter/mod.rs` | `EventSlot.waiters`; `TaskStatus::Waiting`; `CURRENT_TASK` thread-local + `set_current_task`/`current_task_id`; `block_current_task_on_slot`; `fire_slot_wake`; `cancel_task`; `mark_done_with_result`; segment pre-split in `register_pending_task` (+ `mentions_param_field` walker); dead `TASK_TABLE` decl removed from eval.rs |
| `src/interpreter/eval.rs` | Field-on-unready-EventQ blocks inside tasks / strict error outside; fire drains waiters; both segment executors set/clear `CURRENT_TASK`, catch `TaskBlocked`, store results only on completion; `FreeHint` cancels table entry; spawn slot constructions gain `waiters: vec![]` |
| `spec/SPEC.md` | §9.5: firing wakes blocked consumers; §12.2: blocking-read + level-triggered wake semantics, deadlock posture, loop-gating limitation |
| `examples/async-events.bv` | NEW: producer/consumer over wired ports — the concurrent acceptance program |

## Tests (all in `src/interpreter/mod.rs` tests unless noted)

1. `blocked_read_suspends_then_fire_wakes` — consumer blocks on unready
   port; status `Waiting`; await returns handle (no hang); fire; second
   await yields the payload member.
2. `round_robin_wakes_blocked_consumer_mid_await` — one `await(consumer)`
   drives producer segments interleaved; consumer woken by producer's fire;
   final result correct; execution-order log proves interleaving.
3. `free_cancels_blocked_and_ready_tasks` — freed task never runs even when
   its port later fires; status `Cancelled`.
4. `unready_read_outside_task_stays_strict` — top-level TypeError preserved.
5. `segment_presplit_places_read_headed` — registration splits before the
   port-read statement; pre-read statements executed exactly once across a
   block/wake cycle (side-effect counter observable).
6. Existing suite green: `cargo test --lib`.

## Acceptance (from the async plan, updated)

- `examples/async-events.bv` compiles and runs with concurrent tasks via the
  reference interpreter (error-handling.bv itself contains no tasks — the new
  example carries the concurrency criterion).
- `yield;` suspends (existing A3 interleaving coverage + test 2).
- `free task` cancels at the next cancellation point (test 3; runtime-enforced).
- No benchmark regression risk: zero codegen paths touched (interpreter +
  errors only). Rule 12b experiment unnecessary — no compiled-output change;
  verified by `git diff --stat` scope claim in the closing commit message.

## Doc maintenance

- SPEC §9.5/§12.2 prose updated same commit.
- This plan records the cell deferral; `docs/plans/2026-08-23-async-scheduler.md`
  stays historical (never retro-edit).
- Rationale comments carry TEMP-style undo notes per house rule 16.

## Undo

Revert the three source files; the feature is additive behind `Waiting`/
`TaskBlocked`/`waiters` — no existing variant or arm changes meaning outside
the two disclosed behavior upgrades (unready-read-in-task, free-cancels),
both called out above.
