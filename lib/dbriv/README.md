# DBriv - Briv's Hardware Register and Data Description Language

**Version:** 0.1.0  
**Status:** Active development

---

## Overview

DBriv (`.dbv`, `.dbvs`, `.dbvl` files) is Briv's language for:
- Hardware register declarations
- FFI (Foreign Function Interface) bindings
- Data description and schemas
- Line-based mutable data

---

## File Types

| Extension | Name | Purpose |
|-----------|------|---------|
| `.dbvs` | DBriv Schema | Template/schema definitions (declarations only) |
| `.dbv` | DBriv View | Concrete bindings (schema + data) |
| `.dbvl` | DBriv Live | Mutable line-based data records |

---

## Keywords and Abbreviations

All DBriv keywords support abbreviations:

| Keyword | Abbreviations | Description |
|---------|---------------|-------------|
| `register` | `reg`, `regs` | FFI/hardware register declaration |
| `service` | `serv`, `svc` | I/O interface definitions |
| `alias` | `ali` | Named address aliases |
| `struct` | `stru`, `str` | Structure type definitions |
| `enum` | `en` | Enumeration type definitions |
| `rule` | `rl`, `rul` | Logic programming rules |
| `check` | `chk` | Standalone contract conditions |
| `depends` | `dep`, `deps` | Dependency declarations |
| `import` | `imp` | File imports |

---

## Usage Examples

### Register (FFI Binding)

```dbriv
// Full keyword
register 0x00 as "sqrt" {
    type: Float;
    location: "std::f64::sqrt";
    target: native;
    description: "Compute square root";
}

// Abbreviated
reg @sqrt as "sqrt" {
    type: Float;
    location: "std::f64::sqrt";
    target: native;
}
```

### Service (I/O Interface)

```dbriv
// Full keyword
service ImageClassifier {
    INPUT img_data: Vector[UInt[8], 4096];
    OUTPUT label: String;
    OUTPUT confidence: Float;
}

// Abbreviated
serv Classifier {
    INPUT data: Data;
    OUTPUT result: String;
}
```

### Alias (Named Address)

```dbriv
// Full keyword
ALIAS debug_led: UInt[8] = 0xFF5E0000;

// Abbreviated with optional
alias? debug_mode: Bool;
```

### Dependencies

```dbriv
// Single dependency
DEPENDS "sqlite3" VERSION ">=3.36.0" PLATFORM native;

// With features
DEPENDS "math" VERSION ">=2.0" FEATURES [simd, unsafe];

// Abbreviated
dep "openssl" VERSION "^1.1" PLATFORM [native, wasm];
```

### Record (Data Instance)

```dbriv
@0x1000 {
    name: "sensor_1";
    value: 42;
    enabled: true;
}
```

---

## CLI Commands

```bash
# Parse DBVS schema and export to JSON
briv dbvs <file.dbvs> [--out <file.json>] [--pretty]

# Parse DBVL (line-based) and export to JSON  
briv dbvl <file.dbvl> [--out <file.json>] [--pretty]

# Parse DBV (full) and export to JSON
briv dbv <file.dbv> [--out <file.json>] [--pretty]
```

---

## FFI Binding Schema

FFI bindings in DBriv use the `register` keyword to declare external functions:

```dbriv
register <address> as "<name>" {
    type: <function signature>;
    location: "<module::path>";
    target: <platform>;
    description: "<documentation>";
    check: [<contract conditions>];
}
```

### Address Types

- `0x...` - Hexadecimal address
- Numeric - Decimal number
- `@name` - Named identifier (for FFI, not hardware addresses)
- `auto` - Automatic address allocation

### Platforms

- `native` - Native Rust/hosted
- `wasm` - WebAssembly
- `c` - C/embedded

---

## Type System

### Primitive Types

- `Bool` - Boolean
- `Int[N]` - Signed integer (N bits)
- `UInt[N]` - Unsigned integer (N bits)  
- `Float` - Floating point
- `String` - Text
- `Data` - Raw bytes

### Complex Types

- `Vector[T, N]` - Fixed-size array
- `Option[T]` - Optional value
- `Result[T, E]` - Result type
- `Fn(Args...) -> Ret` - Function signature (not yet fully parsed)

---

## Related Files

- `std/bindings/*.dbvs` - FFI binding schemas
- `lib/targets/*.dbv` - Hardware target definitions
- `src/dbriv/parser.rs` - Parser implementation
- `src/dbriv/ast.rs` - AST definitions

---

*Last updated: 2026-05-08*