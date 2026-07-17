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
type Int <: Bits { bytes <~ 8; ctd <~ Int; alu <~ Int; }
type Float <: Bits { bytes <~ 4; ctd <~ Float; alu <~ Float; }
type String <: Bits { bytes <~ 24; ctd <~ String; alu <~ Int; encoding <~ "utf-8"; }
```

- **`bytes <~ N`** — Every backend reads this. `Bits(8)` = 64-bit storage. Always sufficient.
- **`ctd <~ PascalCase`** — Common Type Definition. What the type *is* semantically (exhaustive closed set: `Int`, `UInt`, `Float`, `Double`, `Bool`, `Char`, `String`, `Data`, `Ptr`, `Void`). The normalizer maps CTD to backend-specific types.
- **`alu <~ PascalCase` or `alu <~ "quoted"`** — What hardware computes with values of this type. PascalCase for known ALUs (`Int`, `Float`, `Bool`), lowercase-quoted for backend/plugin-specific hardware.
- **Other metadata** — Any backend is free to use any metadata it needs. Unrecognized metadata is silently ignored.

## Frontend/Backend Detachment

The frontend (parser + AST), normalizer, and backend are coupled through a narrow interface:

| Layer | Owns | Exposes | Backend sees |
|-------|------|---------|--------------|
| **Source** | Type definitions | `bootstrap.bv` | — |
| **Parser** | Syntax + AST construction | `TypeDefBody`, `TypeDefSlot`, `PropertyValue` | — |
| **TypeUniverse** | Type resolution | `ResolvedType { bytes, properties, name, base, alignment }` | `resolve_type()` |
| **Normalizer** | CTD → LLVM type mapping | `llvm_type` property on every `ResolvedType` | `properties["llvm_type"]` |
| **Backend** | IR emission | — | Everything above, read-only |

**The frontend never calls backend code.** The backend never modifies the AST.
The contract is: **frontend provides metadata and CTD, normalizer maps to backend types, backend consumes.**

A metadata slot added to a type definition in source is automatically visible to every backend.
No Rust changes needed. No recompilation. Example:

```brief
type HalfFloat <: Bits { bytes <~ 2; ctd <~ Float; alu <~ Float; }
```

The parser stores `ctd <~ Float` in `TypeDefBody.metadata["ctd"]`.
The universe reads it into `ResolvedType.properties["ctd"] = PropertyValue::Identifier("Float")`.
The LLVM normalizer maps CTD `"Float"` → LLVM type `"half"` via `ctd_to_llvm()`.
The LLVM backend reads `properties["llvm_type"]` — no recomputation needed.
The CIRCT backend ignores CTD, reads `bytes=2` → emits 16 wires.

**Zero Rust changes across the entire pipeline.**

## How Each Backend Interprets Metadata

Every backend receives the same `ResolvedType { bytes, properties, ... }`.
Each backend reads what it needs and ignores the rest.

### LLVM Backend

| Metadata | What it reads | What it emits |
|----------|--------------|---------------|
| `properties["llvm_type"]` | Set by normalizer; direct read, no recomputation | LLVM type string (`"i64"`, `"float"`, `"ptr"`, ...) |
| `properties["alu"] == "Float"` | Determines float arithmetic vs integer | `fadd`/`fsub`/`fmul` vs `add`/`sub`/`mul` |
| `properties["ctd"] == "String"` | Enables string-specific helpers | `__int_to_str__`, `__str_to_int`, `@ll_empty_list` |
| `properties["encoding"]` | String/Data encoding | — (future: null vs length-prefixed) |
| `bytes` | Storage width | `alloca`, `malloc` size, GEP offsets |
| `alignment` | Memory alignment | `align N` attribute on `alloca`/`store` |
| No `ctd` + no `llvm_type` | Falls back to `derive_llvm_type(None, bytes)` | `format!("i{}", bytes * 8)` — structural only |

### CIRCT (Hardware) Backend

| Metadata | What it reads | What it emits |
|----------|--------------|---------------|
| `bytes` | Bit width | `hw.param.value` width, Verilog port width |
| `alignment` | — | (ignored — hardware has native alignment) |
| `properties["ctd"]` / `properties["alu"]` | **IGNORED** | Not needed — hardware doesn't care if it's Int or String |
| `properties["encoding"]` | **IGNORED** | Raw bits only |
| No `ctd` or `alu` | Works correctly | Raw Bits(N) — always sufficient |

**CIRCT example**: `String` with `bytes=8` emits as 64 wires, same as `Int` with `bytes=8`.
The hardware doesn't distinguish between the two — it's the programmer's responsibility to
ensure the connected hardware interprets the bits correctly. This is correct by design.

### GPU Backend

| Metadata | What it reads | What it emits |
|----------|--------------|---------------|
| `bytes` | Storage width | `i32`, `i64` for buffer bindings |
| `properties["ctd"]` or `properties["alu"]` | **Selective** — only checks for subgroup barriers | Barrier intrinsics |
| Everything else | **IGNORED** | Raw bits for compute |
| No `ctd` or `alu` | Works correctly | Raw bits — GPU compute doesn't need types |

### Webstack (WASM + JS) Backend

| Metadata | What it reads | What it emits |
|----------|--------------|---------------|
| `bytes` | Storage width | WASM local type (`i32`, `i64`, `f32`, `f64`) |
| `properties["ctd"]` | JS type mapping | `BigInt64Array` vs `Float64Array` vs `TextEncoder` |
| `properties["encoding"]` | String serialization | `TextEncoder.encode()` vs `TextDecoder.decode()` |
| No `ctd` | Falls back to `bytes` | Raw typed array view — always correct |

### New Backend (Hypothetical)

| Metadata | What it reads | What it emits |
|----------|--------------|---------------|
| `bytes` | Minimum requirement | Any width-based type system |
| `properties["ctd"]` / `properties["alu"]` | Optional: semantic hints | Backend-specific IR patterns |
| Everything else | Whatever it needs | Fully extensible |
| No `ctd` or `alu` | Works correctly | Raw Bits(N) — enough to be functional |

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
    ctd <~ UInt;
    alu <~ Int;
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
    ctd: Some("Int"),              bytes = 8
    alu: Some("Int"),              ctd = None, alu = None
    ...                           normalizer falls back to
  }                               derive_llvm_type(None, 8) → "i64"
```

The Rust compiler NEVER matches the string `"Int"` or `"Float"` to infer semantics.
`Type::int()` is a name constructor — it creates `Custom("Int")` with NO semantics.
Semantics come from the universe, populated by source declarations.

## The LLVM Type Config File

The `(ctd, bytes) → LLVM type string` mapping lives in a standalone config file,
read at compile time by `TypeConfig::load()`. The normalizer's `ctd_to_llvm()`
function handles the primary mapping; the config file provides the fallback
via `derive_llvm_type()`.

```toml
# config/ctd-llvm-mappings.toml
[ctd.Int]
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
