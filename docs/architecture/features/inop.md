# `inop` / `inop!` — User-Defined Intrinsic Operations

**Date added:** 2026-06-22
**Status:** Implemented (parser, LLVM codegen, interpreter fallback, typechecker, transition graph integration)

## Purpose

`inop`/`inop!` allows the standard library and systems programmers to implement
high-performance low-level primitives in LLVM IR without modifying compiler
source code. This is the user-facing counterpart of the builtin `#`-intrinsic
system.

## Syntax

### Declaration

```brief
// Pure (eligible for CSE, DCE, precomputation):
inop sadd(a: Int, b: Int) -> Int { %res = add i64 %a, %b; term %res; } fallback a + b;

// Side-effecting (not reorderable/eliminable):
inop! write_buf(ptr: Ptr<Byte>, len: Int) -> Int { %res = call i32 @write(i32 1, i8* %ptr, i64 %len); term %res; } fallback 0;
```

| Keyword | `has_side_effects` | Semantics |
|---------|-------------------|-----------|
| `inop` / `inop#` | `false` | Pure — foldable, reorderable |
| `inop!` / `inop#!` | `true` | Side-effecting — preserved |

The `#` on the keyword is cosmetic (`inop` ≡ `inop#`, `inop!` ≡ `inop#!`).
Contract brackets `[pre][post]` go before `-> Type` to avoid parse_type
greedily consuming `[` as a generic type parameter.

### Call site

All inops are called via the existing `name#(args)` syntax:

```brief
let r = sadd#(x, y);
let n = write_buf#(ptr, len);
```

The parser checks `Intrinsic::from_name(name)` first (built-in intrinsics),
then falls back to the program's `inop_decls` map via `Intrinsic::UserDefined(name)`.

### Parameter type mapping

| Brief type | LLVM type | Example |
|------------|-----------|---------|
| `Int` | `i64` | `%x` |
| `Float` | `float` | `%f` |
| `Bool` | `i8` | `%b` |
| `Char` | `i32` | `%c` |
| `String` / `Data` | `i8*` | `%s` |

### LLVM body rules

- Statements are separated by `;`
- Newlines are lexer whitespace (discarded)
- `term` / `term!` is the unified terminator — `term %val` lowers to `ret i64 %val`
- The `%state` pointer is unavailable (explicit params only)
- Single output type (Phase 1)

### Fallback block

```brief
} fallback expr;
```

The `fallback { ... }` was initially designed with braces but `{ expr }` is
parsed as an ObjectLiteral (field:value pairs), not a block expression.
Use bare `fallback expr` — no braces.

The fallback is optional but strongly recommended. Without it:
- Interpreter cannot evaluate the intrinsic
- Non-LLVM backends (Webstack, CIRCT) cannot compile
- Contract verification is blocked

## Typechecking

- `InopDeclaration` params are validated against call-site argument types
- The return type is inferred from the declaration's `outputs` field
- Unknown `UserDefined` names emit diagnostic U001
- `inop_decls: HashMap<String, InopDeclaration>` is collected during Pass 1

## Evaluation (interpreter)

`Intrinsic::UserDefined(name)` dispatches to the fallback expression.
If no fallback exists, a `RuntimeError::MissingInopFallback(name)` is raised.

## Codegen

### LLVM backend

- Declaration: `emit_inop()` emits `define i64 @name(%State* %state, <native ty> %param1, ...)`
- Call site: pre-evaluates args, emits `call <native_ty> @name(%State* %state, <native_ty> arg1, ...)`
- `term %res` → `ret i64 %res`
- `term!` → `ret i64` (with swan song if present)
- User `defn main` is renamed to `brief_main` to avoid collision with `define i32 @main()`

### Webstack / CIRCT

Both fall through to the fallback expression on `Intrinsic::UserDefined`.
No LLVM IR codegen is attempted.

## Transition graph integration

`inop!` (side-effecting) `has_side_effects = true` prevents pure-body
optimization (the transaction body is treated as impure). `inop` (pure)
allows the existing fold/precomputation pipeline to proceed.

The `inop_decls: HashMap<String, bool>` (name → has_side_effects) is
collected in `ReactorTransitionGraph::build()` and threaded through
`is_pure_body`, `references_triggers_or_ffi`, `statement_contains_ffi`,
and `compute_effectively_pure`.

## Files

| File | Role |
|------|------|
| `src/ast.rs` | `InopDeclaration` struct, `TopLevel::Inop`, `Intrinsic::UserDefined(String)` |
| `src/lexer.rs` | `Inop` / `InopBang` tokens |
| `src/parser.rs` | `parse_inop_decl()`, `#(args)` fallback |
| `src/typechecker.rs` | `inop_decls`, return type inference, U001 validation |
| `src/interpreter.rs` | `inop_decls`, fallback evaluation |
| `src/analysis/transition_graph.rs` | Side-effect flag via `intrinsic_has_side_effects` |
| `src/backend/llvm/mod.rs` | `inop_decls` collection, emission loop |
| `src/backend/llvm/emit_toplevel.rs` | `emit_inop()`, `term`→`ret` lowering, `brief_main` rename |
| `src/backend/llvm/emit_expr.rs` | `UserDefined` call codegen |
| `src/backend/webstack.rs` | Fallback-only (no LLVM IR) |
| `src/backend/circt.rs` | Fallback-only (no LLVM IR) |
| `src/import_resolver.rs` | Track `Inop` in visibility maps |
| `src/lsp.rs` | Symbol completion for inop declarations |
| `tests/fixtures/inop_sadd.bv` | E2E test fixture |
| `tests/llvm_backend_test.rs` | IR verification + binary execution test |

## Examples

See `examples/inop-sadd.bv` for a complete working example.
