# Proof Engine — Convergence Analysis

**Date:** 2026-06-09  
**Phases:** 1–4, Bug A/B/C Fixes, Convergence Fixes  
**Status:** 24/24 benchmarks pass contract verification

## Overview

The proof engine verifies reactive transaction contracts using two
complementary approaches:

1. **Syntactic convergence detection** (`check_convergence`) — scans the
   transaction body for structural patterns (increments, decrements,
   popcount decay) that guarantee termination.

2. **Symbolic path exploration** (`verify_contract_implication`, via
   `enumerate_paths_recursive`) — explores all execution paths through
   the body and checks that the post-condition is satisfiable.

When syntactic convergence succeeds, symbolic execution is skipped
entirely — the structural proof is stronger.

## `check_convergence` Steps

```
Step 1: extract_var_bound(post_condition)
  → (var, bound_expr) — the counter variable and its limit
  Fails if post-condition is not a comparison on an identifier.

Step 2: Validate post → ¬pre
  Verifies the post-condition being true implies the pre-condition is false.
  Handles AND/OR preconditions via extract_var_relation.
  Supports: Eq(==), Ne(!=), Lt(<), Le(<=), Gt(>), Ge(>=)

Step 3: Detect increment/decrement on var
  Scans the body for an assignment to var. Recognizes:
  - count = count + N      (bare Add, step=N, dir=+1)
  - count = count - N      (bare Sub, step=N, dir=-1)
  - count = count + Sub(N, M)  (algebraic cancellation, step=N-M)
  - count = (count + N) - M    (compound increment, step=N-M)
  - reg & (reg - 1)            (popcount decay, step=1, dir=-1)
  Fails if step == 0 (no recognized pattern).

Step 4: Bound invariance
  If bound is a variable, verifies it is never assigned in the body.
  (Literal bounds like `count == 0` skip this check.)

Step 5: Overshoot detection
  When step > 1 and post-condition is exact equality (var == bound),
  verifies that (bound - initial) % step == 0 to prevent overshoot.
  For runtime-determined bounds, conservatively rejects.
```

## Bug A — Guard-Taken Path Dropping

**File**: `src/proof_engine.rs`, `enumerate_paths_recursive`  
**Date**: 2026-06-09  
**Root cause**: When processing `Statement::Guarded`, the true branch
recursed into the guard body only — never continuing to the remaining
body after the guard. If the guard body had no `term` inside it, the
path was silently dropped with no path produced.

**Fix**: After recursing into the guard body, if `true_paths` is empty,
continue exploring `body[i+1..]` (remaining body after the guard) with
the true state. This ensures guard-taken paths reach `term`.

**Also**: `body[1..]` was replaced with `body[i+1..]` using `.enumerate()`.
The old `body[1..]` skipped the first element of the full body, not the
current guard — causing guards to re-process themselves exponentially.

## Bug B — Missing Mod/Div in eval_numeric

**File**: `src/proof_engine.rs`, `eval_numeric`  
**Date**: 2026-06-09  
**Root cause**: `eval_numeric` handled `Add`, `Sub`, `Mul` but fell
through to `_ => None` for `Mod` and `Div`. Guard conditions like
`count % 5000000 == 0` could never be evaluated, making both guard
branches appear infeasible.

**Fix**: Added `Expr::Mod` and `Expr::Div` cases for concrete integer
operands. Returns `None` on division by zero.

## Bug C — Hidden Negation in Error Output

**File**: `src/proof_engine.rs`, `verify_contract_implication`  
**Date**: 2026-06-09  
**Root cause**: The error printer at line 803-805 rendered only
`format_expr(&constraint.condition)`. The `is_negated` flag was silently
dropped. Negated constraints printed identically to non-negated ones,
making P008 errors impossible to diagnose.

**Fix**: Added `¬` prefix: `if constraint.is_negated { "¬" } else { "" }`.

## Convergence Analysis Fixes

### AND-precondition extraction

`check_convergence` step 2 previously rejected AND/OR preconditions like
`bound > 0 && count < bound` because `check_pre_matches` only handled
bare relations. Fix: `extract_var_relation` recurses into AND trees to
find the sub-expression involving the counter variable.

### Popcount decay (`reg & (reg - 1)`)

The `is_self_minus_one` helper detects `reg & (reg - 1)` assignments.
Each iteration clears exactly one set bit, ensuring monotonic progress
toward zero. Handles both `Expr::Integer(1)` and
`Expr::Literal(LiteralExpr::Integer(1))` variants.

### Algebraic cancellation (`count + (R + 1 - R)`)

`eval_const_expr` now resolves constant identifiers through the
`initial_values` map (built from `StateDecl` and `Constant` declarations),
enabling constant folding of expressions like `(R + 1) - R` → `1`.

### Compound increment (`(count + N) - M`)

The `Expr::Sub` arm now checks for `Add(var, N)` on the left side,
computing `net = N - M`. This handles the `(count + R1) - R2` pattern
where `R1` and `R2` are compile-time constants.

## 2026-06-11 — Convergence Proof Extended to Callable Txns

**File**: `src/proof_engine.rs:1570`  
**Date**: 2026-06-11  
**Root cause**: The structural convergence proof (`check_convergence`) was
gated behind `txn.is_reactive`, limiting it to `node` only. Callable
`txn` fell through to symbolic execution, which cannot verify
projection-based bounds like `i == items:>Size`.

**Fix**: Removed the `txn.is_reactive` guard. `check_convergence` now runs
for all `txn` types (reactive and callable). The proof (post → ¬pre, step
detection, bound invariance, overshoot) applies identically to both.

**Impact**: The documented iteration pattern
`txn f(items, acc, i) [i < items:>Size][i == items:>Size]` can now be
statically proven for callable txns.

## Path Exploration Architecture

```
verify_contract_implication
  └── enumerate_paths(body, state)
        └── enumerate_paths_recursive(body, state, paths)
              For each statement in body:
                Assignment → update symbolic state
                Guarded → fork into true/false branches:
                  [true]  recurse into guard body + remaining body
                  [false] recurse into remaining body only
                Term → push path state
                TermBang → push path state
                Escape → push escape path (vacuously satisfied)
                Expression/Unification/etc → no-op
```

The guard handling was fixed to continue the true branch through to the
remaining body (instead of dropping it), and to use `body[i+1..]` instead
of `body[1..]` for correct indexing.
