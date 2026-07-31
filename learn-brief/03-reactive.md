# Reactive Transactions

Reactive transactions fire **automatically** when their precondition becomes true. The compiler proves they can terminate, then optimizes them to loop until termination.

## 1. The `node` Keyword

A `node` is a reactive transaction — it fires **automatically** when its
precondition becomes true, with no parameters and no return value:

```brief
// Callable (must be called by a txn or node)
txn increment [counter < 100][counter == @counter + 1] {
    &counter = counter + 1;
    term;
};

// Reactive (fires automatically)
node auto_increment [counter < 100][counter == @counter + 1] {
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
node increment [counter < 100][counter == @counter + 1] {
    &counter = counter + 1;
    term;
};
// Compiler proves: counter increases by 1 each iteration, will reach 100

// ❌ REJECTED - cannot prove termination
node bad_increment [counter < 100][counter == @counter + 1] {
    when counter < 50 {
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
node fill_buffer [buffer .^Len < 100][buffer .^Len == 100] {
    &buffer = buffer.append(read_item());
    term;
};
```

**Compilation:**
```rust
// Optimized loop (no repeated precondition checks needed)
while buffer .^Len < 100 {
    buffer.append(read_item());
    // Compiler knows this WILL reach 100
}
```

## 4. Reactive Chains

Reactive transactions can trigger each other:

```brief
let count: Int = 0;
let done: Bool = false;

node increment [count < 10 && !done][count == @count + 1] {
    &count = count + 1;
    term;
};

node finish [count >= 10 && !done][done == true] {
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

async node fetch_data [needs_update][data != @data] {
    &data = data + 1;
    term;
};

async node process_data [data != processed_data][processed == true] {
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
node on_button_click [button_clicked][handled == true] {
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

node start [state == State::Idle][state == State::Running] {
    &state = State::Running;
    term;
};

node finish [state == State::Running][state == State::Done] {
    &state = State::Done;
    term;
};

node reset [state == State::Done][state == State::Idle] {
    &state = State::Idle;
    term;
};
```

### Observer Pattern
```brief
let observers: List<String> = [];
let subject_value: Int = 0;

node notify_observers [subject_value != @notified_value][true] {
    let i: Int = 0;
    when i < observers .^Len {
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

node debounced_action 
    [current_time() - last_trigger > debounce_time]
    [last_trigger == current_time()]
{
    do_action();
    &last_trigger = current_time();
    term;
};
```

## 7. Polling

Reactive transactions fire on **dependency changes** — the system tracks which
variables each transaction's precondition reads, and only evaluates dirty
transactions. This is the reactive equilibrium model.

Fixed tick-rate polling (a `@Hz` annotation) is a planned scheduler feature;
today all firing is dependency-driven. Hardware-polling loops are written
with an explicit counter node instead:

```brief
// Explicit tick-driven polling loop
node poll_sensor [sample_count < total][sample_count == total] {
    &value = read_adc();
    &sample_count = sample_count + 1;
    term;
};
```

**When you might want polling:**
- Hardware polling (ADC, GPIO, I2C)
- Timer-driven logic
- Animation/rendering at fixed frame rates

**Comparison:**

| Mode | Syntax | Fires when | Use case |
|------|--------|-----------|----------|
| Callable | `txn` | Called by a `txn` or `node` only | API, callbacks |
| Reactive | `node` | Precondition becomes true | State machines, event handlers |

## 8. Debugging Reactive Code

Log from inside a driver node with a `when` guard (a node whose precondition
it doesn't change would re-fire forever):

```brief
let total: Int = GetEnvInt!("BOUND");

node tick [counter < total][counter == total] {
    when counter % 100 == 0 {
        __print_int(counter);
        __print_char(10);
    };
    counter = counter + 1;
    term;
};
```

Or use explicit state checks with `escape` for rollback:

```brief
node check_invariants [counter >= 0][counter >= 0] {
    when counter < 0 {
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

txn add_item(price: Float) [true][items == @items + 1] {
    &items = items + 1;
    &total = total + price;
    term;
};

txn remove_item(price: Float) [items > 0][items == @items - 1] {
    &items = items - 1;
    &total = total - price;
    term;
};

node apply_bulk_discount 
    [items > 10 && total > 100.0 && !discount_applied]
    [total < @total && discount_applied == true]
{
    let discount = total * 0.1;
    &total = total - discount;
    &discount_applied = true;
    term;
};

node clear_cart [items > 0][items == 0 && total == 0.0] {
    &items = 0;
    &total = 0.0;
    &discount_applied = false;
    term;
};
```

`add_item`/`remove_item` are callable `txn`s (they take a price parameter);
`apply_bulk_discount`/`clear_cart` are reactive `node`s that fire on
precondition changes.

**Reactive chain:**
1. A driver node calls `add_item` 11 times (items: 0→11, total accumulates)
2. When items > 10 AND total > 100, `apply_bulk_discount` precondition true
3. `apply_bulk_discount` fires, applies 10% discount
4. Equilibrium: no more transactions can fire

## 10. Exercises

1. Create a reactive thermostat that turns on/off based on temperature
2. Build a traffic light system with reactive state transitions
3. Implement a reactive inventory system with auto-reorder

---

*Next: [04-functions.md](04-functions.md) - Functions with contracts*
