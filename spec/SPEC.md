# Brief Language Specification

**Version:** v0.10.0 **Date:** 2026-04-20 **Status:** Development (unstable) **Language Variants:** Core (.bv), Rendered (.rbv), Embedded (.ebv)

## 1. Introduction and Philosophy

Brief is a declarative, contract-enforced logic language designed for building verifiable state machines. It treats program execution as a series of verified state transitions rather than sequential instructions.

Brief is designed for **Formal Verification without the Boilerplate**. It eliminates imperative control flow (`if`, `else`, `while`) in favor of contracts, guards, and atomic transactions.

### 1.1 Core Design Principles

1. **Contracts First**: Every transaction declares what must be true before and after it runs. The compiler verifies these contracts.
2. **Atomic State Transitions**: Transactions are atomic - they either complete fully or roll back completely.
3. **Reactive Execution**: Brief programs use a reactor model where transactions fire automatically when their preconditions are met.
4. **Zero-Nesting Logic**: Branching is handled via guards, not nested blocks. This improves clarity and LLM comprehension.
5. **FFI for External Capabilities**: Brief cannot do everything (file I/O, networking, hardware math). Foreign Function Interface handles these cases with explicit contracts.

### 1.2 Language Variants

* **Core Brief** (`.bv`): Transactional state machines with FFI support
* **Rendered Brief** (`.rbv`): Adds `rstruct`, view components, and UI binding directives  
* **Embedded Brief** (`.ebv`): Adds native `Float` types, vector types, and bit-range addressing

### 1.3 Versioning

* **Semantic**: `v0.10.0` (development)
* **Date-based**: `2026-04-20`

---

## 2. Grammar Specification

### 2.1 Program Structure

```bnf
program ::= (definition | transaction | state_decl | constant | import | struct_def | rstruct_def | enum_def | render_block)*

definition ::= ("defn" | "def" | "definition") identifier type_params? parameters? "->" output_types contract ("{" body "}" ";" | ";")
transaction ::= ("async")? "rct"? "txn" identifier "(" parameters? ")" contract ("{" body "}" ";" | ";")
signature ::= ("sig" | "sign" | "signature") identifier ":" type "->" result_type ("=" identifier "(" arguments? ")" | "from" path)? ";"

constant ::= ("const" | "constant") identifier ":" type "=" expression ";"

struct_def ::= "struct" identifier "{" struct_member* "}"
struct_member ::= field_decl | transaction
field_decl ::= identifier ":" type ";"

rstruct_def ::= "rstruct" identifier "{" struct_member* view_body "}"

enum_def ::= "enum" identifier type_params? "{" enum_variant ("," enum_variant)* ","? "}"
enum_variant ::= identifier ("(" type ("," type)* ")")?

import_stmt ::= "import" ("{" import_item ("," import_item)* "}")? (("from" namespace_path) | namespace_path | string_literal ("as" identifier)? ")? ";"
import_item ::= identifier ("as" identifier)?

render_block ::= "render" identifier "{" view_body "}"
```

### 2.2 Parameters and Types

```bnf
parameters ::= "(" (param ("," param)*)? ")"
param ::= identifier ":" type

type_params ::= "<" identifier ("," identifier)* ">"

type ::= "Int" | "Float" | "String" | "Bool" | "Void" | "Data" | "UInt" | identifier
       | "Vector" "[" type "]"  // Vector type
       | "Option" "[" type "]"  // Optional type
       | type "Union" "[" type ("," type)* "]"  // Union type
       | identifier  // Custom type
       | "Sig" "[" identifier "]"  // Signature type
       | "Result" "[" type "," type "]"  // Result type (for FFI)

output_types ::= type ("," type)*  // Multi-output: (A, B, C)
```

### 2.3 FFI Types and Contracts

```bnf
foreign_sig ::= ("frgn" | "syscall") "sig" identifier "(" parameters? ")" "->" output_types ";"

frgn_binding ::= identifier "(" parameters? ")" "->" Result "[" type_params "]" "from" path") 

contract ::= "[" expression "]" "[" expression "]"
```

The compiler enforces that all FFI calls handle `Result` types. The `frgn` variant returns `Result<T, Error>` and must be handled; the `frgn!` variant returns `void` and is fire-and-forget.

---

## 3. Core Language Features

### 3.1 Transactions and Reactivity

Brief uses a reactor model. Transactions are defined with `rct`:

```brief
rct txn <name> (<params>) [precondition] [postcondition] {
    // Transaction body
}
```

* Without `async`: transactions execute synchronously when preconditions are met
* With `async`: transactions execute concurrently with compiler-verified safety
* Preconditions (`[expression]`): must be true for transaction to fire  
* Postconditions (`[expression]`): must be true after transaction completes

### 3.2 Signatures

Signatures define external FFI bindings:

```brief
sig <name>: <type> -> <result_type> [from <namespace_path>]
```

Result types support:
* `Result<T, E>` - standard FFI call with error handling
* `void` - fire-and-forget (for `frgn!` and `syscall!` variants)

### 3.3 State Management

State is declared globally with `state_decl`:

```brief
state <name>: <type> = <expression>?
```

State declarations support:
* `os_mode: bool` - when true, address is virtual (OS-managed); when false, raw address (embedded)
* `bit_range: Option<BitRange>` - for bit-packed field access
* `span: Option<Span>` - source location for debugging

### 3.4 Control Flow

Brief eliminates imperative branching in favor of guard-based execution:

```brief
[guard_expression] {
    // executes only when guard is true
}
```

Pattern matching via unification:
```brief
unification <identifier>(<pattern>) = <expression>
```

---

## 4. Foreign Function Interface (FFI)

### 4.1 FFI Type System

| Keyword | Return Type | Error Handling | Use Case |
|---------|-------------|---------------|----------|
| `frgn` | `Result<T, E>` | Must handle | Standard foreign function |
| `frgn!` | `void` | None | Fire-and-forget |
| `syscall` | `Result<Int, E>` | Must handle | Kernel calls with returns |
| `syscall!` | `void` | None | Kernel calls without returns |

### 4.2 Address System

The `@` operator has context-aware semantics:

```brief
@address        // Raw, virtual, or WASM offset depending on target
@raw:0xADDRESS  // Raw physical address (embedded only)
@stack:offset   // Offset from stack pointer
@heap:offset    // Offset from heap pointer
```

**Target Behavior:**
* **.bv (OS)**: Virtual offset, compiler manages stack/heap/static via escape analysis
* **.ebv (Embedded)**: Raw physical address, programmer manages memory  
* **.rbv (Browser)**: WASM linear memory offset

### 4.3 Resource System

Resources declare kernel/native objects:

```brief
rsrc <name>: <ResourceType>(<args>)
```

Built-in resource types:
* `FrameBuffer(width, height)` - GPU framebuffer
* `File(path, flags)` - File handles  
* `SharedMemory(name, size)` - Shared memory regions
* `Socket(domain, type)` - Network sockets
* `EventFD()` - Event notification
* `Semaphore(initial)` - Semaphores
* `Mutex` - Mutex locks

*Note: Full kernel negotiation and lifecycle management is planned*

### 4.4 Bit-Packed Structures

Struct fields can be declared with bit widths:

```brief
struct Pixel {
    r: 4bits,
    g: 4bits,
    b: 4bits,
    a: 4bits
}
```

Compiler automatically packs into minimal storage (16 bits for Pixel above).

### 4.5 Vector Types (Embedded)

```brief
let data: Float[64] @/x32;  // 64-element float vector, 32-bit elements
```

---

## 5. Type System

### 5.1 Primitive Types

| Type | Description | Aliases |
|------|-------------|---------|
| `Int` | Signed 64-bit integer | `Signed`, `Sgn` |
| `Float` | 32-bit IEEE 754 float | - |
| `UInt` | Unsigned 64-bit integer | `Unsigned` |
| `Bool` | Boolean (1-bit) | - |
| `String` | UTF-8 string | - |
| `Data` | Opaque binary data | - |
| `Void` | Unit type | - |

### 5.2 Advanced Types

* `Vector[T, N]` - Fixed-size vector
* `Option[T]` - Nullable type
* `Sig[T]` - Signature reference
* Custom types via `struct`, `enum`, `rstruct`

### 5.3 Type Conversion

The compiler performs safe type conversions:
```brief
let x: Float = 3;  // Int → Float (implicit)
let y: Int = x;     // Float → Int (explicit cast needed)
```

---

## 6. Standard Library

### 6.1 FFI Modules

| Module | Purpose | Types |
|--------|---------|-------|
| `std/io` | File I/O | `File`, streams |
| `std/math` | Math operations | `Float`, `Int` |
| `std/string` | String utilities | `String` |
| `std/time` | Time operations | timestamps |
| `std/http` | HTTP client | request/response |
| `std/json` | JSON serialization | `Object`, `Array` |

### 6.2 Core Functions

```brief
// JSON
let json_str: String = to_json(value);
let parsed: Result<Object, String> = from_json(json_str);

// Assertions (for verification)
result.is_ok()   // True if Result is success
result.is_err()  // True if Result is error  
result.value     // Unwrap success value
result.error.code  // Access error code
result.error.message  // Access error message
```

---

## 7. Implementation Status

| Feature | Status | Notes |
|---------|--------|-------|
| Core language (transactions, guards, structs) | ✅ Complete | |
| FFI type taxonomy | ✅ Complete | |
| Address system (virtual/raw) | ✅ Complete | |
| `frgn`, `frgn!`, `syscall`, `syscall!` | ✅ Complete | |
| `rsrc` / `resource` parsing | ✅ Complete | |
| `Float` type | ✅ Complete | |
| Vector types | ✅ Complete | |
| Bit-packing | ✅ Complete | AST only |
| Syscall TOML loading | ⚠️ Planned | Parser reads TOML, backend needs work |
| Resource kernel generation | ⚠️ Planned | Backend needed |
| Bit-packed struct code gen | ⚠️ Planned | Backend needed |
| `to_json`/`from_json` stdlib | ✅ Complete | |

---

## 8. Migration Guide

### From v1 (Legacy)

Legacy code continues to work. The compiler auto-upgrades:

```brief
// v1 style - still valid, auto-upgrades
frgn sqrt(x: Float) -> Result<Float, MathError> from "math.toml";

// v2 explicit forms
frgn  sqrt(x: Float) -> Result<Float, MathError> from "math.toml";  // with Result
frgn! write_to_hw(address, value);  // fire and forget
```

The compiler auto-generates:
- `pre [true]` if no precondition
- `post [true]` if no postcondition  
- Layout auto-calculation if not specified

---

## 9. Error Messages

The compiler produces clear error messages for:
* Unhandled FFI results (violating contracts)
* Type mismatches
* Missing pre/post conditions
* Invalid address modes
* Resource conflicts

---

*Last updated: Brief v0.10.0 (2026-04-20)*