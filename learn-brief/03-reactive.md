# Reactive Transactions

Reactive transactions fire **automatically** when their precondition becomes true. The compiler proves they can terminate, then optimizes them to loop until termination.

## 1. The `rct` Keyword

Add `rct` to make a transaction reactive:

```brief
// Passive transaction (must be called explicitly)
txn increment [counter < 100][counter == @counter + 1] {
    &counter = counter + 1;
    term;
};

// Reactive transaction (fires automatically)
rct txn auto_increment [counter < 100][counter == @counter + 1] {
    &counter = counter + 1;
    term;
};
```

**How it works:**
1. Compiler verifies the postcondition can be satisfied (proves termination)
2. At runtime, transaction fires when precondition is true
3. **Loops until postcondition is met** (not just once!)
4. Only stops when `term` is reached with postcondition satisfied

## 2. Termination Verification

The compiler **proves** reactive transactions can terminate:

```brief
// ✅ VERIFIES - provably terminates
rct txn increment() [counter < 100][counter == @counter + 1] {
    &counter = counter + 1;
    term;
};
// Compiler proves: counter increases by 1 each iteration, will reach 100

// ❌ REJECTED - cannot prove termination
rct txn bad_increment() [counter < 100][counter == @counter + 1] {
    [counter < 50] {
        &counter = counter + 1;
    };
    // No else branch - might not satisfy postcondition!
    term;
};
// Error: Postcondition not satisfied on all paths
```

## 3. Optimized Execution

Once termination is proven, the compiler optimizes:

```brief
rct txn fill_buffer() [buffer :> Size < 100][buffer :> Size == 100] {
    &buffer = buffer.append(read_item());
    term;
};
```

**Compilation:**
```rust
// Optimized loop (no repeated precondition checks needed)
while buffer :> Size < 100 {
    buffer.append(read_item());
    // Compiler knows this WILL reach 100
}
```

## 4. Reactive Chains

Reactive transactions can trigger each other:

```brief
let count: Int = 0;
let done: Bool = false;

rct txn increment [count < 10 && !done][count == @count + 1] {
    &count = count + 1;
    term;
};

rct txn finish [count >= 10 && !done][done == true] {
    &done = true;
    term;
};
```

**Execution:**
1. `increment` fires repeatedly (count: 0→10)
2. When count >= 10, `increment` precondition fails
3. `finish` precondition becomes true
4. `finish` fires once, sets `done = true`
5. Equilibrium reached (no more transactions can fire)

## 5. Async Reactive Transactions

Add `async` for concurrent execution (compiler verifies safety):

```brief
let needs_update: Bool = false;
let data: Int = 0;
let processed_data: Int = -1;
let processed: Bool = false;

rct async txn fetch_data [needs_update][data != @data] {
    &data = data + 1;
    term;
};

rct async txn process_data [data != processed_data][processed == true] {
    &processed_data = data;
    &processed = true;
    term;
};
```

**Compiler verifies:**
- No race conditions (mutual exclusion)
- No deadlocks (no circular dependencies)
- Both can terminate independently

## 6. Common Patterns

### Event Handler
```brief
rct txn on_button_click() [button_clicked][handled == true] {
    do_something();
    &button_clicked = false;
    &handled = true;
    term;
};
```

### State Machine
```brief
enum State { Idle, Running, Done }
let state: State = State::Idle;

rct txn start [state == State::Idle][state == State::Running] {
    &state = State::Running;
    term;
};

rct txn finish [state == State::Running][state == State::Done] {
    &state = State::Done;
    term;
};

rct txn reset [state == State::Done][state == State::Idle] {
    &state = State::Idle;
    term;
};
```

### Observer Pattern
```brief
let observers: List<String> = [];
let subject_value: Int = 0;

rct txn notify_observers() [subject_value != @notified_value][true] {
    let i: Int = 0;
    [i < observers :> Size] {
        notify(observers[i], subject_value);
        i = i + 1;
    };
    &notified_value = subject_value;
    term;
};
```

### Debouncer
```brief
let last_trigger: Int = 0;
let debounce_time: Int = 100;  // ms

rct txn debounced_action() 
    [current_time() - last_trigger > debounce_time]
    [last_trigger == current_time()]
{
    do_action();
    &last_trigger = current_time();
    term;
};
```

## 7. Polling Mode (`@Hz`)

By default, reactive transactions fire on **dependency changes** — the system tracks which variables each transaction's precondition reads, and only evaluates dirty transactions. This is the reactive equilibrium model.

Sometimes you need a **fixed tick rate** instead — e.g., sensor polling, animation frames, watchdog timers. Add `@Hz` to opt into polling:

```brief
// Reactive (default): fires when precondition changes
rct txn on_signal [signal][handled == true] {
    &handled = true;
    term;
};

// Polling: fires every 10ms regardless of precondition state
rct txn read_sensor @100Hz [true][logged == true] {
    &value = read_adc();
    &logged = true;
    term;
};
```

**How polling works:**
1. The `@Hz` annotation attaches a speed requirement to the transaction
2. Multiple files with different `@Hz` speeds are coordinated by the `ReactorScheduler` — the global tick runs at max(`@Hz`) and slower files are intelligently skipped
3. Polling transactions still use the same reactive pipeline (precondition check, term verification, equilibrium loop) — `@Hz` only adds a time-based gate
4. Pure library files with no `rct` blocks consume zero overhead

**When to use polling:**
- Hardware polling (ADC, GPIO, I2C)
- Timer-driven logic
- Animation/rendering at fixed frame rates
- Watchdog or heartbeat patterns

**Comparison:**

| Mode | Syntax | Fires when | Use case |
|------|--------|-----------|----------|
| Passive | `txn` | Explicit call only | API, callbacks |
| Reactive | `rct txn` | Precondition becomes true | State machines, event handlers |
| Polling | `rct txn @Hz` | Precondition true + tick interval met | Sensors, timers, animation |

## 8. Debugging Reactive Code

Add logging transactions:

```brief
rct txn log_state() [true][true] {
    println("Counter: " + String(counter));
    println("Active: " + String(active));
    term;
};
```

Or use explicit state checks:

```brief
rct txn check_invariants() [true][true] {
    [counter >= 0] {
        // Invariant holds
    };
    [counter < 0] {
        escape;  // Invariant violated!
    };
    term;
};
```

## 9. Complete Example: Shopping Cart

```brief
// shopping_cart.bv
let items: Int = 0;
let total: Float = 0.0;
let discount_applied: Bool = false;

rct txn add_item(price: Float) [true][items == @items + 1] {
    &items = items + 1;
    &total = total + price;
    term;
};

rct txn remove_item(price: Float) [items > 0][items == @items - 1] {
    &items = items - 1;
    &total = total - price;
    term;
};

rct txn apply_bulk_discount() 
    [items > 10 && total > 100.0 && !discount_applied]
    [total < @total && discount_applied == true]
{
    let discount = total * 0.1;
    &total = total - discount;
    &discount_applied = true;
    term;
};

rct txn clear_cart() [items > 0][items == 0 && total == 0.0] {
    &items = 0;
    &total = 0.0;
    &discount_applied = false;
    term;
};
```

**Reactive chain:**
1. `add_item` fires 11 times (items: 0→11, total accumulates)
2. When items > 10 AND total > 100, `apply_bulk_discount` precondition true
3. `apply_bulk_discount` fires, applies 10% discount
4. Equilibrium: no more transactions can fire

## 10. Exercises

1. Create a reactive thermostat that turns on/off based on temperature
2. Build a traffic light system with reactive state transitions
3. Implement a reactive inventory system with auto-reorder

---

*Next: [04-functions.md](04-functions.md) - Functions with contracts*
