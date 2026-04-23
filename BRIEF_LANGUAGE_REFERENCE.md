# Brief Language Reference Guide

**Version:** v0.11.0  
**Date:** 2026-04-23  
**Status:** Development

---

## Table of Contents

1. [Language Variants](#language-variants)
2. [Core Syntax](#core-syntax)
3. [Transactions](#transactions)
4. [Contracts](#contracts)
5. [Types](#types)
6. [Expressions](#expressions)
7. [Embedded Brief Extensions](#embedded-brief-extensions)
8. [FFI and Foreign Functions](#ffi-and-foreign-functions)
9. [Rendered Brief](#rendered-brief)
10. [Test Cases Reference](#test-cases-reference)

---

## Language Variants

| Extension | Name | Description |
|-----------|------|-------------|
| `.bv` | Core Brief | Transactional state machines with FFI |
| `.ebv` | Embedded Brief | Adds vectors, bit-ranges, native Float |
| `.rbv` | Rendered Brief | Adds UI/view components |

---

## Core Syntax

### 1. State Declarations

```brief
let counter: Int = 0;
let flag: Bool = true;
let name: String = "test";
```

**Test Case:** `test_cases/v011/core/01_basic_transaction.bv`

---

### 2. Transactions

#### Basic Transaction
```brief
rct txn <name> [precondition] [postcondition] {
    <body>
    term;
};
```

**Example:**
```brief
let counter: Int = 0;

rct txn increment [counter < 10]
  [counter == @counter + 1]
{
    &counter = counter + 1;
    term;
};
```

**Test Case:** `test_cases/v011/core/01_basic_transaction.bv`

#### Async Transaction
```brief
rct async txn <name> [precondition] [postcondition] {
    <body>
    term;
};
```

**Example:**
```brief
rct async txn reader [resource == 0]
  [reader_count == @reader_count + 1]
{
    &reader_count = reader_count + 1;
    term;
};
```

**Test Case:** `test_cases/v011/core/02_async_transaction.bv`

---

## Contracts

### Precondition and Postcondition

```brief
[pre_condition] [post_condition]
```

- **Precondition**: Must be true for transaction to fire
- **Postcondition**: Guaranteed true after transaction completes

**Example:**
```brief
rct txn increment [counter < 10]
  [counter == @counter + 1]
{
    &counter = counter + 1;
    term;
};
```

### Watchdog (Third Contract Bracket)

```brief
[pre][post][watchdog]       // Optional watchdog (default)
[pre][post][?watchdog]      // Explicit optional watchdog
[pre][post][!watchdog]      // Required watchdog (! prefix = always enforced)
```

The watchdog is an optional third bracket that is checked at `term`.

| Prefix | Meaning |
|--------|---------|
| none | Optional - only enforced if proof fails |
| `?` | Explicit optional |
| `!` | Required - always enforced in simulation/synthesis |

**Example:**
```brief
rct txn process [ready == true][done == true][done]
{
    &done = true;
    term;
};

// Explicit optional (? prefix)
rct txn opt_watchdog [count >= 0][count == @count + 1][?watchdog_active]
{
    &count = count + 1;
    term;
};

// Required watchdog (! prefix = always enforced)
rct txn req_watchdog [count >= 0][count == @count + 1][!watchdog_active]
{
    &count = count + 1;
    term;
};
```

**Test Case:** `test_cases/v011/embedded/02_watchdog.bv`

---

## Types

### Primitive Types

| Type | Description | Example |
|------|-------------|---------|
| `Int` | Signed integer | `let x: Int = 42;` |
| `UInt` | Unsigned integer | `let x: UInt = 42;` |
| `Float` | Floating point (embedded only) | `let x: Float = 3.14;` |
| `Bool` | Boolean | `let flag: Bool = true;` |
| `String` | Text | `let name: String = "test";` |
| `Void` | No value | `-> Void` |
| `Data` | Raw bytes | |

### Vector Types (Embedded Brief)

```brief
let buffer: Int[1024];
```

Generates a fixed-size array of 1024 integers.

**Test Case:** `test_cases/v011/embedded/01_vector_types.ebv`

### Union Types

```brief
sig fetch_data: Int -> Bool | Int;

txn load []
{
    let result = fetch_data(1);
    Bool(success) = result;
    [success == true] &status = 1;
    term;
};
```

**Test Case:** `test_cases/v011/core/04_union_types.bv`

---

## Expressions

### Arithmetic Operators

| Operator | Description | Example |
|----------|-------------|---------|
| `+` | Addition | `counter + 1` |
| `-` | Subtraction | `value - 5` |
| `*` | Multiplication | `x * y` |
| `/` | Division | `x / 2` |

### Unary Operators

| Operator | Description | Example |
|----------|-------------|---------|
| `-` | Negation | `-value` or `-5` |
| `!` | Logical NOT | `!flag` |
| `~` | Bitwise NOT | `~mask` |

**Important:** Unary negation is now supported in all expressions.

**Test Case:** `test_cases/v011/core/03_unary_negation.bv`

### Comparison Operators

| Operator | Description |
|----------|-------------|
| `==` | Equal |
| `!=` | Not equal |
| `<` | Less than |
| `<=` | Less or equal |
| `>` | Greater than |
| `>=` | Greater or equal |

### Logical Operators

| Operator | Description |
|----------|-------------|
| `&&` | Logical AND |
| `||` | Logical OR |
| `!` | Logical NOT |

### Prior State (`@variable`)

The `@` prefix accesses the value before the transaction executes:

```brief
[counter == @counter + 1]  // postcondition: counter increased by 1
```

---

## Embedded Brief Extensions

### Bit-Range Addressing

```brief
let control: UInt @ 0x80000000 /0..7;
let buffer: Int[16] @ 0x80001000 /x16;
```

**Syntax:** `@ address /bit_spec`

**Bit Specs:**
- `/xN` - N-bit width (e.g., `/x16` = 16 bits)
- `/lo..hi` - Bit range (e.g., `/0..7` = 8 bits, bits 0-7)
- `/N` - Single bit N

**Test Case:** `test_cases/v011/embedded/01_vector_types.ebv`

### Vector Operations

Vectors support element-wise operations:

```brief
let buffer: Int[16];
&buffer[0] = 42;  // Write to element
let val = buffer[0];  // Read element
```

---

## FFI and Foreign Functions

### Foreign Signatures

```brief
sig my_function: Int -> Bool;
```

### Foreign Bindings

```brief
frgn! my_function(val: Int) from "path/to/lib";
```

### System Calls

```brief
syscall! read(fd: Int, buf: String) -> Int;
syscall! write(fd: Int, data: String) -> Int;
```

---

## Rendered Brief (.rbv)

RBV adds HTML view components for web UIs. The HTML is embedded directly in the Brief code.

### Trigger Syntax

| Syntax | Meaning |
|--------|---------|
| `b-trigger:click="txn"` | HTML button triggers a transaction |
| `b-on:submit="action"` | Alternative trigger syntax |

```html
<button b-trigger:click="increment">+</button>
<button b-trigger:click="decrement">-</button>
<button b-trigger:click="reset">Reset</button>
```

### State Binding Directives

| Directive | Meaning |
|-----------|---------|
| `b-text="expr"` | Bind expression to text content |
| `b-show="expr"` | Show element when expr is true |
| `b-hide="expr"` | Hide element when expr is true |

```html
<p>Count: <span b-text="count">0</span></p>
<div b-show="step == 1">Step 1 content</div>
```

### RStruct (Reactive Struct)

```brief
rstruct Counter {
    let value: Int = 0;

    rct txn increment [value < 100]
      [value == @value + 1]
    {
        &value = value + 1;
        term;
    };
}
```

### View Directives

```brief
#button[id="submit"] => txn.submit
#input[id="name"] => model.name
```

---

## Embedded Brief (.ebv)

### Hardware Triggers

Hardware triggers (`trg`) define external input signals mapped to memory addresses:

```brief
// EBV: Hardware trigger as input signal
trg button: Bool @ 0x4000;
```

Synthesized to: `input logic button;`

### Within Clause

The `within N cycles` syntax is used for cycle-accurate timeouts in assignments:

```brief
let result = expr within N cycles;
```

Time units: `cycles`, `cyc`, `ms`, `s`, `sec`, `min`

```brief
let result = read_spi() within 10 cycles;
```

---

### ~/ Shorthand (Prior State Toggle)

```brief
~/identifier
```

Expands to two contracts: `[~identifier][identifier]` (pre is NOT, post is identifier).

```brief
rct txn toggle [~/ready][ready]
{
    &ready = !ready;
    term;
};
```

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
| `embedded/02_watchdog.bv` | Watchdog contract | ✅ Pass |
| `embedded/03_float_types.bv` | Float (parsing only) | ✅ Pass |
| `embedded/04_triggers.bv` | Trigger syntax | ✅ Pass |
| `embedded/05_within.bv` | Transaction syntax | ✅ Pass |

### Examples Directory

| File | Feature |
|------|---------|
| `examples/simple_contract.bv` | Basic contracts |
| `examples/async_mutual_exclusion.bv` | Async + exclusion |
| `examples/union_types.bv` | Union types |
| `examples/vector_test.ebv` | Vector operations |
| `examples/blinker.ebv` | Timing/within |

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

---

## Error Codes

| Code | Description |
|------|-------------|
| EBV001 | Parse error |
| EBV002 | Transaction cannot be triggered |
| EBV003 | Contract violation |
| EBV004 | Type mismatch |
| EBV005 | Import resolution failed |

---

*Last Updated: 2026-04-23*