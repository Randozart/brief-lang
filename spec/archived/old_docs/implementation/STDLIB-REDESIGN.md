# Brief Standard Library Redesign

## Goal

Separate what Brief can do natively from what requires FFI to Rust. Build a stdlib that leverages Brief's strengths: reactive state management, transactions, contracts.

## What Brief Can Handle Natively

### State & Transactions ✓
- Global state management (via `let`, `const`)
- State transitions with verified contracts
- Reactive transactions that fire automatically
- Atomic rollback on postcondition failure
- Multi-variable state coordination

**Example**: A counter that automatically increments while conditions hold
```brief
let count: Int = 0;
rct txn increment [count < 100] [count == @count + 1] {
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
```brief
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

## What Brief Cannot Handle (Needs FFI)

### I/O Operations ✗
- File reading/writing
- Network operations
- Console output (in browser context)
- Database queries
- Anything that talks to the OS or network

**Why**: Brief doesn't have I/O primitives. These are external capabilities.

### Math Functions (Complex) ✗
- Trigonometry: sin, cos, tan
- Logarithms, exponentials
- Square roots, powers
- Floating point operations beyond basic arithmetic

**Why**: These are CPU operations, not state operations. Brief's arithmetic is integers and comparison.

### String Manipulation (Complex) ✗
- String length, substring, replace
- Case conversion, trimming
- Parsing (string → number)
- Concatenation (can be done but inefficient)

**Why**: Brief has no string operations. These are utility functions.

### Time ✗
- Getting current time
- Measuring elapsed time
- Sleeping

**Why**: External to Brief. Time comes from the runtime/OS.

### Random Numbers ✗
- RNG seeding
- Random integer/float generation

**Why**: Non-deterministic. Can't be proven in Brief.

### Collections (Partially) ✗
- Lists/arrays: Brief has `Data` type but no operations on it
- Maps/dictionaries: Not supported

**Why**: Brief treats collections as opaque `Data`. Operations would need native support.

## Proposed Stdlib Architecture

### Tier 1: Native Brief (No FFI Needed)

**Module: `brief::core`**
- State management patterns
- Transaction templates
- Common guards/contracts

Example:
```brief
# Built-in pattern: Initialize on demand
defn get_or_init(initialized: Bool, init_fn: ... -> ...) -> ... [true][initialized] {
  [initialized] term ...;
  [!initialized] { 
    let result = init_fn();
    term result;
  };
};
```

**Module: `brief::math`** (Integer math only)
- `absolute(x: Int) -> Int`
- `min(a: Int, b: Int) -> Int`
- `max(a: Int, b: Int) -> Int`
- `clamp(x: Int, min: Int, max: Int) -> Int`

All implemented as pure Brief functions with proven contracts.

### Tier 2: FFI to Rust (Current Stdlib)

These genuinely need Rust because Brief can't do I/O or call CPU functions.

**Module: `brief::io`** (FFI)
- `read_file(path: String) -> Result<String, IoError>`
- `write_file(path: String, content: String) -> Result<Void, IoError>`
- Other file operations

**Module: `brief::math`** (FFI)
- `sqrt(x: Float) -> Result<Float, MathError>`
- `sin(x: Float) -> Result<Float, MathError>`
- `pow(base: Float, exp: Float) -> Result<Float, MathError>`
- etc.

**Module: `brief::string`** (FFI)
- `length(s: String) -> Result<Int, StringError>`
- `substring(s: String, start: Int, len: Int) -> Result<String, StringError>`
- `to_upper(s: String) -> Result<String, StringError>`
- etc.

**Module: `brief::time`** (FFI)
- `current_time() -> Result<Int, TimeError>`
- `sleep(ms: Int) -> Result<Void, TimeError>`

### Tier 3: Planned (Future)

What we could add later:

**Module: `brief::collections`** (FFI or native?)
- List operations: append, map, filter, fold
- Dictionary operations: get, set, keys
- Decision: Do we add native collection support to Brief, or FFI them?

**Module: `brief::random`** (FFI)
- `random_int(min: Int, max: Int) -> Result<Int, RandomError>`
- Note: Can't be proven in Brief, but can be called

**Module: `brief::crypto`** (FFI)
- Hash functions
- Encryption (if needed)

**Module: `brief::json`** (FFI)
- Parse JSON
- Stringify values

## Implementation Plan

### Phase 1: Audit Current Stdlib

Review `std/bindings/*.toml`:
1. Identify functions that could be native Brief
2. Identify functions that genuinely need FFI
3. Separate them properly

### Phase 2: Create Native Brief Stdlib

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
- When to use native Brief functions
- When to use FFI functions
- Why the distinction matters

## Benefits of This Approach

1. **Correct by construction**: Native Brief functions have proven contracts
2. **No runtime surprises**: Everything Brief does is verified at compile time
3. **Performance**: Native functions don't cross FFI boundary
4. **Teachable**: Shows what Brief excels at
5. **Maintainable**: Clear separation of concerns

## Example: State Machine Library

We could write a native Brief library for common patterns:

```brief
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

This is what Brief is actually good at. Not string manipulation or math - state and transactions.

## What This Means for Users

- Import native Brief libraries with `import brief.core;` - fully proven
- Use FFI for I/O, math, utilities - same as now
- Clear error messages about what needs what
- Better mental model of Brief's actual capabilities
