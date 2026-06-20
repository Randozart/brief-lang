# TypeDef — Type Derivation System

**Date:** 2026-06-20  
**Phase:** 2 (Unified TypeDef Bodies)  
**Status:** Implemented (TypeBinding replaces TypeProperty, TypeUniverse Pass 1 resolver works, backend synthesis deferred)

## Design

`Type Name <: Base { ... }` — a top-level declaration that defines a new type by deriving from an existing one. The compiler natively recognizes well-known metadata binding names (~13); everything else is a user-defined projection stored in `ResolvedType::projections`.

Phase 2 replaced the 13-variant `TypeProperty` enum with a unified `TypeBinding` struct. Every TypeDef body entry is parsed as `Name[(params)] = Expr;`. The `TypeUniverse::apply_binding` method dispatches known names to `ResolvedType` fields and stores unknown names as projections.

## Syntax

```brief
type MyInt <: Int {
    Bytes = 8;                    // known metadata → ResolvedType::bytes
    Alignment = 8;                // known → ResolvedType::alignment
    IsPositive(x) = x > 0;        // user-defined projection → projections["IsPositive"]
};
```

## Known Metadata Names

| Name | ResolvedType field | Type |
|------|--------------------|------|
| `Bytes` | `bytes` | i64 |
| `Alignment` | `alignment` | i64 |
| `Endian` | `endian` | i64 (0=little, 1=big) |
| `Volatile` | `volatile` | bool |
| `Atomic` | `atomic` | bool |
| `ElementType` | `element_type` | String |
| `FixedSize` | `fixed_size` | Option<bool> |
| `InsertAt` | `insert_at` | Option<String> |
| `ExtractFrom` | `extract_from` | Option<String> |
| `AllowIndex` | `allow_index` | bool |
| `AllowSlice` | `allow_slice` | bool |
| `AllowArrow` | `allow_arrow` | bool |
| `Codec` | `codec` | Option<String> |

## Pass 1: Type-Universe Resolution

`TypeUniverse::build()` collects all `TopLevel::TypeDef` items, resolves each:
1. Create `ResolvedType` with defaults
2. Inherit from base type (if resolved)
3. Apply bindings — known names → fields, unknown → `projections` HashMap

## User-Defined Projections

Any binding name not in the known-metadata table is stored as a `TypeBinding` in `ResolvedType::projections`. Codegen and typecheck are DEFERRED (Phase 3 tasks 5-8 of Bits Thesis plan). Currently:
- **Typechecker**: returns `Type::Void`
- **Interpreter**: returns `UnsupportedProjection` error
- **LLVM**: emits `add i64 0, 0`

## Files

| File | Responsibility |
|------|---------------|
| `src/ast.rs` | `TypeBinding` struct, `TypeDefBody.bindings: Vec<TypeBinding>`, `ProjectionTarget::UserDefined` / `UserDefinedWithArg` |
| `src/type_universe.rs` | Pass 1 resolver — collect, chain-derive, inherit/override, apply_binding, freeze |
| `src/features/toplevel/typedef.rs` | 5 trait impls (evaluate returns error, typecheck returns Void, LLVM returns %void, webstack returns undefined) |
| `src/parser.rs` | `parse_type_def()` — all entries parsed as `Name[(params)] = Expr;` |
| `src/lexer.rs` | `Type` keyword token, `LtColon` token |

## Deferred Items

Marked `DEFERRED` in code:

| ID | Description | Why deferred |
|----|-------------|-------------|
| D-1 | Topological sort for forward references | Current single-pass requires declaration order |
| D-2 | Full codec signature validation | Minimal check sufficient for now |
| D-3 | InsertAt/ExtractFrom strategy synthesis | Stub — actual LLVM codegen deferred |
| D-5 | `:> Size` uniformity across scalars | Not blocking current use |
| D-7 | Runtime guard synthesis for constraints | Backend work deferred |
| D-8 | User-defined projection typecheck/codegen | Needs TypeUniverse wired into typechecker and LLVM backend |
