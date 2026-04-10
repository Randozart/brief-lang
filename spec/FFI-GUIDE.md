# Brief FFI Guide

**Version:** 8.5 (Metropolitan Edition)  
**Purpose:** Unified Foreign Function Interface with dynamic target-aware implementation

---

## Overview

FFI (Foreign Function Interface) in Brief is **Target-Aware** and **Universal**. A single declaration in your code transparently maps to different implementations depending on the target platform (Native binary vs. WebAssembly).

### Design Principles

1. **Zero Magic Words**: The compiler contains no hardcoded FFI function names. All logic is driven by TOML metadata.
2. **TOML is the single Source of Truth**: All FFI metadata—signatures, native locations, and WASM implementations—lives in TOML files.
3. **Logically Closed**: Foreign code returns `Result<T, E>`. Success extracts the value; failure triggers a transaction rollback.
4. **Metropolitan Architecture**: Add new foreign functions at runtime by creating a TOML file. No compiler rebuild required.

---

## TOML Bindings

TOML files define how a Brief function maps to a foreign implementation.

### metropolitan structure

Each function entry can have multiple targets. The compiler selects the correct one during code generation.

```toml
[[functions]]
name = "__http_get"
location = "brief_ffi_native::__http_get"
target = "native"
mapper = "rust"
description = "Native implementation (Rust)"

[functions.input]
url = "String"

[functions.output.success]
result = "String"

[functions.output.error]
type = "HttpError"
code = "Int"
message = "String"

[[functions]]
name = "__http_get"
location = "__http_get"
target = "wasm"
mapper = "wasm"
description = "Web implementation (JavaScript)"
wasm_impl = """
function __http_get(url) {
    try {
        const xhr = new XMLHttpRequest();
        xhr.open('GET', url, false);
        xhr.send();
        return xhr.status >= 200 && xhr.status < 300 ? xhr.responseText : '';
    } catch(e) {
        return '';
    }
}
"""

[functions.input]
url = "String"

[functions.output.success]
result = "String"

[functions.output.error]
type = "HttpError"
code = "Int"
message = "String"
```

### Field Reference

| Field | Required | Description |
|-------|----------|-------------|
| `name` | Yes | Brief function name (e.g., `__parse`) |
| `target` | Yes | `native` or `wasm` |
| `location` | Yes | Native: Rust path. WASM: JS function name. |
| `wasm_impl` | No | Inline JS code for WASM target (optional) |
| `input` | Yes | Parameter name-type pairs |
| `output.success`| Yes | Success return value name and type |
| `output.error` | Yes | Error type name and fields |

---

## Brief Declarations

All foreign functions **must** return a `Result` and specify their TOML source. The `frgn sig` syntax is deprecated.

### Syntax
```brief
frgn name(params) -> Result<SuccessType, ErrorType> from "path.toml";
```

### Path Resolution
The compiler resolves TOML paths in this order:
1. Relative to the declaring `.bv` or `.rbv` file.
2. Standard library bindings (`std/bindings/`).
3. Absolute path.
4. Project-relative path.

---

## WebAssembly (WASM) Integration

When compiling for the web (`brief rbv`), the compiler:
1. Loads the TOML binding.
2. Extracts the `wasm_impl` (if present).
3. Generates a dynamic JavaScript glue file containing the implementation.
4. Exposes the function on the `window` object for WASM interop.
5. Generates a Rust-to-JS FFI call using `js_sys::Reflect`.

This allows standard library functions like `__parse` and `__http_get` to work transparently in the browser without magic hardcoded implementations in the compiler binary.

---

## Error Handling

Brief uses an automatic **Rollback on Error** pattern.

```brief
frgn __http_get(url: String) -> Result<String, HttpError> from "http.toml";

txn get_data [data == ""][~data] {
    let response = __http_get("/api/data");
    // If __http_get returns an error, the txn rolls back automatically.
    // 'response' here is automatically the extracted String.
    &data = response;
    term;
};
```

The postcondition `[~data]` (logical NOT empty) is guaranteed because if the FFI call fails, the transaction is reverted and `&data = response` is never executed.
