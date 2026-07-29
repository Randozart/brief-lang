# CEGIS Counterexample Injection & SMT Reference Verification

Date: 2026-07-29
Status: Plan

## Problem

The derivation CEGIS loop can correctly reject overfitted candidates (ite chains)
via reference function verification, but it **never adds counterexamples** to
the example set on the random verification path. This means:

1. The ite chain is rejected at depth 3
2. `adaptive_depth` increases to 4
3. A new candidate is synthesized from the **original 4 examples** at depth 4
4. The new candidate (larger ite chain) passes the examples but is again rejected
5. Depth increases again — time out after 5 iterations

Without counterexample injection, CEGIS cannot converge. Additionally, the
SMT verification path (`smt_verify_candidate`) completely ignores the reference
function due to `let ref_fn: Option<&Expr> = None; // TODO` at `mod.rs:152`.

## Root Causes

### Root Cause 1: `VerifyResult::Fail` discards the correct output

`verify.rs:17-23`:
```rust
pub enum VerifyResult {
    Pass,
    Fail(Vec<Vec<Expr>>, String),  // failing inputs, reason string
}
```

The `Fail` variant has the failing input row but NOT the correct output.
For reference mismatch (`verify.rs:191-196`), the correct output IS available
as `ref_val` but is stringified into the reason message and lost.

When called from `mod.rs:98-107` / `mod.rs:109-117`, the CEGIS loop receives
`Fail(inputs, reason)` — it has the inputs but doesn't know the correct output.
Without the correct output, it cannot push a `DerivationExample` for re-synthesis.

### Root Cause 2: CEGIS loop doesn't push counterexamples from random verify

`mod.rs:98-107` (Z3 error fallback):
```rust
verify::VerifyResult::Fail(_, r) => {
    adaptive_depth += 1;           // only increases depth
    verified = false;
    // NEVER adds to examples!
}
```

`mod.rs:109-117` (no-postcondition path):
```rust
verify::VerifyResult::Fail(_, reason) => {
    adaptive_depth += 1;           // only increases depth
    continue;                      // NEVER adds to examples!
}
```

Compare with the Z3 path at `mod.rs:85-93` which DOES push:
```rust
CegisResult::Counterexample(inputs, correct_output) => {
    examples.push(DerivationExample { inputs, output, ... });
    verified = false;
}
```

### Root Cause 3: SMT verification ignores ref_fn

`mod.rs:152`: `let ref_fn: Option<&Expr> = None; // TODO: pass through from synthesize`

The variable is shadowed immediately on entry. Even if the signature accepted it,
the implementation at `verify_smt.rs:273` and `verify_smt.rs:298` makes the
postcondition path and reference path mutually exclusive — when both are present,
only the postcondition path fires.

## Fix 1: Counterexample Injection

### Step 1: Change `VerifyResult::Fail` to carry correct output

`verify.rs`:
```rust
pub enum VerifyResult {
    Pass,
    /// Failing inputs (Vec<Expr> per param), correct output for re-synthesis,
    /// and human-readable reason.
    Fail(Vec<Vec<Expr>>, Option<Expr>, String),
}
```

The `Option<Expr>` is the correct output when available:
- Reference mismatch → `Some(ref_val)` (converted from Value to Expr)
- Postcondition `@result = expr` → `Some(evaluated_rhs)` 
- Evaluation error → `None` (no correct output known)
- Constant output → `None` (not input-specific)

### Step 2: Propagate correct output at each Fail site

At `verify.rs:191-196` (reference mismatch):
```rust
// Current:
return VerifyResult::Fail(vec![input_row.clone()], format!("..."));
// New:
return VerifyResult::Fail(vec![input_row.clone()], Some(expr_from_val(&ref_val)), format!("..."));
```

Need a helper `expr_from_val` that converts `Value::Int(n)` → `Expr::Decimal(n)`.

### Step 3: Inject counterexample in CEGIS loop

At `mod.rs:98-107` and `mod.rs:109-117`:
```rust
verify::VerifyResult::Fail(inputs, correct_output, reason) => {
    if let (Some(input_row), Some(output)) = (inputs.first(), correct_output) {
        eprintln!("  cegis[{}/5] '{}': counterexample at {:?}, adding example", ...);
        examples.push(DerivationExample {
            inputs: input_row.clone(),
            output: Box::new(output),
            tolerance: None,
            span: crate::errors::Span::dummy(),
        });
    } else {
        adaptive_depth += 1;  // fallback: increase depth
    }
    verified = false;
}
```

For the `continue` path at line 115, the structure needs adjustment — `continue`
skips the `if verified { return Ok }` check. Either move the counterexample push
before the `continue`, or restructure to use `verified = false;` instead of `continue`.

## Fix 2: SMT Reference Verification

### Step 1: Thread ref_fn through smt_verify_candidate

`mod.rs`:
```rust
fn smt_verify_candidate(
    name: &str,
    candidate: &Expr,
    params: &[(String, Type)],
    examples: &[DerivationExample],
    postcondition: &Expr,
    precondition: Option<&Expr>,
    ref_fn: Option<(&Expr, &[String])>,  // NEW
) -> CegisResult {
    // Remove: let ref_fn: Option<&Expr> = None; // TODO
    ...
    // Pass through to build_verification_query
    let smt_query = verify_smt::build_verification_query(candidate, param_names, param_types, postcondition, precondition, ref_fn);
```

### Step 2: Update build_verification_query signature

`verify_smt.rs`:
```rust
pub fn build_verification_query(
    candidate: &Expr,
    param_names: &[String],
    param_types: &[String],
    postcondition: Option<&Expr>,
    precondition: Option<&Expr>,
    ref_fn: Option<(&Expr, &[String])>,  // NEW: body + its own param names
) -> String {
```

### Step 3: Use ref body's own param names in SMT conversion

When converting the ref body to SMT, bind its parameters by position:
```rust
if let Some((ref_body, ref_param_names)) = ref_fn {
    // Use ref_param_names when converting ref_body to SMT
    let ref_body_smt = expr_to_smt_term(ref_body, ref_param_names);
    // Also define ref's parameters in the forall and define-fun
}
```

### Step 4: Assert BOTH postcondition AND reference when both present

Instead of `if postcondition { A } else if ref_fn { B }`, use:
```rust
let mut conditions = Vec::new();
if let Some(post) = postcondition {
    conditions.push(format!("(=> {} {})", pre, post_with_f));
}
if let Some((ref_body, _)) = ref_fn {
    conditions.push(format!("(= (f {}) (ref {}))", args, args));
}
if conditions.is_empty() {
    return String::new();  // nothing to verify
}
let combined = conditions.join(" ");
query = format!("(assert (not (forall ({}) (and {}))))", params_decl, combined);
```

## Test Plan

### Unit Tests

| Test | File | What it verifies |
|------|------|-----------------|
| `verify_result_carries_output` | `verify.rs` | `Fail` with `Some(Expr::Decimal(N))` carries correct output |
| `cegis_adds_counterexample` | `mod.rs` | After Fail with correct output, examples grows by 1 |
| `ref_fn_passed_to_smt` | `verify_smt.rs` | `build_verification_query` with ref_fn emits `define-fun ref` |
| `combined_post_and_ref` | `verify_smt.rs` | Both postcondition and reference appear in the assertion |

### Integration Test

```
popcount_derive.bv with := popcount_ref:
  - CEGIS iter 1: synthesize ite, verify → Fail(input, ref_val)
    → push DerivationExample { inputs: [2], output: Decimal(1) }
  - CEGIS iter 2: synthesize from 5 examples → larger ite or general formula
    → verify → Fail(input, ref_val) for another input
  - ...eventually converges with a general popcount formula
  - Or fails after 5 iterations with NoSolution (acceptable for depth-3 bound)
```

## Implementation Order

1. `verify.rs`: Change `Fail` variant, propagate correct output at all fail sites
2. `mod.rs`: Inject counterexamples from `Fail` in both CEGIS paths
3. `verify_smt.rs`: Update `build_verification_query` parameters
4. `mod.rs`: Thread `ref_fn` through `smt_verify_candidate`
5. Test: popcount derivation with reference validator
6. Update existing test callers of `verify_candidate`
