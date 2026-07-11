# Brief Standard Library and Foreign Functions

**Version:** 1.0  
**Date:** 2026-04-04  
**Status:** Design Document (Future Implementation)

---

## 1. Philosophy

Brief is a declarative language. Users write pure Brief code. Host/native functions are hidden inside the standard library, wrapped in `frgn defn` implementations.

**Users see:**
```brief
import { println } from std.io;

rct txn greet [~/done] {
  term println("Hello, World!");
}
```

**Users don't see:** the complexity of calling native I/O.

---

## 2. `frgn` Keyword

`frgn` declares foreign (host/native) functions:

| Form | Purpose |
|------|---------|
| `frgn sig` | Declare that a foreign function exists with this signature |
| `frgn defn` | Implement the foreign function (Rust side) |

### Examples

**Declaration (Brief side):**
```brief
frgn sig print(msg: String) -> Bool;
frgn sig read_file(path: String) -> String;
```

**Implementation (Rust side):**
```rust
#[brief_frgn]
fn print(msg: &str) -> bool {
    println!("{}", msg);
    true
}

#[brief_frgn]
fn read_file(path: &str) -> String {
    std::fs::read_to_string(path).unwrap_or_default()
}
```

---

## 3. v1: Built-in Only

For v1, all frgns are built into the interpreter:

```
lib/
└── std/
    ├── io.bv         # frgn sig declarations
    ├── math.bv
    ├── string.bv
    ├── collection.bv
    └── time.bv
```

The interpreter registers all functions in its registry at startup.

---

## 4. v2: Plugin System (Future)

Design for extensibility:

### Option A: Transpilation

Brief → Rust transpilation. Third parties:
1. Write Brief code + `frgn defn` implementations
2. Transpile to Rust
3. Compile with their Rust code
4. Link into final binary

```brief
# my_lib/lib.bv
frgn defn cool_thing(): Int {
  // Brief code that calls native things
}
```

### Option B: Library Plugins

Third party publishes a crate with `brief.toml`:

```
my_cool_lib/
├── Cargo.toml
├── src/lib.rs        # Rust implementations
└── brief.toml       # Describes available frgns
```

Compiler loads at compile time.

### Option C: Manifest + Code Generation

Third party writes manifest, compiler generates FFI bindings.

---

## 5. Recommended v2 Approach

**Transpilation (Option A)** is recommended for v2:

1. Brief already compiles to Rust/WASM
2. Transpilation is a natural extension
3. Leverages Rust's ecosystem (crates.io)
4. Users can link any Rust crate
5. No runtime plugin loading complexity

**Implementation steps:**
1. Add `--target rust` to compiler
2. Brief + `frgn defn` → Rust code
3. User compiles Brief → Rust → links with their Rust code
4. Standard library becomes a Rust crate

---

## 6. Third-Party Contribution (v1)

Until plugin system exists, third parties can:
1. Fork the compiler
2. Add their frgns to the registry
3. Add `frgn sig` declarations to stdlib
4. Submit PR to main repo

---

## 7. Security Considerations (Future)

When allowing third-party frgns:

- **Sandboxing:** frgns should not have arbitrary memory access
- **Verification:** Compiler should verify frgn implementations match signatures
- **Versioning:** frgn APIs should be versioned
- **Signing:** Plugin crates should be signed for trust

---

## 8. Related Specifications

- `brief-lang-spec.md` — Language syntax and semantics
- `rendered-brief-spec-v4.md` — UI framework (uses stdlib)

---

## Appendix A: Planned frgn Functions

### std.io

| Function | Signature | Description |
|----------|-----------|-------------|
| `print` | `String -> Bool` | Print to stdout |
| `println` | `String -> Bool` | Print with newline |
| `input` | `-> String` | Read line from stdin |

### std.math

| Function | Signature | Description |
|----------|-----------|-------------|
| `abs` | `Int -> Int` | Absolute value |
| `sqrt` | `Float -> Float` | Square root |
| `pow` | `Int, Int -> Int` | Power |
| `sin` | `Float -> Float` | Sine |
| `cos` | `Float -> Float` | Cosine |

### std.string

| Function | Signature | Description |
|----------|-----------|-------------|
| `len` | `String -> Int` | String length |
| `concat` | `String, String -> String` | Concatenate |
| `to_string` | `a -> String` | Convert to string |
