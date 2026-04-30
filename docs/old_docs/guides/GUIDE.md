# Brief Language Guide

A transactional, contract-enforced language compiler. Brief treats program execution as verified state transitions with mathematical proofs at compile time.

## Table of Contents

1. [Introduction](#introduction)
2. [Getting Started](#getting-started)
3. [Basic Syntax](#basic-syntax)
4. [Types](#types)
5. [Contracts](#contracts)
6. [Transaction Modifiers](#transaction-modifiers)
7. [Functions](#functions)
8. [Structs & Components](#structs--components)
9. [Expressions](#expressions)
10. [Imports & Foreign](#imports--foreign)
11. [Standard Library](#standard-library)
12. [Rendered Brief (.rbv)](#rendered-brief-rbv)
13. [Examples](#examples)
14. [FAQ](#faq)

---

## Introduction

Brief is a declarative language where:

- **Transactions are contracts.** Every state change is proven valid before execution.
- **No runtime surprises.** The compiler verifies all state transitions at compile time.
- **Lock-free concurrency.** Preconditions act as hardware-level gates — no mutexes needed.
- **Formal verification without boilerplate.** Reactive state machines with pre/post conditions.

```brief
let balance: Int = 100;
let withdrawn: Int = 0;

txn withdraw(amount: Int) [amount > 0 && amount <= balance][balance == @balance - amount] {
  &balance = balance - amount;
  &withdrawn = withdrawn + amount;
  term;
};
```

---

## Getting Started

### Installation

```bash
cargo install --path .
```

### Running Brief Files

```bash
# Type check without execution (fast)
brief check program.bv

# Build and execute
brief build program.bv

# Watch for changes and rebuild
brief watch program.bv
```

### File Types

- **`.bv`** - Pure Brief code (state, transactions, functions)
- **`.rbv`** - Brief + HTML/CSS (Rendered Brief for reactive UIs)

---

## Basic Syntax

### State Declarations

Declare mutable state with `let`:

```brief
let count: Int = 0;
let name: String = "Guest";
let items: List<String> = [];
```

### Transactions

Transactions are the core unit of execution in Brief. They have:

1. A name
2. A contract (pre/post conditions)
3. A body

```brief
txn increment [true][count == @count + 1] {
  &count = count + 1;
  term;
};
```

### Term Statement

The `term` statement outputs values from a transaction:

```brief
txn greet [true] {
  term "Hello, World!";
};

txn add(a: Int, b: Int) [true][result == a + b] {
  let result: Int = a + b;
  term result;
};
```

Multi-output transactions use trailing commas:

```brief
txn swap [true][a == @b && b == @a] {
  let temp: Int = a;
  &a = b;
  &b = temp;
  term a, b,;
};
```

---

## Types

### Primitive Types

| Type | Description | Example |
|------|-------------|---------|
| `Int` | 64-bit integer | `42`, `-17` |
| `Float` | 64-bit float | `3.14`, `-0.5` |
| `String` | UTF-8 string | `"Hello"` |
| `Bool` | Boolean | `true`, `false` |
| `Void` | No value | (return type) |
| `Data` | Opaque data | (opaque) |

### Container Types

```brief
// List of strings
let items: List<String> = ["apple", "banana"];

// Empty list
let empty: List<Int> = [];
```

### Custom Types

```brief
// A custom type alias
let config: MyConfig = Data();
```

### Generic Types

```brief
defn first<T>(list: List<T>) [list.len() > 0][result == list[0]] -> T {
  term list[0];
};
```

### Type Bounds

```brief
defn double<N: Add>(n: N) [true][result == n + n] -> N {
  let result: N = n + n;
  term result;
};
```

---

## Contracts

Contracts define the conditions under which a transaction can execute.

### Syntax

```
txn name [pre_condition][post_condition] { ... }
```

- **[pre]** - Must be true for transaction to execute
- **[post]** - Will be true after execution

### Prior State (`@`)

Use `@` to reference the prior state (before transaction execution):

```brief
txn increment [count < 100][count == @count + 1] {
  &count = count + 1;
  term;
};
```

### Shorthand: `~/`

The `~/` shorthand means "not the prior state":

```brief
// These are equivalent:
txn set_active [~/active][active == true] { ... }
txn set_active [![@active]][active == true] { ... }
```

### Multi-Output with `-> true`

Return `true` to indicate success without specific output:

```brief
txn save_data [data != null][-> true] {
  // save logic
  term;
};
```

---

## Transaction Modifiers

### Basic Transaction (`txn`)

```brief
txn process [true] {
  term;
};
```

### Reactive Transaction (`rct`)

Reactive transactions automatically trigger when their preconditions become true:

```brief
rct auto_save [data_changed][saved == true] {
  &saved = true;
  term;
};
```

### Async Transaction (`async`)

Async transactions run in the background and can interleave:

```brief
async txn fetch_data [url != ""][data == result] {
  let data: String = fetch(url);
  term data;
};
```

### Combined Modifiers

```brief
async rct txn live_update [data != @data][displayed == data] {
  &displayed = data;
  term;
};
```

### Sugar: `txc` (Contract-Only)

Use `txc` for transactions that only need post-conditions:

```brief
txc increment(amount: Int) [count == @count + amount];
```

Desugars to:

```brief
txn increment(amount: Int) [true][count == @count + amount] {
  &count = count + amount;
  term;
};
```

---

## Functions

### Signatures (`sig`)

Declare function interfaces:

```brief
sig add: (Int, Int) -> Int;
sig fetch: (String) -> String;
sig process: (Data) -> (Bool, String);
```

### Definitions (`defn`)

```brief
defn add(a: Int, b: Int) [true][result == a + b] -> Int {
  let result: Int = a + b;
  term result;
};
```

### Type Parameters

```brief
defn first<T>(list: List<T>) [list.len() > 0] -> T {
  term list[0];
};

defn map<A, B>(list: List<A>, f: A -> B) [true][result.len() == list.len()] -> List<B> {
  let result: List<B> = [];
  term result;
};
```

### Type Bounds

```brief
defn double<N: Add>(n: N) [true][result == n + n] -> N {
  let result: N = n + n;
  term result;
};
```

---

## Structs & Components

### Struct Definition

```brief
struct Counter {
  count: Int;

  txn increment [true][count == @count + 1] {
    &count = count + 1;
    term;
  };

  txn decrement [count > 0][count == @count - 1] {
    &count = count - 1;
    term;
  };
}
```

### Render Block (Separate View)

```brief
struct Counter {
  count: Int;
  
  txn increment [true][count == @count + 1] {
    &count = count + 1;
    term;
  };
}

render Counter {
  <div class="counter">
    <span b-text="count">0</span>
    <button b-trigger:click="increment">+</button>
  </div>
}
```

### RStruct (Inline View)

In `.rbv` files, use `rstruct` to combine struct definition with inline view:

```brief
rstruct Counter {
  count: Int;

  txn increment [true][count == @count + 1] {
    &count = count + 1;
    term;
  };

  <div class="counter">
    <span b-text="count">0</span>
    <button b-trigger:click="increment">+</button>
  </div>
}
```

---

## Expressions

### Operators

| Operator | Description |
|----------|-------------|
| `+`, `-`, `*`, `/` | Arithmetic |
| `==`, `!=`, `<`, `>`, `<=`, `>=` | Comparison |
| `&&`, `\|\|` | Logical |
| `!`, `~` | Unary NOT, bitwise NOT |
| `-` (prefix) | Negation |

### Ownership (`&`)

Use `&` to assign to mutable state:

```brief
&count = count + 1;
&name = "Updated";
&items = items + ["new"];
```

Without `&`, you're creating a local binding:

```brief
let x: Int = count;    // local copy
&count = count + 1;    // mutation
```

### Prior State (`@`)

```brief
let prev: Int = @count;    // snapshot before transaction
```

### List Operations

```brief
let items: List<String> = [];           // empty list
let first: String = items[0];            // index
let len: Int = items.len();               // length
let combined: List<String> = a + b;       // concatenate
```

### Field Access

```brief
let value: Int = record.field;
let result: String = obj.method();
```

---

## Imports & Foreign

### Import

```brief
import { add, multiply } from "math";
import { fetch } from "http";
```

### Partial Imports

```brief
import { fetch } from "http";
// Only loads `fetch`, other exports available lazily
```

### Foreign Functions (`frgn`)

Declare functions implemented elsewhere:

```brief
frgn sig log: (String) -> Void;
frgn sig random: () -> Int;
frgn sig http_get: (String) -> String;
```

---

## Standard Library

Brief includes a standard library with common operations. Import modules as needed:

```brief
import { abs, sqrt, sin, cos } from "math";
import { len, trim, split } from "string";
import { append, map, filter } from "collections";
import { now, timestamp } from "time";
import { parse, stringify } from "json";
import { base64_encode, url_encode } from "encoding";
```

### std/math - Mathematics

```brief
abs(n: Int) -> Int           # Absolute value
sqrt(n: Float) -> Float       # Square root
pow(base: Float, exp: Float) -> Float  # Power
sin, cos, tan(n: Float) -> Float  # Trigonometry
asin, acos, atan(n: Float) -> Float  # Inverse trig
floor, ceil, round(n: Float) -> Float  # Rounding
random() -> Float            # 0.0 to 1.0
random_int(min, max) -> Int   # Random integer
min, max(a: Int, b: Int) -> Int
log(n: Float, base: Float) -> Float
exp(n: Float) -> Float
```

### std/string - String Operations

```brief
len(s: String) -> Int
concat(a: String, b: String) -> String
trim(s: String) -> String
to_upper(s: String) -> String
to_lower(s: String) -> String
contains(haystack: String, needle: String) -> Bool
starts_with(s: String, prefix: String) -> Bool
ends_with(s: String, suffix: String) -> Bool
find(s: String, needle: String) -> Int
replace(s: String, old: String, new: String) -> String
split(s: String, delim: String) -> List<String>
to_string(n: Int) -> String
to_int(s: String) -> Int
```

### std/collections - List/Collection Operations

```brief
len<T>(list: List<T>) -> Int
append<T>(list: List<T>, item: T) -> List<T>
prepend<T>(item: T, list: List<T>) -> List<T>
concat<T>(a: List<T>, b: List<T>) -> List<T>
get<T>(list: List<T>, index: Int) -> T
set<T>(list: List<T>, index: Int, item: T) -> List<T>
contains<T>(list: List<T>, item: T) -> Bool
find<T>(list: List<T>, item: T) -> Int
map<T, U>(list: List<T>, fn: T -> U) -> List<U>
filter<T>(list: List<T>, pred: T -> Bool) -> List<T>
reduce<T, U>(list: List<T>, init: U, fn: (U, T) -> U) -> U
reverse<T>(list: List<T>) -> List<T>
unique<T>(list: List<T>) -> List<T>
sort<T>(list: List<T>) -> List<T>
take<T>(list: List<T>, n: Int) -> List<T>
drop<T>(list: List<T>, n: Int) -> List<T>
```

### std/time - Time/Date

```brief
now() -> Int              # Unix timestamp (seconds)
now_millis() -> Int       # Unix timestamp (milliseconds)
year(timestamp: Int) -> Int
month(timestamp: Int) -> Int
day(timestamp: Int) -> Int
hour(timestamp: Int) -> Int
minute(timestamp: Int) -> Int
second(timestamp: Int) -> Int
timestamp(y, m, d) -> Int
add_days(timestamp: Int, days: Int) -> Int
diff_days(t1: Int, t2: Int) -> Int
format_timestamp(timestamp: Int, format: String) -> String
```

### std/json - JSON Processing

```brief
parse(s: String) -> Data
stringify(data: Data) -> String
is_null(data: Data) -> Bool
is_string(data: Data) -> Bool
is_number(data: Data) -> Bool
is_array(data: Data) -> Bool
get_string(data: Data, key: String) -> String
get_index(data: Data, index: Int) -> Data
array_len(data: Data) -> Int
keys(data: Data) -> List<String>
```

### std/encoding - Encoding & Hashing

```brief
base64_encode(s: String) -> String
base64_decode(s: String) -> String
hex_encode(s: String) -> String
hex_decode(s: String) -> String
url_encode(s: String) -> String
url_decode(s: String) -> String
html_escape(s: String) -> String
md5(s: String) -> String
sha256(s: String) -> String
uuid_v4() -> String
```

---

## Rendered Brief (.rbv)

Rendered Brief (`.rbv`) combines Brief logic with HTML/CSS in a single file.

### File Structure

```html
<script type="brief">
  // Brief code here
  let count: Int = 0;

  txn increment [true][count == @count + 1] {
    &count = count + 1;
    term;
  };
</script>

<view>
  <!-- HTML with bindings here -->
  <p>Count: <span b-text="count">0</span></p>
  <button b-trigger:click="increment">+1</button>
</view>

<style>
  /* CSS here */
  button { padding: 10px; }
</style>
```

### Directives

| Directive | Description |
|-----------|-------------|
| `b-text="expr"` | Text content |
| `b-html="expr"` | Inner HTML |
| `b-trigger:event="txn"` | Event handler |
| `b-each:item="list"` | List rendering |

### Example: Counter

```html
<script type="brief">
  let count: Int = 0;

  txn increment [true][count == @count + 1] {
    &count = count + 1;
    term;
  };

  txn decrement [count > 0][count == @count - 1] {
    &count = count - 1;
    term;
  };
</script>

<view>
  <div class="counter">
    <span b-text="count">0</span>
    <button b-trigger:click="increment">+</button>
    <button b-trigger:click="decrement">-</button>
  </div>
</view>

<style>
  .counter span { font-size: 48px; }
  .counter button { padding: 10px 20px; }
</style>
```

### Example: Todo List

```html
<script type="brief">
  let items: List<String> = [];

  txn add_item [true][items.len() == @items.len() + 1] {
    &items = items + ["New task"];
    term;
  };

  txn clear_all [true][items.len() == 0] {
    &items = [];
    term;
  };
</script>

<view>
  <div class="container">
    <h1>Todo App</h1>
    
    <button b-trigger:click="add_item">+ Add Task</button>
    
    <div b-each:item="items">
      <span b-text="item">Task</span>
    </div>
    
    <button b-trigger:click="clear_all">Clear All</button>
  </div>
</view>
```

### Compiling RBV

```bash
# Compile to directory
brief rbv component.rbv --out dist/

# Compile and build WASM
brief run component.rbv
```

---

## Examples

### Hello World

```brief
let greeting: String = "Hello";

txn greet [true] {
  term greeting;
};

txn set_greeting [true] {
  &greeting = "Hello, World!";
  term;
};
```

### Bank Account

```brief
let balance: Int = 1000;
let withdrawn: Int = 0;

txn deposit(amount: Int) [amount > 0][balance == @balance + amount] {
  &balance = balance + amount;
  term;
};

txn withdraw(amount: Int) [amount > 0 && amount <= balance][balance == @balance - amount] {
  &balance = balance - amount;
  &withdrawn = withdrawn + amount;
  term;
};

txn reset [true][balance == 1000 && withdrawn == 0] {
  &balance = 1000;
  &withdrawn = 0;
  term;
};
```

### Generic Function

```brief
defn first<T>(list: List<T>) [list.len() > 0] -> T {
  term list[0];
};

defn last<T>(list: List<T>) [list.len() > 0] -> T {
  term list[list.len() - 1];
};

defn append<T>(list: List<T>, item: T) [true][result.len() == list.len() + 1] -> List<T> {
  let result: List<T> = list + [item];
  term result;
};
```

---

## FAQ

### How is Brief different from other languages?

Brief focuses on **contract-enforced state transitions**. Instead of exceptions or runtime checks, the compiler proves that every state change is valid before execution.

### Can Brief be used for web development?

Yes! Use `.rbv` files with the RBV compiler to generate WASM-based web components with reactive bindings.

### What is the difference between `.bv` and `.rbv`?

- `.bv` - Pure Brief (logic only)
- `.rbv` - Brief + HTML/CSS (reactive UI)

### How do contracts work?

Contracts are boolean expressions that must evaluate to true. The proof engine verifies at compile time that all code paths satisfy the contract.

### Can I use external libraries?

Use `frgn` to declare foreign functions that link to external implementations (Rust, C, JS, etc.).

---

## Next Steps

- Read the [Brief Language Specification](../spec/brief-lang-spec.md)
- Check out the [Rendered Brief Spec](../spec/rendered-brief-spec-v4.md)
- Explore the examples in `examples/`
