# Async/Await — Statement-Level Concurrency

**Date**: 2026-06-19
**Phase**: 2 (Lexer, AST, Parser, Interpreter)

## Syntax

Three statement-level modifiers for concurrent callable execution:

| Form | Meaning |
|------|---------|
| `await f(x);` | Sequential: call `f(x)`, block until result, use result in next stmt |
| `async f(x);` | Fire-and-forget: call `f(x)`, discard result, continue immediately |
| `async await f(x);` | Fork-join: launch `f(x)`, continue executing, barrier at `term;` |

Capture form for `async await`:
```
async await let result = f(x);
```

The captured variable is available to subsequent statements but its value is
only guaranteed to be ready at the next `term;` (implicit barrier).

## AST

Three new `Statement` variants in `src/ast.rs`:

```rust
/// Await: await call_expr; — blocking wait for a callable result
Await { expr: Expr, modifiers: Vec<Hashtag> },

/// Async: async stmt; — fire-and-forget
Async { body: Box<Statement>, modifiers: Vec<Hashtag> },

/// AsyncAwait: async await expr; or async await let x = expr;
AsyncAwait { body: Box<Statement>, lhs: Option<String>, modifiers: Vec<Hashtag> },
```

## Parsing

In `parse_statement()`, `Token::Async` disambiguates:

1. `async` + `await` → `parse_async_await()` (delegates)
2. `async` + `rct`/`txn` → error (these are top-level only)
3. `async` + anything else → `Statement::Async { body: parse_statement(), .. }`
4. `Token::Await` alone → `Statement::Await { expr: parse_expression(), .. }`

The `parse_async_await()` helper handles optional `let x =` capture.

## Interpreter

All three forms execute sequentially in the interpreter (no actual parallelism):

- `Await` evaluates `expr`, stores result in `return_value`
- `Async` evaluates `body`, discards return value
- `AsyncAwait` evaluates `body`, optionally stores in variable via `lhs`

A `pending_barriers: Vec<Value>` field on `Interpreter` tracks
`async await` results. On `Statement::Term`, barriers are resolved
(cleared — no-op in interpreter since execution is already sequential).

## Backend Notes (Future Phases)

| Backend | Await | Async | AsyncAwait |
|---------|-------|-------|------------|
| LLVM | Sequential call + result | Call, no result used | Call + push promise, barrier at term |
| Webstack | `await f(x)` JS | `f(x)` (no await) | `let p = f(x)` + `Promise.all` at term |
| CIRCT | FSM stall state | Fire FSM state, no join | FSM fork + join at term |

## Tests

| Test | Location | What it verifies |
|------|----------|-----------------|
| `test_parse_await_expr` | parser.rs | `await compute(x);` → `Statement::Await` |
| `test_parse_async_expr` | parser.rs | `async compute(x);` → `Statement::Async` |
| `test_parse_async_await_expr` | parser.rs | `async await compute(x);` → `AsyncAwait` with `lhs: None` |
| `test_parse_async_await_let` | parser.rs | `async await let r = compute(x);` → `AsyncAwait` with `lhs: Some("r")` |
| `test_interp_await` | interpreter.rs | Await evaluates callable, captures result |
| `test_interp_async` | interpreter.rs | Async evaluates body, discards return |
| `test_interp_async_await` | interpreter.rs | AsyncAwait captures result, blocks term |
