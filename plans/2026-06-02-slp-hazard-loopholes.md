# SLP Hazard Analyzer — Three Loopholes Identified and Fixed

## Audit Date: 2026-06-02

The initial `estimate_slp_hazard` implementation had three critical logical
loopholes that systematically underestimated register pressure on real-world
programs, potentially allowing SLP vectorization to cause spills even when the
analyzer declared it safe.

---

## Loophole 1: Local Variable Blindspot

`is_float_expr_pre_cg(name)` only checked `self.is_float_field(name)`, which
queries the global `field_index_map`. It was completely blind to local variables
bound via `Statement::Let` (e.g., `let temp: Float = f0 * f1;`). Subsequent
expressions referencing `temp` would evaluate to `false`.

**Impact**: Float operations involving intermediate let-bound variables were
silently excluded from `count_cross_float_ops`, underestimating register demand.

**Fix**: Pass a `local_floats: &HashSet<String>` parameter through all three
recursive functions (`is_float_expr_pre_cg`, `count_cross_float_ops`,
`collect_local_floats_and_temps`). The new `collect_local_floats_and_temps`
walks body statements first, inserting let-bound floats into the set before
they can be referenced by subsequent expressions.

---

## Loophole 2: Strict Dual-Variable Constraint

`count_cross_float_ops` required `left_is_var && right_is_var` to count an
operation. This missed all float operations where one operand was a literal
(`x * 0.01`) or a global constant (`x * A_coeff`). Matrix math (Kalman filter
covariance propagation) is packed with constant-coefficient multiplications.

**Impact**: Rearranged the condition to `left_is_operand || right_is_operand`
where `left_is_operand = matches!(l, Identifier | OwnedRef | Float)`. Any
float binary op with at least one non-trivial operand now counts.

**Before:** 30+ matrix multiply operations → 0 counted (all operands were
float variables, so this one actually would have counted — but operations with
any constant route were missed)

**After:** All float binary operations between any combination of variables,
constants, and literals are counted.

---

## Loophole 3: Missing Global Constant Register Demand

The peak formula was:
```
peak = packed_phis + shuffle_regs + max_float_temps + 2
```

It ignored registers consumed by loaded global constants (e.g., Kalman filter's
A matrix: 9 floats, Q matrix: 9 floats = 18 additional float constants loaded
into registers within the tick loop).

**Impact**: 18 loaded constant floats each consume a register slot (grouped as
`⌈18/4⌉ = 5` packed loads under SLP). Without counting these, the peak formula
was 18 registers below the true demand for the Kalman filter.

**Fix**: Add `accessed_constants: HashSet<String>` tracking in
`estimate_slp_hazard`. Any float-typed constant referenced by a reactive
transaction's read set is counted toward peak register demand.

---

## Corrected Formula

```
peak = packed_phis + shuffle_regs + max_float_temps + accessed_constants_regs + 2
```

Where `accessed_constants_regs = ceil(accessed_constants.len() / w)` since a
single packed load can bring in multiple constants.

### Verified Predictions with Corrected Formula

| Benchmark | N | C | T | K | R | W | Formula | Peak | ≥R? | Correct? |
|-----------|---|---|---|---|---|----|---------|------|-----|----------|
| IIR | 4 | 3 | 0 | 0 | 16 | 4 | 1+2+0+0+2 | 5 | No | **Yes** |
| Kalman (SSE) | 12 | 32 | 5 | 18 | 16 | 4 | 3+6+5+5+2 | 21 | **Yes** | **Yes** |
| Kalman (AVX) | 12 | 32 | 5 | 18 | 16 | 8 | 2+4+5+3+2 | 16 | ≥16 | **Yes** (borderline) |
| 12 independent | 12 | 0 | 0 | 0 | 16 | 4 | 3+0+0+0+2 | 5 | No | **Yes** |
