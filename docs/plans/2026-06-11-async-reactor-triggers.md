# Async / Reactor / Triggers — Implementation Plan

**Date:** 2026-06-11
**Status:** Plan — not yet implemented

## Current State

The AST has all the fields:
- `Transaction.is_async: bool`
- `Transaction.reactor_speed: Option<u32>` (parsed as `@NHz`)
- `Program.reactor_speed: Option<u32>`
- `TriggerDeclaration` struct (parsed from `trg name: Type @ link ...`)
- `Statement::LocalTrigger` (parsed from `trg!` inside transactions)

The interpreter ignores all of them:
- `is_async` set to `false` unconditionally in 3 locations
- `reactor_speed` set to `None` unconditionally in 3 locations
- `TopLevel::Trigger` not dispatched in interpreter's program loop
- `Statement::LocalTrigger` is a stub (evaluates expr, stores value, TODO comment)
- `Reactor::is_async` copied from Transaction but never drives behavior

## Design

### Three firing modes

| Mode | Trigger | When it fires | Semantics |
|------|---------|---------------|-----------|
| **Responsive** (`node`) | Dirty state | Immediately when a dependency changes. Runs convergence loop to quiescence. | Existing behavior, unchanged. |
| **Polled** (`async node @NHz`) | Timer | At N Hz regardless of dirty state. | Sugared timer-backed trigger. Same contract enforcement as responsive. |
| **Event-driven** (`trg @ link <ffi_fn>`) | FFI callback | When the linked FFI function returns non-void. | FFI callback marks trigger variable dirty → convergence runs. |

### Key insight

`@Hz` is syntactic sugar for a timer-backed trigger. The trigger model is
the unifying primitive:
- `trg keypress: String @ link tty_read_key` — FFI-backed trigger
- `trg tick: Bool @ 30Hz` — timer-backed trigger (desugared from async @30Hz)

The reactor becomes event-driven — it blocks on triggers (`epoll`/`select`
equivalent for FFI, timerfd for timers), and when one fires, the convergence
loop runs immediately.

## Implementation Phases

### Phase 1 — Wire `is_async` and `reactor_speed` in the interpreter

Files: `src/interpreter.rs`

- Find the 3 locations that construct ReactiveTransaction (around lines 3526,
  3594, 3648) and replace `is_async: false` with `is_async: txn.is_async` and
  `reactor_speed: None` with `reactor_speed: txn.reactor_speed`

```diff
- is_async: false,
+ is_async: txn.is_async,
- reactor_speed: None,
+ reactor_speed: txn.reactor_speed,
```

- Also update `Reactor::new()` in `src/reactor.rs` to receive the parsed
  values (currently copied at line 61 from `txn.is_async` — verify this
  works after the interpreter fix)

### Phase 2 — Wire `TopLevel::Trigger` in the interpreter

Files: `src/interpreter.rs`

Add a dispatch arm in the interpreter's main program loop for
`TopLevel::Trigger(t)`. Store trigger declarations in a map:
`HashMap<String, TriggerDeclaration>`.

When a trigger's name is referenced as a guard variable in a reactive
transaction, the interpreter should evaluate the trigger's linked FFI
function to produce its value.

```rust
// In the interpreter's run loop:
crate::ast::TopLevel::Trigger(trg) => {
    self.triggers.insert(trg.name.clone(), trg.clone());
}
```

### Phase 3 — Extend the Reactor for `@Hz` transactions

Files: `src/reactor.rs`

- Add `last_fired: Vec<Instant>` tracking per transaction index
- Add `fire_due_async_txns(&mut self, interp: &mut Interpreter) -> bool`
  that iterates transactions with `reactor_speed`, checks if their
  `@Hz` interval has elapsed since `last_fired`, and fires those whose
  interval has elapsed. Returns `true` if any fired.

```rust
pub fn fire_due_async_txns(&mut self, interp: &mut Interpreter) -> Result<bool, RuntimeError> {
    let mut fired = false;
    for (idx, txn) in self.transactions.iter().enumerate() {
        if let Some(hz) = txn.reactor_speed {
            let interval = Duration::from_millis(1000 / hz as u64);
            if self.last_fired[idx].elapsed() >= interval {
                self.run_single(interp, idx)?;
                self.last_fired[idx] = Instant::now();
                fired = true;
            }
        }
    }
    Ok(fired)
}
```

### Phase 4 — Continuous reactor loop

File: `src/reactor.rs`

Add `run_continuous(interp: &mut Interpreter) -> Result<(), RuntimeError>`:

```
loop {
    // (1) Responsive: convergence until quiescence
    reactor.run(interp)?;

    // (2) Polled: fire due async txns and cascade
    reactor.fire_due_async_txns(interp)?;
    reactor.run(interp)?;  // catch cascades

    // (3) Short yield — NOT a tick boundary
    //     Responsive txns fire immediately within (1)
    sleep_ms(1);
}
```

Only enters continuous mode when `reactor_speed` is present or any
transaction has `is_async = true`.

### Phase 5 — FFI-backed triggers (`trg @ link <ffi_fn>`)

Files: `src/reactor.rs`, `src/interpreter.rs`

- When a `TriggerDeclaration` has a `link` target, register an FFI callback
  using the existing FFI registry
- When the FFI function returns a non-void value, mark the trigger variable
  dirty and run the convergence loop
- This replaces the polling loop entirely for event-driven programs

### Phase 6 — `Statement::LocalTrigger` full implementation

File: `src/interpreter.rs`

The current stub (lines 1208-1216) evaluates the expression and stores the
value. Full implementation should:
- Register a nested scoped trigger that persists for the transaction's
  duration
- Add rollback support for transaction escape
- Support the `trg!` syntax within transaction bodies

## Files Affected

| Phase | File | Change |
|-------|------|--------|
| 1 | `src/interpreter.rs` | Wire `is_async` and `reactor_speed` from parsed Transaction |
| 1 | `src/reactor.rs` | Verify `Reactor::is_async` receives the correct value |
| 2 | `src/interpreter.rs` | Add `TopLevel::Trigger` dispatch, store trigger map |
| 3 | `src/reactor.rs` | Add `last_fired` timestamps, `fire_due_async_txns()` |
| 4 | `src/reactor.rs` | Add `run_continuous()` event-driven loop |
| 4 | `src/main.rs` | Use `run_continuous()` when async triggers or @Hz present |
| 5 | `src/reactor.rs` | FFI trigger callback registration and dispatch |
| 5 | `src/interpreter.rs` | FFI trigger storage and lookup |
| 6 | `src/interpreter.rs` | Full LocalTrigger implementation with rollback |
