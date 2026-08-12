# Projection — The Projection Lens Operator

**Date:** 2026-06-20  
**Phase:** 1 (mechanical), extended by Phases 2-3  
**Status:** Implemented (18 built-in targets + UserDefined/UserDefinedWithArg infrastructure)

## Design

The `:>` operator (`expr :> Target(args)`) extracts compile-time metadata or applies a type-defined projection. It is the primary mechanism for type-directed operations in Briev.

Syntax: `source :> ProjectionName(arg1, arg2, ...)`

## Built-in Projection Targets

18 targets are hardcoded in `ProjectionTarget`:

| Target | Input | Output | Description |
|--------|-------|--------|-------------|
| `Size` | List/Tuple/String | Int | Element count or string length |
| `Bytes` | Any | Int | Storage size in bytes |
| `Ptr` | Any | Ptr<T> | Memory pointer |
| `Alignment` | Any | Int | Required alignment |
| `Range` | Int/Float/Bool/Char | Int (pair) | Valid value range |
| `Popcount` | Int | Int | Set bit count |
| `LeadingZeros` | Int | Int | Leading zero bit count |
| `TrailingZeros` | Int | Int | Trailing zero bit count |
| `Absolute` | Int/Float | same | Absolute value |
| `BitReverse` | Int | Int | Reversed bits |
| `Type` | Any | String | Type name as string |
| `Ptr!` | Pointer | Int | Raw address |
| `Match` | Enum | String | Discriminant name |
| `Keys` | HashMap | List | Key list |
| `Values` | HashMap | List | Value list |
| `Contains(key)` | HashMap/HashSet | Bool | Membership check |
| `Pop(key)` | HashMap | Value | Remove and return |
| `Index(key)` | HashMap | Value | Index access |
| `IsEmpty` | List/Tuple/HashMap/HashSet/String | Bool | True if empty |
| `Front` | Queue | Value | Peek front |
| `Top` | Stack | Value | Peek top |
| `Elements` | HashSet | List | Element list |
| `AsStack` | List | Stack | Convert |
| `AsQueue` | List | Queue | Convert |
| `UserDefined(name)` | Any | varies | TypeDef projection |
| `UserDefinedWithArg(name, expr)` | Any | varies | Parameterized projection |

## Evaluation

The interpreter matches on `ProjectionTarget` and the source `Value` variant:
- Collection targets (Size, Keys, Values, Contains, Pop, Index, IsEmpty) dispatch on Value::List/HashMap/HashSet/Stack/Queue
- Numeric targets (Popcount, LeadingZeros, etc.) dispatch on Value::Int/Float
- Ptr/Ptr!/Alignment/Bytes dispatch on Value::Ptr or compute from storage
- **UserDefined** (Phase 3.5): Well-known unary operator names (Neg, Not, BitNot) evaluated directly on Value::Int/Float/Bool. Unknown names return error.
- **UserDefinedWithArg** (Phase 3.5): Fast-path for 40+ (source_type, name) pairs matching Value::Int/Float/Bool with operator names (Add, Sub, Eq, etc.). Unknown combinations return error.

## Typechecker

Each target returns the appropriate `Type`:
- Metadata targets (Size, Bytes, Alignment, Range) → Type::Int
- Collection targets → Type::Int, Type::Applied("List", ...), etc.
- **UserDefined / UserDefinedWithArg** (Phase 3.5): `resolve_user_projection_type()` checks well-known operator names (~25 patterns) first, then falls back to TypeUniverse projection binding lookup for user-defined types. Returns Type::Int default if unresolved.
- UserDefined → Type::Void (DEFERRED — needs TypeUniverse wiring)

## LLVM Backend

Known targets emit real LLVM IR:
- Size → gep for struct length field, load
- Bytes → load from header
- Popcount → `call @llvm.ctpop.i64`
- **UserDefinedWithArg** (Phase 3.5): Fast-path for 40+ (LLVM type, operator) pairs. `try_projection_fast_path()` emits native `add/sub/mul/icmp/fadd/fcmp` instructions for known (Int, Float, Bool) × 18 operator combinations.
- UserDefined → `add i64 0, 0` (stub for unknown operator names)

## Webstack Backend

Known targets emit JS:
- Size → `.length`
- Popcount → `toString(2).split('1').length - 1`
- Unknown targets → return source unchanged

UserDefined projections fall through to the source-value catch-all.

## Files

| File | Responsibility |
|------|---------------|
| `src/ast.rs` | `ProjectionTarget` enum, `Expr::Projection` variant |
| `src/features/projection.rs` | `ProjectionExpr` struct, evaluate/typecheck/emit_llvm/emit_js; `eval_user_projection_fast_path()` for interpreter fast-path |
| `src/interpreter.rs` | `Expr::Projection` routing to `ProjectionExpr::evaluate` |
| `src/typechecker.rs` | Target → Type mapping in `infer_expression`; `resolve_user_projection_type()` for UserDefined/UserDefinedWithArg |
| `src/backend/llvm/emit_expr.rs` | Target → LLVM IR in projection match arms; `try_projection_fast_path()` for LLVM fast-path |
| `src/backend/llvm/mod.rs` | `type_universe: Option<TypeUniverse>` field, `with_type_universe()` builder |
| `src/type_universe.rs` | `TypeUniverse::build()` — constructed in main.rs pipeline for Phase 3.5 |
| `src/main.rs` | `TypeUniverse` built and passed to typechecker + LLVM backend |
| `src/backend/webstack.rs` | Target → JS in `expr_to_ts` match arms |
| `src/parser.rs` | `parse_projection_target()` dispatch to target variants |
