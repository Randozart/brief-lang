# Accumulation Chaining: Recovering LLVM's Horizontal Reduction

**Date:** 2026-07-28
**Status:** Plan → Implementation

## The Problem

nbody_newton's velocity accumulation emits 3 independent `fsub` instructions:

```
%t1 = fsub %nvx0, %a     ; vx0 -= dx01*mag01  (independent)
%t2 = fsub %nvx0, %b     ; vx0 -= dx02*mag02  (independent — uses same %nvx0!)
%t3 = fsub %nvx0, %c     ; vx0 -= dx03*mag03  (independent — uses same %nvx0!)
```

These are INDEPENDENT — each subtraction uses the ORIGINAL `nvx0` phi value, not the
previous subtraction's result. `%t1` and `%t2` are dead (only `%t3` survives to the
phi backedge). LLVM's DCE eliminates them, but LLVM's SLP vectorizer runs BEFORE DCE
and sees only one live `fsub` per field — not enough for vectorization.

Era 5 emitted these as a CHAIN:

```
%t1 = fsub %nvx0, %a     ; vx0 -= dx01*mag01
%t2 = fsub %t1, %b        ; vx0 = (vx0 - a) - b  — chained to %t1!
%t3 = fsub %t2, %c        ; vx0 = ((vx0 - a) - b) - c  — chained to %t2!
```

All three are LIVE (each feeds the next). LLVM's SLP vectorizer finds a horizontal
reduction with cost -295 and tree size 37 — covering all three subtractions as a
single vector operation.

## Mathematical Justification

Chaining is always correct for linear arithmetic:

```
vx0 = vx0 - a - b - c
    = vx0 - (a + b + c)
    = ((vx0 - a) - b) - c
```

The chain is mathematically equivalent to independent subtractions for ALL
operations that are associative and have a neutral element:
- Addition: `((vx0 + a) + b) + c = vx0 + a + b + c` ✓
- Subtraction: `((vx0 - a) - b) - c = vx0 - a - b - c` ✓
- Multiplication: `((vx0 * a) * b) * c = vx0 * a * b * c` ✓
- Bitwise AND/OR/XOR: transitive ✓
- MIN/MAX: transitive ✓

For NON-associative operations, chaining is NOT equivalent:
- Division: `((vx0 / a) / b) / c ≠ vx0 / a / b / c` ✗
  (But `vx0 / a / b / c = vx0 / (a * b * c)`, so `((vx0 / a) / b) / c = vx0 / a / b / c` IS correct!)
- Matrix multiplication: NOT commutative ✗

Our benchmarks only use addition, subtraction, and multiplication for field
accumulations. All three are safe to chain.

## Implementation

**Location:** `src/backend/llvm/loop_engine/counter.rs`
**Mechanism:** Use the existing `last_val_temps` HashMap to track the most recent
result for each state field within a single iteration.

### Current Code (counter.rs, around line 700-730)

For each `Statement::Assign(lhs, rhs)` in the body:
1. `let lhs_name = assign_target_name(lhs);`
2. `let phi_reg = self.fun.phi_field_regs.get(&lhs_name).unwrap();`
3. Emit computation using `phi_reg` as base
4. `self.fun.last_val_temps.insert(lhs_name, result);`

The issue: step 2 always uses `phi_field_regs`, even when `last_val_temps` already
has a chained value from a previous assignment to the same field.

### Fix

Replace step 2 with a lookup chain:

```rust
// 2026-07-28: Chain accumulations through intermediate results.
// If this field was already assigned earlier in the SAME iteration,
// use the PREVIOUS result as the base for the next accumulation.
// This creates a LIVE chain of instructions that LLVM's SLP vectorizer
// can convert into a horizontal reduction (cost -295, tree size 37).
// Without chaining, each assignment uses the phi value independently,
// producing DEAD intermediate results that DCE eliminates before SLP.
let base_reg = self.fun.last_val_temps.get(&lhs_name)
    .unwrap_or_else(|| self.fun.phi_field_regs.get(&lhs_name).unwrap());
```

## Verification

```bash
# 1. Emit IR for nbody_newton
BOUND=5000000 briefc build benchmarks/nbody_newton.bv --llvm --out /tmp

# 2. Check SLP remarks — should show -295 horizontal reduction
opt -O3 -pass-remarks=slp-vectorizer /tmp/nbody_newton.ll -o /dev/null 2>&1 | grep "horizontal reduction"

# 3. Benchmark
BOUND=5000000 /usr/bin/time -f "%e" /tmp/nbody_newton

# 4. Check instructions are chained (not independent) in unoptimized IR
grep "fsub.*nvx0\|fsub.*%t[0-9]*.*%t[0-9]*" /tmp/nbody_newton.ll | head -10
```

## Expected Results

| Benchmark | Before | After | Mechanism |
|-----------|--------|-------|-----------|
| nbody_newton | 1.24s (B=5E6) | **0.80s** (B=5E6) | -295 reduction from chained fsub |
| nbody_sqrt | 2.39s (B=5E7) | **~2.4s** (stable) | Same reduction pattern applies |
| nbody_sqrt_idio | 2.45s (B=5E7) | **~2.4s** (stable) | Already 0.67x, minor improvement |
| All others | stable | stable | No multiple-assignment pattern |

## Revert Criteria

If any benchmark regresses:
1. The `last_val_temps` lookup must be scoped to only chain when:
   - The same field was assigned previously in the same iteration
   - The operation is associative (add, sub, mul, bitwise)
   - The compiler is NOT in a `[guard]` (guard bodies shouldn't chain across iterations)
2. Add a `chainable_ops: Set<BinaryOpKind>` gate.
