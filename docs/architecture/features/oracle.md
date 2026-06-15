<!-- 2026-06-15. ?# proof oracle system. -->

# `?#` Proof Oracle

## Purpose
A third watchdog form (alongside `?[...]` optional and `?![...]` required) that proves termination using the full strategy palette. If no static strategy succeeds, injects a runtime fuel counter with state rollback and a user-supplied handler.

## Syntax
```brief
?#[handler] {
    // body to prove terminating
};

?#[&retries = retries + 1] {
    risky_loop();
    deeper_recursion(n - 1);
};
```

The handler block (between `#[` and `]`) executes on fuel exhaustion after rolling back all state changes made by the body.

## Three Watchdog Forms

| Form | Name | Semantics |
|------|------|-----------|
| `?[cond]` | Optional | "Check this if you can" — runtime check at `term`, optional preemptibility analysis |
| `?![cond]` | Required | "Prove this or fail to compile" — fatal on proof failure |
| `?#[...]` | Oracle | "Prove halting statically; if not, inject fuel + rollback with handler" |

## Compile-Time Strategies (in order)

| Strategy | What it checks | Scope | Status |
|----------|---------------|-------|--------|
| **Bounded counter** | Transition graph finds `[i < N][i == N]` convergence | `rct txn` bodies | ✅ Exists |
| **Structural recursion** | Recursive call on strictly smaller sub-term (`n-1`, `list.tail()`) | `defn` recursion | ✅ New (2026-06-15) |
| **Fuel budget injection** | `--optimize-budget` iteration cap becomes the fuel limit | Fallback when all else fails | ✅ New (2026-06-15) |

## Runtime Fuel Injection (when compile-time fails)

When no static strategy proves termination, the interpreter injects:

```
Statement::Oracle { body, handler, .. } => {
    saved_state = self.state.clone()
    saved_prior = self.prior_state.clone()
    exec_stmts_with_fuel(body, fuel_limit=500)
    on success: continue
    on FuelExhausted:
        self.state = saved_state       // rollback
        self.prior_state = saved_prior
        execute handler statements      // handler writes survive
}
```

### Fuel counter mechanics

- `oracle_fuel: Option<u64>` field on `Interpreter`
- Decremented on every `exec_stmt` call via `oracle_fuel.as_mut()`
- Decremented on every recursive `call_defn` call
- When it hits zero, `RuntimeError::FuelExhausted` is returned
- `exec_stmts_with_fuel` catches the error, restores fuel state

### State rollback

On fuel exhaustion:
1. **Roll back** `self.state` and `self.prior_state` to pre-oracle snapshot
2. **Execute** handler statements (their writes survive)
3. **Continue** the enclosing program — no crash, no abort

## AST

```rust
Statement::Oracle {
    handler: Vec<Statement>,
    body: Vec<Statement>,
    span: Option<Span>,
}
```

## Parser

`?#` is tokenized as `Question` `HashBracket`. The parser extracts the handler block (`#[...]`) and the body block (`{...}`). Handler is a full statement list.

## Interpreter

The interpreter executes Oracle in a sequential fallback with fuel injection. If the proof engine proves structural recursion at compile time (`check_structural_recursion` in `verify_program`), the `?#` compiles transparently — no fuel, no rollback, zero overhead.

## Structural Recursion Checker

`check_structural_recursion` in `proof_engine.rs`:

1. Walks all `TopLevel::Definition` items
2. Detects recursive calls via `contains_call_to(body, defn_name)`
3. Checks for strictly decreasing arguments: currently `n - 1` / `n - literal` patterns via `is_decreasing_expr`
4. Reports `P021` error if recursion is unproven

Standalone helpers: `contains_call_to`, `expr_contains_call_to`, `check_decreasing_arg`, `check_decreasing_arg_expr`, `is_decreasing_expr`.

## Backend coverage

| Backend | Status |
|---------|--------|
| Interpreter  | ✅ Fuel injection + rollback + handler |
| LLVM         | ⚠️ Sequential fallback (no fuel counter yet) |
| Webstack     | ⚠️ Stub |
| Dead backends | ⚠️ Stubs (zero-fix policy) |

## Tests

| Test | Location | What it verifies |
|------|----------|-----------------|
| `test_oracle_executes_body` | `interpreter.rs` | Body runs normally when fuel is sufficient |
| `test_oracle_fuel_exhausts_runs_handler` | `interpreter.rs` | Fuel exhaustion triggers state rollback + handler |

## Future Extensions

- **SMT ranking function**: encode loop body as transition relation, ask Z3 for decreasing measure
- **Runtime thrash detection**: autocorrelation on tick counter / field-write pattern
- **`?@` ordering hints**: explicit strategy priority list like `?#[handler] @[structural > z3(3s) > fuel(10000)]`
- **LLVM fuel injection**: emit descending counter + early-exit to rollback path in the LLVM backend
