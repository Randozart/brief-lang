# Postcondition Syntax Fix — #Term Hashword + [[post] Bracket Correction

Date: 2026-07-28
Status: Plan → Implementation

## Design Decisions

### Decision 1: `#Term` as Return-Value Hashword

**Problem**: `@result` in `[[post]]` expressions is parsed as `Expr::Quoted(b"result")`
(quoted symbol bytes), not as a variable reference. Fixing the parser to treat
`@result` specially is a special-case hack.

**Solution**: Use `#Term` instead — a PascalCase hashword following `#Int`, `#Float`,
`#Bool`, `#Bits` convention. The lexer already treats `#` as a valid identifier
character (lexer.rs:25-26), so `#Term` parses as `Expr::Identifier("#Term")` — a
plain variable lookup. No parser change needed.

**Impact**: Zero parser changes. Three bind sites updated.

### Decision 2: `[[post]` — Single Closing Bracket

**Spec**: `[[post]` = `[true][post]` — postcondition-only. Exactly one `]` closes it.

**Problem**: My parser currently has an optional second `]` via `self.eat(&Token::RBracket)`,
which incorrectly accepts `[[ ... ]]`.

**Fix**: Remove `self.eat(&Token::RBracket)` — `[[ ... ]` with single `]` is the only
valid form. `[[ ... ]]` is rejected.

### Decision 3: Build-Time Postcondition Verification

**Problem**: Removed earlier because `@result` parsing didn't work.

**Solution**: Re-add `verify_postcondition` in `assert.rs`, now using `#Term`.
The assertion checker evaluates the postcondition expression with `#Term` bound
to the function's return value for each example. If violated, the build fails.

## Exact Edits (5 files, ~25 lines)

### File 1: `src/parser/definitions.rs` (1 line removed)

Design: `[[post]` is `[[` + expr + `]` — single close bracket. The `self.eat`
makes `]]` incorrectly valid.

```rust
// Before (line ~941):
                self.eat(&Token::RBracket);
                post_expr

// After:
                post_expr
```

### File 2: `src/derive/verify.rs` (2 lines changed)

Design: `#Term` replaces `@result` as the return-value binding name.

```rust
// Before (line 129):
                post_ctx.bind("@result", crate::interpreter::Value::Int(*n));

// After:
                post_ctx.bind("#Term", crate::interpreter::Value::Int(*n));
```

Also the Constructor match (same function, line 131):
```rust
// Before:
                crate::interpreter::Value::Constructor(_, _) => {
                    post_ctx.bind("@result", val.clone());
                }
// After:
                crate::interpreter::Value::Constructor(_, _) => {
                    post_ctx.bind("#Term", val.clone());
                }
```

### File 3: `src/derive/mod.rs` (3 lines changed)

Design: `compute_correct_output` matches `@result` to detect equality patterns.

```rust
// Before (line ~143):
        if matches!(lhs.as_ref(), Expr::Identifier(n) if n == "@result") {

// After:
        if matches!(lhs.as_ref(), Expr::Identifier(n) if n == "#Term") {
```

### File 4: `src/derive/assert.rs` (re-add verify_postcondition, ~30 lines)

Design: Same structure as `verify_example` but evaluates the postcondition expression
with `#Term` bound to the function's actual output. If the postcondition evaluates
to false (Bits([0]) or Int(0)), the build fails.

```rust
// Add after line 107 (after example verification loop in verify_item):

    // 2026-07-28: Check [[postcondition]] for each example.
    if let Some(ref post) = derivation.postcondition {
        for (i, example) in derivation.examples.iter().enumerate() {
            if let Err(msg) = verify_postcondition(name, i, post, example, interp) {
                errors.push(msg);
            }
        }
    }
```

```rust
// Add new function before the closing of the module:

/// Verify a [[postcondition]] for a given example.
fn verify_postcondition(
    name: &str,
    index: usize,
    post: &Expr,
    example: &DerivationExample,
    interp: &mut Interpreter,
) -> Result<(), String> {
    let args: Result<Vec<Value>, _> = example.inputs
        .iter()
        .map(|input| interp.eval_expr(input))
        .collect();
    let args = match args {
        Ok(a) => a,
        Err(e) => return Err(format!(
            "{} example {}: input evaluation failed: {}",
            name, index + 1, e
        )),
    };
    let result = match interp.call_function(name, &args) {
        Ok(r) => r,
        Err(e) => return Err(format!(
            "{} example {}: body execution failed: {}",
            name, index + 1, e
        )),
    };
    // Bind #Term and evaluate postcondition
    interp.state.insert("#Term".into(), result.clone());
    let post_result = match interp.eval_expr(post) {
        Ok(v) => v,
        Err(e) => {
            interp.state.remove("#Term");
            return Err(format!(
                "{} example {}: postcondition evaluation failed: {}",
                name, index + 1, e
            ));
        }
    };
    interp.state.remove("#Term");
    let pass = match &post_result {
        Value::Int(n) => *n != 0,
        Value::Bits(b) => b.iter().any(|x| *x != 0),
        _ => false,
    };
    if pass { Ok(()) } else {
        Err(format!(
            "{} example {}: postcondition violated (result={:?}, post={:?})",
            name, index + 1, result, post_result
        ))
    }
}
```

### File 5: `benchmarks/popcount_derive.bv` (1 line changed)

Design: `#Term` replaces `@result` in the postcondition expression.

```briev
// Before:
} [[ @result >= 0 && @result < 64 ]];

// After:
} [[ #Term >= 0 && #Term < 64 ]];
```

## Verification

1. `cargo test --lib` — 1167 tests pass
2. `brievc derive popcount_derive.bv` — produces body
3. `brievc build popcount_derive.derive.bv` — assertion checks postcondition
4. If the ite chain violates `#Term < 64` for any example, build FAILS
