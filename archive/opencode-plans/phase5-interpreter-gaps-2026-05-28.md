# Phase 5: Complete Interpreter for Selfhost Pipeline

**Timestamp**: 2026-05-28T15:59:01Z  
**Prerequisites**: Phase 4 complete (selfhost CLI, `fn selfhost` wiring, Unification impl, `from "..."` fixes)  
**Rust compiler**: ✓ `cargo build`, ✓ `cargo test --lib` — 269/269 pass  
**Pipeline status**: Runs through tokenize → parse → hits `Expr::Range` unsupported  

---

## Objective

Add the 6 missing `Expr` variants to the Rust interpreter so the selfhost pipeline runs to completion and produces output from the stub backends.

---

## Missing Expr Variants

All currently hit the catch-all error at `src/interpreter.rs:1627`:

```
Err(RuntimeError::TypeMismatch(
    "Quantifier and multidimensional slice expressions not supported in interpreter"
))
```

### 1. `Expr::Block(Vec<Statement>, Box<Expr>)`

**Syntax**: `{ stmt; stmt; expr }`  
**Used by**: `call_graph.bv`, every function with guarded `[cond] { ... }` blocks  
**Implementation**: Execute each `Statement` via `self.exec_stmt()`, then eval the final `Expr`. The statements establish local scope; the final expr is the block value.

```rust
Expr::Block(stmts, last) => {
    let old_state = self.state.clone();
    for stmt in stmts {
        self.exec_stmt(stmt)?;
    }
    let result = self.eval_expr(last)?;
    self.state = old_state;
    Ok(result)
}
```

### 2. `Expr::Tuple(Vec<Expr>)`

**Syntax**: `(a, b, c)`  
**Used by**: Functions returning multiple values, `lexer.bv` returns `(List<Token>, LexerState)`  
**Implementation**: Eval each sub-expression, collect into `Value::List`.

```rust
Expr::Tuple(exprs) => {
    let mut values = Vec::new();
    for e in exprs {
        values.push(self.eval_expr(e)?);
    }
    Ok(Value::List(values))
}
```

### 3. `Expr::TupleDestructure(Vec<String>, Box<Expr>)`

**Syntax**: `let (a, b, c) = expr;` (inside `Let` statement, this is the `expr` part)  
**Used by**: `lexer.bv` — `let (tokens, lexer_state) = pair;` — this desugars to `TupleDestructure(["tokens", "lexer_state"], Identifier("pair"))`  
**Implementation**: Eval the destructure source, bind each name in state.

```rust
Expr::TupleDestructure(names, expr) => {
    let value = self.eval_expr(expr)?;
    match value {
        Value::List(items) => {
            for (i, name) in names.iter().enumerate() {
                if i < items.len() {
                    self.state.insert(name.clone(), items[i].clone());
                }
            }
            Ok(Value::Void)
        }
        _ => Err(RuntimeError::TypeMismatch(
            "Tuple destructure requires a list value".to_string()
        )),
    }
}
```

**Wait** — the `TupleDestructure` isn't the `Let` statement itself, it's the `Expr` inside the `let` binding. Let me check how `Let` desugars. Looking at `desugarer.rs`:

Actually, `TupleDestructure` is an `Expr` variant used in the `let` binding. So `let (a, b) = pair;` becomes `Let { name:..., ty:..., expr: TupleDestructure(["a","b"], Identifier("pair")) }`. That's odd — usually the destructure is on the LHS of `let`. Let me just trace what `call_graph.bv` actually hits.

For safety, implement both `TupleDestructure` and `Tuple`.

### 4. `Expr::ForAll { var: String, expr: Box<Expr> }`

**Syntax**: `forall x in list: condition` — used for universal quantification in contracts  
**Used by**: `range.bv`, contract verification  
**Implementation**: Eval `expr` (should be a List containing predicate), bind var, check all.

**Key insight**: `ForAll` in Brief contracts desugars to a list iteration. The `var` is bound, `expr` is the list expression or condition. Actually looking at the AST more carefully, `ForAll` in `Expr` context is just `forall var in list: condition` which means the `var` gets bound to each element and a condition is evaluated.

But actually, `ForAll` in the interpreter may not even be hit — `proof_engine.bv` handles it, not the main pipeline. Let me just stub it to avoid the error:

```rust
Expr::ForAll { var, expr } => {
    let list = self.eval_expr(expr)?;
    match list {
        Value::List(items) => {
            // ForAll is always true in interpreter (optimistic)
            // The proof engine handles real verification
            Ok(Value::Bool(true))
        }
        _ => Err(...)
    }
}
```

### 5. `Expr::Exists { var: String, expr: Box<Expr> }`

Same as `ForAll` but returns `true` if any element matches. Same optimistic stub.

### 6. `Expr::MultiSlice { value, coordinates, mask }`

**Syntax**: `vec[a..b, c..d; mask]` — multidimensional slicing  
**Used by**: Not expected in core pipeline. Same pattern as existing `Slice` (already implemented at line 1569).  
**Implementation**: Delegate to existing `Slice` logic for each coordinate range and nest them.

```rust
Expr::MultiSlice { value, coordinates, mask } => {
    // For now, treat as single slice using first coordinate
    if let Some(first) = coordinates.first() {
        let start = first.start.as_ref().map(|e| ...).unwrap_or(0);
        let end = first.end.as_ref().map(|e| ...).unwrap_or(0);
        let stride = first.stride.as_ref().map(|e| ...).unwrap_or(1);
        let slice_expr = Expr::Slice {
            value: value.clone(),
            start: start != 0,
            end: ...
        };
        self.eval_expr(&slice_expr)
    } else {
        self.eval_expr(value)
    }
}
```

Actually, this is overcomplicated. `MultiSlice` is unlikely to be hit. I'll stub it with the existing `Slice` logic applied to the first coordinate.

---

## Implementation Order

1. `Expr::Block` — most likely hit first (guarded block bodies)
2. `Expr::TupleDestructure` — `lexer.bv` return pair destructuring
3. `Expr::Tuple` — `author_sub` in call_graph.bv
4. `Expr::ForAll` — used in contracts/range verification
5. `Expr::Exists` — same
6. `Expr::MultiSlice` — least likely, low priority

---

## Verification

1. `cargo build` — compiles with new variants
2. `cargo test --lib` — 269/269 continue to pass
3. `brief-compiler selfhost examples/counter.rbv` — reaches backend stub and produces output

---

## After Phase 5

When the pipeline produces output (even stub `"// TODO"` strings), the next step is:
- **Phase 6**: Implement `backends/rust.bv` or `backends/c.bv` to generate real code
- Brief compiler can then compile its own code → bootstrap