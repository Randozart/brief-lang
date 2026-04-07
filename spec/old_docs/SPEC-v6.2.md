# Brief Language Specification
**Version:** 6.2 Robust Foreign Function Interface (FFI) System  
**Date:** 2026-04-05  
**Status:** Authoritative Reference  
**Supersedes:** SPEC-v6.1 (FFI sections)

---

## 1. Introduction and Philosophy

**Brief** v6.2 extends v6.1 with a **Robust Foreign Function Interface (FFI)** system that enables seamless integration with external Rust libraries while maintaining Brief's core philosophy: **contracts first, verification always**.

### 1.1 FFI Design Principles

1. **TOML is the Contract** - All foreign function metadata (inputs, outputs, error types) is explicitly declared in TOML binding files. This becomes the ground truth for what foreign functions promise.

2. **JSON as the Bridge Language** - All FFI communication happens through JSON serialization. Foreign libraries emit JSON; Brief's FFI layer deserializes to Brief-native types. This eliminates language-specific coupling.

3. **Brief Wraps Everything** - Foreign functions are assumed untrusted. The defn (not the frgn) handles all failure modes and ensures contract compliance. Every foreign call is safely wrapped.

4. **Never Touch Source Again** - Once a TOML binding is written, the foreign library source code never needs modification. All plumbing happens in TOML + Brief.

5. **Platform Agnostic with Hooks** - The same TOML + Brief code works across targets (native Rust, WASM, future languages) by swapping target implementations. Basic hooks support future cross-language FFI.

6. **Full Generics Support** - FFI signatures support complete generic types, nested structures, and type parameters, enabling complex foreign integrations.

### 1.2 FFI Architecture Overview

```
┌─────────────────────────────────────────┐
│  Brief Application Code (defn + sig)    │
│  - Handles all cases                    │
│  - Enforces contracts                   │
│  - Never directly calls foreign code    │
└──────────────┬──────────────────────────┘
               │
               ↓
┌─────────────────────────────────────────┐
│  frgn Gateway (typed foreign function)  │
│  - Minimal, declaration-only            │
│  - Points to TOML binding               │
│  - Transparent to Brief logic           │
└──────────────┬──────────────────────────┘
               │
               ↓
┌─────────────────────────────────────────┐
│  Brief FFI Layer (JSON ↔ Brief types)   │
│  - Loads TOML binding                   │
│  - Calls foreign function               │
│  - Deserializes JSON result             │
│  - Wraps in Result<T, Error>            │
└──────────────┬──────────────────────────┘
               │
               ↓
┌─────────────────────────────────────────┐
│  Foreign Library (Rust/C/other)         │
│  - Returns JSON: success or error       │
│  - Never knows about Brief              │
│  - Responsible for own correctness      │
└─────────────────────────────────────────┘
```

---

## 2. FFI Syntax and Declarations

### 2.1 Foreign Function Declaration (frgn)

**Syntax:**
```brief
frgn identifier ( parameters? ) -> result_type from "binding_file.toml" ;
```

**Examples:**

```brief
// Simple file read
frgn read_file(path: String) -> Result<String, IoError> from "std/bindings/io.toml";

// File write (void return)
frgn write_file(path: String, content: String) -> Result<void, FileError> from "std/bindings/io.toml";

// Generic type parameters
frgn process<T, U>(input: T, transformer: String) -> Result<U, ProcessError> from "custom/bindings.toml";

// Complex nested types
frgn query(filter: { field: String, value: Int }) -> Result<[{ id: Int, name: String }], DbError> from "std/bindings/db.toml";
```

**Semantics:**
- `identifier` must match the `name` field in the TOML binding file
- `parameters` are Brief-native types; TOML declares type mappings to foreign language
- `result_type` must be `Result<SuccessType, ErrorType>` or shorthand `T` (auto-wrapped as `Result<T, GenericError>`)
- `"binding_file.toml"` can be:
  - Absolute path: `/path/to/bindings.toml`
  - Relative to project: `./bindings/custom.toml` or `bindings/custom.toml`
  - Stdlib: `std/bindings/io.toml` (resolved via stdlib path)
- Generic type parameters `<T, U>` are supported for complex use cases

### 2.2 TOML Binding Format (Complete Schema)

**Location:** `std/bindings/*.toml` or `./bindings/*.toml`

**Complete Example:**
```toml
# std/bindings/io.toml
# Explicit binding contract between Brief and Rust standard library

[[functions]]
name = "read_file"
description = "Read entire file to string. Fails on permission denied or file not found."
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


[[functions]]
name = "write_file"
description = "Write content to file. Fails on permission denied."
location = "std::fs::write"
target = "native"

[functions.input]
path = "String"
content = "String"

[functions.output.success]
# Void/unit return (empty section)

[functions.output.error]
type = "FileError"
code = "Int"
message = "String"
path = "String"


[[functions]]
name = "exists"
description = "Check if file exists. Never fails."
location = "std::path::Path::exists"
target = "native"

[functions.input]
path = "String"

[functions.output.success]
exists = "Bool"

[functions.output.error]
type = "void"  # No error possible
```

**Schema Rules:**

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | String | Yes | Must match `frgn` declaration in Brief code |
| `description` | String | No | Human-readable documentation of function behavior |
| `location` | String | Yes | Foreign language module path (e.g., `std::fs::read_to_string`) |
| `target` | String | Yes | Platform: `"native"` (Rust FFI) or future targets |
| `input` | Table | Yes | TOML table mapping parameter names to Brief types |
| `output.success` | Table | Yes | Named fields describing success type; empty for void |
| `output.error` | Table | Yes | Error type with `type` name + fields |

**Type Mapping:**

Brief types in TOML maps to:
- `String` → Rust `String`
- `Int` → Rust `i64`
- `Float` → Rust `f64`
- `Bool` → Rust `bool`
- Struct types: `{ field1: Type, field2: Type }` → Rust struct
- Arrays/Lists: `[Type]` → Rust `Vec<T>`
- Result types: Auto-wrapped by FFI layer

**JSON Wire Format:**

Foreign functions return JSON. Success example:
```json
{
  "success": {
    "content": "file contents here"
  }
}
```

Error example:
```json
{
  "error": {
    "code": 2,
    "message": "No such file or directory",
    "path": "/nonexistent/file.txt"
  }
}
```

### 2.3 Integration with Brief Grammar

**Updated BNF:**
```bnf
program ::= (signature | definition | foreign_binding | state_decl | constant | transaction | rct_transaction)*

foreign_binding ::= "frgn" identifier "(" parameters? ")" "->" result_type "from" string_literal ";"

# frgn declarations are TOP-LEVEL (not inside defn/txn)
# Must come before any calls to that function
```

**Usage Pattern:**
```brief
// 1. Declare the foreign function binding
frgn read_file(path: String) -> Result<String, IoError> from "std/bindings/io.toml";

// 2. Optionally create a safe wrapper defn
defn safe_read(path: String) [path.len() > 0] [result == Success | result == DefaultContent] -> Result<String, String> {
    let raw = frgn read_file(path);
    
    [raw is Success(content)] term Success(content);
    [raw is IoError(err)] term DefaultContent("Could not read file");
};

// 3. Use in transactions/other defns
txn load_config [true][config_loaded] {
    let result = safe_read("config.toml");
    
    [result is Success(content)] &config = content;
    [result is DefaultContent(default)] &config = default;
    
    term;
};
```

---

## 3. Error Handling Patterns

### 3.1 Basic Pattern: Reactive Error Handling

```brief
// frgn returns Result<String, IoError>
// defn catches IoError and provides fallback

defn get_file_content(path: String) [true] [result != ""] -> String {
    let raw = frgn read_file(path);
    
    [raw is Success(content)] term content;
    [raw is IoError(err)] term "default content";
};
```

### 3.2 Advanced Pattern: Error Inspection

```brief
// Pattern match on error fields to decide handling

defn smart_read(path: String) [true] [result != ""] -> String {
    let raw = frgn read_file(path);
    
    [raw is Success(content)] term content;
    [raw is IoError(err)] {
        [err.code == 2] term "File not found";  // ENOENT
        [err.code == 13] term "Permission denied";  // EACCES
        [true] term "Unknown error: " + err.message;
    };
};
```

### 3.3 Propagation Pattern: Pass Error Up

```brief
// Instead of handling, pass error to caller via union type

defn read_with_error(path: String) [true] [true] -> String | IoError {
    let raw = frgn read_file(path);
    
    [raw is Success(content)] term content;
    [raw is IoError(err)] term err;
};

txn process_file [true] [result] {
    let result = read_with_error("input.txt");
    
    [result is String(content)] {
        // Process content
        term;
    };
    [result is IoError(err)] {
        // Log error, don't crash
        term;
    };
};
```

### 3.4 Fallback Pattern: Nested Attempt

```brief
// Try primary source, fall back to secondary

defn get_data_with_fallback(primary: String, secondary: String) [true] [result != ""] -> String {
    let primary_result = frgn read_file(primary);
    
    [primary_result is Success(content)] term content;
    [primary_result is IoError(_)] {
        let fallback_result = frgn read_file(secondary);
        
        [fallback_result is Success(content)] term content;
        [fallback_result is IoError(_)] term "All sources failed";
    };
};
```

---

## 4. Standard Library Bindings (stdlib)

All stdlib bindings are bundled in source under `std/bindings/`.

### 4.1 io.toml - File I/O Operations

Provides: `read_file`, `write_file`, `file_exists`

```brief
frgn read_file(path: String) -> Result<String, IoError> from "std/bindings/io.toml";
frgn write_file(path: String, content: String) -> Result<void, FileError> from "std/bindings/io.toml";
frgn file_exists(path: String) -> Result<Bool, void> from "std/bindings/io.toml";
```

**Error Types:**
- `IoError`: `{ code: Int, message: String }`
- `FileError`: `{ code: Int, message: String, path: String }`

### 4.2 time.toml - Time Operations

Provides: `now`, `sleep`, `duration_ms`

```brief
frgn now() -> Result<Int, void> from "std/bindings/time.toml";
frgn sleep(ms: Int) -> Result<void, TimeError> from "std/bindings/time.toml";
```

**Error Types:**
- `TimeError`: `{ code: Int, message: String }`

### 4.3 math.toml - Mathematical Operations

Provides: `sqrt`, `sin`, `cos`, `abs`, `pow`

```brief
frgn sqrt(n: Float) -> Result<Float, MathError> from "std/bindings/math.toml";
frgn sin(angle: Float) -> Result<Float, void> from "std/bindings/math.toml";
```

**Error Types:**
- `MathError`: `{ code: Int, message: String }`

### 4.4 string.toml - String Manipulation

Provides: `concat`, `split`, `trim`, `uppercase`, `lowercase`

```brief
frgn concat(a: String, b: String) -> Result<String, void> from "std/bindings/string.toml";
frgn split(s: String, delimiter: String) -> Result<[String], StringError> from "std/bindings/string.toml";
```

**Error Types:**
- `StringError`: `{ code: Int, message: String }`

---

## 5. Writing Custom FFI Bindings

### 5.1 User Guide: Creating Your Own Binding

**Step 1: Create TOML File** (`bindings/custom.toml`)
```toml
[[functions]]
name = "my_function"
description = "What this does"
location = "my_crate::module::my_function"
target = "native"

[functions.input]
param1 = "String"
param2 = "Int"

[functions.output.success]
result = "String"

[functions.output.error]
type = "CustomError"
code = "Int"
message = "String"
```

**Step 2: Declare in Brief** (any `.bv` file)
```brief
frgn my_function(param1: String, param2: Int) -> Result<String, CustomError> from "bindings/custom.toml";
```

**Step 3: Implement Foreign Function** (Rust)
```rust
// my_crate/src/module.rs
pub fn my_function(param1: String, param2: i64) -> String {
    // Return JSON: success or error
    if param1.len() > 0 {
        format!(r#"{{"success": {{"result": "{}"}}}}"#, param1.repeat(param2 as usize))
    } else {
        r#"{"error": {"code": 1, "message": "Empty input"}}"#.to_string()
    }
}
```

**Step 4: Use in Brief**
```brief
defn use_my_function(input: String) [input.len() > 0] [result != ""] -> String {
    let raw = frgn my_function(input, 3);
    
    [raw is Success(result)] term result;
    [raw is CustomError(err)] term "Error: " + err.message;
};
```

### 5.2 Error Type Conventions

**Recommended Error Structure:**
```toml
[functions.output.error]
type = "FunctionNameError"
code = "Int"
message = "String"
context = "String"  # Optional: additional context
```

**Error Codes Convention:**
- `0` - Generic/unknown error
- `1-99` - Validation errors (bad input)
- `100-199` - System errors (file not found, permission denied)
- `200-299` - Resource errors (out of memory, timeout)
- `300+` - Application-specific errors

### 5.3 Type Mapping Guide

| Brief Type | JSON | Rust |
|-----------|------|------|
| `String` | `"value"` | `String` |
| `Int` | `123` | `i64` |
| `Float` | `1.23` | `f64` |
| `Bool` | `true` | `bool` |
| `void` | `null` | `()` |
| `[T]` | `[...]` | `Vec<T>` |
| `{ field: T }` | `{ "field": ... }` | Struct |

---

## 6. Platform Targets and Extensibility (Hooks)

### 6.1 Target Declaration

TOML supports target field for future extensibility:

```toml
target = "native"  # Rust FFI (v6.2)
# target = "wasm"    # WebAssembly (future)
# target = "c_ffi"   # C FFI (future)
```

### 6.2 Type Mapping Hooks (For Future Cross-Language Support)

TOML can declare type mappings for different backends:

```toml
[[functions]]
name = "read_file"

# Basic declaration
[functions.input]
path = "String"

# Future: Type mappings for different languages
# [functions.type_mappings.Rust]
# path = "String"
# 
# [functions.type_mappings.C]
# path = "const char*"
```

### 6.3 Multiple Target Implementations (Future)

```toml
[[functions]]
name = "read_file"
location = "std::fs::read_to_string"

[functions.targets.native]
impl = "std::fs::read_to_string"

# [functions.targets.wasm]
# impl = "wasm_host::read_file"
```

---

## 7. Compiler Integration

### 7.1 Type Checking Phase

When compiler encounters `frgn` declaration:
1. Load TOML file from specified path
2. Parse TOML and extract function metadata
3. Validate Brief signature matches TOML contract
4. Ensure Result type properly structured
5. Check error type is properly named

### 7.2 Verification Phase

When verifying `defn` that calls `frgn`:
1. Treat `frgn` call as statement with known contract
2. Verify defn handles both success and error branches
3. Ensure postcondition holds for all outcomes
4. Check exhaustiveness: all error types handled

### 7.3 Code Generation Phase (Stub for v6.2)

- Generates FFI glue code stubs
- Maps Brief types to JSON serialization
- Creates deserialization handlers
- Hooks for future code generation engines

---

## 8. Complete Working Example

**File: `example_safe_file_read.bv`**

```brief
// Declare FFI binding
frgn read_file(path: String) -> Result<String, IoError> from "std/bindings/io.toml";

// Define safe wrapper that handles all error cases
defn safe_read_with_default(path: String, default: String) 
    [path.len() > 0] 
    [result == @default | result != @default] 
    -> String {
    
    let raw = frgn read_file(path);
    
    [raw is Success(content)] term content;
    [raw is IoError(err)] {
        [err.code == 2] term "File not found: " + path;  // ENOENT
        [err.code == 13] term "Permission denied: " + path;  // EACCES
        [true] term "Error reading " + path + ": " + err.message;
    };
};

// Create signature for this safe wrapper
sig read_safe: String, String -> String;

// Use in transaction
txn load_application_config [true] [config_loaded] {
    let config_text = safe_read_with_default("app.toml", "debug: true");
    &app_config = config_text;
    term;
};
```

---

## 9. Specification Stability

### 9.1 Stable (v6.2 and beyond)

- ✅ FFI declaration syntax (`frgn identifier () -> Result<T, E> from "file.toml"`)
- ✅ TOML binding format and schema
- ✅ Error handling patterns in defn
- ✅ Type system integration
- ✅ Standard library bindings location and structure

### 9.2 Extensible (hooks for future)

- 🔮 Non-Rust language support (via target field)
- 🔮 WASM target (separate target declaration)
- 🔮 Type mapping system for cross-language types
- 🔮 Dynamic binding loading (not compile-time-only)

### 9.3 Not Supported (v6.2)

- ❌ Direct foreign function calls without TOML binding
- ❌ Variadic arguments in frgn
- ❌ Function pointers/callbacks
- ❌ Pointer manipulation or memory control
- ❌ Non-Result return types from frgn (must be `Result<T, E>`)

---

## 10. Design Rationale

### Why TOML?

- Human-readable and writable
- Clear structure for explicit contracts
- Easy to extend with new fields
- Well-tooled, standard format
- Less verbose than YAML, more flexible than JSON

### Why JSON wire format?

- Language-agnostic (works with C, Python, JS, Rust, etc.)
- Human-readable for debugging
- Simple parsing in Brief
- Reduces type mapping complexity
- Future supports multiple serialization formats (MessagePack, CBOR)

### Why Brief wraps foreign code, not the reverse?

- Brief owns contracts; foreign code is untrusted
- Separation of concerns: defn ensures safety, frgn is pure plumbing
- Easier debugging: failures are in Brief code (visible + verifiable)
- Enables exhaustiveness checking at compiler level

### Why full generics support from day one?

- Complex foreign libraries need complex types
- Generic defn wrappers reduce boilerplate
- Future-proofs against sophisticated integrations
- Maintains Brief's type system consistency

---

## 11. Migration from v6.1

**No Breaking Changes** - v6.2 is fully backward compatible with v6.1.

- Existing code: unaffected
- New FFI system: opt-in via `frgn` declarations
- TOML bindings: optional (no existing code uses them)
- All v6.1 features: unchanged

---

## Appendix: TOML Schema BNF

```toml
bindings_file ::= (function_declaration)*

function_declaration ::= 
    "[[functions]]" NL
    "name" "=" STRING NL
    "description" "=" STRING NL
    "location" "=" STRING NL
    "target" "=" ("native" | "wasm") NL
    input_section
    output_section

input_section ::=
    "[functions.input]" NL
    (identifier "=" STRING NL)*

output_section ::=
    "[functions.output.success]" NL
    (identifier "=" STRING NL)*
    "[functions.output.error]" NL
    "type" "=" STRING NL
    (identifier "=" STRING NL)*
```

