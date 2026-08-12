# Briev Standard Library Redesign

## Goal

Separate what Briev can do natively from what requires FFI to Rust. Build a stdlib that leverages Briev's strengths: reactive state management, transactions, contracts.

## What Briev Can Handle Natively

### State & Transactions ✓
- Global state management (via `let`, `const`)
- State transitions with verified contracts
- Reactive transactions that fire automatically
- Atomic rollback on postcondition failure
- Multi-variable state coordination

**Example**: A counter that automatically increments while conditions hold
```briev
let count: Int = 0;
node increment [count < 100] [count == @count + 1] {
  &count = count + 1;
  term;
};
```

### Computation ✓
- Arithmetic: `+`, `-`, `*`, `/`
- Comparisons: `==`, `!=`, `<`, `<=`, `>`, `>=`
- Logic: `&&`, `||`, `!`
- Pattern matching via unification

**Example**: Compute derived values
```briev
defn absolute_value(x: Int) -> Int [true][result >= 0] {
  [x < 0] term -x;
  [x >= 0] term x;
};
```

### Control Flow ✓
- Guards: `[condition] statement`
- Unification patterns: `Pattern(x) = expr`
- Transaction flow with term/escape

### Type Safety ✓
- Type checking at compile time
- Union types for multiple outcomes
- Contract-bound types

### Concurrency (Lock-Free) ✓
- Reactive transactions (no mutexes needed)
- STM rollback handles conflicts
- Preconditions act as gates

## What Briev Cannot Handle (Needs FFI)

### I/O Operations ✗
- File reading/writing
- Network operations
- Console output (in browser context)
- Database queries
- Anything that talks to the OS or network

**Why**: Briev doesn't have I/O primitives. These are external capabilities.

### Math Functions (Complex) ✗
- Trigonometry: sin, cos, tan
- Logarithms, exponentials
- Square roots, powers
- Floating point operations beyond basic arithmetic

**Why**: These are CPU operations, not state operations. Briev's arithmetic is integers and comparison.

### String Manipulation (Complex) ✗
- String length, substring, replace
- Case conversion, trimming
- Parsing (string → number)
- Concatenation (can be done but inefficient)

**Why**: Briev has no string operations. These are utility functions.

### Time ✗
- Getting current time
- Measuring elapsed time
- Sleeping

**Why**: External to Briev. Time comes from the runtime/OS.

### Random Numbers ✗
- RNG seeding
- Random integer/float generation

**Why**: Non-deterministic. Can't be proven in Briev.

### Collections (Partially) ✗
- Lists/arrays: Briev has `Data` type but no operations on it
- Maps/dictionaries: Not supported

**Why**: Briev treats collections as opaque `Data`. Operations would need native support.

## Proposed Stdlib Architecture

### Tier 1: Native Briev (No FFI Needed)

**Module: `briev::core`**
- State management patterns
- Transaction templates
- Common guards/contracts

Example:
```briev
# Built-in pattern: Initialize on demand
defn get_or_init(initialized: Bool, init_fn: ... -> ...) -> ... [true][initialized] {
  [initialized] term ...;
  [!initialized] { 
    let result = init_fn();
    term result;
  };
};
```

**Module: `briev::math`** (Integer math only)
- `absolute(x: Int) -> Int`
- `min(a: Int, b: Int) -> Int`
- `max(a: Int, b: Int) -> Int`
- `clamp(x: Int, min: Int, max: Int) -> Int`

All implemented as pure Briev functions with proven contracts.

### Tier 2: FFI to Rust (Current Stdlib)

These genuinely need Rust because Briev can't do I/O or call CPU functions.

**Module: `briev::io`** (FFI)
- `read_file(path: String) -> Result<String, IoError>`
- `write_file(path: String, content: String) -> Result<Void, IoError>`
- Other file operations

**Module: `briev::math`** (FFI)
- `sqrt(x: Float) -> Result<Float, MathError>`
- `sin(x: Float) -> Result<Float, MathError>`
- `pow(base: Float, exp: Float) -> Result<Float, MathError>`
- etc.

**Module: `briev::string`** (FFI)
- `length(s: String) -> Result<Int, StringError>`
- `substring(s: String, start: Int, len: Int) -> Result<String, StringError>`
- `to_upper(s: String) -> Result<String, StringError>`
- etc.

**Module: `briev::time`** (FFI)
- `current_time() -> Result<Int, TimeError>`
- `sleep(ms: Int) -> Result<Void, TimeError>`

### Tier 3: Planned (Future)

What we could add later:

**Module: `briev::collections`** (FFI or native?)
- List operations: append, map, filter, fold
- Dictionary operations: get, set, keys
- Decision: Do we add native collection support to Briev, or FFI them?

**Module: `briev::random`** (FFI)
- `random_int(min: Int, max: Int) -> Result<Int, RandomError>`
- Note: Can't be proven in Briev, but can be called

**Module: `briev::crypto`** (FFI)
- Hash functions
- Encryption (if needed)

**Module: `briev::json`** (FFI)
- Parse JSON
- Stringify values

## Implementation Plan

### Phase 1: Audit Current Stdlib

Review `std/bindings/*.toml`:
1. Identify functions that could be native Briev
2. Identify functions that genuinely need FFI
3. Separate them properly

### Phase 2: Create Native Briev Stdlib

Create `std/core.bv`:
- Integer math functions (all with proven contracts)
- Common state patterns
- Transaction templates

### Phase 3: Refactor FFI Stdlib

Keep only what genuinely needs Rust:
- I/O operations
- Complex math (sin, sqrt, etc.)
- String utilities
- Time operations
- Random numbers

### Phase 4: Document the Distinction

Make it clear in docs:
- When to use native Briev functions
- When to use FFI functions
- Why the distinction matters

## Benefits of This Approach

1. **Correct by construction**: Native Briev functions have proven contracts
2. **No runtime surprises**: Everything Briev does is verified at compile time
3. **Performance**: Native functions don't cross FFI boundary
4. **Teachable**: Shows what Briev excels at
5. **Maintainable**: Clear separation of concerns

## Example: State Machine Library

We could write a native Briev library for common patterns:

```briev
# State machine template
defn state_machine(state: Int, event: Int) 
  -> Int 
  [valid_state(state) && valid_event(event)]
  [valid_state(result)]
{
  [state == 1 && event == 1] term 2;
  [state == 2 && event == 1] term 3;
  [state == 3 && event == 1] term 1;
  term state;  # No-op for invalid transitions
};
```

This is what Briev is actually good at. Not string manipulation or math - state and transactions.

## What This Means for Users

- Import native Briev libraries with `import briev.core;` - fully proven
- Use FFI for I/O, math, utilities - same as now
- Clear error messages about what needs what
- Better mental model of Briev's actual capabilities
