# Brief FFI Guide

**Version:** 8.0  
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
9. [Creating Mappers](#creating-mappers)

---

## Creating Mappers

For detailed instructions on creating language-specific FFI mappers, see [MAPPER-GUIDE.md](MAPPER-GUIDE.md).

Brief supports mappers for:
- **Rust** - 1:1 identity mapping (native)
- **C** - Null-terminated strings, pointer handling
- **WebAssembly** - Linear memory, JS value conversion
- **Python** - Via CPython (placeholder)
- **...and more** - Create your own using the template!


## Overview

FFI (Foreign Function Interface) allows Brief to call external functions written in other languages (primarily Rust). Brief cannot do everything - file I/O, networking, hardware math - so FFI provides a bridge.

### Design Principles

1. **TOML is the Contract**: All FFI metadata is declared in TOML files
2. **Logically Closed**: Foreign code is wrapped so errors automatically propagate
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
    |
    v
Result<T, Error>  -->  Value extracted, errors cause escape
```

---

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

TOML binding files go in `std/bindings/` or a custom location:

```
std/bindings/
├── io.toml
├── math.toml
├── string.toml
└── time.toml

my_project/lib/
└── my_bindings.toml
```

### Library Metadata

Each binding file should include a `[meta]` section:

```toml
[meta]
name = "io"
version = "1.0.0"

[[functions]]
...
```

### Basic Structure

```toml
[meta]
name = "io"
version = "1.0.0"

[[functions]]
name = "function_name"
description = "What this function does"
location = "module::path::to::function"
target = "native"

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
| `meta.name` | Yes | Library name |
| `meta.version` | No | Version string |
| `name` | Yes | Brief function name |
| `description` | No | Human-readable description |
| `location` | Yes | Rust module path |
| `target` | Yes | Platform: `native`, `wasm` |
| `mapper` | No | Mapper name (default: `rust`) |
| `input` | Yes | Parameter name-type pairs |
| `output.success` | Yes | Success output fields |
| `output.error` | Yes | Error type and fields |

### Example: File Read

```toml
[meta]
name = "io"
version = "1.0.0"

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

### Example: Console Output

```toml
[meta]
name = "io"
version = "1.0.0"

[[functions]]
name = "println"
description = "Print to console with newline"
location = "std::io::println"
target = "native"

[functions.input]
text = "String"

[functions.output.success]
result = "void"

[functions.output.error]
type = "IoError"
code = "Int"
message = "String"
```

### Example: Math

```toml
[meta]
name = "math"
version = "1.0.0"

[[functions]]
name = "sqrt"
description = "Compute square root"
location = "std::f64::sqrt"
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
name = "sin"
description = "Compute sine"
location = "std::f64::sin"
target = "native"

[functions.input]
radians = "Float"

[functions.output.success]
result = "Float"

[functions.output.error]
type = "MathError"
code = "Int"
message = "String"
```

---

## Brief Declarations

### Basic Syntax

```brief
frgn function_name(param: Type, ...) -> Result<SuccessType, ErrorType> from "path/to/bindings.toml";
```

### Rules

1. **Name must match TOML**: The function name in Brief must match the `name` in TOML
2. **Error type must match TOML**: Use `IoError`, `MathError`, `StringError`, etc. as defined in TOML
3. **Result prefix convention**: Many developers use `__` prefix (e.g., `__sqrt`) as a convention to mark FFI functions

### Examples

```brief
// Print to console
frgn println(msg: String) -> Result<void, IoError> from "std/bindings/io.toml";

// Compute square root
frgn sqrt(value: Float) -> Result<Float, MathError> from "std/bindings/math.toml";

// Compute sine
frgn sin(radians: Float) -> Result<Float, MathError> from "std/bindings/math.toml";

// Get string length
frgn string_length(s: String) -> Result<Int, StringError> from "std/bindings/string.toml";

// Read file
frgn read_file(path: String) -> Result<String, IoError> from "std/bindings/io.toml";
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
```

---

## Error Handling

### Logically Closed Pattern

The key innovation in Brief's FFI is **logical closure**: foreign functions that return errors automatically cause the transaction to escape (rollback), making the system logically closed.

```brief
frgn sqrt(x: Float) -> Result<Float, MathError> from "std/bindings/math.toml";

txn calculate [true][result >= 0] {
    term sqrt(16.0);  // Returns 4.0, transaction succeeds
    // If sqrt returns error, transaction escapes automatically
};
```

### How It Works

1. **FFI returns Result**: The foreign function returns `(value: T, error: E)` 
2. **Empty error = success**: If error fields are empty/zero, the call succeeded
3. **Non-empty error = failure**: If any error field has data, the transaction escapes
4. **Value extraction**: For non-void functions, the value is automatically extracted

### Error Semantics

- **Empty error fields = success**: Zero Int, empty String, false Bool
- **Non-empty error fields = failure**: Any populated error field causes escape
- **Automatic handling**: No need for `.is_ok()`/`.is_err()` - it's built into the runtime

### Accessing Error Details (Optional)

When you need to handle errors explicitly:

```brief
frgn read_file(path: String) -> Result<String, IoError> from "std/bindings/io.toml";

txn safe_read [true][result != ""] {
    let result = read_file("config.txt");
    // If result has error, transaction escapes automatically
    // Otherwise, result contains the file contents
    term result;
};
```

### Void Functions

For functions that return nothing on success:

```brief
frgn println(msg: String) -> Result<void, IoError> from "std/bindings/io.toml";

txn test_print [true] {
    term println("Hello!");  // Returns void, escapes on error
};
```

---

## Creating Custom Bindings

For detailed information on creating language-specific mappers, see [MAPPER-GUIDE.md](MAPPER-GUIDE.md).

### Step 1: Create TOML File

Create `lib/my_bindings.toml`:

```toml
[meta]
name = "my_library"
version = "1.0.0"

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
frgn my_function(input1: String, input2: Int) -> Result<String, MyError> from "lib/my_bindings.toml";
```

### Step 3: Implement in Rust

In your Rust crate or the compiler:

```rust
pub fn my_function(input1: &str, input2: i64) -> Result<Value, RuntimeError> {
    // Your implementation returns a Struct with:
    // - success fields (result)
    // - error fields (code, message)
    Ok(Value::Struct(hashmap!(
        "result" => Value::String(format!("{} - {}", input1, input2)),
        "code" => Value::Int(0),
        "message" => Value::String(String::new()),
    )))
}
```

### Step 4: Register in Registry

Add the mapping in `src/ffi/registry.rs`:

```rust
fn resolve_location_to_impl(location: &str) -> Option<ForeignFn> {
    match location {
        "my_crate::my_function" => my_function_impl,
        // ... other mappings
    }
}
```

---

## Type System

### Supported Types

| Brief Type | Rust Type | Description |
|------------|-----------|-------------|
| `String` | `String` | Text |
| `Int` | `i64` | 64-bit integer |
| `Float` | `f64` | 64-bit float |
| `Bool` | `bool` | Boolean |
| `void` | `()` | No return value |
| Custom | Struct | User-defined |

### Type Mapping

The mapper system handles conversion. For detailed type mapping per language, see [MAPPER-GUIDE.md](MAPPER-GUIDE.md#type-conversions).

For native functions:

- Brief `String` ↔ Rust `String`
- Brief `Int` ↔ Rust `i64`
- Brief `Float` ↔ Rust `f64`
- Brief `Bool` ↔ Rust `bool`

---

## Examples

### Example 1: File I/O

**TOML (`std/bindings/io.toml`):**
```toml
[meta]
name = "io"
version = "1.0.0"

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
frgn read_file(path: String) -> Result<String, IoError> from "std/bindings/io.toml";

txn load_config [true][result != ""] {
    term read_file("config.txt");
};
```

### Example 2: Math Operations

**Brief:**
```brief
frgn sqrt(value: Float) -> Result<Float, MathError> from "std/bindings/math.toml";
frgn pow(base: Float, exp: Float) -> Result<Float, MathError> from "std/bindings/math.toml";
frgn sin(radians: Float) -> Result<Float, MathError> from "std/bindings/math.toml";

txn test_math [true][result == 4.0] {
    term sqrt(16.0);
};

txn test_pow [true][result == 8.0] {
    term pow(2.0, 3.0);
};

txn test_sin [true][result == 0.0] {
    term sin(0.0);
};
```

### Example 3: Console I/O

**Brief:**
```brief
frgn println(msg: String) -> Result<void, IoError> from "std/bindings/io.toml";

rct txn greet [true] {
    term println("Hello, World!");
};
```

### Example 4: String Operations

**Brief:**
```brief
frgn string_length(s: String) -> Result<Int, StringError> from "std/bindings/string.toml";
frgn string_concat(a: String, b: String) -> Result<String, StringError> from "std/bindings/string.toml";

txn test_len [result == 5] {
    term string_length("hello");
};

txn test_concat [result == "helloworld"] {
    term string_concat("hello", "world");
};
```

---

## Standard Library Bindings

Brief includes standard bindings in `std/bindings/`:

### io.toml
- `print` - Print without newline
- `println` - Print with newline  
- `input` - Read line from stdin
- `read_file` - Read file contents
- `write_file` - Write to file
- `create_dir` - Create directory
- `delete_file` - Delete file
- `delete_dir` - Delete directory

### math.toml
- `sqrt` - Square root
- `sin` - Sine
- `cos` - Cosine
- `pow` - Power (base^exp)
- `floor` - Floor
- `ceil` - Ceiling
- `round` - Round
- `abs` - Absolute value

### string.toml
- `string_length` - String length
- `string_concat` - Concatenate strings
- `string_contains` - Check substring
- `string_replace` - Replace pattern
- `string_to_upper` - Uppercase
- `string_to_lower` - Lowercase
- `string_trim` - Trim whitespace
- `parse_int` - Parse integer
- `parse_float` - Parse float

---

## Troubleshooting

### "Binding file not found"

Check the path is correct:
```brief
from "std/bindings/io.toml"  // Relative to std/
from "/absolute/path.toml"   // Absolute path
```

### "Function not found in binding"

Verify your function name matches TOML:
```toml
name = "my_func"  // Brief must say: frgn my_func
```

### "Error type name mismatch"

Make sure ErrorType matches TOML:
```brief
// TOML has: type = "MathError"
frgn sqrt(x: Float) -> Result<Float, MathError> ...  // Must use MathError
```

### "Function not found at runtime"

The function's location must be registered in the registry. Check that the TOML `location` field matches a registered implementation in `src/ffi/registry.rs`.