# ReturnKind Enum + SysCall#/SysConf# Rename

**Date:** 2026-07-15
**Status:** Active

## Summary

Replace `return_type: Option<Type>` with a `ReturnKind` enum that properly
distinguishes backend-native types (`#Int`, `#Float`, `#Bool`), inferred
types (`Inferred`), and concrete types (`Exact(Type)`). Also rename
`Syscall#` → `SysCall#` and `Sysconf#` → `SysConf#` for proper PascalCase.

## Scope

**`src/intrinsic_signatures.rs`** — Add `ReturnKind` enum, update `Signature`,
update all ~40 match arms, update tests.

**`src/typechecker/mod.rs`** — Update return-type inference to handle
`Native`, `Inferred`, `Exact` instead of `Option<Type>` → void fallback.

**`src/backend/llvm/intrinsics.rs`** — Rename dispatch arms `Syscall#` → `SysCall#`,
`Sysconf#` → `SysConf#`.

**`src/interpreter/intrinsics.rs`** — Same rename.

**`lib/std/os/*.bv` and `lib/std/string_c.bv`** — Replace all `Syscall#(` →
`SysCall#(` and `Sysconf#(` → `SysConf#(` in Briev source files.

## Implementation Steps

### Step 1: ReturnKind enum + Signature update

```rust
pub enum ReturnKind {
    Native(&'static str),  // #Int, #Float, #Bool — backend decides exact repr
    Inferred,              // from argument types (Add# returns same as input)
    Exact(Type),           // fixed concrete type (Ptr<Bits(8)>, Void)
}

pub struct Signature {
    pub name: &'static str,
    pub parameters: Vec<(&'static str, Type)>,
    pub return_kind: ReturnKind,
    pub observable: bool,
}
```

### Step 2: Update match arms (~40)

All currently `return_type: None` → `ReturnKind::Inferred`
All currently `return_type: Some(Type::int())` → `ReturnKind::Native("Int")`
All currently `return_type: Some(Type::void())` → `ReturnKind::Exact(Type::void())`
All currently `return_type: Some(Type::ptr(...))` → `ReturnKind::Exact(Type::ptr(...))`
All currently `return_type: Some(Type::float())` → `ReturnKind::Native("Float")`

### Step 3: Rename `Syscall#` → `SysCall#` and `Sysconf#` → `SysConf#`

### Step 4: Update type checker inference

Replace:
```rust
sig.return_type.clone().unwrap_or(Type::void())
```
With:
```rust
match &sig.return_kind {
    ReturnKind::Native("Int") => Type::int(),
    ReturnKind::Native("Float") => Type::float(),
    ReturnKind::Native("Bool") => Type::bool(),
    ReturnKind::Inferred => infer_from_args(...),
    ReturnKind::Exact(t) => t.clone(),
    _ => Type::int(), // fallback
}
```

### Step 5: Update `.bv` files

Find all `Syscall#(` and `Sysconf#(` in `lib/std/os/` and `lib/std/string_c.bv`,
replace with `SysCall#(` and `SysConf#(`.

### Step 6: Build + test

`cargo test --lib && cargo build --release`

## Rationale comments

Every changed module header gets: `// 2026-07-15: ReturnKind replaces
return_type: Option<Type> for backend-agnostic type dispatch.`

Every renamed arm gets: `// 2026-07-15: Renamed for proper PascalCase`.
