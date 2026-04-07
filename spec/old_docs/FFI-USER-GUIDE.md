# Brief FFI User Guide v6.2

## Overview

The Foreign Function Interface (FFI) system in Brief v6.2 enables seamless integration with external libraries while maintaining formal verification guarantees. This guide walks you through creating and using FFI bindings.

## Table of Contents

1. [Getting Started](#getting-started)
2. [Creating Bindings](#creating-bindings)
3. [Using Bindings in Brief](#using-bindings-in-brief)
4. [Type System](#type-system)
5. [Error Handling](#error-handling)
6. [Best Practices](#best-practices)
7. [Examples](#examples)
8. [Troubleshooting](#troubleshooting)

---

## Getting Started

### What is FFI?

The Foreign Function Interface (FFI) allows Brief programs to call external functions written in other languages (primarily Rust). Instead of rewriting common functionality, you can leverage existing libraries.

### Key Principles

1. **TOML is the Contract**: All FFI metadata is explicitly declared in TOML files
2. **Brief Wraps Everything**: Brief functions handle error cases, `frgn` is just a declaration
3. **Type Safe**: Only specific types are allowed through the FFI boundary
4. **Formally Verifiable**: Error handling is trackable for proof verification

### Basic Workflow

```
┌─────────────────────────────────────────────────────────────┐
│ 1. Create TOML Binding File                                 │
│    (Declare what functions are available)                   │
└─────────────────────────────────────────────────────────────┘
                          ↓
┌─────────────────────────────────────────────────────────────┐
│ 2. Write FFI Declaration in Brief                           │
│    (Use `frgn` keyword to declare the function signature)   │
└─────────────────────────────────────────────────────────────┘
                          ↓
┌─────────────────────────────────────────────────────────────┐
│ 3. Type Checker Validates                                   │
│    (Verifies signature matches TOML binding)                │
└─────────────────────────────────────────────────────────────┘
                          ↓
┌─────────────────────────────────────────────────────────────┐
│ 4. Use in Brief Code                                        │
│    (Call the function just like any other)                 │
└─────────────────────────────────────────────────────────────┘
```

---

## Creating Bindings

### TOML Binding File Structure

A TOML binding file declares which external functions are available. Here's the format:

```toml
[[functions]]
name = "function_name"                    # Required: Brief name for the function
location = "module::path::to::function"   # Required: Rust module path
target = "native"                         # Required: "native" (WASM support coming)
description = "What this function does"   # Optional: Human-readable description

[functions.input]
param1 = "String"                         # Input parameter name and type
param2 = "Int"

[functions.output.success]
result = "String"                         # Success output fields (can be multiple)

[functions.output.error]
type = "MyError"                          # Required: Error type name
code = "Int"                              # Required: Error code field
message = "String"                        # Required: Error message field
# Additional error fields can be added...
```

### Complete Example

```toml
[[functions]]
name = "read_file"
location = "std::fs::read_to_string"
target = "native"
description = "Read entire file contents"

[functions.input]
path = "String"

[functions.output.success]
content = "String"

[functions.output.error]
type = "IoError"
code = "Int"
message = "String"


[[functions]]
name = "write_file"
location = "std::fs::write"
target = "native"
description = "Write to file"

[functions.input]
path = "String"
content = "String"

[functions.output.success]
# Can have empty success output (returns Void)

[functions.output.error]
type = "IoError"
code = "Int"
message = "String"
```

### Supported Types

Only the following Brief types can cross the FFI boundary:

| Brief Type | FFI Equivalent | Example |
|-----------|----------------|---------|
| `String`  | `&str`         | `"hello"` |
| `Int`     | `i64`          | `42` |
| `Float`   | `f64`          | `3.14` |
| `Bool`    | `bool`         | `true` |
| `Void`    | `()`           | (no value) |
| Custom    | Struct fields  | User-defined types |

**Not Supported**: Lists, nested types, generics

### Error Handling Pattern

Every FFI function returns `Result<SuccessOutputs, ErrorType>`:

- **Success Branch**: Contains the actual results
- **Error Branch**: Must have:
  - `type`: Name of the error struct
  - `code`: Error code (Int)
  - `message`: Error description (String)

Additional error fields are allowed but must be declared in the TOML binding.

---

## Using Bindings in Brief

### Step 1: Create TOML File

Save your binding file (e.g., `my_bindings.toml`):

```toml
[[functions]]
name = "get_time"
location = "std::time::SystemTime::now"
target = "native"

[functions.input]

[functions.output.success]
timestamp = "Int"

[functions.output.error]
type = "TimeError"
code = "Int"
message = "String"
```

### Step 2: Declare in Brief

Use the `frgn` keyword to declare the function:

```brief
frgn get_time() -> Result<Int, TimeError> from "my_bindings.toml";
```

**Syntax**:
```
frgn <name>(<param1>: <Type>, <param2>: <Type>) -> Result<SuccessType, ErrorType> from "<path>";
```

- `<name>`: Must match the binding name in TOML
- `<Type>`: Must be a supported FFI type
- `from "<path>"`: Path to TOML file (can be absolute, project-relative, or `std::`)

### Step 3: Use in Code

```brief
frgn read_file(path: String) -> Result<String, IoError> from "std::io";

let content: String = read_file("data.txt");
let data: String = content;
```

The type checker will verify that:
1. The signature matches the TOML binding
2. All types are valid FFI types
3. The TOML file exists and is valid

---

## Type System

### FFI Type Constraints

```brief
// ✓ Valid: Basic types
frgn calculate(x: Int, y: Int) -> Result<Int, MathError> from "math.toml";

// ✓ Valid: Mix of basic types
frgn process(name: String, count: Int, enabled: Bool) 
    -> Result<String, ProcessError> from "process.toml";

// ✓ Valid: Void inputs/outputs
frgn current_time() -> Result<Int, TimeError> from "time.toml";  // No inputs
frgn log_message(msg: String) -> Result<Void, LogError> from "log.toml";  // Void output

// ✗ Invalid: Complex types not supported
frgn process_list(items: List<Int>) -> Result<String, Error> from "...";  // List not supported
frgn process_opt(val: Option<String>) -> Result<Bool, Error> from "...";  // Option not supported
```

### Success Output Types

The success type in `Result<T, E>` can be:

```brief
// Single output
frgn read() -> Result<String, Error> from "...";

// Multiple outputs require binding with multiple success fields
frgn process() -> Result<String, Error> from "...";
// TOML has: result1 = "String", result2 = "Int"  ← Requires multiple output declarations
```

**Note**: Currently, multiple success outputs must be declared in the TOML binding, but the Brief signature shows just one representative type.

---

## Error Handling

### Error Contracts

Every FFI call returns a `Result` that must be handled:

```brief
frgn read_file(path: String) -> Result<String, IoError> from "std::io";

// The defn must handle both Success and Error cases:
defn load_config(path: String) -> String [true] [true] {
    let content: String = read_file(path);  // Type is Result<String, IoError>
    
    // Handle success case
    // Handle error case
    // Return String
};
```

### Error Fields

Error types in Brief define the structure of errors:

```brief
// In your Brief code, you'd define:
struct IoError {
    code: Int;
    message: String;
};

// When handling errors:
frgn read_file(path: String) -> Result<String, IoError> from "std::io";

defn safe_read(path: String) -> String [true] [true] {
    let result: Result<String, IoError> = read_file(path);
    // result contains: { code: Int, message: String } on error
    result;
};
```

---

## Best Practices

### 1. Organize Bindings by Domain

Create separate TOML files for logical groupings:

```
std/bindings/
├── io.toml           # File I/O operations
├── math.toml         # Mathematical functions
├── string.toml       # String manipulation
├── time.toml         # Time and timing
└── myapp/
    ├── db.toml       # Database operations
    └── network.toml  # Network operations
```

### 2. Name Bindings Consistently

Use descriptive names that clarify what the function does:

```toml
# Good
name = "read_file_contents"
name = "convert_string_to_int"
name = "calculate_distance"

# Avoid
name = "f1"
name = "do_thing"
name = "x"
```

### 3. Document Error Types

Clearly define what errors can occur:

```toml
# Good documentation
description = "Read entire file; fails if file doesn't exist or permission denied"

[functions.output.error]
type = "FileError"
code = "Int"  # 1=NotFound, 2=PermissionDenied, 3=Other
message = "String"
```

### 4. Limit Function Scope

Create focused bindings for specific use cases:

```toml
# Good: Specific, testable
[[functions]]
name = "parse_json"
location = "serde_json::from_str"

# Avoid: Too general
[[functions]]
name = "do_everything"
location = "my_mega_function"
```

### 5. Prefer Void Inputs When Possible

Functions with no inputs are simpler to reason about:

```toml
# Good: Simple, deterministic
[[functions]]
name = "current_timestamp"
location = "std::time::SystemTime::now"
[functions.input]  # Empty

# Less ideal: Many parameters increase testing burden
[[functions]]
name = "complex_calculation"
location = "calc::with_many_params"
[functions.input]
a = "Int"
b = "Int"
c = "Int"
d = "Int"
# ... many more parameters
```

---

## Examples

This section provides 6 progressively advanced examples showing different FFI patterns.

### Example 1: File Operations

**Binding File** (`io.toml`):
```toml
[[functions]]
name = "read_file"
location = "std::fs::read_to_string"
target = "native"
description = "Read file contents"

[functions.input]
path = "String"

[functions.output.success]
content = "String"

[functions.output.error]
type = "IoError"
code = "Int"
message = "String"
```

**Brief Code**:
```brief
frgn read_file(path: String) -> Result<String, IoError> from "io.toml";

defn process_file(filename: String) -> String [true] [true] {
    let data: String = read_file(filename);
    data;
};
```

### Example 2: Math Operations

**Binding File** (`math.toml`):
```toml
[[functions]]
name = "sqrt"
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
```

**Brief Code**:
```brief
frgn sqrt(value: Float) -> Result<Float, MathError> from "math.toml";

defn hypotenuse(a: Float, b: Float) -> Float [true] [true] {
    let a_sq: Float = a * a;
    let b_sq: Float = b * b;
    let sum: Float = a_sq + b_sq;
    let result: Float = sqrt(sum);
    result;
};
```

### Example 3: Multiple Functions

**Binding File** (`strings.toml`):
```toml
[[functions]]
name = "string_to_uppercase"
location = "str::to_uppercase"
target = "native"

[functions.input]
text = "String"

[functions.output.success]
result = "String"

[functions.output.error]
type = "StringError"
code = "Int"
message = "String"


[[functions]]
name = "string_length"
location = "str::len"
target = "native"

[functions.input]
text = "String"

[functions.output.success]
length = "Int"

[functions.output.error]
type = "StringError"
code = "Int"
message = "String"
```

**Brief Code**:
```brief
frgn string_to_uppercase(text: String) -> Result<String, StringError> from "strings.toml";
frgn string_length(text: String) -> Result<Int, StringError> from "strings.toml";

defn process_text(input: String) -> Int [true] [true] {
    let upper: String = string_to_uppercase(input);
    let len: Int = string_length(upper);
    len;
};
```

### Example 4: Error Handling Patterns

**Binding File** (`network.toml`):
```toml
[[functions]]
name = "send_request"
location = "http::send_request"
target = "native"
description = "Send HTTP request and get response"

[functions.input]
url = "String"
method = "String"

[functions.output.success]
status = "Int"
body = "String"

[functions.output.error]
type = "NetworkError"
code = "Int"
message = "String"
```

**Brief Code** (showing different error handling patterns):
```brief
frgn send_request(url: String, method: String) -> Result<{status: Int, body: String}, NetworkError> from "network.toml";

# Pattern 1: Unwrap (fails if error occurs)
defn fetch_with_unwrap(url: String) -> String [true] [true] {
    let response: {status: Int, body: String} = send_request(url, "GET");
    response.body;
};

# Pattern 2: Handle error explicitly
defn fetch_with_error_handling(url: String) -> String [true] [true] {
    let result: Result<{status: Int, body: String}, NetworkError> = send_request(url, "GET");
    [result is success] {
        let response: {status: Int, body: String} = result.success;
        response.body;
    };
    [result is error] {
        let error: NetworkError = result.error;
        "Error: " ++ error.message;
    };
    term "Unknown result";
};

# Pattern 3: Use in transaction with guards
let request_count: Int = 0;
let last_error: String = "";

rct txn track_request
    [request_count < 100]
    [request_count == @request_count + 1]
{
    let response: Result<{status: Int, body: String}, NetworkError> = send_request("https://api.example.com", "GET");
    [response is success] {
        &request_count = request_count + 1;
        &last_error = "";
    };
    [response is error] {
        let error: NetworkError = response.error;
        &last_error = error.message;
    };
    term;
};
```

### Example 5: Custom Binding Module

**Binding File** (`payment.toml`):
```toml
# Payment processing with multiple related functions

[[functions]]
name = "validate_card"
location = "payment::validate_card_number"
target = "native"
description = "Validate credit card format"

[functions.input]
card_number = "String"

[functions.output.success]
valid = "Bool"

[functions.output.error]
type = "PaymentError"
code = "Int"
message = "String"


[[functions]]
name = "process_payment"
location = "payment::charge_card"
target = "native"
description = "Process payment charge"

[functions.input]
card_number = "String"
amount = "Int"

[functions.output.success]
transaction_id = "String"
amount_charged = "Int"

[functions.output.error]
type = "PaymentError"
code = "Int"
message = "String"


[[functions]]
name = "refund_payment"
location = "payment::refund_transaction"
target = "native"
description = "Refund a previous transaction"

[functions.input]
transaction_id = "String"

[functions.output.success]
refunded_amount = "Int"

[functions.output.error]
type = "PaymentError"
code = "Int"
message = "String"
```

**Brief Code**:
```brief
frgn validate_card(card_number: String) -> Result<Bool, PaymentError> from "payment.toml";
frgn process_payment(card_number: String, amount: Int) -> Result<{transaction_id: String, amount_charged: Int}, PaymentError> from "payment.toml";
frgn refund_payment(transaction_id: String) -> Result<Int, PaymentError> from "payment.toml";

let last_transaction_id: String = "";
let processing_payment: Bool = false;
let payment_error_count: Int = 0;

defn charge_customer(card: String, amount: Int) -> String [true] [true] {
    let validation: Result<Bool, PaymentError> = validate_card(card);
    [validation is error] {
        let err: PaymentError = validation.error;
        term "Card validation failed: " ++ err.message;
    };
    let result: Result<{transaction_id: String, amount_charged: Int}, PaymentError> = process_payment(card, amount);
    [result is success] {
        let tx: {transaction_id: String, amount_charged: Int} = result.success;
        term "Payment processed: " ++ tx.transaction_id;
    };
    [result is error] {
        let err: PaymentError = result.error;
        term "Payment failed: " ++ err.message;
    };
    term "Unknown error";
};

# Reactive workflow: Monitor payment processing
rct txn process_payment_workflow
    [processing_payment == false && last_transaction_id == ""]
    [processing_payment == false && last_transaction_id != ""]
{
    &processing_payment = true;
    term;
};

rct txn complete_payment
    [processing_payment == true && last_transaction_id != ""]
    [processing_payment == false]
{
    &processing_payment = false;
    term;
};
```

### Example 6: State Machine with FFI Transitions

**Binding File** (`database.toml`):
```toml
[[functions]]
name = "save_record"
location = "db::insert_record"
target = "native"

[functions.input]
key = "String"
value = "String"

[functions.output.success]
record_id = "Int"

[functions.output.error]
type = "DbError"
code = "Int"
message = "String"


[[functions]]
name = "get_record"
location = "db::select_record"
target = "native"

[functions.input]
record_id = "Int"

[functions.output.success]
value = "String"

[functions.output.error]
type = "DbError"
code = "Int"
message = "String"
```

**Brief Code** (state machine with persistence):
```brief
frgn save_record(key: String, value: String) -> Result<Int, DbError> from "database.toml";
frgn get_record(record_id: Int) -> Result<String, DbError> from "database.toml";

let state: Int = 0;        # 0=init, 1=saving, 2=saved, 3=error
let record_id: Int = -1;
let error_msg: String = "";

# State 0→1: Initiate save
rct txn initiate_save
    [state == 0 && record_id == -1]
    [state == 1]
{
    &state = 1;
    term;
};

# State 1→2: Perform actual save via FFI
rct txn perform_save
    [state == 1 && record_id == -1]
    [state == 2 || state == 3]
{
    let result: Result<Int, DbError> = save_record("user_data", "test_value");
    [result is success] {
        let id: Int = result.success;
        &record_id = id;
        &state = 2;
    };
    [result is error] {
        let err: DbError = result.error;
        &error_msg = err.message;
        &state = 3;
    };
    term;
};

# State 2→... : Retrieve saved data
defn retrieve_data(id: Int) -> String [true] [true] {
    let result: Result<String, DbError> = get_record(id);
    [result is success] {
        let value: String = result.success;
        term value;
    };
    [result is error] {
        let err: DbError = result.error;
        term "Error: " ++ err.message;
    };
    term "Unknown";
};
```

---

## Troubleshooting

### Issue: "Binding file not found"

**Problem**: Type checker can't locate your TOML binding file.

**Solutions**:
1. Check the path is correct and file exists
2. Use absolute paths for certainty: `/home/user/project/bindings.toml`
3. For stdlib bindings, use `std::module_name` format
4. For project-relative paths, paths are relative to project root

```brief
// These all work:
frgn func() -> Result<String, Error> from "/absolute/path/bindings.toml";
frgn func() -> Result<String, Error> from "relative/path/bindings.toml";
frgn func() -> Result<String, Error> from "std::io";
```

### Issue: "Binding validation failed"

**Problem**: Your FFI signature doesn't match the TOML binding.

**Common causes**:
1. Function name mismatch: `frgn read_file` but binding has `read_file_contents`
2. Parameter count mismatch: signature has 2 params but binding has 3
3. Type mismatch: signature says `Int` but binding expects `String`
4. Error type mismatch: signature uses `IoError` but binding uses `FileError`

**Solution**: Verify each part of your signature:
```brief
// Check this...          against this in TOML...
frgn read_file           # name = "read_file"
    (path: String)       # [functions.input]: path = "String"
    -> Result<String,    # [functions.output.success]: result = "String"
              IoError>   # [functions.output.error]: type = "IoError"
    from "io.toml";
```

### Issue: "Invalid FFI type"

**Problem**: You used a type that's not supported across the FFI boundary.

**Supported types only**: `String`, `Int`, `Float`, `Bool`, `Void`, custom structs

**Not supported**: 
- `List<T>` - use `String` and parse
- `Option<T>` - use error handling
- `Result<T, E>` - only in the outer FFI type
- Generics - use concrete types

```brief
// ✗ Invalid
frgn process(items: List<Int>) -> Result<String, Error> from "...";

// ✓ Valid workaround - serialize to string
frgn process(items_json: String) -> Result<String, Error> from "...";
```

### Issue: "Type checker passed but runtime error"

**Problem**: The TOML binding location is incorrect or points to wrong module.

**Solution**: Verify the Rust module path:
```toml
# Wrong - module doesn't exist this way
location = "file::read_to_string"

# Right - standard library path
location = "std::fs::read_to_string"

# Right - custom crate
location = "my_crate::utils::read_file"
```

### Issue: Multiple bindings, which one is used?

**Problem**: You have the same function declared with different binding files.

**Solution**: Only one binding per frgn declaration is used. The path specified in the `from` clause determines which binding is loaded.

```brief
// Each uses its own binding
frgn read_v1() -> Result<String, Error> from "io_v1.toml";
frgn read_v2() -> Result<String, Error> from "io_v2.toml";  // Different file = different binding
```

---

## Next Steps

- **Read Existing Bindings**: Check `std/bindings/` for well-formed examples
- **Review Your Use Case**: Does Brief FFI make sense for your needs?
- **Start Small**: Create one simple binding first, then expand
- **Reference**: See FFI-STDLIB-REFERENCE.md for comprehensive stdlib binding docs
- **Testing**: Write tests to verify your FFI integration works correctly

---

## Support & Feedback

For issues, feature requests, or questions:
- GitHub Issues: https://github.com/anomalyco/brief-compiler/issues
- Discussions: https://github.com/anomalyco/brief-compiler/discussions

---

**Version**: 6.2  
**Last Updated**: 2026-04-05  
**Status**: Production Ready
