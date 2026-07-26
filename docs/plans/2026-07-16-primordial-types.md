# Primordial Types — "Untyped" Built-ins

**Date:** 2026-07-16
**Status:** Implementation
**Applies to:** TypeUniverse, normalizer, meld validation

---

## Background

The Bits thesis (docs/architecture/bits-thesis.md) states that `Bits` is the
sole compiler primitive — every other type (`Int`, `Float`, `String`, etc.) is
a stdlib-defined `type Foo : Bits { ... }` with metadata overlays.

In practice, this created a circular dependency: `meld Int -> Float` needs
`Int` and `Float` in the `TypeUniverse`, but those types only arrive via
stdlib import resolution after the prelude plugin fires. If import resolution
fails or the prelude doesn't fire, the program panics at `universe.get("Int")`.

**Primordial types** resolve this by seeding the `TypeUniverse` with a fixed
set of well-known type names on construction. Each carries default metadata
(bytes, alignment, primitive, llvm_type) matching what the stdlib would
provide. A user `type Int : Bits { bytes <~ 4; }` in source code
**replaces** the primordial entry — HashMap insert wins.

The primordial type table is a pragmatic companion to the Bits thesis, not a
contradiction. The compiler does NOT hardcode these types in the lexer (33
`Token::Type*` variants are already removed), in the parser (dispatch on
identifier strings), or in the backend (metadata-driven dispatch). It simply
seeds the universe so that "bare" type references work without stdlib import.

---

## Primordial Type Table

| Name | bytes | alignment | primitive | llvm_type | Field annotations |
|------|-------|-----------|-----------|-----------|-------------------|
| `Int` | 8 | 8 | signed | i64 | |
| `UInt` | 8 | 8 | unsigned | i64 | |
| `Int8` | 1 | 1 | signed | i8 | |
| `UInt8` | 1 | 1 | unsigned | i8 | |
| `Int16` | 2 | 2 | signed | i16 | |
| `UInt16` | 2 | 2 | unsigned | i16 | |
| `Int32` | 4 | 4 | signed | i32 | |
| `UInt32` | 4 | 4 | unsigned | i32 | |
| `Int64` | 8 | 8 | signed | i64 | |
| `UInt64` | 8 | 8 | unsigned | i64 | |
| `Float`/`Float32` | 4 | 4 | float | float | |
| `Float64`/`Double` | 8 | 8 | float | double | |
| `Bool` | 1 | 1 | unsigned | i8 | |
| `Char` | 4 | 4 | unsigned | i32 | |
| `String` | 24 | 8 | struct | %String | ptr(0,64), len(64,64), codec(128,8) |
| `Data` | 8 | 8 | pointer | i8* | |
| `Void` | 0 | 0 | void | void | |

All primordial types have `base = "Bits"`.

---

## Override Rule

A user `type Int : Bits { bytes <~ 4; }` in source produces a
`TopLevel::TypeDef`. `register_typedefs` in the normalizer calls
`universe.register(rt)` which does `HashMap::insert`. The user's
`ResolvedType` **replaces** the primordial entry by name.

The replacement is complete — all metadata from the primordial version is
lost. The user's type is the single source of truth from that point forward.

---

## Changes

### TypeUniverse

`TypeUniverse::new()` calls `seed_primordial_types()` which inserts the table
above as `ResolvedType` entries.

### Normalizer

The main annotation loop (normalizer.rs lines 19-30) currently calls
`derive_llvm_type` unconditionally. Change to skip if `llvm_type` property
is already set — primordial types with explicit `llvm_type` (like
`%String`) keep it.

### Meld Validation

Replace 5 `.unwrap()` calls on `universe.get()` with
`ok_or(MeldValidationError::TypeNotFound(...))`. Add early returns for
empty routes in validation functions. Filter unit-vector test to only check
field mapped in meld routes (fixes padding field failures).

---

## Files Touched

- `src/type_universe/mod.rs` — seed table in `new()`
- `src/backend/llvm/normalizer.rs` — conditional `derive_llvm_type`
- `src/analysis/meld_validation.rs` — safe `universe.get()`, early returns,
  unit vector field filter
