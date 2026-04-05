# Brief Language Reference Manual v6.2

**A reactive language for verified state machines.**

## Table of Contents

1. [Overview](#overview)
2. [Core Concepts](#core-concepts)
3. [Syntax](#syntax)
4. [Types](#types)
5. [State and Variables](#state-and-variables)
6. [Transactions](#transactions)
7. [Definitions and Functions](#definitions-and-functions)
8. [Contracts and Verification](#contracts-and-verification)
9. [Pattern Matching](#pattern-matching)
10. [Standard Library](#standard-library)
11. [Foreign Functions (FFI)](#foreign-functions-ffi)
12. [Examples](#examples)

---

## Overview

Brief is a language for building systems where you want to prove the state machine is correct. Instead of testing whether your code works, the compiler verifies it mathematically.

**Key idea**: State transitions are transactions with pre- and post-conditions. The compiler proves: if the precondition is true, the code executes and makes the postcondition true.

```brief
let balance: Int = 100;

txn withdraw(amount: Int) 
  [amount > 0 && amount <= balance]      # Pre: valid withdrawal
  [balance == @balance - amount]         # Post: balance decreases
{
  &balance = balance - amount;
  term;
};
```

---

## Core Concepts

### The Blackboard Model

Brief programs don't have a `main()` function. Instead, they use a **blackboard architecture**:

1. **Global state** (`let`, `const` variables) is the single source of truth
2. **Reactive transactions** (`rct txn`) automatically run when their preconditions become true
3. **Passive transactions** (`txn`) run only when explicitly called
4. **The reactor loop** continuously evaluates preconditions and fires reactive transactions
5. **Equilibrium** is reached when no precondition is true

### Atomic Execution

Every transaction is atomic. If it reaches an `escape` statement or its postcondition fails, the entire transaction rolls back via **Software Transactional Memory (STM)**. The state is untouched.

### Verification

The compiler proves:
1. **Termination**: Every transaction will eventually reach `term` or `escape`
2. **Postconditions**: When a transaction completes with `term`, the postcondition is true
3. **Reachability**: All execution paths lead to `term` or can be proven unreachable

---

## Syntax

### File Structure

A Brief program is a collection of **top-level declarations**:

```brief
# State declarations
let counter: Int = 0;
const MAX: Int = 100;

# Transactions
txn increment [counter < MAX][counter == @counter + 1] {
  &counter = counter + 1;
  term;
};

rct txn auto_reset [counter >= MAX][counter == 0] {
  &counter = 0;
  term;
};

# Definitions (functions)
defn double(x: Int) -> Int [true][result == x * 2] {
  term x * 2;
};

# Structs
struct User {
  name: String;
  age: Int;
  
  txn have_birthday [true][age == @age + 1] {
    &age = age + 1;
    term;
  };
};

# Foreign functions
frgn read_file(path: String) -> Result<String, IoError> from "std::io";

# Imports
import std.core;
```

### Statements

Statements execute sequentially within transactions and definitions:

```brief
# Assignment
&count = count + 1;              # Mutate state variable (must use &)

# Let binding
let x: Int = 42;               # Local variable (immutable)

# Guarded statement
[x > 0] &positive = true;      # Only execute if guard is true

# Unification (pattern matching)
User(name, age) = get_user();  # Destructure and handle

# Term (success)
term;                           # Complete transaction successfully
term value;                     # Return a value

# Escape (failure with rollback)
escape;                         # Rollback transaction atomically
escape error;                   # Rollback with error info

# Expression statement
function_call();               # Execute but discard result
x + y;                        # Evaluate but discard
```

---

## Types

Brief has these built-in types:

| Type | Description | Example |
|------|-------------|---------|
| `Int` | 64-bit signed integer | `42`, `-5` |
| `Float` | 64-bit floating point | `3.14`, `-2.5` |
| `String` | Text | `"hello"` |
| `Bool` | Boolean | `true`, `false` |
| `Void` | Empty value | (no value) |
| `Data` | Opaque data | (for FFI) |
| Custom structs | User-defined types | `User`, `Account` |

### Union Types

A value can be one of several types:

```brief
let result: Int | String | Error;
```

### Type Inference

Brief infers types from context:

```brief
let x = 42;           # Inferred: Int
let y = x + 1;        # Inferred: Int
let z = "hello";      # Inferred: String
```

---

## State and Variables

### State Variables (`let`)

Global state that can change:

```brief
let balance: Int = 100;
let ready: Bool = false;
let name: String;              # No initial value (default to 0/""/false)
```

**Mutation** requires the `&` prefix:

```brief
&balance = balance - 10;      # Correct: mutate state
balance = balance - 10;       # Error: can't assign without &
```

### Constants (`const`)

Immutable values:

```brief
const MAX_BALANCE: Int = 1000000;
const PI: Float = 3.14159;
```

### Local Variables (`let` inside transactions)

Immutable within a transaction:

```brief
txn transfer [true][true] {
  let amount: Int = 50;           # Local, immutable
  &balance = balance - amount;    # Use it
  term;
};
```

---

## Transactions

### Passive Transactions (`txn`)

Run only when explicitly called. Have preconditions and postconditions.

```brief
txn withdraw(amount: Int) 
  [amount > 0 && amount <= balance]    # Pre: must be valid
  [balance == @balance - amount]       # Post: must decrease by amount
{
  &balance = balance - amount;
  term;
};
```

**Calling a transaction:**
```brief
rct txn initiate [ready][ready] {
  withdraw(50);      # Call passive transaction
  term;
};
```

### Reactive Transactions (`rct txn`)

Automatically fire when precondition becomes true.

```brief
rct txn process [data_available && !processing]
  [processing == false]
{
  let data: String = read_data();
  process_data(data);
  &processing = false;
  term;
};
```

**Reactor behavior:**
1. Tracks variables referenced in preconditions
2. When a variable changes, marks affected rct's as dirty
3. Re-evaluates dirty preconditions
4. Fires any that became true
5. At equilibrium (nothing can fire), reactor sleeps

### Contracts

Every transaction has pre- and postconditions:

```brief
txn example [precondition][postcondition] {
  # body
};
```

**Precondition** `[pre]`:
- Must evaluate to true before transaction can run
- Cannot contain mutations
- Determines *when* transaction fires

**Postcondition** `[post]`:
- Must evaluate to true after `term` for transaction to complete
- Can reference prior state via `@`
- Determines *if* transaction succeeded
- If false, transaction rolls back

### Prior State `@variable`

Reference the value of a variable at transaction start:

```brief
txn increment [count < 100][count == @count + 1] {
  &count = count + 1;
  term;
};
```

`@count` is the value when the transaction began.

### Syntactic Sugar `~condition`

`[~ready]` is shorthand for `[~ready][ready]`:

```brief
txn initialize [~ready][ready] {    # Fire when ready is false
  &ready = true;                     # Must make ready true
  term;
};
```

### Transaction Loop

If postcondition fails, transaction rolls back and loops:

```brief
txn example [pre][post] {
  # If execution reaches term but post is false:
  # 1. All & mutations roll back
  # 2. Loop back to start
  # 3. Try again
};
```

The compiler must prove the transaction will eventually reach `term` with a true postcondition.

---

## Definitions and Functions

### Defining Functions

```brief
defn add(a: Int, b: Int) 
  -> Int 
  [true]
  [result == a + b]
{
  term a + b;
};
```

**Syntax:**
```
defn <name>(<params>) -> <return_type> [<pre>][<post>] { <body> }
```

### Multiple Return Values

```brief
defn split(text: String) 
  -> String, String, Int 
  [true][true]
{
  let len: Int = length(text);
  term first, second, len;
};
```

Call with unification:

```brief
string, string2, len = split("hello world");
```

### Calling Functions

```brief
let result: Int = add(10, 20);
let x, y, z = multi_return();
```

### No Recursion

Functions cannot be recursive (compiler can't prove termination).

---

## Contracts and Verification

### What the Compiler Proves

For every transaction/definition:

1. **Pre-condition → Post-condition**: If precondition is true, execution satisfies postcondition
2. **Termination**: Code reaches either `term` or `escape`
3. **No infinite loops**: All paths are finite

### Contract Failure

If postcondition fails:

```brief
txn example [true][count == 100] {
  &count = count + 1;     # If count wasn't 100, post fails
  term;                   # Rolls back, loops
};
```

### Guard Failures

If a guard is false, rest of statement is skipped:

```brief
txn example [true][true] {
  [count < 50] &count = count + 1;   # Only if count < 50
  [count >= 50] &count = 0;          # Otherwise, reset
  term;
};
```

---

## Pattern Matching

### Unification

Destructure union types and handle all cases:

```brief
sig fetch_user: Int -> User | Error;

txn load_user [true][true] {
  let result = fetch_user(1);
  
  # Success case
  User(name, age) = result;
  &current_user = result;
  term;
  
  # Error case
  Error(msg) = result;
  escape;
};
```

**Important**: Compiler forces handling all possible outcomes.

### Guards with Patterns

```brief
txn process [true][true] {
  let value = compute();
  [value == 0] escape;           # Handle zero
  [value > 0] &positive = true;  # Handle positive
  [value < 0] &negative = true;  # Handle negative
  term;
};
```

---

## Standard Library

### Native Brief (`std/core.bv`)

Proven at compile time, no FFI:

- **Math**: `absolute(x)`, `min(a, b)`, `max(a, b)`, `clamp(x, min, max)`
- **Predicates**: `is_positive(x)`, `is_negative(x)`, `is_zero(x)`, `is_even(x)`
- **Conditionals**: `choose_if(cond, true_val, false_val)`
- **Helpers**: `always_true()`, `always_false()`, `not_equal(a, b)`

```brief
import std.core;

let x: Int = -42;
let abs_x: Int = absolute(x);      # 42
let min_val: Int = min(10, 5);     # 5
```

### FFI Bindings (`std/bindings/`)

Require Rust, for I/O and operations Brief can't do:

- **I/O**: `read_file()`, `write_file()`, directory operations
- **Math**: `sqrt()`, `sin()`, `pow()`, etc.
- **String**: `string_length()`, `string_to_upper()`, `parse_int()`
- **Time**: `current_timestamp()`, `sleep_ms()`

```brief
frgn read_file(path: String) -> Result<String, IoError> from "std::io";
frgn sqrt(x: Float) -> Result<Float, MathError> from "std::math";
```

---

## Foreign Functions (FFI)

### Declaring FFI Functions

```brief
frgn <name>(<params>) -> Result<SuccessType, ErrorType> from "<binding_file>";
```

### TOML Binding Files

FFI functions are declared in TOML:

```toml
[[functions]]
name = "read_file"
location = "std::fs::read_to_string"
target = "native"

[functions.input]
path = "String"

[functions.output.success]
content = "String"

[functions.output.error]
type = "IoError"
code = "Int"
message = "String"
```

### Handling FFI Results

```brief
frgn read_file(path: String) -> Result<String, IoError> from "std::io";

defn load_config(path: String) 
  -> String 
  [true][true]
{
  let result = read_file(path);
  
  # Handle success
  String(content) = result;
  term content;
  
  # Handle error (compiler forces this)
  IoError(code, msg) = result;
  escape;
};
```

**Key**: Compiler forces you to handle both success and error cases.

### Available FFI Modules

See [FFI-STDLIB-REFERENCE.md](FFI-STDLIB-REFERENCE.md) for complete list.

---

## Examples

### Example 1: Counter

```brief
let count: Int = 0;
let max_count: Int = 10;
let done: Bool = false;

rct txn increment [count < max_count && !done]
  [count == @count + 1]
{
  &count = count + 1;
  term;
};

rct txn finish [count >= max_count && !done]
  [done == true]
{
  &done = true;
  term;
};
```

### Example 2: Bank Transfer

```brief
let alice: Int = 1000;
let bob: Int = 500;

txn transfer_to_bob [alice >= 100]
  [alice == @alice - 100 && bob == @bob + 100]
{
  &alice = alice - 100;
  &bob = bob + 100;
  term;
};
```

### Example 3: Using FFI

```brief
frgn read_file(path: String) -> Result<String, IoError> from "std::io";
frgn string_length(s: String) -> Result<Int, StringError> from "std::string";

defn count_lines(path: String) 
  -> Int 
  [true][true]
{
  let content: String = read_file(path);
  let lines: Int = string_length(content);
  term lines;
};
```

### Example 4: Reactive State Machine

```brief
let state: Int = 0;
let ready: Bool = false;

txn initialize [~ready][ready] {
  &ready = true;
  &state = 1;
  term;
};

rct txn advance [ready && state == 1]
  [state == 2]
{
  &state = 2;
  term;
};

rct txn complete [ready && state == 2]
  [ready == false]
{
  &ready = false;
  term;
};
```

---

## Summary

**Brief is for:**
- Systems where you need to prove correctness
- Reactive state machines
- Lock-free concurrency
- State-heavy applications

**Brief is not for:**
- General-purpose computation (use Rust)
- String/data manipulation (use FFI)
- High-performance math (use FFI)

**The core promise**: If your Brief program compiles, the state machine is correct.

---

**See also:**
- [LANGUAGE-TUTORIAL.md](LANGUAGE-TUTORIAL.md) - Step-by-step guide
- [FFI-USER-GUIDE.md](FFI-USER-GUIDE.md) - Using Rust functions
- [FFI-STDLIB-REFERENCE.md](FFI-STDLIB-REFERENCE.md) - Available functions
- [examples/](../examples/) - Working programs
