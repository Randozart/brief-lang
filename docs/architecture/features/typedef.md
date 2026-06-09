# TypeDef — Type Derivation System

**Date:** 2026-06-09  
**Phase:** 1.5  
**Status:** Implemented (primitive kernel + Pass 1 resolver, backend synthesis deferred)

## Design

`Type Name <: Base { ... }` — a top-level declaration that defines a new type by deriving from an existing one. The compiler natively understands a ~13-property primitive kernel; everything else (`String`, `Stack`, `Queue`, `HashMap`) is user-space Brief in `lib/std/`.

## Files

| File | Responsibility |
|------|---------------|
| `src/ast.rs` | `TypeProperty` (13 variants), `TypeDefBody`, `TypeDef`, `Expr::TypeRef` |
| `src/type_universe.rs` | Pass 1 resolver — collect, chain-derive, inherit/override, freeze |
| `src/features/toplevel/typedef.rs` | 5 stub trait impls (ExprTypecheck, ExprEval, ExprCodegenLLVM/VHDL/Webstack) |
| `src/parser.rs` | `parse_type_def()`, `parse_type_expr_for_typedef()` |
| `src/lexer.rs` | `Type` keyword token, `LtColon` token |

## Primitive Kernel

See `BRIEF_3.0_SPEC.md §10.1` for full table. The compiler hardcodes ~13 properties:

- **Layout**: `Bytes`, `Alignment`, `Endian`, `Volatile`, `Atomic`
- **Collection**: `ElementType`, `FixedSize`, `InsertAt`, `ExtractFrom`, `AllowIndex`, `AllowSlice`, `AllowArrow`
- **Encoding**: `Codec`

## Pass 1: Type-Universe

The `TypeUniverse::build()` function collects all `TopLevel::TypeDef` items from `Program`, resolves each against its base type (inheriting properties, applying overrides), and freezes the result. Subsequent passes access it read-only via `TypeUniverse::get()`.

## Deferred Items

Marked `DEFERRED` in code. Documented in `REFACTOR_PLAN.md §Phase 1.5+`:

| ID | Description | Why deferred |
|----|-------------|-------------|
| D-1 | Expression type parameters for generic ordering | Unresolved binding mechanism |
| D-2 | Full codec signature validation | Minimal check sufficient for now |
| D-3 | InsertAt/ExtractFrom strategy synthesis | Stub — actual LLVM codegen deferred |
| D-4 | Deprecation of AsStack/AsQueue | Requires full collection migration |
| D-5 | :> Size uniformity across scalars | Not blocking current use |
| D-6 | Volatile/Atomic as pragmas | Ergonomic follow-up |
| D-7 | Runtime guard synthesis for constraints | Backend work deferred |
| D-8 | CFG field-level metadata matching | Already works via existing `_` binding |
