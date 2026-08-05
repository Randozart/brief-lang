# Overfitting Prevention: Three-Tier Generalization Enforcement

Date: 2026-07-28
Status: Plan

## Problem: Synthesis Produces Overfitted Formulas

The enumerative synthesis engine finds expressions that match all provided
derivation examples but fail to generalize. Example: popcount at depth 4
produces `((0 + (x0 >> 1)) + (x0 >> (x0 >> 1)))` which matches the 4 examples
(`0→0, 1→1, 3→2, 7→3`) but gives `popcount(2) = 2` (wrong, should be 1).

### Root Cause

The search space at depth 4 with 20 operators contains many expressions that
happen to match the given examples by coincidence rather than by capturing the
intended function. The 4 examples only constrain ~4 of the 2^64 possible
inputs, leaving the search free to pick any shallow expression that passes them.

### How Professional Systems Handle This

The gold standard in program synthesis is **CEGIS** (Counterexample-Guided
Inductive Synthesis), used by Sketch, Rosette, and SyGuS solvers:

```
loop:
  1. Synthesize candidate from examples
  2. Verify candidate against ALL inputs (using SMT solver + quantifier)
  3. If verification succeeds → done
  4. If verification fails → extract counterexample input
  5. Add counterexample to example set → goto 1
```

The SMT quantifier check (`forall x . spec(x) == candidate(x)`) is the
authoritative generalization test. If it passes for ALL x, the candidate is
truly correct. Briv cannot use this approach because Z3 4.8 does not support
`synth-fun` (SyGuS), and quantified bitvector formulas are expensive.

### Briv's Alternative: Three-Tier Generalization

Instead of full SMT verification, we use three increasingly strict tiers:

1. **Tier 1 — Identity operation pruning**: At candidate generation time,
   skip expressions that contain semantically redundant operations.
   These operations add cost but not meaning — they're the synthesis
   engine's equivalent of "padding" to reach the required depth.

2. **Tier 2 — Random-input verification**: After a candidate matches all
   examples, test it against 100+ random inputs. Check that evaluation
   doesn't error (division by zero, etc.) and that any user-specified
   postcondition (`[[post]]`) holds for the generated outputs.

3. **Tier 3 — Boundary testing**: For each parameter, test boundary values
   (0, 1, -1, i64::MAX, i64::MIN) plus the examples' own input values.
   If the candidate behaves pathologically on boundaries (constant output,
   division by zero for edge-case inputs), reject it.

## Tier 1: Identity Operation Pruning

### Research

Common identity operations across synthesis literature (Alur et al. 2017,
Solar-Lezama 2018) are pruned because they create "alias" expressions that
have the same semantics but higher cost. The cost model already penalizes them,
but the engine still generates and evaluates them, wasting time and producing
overfitted formulas.

The popcount overfit `((0 + (x0 >> 1)) + (x0 >> (x0 >> 1)))` contains `0 + x`
which is identity. Removing identity operations reduces the search space and
forces the engine to find cleaner expressions that are more likely to generalize.

### Identities to Prune

| Pattern | Condition | Why |
|---------|-----------|-----|
| `0 + X` / `X + 0` | LHS or RHS is `Decimal(0)` | Identity for Add |
| `1 * X` / `X * 1` | LHS or RHS is `Decimal(1)` | Identity for Mul |
| `X - 0` | RHS is `Decimal(0)` | Identity for Sub |
| `X / 1` | RHS is `Decimal(1)` | Identity for Div |
| `X >> 0` / `X << 0` | RHS is `Decimal(0)` | Identity for Shift |
| `X & 0xFF...FF` | RHS is all-ones (`i64::MAX`) | Identity for BitAnd |
| `X | 0` | RHS is `Decimal(0)` | Identity for BitOr |
| `X ^ 0` | RHS is `Decimal(0)` | Identity for BitXor |

Also prune:
- `X Mod 1` (always 0)
- `X / 0`, `X Mod 0` (division by zero — already pruned by div-zero check)
- Any `UnaryOp(Neg, Decimal(0))` (negation of 0 is 0)

### Implementation

In `generate_next_level` (engine.rs), in the binary ops section, add a check
after `op_result_type` and before the cross-product loop:

```rust
for lhs in *exprs {
    for rhs in *exprs {
        // Div/Mod/Shl with constant-zero RHS always fail evaluation
        if matches!(op, Div | Mod | Shl) && is_constant_zero(rhs) { continue; }
        // Identity pruning: skip semantically redundant operations
        if is_identity_op(*op, lhs, rhs) { continue; }
        result.push(Expr::BinaryOp(*op, ...));
    }
}
```

Helper function:

```rust
fn is_identity_op(op: BinaryOpKind, lhs: &Expr, rhs: &Expr) -> bool {
    match op {
        Add => is_constant_zero(lhs) || is_constant_zero(rhs),
        Sub => is_constant_zero(rhs),
        Mul => is_constant_one(lhs) || is_constant_one(rhs),
        Div => is_constant_one(rhs),
        Shl | Shr => is_constant_zero(rhs),
        BitAnd => is_all_ones(rhs),
        BitOr | BitXor => is_constant_zero(rhs),
        _ => false,
    }
}
```

With helpers:
- `is_constant_one(expr)` — true for `Decimal(1)` or `UnaryOp(Neg, Decimal(-1))`
- `is_all_ones(expr)` — true for `Decimal(-1)` (i64, which is `0xFFFFFFFFFFFFFFFF` in two's complement)

**Note**: `-1` IS all-ones in two's complement (`0xFFFFFFFFFFFFFFFF` for i64).
So `X & -1` should be pruned as identity for BitAnd.

### Tests

- `test_prune_identity_add_zero`: `0 + x0` should NOT appear at depth 2
- `test_prune_identity_mul_one`: `x0 * 1` should NOT appear at depth 2
- `test_prune_identity_shift_zero`: `x0 >> 0` should NOT appear at depth 2
- `test_prune_identity_sub_zero`: `x0 - 0` should NOT appear at depth 2
- `test_prune_non_identity_add_left`: `2 + x0` SHOULD still appear

## Tier 2: Random-Input Contract Verification

### Research

CEGIS systems verify candidates against a formal specification using SMT.
Briv's `[[post]]` syntax provides natural specification constraints:

```briv
defn popcount(x: Int) -> Int := { examples } [[popcount(x) < 64 && popcount(x) >= 0]]
```

When a candidate `f` is synthesized, verification checks: for each random
input `x`, does `f(x)` satisfy `0 <= f(x) < 64`? If 100 random inputs pass,
the candidate is likely correct. If even one fails, the candidate is rejected
and search continues.

Without a postcondition, verification still evaluates candidates on random
inputs and checks for basic soundness: no panics (division by zero), no
unreasonable outputs (e.g., popcount returning negative numbers).

### Architecture

New file: `src/derive/verify.rs`

```rust
/// Result of verification.
pub enum VerifyResult {
    /// Candidate passed all verification checks.
    Pass,
    /// Candidate failed verification. Vec contains failing input expressions.
    Fail(Vec<Vec<Expr>>),
}

/// Verify a synthesized candidate against random inputs and optional postcondition.
///
/// - Generates `sample_count` test inputs: boundary values (0, 1, -1, MAX, MIN)
///   plus random values within the parameter type's range.
/// - Evaluates the candidate on each input.
/// - If a postcondition is provided, checks that `post(candidate(input))` holds.
/// - If no postcondition, checks that evaluation doesn't error.
/// - Also checks for constant-output overfitting: if the candidate produces the
///   same output for ALL tested inputs while examples had diverse outputs, fail.
pub fn verify_candidate(
    candidate: &Expr,
    params: &[(String, Type)],
    postcondition: Option<&Expr>,
    sample_count: usize,
) -> VerifyResult { ... }

/// Generate test inputs: boundary + random.
pub fn generate_test_inputs(
    params: &[(String, Type)],
    count: usize,
) -> Vec<Vec<Expr>> { ... }
```

### Integration into synthesize()

In `src/derive/mod.rs`, after `enumerative_search` returns a candidate:

```rust
pub fn synthesize(name, block, params, max_depth, verify_samples: usize) -> Result<SynthesizedProgram, SynthesizeError> {
    // ... try enumerative search ...
    match engine::enumerative_search(name, params, &block.examples, max_depth) {
        Ok(Some(expr)) => {
            // Tier 2 + 3: Verify candidate against random inputs
            if verify_samples > 0 {
                // Use the definition's postcondition if available
                let post = block.postcondition.as_ref();
                match verify_candidate(&expr, params, post, verify_samples) {
                    VerifyResult::Fail(failing_inputs) => {
                        // Add failing inputs as new examples
                        let mut enriched_examples = block.examples.clone();
                        for input in &failing_inputs {
                            // Evaluate candidate on input to get the wrong output
                            // This becomes a constraint: "for this input, the output
                            // must be DIFFERENT from what the overfitted candidate gives"
                            // But actually we add it as a NON-example (constraint).
                            // For now: just log and try SMT fallback.
                        }
                        // Re-synthesize with enriched examples
                        // (or just return NoSolution)
                    }
                    VerifyResult::Pass => { /* accept */ }
                }
            }
            let cost = engine::CostModel::default().cost_of_expr(&expr);
            return Ok(SynthesizedProgram { body: vec![expr], cost, depth: max_depth as u8 });
        }
        // ...
    }
}
```

### CLI Flag

```rust
pub struct DeriveConfig {
    // ... existing ...
    pub verify_samples: usize,  // default 100, 0 = disable verification
}
```

## Tier 3: Boundary Testing

### Research

Many overfitted formulas fail on edge cases that are unlikely to appear in
random testing but are common in practice:
- Division by zero when a parameter is `0`
- Overflow when a parameter is `i64::MIN`
- Shift overflow when shift amount >= 64

### Implementation

Built into `verify_candidate` — after random testing, test on a fixed set of
boundary inputs:

For each Int parameter:
- `i64::MIN`
- `i64::MAX`
- `0`
- `1`
- `-1`
- Each example's input values for this parameter

If the candidate panics on any boundary input, it's rejected.

### Additional: Constant-Output Detection

If the candidate produces the same output for ALL tested inputs (random +
boundary), but the examples expect different outputs, the candidate is
overfitted to a constant. Reject it.

This catches formulas like `42` (constant) that match a single example.

## Implementation Order

1. **Create `src/derive/verify.rs`** — `verify_candidate()`, `generate_test_inputs()`
2. **Tier 1: Identity pruning** — add `is_identity_op()` + helpers to `engine.rs`
3. **Wire verification** into `synthesize()` in `mod.rs`
4. **Add `--verify-samples` flag** to `cli.rs`
5. **Add tests** for all tiers
6. **Test popcount at depth 4** — verify that pruning + verification reject the overfit

## Files

| File | Action |
|------|--------|
| `src/derive/verify.rs` | New — verification module |
| `src/derive/mod.rs` | Modified — call `verify_candidate()` after synthesis |
| `src/derive/engine.rs` | Modified — identity pruning in `generate_next_level()` |
| `src/derive/cli.rs` | Modified — `--verify-samples` flag |
| `docs/plans/2026-07-28-overfitting-prevention.md` | This plan |

## Verification

- `cargo test --lib` — all existing tests pass
- `cargo test --test derive_pipeline_test` — integration tests pass
- New tests in `verify.rs` — test input generation, boundary detection, identity pruning
- Popcount at depth 4 should either produce a correct formula or error with explanation
