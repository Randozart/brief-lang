# LLVM Backend Constants + Reactive Convergence Contract — Plan

**Date**: 2026-05-31
**Author**: Briv Compiler (via OpenCode)

---

## Guiding Principles

| Rule | Meaning |
|------|---------|
| **Contract-first** | Contracts (`[pre][post]`) are the source of truth — never weaken them |
| **Compile-time only** | Contracts are consumed by the typechecker and proof engine at compile time. **Zero** runtime contract evaluation is emitted in LLVM IR |
| **Reactive convergence** | For `node [pre][post]`: "keep firing until post holds, prove you can get there." The proof engine verifies bounded convergence statically |
| **Per-tick = watchdog** | `?[cond]` / `?![cond]` provides tick-level invariants orthogonal to convergence |
| **More efficiency** | Contracts exist to make code *more* efficient — the analysis extracts loop bounds and constant values that LLVM exploits for unrolling, vectorization, and constant-folding |

---

## Phase 1: `src/backend/llvm.rs` — Constant declarations

### Problem
The LLVM backend silently drops `TopLevel::Constant` (`_ => {}`). State declarations (`let`) are the only way to persist values, forcing everything into the `%State` struct with GEP+load indirection.

### Changes

**1a. Add `constants` map**
```rust
constants: HashMap<String, (Type, Expr)>,
```
Initialize in `LLVMBackend::new()`.

**1b. Populate during scan**
```rust
TopLevel::Constant(c) => {
    self.constants.insert(c.name.clone(), (c.ty.clone(), c.expr.clone()));
}
```

**1c. Emit LLVM `constant` globals**
Iterate `self.constants` and emit:
- `Float` → `@name = constant float bitcast (i32 <hex> to float)`
- `Integer(n)` → `@name = constant i64 <n>`
- `Bool(b)` → `@name = constant i1 <b>`
- `Neg(Float(f))` → `@name = constant float bitcast (i32 <neg_hex>) to float`
- `Neg(Integer(n))` → `@name = constant i64 <-n>`
- Panic on unsupported constant expressions

**1d. Resolve constant identifiers in expressions**
Before `field_index_map` lookup, check `self.constants` — emit `load <ty>, <ty>* @name` and cast to i64.

**1e. Update folding pass**
When looking up `bound_var`, fall back from `field_index_map` to `self.constants`.

**1f. Update `emit_folded_main`**
Accept `Option<&str>` constant name for total; emit `load i64, i64* @total` instead of GEP+load from state.

---

## Phase 2: `benchmarks/iir_filter.bv` — Fix

### Problem
Coefficients, input, and total declared with `let` (mutable state) instead of `const`. Postcondition defaults to `[true]`.

### Changes
- `const` for: `b0`, `b1`, `b2`, `a1`, `a2`, `input`, `total`
- `let` for: `x1`, `x2`, `y1`, `y2`, `count`
- Contract: `node process [count < total][count == total]`
- Postcondition is convergence contract — never emitted in LLVM IR

---

## Phase 3: `src/proof_engine.rs` — Convergence verification

### Problem
Proof engine evaluates postconditions per-path (per-tick). For reactive convergence contracts like `[count < total][count == total]`, the per-tick check fails because `count == total` doesn't hold after one tick.

### Changes

**3a. Add `is_convergence_contract()` helper**
Detects reactive transactions where postcondition compares a state variable to a bound.

**3b. Add `verify_convergence()` method**
1. Extract (var, bound, delta) from contract and body
2. Verify `bound` is invariant (not assigned) or a constant
3. Compute steps to convergence: `(|bound - initial|) / delta`
4. Verify steps is non-negative and bounded
5. Use induction: verify one tick makes progress, compute remaining steps symbolically
6. Accept if convergence provable; error with proof chain otherwise

**3c. Modify `verify_contract_implication`**
Before emitting P008 for a reactive transaction, try convergence verification.

**3d. Add tests**
Convergence accepted (counter + delay-line side effects), convergence rejected (non-monotonic).

---

## Phase 4: Verification

```
cargo test --lib
cargo build
./target/release/briv-compiler llvm benchmarks/iir_filter.bv -o benchmarks/iir_filter.ll
clang -O3 -march=native -o benchmarks/iir_filter benchmarks/iir_filter.ll
./benchmarks/iir_filter
```
