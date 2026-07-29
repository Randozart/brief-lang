# Equivalence Chain — `:=` as Compile-Time Cross-Verification

## Concept

The `:=` operator on a `defn` declares a chain of **structurally equivalent
implementations**. Every body in the chain must produce the same output for
every input. The compiler:

1. Collects all bodies in the chain
2. Cross-verifies each against all others on random test inputs
3. Selects the first body that passes
4. Falls through to the last body (always accepted — the reference)

The chain is a compile-time contract: **these are equivalent, prove it.**
If the compiler can't prove equivalence (test inputs mismatch), it skips
to the next body. The last body is never skipped — it defines what "correct"
means.

## Syntax

```brief
// Equivalence chain: body1 is equivalent to body2 is equivalent to body3
defn name(params) -> ReturnType := body1 := body2 := body3;
```

Each body is one of:

| Body type | Syntax | Evaluated via | Has target? |
|-----------|--------|---------------|-------------|
| **Assembly** | `asm<target> fn ... { "..." }` | Compile-time gcc + FFI | Yes (required) |
| **Brief function** | `defn fn ... { ... }` | Interpreter | No |
| **Examples** | `{ input -> output; ... }` | CEGIS synthesis | No |

The last body in the chain is always unguarded (no target restriction) and
accepted without cross-verification — it is the reference.

## Resolution algorithm

```
For each body in chain (left to right):
  1. Target check: if body has a target annotation and it doesn't
     match the compile target, skip.
  2. Cross-verify: for N random test inputs, evaluate this body and
     compare with every OTHER body in the chain. If all outputs match
     for all inputs → select this body.
  3. If mismatch → skip to next body.
  4. Fall through: the last body is always selected.
```

### Target matching

An `asm<target>` body declares its target architecture. Non-asm bodies
(Brief functions, examples) have no target — they are available on any
target.

```
asm<x86_64> fn popcnt_asm(x: Int) -> Int { "popcnt {result}, {x}" };
asm<aarch64> fn popcnt_arm(x: Int) -> Int { "cnt {result}.8b, {x}.8b" };
defn popcount_ref(x: Int) -> Int { <parallel prefix> };

// On x86-64: asm_popcnt matches, cross-verified against popcount_ref
// On aarch64: asm_popcnt skipped (wrong target), asm_arm matches
// On unknown: both asm skipped, popcount_ref selected
defn popcount(x: Int) -> Int := popcnt_asm := popcnt_arm := popcount_ref;
```

### Cross-verification sampling

Each sample:
1. Generate random inputs matching the function's parameter types
2. Evaluate the candidate body on those inputs
3. Evaluate every other body on the same inputs
4. Compare outputs (float comparison uses tolerance 0.0001)

Default: 50 samples per candidate. Override with `--verify-samples N`.

### Evaluation methods

| Body type | How it's evaluated |
|-----------|-------------------|
| **Ref (Brief defn)** | Brief interpreter via `eval_expr` + `eval_block`. Handles `let` bindings, `term` returns, and `TermReturn` early-return pattern. Params bound by position. |
| **Asm** | Compiles to shared library via `gcc` + `as`, loads via `libloading`, calls function pointer with test inputs. Platform-independent file handling with temp directories. |
| **Synthesized** | Same as Ref — expression evaluated via interpreter. |

### Asm template variable substitution

```
asm<x86_64> fn popcnt_asm(x: Int) -> Int { "popcnt {result}, {x}" };

// At compile time, {result} → rax, {x} → rdi
// Assembles as: popcnt rax, rdi

// ABI register mapping:
//   x86_64: rax (result), rdi/rsi/rdx/rcx/r8/r9 (args 1-6)
//   aarch64: x0 (result + arg1), x1-x7 (args 2-8)
```

## Verification guarantees

### Random sampling (default)

With N random samples, the probability that a buggy body passes
cross-verification depends on how many inputs trigger the bug:

| Bug coverage | P(pass) with N=50 | P(pass) with N=1000 |
|-------------|------------------|--------------------|
| 1% of inputs | 0.99⁵⁰ ≈ 60.5% | 0.99¹⁰⁰⁰ ≈ 0.004% |
| 10% of inputs | 0.90⁵⁰ ≈ 0.5% | 0.90¹⁰⁰⁰ ≈ 1.7e-46 |

A body that passes 1000 random samples is extremely unlikely to differ
from the reference on any input.

### SMT verification (future)

The `build_verification_query` path can emit `(assert (not (forall (x)
(= (f x) (ref x)))))` for Z3. When Z3 can solve this (proven for all
inputs), mathematical certainty replaces sampling.

### Formal guarantee

The last body in the chain is always accepted. Even if every asm body
fails cross-verification, the reference body is used. The compiler never
emits unchecked assembly — the chain design ensures a known-correct
fallback exists.

## Call semantics

An `asm` function is callable like any other `defn`:

```brief
asm<x86_64> fn popcnt_asm(x: Int) -> Int { "popcnt {result}, {x}" };

defn use_popcnt(x: Int) -> Int {
    term popcnt_asm(x);     // direct call — no equivalence chain
};
```

The LLVM backend emits `popcnt_asm` as a standard function definition
with a `call asm sideeffect` body. Regular `Expr::Call` dispatch locates
it via `defn_params`/`defn_return_types` — same as any `defn`.

## Body types in detail

### Assembly (`asm<target> fn`)

```
asm<x86_64> name(params) -> ReturnType {
    "instruction {result}, {param}"
    "instruction {result}, {param2}"
};
```

- Must have a target annotation
- Body is a series of string literals (instruction templates)
- `{result}` and `{param}` are substituted with ABI registers
- Parsed as `TopLevel::AsmFn`
- Emitted as `call asm sideeffect` in LLVM IR

### Brief function (`defn`)

```
defn name(params) -> ReturnType { body };
```

- No target annotation (available on all targets)
- Body is evaluated via the Brief interpreter
- Let bindings, contracts, all Brief features supported

### Examples (derivation block)

```
{ input -> output; input -> output; }
```

- No target annotation
- Body is synthesized from examples via CEGIS
- Requires `--enumerative-depth N` to control search depth

## Integration with derivation

The existing derivation system (CEGIS loop, anti-unification, SMT synthesis)
becomes one way to produce a body for the equivalence chain. When the chain
contains a derivation block:

```
defn f(x: Int) -> Int := { 0->0; 1->1; } := ref_fn;
```

The compiler:
1. Tries CEGIS synthesis from the examples
2. If synthesis succeeds, cross-verifies the result against `ref_fn`
3. If match → use synthesized body
4. If synthesis fails or mismatch → use `ref_fn`

The derivation infrastructure we built (anti-unification, counterexample
injection) operates entirely inside step 1-2. The chain resolves
independently of how each body was generated.

## What this means for safety

**Before equivalence chains:**
```
defn popcnt(x: Int) -> Int { asm("popcnt rax, rdi") };
// Trust the programmer — no verification
```

**With equivalence chains:**
```
defn popcount(x: Int) -> Int := popcnt_asm := popcount_ref;
// Compiler proves popcnt_asm matches popcount_ref, or uses ref
```

The shift: correctness is no longer about "trust this implementation."
It's about "the compiler tested this implementation against the reference
and they agree on all test inputs." The reference is always available as
a fallback — the chain guarantees a correct implementation exists.

## Relationship to other features

| Feature | How it uses equivalence chains |
|---------|-------------------------------|
| **Derivation** | One body source among equals. CEGIS produces a candidate; the chain verifies it against the reference. |
| **Inline asm** | Target-annotated bodies in the chain. Cross-verified against a Brief reference at compile time. |
| **Contracts** | `[pre][post]` are on the function signature, not individual bodies. Every body in the chain must satisfy them. |
| **Metaprogramming** | A `$` metaprogram could generate bodies for the chain at compile time. |
| **MCMC superoptimization** | After chain resolution, the superoptimizer could optimize the selected body (with re-verification). |

## Configuration

```toml
# config/targets.toml
[".bv"]
backend = "llvm"
assembler = "platform"    # "keystone", "platform", "none"
cross_verify_samples = 50
```

The `assembler` field selects the compile-time asm validation backend.
The `cross_verify_samples` field controls the number of test inputs
(default 50, overridable via `--verify-samples` on the CLI).

## References

[CT'01] Chandra, S., Godefroid, P. & Larus, J. R. "Checking the 'Rights'
of a Program's Implementation." *STVR* 11(3), 2001.

[KE'16] Keystone Engine. keystone-engine.org, 2016.
