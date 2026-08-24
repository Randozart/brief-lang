# Async Scheduler — Sub-Plan (Track C)

**Date:** 2026-08-23
**Parent:** `docs/plans/2026-08-22-spec-conformance.md` + readiness assessment
**Status:** plan — implementation not started
**Priority:** Highest remaining language arc

## Why this matters

Briev's reactive identity is design-complete but runtime-eager. Tasks
spawn and run inline; `yield;` is a no-op; `free task` enforces ownership
but cancels nothing; port events are delivered synchronously. The SPEC's
central promise — a concurrent reactive language with proof-carrying
contracts — requires the scheduler to make these real.

## What exists today (foundation)

- Reactor model with goal-based termination (`reactor.rs`)
- Port model: `Value::EventQ` shared slots, ArrowAssign fires, `.Ready` reads
- Task handles: linear, typechecked, capability-staged for LLVM
- `yield;` parsed as no-op statement (SPEC §12.2 checkpoint)
- XOR concurrency gate: proves which nodes can't co-fire
- Deterministic reference scheduler in the interpreter
- Node/txn contracts proven by the frontend (termination, reachability)

## Architecture direction

### Execution model

The scheduler multiplexes tasks as cooperative coroutines on a single
thread (deterministic, matching the reference semantics). Each task owns
a stack frame set; `yield;` saves the continuation and returns control to
the scheduler loop. The scheduler picks the next ready task by priority
(port-waiting > timed > runnable).

### Key components

1. **Task table** — maps spawn handles to coroutine state (saved stack,
   status: Ready/Waiting/Suspended/Done)
2. **Event queue integration** — `Value::EventQ` becomes a scheduler-
   visible wait channel; a task blocked on `.Ready` yields until the
   producer fires
3. **`yield;` lowering** — interp: save continuation, return to scheduler;
   LLVM: call `@briev_yield()` which swaps stack context
4. **Port event delivery** — producer `<-` writes wake waiting consumers;
   delivery order = scheduler order (deterministic, no implicit concurrency)
5. **`free task` gate activation** — cancellation points become real:
   the scheduler checks a cancel flag at every `yield;` and unwinds via
   defer if set

### What does NOT change

- Contracts stay compile-time proofs (no runtime contract checking)
- The XOR concurrency gate stays global (scheduler respects it)
- The interpreter remains the reference implementation
- LLVM backend gets async support LAST (after interp is proven)

## Phased implementation

### Phase A — Interp coroutine scheduler
- Task table in `Interpreter`
- `spawn defn(...)` creates a suspended coroutine instead of running inline
- Scheduler loop: pick next ready → resume until yield/term/endprogram
- `await task` suspends caller until target completes
- `free task` sets cancel flag; checked at `yield;` points
- Existing tests must pass unchanged (the reactor API is preserved)

### Phase B — Port event integration
- EventQ gains a waiter list; producer `<-` wakes consumers
- Input ports become blocking reads when empty (`.Ready` gates prevent this)
- Cell internal nodes schedule as tasks on their instance identity

### Phase C — LLVM async lowering
- Stack switching via `@briev_yield` / `@briev_resume`
- Task handles lower to scheduler-visible indices
- Capability flag `async` flips TRUE

## Acceptance criteria

- examples/error-handling.bv compiles AND runs with concurrent tasks
- `yield;` actually suspends (verified by interleaving test)
- `free task` cancels at the next `yield;` point (verified by cleanup order)
- All existing tests pass without modification
- Benchmarks show no regression on non-async programs
