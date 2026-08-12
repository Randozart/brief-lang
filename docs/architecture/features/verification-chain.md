# Verification Chain — `:=` as Cross-Verified Implementation Selection

Status: Architecture doc
Date: 2026-07-29

## Concept

The `:=` operator on a `defn` declares a chain of **structurally equivalent
implementations**. Every body in the chain must produce the same output for
the same input. The compiler:

1. Collects all bodies in the chain
2. Cross-verifies them against each other using random test inputs
3. Selects the best (fastest, smallest, or first matching) body that passes
4. Compiles the selected body as the function's implementation

This is NOT primarily a derivation mechanism. It's a **compile-time
verification and implementation selection** mechanism. Derivation (synthesis
from examples) is one way to generate a candidate body for the chain, but
it's optional.

## Syntax

```briev
// The chain: := body1 := body2 := ... := bodyN
// Last body is always unguarded (guaranteed fallback).

// Assembly + reference: fastest wins, verified against ref
defn popcount(x: Int) -> Int [result >= 0 && result < 64]
  := asm<x86_64> fn popcount_x86(x: Int) -> Int { "popcnt {result}, {x}" }
  := asm<aarch64> fn popcount_arm(x: Int) -> Int { "cnt {result}.8b, {x}.8b" }
  := defn popcount_ref(x: Int) -> Int { <formula> };

// Examples as test cases + reference fallback
defn popcount(x: Int) -> Int
  := { 0->0; 1->1; 3->2; 7->3 }
  := popcount_ref;

// Pure chain — multiple Briev implementations cross-verified
defn abs(x: Int) -> Int
  := defn abs_branch(x: Int) -> Int { if x < 0 then -x else x }
  := defn abs_math(x: Int) -> Int { sqrt(x * x) };
```

## Chain resolution at compile time

For each body in order (left to right):

1. **Target compatibility**: If the body has a target annotation (e.g.,
   `asm<x86_64>`), check if it matches the compile target. If not → skip.
2. **Cross-verification**: Generate N random test inputs (default 50).
   Evaluate EVERY other body in the chain on those inputs.
   If this body's outputs match all others' → selected.
   If mismatch → skip to next body.
3. **Fall through**: The last body has NO target annotation and is always
   accepted (no cross-verification required — it IS the ground truth).

## Body types

| Body source | Syntax | Needs target? | Cross-verified? |
|------------|--------|---------------|-----------------|
| **Assembly** | `asm<target> fn name(args) -> T { ... }` | Yes | Yes — against all other bodies |
| **Briev reference** | `defn name(args) -> T { ... }` | No | Yes — against all other bodies |
| **Examples** | `{ input->output; ... }` | No | Yes — examples serve as test cases; the examples body is cross-verified too |
| **Derivation** | `{ examples } := ref_fn` | No | Yes — synthesis tries to find a formula; if it passes cross-verification against the chain, use it |

## Assembly body syntax

```briev
asm<x86_64> fn popcount_x86(x: Int) -> Int {
    // Braced variables {name} are substituted with the ABI register
    // for the given parameter/return value on the target architecture.
    // x86-64 SysV: x → rdi, result → rax
    "popcnt {result}, {x}"
};

asm<aarch64> fn popcount_arm(x: Int) -> Int {
    // ARM64: x → x0, result → x0
    "cnt {result}.8b, {x}.8b"
};
```

The compiler knows the ABI for each target and maps `{param}` to the
appropriate register. No explicit register binding syntax needed.

## Cross-verification algorithm

```python
def resolve_chain(bodies, target, samples=50):
    for body in bodies:
        # Step 1: target check
        if body.target is not None and body.target != target:
            continue
        
        # Step 2: cross-verify against all OTHER bodies
        failed = False
        for _ in range(samples):
            input = random_input()
            output = evaluate(body, input)
            for other in bodies:
                if other is body:
                    continue
                if other.target is not None and other.target != target:
                    continue  # skip incompatible others for cross-check
                other_output = evaluate(other, input)
                if output != other_output:
                    failed = True
                    break
            if failed:
                break
        
        if not failed:
            return body  # selected!
    
    # Step 3: fall through to last (unguarded) body
    return bodies[-1]
```

## Performance selection

When multiple bodies pass cross-verification, the compiler selects the
fastest one. This requires benchmarking each candidate at compile time
(a fixed small iteration count, similar to profile-guided optimization).

For the MVP, "first matching" selection is sufficient. Performance
selection is an optimization for later.

## Relationship to existing derivation

The existing derivation logic (CEGIS loop, anti-unification, SMT synthesis)
becomes one way to generate a body for the chain. When the user writes:

```briev
defn f(x: Int) -> Int
  := { 0->0; 1->1; 3->2; }
  := ref_fn;
```

The compiler:
1. Tries to synthesize a formula from the examples
2. Cross-verifies the synthesized formula against ref_fn
3. If it passes → use the synthesized formula
4. If synthesis fails OR the formula fails cross-verification → use ref_fn

The derivation machinery we've built (CEGIS, counterexample injection,
SMT fallback) operates entirely inside step 1-2. The chain resolves
independently of how each body was generated.

## What this means for `asm`

Assembly is the hardest implementation to verify because the compiler
can't reason about its semantics. Cross-verification solves this: the asm
body is tested against a Briev reference function on random inputs. If
all outputs match, the asm is provably equivalent for the tested inputs.

The confidence depends on the number of test samples:
- 50 samples: catches gross errors (wrong instruction, wrong register)
- 1000 samples: catches edge cases (overflow, sign extension)
- Full SMT verification (future): proves equivalence for ALL inputs

For the MVP, random testing with 50-100 samples is sufficient. The chain
design ensures that even if a buggy asm body passes cross-verification,
the reference body is still available as a fallback on the next compile
(the buggy body will fail and get skipped).

## Open questions

1. **Contract verification**: If the function has `[pre][post]` contracts,
   should each body be verified against the contract individually? Yes —
   but this applies to ALL bodies, not just the selected one.
   
2. **Asm clobber list**: Does asm need a clobber declaration (registers it
   modifies beyond inputs/outputs)? For safety, yes — but for the MVP,
   the compiler can assume standard ABI compliance (callee-saved registers
   preserved by the asm).

3. **Asm block syntax**: Single string `"instr"` or multi-line block with
   `;` separators? Multi-line is more readable for complex sequences.

4. **cross-verification of asm**: Requires running the asm at compile time.
   This needs either an interpreter for the asm (complex) or compiling a
   test harness and running it (simpler, but requires a working assembler
   for non-native targets via cross-compilation).
