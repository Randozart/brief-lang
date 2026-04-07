# Brief FFI Guide

**Version:** 7.0  
**Purpose:** How to use and create Foreign Function Interface bindings  

---

## Table of Contents

1. [Overview](#overview)
2. [When to Use FFI](#when-to-use-ffi)
3. [TOML Bindings](#toml-bindings)
4. [Brief Declarations](#brief-declarations)
5. [Error Handling](#error-handling)
6. [Creating Custom Bindings](#creating-custom-bindings)
7. [Type System](#type-system)
8. [Examples](#examples)


## Overview

FFI (Foreign Function Interface) allows Brief to call external functions written in other languages (primarily Rust). Brief cannot do everything - file I/O, networking, hardware math - so FFI provides a bridge.

### Design Principles

1. **TOML is the Contract**: All FFI metadata is declared in TOML files
2. **Brief Wraps Everything**: Foreign code is assumed untrusted; Brief handles all outcomes
3. **Explicit Contracts**: Every foreign function has explicit input/output/error types
4. **Type Safe**: Only specific types can cross the FFI boundary

### How It Works

```
Brief Code
    |
    v
frgn declaration  -->  TOML binding file
    |                      |
    v                      v
Type Checker          Validates signature
    |
    v
Runtime           -->  Rust implementation
```

## When to Use FFI

### Use FFI For

- File I/O (read_file, write_file)
- Console I/O (print, println, input)
- Network access
- Hardware math (sqrt, sin, cos, pow)
- Cryptographic operations
- Time operations (sleep, timestamp)

### Use Native Brief For

- Arithmetic (+, -, *, /, %)
- Comparisons (==, !=, <, >, <=, >=)
- String operations Brief can express
- Logic operations (&&, ||, !)
- Data transformations

### Rule of Thumb

If Brief can express it with a `defn`, use native Brief. If it requires system access or hardware, use FFI.

---

## TOML Bindings

### File Location

TOML binding files go in `lib/std/` or a custom location:

```
lib/std/
├── io.toml
├── math.toml
└── time.toml

my_project/
└── lib/
    └── my_bindings.toml
```

### Basic Structure

```toml
[[functions]]
name = "function_name"
description = "What this function does"
location = "module::path::to::function"
target = "native"
mapper = "rust"  # Optional

[functions.input]
param1 = "Type"
param2 = "Type"

[functions.output.success]
field1 = "Type"
field2 = "Type"

[functions.output.error]
type = "ErrorTypeName"
code = "Int"
message = "String"
```

### Field Reference

| Field | Required | Description |
|-------|----------|-------------|
| `name` | Yes | Brief function name |
| `description` | No | Human-readable description |
| `location` | Yes | Rust module path |
| `target` | Yes | Platform: `native` |
| `mapper` | No | Mapper name (default: `rust`) |
| `input` | Yes | Parameter name-type pairs |
| `output.success` | Yes | Success output fields |
| `output.error` | Error type | Error structure |

### Example: File Read

```toml
[[functions]]
name = "read_file"
description = "Read entire file contents"
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

### Example: Math

```toml
[[functions]]
name = "sqrt"
description = "Compute square root"
location = "libm::sqrt"
target = "native"

[functions.input]
value = "Float"

[functions.output.success]
result = "Float"

[functions.output.error]
type = "MathError"
code = "Int"
message = "String"
```

### Void Success

For functions that return nothing:

```toml
[[functions]]
name = "print"
location = "std::io::print"
target = "native"

[functions.input]
msg = "String"

[functions.output.success]
# Empty - void return

[functions.output.error]
type = "IoError"
code = "Int"
message = "String"
```

---

## Brief Declarations

### Basic Syntax

```brief
frgn function_name(param: Type, ...) -> Result<SuccessType, ErrorType> from "path/to/bindings.toml";
```

### Examples

```brief
// Read a file
frgn read_file(path: String) -> Result<String, IoError> from "lib/std/io.toml";

// Print to console
frgn print(msg: String) -> Result<Void, IoError> from "lib/std/io.toml";

// Compute square root
frgn sqrt(value: Float) -> Result<Float, MathError> from "lib/std/math.toml";

// Get current time
frgn now() -> Result<Int, TimeError> from "lib/std/time.toml";
```

### Multi-Field Success Outputs

FFI functions can return multiple fields on success using tuple syntax:

**TOML:**
```toml
[[functions]]
name = "divide"
location = "lib::math::divide"
target = "native"

[functions.input]
a = "Int"
b = "Int"

[functions.output.success]
quotient = "Int"
remainder = "Int"

[functions.output.error]
type = "MathError"
code = "Int"
message = "String"
```

**Brief Declaration:**
```brief
frgn divide(a: Int, b: Int) -> Result<(quotient: Int, remainder: Int), MathError> from "lib/math.toml";

// Usage:
txn safe_divide [b != 0][result.quotient >= 0] {
    let (q, r) = divide(10, 3);
    term (q, r);
};
```

### Generic Functions

```brief
// Generic identity function
frgn<T> identity(value: T) -> Result<T, Error> from "lib/std/util.toml";
```

### Multiple Parameters

```brief
// Power function
frgn pow(base: Float, exponent: Float) -> Result<Float, MathError> from "lib/std/math.toml";

// String replace
frgn replace(text: String, pattern: String, replacement: String) -> Result<String, StringError> from "lib/std/string.toml";
```

---

## Error Handling

### Result Type

Every FFI function returns `Result<Success, Error>`:

```brief
frgn read_file(path: String) -> Result<String, IoError> from "lib/std/io.toml";
```

### Error Semantics

Foreign functions can ALWAYS return an error:

1. **Type mismatch is always an error**: If the foreign function returns a value that doesn't match the expected type, it's an error.
2. **Any value is potentially wrong**: The foreign language might return unexpected values.
3. **All FFI calls are fallible**: There is no "infallible" FFI call.

### Compiler Enforcement

Any `defn` that calls a `frgn` MUST handle the error case. The compiler will reject code that ignores potential errors:

```brief
// WRONG - This will not compile
frgn get_number() -> Result<Int, Error> from "lib/math.toml";

defn bad_example() -> Int [true][result >= 0] {
    term get_number();  // ERROR: frgn can return error, must handle it
};
```

### Handling Results

Any `defn` that calls a `frgn` MUST handle the error case:

```brief
// CORRECT - Handle both success and error
defn safe_get_number() -> Int [true][result >= 0] {
    let result = get_number();
    [result.is_ok()] {
        term result.unwrap();
    };
    term 0;  // Default value on error
};

// Or with escape for error propagation:
defn strict_get_number() -> Int [true][result >= 0] {
    let result = get_number();
    [result.is_err()] {
        escape;  // Propagate error
    };
    term result.unwrap();
};
```

### Error Propagation

FFI errors propagate up the call stack. If a transaction calls a defn that calls a frgn, and the frgn errors, the error bubbles up through the transaction.

### Error Fields

Errors have fields you can access:

```toml
[functions.output.error]
type = "IoError"
code = "Int"
message = "String"
```

```brief
// Error fields: code: Int, message: String
```

### Result Projection Methods

Result types support these methods for accessing success/error values:

| Method | Returns | Description |
|--------|---------|-------------|
| `.is_ok()` | `Bool` | True if the call succeeded |
| `.is_err()` | `Bool` | True if the call failed |
| `.value` | `T` | The success value (access only after checking `.is_ok()`) |
| `.error.code` | `E.code` | The error code field |
| `.error.message` | `E.message` | The error message field |

**Example:**
```brief
defn read_config(path: String) -> String [true][result.len() >= 0] {
    let result = read_file(path);
    if result.is_ok() {
        term result.value;
    } else {
        let err_code = result.error.code;
        let err_msg = result.error.message;
        // Log error, return default
        term "default_config";
    }
};
```

---

## Creating Custom Bindings

### Step 1: Create TOML File

Create `my_project/lib/my_bindings.toml`:

```toml
[[functions]]
name = "my_function"
description = "My custom function"
location = "my_crate::my_function"
target = "native"

[functions.input]
input1 = "String"
input2 = "Int"

[functions.output.success]
result = "String"

[functions.output.error]
type = "MyError"
code = "Int"
message = "String"
```

### Step 2: Declare in Brief

```brief
frgn my_function(input1: String, input2: Int) -> Result<String, MyError> from "my_project/lib/my_bindings.toml";
```

### Step 3: Implement in Rust

In your Rust crate:

```rust
pub fn my_function(input1: &str, input2: i64) -> Result<String, String> {
    // Your implementation
    Ok(format!("{} - {}", input1, input2))
}
```

### Step 4: Wire It Together

The TOML `location` field points to your Rust function. The mapper system handles type translation.

---

## Type System

### Supported Types

| Brief Type | Rust Type | Description |
|------------|-----------|-------------|
| `String` | `&str` / `String` | Text |
| `Int` | `i64` | 64-bit integer |
| `Float` | `f64` | 64-bit float |
| `Bool` | `bool` | Boolean |
| `Void` | `()` | No return value |
| Custom | Struct | User-defined |

### Type Mapping

The mapper system handles conversion:

```toml
# Brief String <-> Rust String
# Brief Int <-> Rust i64
# Brief Float <-> Rust f64
# Brief Bool <-> Rust bool
```

### Custom Types

```toml
[functions.input]
point = "Point"  # Custom struct

[functions.output.success]
point = "Point"
```

---

## Examples

### Example 1: File I/O

**TOML (`lib/std/io.toml`):**
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

**Brief:**
```brief
frgn read_file(path: String) -> Result<String, IoError> from "lib/std/io.toml";

defn load_config() -> String [true][result.len() >= 0] {
    let result = read_file("config.txt");
    [result.is_ok()] {
        term result.unwrap();
    };
    term "default content";  // Return default on error
};
```

### Example 2: Math Operations

**TOML (`lib/std/math.toml`):**
```toml
[[functions]]
name = "sqrt"
location = "libm::sqrt"
target = "native"

[functions.input]
value = "Float"

[functions.output.success]
result = "Float"

[functions.output.error]
type = "MathError"
code = "Int"
message = "String"

[[functions]]
name = "pow"
location = "libm::powf"
target = "native"

[functions.input]
base = "Float"
exponent = "Float"

[functions.output.success]
result = "Float"

[functions.output.error]
type = "MathError"
code = "Int"
message = "String"
```

**Brief:**
```brief
frgn sqrt(value: Float) -> Result<Float, MathError> from "lib/std/math.toml";
frgn pow(base: Float, exp: Float) -> Result<Float, MathError> from "lib/std/math.toml";

defn hypot(a: Float, b: Float) -> Float [true][result >= 0.0] {
    let a_sq = a * a;
    let b_sq = b * b;
    let sum = a_sq + b_sq;
    let sqrt_result = sqrt(sum);
    [sqrt_result.is_err()] {
        escape;
    };
    term sqrt_result.unwrap();
};
```

### Example 3: Console I/O

**TOML:**
```toml
[[functions]]
name = "println"
location = "std::println"
target = "native"

[functions.input]
msg = "String"

[functions.output.success]

[functions.output.error]
type = "IoError"
code = "Int"
message = "String"
```

**Brief:**
```brief
frgn println(msg: String) -> Result<Void, IoError> from "lib/std/io.toml";

defn greet(name: String) -> Bool [true][true] {
    let greeting = "Hello, " ++ name ++ "!";
    let result = println(greeting);
    [result.is_err()] {
        term false;  // Return false on error
    };
    term true;
};
```

### Example 4: Time Operations

**TOML:**
```toml
[[functions]]
name = "sleep_ms"
location = "std::thread::sleep"
target = "native"

[functions.input]
milliseconds = "Int"

[functions.output.success]

[functions.output.error]
type = "TimeError"
code = "Int"
message = "String"
```

**Brief:**
```brief
frgn sleep_ms(ms: Int) -> Result<Void, TimeError> from "lib/std/time.toml";

txn delayed_action [true][true] {
    let result = sleep_ms(1000);  // Sleep 1 second
    [result.is_err()] {
        escape;  // Abort on error
    };
    term;
};
```

---

## Mappers

### What Are Mappers

Mappers handle type translation between Brief and foreign languages. They are:
- Optional for simple cases
- Useful for language-specific quirks
- Transparent for basic FFI

### Default Mapper

The default mapper is `rust`:
```toml
mapper = "rust"  # Default, can be omitted
```

### When You Might Need a Custom Mapper

- C strings require null-termination handling
- WASM memory management
- Python object conversion

For most cases, the default mapper works fine.

---

## Troubleshooting

### "Binding file not found"

Check the path is correct:
```brief
from "lib/std/io.toml"  // Relative to project root
from "/absolute/path.toml"  // Absolute path
```

### "Type mismatch"

Verify your Brief declaration matches TOML:
```brief
// TOML says: input.path = "String"
frgn read_file(path: String) ...  // Must be String
```

### "Function not found"

Check the function name in TOML matches Brief:
```toml
name = "my_func"  // Brief must say: frgn my_func
```
