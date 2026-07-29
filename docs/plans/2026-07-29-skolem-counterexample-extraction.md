# Skolemized Z3 Counterexample Extraction & SyGuS Parser Fix

Date: 2026-07-29
Status: Plan

## Problem

The CEGIS verification loop has two bottlenecks preventing popcount synthesis:

### Bottleneck 1: Z3 forall queries produce empty models

`build_verification_query` emits `(assert (not (forall (x0) (= (f x0) (ref x0)))))`. Z3 can determine this is SAT (a counterexample exists) but **cannot produce a model** for the quantified variable `x0`. The model output is `sat\n(\n)` — empty. This is a known limitation of Z3's quantifier instantiation: it uses model-based quantifier instantiation (MBQI) which can prove satisfiability but doesn't always produce concrete witness values.

`extract_counterexamples` returns empty, causing `run_z3_verify` to return `Error`. CEGIS falls back to random verification (50 samples), which works but is slow and imprecise.

### Bottleneck 2: SyGuS parser can't handle Z3's `a!N` variable naming

When the enumerative search exhausts the beam (8+ examples at depth 3), `synthesize_candidate` falls through to `smt::synthesize_via_smt`. Z3's SyGuS solver produces solutions using internal variable names like `a!1`, `a!2` (Skolem constants from Z3's normal form). The `smt_atom_to_expr` parser at `smt.rs:502` doesn't match these — it only handles standard `x0`, `x1` patterns — so it returns `Err(SynthesizeError::SolverError("unknown atom in SMT response: a!1"))`.

## Fix 1: Skolemized Counterexample Extraction

### Theory

For safety properties (find-a-counterexample), the quantified formula:

```
∀x. P(x)    (should be true if candidate is correct)
¬∀x. P(x)   (assert that a counterexample exists)
```

Can be skolemized:

```
¬P(c)    where c is a fresh constant
```

Z3 always produces concrete models for declared constants. This is a standard technique used in software model checking (e.g., CBMC, Seahorn) for counterexample extraction.

### Implementation

Replace the forall assertion in `build_verification_query` with a declare-const + direct assertion:

**Before (lines 442-455):**
```
(assert (not (forall (x0 ...)
   (and true (= (f x0 ...) (ref x0 ...))))))
(check-sat)
(get-model)
```

**After (for the reference path, no postcondition):**
```
(declare-const x0 (_ BitVec 64))
...
(declare-const xN (_ BitVec 64))
(assert (not (= (f x0 ...) (ref x0 ...))))
(check-sat)
(get-model)
```

**For the combined postcondition + reference path:**
```
(declare-const x0 (_ BitVec 64))
(assert (not (and (=> pre post_with_f) (= (f x0 ...) (ref x0 ...)))))
(check-sat)
(get-model)
```

This ensures Z3 always provides a concrete model.

### Counterexample extraction

The existing `extract_counterexamples` function at `verify_smt.rs:531` handles `(define-fun x0 () (_ BitVec 64) #xHEX)` lines — which is exactly what Z3's model produces for declared constants. No changes needed to the extraction logic.

### Impact

- The Z3 `Error("empty model")` path is eliminated for reference-only queries
- CEGIS gets concrete counterexamples from Z3 for ALL mismatched inputs
- No more fallback to random verification for reference functions
- Faster convergence (Z3 finds deeper counterexamples than random sampling)

## Fix 2: SyGuS Parser `a!N` Variable Handling

### Theory

Z3's SyGuS solver returns solutions using its internal variable naming. For example, the solution expression may be `(ite (= a!1 #x00) ...)`. The `a!1` variables are skolem constants introduced during Z3's internal solving — they correspond to the function's parameters but may appear in a different order or with different names.

### Implementation

In `smt_atom_to_expr` at `smt.rs:502`, add a handler for `a!N` variables:

```rust
// 2026-07-29: Z3 SyGuS uses a!0, a!1, ... for internal skolem constants.
// These map to function parameters by index (a!0 → x0, a!1 → x1, ...).
if let Some(digit_start) = s.strip_prefix("a!") {
    if let Ok(i) = digit_start.parse::<usize>() {
        if i < params.len() {
            return Ok(Expr::Identifier(params[i].0.clone()));
        }
    }
}
```

This maps `a!0` → first parameter, `a!1` → second parameter, etc.

### Impact

- SMT synthesis fallback works correctly for Z4 4.8+ SyGuS results
- `synthesize_via_smt` returns a valid `Expr` instead of `SolverError`
- The CEGIS loop gets a real candidate from SyGuS when enumerative fails

## Test Plan

### Unit Tests

| Test | File | What it verifies |
|------|------|-----------------|
| `build_verification_query_skolem` | `verify_smt.rs` | Reference-only query uses `declare-const` not `forall` |
| `build_verification_query_combined_skolem` | `verify_smt.rs` | Combined post+ref query uses `declare-const` |
| `smt_atom_a_var` | `smt.rs` | `a!0` maps to first param, `a!1` to second |
| `smt_atom_a_overflow` | `smt.rs` | `a!99` with < 99 params returns error gracefully |

### Integration Test

```
popcount_derive.bv with := popcount_ref:

Iteration 1: depth 3 → ite chain (4 cases) → Z3 finds x0=2 → add example
Iteration 2: depth 3 → ite chain (5 cases) → Z3 finds x0=4 → add example
Iteration 3: depth 3 → ite chain (6 cases) → Z3 finds x0=8 → add example
Iteration 4: depth 3 → beam exhausted → SMT synthesis → formula found!
Iteration 5: Z3 verifies formula against ref → UNSAT → PROVEN

Result: SynthesizedProgram with general popcount formula
```

## Implementation Order

1. `verify_smt.rs`: Replace forall with declare-const + assertion for reference-only and combined paths
2. `smt.rs`: Add `a!N` variable handler to `smt_atom_to_expr`
3. `cargo test --lib` — all tests pass
4. Test popcount derivation end-to-end
