# Top-Level `__init` — Scripting with Atomic Boot Safety

**Date added:** 2026-06-11
**Status:** Implementation complete

## Purpose

Allow executable statements directly at global scope, which the compiler
automatically wraps in a synthesized `rct txn __init` transaction at
compile time. Eliminates boilerplate for simple scripts while retaining
Brief's transactional safety guarantees.

## Syntax

```brief
// Conventional: boilerplate wrapper
rct txn main [true][true] {
    println#("hello");
};

// With __init: just write the statement
println#("hello");
```

## How it Works

### Parser

`TopLevel::Statement(Box<Statement>)` variant in the AST.

All let/const/struct/enum/txn/defn declarations must precede executable
statements. A `Statement` after the first `Statement` is sequential
execution. A declaration after a `Statement` is a compile error:

> "Declarations must precede top-level executable statements"

### Synthesis

`Program::synthesize_init_txn()` (in `src/ast.rs`) collects all
`TopLevel::Statement` items and synthesizes:

```brief
// Synthesized (fannkuch_redux_0.bv was the test case):
let __booted_0: Int = 0;

rct txn __init [!__booted_0][__booted_0] {
    // all top-level statements in order
    &__booted_0 = 1;
    term;
};
```

The collision-avoiding booted flag name (`__booted_N`) is found by checking
`__booted_0` through `__booted_63` against existing state declarations.

### Execution

The synthesized `__init` transaction fires once on program start
(precondition `!__booted_0` is true initially). After execution, `__booted_0`
is set to `1`, preventing re-fire. If `escape` is triggered inside a
top-level statement, the transaction atomically rolls back — zero partial
state, no half-configured program.

### Contract

- Precondition: `!__booted_N` (fires exactly once)
- Postcondition: `__booted_N` (guarantees one-shot completion)

The `__booted_N` flag uses `Type::Int` (not `Bool`) to match LLVM
backend prior-state expectations (all state fields are `i64`).

## Typechecking

The typechecker processes `TopLevel::Statement(Box<Statement>)` by calling
the statement typechecking path. No special handling needed — the statement
is typechecked like any other, and the synthesized transaction wraps the
result.

## Codegen

### LLVM Backend

The `synthesize_init_txn()` call is added in `run_llvm_compile()` and
`run_check()` in `main.rs`, right after import resolution. The LLVM backend
processes the synthesized `__init` transaction through the normal
`emit_direct_ssa_main()` path.

### Interpreter

Not yet wired for top-level `Statement` dispatch (the interpreter handles
program items in its own run loop — `TopLevel::Statement` dispatch must be
added alongside the async/reactor work).

## Key Files

| File | Responsibility |
|------|---------------|
| `src/ast.rs` | `TopLevel::Statement(Box<Statement>)`, `Program::synthesize_init_txn()`, collision-avoiding `find_unique_booted_name()` |
| `src/parser.rs` | Top-level statement parsing, `seen_top_level_stmt` flag, declaration-before-statement enforcement |
| `src/main.rs` | `program.synthesize_init_txn()` call after import resolution (in `run_llvm_compile`, `run_check`) |

## Remaining Work

- Interpreter dispatch for `TopLevel::Statement` (see async/reactor/triggers plan)
