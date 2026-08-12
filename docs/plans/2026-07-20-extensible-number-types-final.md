# Extensible Number Types — Final Architecture

**Date:** 2026-07-20
**Status:** Plan — ready for implementation

---

## The Core Question

How can a programmer define a type (Posit32, ASCIIString, Bfloat16, Decimal64) in pure Briev —
layout + operations — and have the backend compile it without hardcoded compiler knowledge,
without TOML config files, and with LLVM optimizing the result as well as equivalent C code?

## The Answer

**Types are layout + ops. Hashwords (`#Category`) are backend directives, not type
declarations. A type never "belongs to" a category — it interacts with one through
its op signatures.**

## Architecture

### Layer 0: Bits (the only primitive)

```briev
// Exists implicitly. No layout. Only bitwise ops.
op And(#Bits, #Bits);
op Or(#Bits, #Bits);
op Xor(#Bits, #Bits);
op Not(#Bits);
op Shl(#Bits, #Bits);
op Shr(#Bits, #Bits);
```

Every type implicitly inherits from Bits. No explicit `: Bits` needed.

### Layer 1: Structure (fields determine layout)

A type's layout is determined by its fields — not by metadata properties:

```briev
type ASCIIString {
    data: Bits<64>;     // pointer
    len: Bits<64>;      // length
    op Add(#String) = ASCII_concat(#L, #R);
};
```

The normalizer computes `bytes = 16` from the fields. `llvm_type = "{ i64, i64 }"`.
No `bytes <~`, no `alignment <~`, no `ctd <~`, no `alu <~` metadata needed.

For flat numeric types without field syntax:

```briev
type Bfloat16 { data: Bits<16>; };
// normalizer: bytes=2, llvm_type="i16" (raw bits)
// With op Add(#Float): backend knows to use bfloat hardware
```

### Layer 2: Operations (ops define interaction)

```briev
type Bfloat16 {
    data: Bits<16>;
    op Add(#Float, #Float) = bfloat_add(#L, #R);  // backend intrinsic
    op Mul(#Float, #Float) = bfloat_mul(#L, #R);
};

type Posit32 {
    data: Bits<32>;
    // No hashword ops — fully defined through explicit defn bindings
    op Add(Posit32) = Posit32_add(#L, #R);
    op Mul(Posit32) = Posit32_mul(#L, #R);
};
```

Hashwords in op signatures are BACKEND DIRECTIVES:
- `op Add(#Int, #Int)` = "backend, handle this as integer addition — you know what that is"
- `op Add(#Float, #Float)` = "backend, handle this as float addition — you know what that is"
- `op Add(#String, #String)` = "backend, handle this as string concatenation"

No TOML config needed. The backend has intrinsic knowledge of `#Category` operations.

### Layer 3: No TOML Config

| File | Status | Why |
|---|---|---|
| `config/llvm-ops.toml` | **Removed** | Backend has intrinsic knowledge of `#Category` ops |
| `config/ctd-llvm-mappings.toml` | **Removed** | Normalizer derives `llvm_type` from structure alone |
| `config/targets.toml` | **Kept** | Target selection, plugin wiring, compiler rewiring |

The only op mechanism that isn't backend-intrinsic is `op Add(Posit32) = fn(#L, #R)` —
explicit bindings to `defn` functions, which are auto-`alwaysinline`.

### Layer 4: Hashword Property Access (`#Category.#property`)

`#Category :> property` extracts compile-time structural properties:

| Expression | Resolves to | LLVM emission |
|---|---|---|
| `#Float.#bytes` | `4` | `add i64 0, 4` (constant) |
| `#String.#fields.0` | data pointer field | `extractvalue {i64, i64} %reg, 0` |
| `#String.#fields.1` | length field | `extractvalue {i64, i64} %reg, 1` |
| `#String.#Fields(0,1)` | both fields | `extractvalue` pair |

The `:>` projection system already exists (`list .#Size`, `val .#Bytes`).
`#Category.#property` extends it: the source is a hashword category, not a value.

### Layer 5: Protocol Ops (Category Universal Currency)

Every hashword category defines a set of ops that ALL backends must implement.
Types declare `CastTo(#Category)` and `CastFrom(#Category)` to produce and
consume the protocol shape — no intermediate currency needed.

| Category | Protocol ops | Protocol shape |
|---|---|---|
| `#String` | `CastTo(#Char)`, `CastFrom(#Char)`, `Extract(#Char)`, `InsertAt(#Char)`, `Concat(#String)`, `.#Size`, `CastTo(#Bits)`, `CastFrom(#Bits)` | UTF-8 byte sequence |
| `#Char` | `CastTo(#Int)`, `CastFrom(#Int)`, `Eq(#Char)`, `Lt(#Char)` | Unicode scalar (i32) |
| `#Int` | `CastTo(#Bits)`, `CastFrom(#Bits)`, `Add(#Int)`, `Sub(#Int)`, `Mul(#Int)`, `Div(#Int)`, `And(#Bits)`, `Not(#Bits)` | Two's complement i64 |
| `#Float` | `CastTo(Float64)`, `CastFrom(Float64)`, `Add(#Float)`, `Mul(#Float)`, `Sqrt(#Float)`, `CastTo(#Bits)`, `CastFrom(#Bits)` | IEEE 754 binary32/64 |
| `#Bits` | `And(#Bits)`, `Or(#Bits)`, `Xor(#Bits)`, `Not(#Bits)`, `Shl(#Bits)`, `Shr(#Bits)` | Raw iN |

A conversion function between two `#String` types speaks `Char` — the universal text
currency. The backend's `Extract(#Char)` and `InsertAt(#Char)` decode/encode at the
boundary, hiding the internal encoding.

```briev
inline defn any_string_to_ASCII(source: #String) -> ASCIIString {
    let len = source .#Size;
    let result = ASCIIString::alloc(len);
    let mut i = 0;
    do {
        let c: Char = source :> Extract(i);
        result :> InsertAt(i, c);
        i = i + 1;
    } while i < len;
    result
};
```

Float conversion goes through the `CastTo`/`CastFrom` pair directly:

```briev
inline defn any_float_to_posit(source: #Float) -> Posit32 {
    let intermediate: Float64 = source :> Cast(Float64);
    posit_from_double(intermediate)
};
```

### Layer 6: Inheritance (`<:`)

```briev
type ASCIIString : String {
    op Add(ASCIIString) = ASCII_add(#L, #R);  // override String::Add
};
```

Inherits all of String's ops. Overrides only what's declared. String is a primordial
type that exists by default — the user overwrites it by redeclaring.

---

## What Gets Removed

### From stdlib .bv files

```diff
 type Int : Bits {
-    bytes <~ 8;
-    alignment <~ 8;
-    llvm <~ "i64";
-    tbaa <~ "Int";
-    default_width <~ 64;
-    commuting <~ true;
-    op Add ~> "int.add";
-    op Sub ~> "int.sub";
+    // Layout: implicit from Bits<64> or equivalent
+    op Add(#Int, #Int);
+    op Sub(#Int, #Int);
+    op Mul(#Int, #Int);
+    op Div(#Int, #Int);
+    op And(#Bits, #Bits);
+    op Or(#Bits, #Bits);
+    op Xor(#Bits, #Bits);
+    op Not(#Bits);
+    op Shl(#Bits, #Bits);
+    op Shr(#Bits, #Bits);
+    op Cast(#Bits);
 };
```

No `llvm <~`. No `alu <~`. No `op ... ~> "..."` string names. Op signatures use
`#Category` hashwords. The backend decides what `add i64` means.

### From the compiler

- `config/llvm-ops.toml` — **removed entirely**
- `config/ctd-llvm-mappings.toml` — **removed entirely**
- `src/config.rs` — `OpConfig`, `TypeConfig` types **removed** (or reduced to empty)
- `ctd_to_llvm()` hardcoded table — **replaced** with structure-driven inference
- `alu` property — **removed** from the keep list, no longer stamped or read
- `category` property — **removed** from the normalizer (types don't belong to categories)
- `operator_llvm_type()` — **simplified** to read `llvm_type` from structure
- `is_native_float()` — **simplified** to check op signatures, not properties
- `derive_llvm_type()` config fallback — **removed** with TOML config

### What stays

- `config/targets.toml` — target selection, plugin wiring
- `config/module-registry.toml` — import path resolution
- `config/alloc-strategies.toml` — allocation templates

## What the Normalizer Does (Revised)

The normalizer's job shrinks to:

1. **Compute `llvm_type` from structure**: fields determine layout. No metadata needed.
   - Struct with two `Bits<64>` fields → `{ i64, i64 }`
   - Single `Bits<N>` field → `i{N}` (e.g., `Bits<32>` → `i32`)
   - 2-byte struct with `op Add(#Float)` → `bfloat` or `half` based on naming convention
   - Explicit `llvm <~ "..."` override validated and used as-is

2. **Parse layout + compute bytes/alignment** from field structure (already exists)

3. **Stamp `llvm_type` on every type** (already exists, just simplified)

4. **Validate op signatures**: hashword categories must be recognized by the backend.
   Unknown hashwords (`#MyCustomCategory`) produce a compiler error
   ("backend does not understand #MyCustomCategory").

5. **Metadata keep list**: stripped down to just `llvm_type`, `fields`, and `tbaa`.

## Implementation Phases

### Phase 1: Simplify Normalizer (remove metadata-driven inference)

Remove all metadata properties that are no longer needed:
- Remove `alu` from the keep list
- Remove `ctd` from the keep list
- Remove `category` inference (types don't belong to categories)
- Remove `encoding` from the keep list
- Remove `tbaa_parent` (for now — revisit in Phase 5)
- Simplify `llvm_type` computation to use structure only

**Rationale:** `ctd` was the old identity system. `alu` was the old hardware dispatch.
`category` was our intermediate attempt at structural inference. None are needed —
hashwords in op signatures replace all three.

### Phase 2: Remove TOML Config Files

- Delete `config/llvm-ops.toml` — backend has intrinsic knowledge of `#Category` ops
- Delete `config/ctd-llvm-mappings.toml` — normalizer derives `llvm_type` from structure
- Remove `OpConfig::load()` and `TypeConfig::load()` calls from codegen
- Remove `derive_llvm_type()` (no more TOML fallback)
- Simplify `ctd_to_llvm()` — or remove entirely, replaced by structure-driven inference

### Phase 3: Rewrite Stdlib Type Declarations

Remove all metadata properties from stdlib `.bv` files:
- Remove `bytes <~`, `alignment <~`, `llvm <~`, `tbaa <~`, `default_width <~`, `commuting <~`
- Replace `op Add ~> "int.add"` with `op Add(#Int, #Int)`
- Replace all named op bindings with hashword category directives

The stdlib becomes purely about what ops each type declares, not how they're implemented.

### Phase 4: Rewrite Op Signatures in Backend (Hashword Dispatch)

The codegen's op dispatch changes from:
```
look up op template in TOML config → emit template string
```
To:
```
read op signature → if RHS type is #Category → emit backend intrinsic
                      else → emit call to bound defn (auto-alwaysinline)
```

The backend has intrinsic handlers for each `#Category` kenn it recognizes:
- `emit_int_binop(op, regs, llvm_ty)` for `#Int` ops
- `emit_float_binop(op, regs, llvm_ty)` for `#Float` ops
- `emit_string_op(op, regs, llvm_ty)` for `#String` ops
- etc.

### Phase 5: Protocol Ops Validation

The typechecker validates that `#Category` types implement the required protocol ops:
- `#String` types must `op Extract(#Char)` and `op InsertAt(#Char)`
- `#Float` types must `op Cast(Float64)`
- etc.

This is a validation pass, not a codegen concern.

### Phase 6: Extend `:>` Projection for Hashwords

Extend the existing `:>` projection system to work with hashword categories:
- `#String.#fields.0` — compile-time field extraction
- `#Float.#bytes` — compile-time byte width
- `#Type.#llvm` — compile-time LLVM type string

### Phase 7: Target Config (<:> targets.toml)

Extend `config/targets.toml` to declare which hashwords a backend supports:
```toml
[".bv"]
backend = "llvm"
hashwords = ["#Int", "#Float", "#Bool", "#Char", "#String", "#Bits"]
```

A type declaring `op Add(#MyCustomHashword)` on a backend that doesn't recognize
`#MyCustomHashword` produces a compile error.

---

## Documentation Updates

| Document | What changes |
|---|---|
| `docs/architecture/overview.md` | Architecture section — add type-ops-hashword model |
| `docs/architecture/intrinsics-vs-stdlib.md` | Hashwords replace TOML config |
| `docs/architecture/backend-type-dispatch.md` | Complete rewrite — hashword dispatch |
| `docs/architecture/hash-words.md` | Add `#Category` semantics, `:#property` access |
| `docs/plans/2026-07-20-extensible-number-types-final.md` | This file |

## Key Decisions (for Review)

| Decision | What it replaces | Why |
|---|---|---|
| Hashwords are backend directives | TOML op templates | Backend has intrinsic knowledge of its own primitives |
| Structure determines layout | `bytes <~`, `ctd <~`, `alu <~` | A type IS its fields — metadata is optional hints |
| Types don't belong to categories | Category inference | A type interacts with `#Int` when its ops say so |
| CastTo/CastFrom pair handles conversion | Ad-hoc conversion logic | Direct `CastTo`/`CastFrom` inlining |
| `:>` is compile-time property access | Hardcoded projection list | Every type's fields are accessible generically |
| `defn` bound to `op` = auto-`alwaysinline` | `inline` keyword | The binding declares the intent |
| TOML removed except `targets.toml` | Full config directory | The config was bridging a gap that doesn't exist |

---

## Protocol Shapes and Variants

Each hashword category can have multiple protocol variants. The file extension
determines the default (`.bv` → UTF8/unicode, `.ebv` → ASCII).

| Hashword | Variants | Default (`.bv`) | Default (`.ebv`) | Protocol ops |
|---|---|---|---|---|
| `#Int` | *(none)* | — | — | Add, Sub, Mul, Div, And, Or, Xor, Not, Shl, Shr |
| `#Float` | `IEEE754`, `bin32`, `bin64` | `IEEE754` | `IEEE754` | Add, Sub, Mul, Div, Sqrt, FMA, Cast(Float64) |
| `#Bool` | *(none)* | — | — | And, Or, Not |
| `#Char` | `unicode`, `ASCII` | `unicode` | `ASCII` | Cast(#Int), Eq, Lt |
| `#String` | `UTF8`, `ASCII`, `hex`, `base64` | `UTF8` | `ASCII` | Extract(#Char), InsertAt(#Char), Concat(#String), `.#Size` |
| `#Bits` | *(none)* | — | — | And, Or, Xor, Not, Shl, Shr, Cast(N) |

**Cross-variant calls require explicit protocol:**

```briev
fn cross(a: #String<UTF8>, b: #String<ASCII>) { ... };
```

The compiler errors if a `.bv` file calls a `.ebv` function using `#String`
without specifying the variant. Backends declare supported protocols in
`config/targets.toml`:

```toml
[target.desktop]
protocols = ["#String<UTF8>", "#Float<IEEE754>", "#Int", "#Bool", "#Bits"]

[target.embedded-riscv]
protocols = ["#String<ASCII>", "#Int", "#Bool", "#Bits"]
```

A function requiring a protocol the target doesn't support produces a compile
error listing available alternatives.

---

## File Change Audit (Complete)

Audit conducted 2026-07-20. Every code site referencing the old CTD/ALU/TOML
architecture, organized by phase.

### Phase 1: Normalizer Simplification

| File | Change | Est. Lines |
|---|---|---|
| `src/backend/llvm/normalizer.rs` | Remove `ctd_to_llvm()`, `validate_alu_ctd()`, `infer_category()`. Strip `ctd`/`alu`/`category`/`encoding` from keep list. Remove Pass 1 ALU×CTD validation. Simplify `llvm_type` to derive from structure only. | ~100 removed, ~50 rewritten |
| `src/backend/llvm/normalizer.rs` (imports) | Remove `OpConfig::load()`, `TypeConfig::load()`, `derive_llvm_type()` | 3 lines |

### Phase 2: TOML Config Removal

**Files to delete:**
- `config/ctd-llvm-mappings.toml` (36 lines)
- `config/llvm-ops.toml` (222 lines)
- `config/spirv-ops.toml` (78 lines)

**Rust files to gut/remove:**
| File | Change | Lines |
|---|---|---|
| `src/config.rs` | Remove completely — `TypeConfig`, `OpConfig`, `derive_llvm_type()`, `derive_alu_type()` | 210 |
| `src/config_resolver.rs` | Remove completely — config file resolution pipeline | 279 |

**Files to keep (orthogonal):**
- `config/targets.toml` — backend routing
- `config/alloc-strategies.toml` — allocation templates
- `config/module-registry.toml` — import resolution
- `config/address-map.toml` — address mapping for `AddressOf#`

### Phase 3: Intrinsic / Codegen Rewrite

| File | Change | Lines |
|---|---|---|
| `src/backend/llvm/intrinsics.rs` | Remove `OP_CONFIG` static, replace config lookup with hashword category dispatch | ~50 |
| `src/backend/llvm/emit_expr.rs` | `emit_binop_from_config()` — replace with hashword dispatch | ~50 |
| `src/backend/llvm/helpers.rs` | Remove `ctd`/`alu` fallback reads in `rt_llvm_type()`, `operator_llvm_type()`, `emit_operator_call()` | ~30 |
| `src/backend/llvm/emit_toplevel.rs` | Remove `ctd` fallback in `rt_llvm_type()` | ~10 |

### Phase 4: Primordial / Stdlib Migration

| File | Change | Lines |
|---|---|---|
| `src/type_universe/mod.rs` | Stop setting `ctd`/`alu` in primordial seeds. Remove `default_alu()`. Replace `is_string_like()` with hashword variant check. | ~30 |
| `lib/std/types/bootstrap.bv` | Replace `op Add ~> "int.add"` with `op Add(#Int)` — ~121 occurrences | 121 replacements |

### Phase 5: Other Backend Normalizers

| File | Change | Lines |
|---|---|---|
| `src/backend/webstack_normalizer.rs` | Remove `ctd`-based `js_type` derivation | ~20 |
| `src/backend/spirv/normalizer.rs` | Remove `derive_alu_type()` fallback, `ctd` reads | ~20 |

### Summary

| Metric | Count |
|---|---|
| Files modified | ~15 |
| Files deleted (config) | 3 |
| Files deleted (Rust) | 2 (`config.rs`, `config_resolver.rs`) |
| Lines removed | ~900 |
| Lines rewritten | ~300 |
| Total churn | ~1,400 lines |

---

## Old Syntax Cleanup

After implementing parser support for `op Add(#Int, #Int)` hashword syntax,
the old `op Add ~> "int.add"` string-binding syntax must be removed.

### Step 1: Stdlib `.bv` files — 121 replacements

**`lib/std/types/bootstrap.bv`** — Replace all `op Xxx ~> "type.op"` bindings
with hashword syntax. Every integer type gets `(#Int, #Int)` for arithmetic,
every float type gets `(#Float, #Float)`, Bool gets `(#Bool, #Bool)`, Char
gets `(#Char, #Char)`.

**`lib/std/types/float.bv`** — Same for Float16 and Float4 vector types.

### Step 2: Parser — remove `<~` fallback in `parse_op_binding()`

Remove the string-binding fallback path. `op` declarations now REQUIRE
parenthesized parameter types. The `metadata` parameter is removed from
`parse_op_binding()` and its call sites, since old-style metadata insertion
(`metadata["op.Add"] = PropertyValue::String("int.add")`) is no longer needed.

### Step 3: Normalizer — remove `op.*` from metadata retention

The `rt.properties.retain(|k, _| keep.contains(k) || k.starts_with("op."))`
clause is dead after the migration. Remove `|| k.starts_with("op.")`.

### Step 4: Operators — remove `metadata["op.Add"]` fallback

Remove any code that reads `metadata["op.Add"]` from type resolver functions.
All op dispatch now goes through `TypeDefBody.operators` structs.

---

## `disamb` Disambiguation Hint

Resolved decision from Open Questions §1 (2-byte float ambiguity).

The `disamb <~ "value"` metadata property is a **hint** to the normalizer when
structure + bytes + `#Category` ops are insufficient to determine the concrete
representation. Currently only needed for 2-byte floats:

```briev
type Bfloat16 : Bits {
    bytes <~ 2;
    alignment <~ 2;
    tbaa <~ "Float";
    disamb <~ "bfloat";
    op Add(#Float, #Float);
};
```

The normalizer's `llvm_type` derivation for the Float category at `bytes == 2`:

| `disamb` value | Resolved `llvm_type` |
|---|---|
| `"bfloat"` | `"bfloat"` |
| *(absent)* | `"half"` (IEEE 754 default) |

No parser changes needed — `disamb <~ "value"` uses the existing `<~` metadata
handler (`slot_name == "disamb"` falls through to the general TildeArrow path).

Files changed:
- `src/backend/llvm/normalizer.rs` — add `"disamb"` to keep list + 2-byte float
  handling in the `llvm_type` derivation path for float-category types.
- `docs/architecture/casting-protocol.md` — document `disamb`.

---

## InsertAt / ExtractFrom — Metadata Property Migration

The `<-` (push/pop) operator dispatch currently reads operator bindings from
`ResolvedType.properties["op.InsertAt"]` — metadata stored by the parser's
`<~` handler. This is the last remaining metadata-based dispatch path.

### Target

Full refactor: backend reads `OperatorDef` from `CompilerContext.operator_defs`
instead of properties. No metadata properties, no hardcoded string matches.

### Steps

| # | File | Change |
|---|------|--------|
| 1 | `ast/top.rs` | `OperatorDef.impl_fn` → `impl_args: Option<PropertyValue>` (PropertyValue is consumed directly by `emit_strategy_fn_call`) |
| 2 | `parser/definitions.rs` | `parse_op_with_params`: use `parse_metadata_value_standalone()` for impl args instead of `parse_expression()` (handles `#L`/`#R`/`#T` correctly) |
| 3 | `parser/definitions.rs` | Remove `is_op` check from `<~` handler — `InsertAt`/`ExtractFrom` no longer get `"op."` prefix |
| 4 | `normalizer.rs` | Remove `"op.InsertAt"`/`"op.ExtractFrom"` from keep list |
| 5 | `ring_buffer.bv` | `InsertAt <~ ring_push` → `op InsertAt(T) = ring_push(#L, #R)` (T refers to the enclosing type's param, no `#` needed — resolved via TypeVar at instantiation) |
| 6 | `context.rs` | Add `operator_defs: HashMap<String, Vec<OperatorDef>>` + builder setter |
| 7 | `compile.rs` | Extract `OperatorDef`s from `TopLevel::TypeDef` items → pass to backend |
| 8 | `emit_toplevel.rs` | `check_insert_strategy` → `find_insert_strategy` reading from `operator_defs` |
| 9 | `emit_stmt.rs` | `emit_strategy_fn_call` takes `&OperatorDef`, callers updated |
| 10 | `mod.rs` | Remove hardcoded `"ring_push"` string match in `build_field_index` |

### Flat Control Flow / DRY

Every changed function uses guard clauses and early returns. Strategy lookup
is a single chain: `lookup_strategy_type_name → operator_defs.get → find_by_op`.
No nesting > 2 levels. The `emit_strategy_fn_call` extracts `impl_args: PropertyValue`
from the OperatorDef and delegates to the same marker-resolution logic as before.

---

## Open Questions (for the Architecture Doc)

1. **Field syntax for flat types**: `type Bfloat16 { data: Bits<16>; }` vs. a layout
   attribute. The field syntax is consistent but may feel verbose for single-field types.

2. **Bits parameter vs type parameter**: `Bits<16>` could be `Bits<:16>` or just
   `#Int` category with specific bytes. Needs syntax resolution.
