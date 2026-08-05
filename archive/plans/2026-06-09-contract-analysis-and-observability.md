<!-- 2026-06-09 -->

# Contract Analysis & Observability Plan

## Motivation

The compiler does massive work — folding loops, eliminating dead fields,
precomputing closed forms — and says nothing. The user has no idea whether
their program was fully precomputed, fell back to a runtime loop, or will
produce any observable output. A program that hangs with no output is
treated the same as a program that ran correctly in 0.001s.

Second, contract bounds like `[b0 > 0.0 && b0 < 1.0]` are checked by the
proof engine for correctness but never fed into the optimizer. FFI results
are fully opaque — no interval, no `value_set_size`, no path pruning. This
is a gap: Briv's contracts should inform the optimizer about what values
are possible.

---

## M0 — Observability Diagnostics (immediate)

Add 4 messages to the existing `self.warnings: Vec<String>` infrastructure,
which already has A002 (dead field) and A003 (pure-counter fold info).

### A000 — Fully Precomputed

| Field | Value |
|-------|-------|
| **Where** | End of `emit_folded_main` / `emit_precomputed_main` |
| **Trigger** | No runtime loop emitted — main is `init_state + stores + ret i32 0` |
| **Message** | `info: program fully precomputed — no runtime loop emitted. If unexpected, increase --optimize-budget or add frgn calls.` |

### A001 — Budget Exceeded

| Field | Value |
|-------|-------|
| **Where** | `select_optimization_strategy` after budget < bound check |
| **Trigger** | `optimize_budget < min_simulated_iterations` for a pure-counter txn |
| **Message** | `info: budget exceeded for '{txn}' (bound {bound}, budget {budget}). Emitting runtime loop with counter.` |

### A004 — Zero Observability Warning

| Field | Value |
|-------|-------|
| **Where** | After dispatch mode selected in `emit_main` / `emit_folded_main` |
| **Trigger** | Runtime loop body contains zero `frgn` calls (checked via `statement_contains_ffi` or similar) |
| **Message** | `warning: runtime loop in '{txn}' has no observable side effects — LLVM may eliminate it entirely. Add frgn calls for output, or this program may run without producing results.` |

### A005 — Dispatch Path Info

| Field | Value |
|-------|-------|
| **Where** | After `classify_txns` / `select_dispatch_mode` |
| **Trigger** | Any dispatch path selected for a transaction |
| **Message** | `info: txn '{txn}' dispatched via {path} ({details})` |

Path strings: `"pure counter fold"`, `"folded SSA"`, `"enum dispatch"`,
`"reactor loop"`, `"async thread pool"`.

---

## M1 — Integer Range Constraints `<: [lo..hi]`

### Syntax

```briv
let x: Int <: [0..100] = some_source();
let y: Int <: [lo..hi] = another_source();
```

The `let` declaration must have an explicit type (inference not sufficient
for constraint matching — the programmer must opt in).

### AST

```rust
pub enum RangeConstraint {
    Range(Box<Expr>, Box<Expr>),     // lo..hi  (any base type)
    Regex(Box<Expr>),                 // string pattern
}

// Type::Constrained(Box<Type>, RangeConstraint) — already exists?
// Or add a new variant. Check what Type::Constrained currently does.
```

### Parser

In `parse_type()`, after parsing the base type:
- If peek is `<:`, consume it
- If peek is `[`, consume it
- Parse `expr .. expr` or `expr` as the constraint
- If peek is `]`, consume it
- Return `Type::Constrained(base_type, RangeConstraint::Range(lo, hi))`

### Typechecker

- Validate that `lo` and `hi` have the same type as the base type
- For `Int <: [0..100]`, check `0` and `100` are `Int`
- For `Float <: [0.0..1.0]`, check `0.0` and `1.0` are `Float`
- For `String <: [@"pattern"]`, check the constraint is `String`

### Region Analyzer (`src/analysis/region.rs`)

- In `register_state_decl`, when a state variable has
  `Type::Constrained(base, RangeConstraint::Range(lo, hi))`:
  1. Evaluate `lo` and `hi` to concrete values (constants or constant
     expressions)
  2. Compute `interval` from the result
  3. Populate `VarInfo { classification: Bounded, interval, value_set_size }`
- The existing pipeline in `compute_value_set_size` and `build_budget_plan`
  will automatically incorporate these into budget checks

### Optimizer (`src/backend/llvm/optimizer.rs`)

- `extract_trigger_keys` already handles `Bounded` classification — the
  interval feeds into dispatch path counting
- New: branch pruning — if `[x < 50]` and `x ∈ [50..100]`, the path is
  impossible and the guarded block is dead code
- This is an enhancement over current behavior where only loop-counter
  relations (`count < bound`) are recognized

### Proof Engine

- Already evaluates path constraints. With the interval, it can prove
  `x < 50` is false when `x ∈ [50..100]` without simulating the state.

---

## M2 — Float Range Constraints `<: [lo..hi]`

Same as M1, but with float-specific interval arithmetic:

- `f32` bits for interval bounds (32-bit unsigned comparison after
  canonicalization: `-0.0 == 0.0`, NaN excluded)
- `interval` uses `(u32, u32)` for f32 or `(u64, u64)` for f64 for
  ordering-compatible bit patterns
- Disjointness checks must account for NaN: a NaN value always fails
  every comparison, so it's always a separate error path (handled by
  existing FFI error mechanics)
- `expr_to_interval` handles `Expr::Float(f)`,
  `Expr::Literal(LiteralExpr::Float(f))`

---

## M3 — Raw String Syntax `@"..."` + Regex Constraint

### Lexer

When the lexer sees `@"`, enter raw-string mode:
- Read until closing `"` — no escape processing
- Store the raw content as-is
- Emit `Token::String(s)` — same token type as regular strings

The parser never knows whether a string was raw or escaped.

### Constraint

```briv
let email: String <: [@"\A(?:[a-z0-9]+)"] = get_input();
```

Parses as `Type::Constrained(String, RangeConstraint::Regex(pattern))`.

### Analysis

- The forward-DFA is compiled from the pattern
- DFA state count provides a `value_set_size` estimate when combined with
  a length bound (`[len < 64]`)
- If within `--optimize-budget`, the optimizer could enumerate matching
  strings (practical only for tiny DFAs)
- In practice, most patterns will exceed budget → A005 reports
  `"regex constraint — assumption only, exceeds budget"`
- Always valid as an assumption: the optimizer assumes the constraint holds

### Where `@"..."` applies

Everywhere a string literal is valid:
```briv
frgn open(path: String) -> Int;
let fd = open(@"C:\tmp\file");       // raw path
let name: String <: [@"[A-Z]+"];     // regex constraint
```

---

## Benchmark Audit

One-by-one, compile + link + BOUND=5:

1. `ring_buffer` — verify links now (was `@str.0` blocked)
2. `sparse_dispatch`, `cancel_math`, `bit_clear`, `interval_step`,
   `queue_drain` — compile + link + output check
3. `float_math`, `float_math_nonzero`, `const_heavy` — C reference
   exits 6, GDB per-binary
4. `async_counters`, `fannkuch_redux`, `mandelbrot`,
   `kalman_filter_runtime`, `knucleotide` — heavier, verify output
5. `nbody_newton`, `nbody_sqrt` — budget=2048, verify convergence
6. `queue_drain` — assess asymmetry, plan `_sym` / `_idio` split

---

## Backlog

- `iir_filter_runtime.bv` — waits for M2. Coefficients read from
  `__get_env_float` with `<: [lo..hi]` constraints.
- `<: [single_value]` — `let x: Int <: [42]` as degenerate range
  (lo == hi). Natural extension of M1.
- `_sym` / `_idio` pairs for asymmetric benchmarks.
