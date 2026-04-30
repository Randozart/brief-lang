# Brief Compiler: Unified Signal & Polling Architecture Plan

## Executive Summary

This document outlines the plan to transition the Brief compiler from a poll-loop architecture to a unified signal-driven and polling model. The goal is to improve performance by eliminating unnecessary polling loops while retaining the flexibility of time-based reactions (e.g., for gaming loops).

### Current State
- **Frontend:** Uses a `requestAnimationFrame` loop to poll WASM for dispatch updates.
- **Reactor:** Uses a `ReactorScheduler` to manage polling rates (`@10Hz`, `@60Hz`, etc.).
- **Syntax:** Requires `Hz` suffix (e.g., `reactor @10Hz;`).
- **Async:** Sequential execution with mutual exclusion (verified by borrow-checker).

### Target State
- **Frontend:** Event-driven; updates only when signal changes occur.
- **Reactor:** Hybrid model:
  - **Signal-driven (default):** Transactions fire only when subscribed state variables change.
  - **Polling mode (`@`):** Transactions run at fixed intervals (e.g., `@10`, `@30`).
- **Syntax:** Support `reactor @10;` (optional `Hz` suffix).
- **Async:** True concurrency using Rust async/await.

---

## 1. Signal Graph Implementation

### Goal
Track dependencies and trigger transactions based on signal updates.

### Files
- **New:** `src/signal_graph.rs`
- **Modify:** `src/ast.rs`, `src/wasm_gen.rs`

### Details

#### 1.1 Define `SignalGraph` Structure
Create a new file `src/signal_graph.rs`:

```rust
use std::collections::{HashMap, HashSet};
use wasm_bindgen::JsValue;

pub struct SignalGraph {
    // Maps signal name -> Set of transaction names that depend on it
    subscribers: HashMap<String, HashSet<String>>,
    // Current state of signals
    values: HashMap<String, JsValue>,
}

impl SignalGraph {
    pub fn new() -> Self {
        Self {
            subscribers: HashMap::new(),
            values: HashMap::new(),
        }
    }

    pub fn subscribe(&mut self, signal: &str, txn_name: &str) {
        self.subscribers
            .entry(signal.to_string())
            .or_default()
            .insert(txn_name.to_string());
    }

    pub fn update_signal(&mut self, signal: &str, value: JsValue) -> Vec<String> {
        self.values.insert(signal.to_string(), value);
        self.subscribers
            .get(signal)
            .map(|set| set.iter().cloned().collect())
            .unwrap_or_default()
    }

    pub fn get_value(&self, signal: &str) -> Option<&JsValue> {
        self.values.get(signal)
    }
}
```

#### 1.2 Update `Transaction` Struct
In `src/ast.rs`, add a `dependencies` field to the `Transaction` struct:

```rust
pub struct Transaction {
    pub is_async: bool,
    pub is_reactive: bool,
    pub name: String,
    pub parameters: Vec<(String, Type)>,
    pub contract: Contract,
    pub body: Vec<Statement>,
    pub reactor_speed: Option<u32>, // Existing field
    pub span: Option<Span>,
    pub is_lambda: bool,
    pub dependencies: Vec<String>, // New field: variables read in preconditions
}
```

#### 1.3 Integrate `SignalGraph` into `State`
In `src/wasm_gen.rs`, add `signal_graph` to the `State` struct:

```rust
#[wasm_bindgen]
pub struct State {
    signals: Vec<JsValue>,
    dirty_signals: Vec<bool>,
    // ... existing fields ...
    signal_graph: SignalGraph, // New field
}
```

Update `State::new()` to initialize `signal_graph`.

#### 1.4 Track Dependencies During Parsing
In `src/parser.rs`, when parsing `rct` preconditions, extract variable names and populate `Transaction.dependencies`.

Example:
```rust
// Pseudo-code for parsing preconditions
fn extract_dependencies(expr: &Expr) -> Vec<String> {
    // Recursively traverse expr to find identifiers
    // Return list of unique variable names
}
```

---

## 2. Reactor Logic Integration

### Goal
Combine signal-driven and polling triggers in the reactor.

### Files
- **Modify:** `src/wasm_gen.rs`

### Details

#### 2.1 Signal-Driven Dispatch
When an assignment (`&count = 11`) is executed:

1.  Update the signal value in `self.signals[id]`.
2.  Call `self.mark_dirty(id)` (existing logic).
3.  **New:** Call `self.signal_graph.update_signal("count", value)`.
4.  For each affected transaction:
    - If `poll_rate.is_none()` (signal-driven), evaluate preconditions immediately.
    - If `poll_rate.is_some()` (polling), mark as "dirty" for next scheduler tick.

#### 2.2 Polling-Driven Dispatch
The existing `ReactorScheduler` continues to manage polling transactions:

1.  On each tick, check `scheduler.should_check_file(file_id)`.
2.  If true, evaluate preconditions for all polling transactions in that file.

#### 2.3 Unified Dispatch Loop
Pseudo-code for the reactor loop:

```rust
loop {
    // 1. Check for signal updates (non-blocking)
    if let Some(signal_update) = check_signal_update() {
        let subscribers = signal_graph.update_signal(signal_update.name, signal_update.value);
        for txn_name in subscribers {
            let txn = get_transaction(txn_name);
            if txn.poll_rate.is_none() { // Signal-driven
                evaluate_and_fire(txn);
            } else {
                mark_dirty(txn); // Polling-based, wait for scheduler
            }
        }
    }

    // 2. Check scheduler for polling transactions
    scheduler.tick();
    for file_id in active_files {
        if scheduler.should_check_file(file_id) {
            for txn in get_polling_transactions(file_id) {
                evaluate_and_fire(txn);
            }
        }
    }
}
```

---

## 3. Syntax Updates

### Goal
Support `reactor @10;` (without "Hz") as requested.

### Files
- **Modify:** `src/parser.rs`

### Details

#### 3.1 File-Level Reactor
Update the parser to accept `reactor @10;` (optional "Hz" suffix):

```rust
// In parser.rs, around line 127
if let Some(Ok(Token::Identifier(name))) = self.current_token() {
    if name == "reactor" {
        self.advance(); // consume 'reactor'
        self.expect(Token::At)?;

        // Parse the speed number
        if let Some(Ok(Token::Integer(speed_num))) = self.current_token() {
            let speed = *speed_num as u32;
            self.advance();

            // Optional 'Hz' suffix
            if let Some(Ok(Token::Identifier(hz))) = self.current_token() {
                if hz == "Hz" {
                    self.advance();
                } else if hz != "Hz" {
                    // If it's not "Hz", it might be the next token (e.g., semicolon)
                    // We should not consume it if it's not "Hz"
                    // But parser logic needs to handle this carefully
                }
            }
            // ... validation ...
            reactor_speed = Some(speed);
            self.expect(Token::Semicolon)?;
        }
    }
}
```

#### 3.2 Per-Transaction Reactor
Update the parser to accept `rct txn ... @10;` (optional "Hz" suffix):

```rust
// In parser.rs, around line 1090
let reactor_speed = if is_reactive && matches!(self.current_token(), Some(Ok(Token::At))) {
    self.advance(); // consume @

    if let Some(Ok(Token::Integer(speed_num))) = self.current_token() {
        let speed = *speed_num as u32;
        self.advance();

        // Optional 'Hz' suffix
        if let Some(Ok(Token::Identifier(hz))) = self.current_token() {
            if hz == "Hz" {
                self.advance();
            }
        }
        // ... validation ...
        Some(speed)
    } else {
        None
    }
} else {
    None
};
```

---

## 4. Async Transaction Concurrency

### Goal
True concurrency for async transactions using Rust async/await.

### Files
- **Modify:** `src/reactor.rs`, `src/interpreter.rs`

### Details

#### 4.1 Async Task Scheduler
Use `tokio` for async runtime (add to `Cargo.toml`):

```toml
[dependencies]
tokio = { version = "1", features = ["full"] }
```

#### 4.2 Update `ReactiveTransaction`
Add an `async` method to execute transactions:

```rust
impl ReactiveTransaction {
    pub async fn run_async(&self, interp: &mut Interpreter) -> Result<bool, RuntimeError> {
        // Execute transaction body asynchronously
        // Non-async calls within the body block this task, not the reactor
    }
}
```

#### 4.3 Reactor Loop with Concurrency
Update the reactor loop to spawn async tasks:

```rust
use tokio::task;

async fn run_reactor() {
    loop {
        // ... signal-driven dispatch ...

        // Spawn async transactions
        for txn in async_transactions {
            let interp_clone = interp.clone(); // Ensure thread safety
            task::spawn(async move {
                txn.run_async(&mut interp_clone).await;
            });
        }

        // ... polling-driven dispatch ...
    }
}
```

#### 4.4 Borrow-Checker Enforcement
The compiler's borrow-checker already prevents race conditions:
- Async transactions cannot run simultaneously if they conflict on shared variables.
- This is verified at compile time; no runtime locks needed.

---

## 5. Frontend Integration

### Goal
Eliminate poll loop in `landing_glue.js`.

### Files
- **Modify:** `landing-build/landing_glue.js`
- **Modify:** `src/wasm_gen.rs` (WASM bindings)

### Details

#### 5.1 WASM Bindings
Add a new method to `State` in `src/wasm_gen.rs`:

```rust
#[wasm_bindgen]
impl State {
    // ... existing methods ...

    pub fn get_pending_dispatch(&mut self) -> JsValue {
        // Block until a signal change or timer tick occurs
        // Return dispatch instructions as JSON
        // This replaces the poll loop
    }
}
```

#### 5.2 JavaScript Update
Replace the poll loop in `landing_glue.js`:

```javascript
// Old code:
function startPollLoop() {
    function poll() {
        const dispatch = wasm.poll_dispatch();
        if (dispatch && dispatch !== '[]') {
            applyInstructions(JSON.parse(dispatch));
        }
        requestAnimationFrame(poll);
    }
    requestAnimationFrame(poll);
}

// New code:
async function startEventLoop() {
    while (true) {
        const dispatch = await wasm.get_pending_dispatch();
        if (dispatch && dispatch !== '[]') {
            applyInstructions(JSON.parse(dispatch));
        }
    }
}
```

---

## 6. Trade-offs and Considerations

| Aspect | Signal-Driven | Polling (`@`) | Unified Approach |
|--------|---------------|---------------|------------------|
| **CPU Usage** | Low (event-based) | Medium (fixed rate) | Adaptive |
| **Latency** | Immediate | Frame-dependent | Configurable |
| **Complexity** | Higher (dependency tracking) | Lower (simple scheduler) | Moderate |
| **Gaming Fit** | Poor (variable update) | Excellent (fixed update) | Good (via `@X`) |
| **UI Fit** | Excellent | Good (throttling) | Excellent |

---

## 7. Implementation Order

1.  **Signal Graph:** Create `src/signal_graph.rs` and integrate into `State`.
2.  **Syntax Updates:** Modify parser to support `reactor @10;`.
3.  **Reactor Logic:** Integrate signal-driven and polling dispatch.
4.  **Async Concurrency:** Implement async task scheduling with `tokio`.
5.  **Frontend Integration:** Replace poll loop with event-driven model.

---

## 8. Clarifying Questions (To Confirm Before Implementation)

1.  **Reactor Syntax:** Confirm support for `reactor @10;` (without "Hz").
2.  **Async Runtime:** Use `tokio` (recommended) or `async-std`?
3.  **Frontend Blocking:** Should `get_pending_dispatch()` block in WASM until signal/timer?
4.  **Default Polling Rate:** What should be the default for `@` (without number)? Current is 10Hz.

---

## 9. Next Steps

Awaiting your command to begin implementation.
