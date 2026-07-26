# Pipe Chaining (`|>`, `.N|>`) — Implementation Plan

**Date**: 2026-06-22  
**Author**: rando (via OpenCode)  
**Phase**: Expression-level desugaring  
**Status**: Complete — see BUGS.md for any issues discovered during implementation

---

## 1. Motivation

Brief's reactive transaction model already enables elegant loop/iteration semantics.
But chaining function calls sequentially — especially the data-transformation pattern
of "take X, apply f, apply g, apply h" — is currently expressed as nested calls:

```brief
h(g(f(x)))
```

This is hard to read, hard to extend, and gets worse with each step. Pipe chaining
(`|>`) flips the order to match the dataflow direction:

```brief
x |> f() |> g() |> h()
```

The innovation is the **dot-skip `.N|>` variant**, which lets a downstream step reach
back to an earlier pipeline value — not just the immediately preceding result:

```brief
x |> f() |> g() .|> h()
// h receives f(x) (the value at position 1, skipping position 2)
```

## 2. Semantics

### Pipeline History Stack

Each `|>` step appends its result to a compile-time value stack. The reader looks
back from the current position:

| Position | Expression | Stack after |
|----------|-----------|-------------|
| 0 (initial) | `x` | `[x]` |
| 1 | `\|> f()` | `[x, f(x)]` |
| 2 | `\|> g()` | `[x, f(x), g(f(x))]` |
| 3 | `.\|> h()` | `[x, f(x), g(f(x)), h(f(x))]` |

The skip formula: step at pipe-position `pos` (1-indexed from initial, so step 1 is
the first `|>`) with skip `s` reads from `stack[pos - 1 - s]`.

- `|>` == skip `0` — reads the immediately preceding result
- `.|>` == skip `1` — skips one, reads the result before that
- `.2|>` == skip `2` — skips two, reads the result before that
- `.N|>` == skip `N` — reads the Nth result back

### Argument Routing

The pipeline value is passed as the **first argument** to the target function call.
If the target is a bare identifier (e.g. `x |> f`), it is auto-wrapped as `f(pipeline_value)`.

No additional arguments are automatically passed. For multi-argument routing,
use explicit `_` placeholder syntax (future work) or wrap in a lambda.

## 3. Examples

### Basic chaining

```brief
// Input: x = 42
// f: Int -> String, g: String -> Bool
x |> to_string() |> parse_bool()
// Desugars to: parse_bool(to_string(42))
```

### Dot-skip chaining

```brief
// a |> f() produces T1
// |> g() produces T2  
// .|> h() receives T1 (skip=1)
a |> f() |> g() .|> h()
```

### With `<:` subtype query (two-statement form)

```brief
// Step 1: query produces a filtered subset
let result : database { FILTER(.active); };
// Step 2: pipe the subset to a print function
result |> print_rows();
```

### Multiple skip levels

```brief
a |> f()   // pos 1: reads __p0
  |> g()   // pos 2: reads __p1
  .|> h()  // pos 3: reads __p1 (skip=1 from pos 2)
  ..|> i() // pos 4: reads __p0 (skip=2 from pos 2)
```

## 4. Desugaring Model

Pipe chains are **fully desugared at parse time** — they never reach the typechecker,
interpreter, or any backend. This means zero runtime overhead and zero codegen changes.

For `x |> f() |> g() .|> h()`:

```brief
// Desugared to block expression:
{
    let __pipe_0 = x;
    let __pipe_1 = f(__pipe_0);
    let __pipe_2 = g(__pipe_1);
    let __pipe_3 = h(__pipe_1);  // skip=1 → reads __pipe_{3-1-1}=__pipe_1
    __pipe_3
}
```

### Bare identifier auto-wrap

If the target is a bare `Identifier`, wrap it as a call with the pipeline value:

```brief
x |> f
// → { let __p0 = x; let __p1 = f(__p0); __p1 }
```

### Non-callable target

If the target is not a call and not an identifier, it's a parse error:
"pipe target must be a function call".

### Precedence

`|>` is at the LOWEST expression-precedence level, wrapping `parse_or`.
This ensures `a + b |> f()` parses as `(a + b) |> f()` — the entire `a + b`
expression is the pipeline initial value. This is consistent with bash pipes.

## 5. Implementation

### 5.1 AST (`src/ast.rs`)

Add two new types:

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct PipeStep {
    pub target: Box<Expr>,
    pub skip: usize,
}

// In Expr enum:
PipeChain {
    initial: Box<Expr>,
    steps: Vec<PipeStep>,
}
```

### 5.2 Lexer (`src/lexer.rs`)

Add `PipeGreater` token for `|>`. Must appear BEFORE `Pipe` (`|`) to ensure
greedy two-character matching by the `logos` lexer:

```rust
#[token("|>")]
PipeGreater,
```

### 5.3 Parser (`src/parser.rs`)

Insert `parse_pipe_chain` as the topmost expression parser, wrapping `parse_or`:

```
parse_expression → parse_pipe_chain
  parse_pipe_chain:
    1. Parse initial via parse_or
    2. Loop checking current token:
       - Token::PipeGreater → step with skip=0
       - Token::Dot → peek ahead:
         - If followed by Integer then PipeGreater → step with skip=N
         - If followed directly by PipeGreater → step with skip=1
         - Otherwise → break (not a pipe)
    3. Parse target via parse_unary (not parse_primary — allow unary ops)
    4. If no steps consumed → return initial expr
    5. If steps consumed → return Expr::PipeChain
```

**Dot ambiguity:** The `.` token in Brief is used for field access (`.field`).
`.N|>` parsing only activates when `.` is immediately followed by `|>` (with
optional integer in between). At the parse_pipe level, we're already past
postfix operators, so `.` at this level is unambiguous — field access is
handled by `parse_postfix_expr` at a much lower level.

Wait — actually, `.` is consumed in `parse_postfix_expr`, not at the primary
level. Since `parse_pipe_chain` wraps `parse_or` which descends through all
the other levels, by the time we return to `parse_pipe_chain`, all field accesses
on the initial expression have already been consumed. The `.` at the pipe level
would only appear if there's no field access — which means it's our dot-skip.

**Exception:** For auto-wrapping, the target is parsed at `parse_unary` level
to allow expressions like `!f()`, then checked for callability.

### 5.4 Desugarer (`src/desugarer.rs`)

Add a method `desugar_expr` that recurses through an expression tree and
transforms any `PipeChain` nodes into `Block` expressions.

The desugaring produces:

```rust
let mut stmts = Vec::new();
stmts.push(Statement::Let {
    name: "__pipe_0".into(), ty: None,
    expr: Some(pipe.initial),
    address: None, address_expr: None, bit_range: None,
    constraint: None, is_override: false, modifiers: vec![],
});
for (i, step) in pipe.steps.iter().enumerate() {
    let pos = i + 1; // 1-indexed position
    let read_idx = pos - 1 - step.skip;
    let read_name = format!("__pipe_{}", read_idx);
    let read_expr = Expr::Identifier(read_name);
    let target_call = prepend_pipeline_arg(&step.target, read_expr);
    stmts.push(Statement::Let {
        name: format!("__pipe_{}", pos), ty: None,
        expr: Some(target_call),
        address: None, address_expr: None, bit_range: None,
        constraint: None, is_override: false, modifiers: vec![],
    });
}
let final_expr = Expr::Identifier(format!("__pipe_{}", pipe.steps.len()));
Expr::Block(stmts, Box::new(final_expr))
```

The `prepend_pipeline_arg` helper:
- If target is `Expr::Call(name, args)` → `Expr::Call(name, [pipeline_val] + args)`
- If target is bare `Expr::Identifier(name)` → `Expr::Call(name, [pipeline_val])`
- Otherwise → wrap as call via `Expr::Call("__pipe_apply", [target, pipeline_val])`

Call `desugar_expr` from the main `desugar` method on every expression found
in statement bodies, contracts, and initializers.

## 6. `<:` Query Compatibility

The `let x : database { FILTER(.active); }` syntax is a special form in the
parser that produces `Expr::SubtypeProjection` as the let-binding's value.
Pipe chaining adds zero additional complexity here because:

```
let result : database { FILTER(.active); };
result |> print_rows();
```

The `|>` operator works on any expression — including an identifier bound to
a subtype projection result. The two-statement form is idiomatic and clear.

**Inline form** (`let x : db { ... } |> f()`) is NOT implemented in this phase.
It would require changes to the let-statement parser to optionally consume `|>`
after the subtype projection block. Future work if needed.

## 7. Syntax Reference

| Syntax | Skip | Reads from |
|--------|------|-----------|
| `x \|> f()` | 0 | Immediately preceding result |
| `x \|> f() .\|> g()` | 1 | Result before the preceding one |
| `x \|> f() ..\|> g()` | 2 | Two before the preceding |
| `x \|> f() .2\|> g()` | 2 | Same as `..\|>` |
| `x \|> f() .N\|> g()` | N | N before the preceding |

## 8. Files Modified

| File | Change |
|------|--------|
| `src/ast.rs` | Add `PipeChain` struct, `PipeStep` struct, `Expr::PipeChain` variant |
| `src/lexer.rs` | Add `PipeGreater` token |
| `src/parser.rs` | Add `peek2` field, `parse_pipe_chain`, dot-skip guard in `parse_postfix` |
| `src/desugarer.rs` | Add `desugar_expr`/`stmt`/`toplevel` walkers, `desugar_pipe_chain` |
| `src/interpreter.rs` | E2E tests for pipe chaining |
| `src/analysis/dependency_graph.rs` | `unreachable!` arm |
| `src/backend/llvm/mod.rs` | `unreachable!` arm |
| `src/proof_engine.rs` | `unreachable!` arm |
| `src/symbolic.rs` | `unreachable!` arm |
| `docs/architecture/features/pipe.md` | Architecture doc |
| `learn-brief/01-basics.md` | Pipe chaining tutorial section |
| `examples/pipe-chain.bv` | Example program |
| `examples/pipe-skip.bv` | Example program |
| `docs/plans/2026-06-22-pipe-chaining.md` | This file |

## 9. Example .bv Files

### `examples/pipe-chain.bv` — Basic pipe chaining

```brief
defn add_one(x: Int) -> Int { term x + 1; };
defn double(x: Int) -> Int { term x * 2; };

txn main() [true][true] -> Bool {
    let result: Int = 5 |> add_one() |> double() |> add_one();
    term! -> print_int#(result);   // prints 13
};
```

### `examples/pipe-skip.bv` — Dot-skip chaining

```brief
defn square(x: Int) -> Int { term x * x; };
defn add_one(x: Int) -> Int { term x + 1; };
defn double(x: Int) -> Int { term x * 2; };

txn main() [true][true] -> Bool {
    let result: Int = 3 |> square() |> add_one() .2|> double();
    term! -> print_int#(result);   // prints 6
};
```

### `examples/pipe-query.bv` — Pipe with `<:` query (two-statement form)

```brief
struct Record { id: Int, active: Bool };

defn print_count(n: Int) -> Bool { term print_int#(n); };

txn main() [true][true] -> Bool {
    let result : database { FILTER(.active); };
    result |> print_count();
    term;
};
```

## 10. Test Coverage

### Parser tests (`src/parser.rs`)

- `test_parse_basic_pipe`: `x |> f()` parses to `PipeChain(x, [f])`
- `test_parse_pipe_chaining`: `x |> f() |> g()` gives correct step list
- `test_parse_dot_skip`: `x |> f() .|> g()` gives skip=1 on second step
- `test_parse_dot_skip_two`: `x |> f() ..|> g()` gives skip=2
- `test_parse_dot_n_skip`: `x |> f() .2|> g()` gives skip=2
- `test_pipe_precedence_and`: `a + b |> f()` binds as `(a + b) |> f()`
- `test_pipe_precedence_or`: `a || b |> f()` binds as `a || (b |> f())`
- `test_pipe_no_pipe`: `x + y` returns `Add` not `PipeChain`
- `test_pipe_target_auto_wrap`: `x |> f` wraps bare ident as call
- `test_pipe_semicolon_terminates`: chain stops at `;`

### Desugarer tests (`src/desugarer.rs`)

- `test_desugar_basic_pipe`: `x |> f()` → block with let bindings
- `test_desugar_pipe_chaining`: `x |> f() |> g()` → two let bindings, correct read indices
- `test_desugar_dot_skip`: `x |> f() .|> g()` → second binding reads from __pipe_0
- `test_desugar_auto_wrap`: `x |> f` → `f(__pipe_0)`

### Interpreter E2E tests (`src/interpreter.rs`)

- `test_pipe_chain_e2e`: Run desugared block, verify correct output
- `test_pipe_dot_skip_e2e`: Dot-skip routes to correct predecessor

### How to run

```bash
cargo test --lib -- pipe
```
