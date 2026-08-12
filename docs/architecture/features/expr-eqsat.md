# Expression Equality Saturation (Simplify Pass)

**Date:** 2026-06-13
**Phase:** Post-bare-label fix

## Purpose

Algebraic expression rewriting on the Briev AST before LLVM codegen.
Eliminates identity operations (`x + 0`, `x * 1`, `x && true`, `!!x`, etc.)
so LLVM gets cleaner IR. Redundant operations are handled by LLVM's own
optimizers (`-O3` catches `add i64 0, %x` → `%x`), but simplifying at
the AST level means LLVM sees fewer instructions to begin with.

## Syntax

No surface syntax — compiler internal pass. Gated by `--prod`/`--release`
CLI flag. Runs on every `Definition` and `Transaction` body.

## Algorithm: Bottom-Up Rewriting with Hash-Cons Cache

```rust
fn simplify_cached(expr, cache) -> Option<Expr> {
    let h = structural_hash(expr);
    if cache[h]: return cache[h];     // hash-cons hit

    result = match expr {
        Binary(op, l, r) =>
            sl = simplify_cached(l)   // children first
            sr = simplify_cached(r)
            try_rewrite(op, sl, sr) ?? Binary(sl, sr)

        Unary(op, inner) =>
            si = simplify_cached(inner)
            try_rewrite(op, si) ?? Unary(si)

        Variadic(children) =>
            op(children.map(simplify_cached))

        Leaf => expr.clone()
    }

    cache[h] = result
    result
}
```

- **O(n)**: each node visited exactly once. No fixpoint loop.
- **Bottom-up order**: children simplified before parent, so `(x+0)+0` works
  because `x+0` resolves before `+0` evaluates.
- **Hash-cons**: structural hash → simplified result. Same sub-expression
  anywhere in the tree is simplified once. Cache key uses FNV offset basis
  + boost `hash_combine` — pure integer arithmetic, no allocations.

## Rewrite Rules

### Identity Elimination (18 rules, preserved from original simplify_pass)

| Pattern | Result | Condition |
|---------|--------|-----------|
| `x + 0` | `x` | — |
| `0 + x` | `x` | — |
| `x - 0` | `x` | — |
| `x - x` | `0` | — |
| `(a + b) - b` | `a` | — |
| `(a + b) - a` | `b` | — |
| `(a - b) + b` | `a` | — |
| `x * 0` | `0` | — |
| `0 * x` | `0` | — |
| `x * 1` | `x` | — |
| `1 * x` | `x` | — |
| `x / 1` | `x` | — |
| `x & 0` / `0 & x` | `0` | — |
| `x | 0` / `0 | x` | `x` | — |
| `x ^ 0` / `0 ^ x` | `x` | — |
| `x << 0` / `x >> 0` | `x` | — |
| `true && x` / `x && true` | `x` | — |
| `false && x` / `x && false` | `false` | — |
| `false || x` / `x || false` | `x` | — |
| `true || x` / `x || true` | `true` | — |
| `x && x` / `x || x` | `x` | — |
| `!!x` | `x` | — |
| `--x` | `x` | — |

## Complexity Analysis

| Metric | Old (removed 2026-06-13) | New |
|--------|--------------------------|-----|
| Node visits | O(10^n) | O(n) |
| Fixpoint iterations | 5 per node | 0 |
| Hash comparisons | `format!("{:?}")` (allocation) | u64 arithmetic |
| Cache | None | HashMap<u64, Expr> |
| 26-term `\|\|` chain | ~10^26 calls | 26 calls |

## CLI Integration

- `--dev` (default): simplify disabled (`simplify_budget = 0`)
- `--prod` / `--release`: simplify enabled (`simplify_budget = u64::MAX`)
- `--simplify-budget <N>`: explicit budget cap
- `--no-simplify`: disable regardless of mode

## Files

| File | Role |
|------|------|
| `src/analysis/equality_saturation.rs` | Implementation: `simplify`, `simplify_cached`, `SimplifyCache`, `simplify_program` |
| `src/main.rs` | CLI flags, pipeline wiring (`run_llvm_compile` calls `simplify_program`) |
| `docs/architecture/features/expr-eqsat.md` | This document |
| `plans/2026-06-13-bare-label-simplify-plan.md` | Design plan |
