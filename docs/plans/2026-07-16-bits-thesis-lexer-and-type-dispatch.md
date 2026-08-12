# Bits-Thesis: Lexer Type Tokens → Identifier + Config-Driven Backend Dispatch

**Date:** 2026-07-16
**Status:** Implementation
**Applies to:** Lexer, parser, LLVM backend normalizer, backend emission

---

## Problem

The compiler violates the Bits thesis in three ways:

1. **Lexer** has 33 dedicated `Token::Type*` variants (`TypeInt`, `TypeFloat`, `TypeBool`, `TypeI8`, `TypeU32`, etc.) that privilege specific type names. Under the Bits thesis, `Int`, `Float`, `Bool`, `i32`, `u64` etc. are stdlib-defined types, not compiler primitives — they should all be `Token::Identifier(name)`.

2. **Parser `parse_type()`** matches on these token variants rather than reading a string from `Token::Identifier(name)` and dispatching. `parse_type_or_group()` has a duplicate 6-case peek-match that became redundant after P2.

3. **LLVM backend** has ~200+ sites that match on type name strings (`"Int"`, `"Float"`, `"Bool"`, `"String"`, `"Char"`) instead of reading pre-annotated metadata (`primitive`, `alu`, `llvm_type`) from the type's properties. The normalizer already derives and attaches these annotations; the backend should use them.

---

## Layer 1: Remove 33 `Token::Type*` Variants from the Lexer

### `src/lexer.rs`

Delete three blocks:

**Block A — Enum variants** (lines 503–603):
```
// ── Type keywords ─────────────────────────────────────────
#[token("Int")]     TypeInt,
#[token("UInt")]    TypeUInt,
...
#[token("Double")]  TypeDouble,
```
Remove all 33 entries. After this, `"Int"` lexes as `Token::Identifier("Int")`, `"Float"` as `Token::Identifier("Float")`, `"i32"` as `Token::Identifier("i32")`, etc.

**Block B — Display impl arms** (lines 749–781):
```
Token::TypeInt => write!(f, "Int"),
...
Token::TypeDouble => write!(f, "Double"),
```
Remove all 33 arms.

**Block C — Test assertion** (line 800, 802):
```rust
// Before:
assert_eq!(lexer.next(), Some(Ok(Token::TypeInt)));
// After:
assert_eq!(lexer.next(), Some(Ok(Token::Identifier("Int".to_string()))));
```

**Rationale:** All type names become identifiers. The Bits thesis axiom 1 says `Bits` is the sole primitive — everything else is a user-defined type with metadata. No type name deserves a dedicated token.

---

## Layer 2: Update Parser to Dispatch on Identifier Strings

### `src/parser/types.rs` — `parse_type()` (lines 12–68)

Replace the explicit `match self.peek()` on `Token::TypeInt` etc. with a `match self.peek()` on `Token::Identifier(name)` + string dispatch:

```rust
pub fn parse_type(&mut self) -> Result<Type, SyntaxError> {
    let base = match self.peek() {
        Some(Token::Identifier(name)) => {
            let name = name.clone();
            self.pos += 1;
            match name.as_str() {
                "Int" => return Ok(Type::int()),
                "UInt" => ("UInt", Type::Custom("UInt".into())),
                "Float" | "Float32" | "F32" => return Ok(Type::float()),
                "Float64" | "F64" | "Double" => return Ok(Type::float64()),
                "String" => return Ok(Type::string()),
                "Bool" => return Ok(Type::bool_()),
                "Void" => return Ok(Type::void()),
                "Char" => return Ok(Type::char_()),
                "Data" => return Ok(Type::data()),
                _ => {
                    let ty = self.parse_named_type_body(&name)?;
                    return Ok(ty);
                }
            }
        }
        Some(Token::LParen) => return self.parse_tuple_type(),
        _ => return self.error_at_current("expected type"),
    };
    // 2026-07-16: P2 — .ext suffix check unchanged
    if let Some(ext) = self.try_parse_dot_extension() {
        let mut full = format!("{}.{}", base.0, ext);
        while let Some(next) = self.try_parse_dot_extension() {
            full = format!("{}.{}", full, next);
        }
        return Ok(Type::Custom(full));
    }
    Ok(base.1)
}
```

Key changes:
- `Token::TypeInt` becomes `"Int"` in a string match on `Token::Identifier(name)`
- `Token::TypeFloat | Token::TypeFloat32` becomes `"Float" | "Float32" | "F32"`
- All type name strings are explicit; any unrecognized name delegates to `parse_named_type_body()`
- `Type::float64()` is only produced by `"Float64"`, `"F64"`, `"Double"` — no longer by `Token::TypeFloat64`
- `.ext` suffix handling (P2) is unchanged

### `src/parser/definitions.rs` — `parse_type_or_group()` (lines 731–743)

Replace the 6-case peek-match block with a single `expect_identifier()` call:

```rust
// Before:
let name = match self.peek() {
    Some(Token::TypeInt) => { self.advance(); "Int".to_string() }
    Some(Token::TypeFloat) => { self.advance(); "Float".to_string() }
    Some(Token::TypeUInt) => { self.advance(); "UInt".to_string() }
    Some(Token::TypeString) => { self.advance(); "String".to_string() }
    Some(Token::TypeBool) => { self.advance(); "Bool".to_string() }
    Some(Token::TypeChar) => { self.advance(); "Char".to_string() }
    _ => self.expect_identifier()?,
};

// After:
let name = self.expect_identifier()?;
```

Because all type names are now `Token::Identifier`, `expect_identifier()` handles them naturally. The explicit match was a P2 workaround that's no longer needed.

### `src/parser/helpers.rs` — `keyword_as_identifier()` (lines 260–269)

Remove the 10 type-token entries added in the earlier P6 implementation:

```rust
// Remove these 10 lines:
Token::TypeInt => "Int".into(),
Token::TypeFloat => "Float".into(),
Token::TypeBool => "Bool".into(),
Token::TypeString => "String".into(),
Token::TypeChar => "Char".into(),
Token::TypeInt8 => "Int8".into(),
Token::TypeInt16 => "Int16".into(),
Token::TypeInt32 => "Int32".into(),
Token::TypeInt64 => "Int64".into(),
Token::TypeFloat32 => "Float32".into(),
```

These variants no longer exist in the `Token` enum, so the code won't compile otherwise.

**No other parser changes needed.** `parse_meld()` line 537, `parse_struct_like()` line 937, `parse_enum_like()` line 965 — all already use `expect_identifier()` and work correctly once `Int` is `Token::Identifier("Int")`.

---

## Layer 3: Eliminate Backend Type-Name String Matching via Normalizer

### Problem

The LLVM backend emits instructions by matching on type name strings:

```rust
// src/backend/llvm/emit_toplevel.rs:1081
if matches!(t, Type::Custom(__t) if __t == "Bool" || __t == "Char" || ...)

// src/backend/llvm/helpers.rs:440
("Int" | "UInt", "Float") => { ... }

// src/backend/llvm/builder.rs:546
BrievType::Custom(__t) if __t == "Bool" => { ... }
```

This is ~200+ sites. Each one reads a type name and decides behavior — a violation of the Bits thesis which says all semantics are in metadata, not names.

### Solution: Normalizer-Annotated Dispatch

The normalizer (`src/backend/llvm/normalizer.rs`) already:

1. Reads `primitive` metadata from each type (`rt.primitive()`)
2. Derives `llvm_type` via `derive_llvm_type(prim, rt.bytes, &prim_config)`
3. Stores it as a property: `rt.properties.insert("llvm_type", PropertyValue::String(...))`

The backend should read `llvm_type` and `primitive` properties instead of matching on type name strings. The normalizer is the single place where metadata → backend-representation translation happens.

#### Step 3a: Add metadata key to keep-list (normalizer.rs line 70)

The normalizer strips non-LLVM metadata at line 70:
```rust
let keep: HashSet<String> = ["primitive", "llvm_type", "encoding", "layout"]
```
This already includes `"primitive"` and `"llvm_type"`. No change needed here — they survive.

#### Step 3b: Audit and replace all type-name string matches

Every backend site that currently matches on `__t == "Int"`, `__t == "Float"`, `__t == "Bool"` etc. should instead:

- **Read `llvm_type`** from the type's properties for LLVM IR representation questions
- **Read `primitive`** from the type's properties for ALU/cast dispatch
- **Read `bytes`** from `ResolvedType.bytes` for width

The replacement helpers:

```rust
/// Read the primitive metadata from a type (safe fallback).
pub fn type_primitive<'a>(universe: &'a TypeUniverse, ty: &Type) -> Option<&'a str> {
    match ty {
        Type::Custom(name) => universe.get(name)
            .and_then(|rt| rt.properties.get("primitive"))
            .and_then(|pv| if let PropertyValue::Identifier(p) = pv { Some(p.as_str()) } else { None }),
        _ => None,
    }
}
```

Each replacement is mechanical: replace `if __t == "Bool"` with `type_primitive(ctx.universe, ty) == Some("Bool")`. The string "Bool" still appears, but it's the `primitive` metadata key (a config-driven identifier), not the type name. The config file is the single source of truth.

**Scope:** ~200 sites in:
- `src/backend/llvm/emit_toplevel.rs` (~80 sites)
- `src/backend/llvm/helpers.rs` (~50 sites)
- `src/backend/llvm/builder.rs` (~30 sites)
- `src/backend/llvm/dispatch.rs` (~20 sites)
- `src/backend/llvm/context.rs` (~10 sites)
- Other LLVM files (~10 sites)

### Step 3c: Add `type_primitive()` / `type_llvm_type()` helpers

Add to `src/backend/llvm/helpers.rs` or a new `src/backend/llvm/metadata.rs`:

```rust
/// 2026-07-16: Read the LLVM type string from a type's annotations.
/// Returns the llvm_type property if set, otherwise None.
pub fn type_llvm_type(universe: &TypeUniverse, ty: &Type) -> Option<String> {
    match ty {
        Type::Custom(name) => universe.get(name).and_then(|rt|
            rt.properties.get("llvm_type").and_then(|pv|
                if let PropertyValue::String(s) = pv { Some(s.clone()) } else { None }
            )
        ),
        _ => None,
    }
}
```

### Step 3d: Verify test suite after each batch of replacements

Replacements are done one function at a time, running `cargo test --lib` after each batch.

---

## Implementation Order

| Step | Layer | Files | Est. Changes | Verification |
|------|-------|-------|-------------|-------------|
| 1 | 1 | `src/lexer.rs` | 66 lines removed | `cargo test --lib` |
| 2 | 2 | `src/parser/types.rs` | ~30 lines replaced | `cargo test --lib` |
| 3 | 2 | `src/parser/definitions.rs` | ~8 lines removed | `cargo test --lib` |
| 4 | 2 | `src/parser/helpers.rs` | 10 lines removed | `cargo test --lib` |
| 5 | 3 | `src/backend/llvm/helpers.rs` | Add helpers | `cargo test --lib` |
| 6 | 3 | `src/backend/llvm/*.rs` | ~200 sites replaced | Batch `cargo build && cargo test --lib` |

---

## Rationale Comments to Add

Every replaced backend site gets:

```
// 2026-07-16: Bits-thesis — dispatch on primitive metadata, not type name.
// The normalizer attaches `primitive` from source metadata; the config file
// (llvm-primitives.toml) maps it to backend behavior.
```

---

## Risks

- **Layer 3 is large.** ~200 changes across 5+ files. Each is mechanical but collectively significant. If confidence is low, split into sub-phases by file.
- **`type_primitive()` returns `None`** for types without `primitive` property (e.g., generic `Type::Bits(N)`). The fallback behavior (`None`) must match what the original code did when the type name didn't match any arm.
- **SPIR-V backend** has similar name-matching issues but is out of scope for this plan.
- **`Type::Custom("Int")` vs `Type::int()`** — after Layer 2, `Type::int()` is produced for "Int" identifier. The `Type::Custom("Int")` is now rare (only for types not in the static dispatch table). Backend sites matching on `Type::Custom(__t)` need to check for both `Type::int()` and `Type::Custom("Int")` during migration.
