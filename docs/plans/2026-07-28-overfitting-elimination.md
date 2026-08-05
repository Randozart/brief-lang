# Overfitting Elimination: Cost-Ordered Evaluation + Comparison Priority

Date: 2026-07-28
Status: Plan

## Problem

The synthesis engine finds table-lookup ite chains instead of general
formulas. Root cause: candidates are evaluated in generation order (not
cost order). X == 5 is generated before X < 0, so the ite chain (cost 56)
is found before the general abs formula If(X < 0, -X, X) (cost ~14).

## Options Implemented

### Option 1: Cost-Ordered Evaluation (engine.rs)

After generate_next_level returns candidates, sort by cost ascending.
First evaluated = cheapest = most likely to be general.

### Option 6: Comparison Priority in Generation (engine.rs)

Reorder operator list in generate_next_level to generate comparison
operators (<, >, <=, >=) BEFORE equality operators (==, !=). General
formulas use <, >; ite chains use ==, !=.

### Option 2: Equality Penalty (engine.rs)

CostModel: increase Eq/Neq from binary_op(3) to 5. Pushes ite chains
to higher effective cost. General formulas using <, > keep lower cost.

### Option 5: Postconditions on Benchmarks (.bv files)

Add [[post]] to each derivation benchmark:
- popcount_derive: [[ @result >= 0 && @result < 64 ]]
- minmax_derive: [[ @result >= -2^62 ]]
- abs_derive: [[ @result >= 0 ]]

## Files

| File | Change |
|------|--------|
| `src/derive/engine.rs` | Cost-ordered sort, comparison priority, Eq penalty |
| `benchmarks/*_derive.bv` | Add [[postcondition]] |

## Verification

- `cargo test --lib` — 1167+ tests pass
- `briv derive --enumerative-depth 3 benchmarks/abs_derive.bv` finds
  general formula `when X < 0 { term -X; }; term X;` not ite chain
