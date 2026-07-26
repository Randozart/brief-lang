# `inop` / `inop!` — User-Defined Intrinsic Operations

**Date added:** 2026-06-22  
**Updated:** 2026-06-26 (generic <T> params, Custom strategy dispatch)  
**Status:** Implemented (parser, LLVM codegen, interpreter fallback, typechecker, Custom strategy)

## Purpose

`inop`/`inop!` allows the standard library and systems programmers to implement
high-performance low-level primitives in LLVM IR directly,
without modifying compiler source code. The body uses raw LLVM IR with
Brief-flavored syntax.

This is the user-facing counterpart of the builtin `#`-intrinsic system.

Inops can also be bound to the `<-` arrow operator via TypeDef `InsertAt`/`ExtractFrom`
with `Custom` strategy names — see `docs/architecture/features/typedef.md`.

## Syntax

### Declaration

```brief
// Pure (eligible for CSE, DCE, precomputation):
inop sadd(a: Int, b: Int) -> Int { %res = add i64 %a, %b; term %res; } fallback a + b;

// Generic type parameter:
inop sl_insert<T>(list: SkipList<T>, val: T) -> SkipList<T>
    [[term .#Size == list .#Size + 1]
{ ... body ... } fallback sl_append(list, val);

// Side-effecting (not reorderable/eliminable):
inop! write_buf(ptr: Ptr<Int>, len: Int) -> Int { %res = call i32 @write(i32 1, i8* %ptr, i64 %len); term %res; } fallback 0;
```

| Keyword | `has_side_effects` | Semantics |
|---------|-------------------|-----------|
| `inop` / `inop#` | `false` | Pure — foldable, reorderable |
| `inop!` / `inop#!` | `true` | Side-effecting — preserved |

The `#` on the keyword is cosmetic (`inop` ≡ `inop#`, `inop!` ≡ `inop#!`).
Contract brackets `[pre][post]` go before `-> Type` to avoid parse_type
greedily consuming `[` as a generic type parameter. The `(%state)` marker
must come AFTER the contract: `inop! foo() -> Int [pre][post] (%state) { ... }`.

### Generic type parameters

Inops support type parameters `<T, U, ...>` for type-level polymorphism.
The type variables are resolved at compile time from the call-site argument
types. The body uses concrete LLVM types (all Brief values are `i64` at
the LLVM level), so generics are a type-checker-only abstraction:

```brief
inop atomic_load<T>(ptr: Ptr<T>) -> T {
    %p = inttoptr i64 %ptr to ptr
    %v = load atomic i64, ptr %p acquire, align 8
    term %v;
} fallback 0;
```

### Call site

All inops are called via the existing `name#(args)` syntax, or via direct
function call when imported by name:

```brief
let r = sadd#(x, y);
let n = write_buf#(ptr, len);
let sl: SkipList<Int> = [];
&sl <- 42;        // dispatches via Custom strategy to sl_insert#
```

Direct calls (`sl_insert(list, val)`) work when the inop is imported by name.
The typechecker resolves `Expr::Call` against inop_decls before falling through
to `Type::Custom(name)`.

### Body

The body uses raw LLVM IR with Brief-flavored syntax.

Key rules:
- Statements are separated by `;`
- Newlines are lexer whitespace (discarded)
- `term` / `term!` is the unified terminator — `term %val` lowers to `ret i64 %val`
- Multi-output: `term %v, %list;` lowers to `insertvalue { i64, i64 } undef, i64 %v, 0` / `insertvalue ... , i64 %list, 1` / `ret { i64, i64 } ...`
- The `%state` pointer is available via `(%state)` marker on the declaration
- `#section(".init_array")` attribute before `inop!` emits the function in a specific ELF section

### Fallback block

```brief
} fallback expr;
} fallback { block_expr };
```

The fallback is a single Brief expression or block. It provides the reference
implementation for the interpreter and non-LLVM backends (Webstack, CIRCT).
Without a fallback, the interpreter raises `MissingInopFallback` and the
Webstack/CIRCT backends cannot compile.

## Typechecking

- `InopDeclaration` params are validated against call-site argument types
- Generic type parameters are resolved from call-site argument types and substituted into the return type
- The return type is inferred from the declaration's `outputs` field
- Unknown `UserDefined` names emit diagnostic U001
- `inop_decls: HashMap<String, InopDeclaration>` is collected during Pass 1

## Evaluation (interpreter)

`Intrinsic::UserDefined(name)` dispatches to the fallback expression.
If no fallback exists, a `RuntimeError::MissingInopFallback(name)` is raised.

## Codegen

### LLVM backend

- Declaration: `emit_inop()` emits the body as `define i64 @name(%State* %state, <native ty> %param1, ...)`
- Call site: pre-evaluates args, emits `call <native_ty> @name(%State* %state, <native_ty> arg1, ...)`
- `term %res` → `ret i64 %res`
- `term %v, %lh;` (multi-output) → `insertvalue { i64, i64 } ...` / `ret { i64, i64 } ...`
- `term!` → `ret i64` (with swan song if present)
- User `defn main` is renamed to `brief_main` to avoid collision with `define i32 @main()`

### Webstack / CIRCT

Both fall through to the fallback expression on `Intrinsic::UserDefined`.

### Custom strategy binding

When a TypeDef declares `InsertAt = fn_name` or `ExtractFrom = fn_name`,
the `<-` operator dispatches to the named inop via the `Custom(String)`
strategy variant. The LLVM backend emits:
- Push: `call i64 @fn_name(i64 %collection, i64 %value)`
- Pop: `call { i64, i64 } @fn_name(i64 %collection)` (returns pair)

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
| `src/ast.rs` | `InopDeclaration` struct, `type_params` field, `TopLevel::Inop`, `Intrinsic::UserDefined(String)` |
| `src/lexer.rs` | `Inop` / `InopBang` tokens |
| `src/parser.rs` | `parse_inop_decl()`, `#(args)` fallback, `<T>` type param parsing |
| `src/typechecker.rs` | `inop_decls`, return type inference, generic substitution, U001 validation |
| `src/interpreter.rs` | `inop_decls`, fallback evaluation |
| `src/analysis/transition_graph.rs` | Side-effect flag via `intrinsic_has_side_effects` |
| `src/backend/llvm/mod.rs` | `inop_decls` collection, emission loop |
| `src/backend/llvm/emit_toplevel.rs` | `emit_inop()`, `term`→`ret` lowering, `brief_main` rename |
| `src/backend/llvm/emit_expr.rs` | `UserDefined` call codegen, `check_extract_strategy` |
| `src/backend/webstack.rs` | Fallback-only (no LLVM IR) |
| `src/backend/circt.rs` | Fallback-only (no LLVM IR) |
| `src/import_resolver.rs` | Track `Inop` in visibility maps |
| `src/lsp.rs` | Symbol completion for inop declarations |
| `lib/std/skiplist.bv` | SkipList stdlib with Custom strategy bindings |
| `lib/std/atomic.bv` | Atomic operations via inop |
| `lib/std/state.bv` | Stateful `(%state)` inop pattern |

## See also

- `docs/architecture/features/typedef.md` — `InsertAt`/`ExtractFrom` Custom strategy for `<-` dispatch
- `examples/inop-sadd.bv` — basic inop example
- `examples/inop-skiplist-dispatch.bv` — SkipList Custom strategy dispatch
- `examples/inop-ring-buffer.bv` — ring buffer demo
- `examples/inop-syscall-io.bv` — syscall wrapper inops
- `examples/inop-isr-table.bv` — ISR vector table with `#section` + `(%state)`
- `examples/inop-uart-mmap.bv` — MMIO registers with flattened board constants
