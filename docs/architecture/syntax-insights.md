# Briv Syntax Insights

## Termination: `term`, not `return`

Briv does NOT have a `return` keyword. The correct way to produce a value
from a `defn` or `txn` body is `term expr;`:

```briv
defn add(x: Int, y: Int) -> Int {
    term x + y;
};
```

`Statement::Term(Some(expr))` stores the expression value to the function's
`%result` slot and branches to the convergence check. In a `defn`, convergence
is trivially proven — the function exits immediately. In a `txn`, the value
is stored and the postcondition check runs.

### Why not `return`?

`Statement::Return` exists in the compiler internals (LLVM backend, interpreter)
but is NOT a parseable Briv keyword. It's an internal IR construct used by
the backend for early exits. The doppelganger's previous use of `{ return ... }`
worked by accident: the parser produced `Statement::Return` for a then-unreserved
keyword, but this is not guaranteed to work across parser changes.

### Why not Rust-style last-expression?

Briv is not Rust. Expressions in a `defn` body are NOT implicit return values.
The body is `Vec<Statement>`, not `Vec<Expr>`. Only explicit `term expr;`
produces a return value. A bare `expr;` is an expression statement whose
value is discarded.

## Conditional Logic: `when`, not `if`/`then`/`else`

Briv does NOT have `if` expressions. `Expr::If(cond, then, else_)` exists
in the AST but is used only for the synthesis engine's internal representation
of SMT `ite` chains. It cannot be parsed from source text.

The correct Briv pattern for conditional execution is `when`:

```briv
when x0 == 5 { term 5; };
when x0 == -3 { term 3; };
term 128;
```

Each `when` guard is evaluated in order. The first matching guard executes
its body. An unguarded trailing expression/term serves as the else/finally.

## SMT ite Conversion

When the SMT solver returns an `ite` chain (table lookup), it must be
converted to `when` guards because:
1. `Expr::If` cannot be parsed from source (no `if` keyword)
2. `when` guards are the idiomatic Briv conditional pattern
3. Each `ite` arm becomes a `when` guard with a `term`
4. The `ite` else arm becomes the final unguarded `term`

The conversion happens in `doppelganger.rs::format_ite_body()`:
- Detects `Expr::If` chain in the synthesized body
- Recursively decomposes: `If(cond, then, else)` → `when cond { term then; };`
- Falls through: `else` (non-If) → `term else;`
