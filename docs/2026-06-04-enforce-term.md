# Enforce `term;` in Reactive Transaction Bodies

**Date:** 2026-06-04
**Status:** Implementing

## Background

`term;` is the body terminator for a reactive transaction. It marks the point
where the body iteration completes and control returns to the reactor. `escape;`
is the cancellation path — it halts the body and the prior state is preserved
(no mutation committed).

Currently the parser accepts bodies without `term;` or `escape;`. 14 of 20
benchmarks omit both, relying solely on the convergence contract
(`[count < N][count == N]`) for termination.

## The rule

Every reactive transaction body must have at least one valid termination path:

| Path | Mechanism | Effect |
|------|-----------|--------|
| `term;` | Explicit body terminator | Returns to reactor/precondition check |
| `escape;` | Cancellation | Halts body, prior state preserved |
| Convergence contract | `post ⇒ ¬pre` + monotonic counter | Self-terminating — compiler proves convergence |

At least one must be present. The proof engine already enforces this for proof
purposes; the parser will now enforce it syntactically.

## Implementation

### 1. Parser validation function (`src/parser.rs`)

```rust
fn validate_termination_path(&self, body: &[Statement], contract: &Contract) -> Result<(), SyntaxError> {
    // Check for term; or escape; anywhere in the body (including guarded blocks)
    if body.iter().any(|s| self.statement_tree_contains_term_or_escape(s)) {
        return Ok(());
    }
    // Check for convergence via structural contract pairing
    if is_convergent_contract(&contract.pre_condition, &contract.post_condition) {
        return Ok(());
    }
    Err(crate::errors::SyntaxError::InvalidStatement {
        reason: "reactive transaction has no valid termination path — add term;, escape;, or a convergent contract like [count < N][count == N]".to_string(),
        span: Span::dummy(),
    })
}
```

`is_convergent_contract` duplicates the structural check from `proof_engine.rs`
(`check_convergence` first two steps) — verifies `post ⇒ ¬pre` via expr comparison.

### 2. Add `term;` to all benchmarks missing it (14 files)

Zero runtime impact — SSA path already filters `term;`, non-SSA path already
emits `ret void` after the body.

### 3. nbody_sqrt liveness fix

- Add `import { io_pending }` + `frgn __print_float`
- Add `[count % 5000000 == 0] { __print_float(energy); }`
- Add `term;`
- Rebuild, re-bench

### 4. Escape LLVM codegen — DEFERRED

No benchmark uses `escape;`. The interpreter handles it correctly (stores
`prior_state`, restores on `Escaped`). Codegen follows the same "no features
solely for benchmarks" rule.

## No LLVM or interpreter changes

The SSA path correctly filters `Statement::Term` from the inline body.
The non-SSA path correctly emits `ret void` after the body. `escape;` emission
is deferred until needed by a benchmark.
