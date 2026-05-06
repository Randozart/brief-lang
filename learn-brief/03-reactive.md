# Reactive Transactions

Reactive transactions fire **automatically** when their precondition becomes true. No explicit call needed.

## 1. The `rct` Keyword

Add `rct` to make a transaction reactive:

```brief
// Passive transaction (must be called explicitly)
txn increment() [counter < 100][counter == @counter + 1] {
    &counter = counter + 1;
    term;
};

// Reactive transaction (fires automatically)
rct txn auto_increment() [counter < 100][counter == @counter + 1] {
    &counter = counter + 1;
    term;
};
```

**The difference:**
- Passive: You call `increment()`
- Reactive: Fires automatically when `counter < 100`

## 2. How Reactivity Works

The Brief runtime continuously checks all reactive transactions:

```
1. State changes (counter = 50)
2. Reactor scans reactive transactions
3. Finds auto_increment: [counter < 100] ✓
4. Fires auto_increment automatically
5. counter = 51
6. Loop back to step 2
```

This continues until no reactive transactions can fire (equilibrium).

## 3. Reactive Chains

Reactive transactions can trigger each other:

```brief
let count: Int = 0;
let done: Bool = false;

rct txn increment() [count < 10 && !done][count == @count + 1] {
    &count = count + 1;
    term;
};

rct txn finish() [count >= 10 && !done][done == true] {
    &done = true;
    println("Done!");
    term;
};

// Execution:
// increment fires 10 times (count: 0→10)
// finish fires once (done: false→true)
```

## 4. Mutual Exclusion

The compiler prevents reactive conflicts:

```brief
// ❌ This FAILS - mutual exclusion violation
rct async txn reader() [!writing][reading = true] { ... }
rct async txn writer() [!reading][writing = true] { ... }

// Error: Both transactions can fire simultaneously
// reader reads 'reading', writer writes 'reading'
```

**Fix with guards:**
```brief
// ✅ This PASSES
rct async txn reader() [!writing && readers == 0][readers = 1] { ... }
rct async txn writer() [readers == 0 && !writing][writing = true] { ... }
```

## 5. Async Reactive Transactions

Add `async` for concurrent execution:

```brief
rct async txn fetch_data() [needs_update][data != @data] {
    let result = http_get(url);
    [result.is_ok()] {
        &data = result.value;
    };
    term;
};

rct async txn process_data() [data != @processed_data][processed == true] {
    process(data);
    &processed_data = data;
    &processed = true;
    term;
};
```

Both can run concurrently (verified safe by compiler).

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

rct txn start() [state == State::Idle][state == State::Running] {
    &state = State::Running;
    term;
};

rct txn finish() [state == State::Running][state == State::Done] {
    &state = State::Done;
    term;
};

rct txn reset() [state == State::Done][state == State::Idle] {
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
    [i < observers.len()] {
        notify(observers[i], subject_value);
        &i = i + 1;
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
    [last_trigger == @current_time()]
{
    do_action();
    &last_trigger = current_time();
    term;
};
```

## 7. Debugging Reactive Code

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

## 8. Complete Example: Shopping Cart

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
1. `add_item` fires 11 times
2. `apply_bulk_discount` fires automatically (items > 10, total > 100)
3. Cart now has 10% discount applied

## Exercises

1. Create a reactive thermostat that turns on/off based on temperature
2. Build a traffic light system with reactive state transitions
3. Implement a reactive inventory system with auto-reorder

---

*Next: [04-functions.md](04-functions.md) - Functions with contracts*
