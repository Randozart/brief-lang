# Backend Type Dispatch Architecture

## Design Philosophy

The Brief compiler does NOT hardcode primitive type mappings in Rust match arms.
Type metadata declared in **source** drives all backend emission decisions.
The Rust binary is a *thin reader* of the type system defined in Brief's own bootstrap files.

If the prelude (`bootstrap.bv`) is not loaded, every type is raw `Bits(N)` — no assumptions,
no guesswork. The programmer defines their own types, or loads the prelude.

## The Core Insight

Every type is `Bits(N)` at minimum:

```brief
type Int <: Bits { bytes <~ 8; primitive <~ Int; }
type Float <: Bits { bytes <~ 4; primitive <~ Float; }
type String <: Bits { bytes <~ 8; primitive <~ String; encoding <~ "utf-8"; }
```

- **`bytes <~ N`** — Every backend reads this. `Bits(8)` = 64-bit storage. Always sufficient.
- **`primitive <~ PascalCase`** — Optional semantic hint for backends. Declared in source. NEVER hardcoded in Rust.
- **Other metadata** — Any backend is free to use any metadata it needs. Unrecognized metadata is silently ignored.

## Frontend/Backend Detachment

The frontend (parser + AST) and backend (codegen) are coupled through exactly one narrow interface:

| Layer | Owns | Exposes | Backend sees |
|-------|------|---------|--------------|
| **Source** | Type definitions | `bootstrap.bv` | — |
| **Parser** | Syntax + AST construction | `TypeDefBody`, `TypeDefSlot`, `PropertyValue` | — |
| **TypeUniverse** | Type resolution | `ResolvedType { bytes, properties, name, base, alignment }` | `resolve_type()` |
| **Config** | LLVM type mapping | `config/llvm-primitives.toml` | `derive_llvm_type()` |
| **Backend** | IR emission | — | Everything above, read-only |

**The frontend never calls backend code.** The backend never modifies the AST.
The contract is: **frontend provides metadata, backend interprets it.**

A metadata slot added to a type definition in source is automatically visible to every backend.
No Rust changes needed. No recompilation. Example:

```brief
type HalfFloat <: Bits { bytes <~ 2; primitive <~ Float; }
```

The parser stores `primitive <~ Float` in `TypeDefBody.metadata["primitive"]`.
The universe reads it into `ResolvedType.properties["primitive"] = PropertyValue::Identifier("Float")`.
The LLVM backend finds `(primitive="Float", bytes=2)` → config → `"half"`.
The CIRCT backend ignores the primitive, reads `bytes=2` → emits 16 wires.

**Zero Rust changes across the entire pipeline.**

## How Each Backend Interprets Metadata

Every backend receives the same `ResolvedType { bytes, properties, ... }`.
Each backend reads what it needs and ignores the rest.

### LLVM Backend

| Metadata | What it reads | What it emits |
|----------|--------------|---------------|
| `properties["primitive"]` + `bytes` | Config lookup via `derive_llvm_type()` | LLVM type string (`"i64"`, `"float"`, `"ptr"`, ...) |
| `properties["primitive"] == "Float"` | Determines float arithmetic vs integer | `fadd`/`fsub`/`fmul` vs `add`/`sub`/`mul` |
| `properties["primitive"] == "String"` | Enables string-specific helpers | `__int_to_str__`, `__str_to_int`, `@ll_empty_list` |
| `properties["encoding"]` | String/Data encoding | — (future: null vs length-prefixed) |
| `bytes` | Storage width | `alloca`, `malloc` size, GEP offsets |
| `alignment` | Memory alignment | `align N` attribute on `alloca`/`store` |
| No `primitive` | Falls back to raw Bits | `format!("i{}", bytes * 8)` — structural only |

### CIRCT (Hardware) Backend

| Metadata | What it reads | What it emits |
|----------|--------------|---------------|
| `bytes` | Bit width | `hw.param.value` width, Verilog port width |
| `alignment` | — | (ignored — hardware has native alignment) |
| `properties["primitive"]` | **IGNORED** | Not needed — hardware doesn't care if it's Int or String |
| `properties["encoding"]` | **IGNORED** | Raw bits only |
| No `primitive` | Works correctly | Raw Bits(N) — always sufficient |

**CIRCT example**: `String` with `bytes=8` emits as 64 wires, same as `Int` with `bytes=8`.
The hardware doesn't distinguish between the two — it's the programmer's responsibility to
ensure the connected hardware interprets the bits correctly. This is correct by design.

### GPU Backend

| Metadata | What it reads | What it emits |
|----------|--------------|---------------|
| `bytes` | Storage width | `i32`, `i64` for buffer bindings |
| `properties["primitive"]` | **Selective** — only checks for subgroup barriers | Barrier intrinsics |
| Everything else | **IGNORED** | Raw bits for compute |
| No `primitive` | Works correctly | Raw bits — GPU compute doesn't need types |

### Webstack (WASM + JS) Backend

| Metadata | What it reads | What it emits |
|----------|--------------|---------------|
| `bytes` | Storage width | WASM local type (`i32`, `i64`, `f32`, `f64`) |
| `properties["primitive"]` | JS type mapping | `BigInt64Array` vs `Float64Array` vs `TextEncoder` |
| `properties["encoding"]` | String serialization | `TextEncoder.encode()` vs `TextDecoder.decode()` |
| No `primitive` | Falls back to `bytes` | Raw typed array view — always correct |

### New Backend (Hypothetical)

| Metadata | What it reads | What it emits |
|----------|--------------|---------------|
| `bytes` | Minimum requirement | Any width-based type system |
| `properties["primitive"]` | Optional: semantic hints | Backend-specific IR patterns |
| Everything else | Whatever it needs | Fully extensible |
| No `primitive` | Works correctly | Raw Bits(N) — enough to be functional |

## The Coupling Boundary

```
SOURCE ──► PARSER ──► AST ──► UNIVERSE ──► BACKEND
                        │                     │
                        │   NEVER mutates     │
                        │   the AST           │
                        └─────────────────────┘

The frontend OWNS:  Expr, Statement, Type, TopLevel, TypeDef, PropertyValue
The backend READS:   All of the above, via &reference
The backend OWNS:   CompilerContext, TypedRegister, LLVM IR output string
```

Adding a new metadata field to a type definition:

```brief
type MyColor <: Bits {
    bytes <~ 4;
    primitive <~ UInt;
    gamma <~ "sRGB";         // new metadata — no Rust changes
}
```

**Parser**: Already handles `identifier <~ expression` in type bodies. No change.
**Universe**: `properties["gamma"] = PropertyValue::Quoted("sRGB")`. No change.
**Any backend**: Reads `properties.get("gamma")` if it cares. No change otherwise.

## The Type Resolution Flow

```
Source type:  Custom("Int")  or  Custom("MyCustomType")
                    │
                    ▼
     universe.get("Int") ?
     ┌───── Yes ─────┴───── No ─────┐
     │                              │
     ▼                              ▼
  ResolvedType{                  resolve_type() returns None
    bytes: 8,                    Backend treats as raw Bits(8):
    primitive: Some("Int"),        bytes = 8
    ...                           primitive = None
  }                               derive_llvm_type(None, 8) → "i64"
```

The Rust compiler NEVER matches the string `"Int"` or `"Float"` to infer semantics.
`Type::int()` is a name constructor — it creates `Custom("Int")` with NO semantics.
Semantics come from the universe, populated by source declarations.

## The LLVM Type Config File

The `(primitive, bytes) → LLVM type string` mapping lives in a standalone config file,
read at compile time by `TypeConfig::load()`.

```toml
# config/llvm-primitives.toml
[primitive.Int]
1 = "i8"
2 = "i16"
4 = "i32"
8 = "i64"

[primitive.UInt]
1 = "i8"
2 = "i16"
4 = "i32"
8 = "i64"

[primitive.Float]
2 = "half"
4 = "float"
8 = "double"

[primitive.Bool]
1 = "i8"

[primitive.Char]
4 = "i32"

[primitive.String]
8 = "ptr"

[primitive.Data]
8 = "ptr"
```

## The Derive Function

```rust
fn derive_llvm_type(primitive: Option<&str>, bytes: u64, config: &TypeConfig) -> String {
    if let Some(prim) = primitive {
        if let Some(llvm) = config.lookup(prim, bytes) {
            return llvm.to_string();
        }
    }
    format!("i{}", bytes * 8)  // raw Bits(N) — always correct
}
```

## Decision Tree (Every Backend)

```
For any Type value:

1. Resolve to ResolvedType via TypeUniverse
   └── Not found → treat as raw Bits(8) — always safe

2. Read bytes from ResolvedType.bytes (or default 8)
   └── Every backend uses this for storage layout

3. Read properties["primitive"] (optional, from source only)
   └── LLVM: combine with config to get LLVM type string
   └── Webstack: use for JS type mapping
   └── CIRCT: IGNORE — just use bytes + alignment
   └── Not present → use bytes only (correct for any type)

4. Read other properties for backend-specific behavior
   └── encoding: "utf-8" → string operations
   └── alignment: 16 → vectorized load/store
   └── Any backend-specific metadata
```

## Backend Independence

| Backend | Reads | Ignores | Correct without primitive? |
|---------|-------|---------|---------------------------|
| **LLVM** | `bytes`, `primitive` + config | — | Yes — raw Bits(N) with i{N*8} |
| **CIRCT** | `bytes`, `alignment` | `primitive` entirely | Yes — always correct |
| **GPU** | `bytes` | Everything except subgroup barriers | Yes — compute doesn't need types |
| **Webstack** | `bytes`, `primitive` | Config overrides | Yes — raw typed array views |
| **Any new** | `bytes` minimum | Whatever isn't relevant | Yes — Bits(N) is universal |

A new backend starts with just `bytes` and is fully correct.
It then opts into `primitive`, `encoding`, and other metadata as needed,
one property at a time, without touching any other backend.

## No Fallback Tables

There is NO `builtin_resolved` function. There is NO hardcoded `"Int" → i64` mapping anywhere.
The only way a type gets semantics is through source declarations that populate the universe.

`Type::int()`, `Type::float()`, etc. are pure name constructors — they create `Custom("Int")`,
`Custom("Float")` with zero semantics attached. Resolution comes from the universe or not at all.
