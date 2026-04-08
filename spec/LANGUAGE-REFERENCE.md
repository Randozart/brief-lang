# Brief Language Reference Manual

**Version:** 7.0  
**Status:** Authoritative Reference  

---

## Table of Contents

1. [Overview](#overview)
2. [Lexical Structure](#lexical-structure)
3. [Types](#types)
4. [State and Variables](#state-and-variables)
5. [Transactions](#transactions)
6. [Definitions](#definitions)
7. [Structs](#structs)
8. [Pattern Matching](#pattern-matching)
9. [Imports](#imports)
10. [FFI](#ffi)
11. [Rendered Brief](#rendered-brief)
12. [Standard Library](#standard-library)
13. [Error Messages](#error-messages)

---

## Overview

Brief is a declarative language for building verifiable state machines. Key concepts:

- **Contracts**: Preconditions and postconditions on all state transitions
- **Atomic**: Transactions either complete or roll back completely
- **Reactive**: `rct` blocks fire automatically when conditions are met
- **FFI**: Foreign functions for system access

---

## Lexical Structure

### Comments

```brief
// Single-line comment
let x: Int = 1;  // Inline comment
```

### Identifiers

```brief
// Valid identifiers
counter
my_function
isValid
_value42

// Invalid
// 2fast     // Cannot start with number
// my-var     // Cannot contain hyphen
```

### Keywords

```
defn    txn     rct     async
let     const   term    escape
from    import  struct  rstruct
view    frgn    sig     as
true    false
```

### Literals

```brief
42          // Int
-5          // Int (negative)
3.14        // Float
"hello"     // String
true        // Bool
false       // Bool
```

---

## Types

### Built-in Types

| Type | Description | Example |
|------|-------------|---------|
| `Int` | 64-bit signed integer | `42`, `-5` |
| `Float` | 64-bit floating point | `3.14` |
| `String` | Text | `"hello"` |
| `Bool` | Boolean | `true`, `false` |
| `Void` | Empty value | (no literal) |
| `Data` | Opaque data | (FFI) |

### Custom Types

```brief
struct Point {
    x: Int;
    y: Int;
};

struct User {
    name: String;
    age: Int;
    active: Bool;
};
```

### Type Parameters

```brief
defn identity<T>(value: T) -> T [true][result == value] {
    term value;
};
```

---

## State and Variables

### State Variables

```brief
let counter: Int = 0;
let name: String = "Alice";
let active: Bool = true;
let data: Data;
```

### Constants

```brief
const MAX_SIZE: Int = 1000;
const PI: Float = 3.14159;
```

### Write Access

State variables require `&` for mutation:

```brief
let count: Int = 0;

&count = count + 1;  // Mutate
let x = count;       // Read (no &)
```

---

## Transactions

### Passive Transactions

```brief
txn withdraw(amount: Int)
    [amount > 0 && amount <= balance]
    [balance == @balance - amount]
{
    &balance = balance - amount;
    term;
};
```

### Reactive Transactions

```brief
rct txn increment [count < 100][count == @count + 1] {
    &count = count + 1;
    term;
};
```

### Async Transactions

```brief
rct async txn write_data [ready][data == @data] {
    &data = new_value;
    term;
};
```

### Contracts

Every transaction has:

```brief
txn name [precondition][postcondition] {
    // body
};
```

- **Precondition**: When the transaction can fire
- **Postcondition**: What must be true after `term`

#### Implicit `term true;`

When a definition or transaction has a Bool postcondition (`true`), `term;` is implicitly treated as `term true;`:

```brief
// Postcondition is literal true - term; becomes term true;
txn activate [ready][true] {
    term;  // implicitly: term true;
};

// Postcondition is a Bool expression - term; checks if postcondition is met
txn set_flag [true][flag == true] {
    &flag = true;
    term;  // checks: is flag == true satisfied? Yes, so terminates
};
```

#### `term functionCall();`

When `term` contains a function call, the compiler verifies the call's output satisfies the postcondition:

```brief
defn addOne(x: Int) -> Int [true][result == x + 1] {
    term x + 1;
};

txn increment [count < 100][count == @count + 1] {
    term addOne(@count);  // Verifies: addOne(@count) == @count + 1
};
```

### Prior State

```brief
txn increment [count < 100][count == @count + 1] {
    &count = count + 1;
    term;
};
```

`@count` is the value when the transaction started.

### Syntactic Sugar

```brief
// [~/ready] means [~ready][ready]
txn initialize [~/ready] {
    &ready = true;
    term;
};

// Implicit state declaration - let ready: Bool = false is automatic
rct txn start [~/ready] {
    &ready = true;
    term;
};

// Lambda-style - body is trivial, implicit term
txn inc [count < 100][count == @count + 1];

// Lambda-style for defn
defn double(x: Int) -> Int [true][result == x * 2];
```

### Transaction Loop Behavior

Transactions loop until the postcondition is satisfied. They continue mutating until the postcondition holds.

```brief
// Loops until postcondition is met
txn increment_by_2 [count < 100][count == @count + 2] {
    &count = count + 1;
    term;
};
// Starting at count=99, @count=99: 99->100->101->102 (stops at 102)
```

### Guards

```brief
txn process [true][true] {
    let value = get_value();
    [value > 0] &positive = true;
    [value <= 0] escape;
    term;
};
```

### Escape

```brief
txn validate [x > 0][x == @x] {
    [x > 1000] escape;  // Rollback
    &state = x;
    term;
};
```

### Calling Transactions

```brief
txn do_work [true][true] {
    withdraw(50);  // Call passive transaction
    term;
};
```

---

## Definitions

### Function Definition

```brief
defn add(a: Int, b: Int) -> Int [true][result == a + b] {
    term a + b;
};
```

### Multiple Outputs

```brief
defn divmod(a: Int, b: Int) -> Int, Int [b != 0][true] {
    term a / b, a % b;
};
```

### Calling Functions

```brief
let sum: Int = add(10, 20);
let quotient, remainder = divmod(17, 5);
```

---

## Structs

### Plain Struct

```brief
struct BankAccount {
    balance: Int;
    overdraft: Int;
    
    txn withdraw(amount: Int)
        [amount > 0 && amount <= balance + overdraft]
        [balance == @balance - amount]
    {
        &balance = balance - amount;
        term;
    };
};
```

### Render Struct

```brief
rstruct Counter {
    count: Int;
    
    rct txn increment [count < 100][count == @count + 1] {
        &count = count + 1;
        term;
    };
} -> "
<div>
    <span>{count}</span>
    <button onclick='increment()'>+</button>
</div>
";
```

### Using Structs

```brief
// Create instances
let counter1 = Counter {};           // Default values
let counter2 = Counter { count: 5 }; // Partial init

// Access fields
let n = counter1.count;

// Clone instance
let counter3 = clone(counter2);

// List of instances
let counters = [Counter {}, Counter {}];
let first = counters[0];
let first_count = first.count;
```

### Instance Method Resolution

When you call `counter.increment()`:

1. If `counter` is an instance of type `Counter`, the compiler resolves it to `Counter.increment(counter)`
2. The method operates on the instance's fields, not global state
3. Method calls are verified against the instance's contract

### clone() Function

The built-in `clone()` function duplicates any value:

```brief
let original = Counter { count: 42 };
let copy = clone(original);  // copy.count == 42
```

---

## Pattern Matching

### Unification

```brief
let result: Int | String;

[Int(n) = result] &int_val = n;
[String(s) = result] &str_val = s;
```

### Guards

```brief
let value: Int;

[value > 0] &positive = true;
[value == 0] &zero = true;
[value < 0] &negative = true;
```

---

## Imports

### Namespace Import

```brief
import std.io;
import std.math;
```

### Selective Import

```brief
import { print, println } from std.io;
```

### Aliased Import

```brief
import { println as log } from std.io;
```

---

## FFI

### TOML Binding

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

### Brief Declaration

```brief
frgn read_file(path: String) -> Result<String, IoError> from "lib/std/io.toml";
```

### Multi-Field Success Outputs

FFI functions can return multiple fields on success:

```toml
[functions.output.success]
quotient = "Int"
remainder = "Int"
```

```brief
frgn divide(a: Int, b: Int) -> Result<(quotient: Int, remainder: Int), MathError> from "lib/std/math.toml";
```

### Generic FFI

```brief
frgn<T> identity(value: T) -> Result<T, Error> from "lib/std/util.toml";
```

### Using FFI

```brief
frgn read_file(path: String) -> Result<String, IoError> from "lib/std/io.toml";

defn load_config() -> String [true][result.len() >= 0] {
    let result = read_file("config.txt");
    if result.is_ok() {
        term result.value;
    } else {
        term "default";
    }
};
```

### Error Handling Requirements

The compiler enforces that FFI errors must be handled:

| Method | Returns | Description |
|--------|---------|-------------|
| `.is_ok()` | `Bool` | True if success |
| `.is_err()` | `Bool` | True if error |
| `.value` | `T` | Success value |
| `.error.code` | `E.code` | Error code |
| `.error.message` | `E.message` | Error message |

---

## Rendered Brief

### rstruct (Render Struct)

Combines state with HTML view:

```brief
import "./styles.css";

rstruct Counter {
    count: Int;
    
    rct txn increment [count < 100][count == @count + 1] @30Hz {
        &count = count + 1;
        term;
    };

    <button>{count}</button>
}
```

HTML is embedded inline using `<` at the start of a tag.

### Multi-Element Output

Render structs can include multiple HTML elements:

```brief
rstruct Form {
    name: String;
    email: String;

    <div class="name-field">{name}</div>
    <div class="email-field">{email}</div>
}
```

### Standalone Render

A `render` block provides HTML without state:

```brief
render Button {
    <button class="primary">Click me</button>
}
```

### CSS Import

CSS files are imported at the top:

```brief
import "./styles/main.css";
import "./styles/theme.css";
```

---

## Standard Library

### Native Functions

Functions implemented in Brief:

```brief
defn absolute(x: Int) -> Int [true][result >= 0] {
    [x < 0] term -x;
    [x >= 0] term x;
};

defn min(a: Int, b: Int) -> Int [true][result == a || result == b] {
    [a <= b] term a;
    [a > b] term b;
};

defn max(a: Int, b: Int) -> Int [true][result == a || result == b] {
    [a >= b] term a;
    [a < b] term b;
};

defn clamp(value: Int, min_val: Int, max_val: Int) -> Int [min_val <= max_val][result >= min_val && result <= max_val] {
    [value < min_val] term min_val;
    [value > max_val] term max_val;
    [value >= min_val && value <= max_val] term value;
};
```

### FFI Functions

Functions requiring system access:

```brief
frgn print(msg: String) -> Result<Bool, IoError> from "lib/std/io.toml";
frgn println(msg: String) -> Result<Bool, IoError> from "lib/std/io.toml";
frgn input() -> Result<String, IoError> from "lib/std/io.toml";

frgn sqrt(x: Float) -> Result<Float, MathError> from "lib/std/math.toml";
frgn sin(x: Float) -> Result<Float, MathError> from "lib/std/math.toml";
frgn cos(x: Float) -> Result<Float, MathError> from "lib/std/math.toml";
frgn pow(base: Float, exp: Float) -> Result<Float, MathError> from "lib/std/math.toml";

frgn now() -> Result<Int, TimeError> from "lib/std/time.toml";
frgn sleep_ms(ms: Int) -> Result<Void, TimeError> from "lib/std/time.toml";
```

---

## Error Messages

### Precondition Not Satisfiable

```
[E001] Precondition not satisfiable

Transaction: increment
Precondition: count < 0 && count > 100

Hint: Precondition is contradictory (cannot be true)
```

### Infinite Loop

```
[E002] Infinite loop detected

Transaction: impossible
Issue: Postcondition can never be satisfied

The transaction:
  txn impossible [count >= 0][count < 0] {
      &count = count + 1;
      term;
  };

can never satisfy postcondition [count < 0]:
  - @count is captured at start
  - count only increases
  - count can never be less than 0

Hint: If postcondition can never be satisfied, compiler throws error
```

### Termination Unreachable

```
[E003] Termination unreachable

Transaction: loop
No path from precondition to term

Hint: Add a path that can reach term
```

### Type Mismatch

```
[E004] Type mismatch

Expected: Int
Got: String

Hint: Check the types of assigned values
```

### FFI Binding Error

```
[E005] FFI binding validation failed

Function: read_file
Issue: Parameter type mismatch

Hint: Check frgn declaration matches TOML binding
```

---

## Quick Reference

### Syntax Summary

| Construct | Syntax |
|-----------|--------|
| State | `let x: Int = 0;` |
| Constant | `const MAX: Int = 100;` |
| Transaction | `txn name [pre][post] { }` |
| Reactive | `rct txn name [pre][post] { }` |
| Async | `rct async txn name [pre][post] { }` |
| Definition | `defn f(x: T) -> R [pre][post] { }` |
| FFI | `frgn f(x: T) -> R from "path";` |
| Struct | `struct Name { field: T; }` |
| Render | `rstruct Name { } -> "html";` |

### Operators

| Operator | Meaning |
|----------|---------|
| `==` | Equal |
| `!=` | Not equal |
| `<` | Less than |
| `<=` | Less or equal |
| `>` | Greater than |
| `>=` | Greater or equal |
| `&&` | Logical AND |
| `\|\|` | Logical OR |
| `!` | Logical NOT |
| `-` | Negation |
| `+` | Add |
| `-` | Subtract |
| `*` | Multiply |
| `/` | Divide |
| `%` | Modulo |

### Special Symbols

| Symbol | Meaning |
|--------|---------|
| `&x` | Write access to x |
| `@x` | Value of x at transaction start |
| `~/x` | Shorthand for [~x][x] |
| `term` | Successful completion |
| `escape` | Rollback and exit |
