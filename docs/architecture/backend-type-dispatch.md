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

- **`bytes <~ N`** — Every backend reads this. `Bits(8)` = 64-bit storage.
- **`primitive <~ PascalCase`** — Optional semantic hint for backends.
  Declared in source. NEVER hardcoded in Rust.
- **Other metadata** — Any backend uses whatever metadata it needs.

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

The `(primitive, bytes) → LLVM type string` mapping lives in a standalone config file.

```toml
# config/llvm-primitives.toml
[primitive.Int]
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

[primitive.String]
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

4. Read other metadata for backend-specific behavior
```

## Backend Independence

| Backend | Reads | Ignores |
|---------|-------|---------|
| **LLVM** | `bytes`, `primitive` + config | — |
| **CIRCT** | `bytes`, `alignment` | `primitive` entirely |
| **GPU** | `bytes` | Everything else |
| **Webstack** | `bytes`, `primitive` | Config overrides |
| **Any new** | `bytes` (minimum) | Whatever isn't relevant |

A new backend starts with just `bytes` and is fully correct.
It opts into `primitive` and other metadata as needed.

## No Fallback Tables

There is NO `builtin_resolved` function. There is NO hardcoded `"Int" → i64` mapping anywhere.
The only way a type gets semantics is through source declarations that populate the universe.

`Type::int()`, `Type::float()`, etc. are pure name constructors — they create `Custom("Int")`,
`Custom("Float")` with zero semantics attached. Resolution comes from the universe or not at all.
