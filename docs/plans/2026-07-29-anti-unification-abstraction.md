# Anti-Unification Abstraction for Depth-Bounded Enumerative Synthesis

Date: 2026-07-29
Status: Plan — replaces `2026-07-29-derivation-abstraction-discovery.md`

## Executive Summary

The enumerative synthesis engine (`src/derive/engine.rs`) hits a combinatorial
wall at depth 4 (beam: ~16000 candidates, raw space: ~4 million). The previous
attempt at abstraction discovery (`library.rs` as committed at `4ab76f7b`) used
frequency-based extraction with additive registration — it discovered sub-expres-
sions appearing in ≥5% of candidates and registered them as standalone helper
calls alongside raw expressions. This failed at depth 4 because helpers competed
with raw expressions at equal cost and were outcompeted by simpler ite-chain
solutions.

The literature converges on a different mechanism: **anti-unification with
replacement** — extract common sub-structure between pairs of expressions,
promote it to a helper, and REPLACE the originals with calls to the helper.
This *shrinks* the search space rather than growing it. Proven in λ² (Feser et
al., PLDI 2015) for recursive program synthesis on algebraic data types, and
at the architectural level in PROSE/FlashMeta (Polozov & Gulwani, POPL 2015)
where version-space Join operations naturally replace disjoint representations
with shared ones.

## What Went Wrong

The prior implementation (`library.rs` as of `4ab76f7b`) had two flaws:

1. **Additive registration** — helpers were added to `helper_names`/`helper_info`
   but raw expressions stayed in the LevelCache. At depth 3+, the search could
   choose between a helper call (cost 3+N) or the equivalent raw expression
   (cost typically 3+N as well). They were cost-equal competitors. The ite-chain
   (cost 40 for 4 examples) was found before any helper was referenced.

2. **Frequency ≠ importance** — a sub-expression like `x + 1` may appear in
   many depth-2 candidates but never be useful for the target function.
   Frequency correlates with simplicity, not relevance.

The outcome: helpers were discovered at depth 2-3, registered, then immediately
garbage-collected (use_count=0) because no solution referenced them. Depth 4 saw
zero helpers.

## Anti-Unification: The Proven Technique

### Definition

Anti-unification (Plotkin 1970, Reynolds 1970) finds the *least general
generalization* of two expressions. Given `expr1` and `expr2`, it produces:

- A **common pattern** `P` (the anti-unifier) with placeholder variables
- Two substitutions `σ1, σ2` that map the placeholders to the differences

Example:
```
expr1: x + y + 1
expr2: x + y - 3
anti-unifier: x + y + t    (placeholder t)
σ1: t → 1
σ2: t → -3
```

### How λ² Uses It (Feser et al., PLDI 2015, §4)

In λ², abstraction is triggered when the synthesizer needs to produce a
conditional expression where both branches share computation. The system:

1. Generates a conditional `if cond then e1 else e2`
2. Anti-unifies `e1` and `e2` to find the shared pattern `P`
3. Extracts `P` as a lambda `λ(t). P(t)` where `t` is the differing part
4. Replaces the conditional body with the abstraction applied to each branch:
   `let f = λ(t). P(t); if cond then f(sub1) else f(sub2)`

This is not optional — the abstraction is STRUCTURALLY NECESSARY because allowing
`e1` and `e2` to exist independently would miss the generalization. λ²'s
correctness proof shows that abstraction via anti-unification always reduces the
program size (measured in AST nodes) and never increases it.

### How PROSE Uses Replacement (Polozov & Gulwani, POPL 2015, §3.3)

PROSE's version-space algebra has three operations: Join (union), Compose
(function application), and Transform (mapping). When two version-spaces are
Joined, the resulting VSA is a compact representation of all programs consistent
with both sets of examples — it REPLACES the two input version-spaces, it does
not coexist with them.

The key insight for our architecture: **replacement is the mechanism that keeps
the search polynomial.** Without replacement, version-spaces grow exponentially
with depth. With replacement (Join absorbs both inputs), they grow linearly.

## Proposed Design

### Algorithm

After pruning at each depth ≥ 2, for each pair of expressions in the LevelCache
with the same return type:

1. **Anti-unify** — find the maximal common sub-expression (the "shared pattern")
2. **Score** — `savings = (size(e1) + size(e2)) - (size(helper) + size(diff1) + size(diff2))`
3. **Extract** — if savings > 0, promote the shared pattern to a helper function
4. **Replace** — remove `e1` and `e2` from the LevelCache, add the helper call
   to the LevelCache

### Concrete Example

LevelCache at depth 2 (before anti-unification):
```
int_exprs: [x+1, x+2, x-1, x+y, y+1, 2*x, 2*y, ...]
```

Anti-unify pairs:
- `x+1` and `x+2` → shared `x+t`, diffs: `t→1`, `t→2` → savings = (5+5) - (5+1+1) = 3
- `x+1` and `x-1` → shared `x+t`, diffs: `t→1`, `t→-1` → savings = (5+5) - (5+1+1) = 3
- `2*x` and `2*y` → shared `t*z`, diffs: `t→2,z→x`, `t→2,z→y` → savings = (5+5) - (5+1+1) = 3

Extract helper `_h0(x, t) = x + t`.
Replace `x+1`, `x+2`, `x-1` with `_h0(x, 1)`, `_h0(x, 2)`, `_h0(x, -1)`.

LevelCache at depth 2 (after anti-unification):
```
int_exprs: [_h0(x,1), _h0(x,2), _h0(x,-1), x+y, _h0(y,1), 2*x, 2*y, ...]
```

Size change: 8 raw expressions → 7 helper calls + 2 remaining raw = 9 expressions
BUT each helper call has size 1 (single node) vs raw size 3+.
Depth-3 cross product: 9² = 81 vs original 8² = 64. Slightly larger, but each
candidate is cheaper.

### When It Works

Anti-unification is most effective when the LevelCache contains MANY structurally
similar expressions — exactly the situation that leads to depth-4 combinatorial
explosion. The classic pattern: at depth 2, we have `x+0, x+1, x+2, ..., x+N`.
Without abstraction, at depth 3 these combine to give N² candidates. With
abstraction, they share the `x + t` pattern via `_h0`, and depth 3 sees only
the helper call + parameter values.

### Implementation Sketch

```rust
pub(crate) fn anti_unify(a: &Expr, b: &Expr) -> Option<(Expr, Vec<(String, Expr)>, Vec<(String, Expr)>)> {
    // Returns:
    // - The anti-unifier (common pattern with placeholder variables)
    // - Substitution σ1: placeholder → expression for a
    // - Substitution σ2: placeholder → expression for b
    // Returns None if no common structure exists.
}
```

The anti-unification algorithm for our AST types:

```rust
fn anti_unify_expr(a: &Expr, b: &Expr, vars: &mut HashMap<(usize, usize), String>, counter: &mut usize) -> Option<Expr> {
    match (a, b) {
        // Same operator: anti-unify children pairwise
        (Expr::BinaryOp(k1, l1, r1), Expr::BinaryOp(k2, l2, r2)) if k1 == k2 => {
            let lhs = anti_unify_expr(l1, l2, vars, counter)?;
            let rhs = anti_unify_expr(r1, r2, vars, counter)?;
            Some(Expr::BinaryOp(*k1, Box::new(lhs), Box::new(rhs)))
        }
        (Expr::UnaryOp(k1, i1), Expr::UnaryOp(k2, i2)) if k1 == k2 => {
            let inner = anti_unify_expr(i1, i2, vars, counter)?;
            Some(Expr::UnaryOp(*k1, Box::new(inner)))
        }
        // Different operators or leaves: create placeholder variable
        _ => {
            let key = (a as *const _ as usize, b as *const _ as usize);
            let var = vars.entry(key).or_insert_with(|| {
                let name = format!("_t{}", *counter);
                *counter += 1;
                name
            });
            Some(Expr::Identifier(var.clone()))
        }
    }
}
```

The scoring function:

```rust
fn anti_unify_savings(a: &Expr, b: &Expr, common: &Expr, sigma1: &[(String, Expr)], sigma2: &[(String, Expr)]) -> i64 {
    let size_a = expr_size(a);
    let size_b = expr_size(b);
    let size_common = expr_size(common);
    let size_diff1: u64 = sigma1.iter().map(|(_, e)| expr_size(e)).sum();
    let size_diff2: u64 = sigma2.iter().map(|(_, e)| expr_size(e)).sum();
    // savings = old_total - new_total
    (size_a + size_b) as i64 - (size_common + size_diff1 + size_diff2) as i64
}
```

### Registration with Replacement

When a helper is extracted:

1. Compute the helper call: `Expr::Call("_hN", params, None)` where `params` are
   the placeholder variables' actual argument expressions
2. Add the helper to `helper_names`/`helper_info`
3. Remove the two original expressions from the LevelCache
4. Add the two helper calls to the LevelCache
5. The net effect: size of the LevelCache shrinks or stays the same
   (2 items removed, 2 items added, but each item is smaller)

## Changes to Existing Code

### `src/derive/library.rs` — Replace frequency-based discovery with anti-unification

| Current | Replacement |
|---------|-------------|
| `discover_helpers()` — frequency + cost-savings scoring | `discover_helpers_anti_unify()` — pairwise anti-unification + savings |
| `collect_sub_trees()` — extract depth-2 sub-trees | Remove entirely |
| `expr_fingerprint()` — sort-based dedup for commutative ops | Keep (used in anti-unify for commutative op normalization) |
| `register_helpers()` — add to helper_names/helper_info only | Add REPLACEMENT step: remove originals from LevelCache, add helper calls |
| `gc_helpers()` — clean unused helpers | Keep (still needed for anti-unification at higher depths) |

### `src/derive/engine.rs` — Keep integration points

The integration in `synthesize_enumerative()` stays the same:
- After `prune_level()` at depth ≥ 2, call `discover_helpers_anti_unify()`
- Register with replacement
- GC unused after each depth

The helper generation in `generate_next_level()` stays the same:
- Emit helper calls as standalone expressions via `helper_names` loop

### `src/derive/mod.rs` — Keep synthesize_candidate change

- Keep direct call to `synthesize_enumerative()` (preserves helpers in output)

## Test Strategy

### Unit Tests (anti-unification)

| Test | What it verifies |
|------|------------------|
| `anti_unify_same_op_same_args` | `x + 1` and `x + 1` → returns `x + 1` (identical) |
| `anti_unify_same_op_diff_args` | `x + 1` and `x + 2` → returns `x + t` with `t→1/t→2` |
| `anti_unify_diff_op` | `x + 1` and `x * 2` → returns `f(x, t)` with full placeholder |
| `anti_unify_nested` | `(x + 1) * 2` and `(x + 3) * 4` → returns `f(x, t1, t2)` |
| `anti_unify_commutative` | `x + y` and `y + x` → returns `x + y` (normalized) |
| `anti_unify_no_match` | `x + 1` and `42` → returns `f()` (completely different) |

### Unit Tests (savings)

| Test | What it verifies |
|------|------------------|
| `savings_positive` | `x+1` and `x+2` → savings > 0 |
| `savings_negative` | `x+1` and `y+2` → savings < 0 (different first arg) |
| `savings_zero` | `x+1` and `x+1` → savings = 0 (identical, no abstraction needed) |

### Integration Tests

| Test | What it verifies |
|------|------------------|
| `levelcache_shrinks` | After anti-unification, LevelCache size is ≤ original |
| `helper_survives` | Helper with use_count > 0 survives GC |
| `repeated_computation` | `f(x,y,z) = (x+y)*(x+z) + (x+y) - (x+z)` found at depth 3 with helpers |

## Literature References

[PL'70] Plotkin, G. "A note on inductive generalization." *Machine Intelligence* 5, 1970.

[Ry'70] Reynolds, J. C. "Transformational systems and the algebraic structure of
atomic formulas." *Machine Intelligence* 5, 1970.

[FCD'15] Feser, J. K., Chaudhuri, S. & Dillig, I. "Synthesizing Data Structure
Transformations from Input-Output Examples." PLDI 2015, §4: "Abstraction."
doi: 10.1145/2737924.2737977

[PG'15] Polozov, O. & Gulwani, S. "FlashMeta: A Framework for Inductive Program
Synthesis." POPL 2015, §3: "Version Space Algebra."
doi: 10.1145/2676726.2676981

[GP'92] Koza, J. R. *Genetic Programming.* MIT Press, 1992. Chapter 6:
"Automatically Defined Functions."

[SL'09] Schmidt, M. D. & Lipson, H. "Distilling Free-Form Natural Laws from
Experimental Data." *Science* 324(5923), 2009.

## Plan Directives Compliance

| Directive | How this plan meets it |
|-----------|----------------------|
| **FLAT CONTROL FLOW** | `anti_unify` is a recursive function but each recursion is a single match arm. `discover_helpers_anti_unify` is two loops (outer: pairs, inner: anti-unify + score) — max 2 levels. |
| **COMMENT THE CODE** | Every new code site gets `// 2026-07-29:` with literature citations. The anti-unification function has a module-level doc explaining the algorithm. |
| **BEHAVIORAL TESTS, NOT LITERAL** | Tests assert savings calculations, LevelCache size changes, helper survival — not specific IR snapshots. |

## Migration Path

1. Add `anti_unify()` function to `library.rs` alongside existing code
2. Add `discover_helpers_anti_unify()` function
3. Update `register_helpers()` to support replacement
4. Update `synthesize_enumerative()` to call new discovery
5. Keep old frequency-based code as `#[cfg(test)]` or remove once anti-unify is tested
6. Test: `cargo test --lib` — all 1190+ tests pass
7. Test: `popcount_derive.bv` at depth 3 — should still find ite chain
8. Test: `repeated_computation.bv` (new benchmark) — should find helper-based solution at depth 3
