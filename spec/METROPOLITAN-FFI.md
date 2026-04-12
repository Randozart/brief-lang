# Metropolitan FFI: Unified & Universal Bindings

Brief 8.5 introduces the **Metropolitan FFI** system. This architecture ensures that Foreign Function Interface (FFI) declarations are target-aware, logically closed, and entirely free of hardcoded compiler logic ("Magic Words").

## Core Philosophy

1.  **Zero Magic Words**: The compiler contains no hardcoded function names. All foreign logic is defined in TOML.
2.  **The TOML is the Contract**: A single `.toml` file defines the signature, error structure, and platform-specific implementations.
3.  **Target Awareness**: One Brief declaration automatically maps to the correct implementation based on whether you are compiling for **Native** (binary) or **Web** (WASM).
4.  **Logically Closed**: All FFI calls return a `Result<T, E>`. Success extracts the value; failure triggers an automatic transaction rollback.

---

## 1. The TOML Structure

Every FFI function requires a TOML binding. You can define a library-level `wasm_setup` in the `[meta]` section for shared imports, and specific `native` (Rust) and `wasm` (JS) targets for each function.

### Example: `storage.toml`
```toml
[meta]
name = "storage"
version = "1.0.0"
# This code is placed ONCE at the top of the glue file
wasm_setup = "console.log('Storage library initialized');"

# Native implementation (Local File)
[[functions]]
# ...
```

---

## 2. Using Libraries from Other Ecosystems

The `wasm_impl` field is raw JavaScript injected into the glue file. This allows you to bridge into any ecosystem available to the host.

### A. Node.js Ecosystem
If your app runs in a Node-like environment (Electron, CLI), use `require` or `import`:
```toml
wasm_impl = """
const crypto = require('crypto');
function hash_data(data) {
    return crypto.createHash('sha256').update(data).digest('hex');
}
"""
```

### B. Python Ecosystem
Browsers can't run Python natively, but you can bridge to it via an API:
```toml
wasm_impl = """
function python_bridge(text) {
    const xhr = new XMLHttpRequest();
    xhr.open('POST', 'http://localhost:5000/process', false); // Sync call
    xhr.send(text);
    return xhr.responseText;
}
"""
```

### C. Web/NPM Ecosystem
Use ES Modules to import modern libraries from CDNs like Skypack or Unpkg:
```toml
wasm_impl = """
import confetti from 'https://cdn.skypack.dev/canvas-confetti';
function celebrate() {
    confetti();
    return true;
}
"""
```

---

## 3. The Brief Declaration

In your `.bv` or `.rbv` file, link to the TOML:

```brief
frgn save_config(path: String, content: String) -> Result<void, StorageError> from "std/bindings/storage.toml";

txn save [true][true] {
    term save_config("settings.txt", "{ theme: 'dark' }");
};
```

The compiler will:
1.  **Validate** the signature against the TOML.
2.  **Native**: Link to the Rust path in `location`.
3.  **WASM**: Extract the `wasm_impl`, put it in the JS glue, and call it via `window.save_config`.

---

## 4. Path Resolution

The `from "path.toml"` clause resolves in this order:
1.  Relative to the current `.bv` file (Recommended for project-specific FFIs).
2.  Standard library bindings (`std/bindings/`).
3.  Absolute filesystem path.
4.  Project root relative path.

## Why "Metropolitan"?

We call this Metropolitan because it allows Brief code to live in "different neighborhoods" (Native, Web, Cloud) while maintaining the same identity (Source Code). You don't change the laws (Brief logic); you just change the infrastructure (TOML implementation).
