# Temporal Fallback (`~?`)

The `~?` operator provides compile-time-proven timeouts for any expression.
It integrates with the existing watchdog system (`?[cycles <= N]`) and the
proof oracle (`?#`) to guarantee that programs never hang.

## Syntax

```
expr within <bound> <unit> [retry <N> | (<N>)] ~? <fallback_expr>
```

- `expr` — the expression being timed
- `bound` — maximum cycles/seconds before timeout
- `unit` — time unit: `cyc`, `s`, `ms`, `min`, `ns` (or full names)
- `retry N` or `(N)` — optional retry count (default 0)
- `~?` — the temporal fallback operator
- `fallback_expr` — expression evaluated on timeout (must be provably terminable)

## Chaining

`~?` is right-associative:

```
a() within 10 cyc (3) ~? b() within 5 cyc (2) ~? c()
// Parsed as: a() within 10 cyc (3) ~? (b() within 5 cyc (2) ~? c())
```

Only the final fallback (`c()`) must be provably terminable.

## Fallback Terminability

The final fallback must be proven to execute in 0 cycles:

| Allowed | Not Allowed |
|---------|-------------|
| Literals (`0`, `""`, `true`) | FFI calls (`frgn`) |
| 0-cycle intrinsics (`compile#`) | `within ~?` chains |
| Arithmetic on literals | Function calls with unknown cost |

## Integration with Watchdogs

```
node process [pre][post] ?![cycles <= 1000 retry 3] ~? log_timeout() {
    ...
};
```

## Implementation

| Component | Location | Behavior |
|-----------|----------|----------|
| Lexer | `src/lexer.rs` | `TildeQuestion` token, time unit tokens |
| AST | `src/ast.rs` | `Expr::Within`, `WatchdogSpec.retries/fallback` |
| Parser | `src/parser.rs` | Postfix `within ... ~?` parsing |
| Typechecker | `src/typechecker.rs` | Body/fallback type unification |
| Proof engine | `src/proof_engine.rs` | `is_proven_terminable()` |
| Interpreter | `src/interpreter.rs` | Retry loop, cycle budget, fallback eval |
| LLVM backend | `src/backend/llvm/emit_expr.rs` | GEP+load from %State for body; `within_counter` to avoid SSA collisions |

## LLVM Codegen Notes

The within body is evaluated in the **current block** (before branching to
within-specific blocks) to avoid SSA register dominance issues. For `Expr::Identifier`
bodies, a direct `GEP + load` from `%State` is emitted (bypassing all SSA caches:
`ssa_old_int_regs`, `ssa_state_reg`, `let_bindings`). A dedicated `within_counter`
(separate from `txn_counter`) ensures label/register uniqueness.

If the identifier is not resolvable in the current context (e.g., `init_state`
function), the fallback emits 0. The interpreter path handles all cases correctly.
