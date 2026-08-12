# TypeDef — The Derivation Lens Operator

**Date:** 2026-07-11  
**Phase:** 1A (Generic Property System)  
**Status:** Refactored — TypeDefBody.split into metadata + projections, slot syntax added.

## Design

`Type Name : Base { ... }` — a top-level declaration that defines a new type by deriving from an existing one using the **Derivation lens** (`<:`). The compiler natively recognizes well-known metadata names (~13); everything else is a user-defined projection stored in `ResolvedType::projections`.

The type body supports four kinds of entries:

| Entry | Syntax | Storage |
|-------|--------|---------|
| **Slot** | `name: Type;` | `TypeDefBody.slots` — structural bit partitions |
| **Metadata** | `name <~ expr;` | `TypeDefBody.metadata` — compile-time constants |
| **Projection** | `name(params) = expr;` | `TypeDefBody.projections` — lazy computed properties |
| **Operator** | `op Rune(Param) -> Ret = impl;` | `TypeDefBody.operators` — intrinsic dispatch |

The old `TypeDefBody.bindings` field is retained as a migration compat field (dual-written).

## Syntax

```briev
type MyInt : Int {
    maxbits <~ 64;                    // known metadata → PropertyValue::Int(64)
    alignment <~ 8;                // known metadata → PropertyValue::Int(8)
    IsPositive(x) = x > 0;         // user-defined projection → projections["IsPositive"]
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
3. Apply bindings from `TypeDefBody.bindings` (legacy) — known names → hardcoded fields, unknown → `projections`
4. Dual-write to `ResolvedType.properties` HashMap (Phase 1B) — every `<~` metadata entry AND every `apply_binding()` result also populates the generic properties map
5. After Phase 2 migration completes, step 3 will read from `metadata` + `projections` separately and the hardcoded fields will be removed

## User-Defined Projections

Any binding name not in the known-metadata table is stored as a `TypeBinding` in `ResolvedType::projections`. Codegen and typecheck are DEFERRED (Phase 3 tasks 5-8 of Bits Thesis plan). Currently:
- **Typechecker**: returns `Type::Void`
- **Interpreter**: returns `UnsupportedProjection` error
- **LLVM**: emits `add i64 0, 0`

## Files

| File | Responsibility |
|------|---------------|
| `src/ast.rs` | `PropertyValue` enum, `TypeBinding` struct, `TypeDefBody.metadata: HashMap<String, PropertyValue>` + `projections: Vec<TypeBinding>`, slot/op/constraint fields |
| `src/type_universe.rs` | Pass 1 resolver — collect, chain-derive, inherit/override, apply_binding, freeze |
| `src/features/toplevel/typedef.rs` | 5 trait impls (evaluate returns error, typecheck returns Void, LLVM returns %void, webstack returns undefined) |
| `src/parser.rs` | `parse_type_def()` — `name <~ expr;` for metadata, `name(params) = expr;` for projections |
| `src/lexer.rs` | `Type` keyword token, `LtColon` token, `TildeArrow` token |

## Deferred Items

Marked `DEFERRED` in code:

| ID | Description | Why deferred |
|----|-------------|-------------|
| D-1 | Topological sort for forward references | Current single-pass requires declaration order |
| D-2 | Full codec signature validation | Minimal check sufficient for now |
| D-3 | ~~InsertAt/ExtractFrom strategy synthesis~~ | **Implemented** — Custom(String) variant dispatches to named inops |
| D-5 | `.#Size` uniformity across scalars | Not blocking current use |
| D-7 | Runtime guard synthesis for constraints | Backend work deferred |
| D-8 | User-defined projection typecheck/codegen | Needs TypeUniverse wired into typechecker and LLVM backend |

### D-3: InsertAt/ExtractFrom Custom strategy (implemented 2026-06-25)

When a TypeDef declares `InsertAt <~ fn_name` or `ExtractFrom <~ fn_name`
and the strategy string doesn't match any built-in name (`append`, `prepend`,
`sorted`, `hash`, `pop`, `shift`), the resolver returns
`InsertStrategy::Custom(fn_name)` instead of `None`.

Both the interpreter and LLVM backend dispatch `<-` to the named function:

- **Interpreter**: `call_custom_fn(fn_name, [collection, value])` for Push;
  `call_custom_fn(fn_name, [collection])` expecting `(popped, new_collection)` for Pop.
- **LLVM backend**: emits `call i64 @fn_name(i64, i64)` for Push;
  `call { i64, i64 } @fn_name(i64)` for Pop.

The function name is looked up first as an `inop` declaration (uses fallback),
then as a `defn` (executes body).

Example:
```briev
type SkipList<T> : List<T> {
    InsertAt <~ sl_insert;
    ExtractFrom <~ sl_remove;
};

inop sl_insert<T>(list: SkipList<T>, val: T) -> SkipList<T>
    [[term .#Size == list .#Size + 1]
{ ... body ... } fallback sl_append(list, val);
```

The strategy system also fixes **interpreter variable-name vs type-name lookup**:
`lookup_insert_strategy` uses `let_types` (declared type annotations) to resolve
the type name, not the variable name. This means `let sl: SkipList<Int> = ...`
correctly maps `sl` to `SkipList` for strategy resolution.

### InsertAt/ExtractFrom binding format

```briev
type MyCollection : List {
    InsertAt <~ sl_insert;       // Custom(strategy) → calls sl_insert#(list, val)
    ExtractFrom <~ sl_remove;    // Custom(strategy) → calls sl_remove#(list)
};
```

Known built-in strategy names:
- `InsertAt`: `append`, `prepend`, `sorted`, `hash`
- `ExtractFrom`: `pop`, `shift`, `head`, `tail`, `hash`

Any other string is treated as a `Custom(fn_name)` function name reference.
