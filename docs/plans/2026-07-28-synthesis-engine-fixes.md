# Synthesis Engine Fixes — Param Names, Lazy Generation, Div-By-Zero Pruning

Date: 2026-07-28
Status: Plan

## Problem Summary

The derivation pipeline (Phases A–I) is committed and tested, but derivation
benchmarks (popcount, minmax, abs) fail to produce correct binaries due to
four interconnected issues in the synthesis engine:

1. **Param name mismatch**: `enumerative_search` hardcodes `x0`/`x1` as param
   names, but the actual function uses the user's names. Synthesized bodies
   reference `x0` but the function declares `a` → build fails with
   "undefined variable 'x0'".

2. **Candidate explosion at depth 4+**: 20 operators × nested recursion
   generates ~10^6 candidates at depth 4, all stored in memory before
   evaluation. At depth 5 it's ~10^8. The popcount examples need depth 4+.

3. **Division-by-zero noise**: ~40% of `Div`/`Mod` candidates have RHS = 0
   and fail immediately, wasting evaluation time.

4. **SMT `ite` chains**: `declare-fun` + `get-model` returns table-lookup
   `ite` chains that don't generalize — acceptable as MCMC starting point.

## Fixes

All fixes are in `src/derive/` — no backend changes needed.

### Fix 1: Param Name Remapping

**Files**: `mod.rs`, `engine.rs`, `smt.rs`, `cli.rs`

**Problem**: `enumerative_search` (engine.rs:528) creates params as
`(format!("x{}", i), Type::int())` — no access to actual function params.
`generate_typed_expressions` already uses the given names correctly.

**Fix** (5 edits, no new files):

1. `mod.rs:35` — `synthesize(name, block, max_depth)` → `synthesize(name, block, params, max_depth)`
2. `engine.rs:520` — `enumerative_search(name, examples, max_depth)` → `enumerative_search(name, params, examples, max_depth)`
3. `engine.rs:535` — pass params to `synthesize_enumerative` instead of hardcoded names
4. `smt.rs:545` — `synthesize_via_smt(name, examples)` → `synthesize_via_smt(name, params, examples)`
5. `cli.rs:62` — extract `&[(String, Type)]` from parsed `Definition.parameters` and pass to `synthesize`

### Fix 2: Lazy Candidate Generation

**File**: `engine.rs`

**Problem**: `generate_typed_expressions` builds ALL candidates at a given
depth into a `Vec<Expr>` before returning. The caller then iterates over them.
At depth 4 this is ~1e6 candidates in memory.

**Fix**: Add `generate_typed_expressions_lazy()` that accepts a callback
`&mut dyn FnMut(&Expr) -> bool` (returns true to stop early). The recursion
generates and yields one candidate at a time. When depth 4 finds a match,
it returns immediately without generating the remaining candidates.

No allocation for the full candidate set — only the current chain on the stack.

### Fix 3: Division-by-Zero Pruning

**File**: `engine.rs`

**Problem**: `Div` and `Mod` with constant-zero RHS always fail evaluation.
`generate_typed_expressions` still generates them.

**Fix**: When `op` is `Div` or `Mod`, skip candidates where the RHS sub-
expression is `Expr::Decimal(0)` or `Expr::UnaryOp(Neg, Decimal(0))`.

## Implementation Order

1. Fix 1a–1e: Param name remapping (5 edits) → unblocks ALL benchmarks
2. Fix 2: Lazy candidate generation → makes depth 4 feasible
3. Fix 3: Division-by-zero pruning → cheap additional speedup
4. Test popcount end-to-end with `briv derive --enumerative-depth 4`
5. Run `bash build_and_bench.sh --derive --correctness`

## Verification

- `cargo test --lib` — all 1148+ existing tests pass
- `cargo test --test derive_pipeline_test` — all 4 integration tests pass
- Manual test: `brivc derive --enumerative-depth 4 popcount_derive.bv`
  produces a valid `.derive.bv` with a correct body
- `brivc build popcount_derive.derive.bv` compiles without "undefined variable" errors
