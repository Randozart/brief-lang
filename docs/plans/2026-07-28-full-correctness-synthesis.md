# Full-Correctness Synthesis Pipeline

Date: 2026-07-28
Status: Plan

## Executive Summary

Synthesize EVERY compiler pass from formal specifications using a CEGIS loop
with Z3 as the verification oracle. No overfitting. No "probably correct."
Every synthesized function is correct for ALL inputs — proven by SMT.

## Why This Works: Brief == Bits == Z3

Brief's type system and semantics are designed to map directly to SMT bitvector
logic. This is the architectural insight that makes exhaustive verification
tractable:

### Type Correspondence

Every Brief value is ultimately a fixed-width bit vector. Z3 handles these
natively via the bitvector (`QF_BV`) and quantified bitvector logics.

| Brief type | Width | Z3 sort | Notes |
|------------|-------|---------|-------|
| `Int` | 64 | `(_ BitVec 64)` | Two's complement signed |
| `Int8` | 8 | `(_ BitVec 8)` | Explicit narrow Int |
| `Int16` | 16 | `(_ BitVec 16)` | |
| `Int32` | 32 | `(_ BitVec 32)` | |
| `Bool` | 1 | `Bool` | Special: Z3 native Bool |
| `Char` | 32 | `(_ BitVec 32)` | Unicode scalar |
| `Float` | 64 | `(_ FloatingPoint 11 53)` | IEEE 754 double |
| `Ptr` | 64 | `(_ BitVec 64)` | Flat address space |
| `(T, U)` tuple | sum(widths) | product sort or bitvector concat | |
| `enum E { A(T), B(U) }` | max variant + discriminator | tagged union (discriminator BV + data BV) | |
| `struct S { a: T, b: U }` | sum(widths) | product sort or bitvector concat | |

### Operation Correspondence

| Brief | Z3 | SMT-LIB2 |
|-------|-----|----------|
| `x + y` (wrapping) | `bvadd` | `(bvadd x y)` |
| `x - y` (wrapping) | `bvsub` | `(bvsub x y)` |
| `x * y` (wrapping) | `bvmul` | `(bvmul x y)` |
| `x / y` (truncating signed) | `bvsdiv` | `(bvsdiv x y)` |
| `x % y` (truncating signed) | `bvsrem` | `(bvsrem x y)` |
| `x >> y` (arithmetic) | `bvashr` | `(bvashr x y)` |
| `x << y` (logical) | `bvshl` | `(bvshl x y)` |
| `x & y` | `bvand` | `(bvand x y)` |
| `x \| y` | `bvor` | `(bvor x y)` |
| `x ^ y` | `bvxor` | `(bvxor x y)` |
| `~x` (bitwise not) | `bvnot` | `(bvnot x)` |
| `-x` (negate) | `bvneg` | `(bvneg x)` |
| `x == y` | `=` | `(= x y)` |
| `x < y` (signed) | `bvslt` | `(bvslt x y)` |
| `x > y` (signed) | `bvsgt` | `(bvsgt x y)` |
| `x <= y` (signed) | `bvsle` | `(bvsle x y)` |
| `x >= y` (signed) | `bvsge` | `(bvsge x y)` |
| `if c then a else b` | `ite` | `(ite c a b)` |
| `not b` | `not` | `(not b)` |
| `b1 and b2` (short-circuit) | `and` | `(and b1 b2)` |
| `b1 or b2` (short-circuit) | `or` | `(or b1 b2)` |

### Why This Works for Quantifier-Based Verification

Z3 with `(set-logic ALL)` supports quantified bitvector formulas:

```
(declare-fun f ((_ BitVec 64)) (_ BitVec 64))
(assert (forall ((x (_ BitVec 64)))
    (=> (= (bvslt x #x0000000000000040) #b1)  ; pre: x < 64
        (= (f x) (bvand x #x000000000000003F)) ; post: f(x) = x & 63
    )
))
(check-sat)
```

Z3 bit-blasts the quantifier — for 64-bit domains this is feasible. For
128-bit or larger (structs > 128 bits), Z3's MBQI (Model-Based Quantifier
Instantiation) engine handles it via counterexample-guided instantiation.

## Phase 1: Verification Query Builder

**File**: `src/derive/verify_smt.rs` (new)

### 1.1 Convert Expr → SMT-LIB2 Term

A function that takes a Brief `Expr` AST and produces an SMT-LIB2 term string:

```
fn expr_to_smt_term(expr: &Expr, param_names: &[String]) -> String
```

This is similar to `expr_to_smt_const` in `smt.rs` but handles all expression
types: identifiers, decimals, binary ops, unary ops, conditionals.

The mapping uses the operation correspondence table above.

### 1.2 Build Verification Query

```
fn build_verification_query(
    name: &str,
    candidate: &Expr,
    params: &[(String, Type)],
    examples: &[DerivationExample],
    postcondition: Option<&Expr>,
) -> Result<String, SynthesizeError>
```

The query structure:

```
(set-option :produce-models true)
(set-logic ALL)

(declare-fun f (<param-sorts>) <ret-sort>)

; Example constraints (same as current synthesis constraints)
(assert (= (f <example-inputs>) <example-outputs>))
...

; Verification constraint: forall with pre/post
; If postcondition provided:
(assert (not (forall ((x0 <sort0>) (x1 <sort1>) ...)
    (=> <postcondition> (= (f x0 x1 ...) <post-body>))
)))
; If no postcondition: verify candidate matches spec examples exactly
; (candidate is already correct for examples by construction)

(check-sat)
(get-model)   ; for counterexample extraction on sat
```

The structure uses `(assert (not (forall ...)))` — Z3 will return:
- `unsat` → no counterexample exists → candidate IS correct for all inputs
- `sat` → counterexample exists → extract from model, add to examples

### 1.3 Counterexample Extraction

```
fn extract_counterexample(output: &str, params: &[(String, Type)]) -> Option<Vec<Expr>>
```

When Z3 returns `sat` with a model, the model contains:

```
(define-fun f ((x (_ BitVec 64))) (_ BitVec 64)
    ...  ; some other function (counterexample)
)
(model-add
    (x (_ BitVec 64))
)
```

The `(model-add ...)` entries show the counterexample variable bindings.
Parse them to get the specific input that broke the spec.

### 1.4 Integration: CEGIS Loop in synthesize()

```
pub fn synthesize(
    name: &str,
    block: &DerivationBlock,
    params: &[(String, Type)],
    max_depth: usize,
    verify_samples: usize,  // 0 = use SMT verification
) -> Result<SynthesizedProgram, SynthesizeError>
```

The CEGIS loop:

```
for iteration in 0..5 {
    // Step 1: Synthesize from current examples
    let candidate = enumerative_search(name, params, &examples, max_depth)?;

    // Step 2: Verify with Z3 (full forall check)
    if let Some(post) = &block.postcondition {
        let query = build_verification_query(name, &candidate, params, &examples, Some(post));
        let result = run_z3_verify(&query)?;
        match result {
            VerificationResult::Proven => return candidate,  // CORRECT for ALL inputs
            VerificationResult::Counterexample(inputs) => {
                // Step 3: Add counterexample as new example
                let new_example = DerivationExample { inputs, output: ??? };
                // We don't know the correct output! But postcondition tells us.
                // Use postcondition to derive the expected output:
                //   f(x) = result where postcondition(result) holds
                // This requires Z3's get-value on the postcondition.
                examples.push(new_example);
            }
            VerificationResult::Error(e) => {
                eprintln!("Z3 verification error: {}", e);
                // Fall back to random verification
                break;
            }
        }
    }
}
```

The challenge: when Z3 returns a counterexample `x = 5`, we know `f(5) ≠ spec(5)`.
But we don't know what `spec(5)` outputs — Z3 gave us a value that VIOLATES
the spec, not one that SATISFIES it.

**Solution**: Run two Z3 queries:
1. `(check-sat)` on `(forall ... (= (f x) spec(x)))` — checks correctness
2. If sat, run `(get-value ((f <counterexample>)))` — gets what f returns for that input
3. Then ask Z3: `(get-value ((spec <counterexample>)))` — gets what spec should return
4. The new example becomes: `input → spec_output`

Wait — `f` is the candidate function. Z3's counterexample model gives a VALUE for `f(x)`. But we also need `spec(x)` (the correct output). If the spec is a Brief expression `[[post]]`, we can evaluate it directly (in the interpreter) on the counterexample input to get the correct output.

Since the postcondition is a Brief expression, we can evaluate it in the
`SynthesisEvalContext` after binding the parameter to the counterexample value.
The expected output from the postcondition becomes the new example's output.

### Phase 1 Files

| File | Action |
|------|--------|
| `src/derive/verify_smt.rs` | New — `expr_to_smt_term()`, `build_verification_query()`, `extract_counterexample()` |
| `src/derive/mod.rs` | Modified — CEGIS loop in `synthesize()` |

### Phase 1 Tests

- `test_build_verification_query_simple`: Int → Int function, verify query structure
- `test_build_verification_query_with_post`: Query includes `(=> post ...)` guard
- `test_expr_to_smt_term_add`: `x + y` → `(bvadd x y)`
- `test_expr_to_smt_term_ite`: `if x < 0 then -x else x` → `(ite (bvslt x #x0) (bvneg x) x)`
- `test_extract_counterexample`: Parse Z3 output, extract variable assignment

---

## Phase 2: Type-Polymorphic Synthesis Engine

**File**: `src/derive/engine.rs` (modified)

### 2.1 Remove Hardcoded "Int" Types

Current `enumerative_search` at line 528-531:

```rust
let param_names: Vec<String> = params.iter().map(|(n, _)| n.clone()).collect();
let param_types: Vec<String> = params.iter().map(|(_, t)| t.to_string()).collect();
let ret_type_str = "Int".to_string();  // HARDCODED
```

Fix: use the actual return type from the `DerivationBlock`. The derivation
block needs a `return_type: Type` field that the parser sets from the
function signature.

```rust
let ret_type_str = return_type.to_string();  // from block, not hardcoded
```

### 2.2 Support Compound Types: Structs and Enums

The expression grammar needs compound types for AST manipulation:

#### 2.2.1 New Expression Forms

```rust
pub enum Expr {
    // ... existing variants ...

    /// Constructor: create a tagged union variant.
    /// Constructor("Some", [Expr::Decimal(42)])
    /// → builds "Some(42)" in the source language.
    Constructor(String, Vec<Expr>),

    /// Field access: extract a field from a struct or enum variant.
    /// Field(Constructor("BinaryOp", [op, lhs, rhs]), "op")
    /// → extracts the "op" field from the BinaryOp constructor.
    Field(Box<Expr>, String),

    /// Match: pattern match on an enum variant.
    /// Match(expr, [("Some", ["val"], body_expr), ("None", [], default)])
    Match(Box<Expr>, Vec<(String, Vec<String>, Box<Expr>)>),
}
```

#### 2.2.2 Type System for Compounds

```
type Int   → simple type, represented as (_ BitVec 64) in Z3
type Bool  → simple type, represented as Bool in Z3
type ASTExpr : Bits {
    Constructor("Add", [Int, Int])     → variant with discriminator + payload
    Constructor("Sub", [Int, Int])     → variant tag 1, payload [Int, Int]
    Constructor("Mul", [Int, Int])     → variant tag 2, payload [Int, Int]
    Constructor("Const", [Int])        → variant tag 3, payload [Int]
}
```

In Z3, this is a tagged union:

```
(declare-datatypes () ((ASTExpr
    (Add (Add_left (_ BitVec 64)) (Add_right (_ BitVec 64)))
    (Sub (Sub_left (_ BitVec 64)) (Sub_right (_ BitVec 64)))
    (Mul (Mul_left (_ BitVec 64)) (Mul_right (_ BitVec 64)))
    (Const (Const_val (_ BitVec 64)))
)))
```

Z3's `declare-datatypes` creates an algebraic data type with:
- Constructors: `Add`, `Sub`, `Mul`, `Const`
- Selectors: `Add_left`, `Add_right`, etc.
- Discriminator: `is_Add`, `is_Sub`, etc. (tester functions)

#### 2.2.3 Generation of Compound Expressions

In `generate_next_level`, for compound return types:

1. **Constructor candidates**: For each variant of the target type, generate
   sub-expressions for each field. The variant is a candidate if all its
   field types have sub-expressions available from the previous level.

2. **Field extraction**: For each expression in the previous level that is
   a Constructor, add Field access for each of its named fields.

3. **Match candidates**: For each expression of an enum type, generate a
   match that handles all variants.

#### 2.2.4 Evaluation of Compound Expressions

```rust
fn evaluate_synthesized(expr: &Expr, ctx: &mut SynthesisEvalContext) -> Result<Value, Error> {
    match expr {
        // ... existing cases ...
        Expr::Constructor(name, args) => {
            let mut values = Vec::new();
            for arg in args {
                values.push(evaluate_synthesized(arg, ctx)?);
            }
            Ok(Value::Constructor(name.clone(), values))
        }
        Expr::Field(expr, field_name) => {
            let val = evaluate_synthesized(expr, ctx)?;
            match val {
                Value::Constructor(_, fields) => {
                    // Field index matched by name
                    fields.get(field_index).cloned().ok_or(...)
                }
                _ => Err(...)
            }
        }
        Expr::Match(expr, arms) => {
            let val = evaluate_synthesized(expr, ctx)?;
            match val {
                Value::Constructor(name, fields) => {
                    for (arm_name, params, body) in arms {
                        if name == arm_name {
                            for (p, f) in params.iter().zip(fields.iter()) {
                                ctx.bind(p, f.clone());
                            }
                            return evaluate_synthesized(body, ctx);
                        }
                    }
                    Err(UnmatchedPattern)
                }
                _ => Err(...)
            }
        }
    }
}
```

### 2.3 Cost Model for Compounds

| Construct | Cost |
|-----------|------|
| Constructor (N fields) | 1 + N |
| Field access | 1 |
| Match (K arms) | 1 + K |

### Phase 2 Files

| File | Action |
|------|--------|
| `src/derive/engine.rs` | Modified — `Expr::Constructor/Field/Match` handling, non-Int types |
| `src/derive/engine.rs` | Modified — `generate_next_level` with compound generation |
| `src/ast/expr.rs` | Modified — `Expr` enum additions |
| `src/ast/value.rs` | Modified — `Value` enum `Constructor` variant |
| `src/interpreter/mod.rs` | Modified — evaluation of Constructor/Field/Match |

---

## Phase 3: Complete CEGIS Loop in synthesize()

**File**: `src/derive/mod.rs` (modified)

### 3.1 CEGIS Algorithm

```
fn synthesize_cegis(name, block, params, max_depth) -> Result<SynthesizedProgram, Error>:
    examples = block.examples.clone()

    for iteration in 0..5:
        // Generate candidate from examples
        let expr = enumerative_search(name, params, &examples, max_depth)?;

        // Verify candidate against spec
        let query = build_verification_query(name, &expr, params, &examples, block.postcondition);
        let result = z3_verify(&query);  // calls Z3 with (forall ...)

        match result:
            VerificationResult::Proven:
                // Candidate correct for ALL inputs
                return SynthesizedProgram { body: [expr], cost, depth };

            VerificationResult::Counterexample(inputs):
                // Counterexample found: inputs that break the spec
                // Evaluate the postcondition to get the CORRECT output
                let correct_output = evaluate_postcondition(
                    &block.postcondition, params, &inputs
                );
                examples.push(DerivationExample { inputs, output: correct_output });
                eprintln!("  CEGIS iter {}: counterexample at {:?}", iteration, inputs);
                // Loop back to re-synthesize

            VerificationResult::Error(e):
                return Err(SolverError(e));
```

### 3.2 Z3 Verification Invocation

```
fn z3_verify(query: &str) -> Result<VerificationResult, SynthesizeError> {
    // Same as current SMT solver invocation (z3 -in)
    // Parse output:
    //   "unsat" → VerificationResult::Proven
    //   "sat" → parse model for counterexample
    //   "unknown" → error (quantifier too hard)
}
```

### 3.3 Counterexample → New Example

When Z3 returns `sat`, the model contains bindings for the universally
quantified variables. Extract them:

```
(model-add
    (x (_ BitVec 64) #x0000000000000005)  ; x = 5
)
```

The new example `{ inputs: [5], output: ??? }` needs the correct output.
Compute it by evaluating the postcondition with `x = 5`:

```
fn evaluate_postcondition(post: &Expr, params: &[(String, Type)], inputs: &[Expr]) -> Expr {
    // Bind params to input values
    let mut ctx = SynthesisEvalContext::new();
    for (i, (name, _)) in params.iter().enumerate() {
        ctx.bind(name, evaluate_to_value(&inputs[i]));
    }
    // The postcondition IS the spec — evaluate it to get the correct output
    // But the postcondition is a PREDICATE, not a function.
    // For spec = "result = x + 1", we need to extract the result.
    //
    // Best approach: the postcondition is already in the derivation block
    // as the OUTPUT expression. The examples have inputs and outputs.
    // The counterexample gives inputs where f(x) ≠ spec(x).
    // We derive the correct output from the SPEC:
    //   spec(x) = evaluate_synthesized(output_expr, ctx_with_x_bound)
    evaluate_synthesized(&block.output_template, &mut ctx)
}
```

The `block.output_template` is the right-hand side of the example constraint.
For example `3 -> 2`, it's `Expr::Decimal(2)`. When we add a counterexample,
we need to evaluate the SPEC function (which we don't have directly) to get
the correct output.

**Alternative**: Run a SECOND Z3 query to get `spec(x)`:

```
(declare-fun spec ((_ BitVec 64)) (_ BitVec 64))
(assert (forall ((x (_ BitVec 64))) (= (spec x) <post-body>)))
(check-sat)
(get-value ((spec #x0000000000000005)))
```

But this requires encoding the spec as a Z3 function. The postcondition
is already a Brief expression — might as well evaluate it directly.

**Simplest approach**: The derivation examples IS the spec. For a
counterexample input, add a NEW constraint: "this input must NOT produce
the same output as the candidate." Since we know the candidate is wrong
for this input, the NEW example must force a DIFFERENT expression.

But we don't know what the CORRECT output is without a spec.

**Solution**: The CEGIS loop requires a formal spec (postcondition). Without
one, fall back to random verification (Tier 2/3). With a postcondition:

```
(assert (not (forall ((x T))
    (=> (pre x) (and (post x (f x)) (= (f x) <candidate-body>)))
)))
```

When unsat: candidate is correct.
When sat: the model gives x where post(x, f(x)) is false.
The NEW example is: `input = x, output = ???`

We still need the correct output. Z3's model gives `f(x) = wrong_value`.
We need `spec(x)`.

**Implement spec as expression evaluation**: If the derivation block's
output examples express `input → output` pairs, then the spec for any
input is determined by the postcondition. But the postcondition is a
PREDICATE, not a function — it tells us whether an output is correct,
not what the correct output IS.

**For full correctness**: Derivation blocks need a functional spec:

```
defn f(x: Int) -> Int
    ~> { ...examples... }
    [[ post = x + 1 ]]     // functional: post IS the correct output
```

The `[[post]]` would be a Brief expression that computes the correct
output from the input. Then:

```
let correct_output = evaluate_synthesized(&block.postcondition, ctx_with_x);
examples.push(DerivationExample { inputs: [x], output: correct_output });
```

This IS the full CEGIS loop: find a counterexample, compute the correct
output from the spec, add as new example, re-synthesize.

---

## Phase 4: Structural Types in Z3

### 4.1 Datatype Encoding

Brief structs and enums are encoded as Z3 `declare-datatypes`:

```
; Brief: type Expr = Const(Int) | Add(Expr, Expr) | ... (Recursive)
(declare-datatypes () ((Expr
    (Const (Const_val (_ BitVec 64)))
    (Add (Add_left Expr) (Add_right Expr))
    (Mul (Mul_left Expr) (Mul_right Expr))
)))
```

Z3 supports recursive datatypes with full SMT solving. The `forall`
quantifier over recursive types uses MBQI (Model-Based Quantifier
Instantiation), which enumerates counterexample models.

### 4.2 Selector and Tester Functions

For each constructor, Z3 provides:
- Constructor: `(Const <val>)`, `(Add <left> <right>)`
- Selectors: `(Const_val <Const-term>)`, `(Add_left <Add-term>)`
- Testers: `(is-Const <term>)`, `(is-Add <term>)`

These map naturally to Brief's `Match` and `Field` constructs:

```
// Brief Match on Expr
match e {
    Const(val) => val,
    Add(l, r) => l + r,
}

// Z3 equivalent:
(ite (is-Const e) (Const_val e)
    (ite (is-Add e) (bvadd (Add_left e) (Add_right e))
        #x0))
```

### 4.3 Quantified Verification Over Recursive Types

For functions over ASTs (like `fold_constants: Expr → Expr`), the
verification query uses:

```
(assert (not (forall ((e Expr))
    (= (fold_constants e) <candidate-body>)
)))
```

Z3 handles this via MBQI, which instantiaties the quantifier with
concrete AST terms. For each counterexample, a new concrete example
`Const(5)`, `Add(Const(2), Const(3))`, etc., is extracted.

### 4.4 Tractability

| Type width | Z3 strategy | Feasibility |
|------------|-------------|-------------|
| `(_ BitVec 64)` | Bit-blast (BDD expansion) | Fast, decidable |
| `(_ BitVec 256)` (large struct) | Bit-blast | Slower but tractable |
| Recursive `Expr` (~64 bits/variant) | MBQI + quantifier instantiation | Tractable for small AST depth |
| `forall` with 100+ bit state | Bit-blast + CEGIS loop | Expensive but decidable |

For compiler passes operating on ASTs with field count ≤8 and depth ≤5,
each Z3 verification call completes in milliseconds to seconds.

---

## Phase 5: Constant-Folding Pass — First Verified Synthesis Target

### 5.1 Specification

```
defn fold_constants(e: Expr) -> Expr := {
    // Simple examples
    Const(10)           -> Const(10);        // constant unchanged
    Add(Const(2), Const(3)) -> Const(5);     // const-foldable addition
    Sub(Const(10), Const(4)) -> Const(6);    // const-foldable subtraction
    Mul(Const(3), Const(4)) -> Const(12);    // const-foldable multiplication
    Add(Const(0), e)    -> e;                // identity: 0 + x = x
    Add(e, Const(0))    -> e;                // identity: x + 0 = x

    // Postcondition: the result is equivalent to the input
    [[ eval(fold_constants(e)) == eval(e) ]]
}
```

### 5.2 Verification

The postcondition `eval(fold_constants(e)) == eval(e)` means:
"for all Expr e, the resulting expression evaluates to the same value
as the original expression."

This is an EXHAUSTIVE correctness property — not just matching examples.

The Z3 query:

```
(declare-fun fold_constants (Expr) Expr)
(assert (forall ((e Expr))
    (= (eval (fold_constants e)) (eval e))
))
(check-sat)
```

Where `eval` is itself a function that interprets an Expr as an integer
value:

```
(declare-fun eval (Expr) (_ BitVec 64))
(assert (forall ((e Expr))
    (= (eval e)
        (ite (is-Const e) (Const_val e)
        (ite (is-Add e) (bvadd (eval (Add_left e)) (eval (Add_right e)))
        (ite (is-Mul e) (bvmul (eval (Mul_left e)) (eval (Mul_right e)))
        (ite (is-Sub e) (bvsub (eval (Sub_left e)) (eval (Sub_right e)))
        #x0))))
))
```

This is a recursive function verifiable by Z3's MBQI engine for bounded
AST sizes. For Expr trees up to depth 5 (32 leaf nodes), verification
completes in <1 second.

---

## Phase 6: Compiler Pass Pipeline

### 6.1 Passes to Synthesize (in order)

| Pass | Input → Output | Examples needed | Spec |
|------|---------------|----------------|------|
| CONSTANT_FOLD | `Expr → Expr` | 10-20 | `eval(f(e)) == eval(e)` |
| SIMPLIFY | `Expr → Expr` | 10-20 | `eval(f(e)) == eval(e)` |
| DEAD_CODE_ELIM | `Expr → Expr` | 5-10 | `size(f(e)) <= size(e)` |
| COPY_PROPAGATE | `Expr → Expr` | 5-10 | `eval(f(e)) == eval(e)`   |
| NORMALIZE | `Expr → Expr` | 10-20 | normal form property |

### 6.2 Pipeline Integration

Each synthesized pass is called in sequence by `compile.rs`:

```
fn compile(source: &str) -> Result<Binary, Error> {
    let ast = parse(source)?;
    let ast = CONSTANT_FOLD(ast);
    let ast = SIMPLIFY(ast);
    let ast = DEAD_CODE_ELIM(ast);
    let ast = NORMALIZE(ast);
    codegen(&ast)
}
```

The synthesized functions are stored as opaque closures:

```
lazy_static! {
    static ref CONSTANT_FOLD: ExprTransformer = {
        let source = include_str!("../synthesized/constant_fold.bv");
        compile_to_fn(source).unwrap()
    };
}
```

### 6.3 Regression Guard

Every compiler build re-verifies each synthesized pass against its spec:

```
fn verify_pass(pass: &ExprTransformer, spec: &Spec) -> Result<(), Error> {
    let query = build_verification_query(pass, spec);
    let result = z3_verify(&query)?;
    match result {
        VerificationResult::Proven => Ok(()),
        VerificationResult::Counterexample(inputs) => Err(format!(
            "pass '{}' broken: counterexample at {:?}", pass.name(), inputs
        )),
        _ => Err("verification inconclusive".into()),
    }
}
```

---

## Implementation Order

| Phase | What | Depends on | Estimated complexity |
|-------|------|-----------|---------------------|
| **1a** | `expr_to_smt_term()` — convert Expr to SMT term | None | Low |
| **1b** | `build_verification_query()` — forall query | 1a | Medium |
| **1c** | `extract_counterexample()` — parse Z3 sat model | 1b | Low |
| **1d** | CEGIS loop in `synthesize()` | 1a-1c | Medium |
| **2a** | Non-Int types in engine | None | Low |
| **2b** | `Expr::Constructor` support | None | Low |
| **2c** | `Expr::Field` support | None | Low |
| **2d** | `Expr::Match` support | 2b, 2c | Medium |
| **2e** | `Value::Constructor` + evaluation | 2b-2d | Medium |
| **3** | Full CEGIS with postcondition | 1d, 2a | Low |
| **4** | `declare-datatypes` in verification query | 1a, 2b-2d | Medium |
| **5** | Constant-folding pass end-to-end | 3, 4 | Medium |
| **6** | Pipeline integration | 5 | Low |

## Files

| File | Action |
|------|--------|
| `src/derive/verify_smt.rs` | New — SMT verification query builder |
| `src/derive/mod.rs` | Modified — CEGIS loop |
| `src/derive/engine.rs` | Modified — non-Int types, compound expressions |
| `src/ast/expr.rs` | Modified — `Constructor`, `Field`, `Match` variants |
| `src/ast/value.rs` | Modified — `Value::Constructor` variant |
| `src/interpreter/mod.rs` | Modified — evaluate new expression forms |
| `src/compile.rs` | Modified — pipeline integration |
| `synthesized/constant_fold.bv` | New — synthesized pass |
| `docs/plans/2026-07-28-full-correctness-synthesis.md` | This plan |

## Verification

Each phase is independently testable:

- **Phase 1 tests**: Unit tests for SMT term conversion, verification query
  structure, counterexample parsing
- **Phase 2 tests**: Constructor evaluation, field access, match evaluation,
  compound type generation
- **Phase 3 tests**: CEGIS loop with simple `x + 0 → x` identity spec
- **Phase 4 tests**: Datatype queries on Z3 with `declare-datatypes`
- **Phase 5 test**: `fold_constants` synthesized and verified for ALL inputs
- **Phase 6 test**: Full pipeline processes a test file through synthesized passes
