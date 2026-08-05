# Reference Function Verification — `verifying <fn> [tol: N]`

Date: 2026-07-29
Status: Plan → Implementation

## Problem

The synthesis engine produces ite chains (table lookups) that match the
derivation examples but fail to generalize. Postconditions (`[[post]`) can
reject obviously wrong outputs but require the user to write a formal spec
— which for complex functions like popcount is as hard as writing the
implementation itself.

## Solution: Reference Function as Oracle

A derivation block can declare an existing function as the correctness
oracle. The synthesized function must match the reference for ALL inputs
(within optional tolerance):

```briv
defn popcount_ref(x: Int) -> Int := {
    // Reference implementation — simple loop, provably correct
};

defn popcount(x: Int) -> Int := {
    // Synthesis target — must match popcount_ref for ALL inputs
} verifying popcount_ref;
```

## Syntax

```
derivation_block ::=
    ":=" "{" examples "}"
    ( "[[" post "]" | "[" pre "][" post "]" | "[" pre "]]" )?
    ( "verifying" identifier ( "[" "tol" ":" float "]" )? )?
    ";"

Examples:
  := { 0 -> 0; 1 -> 1; } verifying popcount_ref;
  := { 5 -> 5; -3 -> 3; } verifying abs_ref [tol: 1e-9];
```

Three sections after `:= { ... }` (all optional):
1. Contract `[[post]` / `[pre][post]` / `[pre]]`
2. `verifying <fn>` — reference function name
3. `[tol: N]` — tolerance for comparison (default 0.0)

## Pipeline Integration

### 1. Parse Time — Verify Reference Against Examples

When parsing `verifying popcount_ref`, the compiler:
1. Looks up `popcount_ref` in the same compilation unit (must be a `defn`)
2. Evaluates the reference on every derivation example's inputs
3. If the reference's outputs don't match the example outputs → compilation error
4. If they match → reference IS the spec for CEGIS

This prevents the reference from being a bluff — it must actually satisfy
the examples the user provided.

### 2. Synthesis Time — Enumerative Search (unchanged)

Search for cheapest expression matching all examples. Same as before.
The reference is NOT consulted during search — only during verification.

### 3. Verification Time — Z3 Forall Against Reference

The CEGIS loop uses the reference for verification instead of (or in
addition to) postconditions:

- **With `verifying ref`**: `forall x: |candidate(x) - ref(x)| < tol`
- **With `[[post]` + `verifying ref`**: both checks run
- **With neither**: random-input verification (Tier 2/3)

Z3 query structure with reference:

```smt
(define-fun f ((x (_ BitVec 64))) (_ BitVec 64)
  <candidate-body>
)

; Reference function (inlined from the Briv defn's body)
(define-fun ref ((x (_ BitVec 64))) (_ BitVec 64)
  <ref-body>
)

; Verify: forall x, |f(x) - ref(x)| < tol
(assert (not (forall ((x (_ BitVec 64)))
  (bvslt (bvsub (f x) (ref x)) #x...tol_in_bitvector...)
)))
```

### 4. Counterexample → Re-synthesize

When Z3 returns `sat`, it provides a counterexample input `x` where
`|f(x) - ref(x)| >= tol`. The pipeline:
1. Extract `x` from Z3's model
2. Evaluate `ref(x)` using the interpreter to get the correct output
3. Add `x -> ref(x)` as a new example
4. Re-synthesize with enriched examples

### 5. Reference as Performance Baseline

The reference's cost (from CostModel) serves as an upper bound for
acceptance. If the candidate's cost exceeds the reference's cost, the
synthesized function is WORSE than the reference — reject it.
The MCMC superoptimizer (Phase F) can further optimize below this bound.

## Files

| File | Change | Reason |
|------|--------|--------|
| `src/lexer.rs` | Add `#[token("verifying")]` → `Token::Verifying` | New keyword |
| `src/ast/expr.rs` | Add `ref_name`, `ref_tolerance` to `DerivationBlock` | AST storage |
| `src/parser/definitions.rs` | Parse `verifying <name> [tol:N]` | Syntax support |
| `src/derive/verify.rs` | Reference evaluation in `verify_candidate` | Runtime verification |
| `src/derive/mod.rs` | Reference lookup + counterexample output computation | CEGIS loop |
| `src/derive/verify_smt.rs` | Z3 query with `ref` function inlined | Formal verification |
| `src/derive/doppelganger.rs` | Pass through new fields | Construction sites |
| `src/derive/assert.rs` | Pass through new fields | Construction sites |
| `src/backend/llvm/helpers.rs` | Pass through new fields | Construction sites |

## Tests

- `test_verifying_reference_valid`: reference matches examples → synthesis runs
- `test_verifying_reference_invalid`: reference mismatches examples → parse error
- `test_verifying_with_tolerance`: tolerance allows small deviations
- `test_verifying_rejects_ite_chain`: reference catches popcount ite at `x=42`
- `test_verifying_smt_query`: Z3 query includes `ref` function
- `test_verifying_z3_proven`: candidate matches reference for ALL inputs
