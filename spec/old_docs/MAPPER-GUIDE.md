# Brief Mapper Guide

**Version:** 1.0  
**Purpose:** How to create language-specific FFI mappers  

---

## Table of Contents

1. [Overview](#overview)
2. [How Mappers Fit Into FFI](#how-mappers-fit-into-ffi)
3. [Mapper Types](#mapper-types)
4. [Required Functions](#required-functions)
5. [Type Conversions](#type-conversions)
6. [Brief Mapper Examples](#brief-mapper-examples)
7. [Creating a New Mapper](#creating-a-new-mapper)
8. [Testing Your Mapper](#testing-your-mapper)
9. [Template](#template)

---

## Overview

A **mapper** translates between foreign language types and Brief types. When you call a foreign function, the mapper ensures data flows correctly across the language boundary.

```
Brief Code
    |
    v
frgn declaration  -->  TOML binding
    |                      |
    v                      v
Mapper (map_input)    Foreign input types
    |
    v
Foreign Function
    |
    v
Mapper (map_output)    Brief output types
```

---

## How Mappers Fit Into FFI

1. **TOML binding** declares the foreign function
2. **Language analyzer** extracts the function signature
3. **Mapper** transforms input/output/error types
4. **Runtime** executes the foreign function with mapped data

```
┌─────────────────────────────────────────────────────────┐
│                    TOML Binding                          │
│  frgn read_file(path: String) -> String from "libc";    │
└─────────────────────────────────────────────────────────┘
                            │
                            v
┌─────────────────────────────────────────────────────────┐
│                   Language Analyzer                      │
│  Detects: extern crate libc { fn read_file(...) }       │
└─────────────────────────────────────────────────────────┘
                            │
                            v
┌─────────────────────────────────────────────────────────┐
│                      Mapper                             │
│  map_input: CString -> String                          │
│  map_output: CString -> String                         │
│  map_error: CError -> BriefError                       │
└─────────────────────────────────────────────────────────┘
                            │
                            v
┌─────────────────────────────────────────────────────────┐
│                   Foreign Library                       │
│  libc::read_file(path: *const c_char) -> *mut c_char  │
└─────────────────────────────────────────────────────────┘
```

---

## Mapper Types

### Brief Mappers (.bv)

Written in Brief itself. Best for simple type conversions.

**Location:** `lib/ffi/mappers/<lang>_mapper.bv`

```brief
// Example: Brief mapper for simple types
defn map_input(value: Int) -> Int [true][result == value] {
    term value;
};

defn map_output(value: Int) -> Int [true][result == value] {
    term value;
};
```

### Rust Mappers (Cargo crate)

Written in Rust. Required for complex transformations or FFI with unsafe code.

**Location:** `lib/ffi/mappers/<lang>/`

```
lib/ffi/mappers/python/
├── Cargo.toml
└── src/
    └── lib.rs
```

---

## Required Functions

Every mapper must implement these three functions:

### map_input(value) -> Value

Transforms Brief input into foreign input type.

```brief
defn map_input(value: Value) -> Value [true][true] {
    term value;  // Default: pass through unchanged
};
```

### map_output(value) -> Value

Transforms foreign output into Brief output type.

```brief
defn map_output(value: Value) -> Value [true][true] {
    term value;  // Default: pass through unchanged
};
```

### map_error(err: Error) -> Error

Transforms foreign error into Brief error.

```brief
defn map_error(err: Error) -> Error [true][true] {
    term err;  // Default: pass through unchanged
};
```

---

## Type Conversions

### String Types

| Foreign Type | Brief Type | Conversion |
|--------------|------------|------------|
| `char*` (C) | `String` | Null-terminated UTF-8 |
| `String` (Rust) | `String` | 1:1 copy |
| `str` (Python) | `String` | Python str to Brief String |
| `String` (JS) | `String` | JS String to Brief String |

### Numeric Types

| Foreign Type | Brief Type |
|--------------|------------|
| `int` (C) | `Int` |
| `i32/i64` (Rust) | `Int` |
| `int` (Python) | `Int` |
| `number` (JS) | `Float` or `Int` |

### Complex Types

| Foreign Type | Brief Type | Notes |
|--------------|------------|-------|
| `struct` (C) | `Data` | Raw bytes |
| `Vec<u8>` (Rust) | `Data` | Raw bytes |
| `bytes` (Python) | `Data` | Raw bytes |
| `ArrayBuffer` (JS) | `Data` | Raw bytes |

---

## Brief Mapper Examples

### C Mapper (lib/ffi/mappers/c_mapper.bv)

```brief
// C Mapper - Handles C string null-termination, UTF-8, etc.

// Convert C string to Brief String
defn c_string_to_brief(c_str: CString) -> String [c_str.is_valid() && c_str.not_null()][true] {
    term c_str.to_str();
};

// Convert Brief String to C string
defn brief_string_to_c(s: String) -> CString [true][true] {
    term CString::new(s);
};

// Convert C int to Brief Int
defn c_int_to_brief(c_int: CInt) -> Int [true][true] {
    term c_int.value;
};

// Handle nullable C pointer
defn c_ptr_to_maybe(ptr: CPtr) -> Bool [true][result == true || result == false] {
    [ptr.is_null()] { term false; };
    term true;
};

// Handle C array conversion
defn c_array_to_list(ptr: CPtr, len: Int) -> List<Int> [ptr.not_null() && len >= 0][result.len() == len] {
    term ptr.to_list(len);
};
```

### WASM Mapper (lib/ffi/mappers/wasm_mapper.bv)

```brief
// WASM Mapper - Handles WebAssembly linear memory, JS value conversion

// Convert WASM pointer to Brief String
defn wasm_ptr_to_string(ptr: Int, memory: Memory) -> String [ptr > 0 && memory.valid()][true] {
    term memory.read_string(ptr);
};

// Convert Brief String to WASM pointer
defn brief_string_to_wasm(s: String, memory: Memory) -> Int [memory.valid()][result > 0] {
    term memory.write_string(s);
};

// Convert WASM pointer to Brief Data
defn wasm_ptr_to_data(ptr: Int, len: Int, memory: Memory) -> Data [ptr > 0 && len >= 0][result.len() == len] {
    term memory.read_bytes(ptr, len);
};

// Write Brief Data to WASM memory
defn brief_data_to_wasm(data: Data, memory: Memory) -> Int [memory.valid()][result > 0] {
    term memory.write_bytes(data);
};

// Convert JS value to Brief Data
defn js_value_to_data(js_val: JsValue) -> Data [true][true] {
    term js_val.to_bytes();
};

// Convert Brief Data to JS value
defn data_to_js_value(data: Data) -> JsValue [true][true] {
    term JsValue::from_bytes(data);
};
```

### Rust Mapper (lib/ffi/mappers/rust_mapper.bv)

```brief
// Rust Mapper - 1:1 mapping
// No transformation between Rust and Brief types

// Map input: 1:1 identity
defn map_input(value: Value) -> Value [true][true] {
    term value;
};

// Map output: 1:1 identity  
defn map_output(value: Value) -> Value [true][true] {
    term value;
};

// Map error: 1:1 identity
defn map_error(err: Error) -> Error [true][true] {
    term err;
};

// Identity mapper - for simple Rust FFI
defn identity(value: Value) -> Value [true][true] {
    term value;
};
```

---

## Creating a New Mapper

### Step 1: Analyze the Target Language

Understand:
- How strings are represented (null-terminated, UTF-8, etc.)
- How memory is managed (garbage collected, manual, etc.)
- What numeric types exist
- How errors are reported

### Step 2: Create the Mapper File

**For simple mappers:** Create `lib/ffi/mappers/<lang>_mapper.bv`

**For complex mappers:** Create a directory `lib/ffi/mappers/<lang>/` with Rust code

### Step 3: Implement Required Functions

```brief
// Required: identity mapper
defn map_input(value: Value) -> Value [true][true] { term value; };
defn map_output(value: Value) -> Value [true][true] { term value; };
defn map_error(err: Error) -> Error [true][true] { term err; };
```

### Step 4: Add Type-Specific Functions

For C:
```brief
defn c_string_to_brief(c_str: CString) -> String [c_str.is_valid()][true] {
    term c_str.to_str();
};
```

For Python:
```brief
defn py_string_to_brief(py_str: PyString) -> String [py_str.is_valid()][true] {
    term py_str.to_string();
};
```

For JavaScript:
```brief
defn js_to_brief_string(js_val: JsValue) -> String [js_val.is_string()][true] {
    term js_val.to_string();
};
```

### Step 5: Register the Mapper

Add to `lib/ffi/mappers.rs` or use the auto-discovery path:
- `lib/mappers/<name>.bv`
- `lib/mappers/<name>/`
- `lib/ffi/mappers/<name>.bv`
- `lib/ffi/mappers/<name>/`

### Step 6: Test the Mapper

```brief
// Test file: test_<lang>_mapper.bv
frgn sig test_function(param: String) -> String from "test_lib";

defn test [true][result == "hello"] {
    let result = test_function("hello");
    term result;
};
```

---

## Testing Your Mapper

### Unit Tests

```brief
defn test_c_string_conversion [true][result == "test"] {
    let c_str = CString::new("test");
    let brief_str = c_string_to_brief(c_str);
    term brief_str;
};
```

### Integration Tests

```brief
frgn sig strlen(s: String) -> Int from "libc";

defn test_strlen [true][result == 5] {
    let len = strlen("hello");
    term len;
};
```

### Error Handling Tests

```brief
frgn sig may_fail() -> Result<Int, MyError> from "test_lib";

defn test_error_mapping [true][result == true] {
    let result = may_fail();
    [result.is_err()] {
        let err = map_error(result.err());
        term err.message == "expected error";
    };
    [result.is_ok()] { term false; };
};
```

---

## Template

Use this template to create a new Brief mapper:

```brief
// <Language> Mapper
// 
// Brief FFI mapper for <language>.
// Transforms between <language> types and Brief types.
//
// For documentation, see: spec/MAPPER-GUIDE.md

// ============================================================
// Required Functions (Identity by default)
// ============================================================

defn map_input(value: Value) -> Value [true][true] {
    term value;
};

defn map_output(value: Value) -> Value [true][true] {
    term value;
};

defn map_error(err: Error) -> Error [true][true] {
    term err;
};

// ============================================================
// String Conversions
// ============================================================

// TODO: Add string conversion functions

// defn <lang>_string_to_brief(<lang>_str: <LangString>) -> String [<conditions>][true] {
//     term <lang>_str.to_string();
// };

// defn brief_string_to_<lang>(s: String) -> <LangString> [true][true] {
//     term <LangString>::new(s);
// };

// ============================================================
// Numeric Conversions  
// ============================================================

// TODO: Add numeric conversion functions

// defn <lang>_int_to_brief(<lang>_int: <LangInt>) -> Int [true][true] {
//     term <lang>_int.value;
// };

// ============================================================
// Complex Type Conversions
// ============================================================

// TODO: Add complex type conversions (Data, Lists, etc.)

// ============================================================
// Memory Management
// ============================================================

// TODO: Add memory handling if applicable

// ============================================================
// Error Handling
// ============================================================

// TODO: Add error type conversions

// defn <lang>_error_to_brief(<lang>_err: <LangError>) -> Error [true][true] {
//     term Error { message: <lang>_err.message };
// };
```

---

## Checklist for New Mappers

- [ ] Created mapper file at correct path
- [ ] Implemented `map_input(value) -> Value`
- [ ] Implemented `map_output(value) -> Value`
- [ ] Implemented `map_error(err) -> Error`
- [ ] Added string conversion functions
- [ ] Added numeric conversion functions
- [ ] Added complex type conversions (Data, Lists) if needed
- [ ] Added memory management functions if needed
- [ ] Added error handling functions
- [ ] Created unit tests for each conversion
- [ ] Created integration tests with actual foreign functions
- [ ] Documented type mappings in comments
- [ ] Updated TOML bindings to use the mapper

---

## See Also

- [FFI-GUIDE.md](FFI-GUIDE.md) - FFI overview
- [FFI-MAPPER-SPEC.md](FFI-MAPPER-SPEC.md) - Mapper specification
- `lib/ffi/mappers/c_mapper.bv` - C mapper example
- `lib/ffi/mappers/wasm_mapper.bv` - WASM mapper example
- `lib/ffi/mappers/rust_mapper.bv` - Rust mapper example
