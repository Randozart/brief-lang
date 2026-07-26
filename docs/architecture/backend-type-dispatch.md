# Backend Type Dispatch Architecture

> **2026-07-20:** This document describes the superseded CTD/ALU/config
> architecture. See `docs/architecture/casting-protocol.md` for the current
> hashword-based system. Key changes:
>
> - `ctd` and `alu` metadata: **removed** — hashwords in op signatures replace them
> - `config/llvm-ops.toml`: **removed** — backend has intrinsic `#Category` knowledge
> - `config/ctd-llvm-mappings.toml`: **removed** — `llvm_type` derived from structure
> - `op Add ~> "int.add"` (string binding): **replaced** by `op Add(#Int, #Int)`
>
> The document is retained for historical reference during the transition.

## Design Philosophy

The Brief compiler does NOT hardcode primitive type mappings in Rust match arms.
Type metadata declared in **source** drives all backend emission decisions.
The Rust binary is a *thin reader* of the type system defined in Brief's own bootstrap files.

If the prelude (`bootstrap.bv`) is not loaded, every type is raw `Bits(N)` — no assumptions,
no guesswork. The programmer defines their own types, or loads the prelude.

## The Core Insight

Every type is `Bits(N)` at minimum:

```brief
type Int <: Bits { maxbits <~ 64; ctd <~ Int; alu <~ Int; op Add ~> "int.add"; }
type Float <: Bits { maxbits <~ 32; ctd <~ Float; alu <~ Float; op Add ~> "float.add"; }
type String { data: Int; len: Int; encoding <~ "UTF-8"; tbaa <~ "String"; };
```

- **`maxbits <~ N`** — Every backend reads this. `maxbits=64` = 64-bit storage. For struct types with `fields`, maxbits is derived from field type sizes (summed). Explicit `maxbits <~ N` overrides the derivation.
- **`ctd <~ PascalCase`** — Common Type Definition. What the type *is* semantically (exhaustive closed set: `Int`, `UInt`, `Float`, `Double`, `Bool`, `Char`, `String`, `Data`, `Ptr`, `Void`). The normalizer maps CTD to backend-specific types. Inherited from the primordial when not set in source.
- **`alu <~ PascalCase` or `alu <~ "quoted"`** — What hardware computes with values of this type. PascalCase for known ALUs (`Int`, `Float`, `Bool`), lowercase-quoted for backend/plugin-specific hardware.
- **`fields: Vec<(String, Type)>`** — Struct field declarations on `ResolvedType`. Populated from `TypeDef.body.slots` by the normalizer. Drives LLVM struct type lowering, state slot width, and `is_string_like()` detection. Example: String with `data: Int; len: Int;` → `fields = [("data", Int), ("len", Int)]`.
- **`op Add ~> "int.add"`** — Operator binding to a generic backend-agnostic identifier. The typechecker reads this via `get_operator_intrinsic(universe, "+", &Int)`, which returns `OpBinding::Function("int.add")`. The backend then looks up `("Add", "Int", 8)` in `config/llvm-ops.toml` for the LLVM IR template.
- **`encoding <~ "UTF-8"`** — String encoding, resolved through `config/encodings.toml`. All encoding names are quoted strings — no PascalCase hardcoded table. The config specifies `char_width` (for Index# GEP eligibility) and optional `ops.index_at`/`char_len` (stdlib functions for runtime dispatch).
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
type HalfFloat <: Bits { maxbits <~ 16; ctd <~ Float; alu <~ Float; }
```

The parser stores `ctd <~ Float` in `TypeDefBody.metadata["ctd"]`.
The universe reads it into `ResolvedType.properties["ctd"] = PropertyValue::Identifier("Float")`.
The LLVM normalizer maps CTD `"Float"` → LLVM type `"half"` via `ctd_to_llvm()`.
The LLVM backend reads `properties["llvm_type"]` — no recomputation needed.
The CIRCT backend ignores CTD, reads `maxbits=16` → emits 16 wires.

**Zero Rust changes across the entire pipeline.**

## How Each Backend Interprets Metadata

Every backend receives the same `ResolvedType { bytes, properties, ... }`.
Each backend reads what it needs and ignores the rest.

### LLVM Backend

| Metadata | What it reads | What it emits |
|----------|--------------|---------------|
| `properties["llvm_type"]` | Set by normalizer; direct read, no recomputation | LLVM type string (`"i64"`, `"float"`, `"ptr"`, ...) |
| `properties["alu"] == "Float"` | Determines float arithmetic vs integer | `fadd`/`fsub`/`fmul` vs `add`/`sub`/`mul` |
| `is_string_like(ty, universe)` | Shape (2 Int fields) + encoding property | SSO handle or heap-allocated string helpers |
| `is_vector_like(ty, universe)` | Has `op.SVO <~ N` metadata | SVO inline list handle (N+1 slot struct) |
| `svo_capacity(ty, universe)` | Reads `N` from `op.SVO` metadata | Number of inline elements before heap promotion |
| `properties["encoding"]` | Looked up in `config/encodings.toml` for `char_width` and stdlib ops | `Index#` emits GEP (fixed-width) or stdlib call (variable-width) |
| `properties["op.Add"]` etc. | Generic identifier used for config dispatch | `OP_CONFIG.lookup("Add", "Int", 8)` → template fill |
| `bytes` | Storage width. Derived from fields for struct types | `alloca`, `malloc` size, GEP offsets |
| `alignment` | Memory alignment | `align N` attribute on `alloca`/`store` |
| `fields` | Struct field list on `ResolvedType` | LLVM `{ i64, i64 }` type, `extractvalue`/`insertvalue` |
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
    maxbits <~ 32;
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

## Operator Dispatch (Phase 0)

Operator dispatch follows a layered approach with increasingly generic fallbacks:

```
                 Source: bootstrap.bv
                    op Add ~> "int.add"
                         ↓
              Parser: metadata["op.Add"] = "int.add"
                         ↓
              Universe: ResolvedType.properties
                         ↓
        Typechecker: get_operator_intrinsic(universe, "+", &Int)
            ┌───────────┴───────────┐
            ↓                       ↓
    universe["op.Add"] exists    fallback: builtin_operator_binding()
    → OpBinding::Function        → OpBinding::Intrinsic("AddI64#")
            ↓                       ↓
        Backend: emit_binop_from_config()
    OP_CONFIG.lookup("Add", "Int", 8)
            ↓
    template: "add nsw i64 %a, %b" → fill operands
            ↓
    Fallback: hardcoded BinaryOpKind matches in emit_binary_op()
```

The `config/llvm-ops.toml` file is the primary dispatch for the LLVM backend.
The hardcoded `builtin_operator_binding()` table (in `operators.rs`) is a
typechecker fallback for types without universe entries (e.g. during tests
or `--no-stdlib` mode). The hardcoded `emit_binary_op` match arms are a
codegen fallback when config lookup returns None.

## Encoding Dispatch

All encoding names are quoted strings resolved through `config/encodings.toml`:

```
encoding <~ "UTF-8"      → config/encodings.toml (char_width=0, ops.index_at, ops.char_len)
encoding <~ "ASCII"      → config/encodings.toml (char_width=1, no ops — direct GEP)
encoding <~ "shift_jis"  → config/encodings.toml (char_width=0, ops.index_at, ops.char_len)
```

No PascalCase hardcoded table. The `char_width` field tells the compiler
whether `Index#` can emit a direct GEP (fixed-width, char_width > 0) or
must delegate to a stdlib function (variable-width, char_width = 0).
The `ops` map specifies which stdlib functions to call for encoding-aware
operations.

## No Fallback Tables

There is NO `builtin_resolved` function. There is NO hardcoded `"Int" → i64` mapping anywhere.
The only way a type gets semantics is through source declarations that populate the universe.

`Type::int()`, `Type::float()`, etc. are pure name constructors — they create `Custom("Int")`,
`Custom("Float")` with zero semantics attached. Resolution comes from the universe or not at all.

---

## String Type Dispatch (SSO)

With `feature_sso_strings = true`, String is lowered as `{ i64, i64 }` instead
of `ptr`. The `llvm_type` override in `emit_toplevel.rs` checks for
`name == "String" && feature_sso_strings` and returns `"{ i64, i64 }"`.
`push_field_type` allocates 2 state slots per String field.

When SSO is OFF, String is a single `i64` (ptrtoint of heap/stack buffer).
The `.#data` and `.#len` field access uses layout field read (bit offset
from the type's field properties), which works for both representations
via `Load#(addr + offset, width)`.

## UTF8View Dispatch

`UTF8View` is always `{ i64, i64 }` regardless of `feature_sso_strings`.
It is excluded from `type_is_heap_allocated` (never owns memory). Always
2 state slots. The `encoding` property is hardcoded to `"UTF-8"`.

## SmallString64 Dispatch

`SmallString64` has 9 Int fields (60 bytes data + length), making the LLVM
type `{ i64 x 9 }` = 72 bytes. Not detected by `is_string_like()` (needs
exactly 2 Int fields). Never heap-allocated. Operations read/write bytes
from individual slots using `when`-chained slot selection based on `i / 8`.

## Vector Type Dispatch (SVO)

With `feature_svo = true` and `op.SVO <~ N` metadata on a type, the type is
detected as vector-like via `is_vector_like()`. The LLVM type becomes
`{ i64 x (N+1) }` — N data slots + 1 len+cap+tag slot. `push_field_type`
allocates N+1 state slots per field.

Tag bit 0 of the last slot: `1` = inline, `0` = heap (ptr in slot 0).
Inline elements are stored directly in the struct slots. Heap elements use
the existing ptr/len/cap format in slots 0..2.

Indexing uses a stack array + GEP for dynamic indices (extractvalue requires
constant indices). The tag branch selects inline vs heap access.
