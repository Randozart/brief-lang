# Timing Bounds via Watchdog — `~?` Temporal Fallback Operator

**Date:** 2026-06-24
**Status:** Implementation in progress

## Overview

Integrate timing bounds (`cycles <= N`, `seconds <= N`) into the existing
watchdog system, with the `~?` temporal fallback operator for ergonomic
timeout handling. The fallback must be **provably terminable** at compile
time (0-cycle intrinsic or proven-termination function).

## Semantics of `?` / `?!` / `?#`

| Syntax | Compile-time | Runtime |
|--------|-------------|---------|
| `?` | Tries to prove termination. If unprovable, inserts runtime counter | Cycle counter + bounds check |
| `?!` | Full proof attempt but inserts runtime counter anyway | Always enforces at runtime |
| `?#` | Maximum proof effort (structural recursion + bounded counter + SMT) | Runtime counter as fallback |

## Syntax

### Expression form — `within ... ~?`

```brief
// Basic: time foo(), retry up to 3 times, fallback to bar()
let result = foo() within 10 cycles (3) ~? bar();

// Retry keyword (equivalent):
let result = foo() within 10 cycles retry 3 ~? bar();

// No retry (single attempt):
let result = foo() within 5 ms ~? default_value;

// Literal fallback:
let result = sensor() within 100 cyc (5) ~? 0;

// Proven function fallback:
let result = query() within 50 ms (2) ~? cache_lookup();
```

### Watchdog bracket form — `?[cycles <= N retry M] ~? fallback`

```brief
node process [x < N][x == N] ?[cycles <= 1000 retry 3] ~? log_timeout() {
    &x = x + 1; term;
};
```

### Import-level default

```brief
import frgn read_sensor() -> Int within 10 cycles (3) ~? 0;
// Every call to read_sensor() is automatically wrapped
```

### Chaining (right-associative `~?`)

```brief
let result = foo() within 10 cyc (3) ~? bar() within 5 cyc (2) ~? baz();
// Parsed as: foo() within 10 cyc (3) ~? (bar() within 5 cyc (2) ~? baz())
// If foo times out → try bar (5 cyc, 2 retries) → if that times out → run baz
```

### Retry syntax (both forms)

```brief
foo() within 10 cycles retry 3 ~? bar()     // explicit keyword
foo() within 10 cycles (3) ~? bar()          // compact parenthesized
```

### Time units (pl only — programming, not literature)

| Short | Full | Meaning |
|-------|------|---------|
| `cyc` | `cycles` | CPU cycles |
| `s` | `seconds` | Wall-clock seconds |
| `ms` | `milliseconds` | Milliseconds |
| `min` | `minutes` | Minutes |
| `ns` | `nanoseconds` | Nanoseconds |

## Operator: `~?`

A new multi-character token `TildeQuestion`. Design rationale:

- **`~`** = temporal uncertainty / waiting / approximation
- **`?`** = conditional fallback / "if not" / optionality (like Rust's `?`, Swift's `try?`)
- Combined: "if this times out, fall back to that"
- **Right-associative**: `a ~? b ~? c` = `a ~? (b ~? c)`
- **Precedence**: binds looser than function calls — `foo() within 10 cyc ~? bar()` times `foo()`, not `foo() within 10 cyc` as some unit
- Parser sees `~?` as a single token (not `~` followed by `?`), avoiding ambiguity with bitwise NOT

### The `retry` parenthesized syntax

`foo() within 10 cycles (3) ~? bar()` — the `(3)` is a retry count modifier attached to the `within` clause, parsed as part of the timing guard, not as a parenthesized expression. Distinguishable from `(foo() + 3)` because `(N)` follows a time unit keyword, not an expression.

## Type Rule

The body and the fallback must unify to the same type:

```brief
let x: Int = foo() within 10 cyc (3) ~? 0;           // OK: both Int
let x: String = foo() within 10 cyc (3) ~? "";        // OK: both String
let x: Int = foo() within 10 cyc (3) ~? "fallback";   // ERROR: Int vs String
```

`Expr::Within { body, bound, unit, retries, fallback }` has the unified type.

## Fallback Constraint: Only the Final Fallback Must Be Provably Terminable

In a chained expression `a ~? b ~? c`, only `c` (the final fallback)
must be proven terminable at compile time. Intermediate fallbacks like `b`
can themselves be `within ~?` chains — they may time out too.

**Final fallback rules** (last expression in any `~?` chain):
- A literal value (`0`, `""`, `true`, `null`) — trivially 0 cycles
- A function the proof engine can prove terminates in 0 cycles
- Only proven-terminable intrinsics (memory ops, etc.)
- NO recursion, NO FFI calls, NO `within ~?` chains

**Intermediate fallbacks** have no restrictions — they are normal expressions.

Rationale: the final fallback is the LAST resort — it must never fail.
The proof engine traverses the `~?` chain to find the final fallback and
applies the check there.

## Lexer Changes

| Token | Source | Notes |
|-------|--------|-------|
| `TildeQuestion` | `~?` | Single multi-char token |
| `Within` | `within` | Reserved keyword |
| `Retry` | `retry` | Reserved keyword |
| `Cycles` / `Cyc` | `cycles` / `cyc` | Time unit |
| `Seconds` / `S` | `seconds` / `s` | Time unit |
| `Milliseconds` / `Ms` | `milliseconds` / `ms` | Time unit |
| `Minutes` / `Min` | `minutes` / `min` | Time unit |
| `Nanoseconds` / `Ns` | `nanoseconds` / `ns` | Time unit |

## AST Changes

### New variant on `Expr`

```rust
pub enum Expr {
    // ... existing ...
    Within {
        body: Box<Expr>,
        bound: u64,
        unit: TimeUnit,
        retries: u64,           // 0 = single attempt, no retry
        fallback: Box<Expr>,    // must be provably terminable
    },
}
```

### New enum

```rust
pub enum TimeUnit {
    Cycles,
    Seconds,
    Milliseconds,
    Minutes,
    Nanoseconds,
}
```

### Extended `WatchdogSpec`

```rust
pub struct WatchdogSpec {
    pub condition: Expr,
    pub is_required: bool,
    pub cycles_bound: Option<u64>,
    pub seconds_bound: Option<u64>,
    pub is_proven: bool,
    pub retries: u64,
    pub fallback: Option<Box<Expr>>,
}
```

### Extended `ForeignBinding`

```rust
pub struct ForeignBinding {
    // ... existing fields ...
    pub default_watchdog: Option<(u64, TimeUnit, u64, Box<Expr>)>,
}
```

## Parser Changes

### Expression parsing order

After parsing any expression, check for `within` keyword:

```
parse_expr() → parse_within_tail(expr):
  if next token is "within":
    parse bound (u64)
    parse time unit (cycles/cyc/s/ms/min/ns)
    parse optional retry:  "(" u64 ")"  or  "retry" u64
    expect "~?"
    parse fallback expression (recursive, for chaining)
    return Expr::Within { body: expr, bound, unit, retries, fallback }
  else:
    return expr
```

Right-associativity: `a ~? b ~? c` → parse `a` as body, then `b ~? c` as
the fallback expression (recursive call).

### Watchdog bracket parsing

Extend the external watchdog `?[cond]` parser to detect:

```
?[cycles <= N retry M]    → cycles_bound = N, retries = M
?[seconds <= N retry M]   → seconds_bound = N, retries = M
```

Followed by optional `~? fallback_expr`.

### Import frgn parsing

```brief
import frgn name(args) -> Type within N cycles (M) ~? fallback_expr;
```

Store parsed bounds in `ForeignBinding.default_watchdog`.

## Typechecker Changes

For `Expr::Within`:
1. Typecheck `body` → type T
2. Typecheck `fallback` → type F
3. Unify(T, F) — must be the same type
4. Result type of the expression is T

## Proof Engine Changes

### Fallback terminability check

```rust
pub fn is_proven_terminable(expr: &Expr) -> bool {
    match expr {
        // Literals are trivially 0-cycle
        Expr::Integer(_) | Expr::Float(_) | Expr::Bool(_)
        | Expr::String(_) | Expr::Char(_) | Expr::Term => true,
        Expr::Identifier(_) => {
            // Look up the definition; check if it terminates in 0 cycles
            check_definition_terminates(name)
        }
        Expr::Call(name, args) => {
            // Intrinsic cost == Some(0) or proven function
            intrinsic_cost(name) == Some(0)
                && args.iter().all(|a| is_proven_terminable(a))
        }
        // NO frgn calls, NO recursion, NO within ~? chains
        _ => false,
    }
}
```

This runs at compile time during typechecking. If the fallback fails
the check, emit a compile error.

## Interpreter Changes

### `eval_expr` for `Expr::Within`

```
fn eval_within(expr: Expr::Within) -> Result<Value, RuntimeError>:
    let saved_budget = self.cycle_budget
    let saved_counter = self.cycle_counter
    let max_cycles = saved_counter + expr.bound

    for attempt in 0..=expr.retries:
        self.cycle_counter = saved_counter
        self.cycle_budget = max_cycles

        match self.eval_expr(&expr.body):
            Ok(val) => {
                self.cycle_budget = saved_budget
                return Ok(val)
            }
            Err(RuntimeError::Timeout(_)) => {
                // Reset state, continue to next retry
                self.state = saved_state  // rollback
            }
            Err(e) => {
                self.cycle_budget = saved_budget
                return Err(e)
            }

    // All retries exhausted — evaluate fallback
    self.cycle_budget = saved_budget
    self.eval_expr(&expr.fallback)
```

### FFI default watchdog wrapping

When `call_txn` invokes a `frgn` that has a `default_watchdog`, wrap the
call in an implicit `Expr::Within` using the import's defaults.

## LLVM Backend Changes

### `emit_expr` for `Expr::Within`

1. **Entry**: Alloca for retry counter + result register
2. **Retry loop header**: Load counter, compare with `retries`, branch
3. **Body**: Emit the guarded expression with cycle_count bounds check
4. **On timeout**: Increment retry counter, branch to loop header
5. **On retries exhausted**: Emit fallback expression
6. **Merge phi**: Phi node to select body result or fallback result

### Cycle counter connection

The `%cycle_count` field in `%State` (from Phase 4) is read before the
body starts, the bound is added, and a bounds check is inserted before
each statement in the body. On overflow, branch to the retry/trap logic.

## Syntax Highlighter (`brief.tmLanguage.json`, `dbrief.tmLanguage.json`)

### Operators section — add `~?` before `~`

Must come first so `~?` is matched as a single token before `~` grabs it.

```json
{
  "name": "keyword.operator.temporal-fallback.brief",
  "match": "~\\?"
}
```

Also: move `~/` (term-until) before `~` for the same reason (fixes
pre-existing bug where `~/` was never matched).

### Keywords section — add `within`, `retry`

```json
{
  "name": "keyword.control.temporal.brief",
  "match": "\\b(within|retry)\\b"
}
```

### Time units — add all forms

```json
{
  "name": "keyword.other.time-unit.brief",
  "match": "\\b(cycles|cyc|seconds|s|milliseconds|ms|minutes|min|nanoseconds|ns)\\b"
}
```

## Example Files

Create `examples/temporal-fallback.bv` demonstrating:

1. **Basic form**: `foo() within 10 cycles (3) ~? fallback()`
2. **Chaining**: `a() within 5 cyc (2) ~? b() within 5 cyc (2) ~? c()`
3. **Watchdog bracket**: `?[cycles <= 1000 retry 3] ~? handler()`
4. **Import default**: `import frgn read_sensor() -> Int within 10 cycles (3) ~? 0`
5. **Literal fallback**: `read() within 100 cyc ~? 0`
6. **All time units**: cyc, s, ms, min, ns
7. **Both retry syntaxes**: `retry 3` and `(3)`

## Architecture Docs

Update `docs/architecture/features/` with:

1. `docs/architecture/features/temporal-fallback.md` — full design
2. Update `docs/architecture/features/statement.md` — watchdog section

## File Manifest

| File | Change |
|------|--------|
| `syntax-highlighter/syntaxes/brief.tmLanguage.json` | Add `~?`, `within`, `retry`, time units |
| `syntax-highlighter/syntaxes/dbrief.tmLanguage.json` | Add `~?`, `within`, `retry`, time units |
| `src/lexer.rs` | `TildeQuestion`, `within`, `retry`, time unit tokens |
| `src/ast.rs` | `Expr::Within`, `TimeUnit`, `WatchdogSpec.*`, `ForeignBinding.default_watchdog` |
| `src/parser.rs` | Parse `within` expressions, retry in watchdogs, frgn defaults |
| `src/typechecker.rs` | Body/fallback type unification for `Expr::Within` |
| `src/proof_engine.rs` | `is_proven_terminable()` check |
| `src/interpreter.rs` | `eval_expr` for `Within`, retry loop, FFI default wrapping |
| `src/backend/llvm/emit_expr.rs` | LLVM codegen for `Within` |
| `src/backend/llvm/emit_toplevel.rs` | FFI default watchdog init |
| `examples/temporal-fallback.bv` | All syntax forms demonstrated |
| `docs/architecture/features/temporal-fallback.md` | Full design doc |

## Execution Order

1. ✅ Syntax highlighter — add `~?`, `within`, `retry`, time units
2. Lexer — `TildeQuestion` token, `within` keyword, time unit keywords
3. AST — `Expr::Within`, `TimeUnit`, `WatchdogSpec`/`ForeignBinding` extensions
4. Parser — within expressions, retry, frgn defaults
5. Typechecker — body/fallback type unification
6. Proof engine — `is_proven_terminable()` check
7. Interpreter — retry loop + fallback eval in `eval_expr`
8. LLVM backend — retry phi loop + fallback codegen
9. FFI import defaults — automatic wrapping of frgn calls
10. Tests — parser, typechecker, interpreter, LLVM, integration
11. Example files — `examples/temporal-fallback.bv`
12. Architecture docs — `temporal-fallback.md`
