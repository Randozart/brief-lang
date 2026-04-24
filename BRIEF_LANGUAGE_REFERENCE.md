# Brief Language Reference Guide

**Version:** v0.11.0
**Date:** 2026-04-23
**Status:** Development

---

## Table of Contents

1. [Lexical Structure](#lexical-structure)
2. [Types](#types)
3. [State Declarations](#state-declarations)
4. [Constants](#constants)
5. [Transactions](#transactions)
6. [Contracts](#contracts)
7. [Definitions (defn)](#definitions-defn)
8. [Structs](#structs)
9. [RStructs (Reactive Structs)](#rstructs-reactive-structs)
10. [Enums](#enums)
11. [Signatures (FFI)](#signatures-ffi)
12. [Foreign Bindings](#foreign-bindings)
13. [Resources](#resources)
14. [Triggers (EBV)](#triggers-ebv)
15. [Render Blocks (RBV)](#render-blocks-rbv)
16. [Imports](#imports)
17. [Expressions](#expressions)
18. [Statements](#statements)
19. [Time Units](#time-units)

---

## Lexical Structure

### Keywords

| Keyword | Aliases | Description |
|---------|---------|-------------|
| `sig` | `sign`, `signature` | Foreign function signature |
| `defn` | `def`, `definition` | Function/predicate definition |
| `let` | - | State variable declaration |
| `const` | `constant` | Constant declaration |
| `txn` | `transact`, `transaction` | Transaction |
| `rct` | - | Reactive transaction |
| `async` | - | Async modifier |
| `term` | - | Termination statement |
| `escape` | - | Escape/return statement |
| `import` | - | Import statement |
| `from` | - | Import path delimiter |
| `as` | - | Alias/rename |
| `frgn` | - | Foreign binding |
| `frgn!` | - | Foreign binding (native) |
| `syscall` | - | System call binding |
| `syscall!` | - | System call binding (native) |
| `resource` | `rsrc` | Resource declaration |
| `struct` | - | Struct definition |
| `rstruct` | - | Reactive struct definition |
| `render` | - | Render block (RBV) |
| `enum` | - | Enum definition |
| `trg` | - | Hardware trigger (EBV) |
| `stage` | - | Pipeline stage |
| `on` | - | Trigger condition |
| `forall` | - | Universal quantifier *(planned)* |
| `exists` | - | Existential quantifier *(planned)* |
| `within` | - | Timeout clause |
| `bank` | - | Memory bank |
| `match` | - | Match expression *(planned/not fully implemented)* |
| `some` | `none` | Option variants |

### Type Keywords

| Keyword | Aliases | Description |
|---------|---------|-------------|
| `Int` | - | Signed integer |
| `UInt` | `Unsigned`, `USgn` | Unsigned integer |
| `Signed` | `Sgn` | Signed type |
| `Float` | - | Floating point |
| `String` | - | String type |
| `Bool` | - | Boolean |
| `Data` | - | Raw data |
| `Void` | - | Void/no return |

### Operators

| Operator | Description |
|----------|-------------|
| `=` | Assignment |
| `==` | Equality |
| `!=` | Inequality |
| `<` | Less than |
| `<=` | Less or equal |
| `>` | Greater than |
| `>=` | Greater or equal |
| `<<` | Shift left |
| `>>` | Shift right |
| `&` | Mutable reference / Bitwise AND |
| `\|` | Bitwise OR |
| `\|\|` | Logical OR |
| `&&` | Logical AND |
| `!` | Logical NOT |
| `-` | Negation |
| `~` | Bitwise NOT |
| `~/` | Prior state toggle |
| `+` | Addition |
| `*` | Multiplication |
| `/` | Division |
| `^` | Bitwise XOR |
| `->` | Arrow/return type |
| `@` | Address / Prior state |
| `?` | Optional watchdog prefix |

### Punctuation

| Token | Description |
|-------|-------------|
| `[` `]` | Brackets / Contracts |
| `{` `}` | Blocks |
| `(` `)` | Groups / Parameters |
| `:` | Type annotation |
| `,` | Separator |
| `;` | Statement terminator |
| `..` | Range |
| `.` | Field access / Namespace / RStruct method |

---

## Types

### Primitive Types

```brief
let x: Int = 42;
let y: UInt = 100;
let flag: Bool = true;
let name: String = "hello";
let pi: Float = 3.14;
```

### Vector Types

```brief
let buffer: Int[16];        // Fixed-size array
let matrix: Float[4][4];     // 2D array
```

### Constrained Types (Bit-Range)

```brief
let byte: UInt /8;          // 8-bit unsigned
let nibble: UInt /4;        // 4-bit unsigned
let word: Int /16;          // 16-bit signed
let flags: UInt /x8;        // Inferred 8-bit
```

### Union Types

```brief
let result: Int | Error;    // Either Int or Error
let state: Idle | Active | Error;
```

### Custom Types

```brief
let point: Point;  // Declare variable of custom type
let status: Status;  // Declare variable of enum type
```

---

## State Declarations

### Basic Declaration

```brief
let counter: Int = 0;
let name: String = "test";
let enabled: Bool = false;
```

### With Address Mapping (EBV)

```brief
let led: Bool @ 0x4000 = false;           // Memory-mapped at 0x4000
let sensor: UInt @ 0x8000 /8;             // 8-bit sensor at 0x8000
```

### With Bit-Range

```brief
let flags: UInt @ 0x1000 /0..7;           // Bits 0-7 at address 0x1000
let status: UInt /4;                      // 4-bit field
```

### Address with Bit-Range Shorthand

```brief
let data: UInt @ 0x2000 /x16;             // 16-bit value at 0x2000
```

### Memory Regions

```brief
let stack_var: Int @ stack:8;             // Stack offset 8
let heap_ptr: Int @ heap:16;               // Heap offset 16
```

### Vector with Address

```brief
let buffer: UInt[256] @ 0x1000;            // 256-element buffer at 0x1000
```

---

## Constants

```brief
const MAX_SIZE: Int = 100;
const VERSION: String = "1.0.0";
const FLAGS: UInt = 0xFF;
```

---

## Transactions

### Basic Transaction

```brief
txn name [precondition] [postcondition] {
    // body
    term;
};
```

### Reactive Transaction (RCT)

```brief
rct txn name [precondition] [postcondition] {
    &variable = value;
    term;
};
```

### Async Transaction

```brief
async txn name [precondition] [postcondition] {
    term;
};
```

### Reactive Async Transaction

```brief
rct async txn name [precondition] [postcondition] {
    term;
};
```

### With Parameters

```brief
txn add [a: Int] [b: Int] [result == a + b] {
    term result;
};
```

### Lambda-style (No Body)

```brief
txn identity [x: Int] [result == x];
```

### With Reactor Speed

```brief
rct txn blink @60Hz [true] [led == !led] {
    term;
};
```

### Transaction Method (dot syntax)

```brief
rct txn counter.increment [count < max] [count == @count + 1] {
    &count = count + 1;
    term;
};
```

### Transaction Dependencies

Dependencies are inferred from pre/post conditions automatically.

---

## Contracts

### Precondition and Postcondition

```brief
[pre_condition] [post_condition]
```

### Watchdog (Third Contract Bracket)

```brief
[pre][post][watchdog]     // Required watchdog (default)
[pre][post][?watchdog]     // Optional watchdog
```

The watchdog is checked at `term`.

### Prior State Toggle Shorthand

```brief
~/identifier
```

Expands to: `[~identifier][identifier]`

```brief
rct txn toggle [~/ready][ready] {
    &ready = !ready;
    term;
};
```

### Examples

```brief
rct txn increment [counter < 10]
  [counter == @counter + 1]
{
    &counter = counter + 1;
    term;
};

rct txn guarded [x > 0][y == x * 2]
{
    &y = x * 2;
    term;
};

rct txn with_watchdog [ready == true][done == true][?timeout] {
    &done = true;
    term;
};
```

---

## Definitions (defn)

### Predicate Definition

```brief
defn sufficient_funds(amount: Int) [amount > 0][true] -> Bool {
    term amount >= minimum_balance;
};
```

### Function Definition with Contract

```brief
defn square(x: Int) [true] [result == x * x] -> Int {
    term x * x;
};
```

---

## Structs

```brief
struct Point {
    let x: Int = 0;
    let y: Int = 0;
};

struct Rectangle {
    width: Int,
    height: Int,
};

// Struct with embedded transactions
struct Counter {
    let value: Int = 0;

    txn increment [value < 100][value == @value + 1] {
        &value = value + 1;
        term;
    };
};
```

### Field Declaration Syntax

```brief
// Using let (required initializer)
let x: Int = 0;
let y: Int = 0;

// Direct field syntax
field_name: Type,
```

---

## RStructs (Reactive Structs)

RStructs automatically namespace transactions with the struct name.

```brief
rstruct Counter {
    let value: Int = 0;

    txn increment [value < 100][value == @value + 1] {
        &value = value + 1;
        term;
    };
};
```

After parsing, `increment` becomes `Counter.increment`.

---

## Enums

### Simple Enum

```brief
enum Status {
    Idle,
    Processing,
    Done,
    Error,
};
```

### Enum with Type Parameters

```brief
enum Result<T, E> {
    Ok(T),
    Err(E),
};
```

### Tuple Variants

```brief
enum Value {
    Int(Int),
    Float(Float),
    Pair(Int, Float),
};
```

### Enum Usage

Enums use bare variant names:

```brief
let state: Status = Idle;

// Comparison
[state == Idle] &state = Processing;

// Pattern matching via guard
Status(s) = state;
[s == Processing] &state = Done;
```

**Note:** Enum variants are compared and assigned using bare names, not namespaced syntax.

---

## Signatures (FFI)

### Basic Signature

```brief
sig my_function: Int -> Bool;
```

### With Source

```brief
sig read: String -> String from "io.fs";
```

### With Binding

```brief
sig process: Int -> Int = complex(x);
```

---

## Foreign Bindings

### Foreign Binding (Native)

```brief
frgn! fetch(url: String) -> Result<Data, Error> from "http.toml";
```

### Foreign Binding (WebAssembly)

```brief
frgn fetch(url: String) -> Result<Data, Error> from "http.toml";
```

### System Call

```brief
syscall! read(fd: Int, buf: String) -> Result<Int, Error>;
```

---

## Resources

```brief
resource uart: UART {
    baud_rate: 9600,
    parity: None,
};

rsrc buffer: RingBuffer {
    size: 1024,
    element_type: UInt,
};
```

---

## Triggers (EBV)

Hardware triggers define external input signals.

```brief
trg button: Bool @ 0x4000;
trg sensor: UInt @ 0x8000 /8;
```

Synthesized to: `input logic button;`

---

## Render Blocks (RBV)

```brief
render Counter {
    <div class="counter">
        <span b-text="value">0</span>
        <button b-trigger:click="increment">+</button>
        <button b-trigger:click="decrement">-</button>
    </div>
}
```

### RBV Directives

| Directive | Example | Description |
|-----------|---------|-------------|
| `b-text` | `b-text="count"` | Text content binding |
| `b-show` | `b-show="visible"` | Conditional show |
| `b-hide` | `b-hide="hidden"` | Conditional hide |
| `b-trigger:event` | `b-trigger:click="txn"` | Event trigger |
| `b-on:event` | `b-on:submit="action"` | Event trigger (alt) |
| `b-class` | `b-class="{active: isActive}"` | Dynamic class |
| `b-attr` | `b-attr="disabled: isDisabled"` | Dynamic attribute |
| `b-style` | `b-style="color: fg"` | Dynamic style |
| `b-each` | `b-each="item in items"` | List rendering |

---

## Imports

### Single Import

```brief
import "std/io";
```

### Multiple Imports

```brief
import {
    "std/io",
    "std/strings",
    "custom/utils",
};
```

### With Alias

```brief
import "std/io" as io;
```

---

## Expressions

### Literals

```brief
42          // Integer
3.14        // Float
"hello"     // String
true        // Boolean
false       // Boolean
```

### Identifiers

```brief
counter
max_value
is_enabled
```

### Prior State (@)

```brief
@counter        // Previous value of counter
@x + 1          // Prior x plus 1
```

### Mutable Reference (&)

```brief
&variable       // Mutable reference for assignment
```

### Unary Operations

```brief
!flag           // Logical NOT
-x              // Arithmetic negation
~bits           // Bitwise NOT
```

### Binary Operations

```brief
x + y           // Addition
x - y           // Subtraction
x * y           // Multiplication
x / y           // Division
x == y          // Equality
x != y          // Inequality
x < y           // Less than
x <= y          // Less or equal
x > y           // Greater than
x >= y          // Greater or equal
x && y          // Logical AND
x || y          // Logical OR
x & y           // Bitwise AND
x | y           // Bitwise OR
x ^ y           // Bitwise XOR
x << n          // Shift left
x >> n          // Shift right
```

### Function Call

```brief
process(data)
max(a, b)
```

### Method Call

```brief
result.validate()
list.length()
```

### Field Access

```brief
point.x
rect.width
```

### Index Access

```brief
buffer[0]
matrix[i][j]
```

### Pattern Matching

Brief uses **guard-based pattern matching** for unions and enums:

```brief
// Extract variant from union type
let result: Int | Error = fetch_data();
Ok(value) = result;
[value > 0] &status = Success;

// Enum pattern matching
let state: Status = Idle;
Status(s) = state;
[s == Processing] &state = Done;
```

*(Note: `match { }` expression syntax is planned but not yet implemented)*

### Quantifiers *(planned)*

```brief
forall x in range(0, 10) { x >= 0 }
exists y in set { y > 0 }
```

---

## Statements

### Assignment

```brief
x = 42;
counter = counter + 1;
```

### Mutable Assignment

```brief
&variable = new_value;
```

### With Timeout

```brief
result = read_spi() within 10 cycles;
data = fetch(url) within 100 ms;
```

### Guarded Statement

```brief
[condition] statement;
[condition] {
    // multiple statements
};
```

### Pattern Matching (Guard-based)

```brief
// Union type pattern extraction
let result: Int | Error = fetch();
Ok(value) = result;
[value > 0] &status = Success;

// Enum variant extraction
let state: Status = Idle;
Status(s) = state;
```

### Term (Termination)

```brief
term;                     // Void termination
term result;              // Return value
term (a, b);              // Multiple outputs
```

### Escape

```brief
escape;                   // Early exit
escape error_code;        // Exit with value
```

### Expression Statement

```brief
process_data();
update_state();
```

---

## Time Units

| Unit | Aliases | Description |
|------|---------|-------------|
| `cycles` | `cyc` | Clock cycles |
| `ms` | - | Milliseconds |
| `s` | `sec`, `seconds` | Seconds |
| `min` | `minute` | Minutes |

---

## Test Cases Reference

### Core Brief (.bv)

| File | Feature | Status |
|------|---------|--------|
| `core/01_basic_transaction.bv` | Basic transaction | ✅ Pass |
| `core/02_async_transaction.bv` | Async transactions | ✅ Pass |
| `core/03_unary_negation.bv` | Unary negation | ✅ Pass |
| `core/04_union_types.bv` | Union types | ✅ Pass |
| `core/05_guards.bv` | Guards | ✅ Pass |
| `core/06_dependencies.bv` | Transaction dependencies | ✅ Pass |
| `core/07_structs.bv` | Struct syntax | ✅ Pass |
| `core/08_enums.bv` | Enum syntax | ✅ Pass |
| `core/09_sig_type.bv` | Foreign signatures | ✅ Pass |
| `core/10_imports.bv` | Import statements | ✅ Pass |

### Embedded Brief (.bv - extended)

| File | Feature | Status |
|------|---------|--------|
| `embedded/01_vector_types.bv` | Vectors + bit-range | ✅ Pass |
| `embedded/02_watchdog.bv` | Watchdog contracts | ✅ Pass |
| `embedded/03_float_types.bv` | Float (parsing only) | ✅ Pass |
| `embedded/04_triggers.bv` | Trigger syntax | ✅ Pass |
| `embedded/05_within.bv` | Transaction syntax | ✅ Pass |
| `embedded/06_within_clause.bv` | Within clause | ✅ Pass |

---

## Language Variants

| Extension | Name | Description |
|-----------|------|-------------|
| `.bv` | Core Brief | Transactional state machines with FFI |
| `.ebv` | Embedded Brief | Adds vectors, bit-ranges, triggers, hardware mapping |
| `.rbv` | Rendered Brief | Adds UI/view components with reactive bindings |

---

## Compilation Targets

### Verilog/SystemVerilog (FPGA)
```bash
brief-compiler verilog input.ebv --hw hardware.toml
```

### ARM Rust (Bare-Metal)
```bash
brief-compiler arm input.ebv --hw hardware.toml
```

### WASM (Browser)
```bash
brief-compiler wasm input.bv
```
