# Verification Chain — Phase C: Chain Resolver + Cross-Verification

Date: 2026-07-29
Status: Plan
Parent: `docs/plans/2026-07-29-verification-chain.md`

## Scope

Implement the verification chain resolver algorithm: given a chain of
bodies (from Phase B), select the first body whose target matches the
compile target and passes cross-verification against every other body
in the chain.

## 1. Chain Resolver

File: `new file — src/derive/chain.rs`

### 1.1 Core Algorithm

```rust
/// 2026-07-29: Resolution result for a verification chain.
pub enum ChainResult {
    /// A specific body was selected and compiled.
    Selected(Body),
    /// No body could be selected (empty chain or all failed).
    NoSelection,
}

/// 2026-07-29: A body in the verification chain — resolved to an
/// evaluable form for cross-verification.
pub enum Body {
    /// Assembly function (asm<target> ...).
    Asm(AsmFn),
    /// Reference function (defn body or inline expression).
    Ref(Expr),
    /// Derivation from examples (with optional reference).
    Derivation {
        examples: Vec<DerivationExample>,
        ref_fn: Option<Expr>,
    },
    /// Pre-synthesized expression (result of CEGIS for derivation).
    Synthesized(Expr),
}

/// 2026-07-29: Resolve a verification chain: select the first body
/// that passes target check and cross-verification.
///
/// Flow:
/// 1. Filter bodies by target compatibility (asm targets must match).
/// 2. Cross-verify each candidate against all others.
/// 3. Return the first candidate that passes, or the last (unguarded).
fn verify_candidate(
    idx: usize,
    candidates: &[Body],
    params: &[(String, Type)],
    samples: usize,
    assembler: &dyn AsmAssembler,
) -> Result<bool> {
    let body = &candidates[idx];
    for _ in 0..samples {
        let input = generate_sample_input(params);
        let output = evaluate_body(body, &input, assembler)?;
        for j in 0..candidates.len() {
            if idx == j { continue; }
            let other = evaluate_body(&candidates[j], &input, assembler)?;
            if !values_equal(&output, &other) {
                return Ok(false);
            }
        }
    }
    Ok(true)
}

pub fn resolve_chain(
    chain: &[ChainSegment],
    target_arch: &str,
    params: &[(String, Type)],
    samples: u32,
    assembler: &dyn AsmAssembler,
) -> ChainResult {
    let mut candidates: Vec<Body> = Vec::new();
    for segment in chain {
        if let Some(body) = resolve_segment(segment, target_arch) {
            candidates.push(body);
        }
    }
    if candidates.is_empty() {
        return ChainResult::NoSelection;
    }

    let samples = (samples as usize).max(1);
    for i in 0..candidates.len() {
        if let Ok(true) = verify_candidate(i, &candidates, params, samples, assembler) {
            return ChainResult::Selected(candidates.swap_remove(i));
        }
    }

    ChainResult::Selected(candidates.pop().unwrap())
}
```

### 1.2 Segment Resolution

```rust
/// 2026-07-29: Resolve a Ref(name) chain segment to a Body.
/// Returns None if the referenced definition has a target restriction
/// that doesn't match the compile target.
fn resolve_ref(name: &str, target_arch: &str) -> Option<Body> {
    let def = lookup_definition(name).ok()?;
    match def {
        TopLevel::AsmFn(asm_fn) => {
            if asm_fn.target != target_arch { return None; }
            Some(Body::Asm(asm_fn.clone()))
        }
        TopLevel::Definition(d) => {
            let body_expr = extract_body_expr(&d).ok()?;
            Some(Body::Ref(body_expr))
        }
        _ => None,
    }
}

/// 2026-07-29: Resolve a ChainSegment to a Body.
/// Returns None if the body has a target restriction that doesn't match,
/// or if synthesis from examples fails.
fn resolve_segment(segment: &ChainSegment, target_arch: &str) -> Option<Body> {
    match segment {
        ChainSegment::Ref(name) => resolve_ref(name, target_arch),
        ChainSegment::Derivation(block) => {
            synthesize_from_examples(&block.examples, block.ref_name.as_ref())
                .ok().map(Body::Synthesized)
        }
        ChainSegment::Examples(examples) => {
            synthesize_from_examples(examples, None)
                .ok().map(Body::Synthesized)
        }
    }
}
```

### 1.3 Body Evaluation

```rust
/// 2026-07-29: Evaluate a body on a single set of inputs.
/// 2026-07-29: Evaluate a Briv expression against test inputs.
/// Returns the result value. Handles TermReturn from defn bodies.
fn evaluate_ref_expr(expr: &Expr, input: &[Value]) -> Result<Value> {
    let mut bindings: HashMap<String, Value> = HashMap::new();
    for (i, val) in input.iter().enumerate() {
        bindings.insert(format!("x{}", i), val.clone());
    }
    let mut heap = VirtualHeap::new();
    let result = eval_expr(expr, &mut heap, &mut bindings)
        .map_err(|e| format!("evaluation error: {:?}", e))?;
    match result {
        Err(RuntimeError::TermReturn(v)) => Ok(v),
        other => other.map_err(|e| format!("eval error: {:?}", e)),
    }
}

/// 2026-07-29: Evaluate a body on a single set of inputs.
/// Returns the output value, or an error if evaluation fails.
fn evaluate_body(
    body: &Body,
    input: &[Value],
    _assembler: &dyn AsmAssembler,
) -> Result<Value> {
    match body {
        Body::Asm(_asm_fn) => {
            // For asm bodies: substitute {param} → ABI register, validate via
            // assembler, compile via LLVM JIT (JIT pending Phase D uses
            // interpreter with no-op asm body as MVP).
            Err("asm evaluation requires LLVM JIT — pending Phase D".into())
        }
        Body::Ref(expr) => evaluate_ref_expr(expr, input),
        Body::Synthesized(expr) => evaluate_ref_expr(expr, input),
    }
}
```

## 2. Sample Generation

```rust
/// 2026-07-29: Generate random test inputs for cross-verification.
/// Produces values that match the parameter types.
fn generate_sample_input(params: &[(String, Type)]) -> Vec<Value> {
    let mut rng = rand::thread_rng();
    params.iter().map(|(_, ty)| match ty {
        Type::Int => Value::Int(rng.gen_range(-1000i64..=1000)),
        Type::Float => Value::Float(rng.gen_range(-1000.0f64..=1000.0)),
        Type::Bool => Value::Bool(rng.gen_bool(0.5)),
        _ => Value::Int(0),
    }).collect()
}
```

## 3. Value Comparison

```rust
/// 2026-07-29: Compare two values with tolerance.
fn values_equal(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Int(ai), Value::Int(bi)) => ai == bi,
        (Value::Float(af), Value::Float(bf)) => {
            (af - bf).abs() < 0.0001  // tolerance
        }
        (Value::Bool(ab), Value::Bool(bb)) => ab == bb,
        _ => false,
    }
}
```

## 4. Integration with Existing CEGIS Loop

File: `src/derive/mod.rs`

### 4.1 New Entry Point

```rust
/// 2026-07-29: Resolve a verification chain, returning the compiled
/// body and any discovered helpers from the derivation process.
pub fn resolve_derivation_chain(
    chain: &[ChainSegment],
    params: &[(String, Type)],
    ret_type: &Type,
    target_arch: &str,
) -> Result<SynthesizedProgram> {
    let target_config = TargetConfig::load();
    let entry = target_config.lookup(".bv").unwrap();
    let assembler = get_assembler(entry);
    let samples = entry.cross_verify_samples;

    match chain_resolve(chain, target_arch, params, samples, assembler.as_ref()) {
        ChainResult::Selected(body) => {
            // Convert the selected body to a SynthesizedProgram
            body_to_program(body)
        }
        ChainResult::NoSelection => {
            Err(SynthesizeError::NoSolution("no body selected from chain".into()))
        }
    }
}
```

### 4.2 Modifications to `synthesize()`

The existing `synthesize()` function in `mod.rs` currently handles a
single derivation block. It gains an early path:

```rust
pub fn synthesize(
    // ... existing params ...
    chain: &[ChainSegment],
) -> Result<SynthesizedProgram> {
    // 2026-07-29: If there's a verification chain, resolve it first
    if !chain.is_empty() {
        match resolve_derivation_chain(chain, params, ret_type, target_arch) {
            Ok(prog) => return Ok(prog),
            Err(_) => {} // fall through to existing derivation path
        }
    }
    // ... existing single-block derivation logic ...
}
```

## 5. Tests

File: `src/derive/tests/chain.rs` (new) or `src/derive/engine.rs`

| Test | What it verifies |
|------|-----------------|
| `chain_selects_matching_target` | Asm body with matching target is preferred |
| `chain_skips_nonmatching_target` | Asm body with wrong target is skipped |
| `chain_falls_through_to_last` | When all guarded bodies fail, last is used |
| `chain_cross_verify_mismatch` | Body that fails cross-verification is skipped |
| `chain_cross_verify_match` | Body that matches all others is selected |
| `chain_single_segment` | Single-element chain resolves (backward compat) |
| `chain_derivation_fallback` | Derivation failure falls through to next body |
| `chain_empty` | Empty chain returns NoSelection |

## 6. Files Changed

| File | Change |
|------|--------|
| `src/derive/chain.rs` | New: ChainResult, Body, resolve_chain, evaluate_body, resolve_segment |
| `src/derive/mod.rs` | Add `resolve_derivation_chain()`, integrate with existing `synthesize()` |
| `src/derive/lib.rs` | N/A — mod.rs re-exports |

## 7. Implementation Order

1. `ChainResult` + `Body` enums in `src/derive/chain.rs`
2. `generate_sample_input()` + `values_equal()`
3. `resolve_segment()` — reference and asm body resolution
4. `evaluate_body()` — interpreter-based evaluation for ref/synthesized
5. `resolve_chain()` — main algorithm
6. `resolve_derivation_chain()` entry point in mod.rs
7. Integration into `synthesize()` flow
8. Tests
9. `cargo test --lib` — all pass
