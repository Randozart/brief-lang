# 11 - Triggers and Reactive I/O

## What Are Triggers?

In Brief, triggers (`trg`) represent **external events** that can change at any time. They are the bridge between your verified state machine and the unpredictable outside world.

Unlike regular variables, triggers are **volatile** - the compiler cannot assume their value stays the same between reads. This makes them fundamentally different from `let` declarations.

## Top-Level Triggers

Top-level triggers are declared in the global scope and represent events that can wake up your reactive transaction loop:

```brief
// Hardware trigger (Embedded Brief)
trg button: Bool @ 0x1000A000;

// System trigger (Regular Brief)
trg sigint: Bool;
trg stdin_line: String;
trg clock_tick_1hz: Int;

// Network trigger
trg network_data: String;
```

### Trigger Aliases

Brief accepts multiple forms for trigger declarations:
- `trg` / `TRG` / `trigger` / `TRIGGER` - all equivalent for top-level triggers

```brief
trg button: Bool;       // lowercase
TRG button: Bool;       // uppercase
trigger button: Bool;   // full word
TRIGGER button: Bool;   // uppercase full word
```

## Local Triggers (`trg!`)

Local triggers are declared **inside transaction bodies** and represent mid-flight async waits. They require the `!` suffix as a psychological speedbump - you're introducing asynchronous chaos into a verified transaction.

```brief
txn fetch_user[user_requested] {
    let user_id = state.current_user;
    
    // Local trigger: await DB response
    // The ! warns: "this may cause a rollback!"
    trg! db_response: Result<Data, DbError> = fetch_from_db(user_id);
    
    [db_response.is_err()] {
        escape Err("DB Timeout");  // Rolls back all state changes
    };
    
    state.verified = true;
    term;
};
```

### Why the `!`?

The `!` suffix follows a tradition in language design:
- **Ruby**: `sort!` warns "this mutates in-place"
- **Rust**: `unsafe {}` warns "pointer math ahead"
- **Rust**: `println!` warns "this isn't a normal function"
- **Brief**: `trg!` warns "async rollback risk here"

If you forget the `!` inside a transaction, the compiler gives a helpful error:

```
Error: Local triggers introduce asynchronous rollback risks.
       You must use 'trg!' or 'trigger!' to explicitly acknowledge this boundary.
       (Top-level trigger declarations use 'trg' without '!')
```

### Local Trigger Aliases

- `trg!` / `TRG!` / `trigger!` / `TRIGGER!` - all equivalent for local triggers

## Volatile Semantics

The Brief compiler treats trigger variables as **volatile**:

1. **Each read creates a new symbolic value** - `x` read twice becomes `x@t1` and `x@t2`
2. **`x == x` is NOT assumed true** for trigger variables
3. **Proof engine cannot prove** conditions that depend solely on trigger values
4. **Stricter verification** on paths "polluted" by trigger variables

This means:
```brief
trg sensor: Int;

txn read_sensor[sensor > 0] {
    let a = sensor;
    let b = sensor;
    // The compiler does NOT assume a == b!
    // Each read may return a different value
};
```

## Pre-Evaluation Guards

Brief's reactor uses a **two-tier execution model** to avoid wasted FFI side effects:

**Tier 1: Pre-Evaluation Guard**
Before running a transaction, the reactor checks if any escape conditions are provably true based on currently-known state. If so, it **skips the transaction entirely** - no FFI calls fired, zero risk.

**Tier 2: Speculative Execution**
When escape conditions depend on unpredictable events (FFI responses, future triggers), the transaction runs speculatively. If escape hits mid-flight, state rolls back automatically.

```brief
trg button: Bool;
let counter: Int = 0;

rct txn handle_button[button == true] {
    // Pre-evaluation: if button is false, skip entirely
    // No FFI calls, no side effects
    
    [counter > 100] {
        escape;  // Would escape - skipped by pre-evaluation guard
    };
    
    &counter = counter + 1;
    term;
};
```

## System Triggers

Brief provides standard system triggers in `std/system.bv`:

| Trigger | Type | Description |
|---------|------|-------------|
| `sigint` | Bool | Ctrl+C signal |
| `sigterm` | Bool | Termination signal |
| `sighup` | Bool | Terminal hangup |
| `stdin_line` | String | Line available on stdin |
| `stdin_ready` | Bool | Stdin has data |
| `clock_tick_1hz` | Int | Fires once per second |
| `clock_tick_10hz` | Int | Fires 10 times/second |
| `clock_tick_100hz` | Int | Fires 100 times/second |
| `stdout_ready` | Bool | Stdout buffer has space |
| `file_event` | String | Watched file changed |
| `network_data` | String | Data arrived on socket |
| `network_connected` | Bool | Connection established |
| `network_disconnected` | Bool | Connection dropped |

### Example: Reactive Signal Handler

```brief
import std.system;

let shutting_down: Bool = false;
let tick_count: Int = 0;

// Handle Ctrl+C
rct txn handle_sigint[sigint == true] {
    &shutting_down = true;
    term;
};

// Count seconds
rct txn count_ticks[clock_tick_1hz > 0] {
    &tick_count = tick_count + 1;
    term;
};

// Graceful shutdown
rct txn graceful_shutdown[shutting_down == true] {
    println("Shutting down after {} seconds", tick_count);
    term;
};
```

## Trigger Configuration

Triggers map to OS events through DBVS schema files (`std/bindings/system_triggers.dbvs`):

```dbvs
register 0x100 as "sigint" {
    type: Trigger(Bool);
    location: "signal::SIGINT";
    target: native;
    trigger: {
        event_type: "signal";
        signal: "SIGINT";
        mode: "edge";
    }
}
```

## Escape and Rollback

When a transaction hits `escape`, all state modifications are **rolled back** to the pre-transaction snapshot:

```brief
txn transfer(amount: Int) [amount > 0][balance == @balance] {
    state.processing = true;  // Tentative change
    
    trg! result: Result<Void, Error> = send_payment(amount);
    
    [result.is_err()] {
        escape;  // state.processing rolls back to false!
    };
    
    &balance = balance - amount;
    term;  // Both changes commit atomically
};
```

## Key Differences: Top-Level vs Local Triggers

| Aspect | Top-Level `trg` | Local `trg!` |
|--------|----------------|--------------|
| Location | Global scope | Inside transaction |
| Syntax | `trg name: Type;` | `trg! name: Type = expr;` |
| Purpose | Wake up reactor | Mid-flight async wait |
| Rollback risk | None | Yes (requires `!`) |
| FFI side effects | N/A | May fire before escape |

## Best Practices

1. **Use top-level `trg`** for external events that drive your state machine
2. **Use `trg!` sparingly** - only when you truly need mid-transaction async waits
3. **Place `trg!` early** in transactions to minimize wasted FFI calls on escape
4. **Test with fault injection** - use `brief test` to verify your code handles trigger chaos
5. **Pre-evaluate escape conditions** - the compiler does this automatically, but design your guards to be checkable
