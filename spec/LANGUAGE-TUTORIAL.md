# Brief Language Tutorial

**Learn Brief by building real systems, step by step.**

## Part 1: Getting Started

### Installation

```bash
cargo install --path .
```

### Your First Program

Create `hello.bv`:

```brief
let message: String = "hello world";
```

Run it:

```bash
brief check hello.bv
brief build hello.bv
```

That's it. Brief programs don't need a main() — they're just state.

### State is Everything

```brief
let counter: Int = 0;
let name: String = "Alice";
let active: Bool = true;
let balance: Float = 1000.50;
```

State is declared with `let`. You can:
- Give it a type and initial value
- Give it a type with no value (defaults to 0, "", false, 0.0)
- Let Brief infer the type

---

## Part 2: Transactions - Making Changes

### Your First Transaction

```brief
let count: Int = 0;

txn increment [count < 100][count == @count + 1] {
  &count = count + 1;
  term;
};
```

**Breaking this down:**

- `txn` - transaction (a named state change)
- `increment` - the name
- `[count < 100]` - precondition (when can this run?)
- `[count == @count + 1]` - postcondition (what must be true after?)
- `&count = count + 1;` - mutate state (& prefix required)
- `term;` - complete successfully

**Key insight**: You're not just describing code that runs. You're declaring:
- When it's allowed to run (precondition)
- What must be true after it runs (postcondition)

The compiler proves the code actually satisfies the postcondition.

### The Prior-State Operator `@`

```brief
let balance: Int = 100;

txn withdraw(amount: Int) 
  [amount > 0 && amount <= balance]
  [balance == @balance - amount]
{
  &balance = balance - amount;
  term;
};
```

`@balance` means "the value of balance when this transaction started".

The postcondition says: "balance must equal what it was minus the amount".

### Guards: Conditional Execution

```brief
txn process [true][true] {
  let value: Int = compute();
  
  [value > 0] &positive = true;
  [value < 0] &negative = true;
  [value == 0] escape;              # Rollback if zero
  
  term;
};
```

`[condition] statement` only executes if the condition is true.

### Escape: Rollback

```brief
txn validate(x: Int) 
  [x >= 0][state == @state]
{
  [x > 100] {
    escape;                        # Rollback, nothing changes
  };
  &state = x;
  term;
};
```

`escape` rolls back all mutations and terminates the transaction.

---

## Part 3: Reactive Transactions

### Auto-Firing Transactions

```brief
let count: Int = 0;
let done: Bool = false;

rct txn increment [count < 10 && !done]
  [count == @count + 1]
{
  &count = count + 1;
  term;
};

rct txn finish [count >= 10 && !done]
  [done == true]
{
  &done = true;
  term;
};
```

`rct txn` (reactive transaction) automatically runs whenever its precondition becomes true.

**How it works:**
1. You change `count` from 9 to 10
2. Reactor sees `count >= 10 && !done` is now true
3. `finish` fires automatically
4. `done` becomes true
5. `increment` can't fire anymore (precondition now false)
6. Program reaches equilibrium

### Reactive State Machines

This is Brief's superpower - describe state transitions, compiler handles the rest:

```brief
let state: Int = 0;

rct txn step_1 [state == 0][state == 1] {
  &state = 1;
  term;
};

rct txn step_2 [state == 1][state == 2] {
  &state = 2;
  term;
};

rct txn step_3 [state == 2][state == 0] {
  &state = 0;
  term;
};
```

When you set `state = 0`, the machine automatically cycles through all three steps.

---

## Part 4: Functions (Definitions)

### Writing Functions

```brief
defn double(x: Int) 
  -> Int 
  [true]
  [result == x * 2]
{
  term x * 2;
};
```

**Parts:**
- `defn` - define a function
- `double` - function name
- `(x: Int)` - parameter
- `-> Int` - return type
- `[true]` - precondition (always runnable)
- `[result == x * 2]` - postcondition (what must be true about result)
- `term x * 2;` - return the value

### Multiple Return Values

```brief
defn split_name(full: String) 
  -> String, String 
  [true][true]
{
  # In real Brief, you'd parse the string
  # For now, just return placeholders
  term "first", "last";
};
```

**Using it:**
```brief
let first: String, last: String;
first, last = split_name("Alice Smith");
```

### Functions Can Call Other Functions

```brief
defn absolute(x: Int) 
  -> Int 
  [true]
  [result >= 0]
{
  [x < 0] term -x;
  [x >= 0] term x;
};

defn is_positive(x: Int) 
  -> Bool 
  [true][true]
{
  let abs_x: Int = absolute(x);
  [abs_x > 0] term true;
  term false;
};
```

### Using Native Stdlib Functions

```brief
import std.core;

let x: Int = -42;
let abs_x: Int = absolute(x);        # From stdlib
let smaller: Int = min(10, 5);       # From stdlib
let bounded: Int = clamp(x, -50, 50); # From stdlib
```

---

## Part 5: Pattern Matching

### Handling Multiple Outcomes

```brief
# Define what the function returns
sig fetch_user: Int -> User | Error;

txn load_user [true][true] {
  let result = fetch_user(123);
  
  # Handle the success case
  User(name, age) = result;
  &active_user = result;
  term;
  
  # Handle the error case (MUST be handled)
  Error(msg) = result;
  escape;
};
```

**Key**: Compiler forces you to handle every possible outcome.

### Union Types

```brief
let result: Int | String | Error;

[result == Int(x)] {
  # It's an integer
};

[result == String(s)] {
  # It's a string
};

[result == Error(e)] {
  # It's an error
};
```

---

## Part 6: Using Rust (FFI)

### Calling Rust Functions

```brief
# Declare the function
frgn read_file(path: String) -> Result<String, IoError> from "std::io";

# Use it in a transaction
defn load_config(path: String) 
  -> String 
  [true][true]
{
  let content: String = read_file(path);
  content;
};
```

**Result type**: `Result<Success, Error>` means the function can succeed or fail.

### Handling Errors from Rust

```brief
frgn parse_number(text: String) -> Result<Int, ParseError> from "std::string";

defn parse_age(text: String) 
  -> Int 
  [true][true]
{
  let result = parse_number(text);
  
  # Success case
  Int(age) = result;
  [age >= 0 && age < 150] term age;
  escape;                            # Out of range
  
  # Error case
  ParseError(code, msg) = result;
  escape;
};
```

### Available FFI Functions

From `std/core.bv` (native Brief - no FFI):
- `absolute()`, `min()`, `max()`, `clamp()`
- `is_positive()`, `is_negative()`, `is_zero()`, `is_even()`

From `std/bindings/` (FFI to Rust):
- I/O: `read_file()`, `write_file()`
- Math: `sqrt()`, `sin()`, `pow()`, etc.
- String: `string_length()`, `string_to_upper()`, `parse_int()`
- Time: `current_timestamp()`, `sleep_ms()`

See [FFI-STDLIB-REFERENCE.md](FFI-STDLIB-REFERENCE.md) for complete list.

---

## Part 7: Real Example - Bank System

```brief
# State
let alice: Int = 1000;
let bob: Int = 500;
let in_transfer: Bool = false;

# Passive transaction - must be called explicitly
txn transfer_alice_to_bob(amount: Int)
  [!in_transfer && alice >= amount]
  [alice == @alice - amount && bob == @bob + amount && !in_transfer]
{
  &in_transfer = true;
  &alice = alice - amount;
  &bob = bob + amount;
  &in_transfer = false;
  term;
};

# Reactive transactions - fire when conditions are met
rct txn alert_low_balance [alice < 100][alice == @alice]
{
  # In real code, you'd call an FFI function to send an alert
  term;
};

rct txn alert_high_balance [bob > 1000][bob == @bob]
{
  # Send another alert
  term;
};
```

**How it works:**
1. You call `transfer_alice_to_bob(100)`
2. If precondition is true, code executes
3. If postcondition is satisfied, state changes
4. If postcondition fails, entire transaction rolls back
5. Reactive transactions automatically fire based on new state

---

## Part 8: Common Patterns

### Lazy Initialization

```brief
let initialized: Bool = false;
let value: Int = 0;

txn initialize [~initialized][initialized] {
  &initialized = true;
  &value = 100;
  term;
};

rct txn use_value [initialized][initialized] {
  # Use value
  term;
};
```

### State Machine

```brief
let state: Int = 0;  # 0=idle, 1=processing, 2=done

rct txn process [state == 0][state == 1] {
  # Do work
  &state = 1;
  term;
};

rct txn complete [state == 1][state == 2] {
  # Finish up
  &state = 2;
  term;
};

rct txn reset [state == 2][state == 0] {
  &state = 0;
  term;
};
```

### Synchronization with Flags

```brief
let ready: Bool = false;
let busy: Bool = false;

txn start_work [ready && !busy][busy == true] {
  &busy = true;
  term;
};

txn finish_work [busy][busy == false] {
  &busy = false;
  term;
};
```

---

## Part 9: Tips and Gotchas

### Postconditions Must Be Satisfiable

```brief
# ✓ GOOD - postcondition CAN be true
txn increment [count < 100][count == @count + 1] {
  &count = count + 1;
  term;
};

# ✗ BAD - postcondition can NEVER be true
txn bad_increment [count < 100][count == @count + 2] {
  &count = count + 1;    # Only adds 1, not 2
  term;                   # Compiler will reject this
};
```

### All Outcomes Must Be Handled

```brief
sig fetch: Int -> User | Error;

txn load [true][true] {
  let result = fetch(1);
  User(u) = result;
  &user = u;
  term;
  
  # ✗ MISSING ERROR CASE
  # Compiler will reject: "Unhandled outcome: Error"
};
```

### Use Guards Instead of If/Else

```brief
# ✓ BRIEF WAY
[value > 0] &positive = true;
[value < 0] &negative = true;
[value == 0] escape;

# ✗ C-LIKE WAY (doesn't exist in Brief)
if (value > 0) {
  &positive = true;
} else if (value < 0) {
  &negative = true;
}
```

### Mutations Need `&`

```brief
let count: Int = 0;

&count = count + 1;    # ✓ Correct - use &

count = count + 1;     # ✗ Error - & required
```

### Reactive Transactions Don't Return Values

```brief
# ✓ Reactive transaction
rct txn process [ready][done] {
  # Do work
  &done = true;
  term;                # No value, just success
};

# ✓ Passive transaction CAN return values
txn compute() -> Int [true][true] {
  term 42;            # Return 42
};
```

---

## Part 10: Debugging

### Type Checking

```bash
brief check program.bv
```

Shows all type errors before running.

### Proof Verification

The proof engine checks:
1. Can the precondition be false? (If not, always runs)
2. Does code reach `term` or `escape`? (Termination)
3. Is the postcondition satisfiable? (Correctness)

### Clear Error Messages

```
error[B001]: Postcondition not satisfiable
  → transaction 'increment' at line 5
  → postcondition: count == @count + 2
  → code: &count = count + 1;
  = Hint: postcondition requires +2, but code does +1
```

---

## Next Steps

1. **Try the examples**: `brief examples/reactive_counter.bv`
2. **Read the reference**: [LANGUAGE-REFERENCE.md](LANGUAGE-REFERENCE.md)
3. **Learn FFI**: [FFI-USER-GUIDE.md](FFI-USER-GUIDE.md)
4. **Build something**: Create your own reactive system

---

## Key Takeaways

- **State first**: Everything is about state transitions
- **Transactions have contracts**: Preconditions and postconditions
- **Reactive**: Transactions fire automatically when conditions are met
- **Proven**: Compiler verifies your state machine is correct
- **Atomic**: Transactions either complete or rollback completely
- **No recursion**: Compiler must be able to prove termination
- **FFI for external stuff**: Use Rust for I/O and complex operations

**The goal**: Build systems you can prove are correct.
