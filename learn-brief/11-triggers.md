# 11 - Triggers and Reactive I/O

## What Are Triggers?

In Brief, triggers (`trg`) represent **external events** that can change at any time. They are the bridge between your verified state machine and the unpredictable outside world.

Unlike regular variables, triggers are **volatile** - the compiler cannot assume their value stays the same between reads. This makes them fundamentally different from `let` declarations.

## Top-Level Triggers

Top-level triggers are declared in the global scope and represent events that can wake up your reactive transaction loop:

```brief
// Hardware trigger (Embedded Brief)
trg button: Bool @ 0x1000A000;

// System trigger — linked to a runtime symbol (LLVM backend)
trg sigint: Bool @ link __sigint_flag;
trg stdin_ready: Bool @ link __stdin_ready;
trg clock_tick_1hz: Int @ link __timer_1hz;
```

### `@ link` — Binding to External Symbols

The `@ link sym` syntax binds a trigger to an external symbol defined by the runtime. The compiler emits:

- **LLVM backend**: `@sym = external global <type>, align N` — the linker resolves this
- **C backend**: `extern volatile <type> sym;`
- **Runtime**: `runtime/brief_rt.c` provides `volatile char __io_pending`, `volatile int64_t __timer_1hz`, etc.

The `@ link` mechanism is **zero-magic**: the compiler knows nothing about the symbol name. It emits whatever name you provide. The runtime/OS provides the definition. This lets any C/assembly symbol be used as a Brief trigger.

**Supported trigger types** for `@ link`:
| Brief Type | Runtime C Type | LLVM IR Type |
|-----------|----------------|-------------|
| `Bool` | `volatile char` | `i8` |
| `Int` | `volatile int64_t` | `i64` |
| `Char` | `volatile int32_t` | `i32` |
| `String` | `volatile char*` | `i8*` |

**Unsupported types** (e.g., `Float`, `List`) produce a compiler warning and fall back to `i8` storage.

### Trigger Aliases

Brief accepts multiple forms for trigger declarations:
- `trg` / `TRG` / `trigger` / `TRIGGER` - all equivalent for top-level triggers

```brief
trg button: Bool;       // lowercase
TRG button: Bool;       // uppercase
trigger button: Bool;   // full word
TRIGGER button: Bool;   // uppercase full word
```

## Event Model: `node` + `@ link`

The event-driven dispatch uses `node` (reactive transaction) with trigger-based preconditions:

```brief
import io from "std/io.bv";

node handle_input [io.io_ready] {
    let c = io.get_char();
    io.consume();
    term;
};
```

**Dispatch semantics** (first-true-wins fallthrough):
1. Evaluate all transaction preconditions in declaration order
2. Fire the **first** transaction whose precondition is true
3. Fall through to the next tick

**Parallel dispatch** (with `#pragma dispatch parallel`):
1. Evaluate ALL preconditions upfront
2. Fire every transaction whose precondition is true AND whose write set does not conflict with any already-fired transaction
3. Non-conflicting transactions fire in the same tick

### Zero-Magic Sleep

Blocking sleep is a library pattern, not a compiler intrinsic:

```brief
// lib/std/io.bv
frgn __wait_for_event() -> Void from "libruntime";
node __io_sleep [true] {
    __wait_for_event();
    term;
};
```

Because `[true]` is always the last precondition evaluated (it's declared last), `__io_sleep` only fires when no other transaction has work to do. This is the **equilibrium sleep** pattern.

## Local Triggers (`trg!`) — DEPRECATED

> **Deprecation notice**: `trg!` inside transaction bodies is deprecated. Use `@ link` triggers + `node` instead. The compiler emits a deprecation warning at parse time.

Local triggers were declared **inside transaction bodies** and represented mid-flight async waits. They required the `!` suffix as a psychological speedbump — warning "async rollback risk here".

```brief
// OLD pattern — deprecated:
txn fetch_user[user_requested] {
    trg! db_response: Result<Data, DbError> = fetch_from_db(user_id);
    [db_response.is_err()] {
        escape Err("DB Timeout");
    };
    term;
};

// NEW pattern — use @ link triggers:
trg db_response: Bool @ link __db_response;
node handle_db_response [db_response] {
    // ... handle response
    term;
};
```

### Why the `!` was used?

The `!` suffix follows a tradition in language design:
- **Ruby**: `sort!` warns "this mutates in-place"
- **Rust**: `unsafe {}` warns "pointer math ahead"
- **Brief**: `trg!` warned "async rollback risk here"

The modern event model (`@ link` + `node`) eliminates rollback risk by keeping triggers at the top level, making the entire reactive loop stateless with respect to event arrival.

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

node handle_button[button == true] {
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

Brief provides standard system triggers in `lib/std/system.bv`. These use `@ link` bindings to runtime symbols:

| Trigger | Type | `@ link` Symbol | Description |
|---------|------|-----------------|-------------|
| `sigint` | Bool | `__sigint_flag` | Ctrl+C signal |
| `sigterm` | Bool | `__sigterm_flag` | Termination signal |
| `sighup` | Bool | `__sighup_flag` | Terminal hangup |
| `stdin_ready` | Bool | `__stdin_ready` | Stdin has data |
| `stdin_buffer` | String | `__stdin_buffer` | Raw stdin data |
| `clock_tick_1hz` | Int | `__timer_1hz` | Fires once per second |
| `clock_tick_100hz` | Int | `__timer_100hz` | Fires 100 times/second |
| `__io_pending` | Bool | `__io_pending` | Any event pending |

Additional triggers in `io.bv`:
| Trigger | `@ link` Symbol | Description |
|---------|----------------|-------------|
| `__io_pending` | `__io_pending` | Set by runtime when any event arrives |

### Example: Reactive Event Handler (LLVM Backend)

```brief
import io from "std/io.bv";

let counter: Int = 0;

node count_input [io.io_ready] {
    let key = io.get_char();
    [key == " "] {
        &counter = counter + 1;
    };
    io.consume();
    term;
};

// Compile:     brief build --llvm program.bv
// Produces program.ll + brief_rt.c, then links to binary.
// Link:        ld program.o brief_rt.o -o program
```

## Trigger Configuration

Triggers map to OS events through:
1. **`@ link` symbol bindings** in `.bv` files (LLVM backend)
2. **`.dbv` binding files** for hardware targets: `std/bindings/system_triggers.dbv`

```brief
// Triggers are declared with `trg` in the BV source file:
trg sigint: Bool @ link __sigint_flag;
trg sigterm: Bool @ link __sigterm_flag;
trg stdin_ready: Bool @ link __stdin_ready;

// Bindings define the trigger implementation location
import bindings from "std/bindings/system_triggers.dbv";
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

## LLVM Backend + Runtime

The LLVM backend is the primary compilation target for event-driven Brief programs.

### Compile and Link

```bash
# Build (produces .ll + links with brief_rt)
brief build --llvm program.bv --out output/

# Or manually:
brief build --llvm program.bv --out output/
llc output/program.ll -filetype=obj -o output/program.o
cc -c runtime/brief_rt.c -o output/brief_rt.o
ld output/program.o output/brief_rt.o -o program
```

The `build --llvm` writes the embedded `brief_rt.c` source to the output directory, compiles it with `cc`, and prints the final `ld` command.

### Runtime Source

`runtime/brief_rt.c` is a single C file that provides:
- **`@ link` global definitions**: `volatile` variables in section `brief_trg` for signal handlers, timers, stdin
- **`__wait_for_event()`**: platform-optimized blocking sleep (epoll on Linux, kqueue on BSD, WFI on ARM, HLT on x86, fallback nanosleep)
- **Constructor**: auto-runs before `main()` to set up signal handlers and timers

### Platform Support

| Platform | Mechanism | Source |
|----------|-----------|--------|
| Linux | `epoll` + `signal` | `runtime/brief_rt.c` |
| macOS/BSD | `kqueue` + `signal` | `runtime/brief_rt.c` |
| ARM bare-metal | `WFI` instruction | `runtime/brief_rt.c` |
| x86 bare-metal | `STI; HLT` | `runtime/brief_rt.c` |
| WASM | `memory.grow` yield | `runtime/brief_rt.c` |
| Fallback | `nanosleep` (1ms) | `runtime/brief_rt.c` |

## Best Practices

1. **Use top-level `trg @ link`** for all external events — no rollback risk, verifiable by proof engine
2. **Import `io`** from `lib/std/io.bv` for the pump/sleep/accessor pattern
3. **Use `io.consume()`** after processing input to reset `io_ready`
4. **Place `__io_sleep` last in dispatch** — the `[true]` precondition ensures it fires only when idle
5. **Test with fault injection** to verify your code handles trigger chaos
