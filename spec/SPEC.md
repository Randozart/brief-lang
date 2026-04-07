# Brief Language Specification

**Version:** 7.0  
**Date:** 2026-04-07  
**Status:** Authoritative Reference  

---

## 1. Introduction and Philosophy

Brief is a declarative, contract-enforced logic language designed for building verifiable state machines. It treats program execution as a series of verified state transitions rather than sequential instructions.

Brief is designed for **Formal Verification without the Boilerplate**. It eliminates imperative control flow (`if`, `else`, `while`) in favor of contracts, guards, and atomic transactions.

### 1.1 Core Design Principles

1. **Contracts First**: Every transaction declares what must be true before and after it runs. The compiler verifies these contracts.
2. **Atomic State Transitions**: Transactions are atomic - they either complete fully or roll back completely.
3. **Reactive Execution**: Brief programs use a reactor model where transactions fire automatically when their preconditions are met.
4. **Zero-Nesting Logic**: Branching is handled via guards, not nested blocks. This improves clarity and LLM comprehension.
5. **FFI for External Capabilities**: Brief cannot do everything (file I/O, networking, hardware math). Foreign Function Interface handles these cases with explicit contracts.

### 1.2 File Extension

Brief source files use the `.bv` extension.

---

## 2. Grammar Specification

### 2.1 Program Structure

```bnf
program ::= (definition | transaction | state_decl | constant | import | struct_def | rstruct_def | render_block)*

definition ::= "defn" identifier type_params? parameters? contract "->" output_types "{" body "}" ";"
transaction ::= ("async")? "txn" identifier contract "{" body "}" ";"
rct_transaction ::= "rct" ("async")? "txn" identifier contract "{" body "}" ";"
foreign_binding ::= "frgn" identifier type_params? parameters "->" "Result<" (success_type | "(" success_fields ")") "," error_type ">" "from" string ";"
success_fields ::= identifier ":" type ("," identifier ":" type)*
foreign_sig ::= "frgn" "sig" identifier "(" parameters? ")" "->" output_types ";"

state_decl ::= "let" identifier ":" type ("=" expression)? ";"
constant ::= "const" identifier ":" type "=" expression ";"

struct_def ::= "struct" identifier "{" struct_member* "}"
struct_member ::= field_decl | transaction
field_decl ::= identifier ":" type ";"

rstruct_def ::= "rstruct" identifier "{" struct_member* view_body "}"

import_stmt ::= "import" ("{" import_item ("," import_item)* "}")? (("from" namespace_path) | namespace_path)? ";"
import_item ::= identifier ("as" identifier)?

render_block ::= "view" identifier "->" view_body
```

### 2.2 Parameters and Types

```bnf
parameters ::= "(" (param ("," param)*)? ")"
param ::= identifier ":" type
type_params ::= "<" identifier ("," identifier)* ">"

type ::= "Int" | "Float" | "String" | "Bool" | "Void" | "Data" | identifier
output_types ::= type ("," type)*

contract ::= "[" expression "]" "[" expression "]"
```

### 2.3 Statements

```bnf
statement ::=
    | assignment ";"
    | unification ";"
    | guarded_stmt
    | term_stmt ";"
    | escape_stmt ";"
    | expression ";"

assignment ::= ("&")? identifier "=" expression
unification ::= identifier "(" pattern ")" "=" expression
guarded_stmt ::= "[" expression "]" (statement | "{" statement* "}")
term_stmt ::= "term" expression? ("," expression?)*
escape_stmt ::= "escape" expression?
```

### 2.4 Expressions

```bnf
expression ::= or_expr
or_expr ::= and_expr ("||" and_expr)*
and_expr ::= equality (("&&") equality)*
equality ::= comparison (("==" | "!=") comparison)*
comparison ::= term (("<" | "<=" | ">" | ">=") term)*
term ::= factor (("+" | "-") factor)*
factor ::= unary (("*" | "/" | "%") unary)*
unary ::= ("!" | "-") unary | primary
primary ::=
    | literal
    | identifier
    | "&" identifier
    | "@" identifier
    | call
    | "(" expression ")"

call ::= identifier "(" arguments? ")"
arguments ::= expression ("," expression)*
literal ::= integer | float | string | "true" | "false"
```

### 2.5 Imports

```bnf
import_stmt ::= "import" items? namespace_path ";"
items ::= "{" import_item ("," import_item)* "}"
import_item ::= identifier ("as" identifier)?
namespace_path ::= identifier ("." identifier)*
```

---

## 3. Types

### 3.1 Built-in Types

| Type | Description | Literals |
|------|-------------|----------|
| `Int` | 64-bit signed integer | `42`, `-5`, `0` |
| `Float` | 64-bit floating point | `3.14`, `-2.5`, `1.0` |
| `String` | Text | `"hello"`, `""` |
| `Bool` | Boolean | `true`, `false` |
| `Void` | Empty value | (no literal) |
| `Data` | Opaque data (FFI) | (opaque) |

### 3.2 Custom Types

```brief
struct Point {
    x: Int;
    y: Int;
};

rstruct Counter {
    count: Int;
    
    txn increment [count < 100][count == @count + 1] {
        &count = count + 1;
        term;
    };
} -> "<div>{count}</div>";
```

### 3.3 Type Parameters (Generics)

```brief
defn identity<T>(value: T) -> T [true][result == value] {
    term value;
};
```

---

## 4. State and Variables

### 4.1 State Variables

```brief
let counter: Int = 0;
let name: String = "Alice";
let active: Bool = true;
let data: Data;
```

### 4.2 Constants

```brief
const MAX_SIZE: Int = 1000;
const PI: Float = 3.14159;
```

### 4.3 Write Access

State variables require explicit write access using `&`:

```brief
&counter = counter + 1;  // Mutate state
let local = counter;     // Read state
```

---

## 5. Transactions

### 5.1 Passive Transactions

Passive transactions run only when explicitly called:

```brief
txn withdraw(amount: Int)
    [amount > 0 && amount <= balance]
    [balance == @balance - amount]
{
    &balance = balance - amount;
    term;
};
```

### 5.2 Reactive Transactions

Reactive transactions (`rct`) fire automatically when preconditions are met:

```brief
rct txn increment [count < max][count == @count + 1] {
    &count = count + 1;
    term;
};
```

### 5.3 Contracts

Every transaction has:
- **Precondition**: When the transaction can fire
- **Postcondition**: What must be true after `term`

```brief
txn example [pre_condition][post_condition] {
    // body
};
```

### 5.3.1 Implicit `term true;`

When a definition or transaction has a literal Bool `true` postcondition, `term;` is implicitly treated as `term true;`:

```brief
// Postcondition is literal true - term; becomes term true;
txn activate [ready][true] {
    term;  // implicitly: term true;
};

// Postcondition is a Bool expression - term; checks if postcondition is met
txn set_flag [true][flag == true] {
    &flag = true;
    term;  // checks: is flag == true satisfied?
};
```

### 5.3.2 `term functionCall();`

When `term` contains a function call, the compiler verifies that the function's output satisfies the postcondition:

```brief
defn addOne(x: Int) -> Int [true][result == x + 1] {
    term x + 1;
};

txn increment [count < 100][count == @count + 1] {
    term addOne(@count);  // Compiler verifies: addOne(@count) == @count + 1
};
```

If the function does not satisfy the postcondition, the compiler reports an error.

### 5.4 Prior State

The `@` operator references the value at transaction start:

```brief
txn increment [count < 100][count == @count + 1] {
    &count = count + 1;
    term;
};
```

### 5.5 Syntactic Sugar

`[~/x]` is shorthand for `[~x][x]`:

```brief
txn initialize [~/ready][ready] {
    &ready = true;
    term;
};
```

### 5.6 Guards

Guards are inline conditions that skip execution when false:

```brief
txn process [true][true] {
    let value = compute();
    [value > 0] &positive = true;
    [value <= 0] escape;
    term;
};
```

### 5.7 Escape

`escape` rolls back all mutations and terminates the transaction:

```brief
txn validate [x > 0][state == @state] {
    [x > 1000] escape;
    &state = x;
    term;
};
```

---

## 6. Definitions

### 6.1 Function Definitions

```brief
defn add(a: Int, b: Int) -> Int [true][result == a + b] {
    term a + b;
};
```

### 6.2 Multiple Outputs

```brief
defn divide(a: Int, b: Int) -> Int, Int, Bool [b != 0][true] {
    term a / b, a % b, true;
};
```

---

## 7. Foreign Function Interface

### 7.1 Overview

FFI allows Brief to call external functions (typically Rust) through explicit contracts. Foreign functions are declared in TOML binding files and referenced in Brief code.

### 7.2 TOML Binding Format

```toml
[[functions]]
name = "read_file"
description = "Read entire file contents"
location = "std::fs::read_to_string"
target = "native"
mapper = "rust"

[functions.input]
path = "String"

[functions.output.success]
content = "String"

[functions.output.error]
type = "IoError"
code = "Int"
message = "String"
```

### 7.3 Binding Fields

| Field | Required | Description |
|-------|----------|-------------|
| `name` | Yes | Brief function name |
| `location` | Yes | Rust module path |
| `target` | Yes | Target platform (`native`) |
| `mapper` | No | Mapper name (default: `rust`) |
| `description` | No | Human-readable description |
| `input` | Yes | Parameter name-type pairs |
| `output.success` | Yes | Success output fields |
| `output.error` | Yes | Error type and fields |

### 7.4 Brief Declaration

```brief
frgn read_file(path: String) -> Result<String, IoError> from "lib/std/io.toml";
```

### 7.4.1 Multi-Field Success Outputs

FFI functions can return multiple fields on success using tuple syntax:

```toml
[functions.output.success]
x = "Int"
y = "Int"
```

```brief
frgn divide(a: Int, b: Int) -> Result<(quotient: Int, remainder: Int), MathError> from "lib/std/math.toml";

txn safe_divide [b != 0][result.quotient >= 0] {
    let (q, r) = divide(10, 3);
    term (q, r);
};
```

The brief declaration uses `(field1: Type1, field2: Type2)` syntax for multi-field returns.

### 7.5 Supported Types

| Brief Type | Description |
|------------|-------------|
| `String` | Text |
| `Int` | 64-bit integer |
| `Float` | 64-bit float |
| `Bool` | Boolean |
| `Void` | No return value |
| Custom | User-defined structs |

### 7.6 Generic FFI

```brief
frgn<T> identity(value: T) -> Result<T, Error> from "lib/std/util.toml";
```

### 7.7 Error Handling

FFI functions return `Result<T, E>` types. The compiler enforces that FFI errors must be handled - code that ignores errors is rejected.

```brief
frgn read_file(path: String) -> Result<String, IoError> from "lib/std/io.toml";

defn safe_read(path: String) -> String [true][result.len() >= 0] {
    let result = read_file(path);
    if result.is_ok() {
        term result.value;
    } else {
        term "default";
    }
};
```

#### Error Projection Methods

Result types support these projection methods:

| Method | Returns | Description |
|--------|---------|-------------|
| `.is_ok()` | `Bool` | True if success |
| `.is_err()` | `Bool` | True if error |
| `.value` | `T` | Success value (fields as declared) |
| `.error.code` | `E.code` | Error code |
| `.error.message` | `E.message` | Error message |

#### FFI Error Contract Enforcement

The compiler rejects code that:
- Calls an FFI function without handling the Result
- Accesses `.value` without first checking `.is_ok()`

This ensures all error paths are explicitly handled.

---

## 8. Reactor Model

### 8.1 Blackboard Architecture

Brief programs have no `main()` function. Instead:

1. **State**: Global variables form the blackboard
2. **Reactor**: Continuously evaluates `rct` preconditions
3. **Transactions**: Fire when preconditions become true
4. **Equilibrium**: Program ends when nothing can fire

### 8.2 Dependency Tracking

The reactor tracks which variables each `rct` reads:
- Only dirty preconditions are re-evaluated
- At equilibrium, reactor sleeps (zero CPU)

### 8.3 Async Transactions

Async transactions run concurrently with compiler-verified safety:

```brief
rct async txn write_a [ready && !busy][busy == true] {
    &data = "A";
    &busy = false;
    term;
};

rct async txn write_b [ready && !busy][busy == true] {
    &data = "B";
    &busy = false;
    term;
};
```

The compiler verifies preconditions are mutually exclusive **when they write to overlapping state**. Preconditions that only read variables, or write to completely different variables, can coexist.

### 8.4 Reactor Throttling

Reactor polling frequency can be controlled at file and transaction level:

**File-level default:**
```brief
reactor @10Hz;  // Default if not specified
```

**Per-transaction override:**
```brief
rct txn fast [condition][post] { ... } @60Hz;
rct txn slow [condition][post] { ... } @5Hz;
```

**Rules:**
- Default: `@10Hz` if not specified
- Global speed: `max(@Hz)` across all files
- Adaptive scheduling: slower files are checked less frequently
- Compiler warns for `@10000Hz+` (usually unintended)

**Common speeds:**
| Use case | Speed | Description |
|----------|-------|-------------|
| Browser UI | `@10Hz` | Smooth interaction, low CPU |
| Game logic | `@60Hz` | Frame-synchronized |
| Data sync | `@1Hz` | Occasional polling |

---

## 9. Structs

### 9.1 Plain Structs

```brief
struct BankAccount {
    balance: Int;
    overdraft_limit: Int;
    
    txn withdraw(amount: Int)
        [amount > 0 && amount <= balance + overdraft_limit]
        [balance == @balance - amount]
    {
        &balance = balance - amount;
        term;
    };
};
```

### 9.2 Render Structs

Render structs combine state with HTML views for UI components:

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

### 9.2.1 Multi-Element HTML

Render structs can produce multiple elements - just include multiple HTML blocks:

```brief
rstruct Form {
    name: String;
    email: String;

    <div class="name-field">{name}</div>
    <div class="email-field">{email}</div>
}
```

### 9.2.2 Standalone Render

A `render` block provides HTML without associated state:

```brief
render Button {
    <button class="primary">Click me</button>
}
```

### 9.2.3 CSS Import

CSS files are imported at the top of the file with standard imports:

```brief
import "./styles/main.css";
import "./styles/theme.css";
```

---

## 10. Imports

### 10.1 Namespace Import

```brief
import std.io;
```

### 10.2 Selective Import

```brief
import { print, println } from std.io;
```

### 10.3 Aliased Import

```brief
import { println as log } from std.io;
```

---

## 11. Standard Library

### 11.1 Philosophy

- **Native Brief**: Functions that can be expressed in Brief use `defn`
- **FFI**: Functions requiring system access use `frgn` with TOML bindings

### 11.2 Native Functions (defn)

```brief
defn absolute(x: Int) -> Int [true][result >= 0] {
    [x < 0] term -x;
    [x >= 0] term x;
};

defn min(a: Int, b: Int) -> Int [true][result == a || result == b] {
    [a <= b] term a;
    [a > b] term b;
};
```

### 11.3 FFI Functions (frgn)

```brief
frgn print(msg: String) -> Result<Bool, IoError> from "lib/std/io.toml";
frgn sqrt(x: Float) -> Result<Float, MathError> from "lib/std/math.toml";
```

---

## 12. Contract Verification

### 12.1 What the Compiler Proves

1. **Precondition satisfiability**: Precondition can be true
2. **Postcondition implication**: All paths satisfy postcondition
3. **Termination reachability**: At least one path to `term`
4. **Mutual exclusion**: Async transactions don't conflict

### 12.2 Error Messages

Brief errors teach the programmer:

```
[E001] Transaction 'increment' violates postcondition

Path analysis:
  1. Precondition: count < 100
  2. Assignment: count' = count + 1
  3. Postcondition: count' == @count + 1

Hint: The postcondition can be satisfied when count < 100.
```

---

## 13. Examples

### 13.1 Counter

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

### 13.2 Bank Transfer

```brief
let alice: Int = 1000;
let bob: Int = 500;

txn transfer(amount: Int)
    [amount > 0 && amount <= alice]
    [alice == @alice - amount && bob == @bob + amount]
{
    &alice = alice - amount;
    &bob = bob + amount;
    term;
};
```

### 13.3 State Machine

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

rct txn reset [state == 2][state == 0] {
    &state = 0;
    term;
};
```

---

*End of Specification v6.2*
