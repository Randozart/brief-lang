# Search Optimization: Incremental Eval Caching + Symmetry Breaking

Date: 2026-07-28
Status: Plan

## Problem

Depth 4+ candidate generation is slow because every candidate evaluation
recursively re-evaluates all sub-expressions. At depth D, a binary op
candidate `Add(e1, e2)` must evaluate `e1` and `e2` — both depth-(D-1)
expressions that were already evaluated during the previous pruning step.
The results were discarded after pruning. Each re-evaluation costs O(2^D)
in the worst case, making depth 5's ~10^6 evaluations infeasible.

## Optimization 5: Incremental Eval Caching (High Impact)

### Architecture

Add `EvalCache` struct that stores evaluation results per expression:

```rust
struct EvalCache {
    level: LevelCache,   // existing per-type pruned expressions
    results: HashMap<Expr, Vec<Value>>,  // expr → [result_for_example_0, result_for_example_1, ...]
}
```

### Changes (3 functions, engine.rs)

#### 1. `EvalCache` struct (new, ~30 lines)

After `LevelCache` definition:

```rust
struct EvalCache {
    level: LevelCache,
    results: std::collections::HashMap<Expr, Vec<Value>>,
}
```

Methods:
- `empty() -> Self` — creates empty cache
- `set(e, ty, results)` — stores expr in LevelCache by type, results in HashMap
- `get_result(expr, example_idx) -> Option<&Value>` — cached evaluation result

#### 2. `prune_level` → returns `EvalCache` (~10 lines changed)

Current signature:
```rust
fn prune_level(
    candidates: Vec<Expr>,
    param_names: &[String],
    param_types: &[String],
    examples: &[DerivationExample],
) -> LevelCache
```

New signature:
```rust
fn prune_level(
    candidates: Vec<Expr>,
    param_names: &[String],
    param_types: &[String],
    examples: &[DerivationExample],
) -> EvalCache
```

Inside the loop, after computing `outputs: Vec<(i64, i64)>` (the fingerprint),
also store the full `Vec<Value>` results in the EvalCache before returning.

#### 3. `generate_next_level` → takes `&EvalCache` (~5 lines changed)

Current signature:
```rust
fn generate_next_level(
    param_names: &[String],
    param_types: &[String],
    ret_type: &str,
    prev: &LevelCache,
) -> Vec<Expr>
```

New signature:
```rust
fn generate_next_level(
    param_names: &[String],
    param_types: &[String],
    ret_type: &str,
    prev: &EvalCache,
) -> Vec<Expr>
```

The function only uses `prev.level` internally (the LevelCache). All `prev.int_exprs`,
`prev.float_exprs`, `prev.bool_exprs`, `prev.compound_exprs` references become
`prev.level.int_exprs`, etc.

#### 4. `synthesize_enumerative` threads EvalCache (~10 lines)

Current code:
```rust
let mut prev_cache = LevelCache::empty();
...
prev_cache = prune_level(next_level, param_names, param_types, examples);
```

New code:
```rust
let mut prev_cache = EvalCache::empty();
...
prev_cache = prune_level(next_level, param_names, param_types, examples);
```

#### 5. `candidate_matches_all_examples` uses eval cache (~20 lines)

Current: evaluates each candidate from scratch by calling `evaluate_synthesized`
recursively for each example.

New: checks the eval cache first. If the candidate is in the cache, uses the
cached results. This eliminates recursive re-evaluation for every parent expression.

Implementation in `candidate_matches_all_examples`:

```rust
fn candidate_matches_all_examples(
    candidate: &Expr,
    param_names: &[String],
    examples: &[DerivationExample],
    eval_cache: &EvalCache,
) -> bool {
    // Check cache first
    if let Some(cached) = eval_cache.results.get(candidate) {
        return cached.iter().enumerate().all(|(i, val)| {
            let expected = evaluate_expected(&examples[i]);
            let tol = examples[i].tolerance.unwrap_or(0.0);
            values_within_tolerance(val, &expected, tol)
        });
    }
    // Full evaluation (cache miss — should not happen for pruned candidates)
    examples.iter().all(|ex| { ... existing logic ... })
}
```

Similarly, the `evaluate_synthesized` calls for sub-expressions should check
the cache. This is the key: when we evaluate `Add(e1, e2)` at depth D, we
look up `e1` and `e2` in the cache instead of recursively evaluating them.

### Performance Impact

At depth D with E candidates and S sub-expressions:
- Without cache: O(E * 2^D) evaluations
- With cache: O(E + S) evaluations (each sub-expression evaluated once, cached)

For depth 4 with ~10^5 candidates and ~10^3 sub-expressions:
- Without cache: ~10^5 * 16 = ~1.6M evaluations
- With cache: ~10^5 + 10^3 = ~101K evaluations

For depth 5: ~5× improvement (from ~10^7 to ~2×10^6)

## Optimization 4: Symmetry Breaking (Low Effort)

### Idea

For commutative operators (Add, Mul, BitAnd, BitOr, BitXor, Eq), the
expression `Add(A, B)` and `Add(B, A)` are semantically identical. Half
of the generated candidates are redundant. We can prune commutative
expressions where `lhs > rhs` by some canonical ordering.

### Changes (~15 lines in generate_next_level)

```rust
fn is_commutative(op: BinaryOpKind) -> bool {
    matches!(op, BinaryOpKind::Add | BinaryOpKind::Mul
        | BinaryOpKind::BitAnd | BinaryOpKind::BitOr
        | BinaryOpKind::BitXor | BinaryOpKind::Eq)
}

fn expr_less_than(a: &Expr, b: &Expr) -> bool {
    // Canonical ordering: compare debug representations
    format!("{:?}", a) < format!("{:?}", b)
}
```

In `generate_next_level`, inside the binary ops loop:

```rust
for lhs in *exprs {
    for rhs in *exprs {
        // ... existing checks ...
        // Symmetry breaking: skip commutative duplicates
        if is_commutative(*op) && expr_less_than(rhs, lhs) {
            continue;
        }
        result.push(Expr::BinaryOp(...));
    }
}
```

### Performance Impact

~2× fewer candidates for commutative ops (about 6 of 19 operators).
Overall speedup: ~1.5× at any depth.

## Implementation Order

1. Add `EvalCache` struct and update `prune_level` return type
2. Update `generate_next_level` to use `prev.level` instead of `prev` directly
3. Update `synthesize_enumerative` to thread `EvalCache`
4. Update `candidate_matches_all_examples` to use cache
5. Add symmetry breaking
6. Test all benchmarks at depth 5

## Files

| File | Action |
|------|--------|
| `src/derive/engine.rs` | All changes: EvalCache, prune_level, generate_next_level, synthesize_enumerative, candidate_matches_all_examples, symmetry breaking |

## Verification

- `cargo test --lib` — 1167+ existing tests pass
- Manual: `briv derive --enumerative-depth 4 benchmarks/popcount_derive.bv` completes in <30s
- Manual: `briv derive --enumerative-depth 5 benchmarks/popcount_derive.bv` completes in <120s
