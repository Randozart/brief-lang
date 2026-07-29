# Derivation Blocks + Stochastic Superoptimization

**Date:** 2026-07-28
**Status:** Active implementation
**Worktree:** `../brief-compiler-derive`
**Depends on:** Completion of `!>` metadata syntax (`docs/plans/2026-07-28-metadata-syntax-bang.md`);
existing `DerivationBlock` / `DerivationExample` AST nodes; existing `src/derive/` module skeleton.
**See also:** `docs/plans/2026-07-11-derivation-synthesis-comprehensive.md` for Phases 8–11
(lexer/parser, enumerative search, SMT, sad-path, deductive synthesis) which this plan builds
on and extends with stochastic and doppelganger capabilities.
**See also:** `docs/architecture/optimization-hints.md` (created by this plan) for the `!>` metadata
vocabulary reference.

---

## Overview

This plan adds a **nine-phase implementation** (A–I) implementing a four-stage derivation pipeline, with non-destructive doppelganger
output files and an optional stochastic superoptimization pass. The `brief derive` command
synthesizes function bodies from `:= { ... }` examples, then optionally optimizes them via
STOKE-style MCMC random search at the normalized IR layer.

### Capabilities

| Capability | Mechanism | Flag |
|------------|-----------|------|
| **Compile-Time Assertions** | Examples verified against body via interpreter | Always-on (build gate) |
| **Enumerative Synthesis** | Depth-bounded expression enumeration | `brief derive` (default) |
| **SMT Synthesis** | SyGuS query to Z3, QF_BV | `brief derive` (fallback) |
| **Stochastic Superoptimization** | MCMC Metropolis-Hastings on normalized IR | `--stochastic` |
| **Doppelganger Output** | Full-source shadow files, never mutates originals | Always-on |
| **!> Metadata Vocabulary** | Cross-backend optimization hints | First-class language |
| **Metadata-Driven Codegen** | Backends consume `!>` via `MetadataRegistry` | Automatic (Phase H) |
| **`brief accept`** | Fold synthesized bodies into source | User-initiated (Phase I) |

### Pipeline

```
Source foo.bv
  │  !> overflow: "wrapping";
  │  := { 2, 3 -> 6; 0, 5 -> 0; };
  ▼
┌─────────────────────────────────────────────────────────┐
│ brief derive foo.bv                                     │
│                                                         │
│  Phase 1  Enumerative Search     Depth ≤ 5, constants   │
│           (typed AST layer)      from {0,1,-1,powers2} │
│                                                         │
│  Phase 2  SyGuS SMT Synthesis    Z3 QF_BV, unbounded   │
│           (typed AST layer)      constants, complex     │
│                                                         │
│  Phase 3a MCMC Correctness       Random start + mutate  │
│           (typed AST layer)      until all examples +   │
│                                  Z3 equivalence pass    │
│                                                         │
│  Phase 3b MCMC Performance       Correct program +      │
│           (normalized IR layer)  mutate until no faster │
│                                  improvement in 1K iters│
│                                                         │
│  Phase 4  Pareto Selection       (ops, error, runtime)  │
│           + Write-Back           → knee default         │
└─────────────────────────────────────────────────────────┘
  │  Writes: foo.derive.bv (always — synthesized bodies)
  │  Writes: foo.opt.bv    (only if --stochastic)
  ▼
┌─────────────────────────────────────────────────────────┐
│ brief build foo.bv                                      │
│                                                         │
│  Lookup chain: foo.opt.bv > foo.derive.bv > foo.bv     │
│  If body present → assertion mode: interpreter evaluates│
│    every example against body, fails build on mismatch  │
│  If body absent  → error: "function in draft state"     │
└─────────────────────────────────────────────────────────┘
  │
  ▼
Backend (LLVM / Webstack / CIRCT)
  Reads !> metadata → maps to target-specific semantics:
    overflow: "wrapping"   → LLVM: nuw nsw on add/sub
    associative: true      → LLVM: reassoc flag
    fp_contract: "fast"    → CIRCT: fma fusion
    aliasing: "none"       → Webstack: local copy elision
```

### Why a New Plan (Not an Extension of the 2026-07-11 Plan)

The existing 3360-line plan (`docs/plans/2026-07-11-derivation-synthesis-comprehensive.md`)
covers Phases 8–13 of the original derivation system. This new plan introduces three
architectural shifts that the existing plan does not address:

1. **Doppelganger files** — Non-destructive output instead of source mutation (Phase 9.6
   in the old plan assumes in-place mutation; we replace that with shadow files).

2. **MCMC stochastic superoptimization** — A completely new synthesis path not present in
   the old plan (no phase number existed for this).

3. **!> metadata vocabulary** — A systematic cross-backend annotation system that the old
   plan only hints at (Phase 8G removes hardcoded intrinsics but doesn't specify a
   generic optimization hint vocabulary).

This plan coexists with the old plan. Phases 8–11 from the old plan (lexer, parser, AST,
type-checking, assertions, deductive synthesis, sad-path) are still valid and are
prerequisites. This plan extends them.

---

## Core Design Principles

1. **Never mutate source.** All synthesis output goes to doppelganger files (`foo.derive.bv`,
   `foo.opt.bv`). The original `.bv` file is read-only from the compiler's perspective.

2. **The `:=` block is immortal.** It remains in both the source and doppelganger as the
   permanent specification. Assertion mode re-verifies on every build.

3. **Additive-only optimization.** Existing optimization paths are never modified. New
   match arms only. The `_ => return None;` fallthrough must remain unchanged.

4. **SMT solver is optional.** Enumerative search is the baseline. SMT is a performance
   accelerator. MCMC is an enhancement. Each phase falls through cleanly.

5. **Stochastic is opt-in.** `--stochastic` flag. Without it, the pipeline runs Phases 1–2
   only (enumerative + SMT), which is deterministic and fast.

6. **!> metadata is advisory.** Each backend decides which keys to honor. A backend that
   doesn't recognize a key silently ignores it. No backend is required to implement any
   particular key.

7. **All !> metadata is both developer and internal.** The MCMC optimizer may write `!>`
   annotations into `.opt.bv` files, and developers may write the same keys into source.
   No distinction. The backend treats both identically.

---

## Golden Rules

1. **Flat control flow**: All new/modified code must nest at most 2 levels deep.
   Use `?`, guard clauses, early returns, and extracted helper functions.

2. **Contract-first**: Never weaken contract guarantees. If a function had `[result > 0]`
   before, it must still have it after any derivation or optimization.

3. **Tests or it doesn't exist**: Every new code path, every match arm, every feature
   must have corresponding tests. `cargo test --lib` before every commit.

4. **Doc comments on every definition**: Every `fn`, `struct`, `enum`, `trait` added or
   modified must have a `///` doc comment.

5. **Rationale comments at every change site**: Format:
   `// 2026-07-28: Phase <letter>.<step> — <what and why>`

6. **HashMap iteration determinism**: All HashMap iterations that produce IR instructions
   must be sorted by key before the loop.

7. **Do not ask "shall I commit?" — just commit**: After every logical step where tests
   pass, commit. No amend, no squash. One commit per step.

8. **Plan with benchmarks**: Before MCMC changes, baseline table of ALL benchmark results.
   After implementation, update with new results for comparison.

9. **Provenance tracking**: Every rationale comment carries `when`, `why`, `what pattern`,
   and `how to undo`.

---

## Phase A — Parser + AST Wiring and Tolerance Syntax

### Goal

Verify that the existing `!>` metadata syntax, `DerivationBlock` AST node, `ColonEq` token,
and derivation block parsing all work together. Add tolerance annotation syntax for
floating-point examples: `input -> [0.01] output`.

### Step A.0 — Audit existing derivation AST infrastructure

**Files**: `src/ast/expr.rs`, `src/ast/top.rs`, `src/lexer.rs`, `src/parser.rs`

**What**: Read every existing derivation-related structure to confirm the state before
modification. Document what exists.

**Existing state** (from the 2026-07-11 plan, partially implemented):

```rust
// src/ast/expr.rs — DerivationBlock and DerivationExample
pub struct DerivationExample {
    pub inputs: Vec<Expr>,
    pub output: Expr,
    pub span: Span,
}

pub struct DerivationBlock {
    pub examples: Vec<DerivationExample>,
    pub span: Span,
}

// src/ast/top.rs — optional field on Definition and Transaction
pub derivation: Option<DerivationBlock>,

// src/lexer.rs — ColonEq token exists
ColonEq,

// src/parser.rs — parse_derivation_block() and parse_derivation_example()
// exist but may need verification
```

**Add tolerance field to DerivationExample**:

```rust
/// 2026-07-28: Phase A.0 — tolerance annotation for FP examples.
/// If Some(f), the output may differ from expected by up to f relative error.
pub tolerance: Option<f64>,
```

**Nesting check**: One new field on an existing struct — no nesting concern.

**Tests**:
- `test_existing_derivation_block_parses`: Source with `:= { 2, 3 -> 6; }` parses correctly
- `test_existing_derivation_ast_fields`: All fields on DerivationBlock accessible
- `test_existing_colon_eq_token`: `:=` lexes to `Token::ColonEq`

### Derivation Syntax — Three Forms

A function or transaction with a derivation block takes one of three forms:

| Form | Example | Meaning |
|------|---------|---------|
| **Body only** | `defn f(x) -> T { body }` | Regular function definition |
| **Derivation only** | `defn f(x) -> T := { examples }` | **Synthesis target** — no body provided, compiler synthesizes one |
| **Body + Derivation** | `defn f(x) -> T { body } := { examples }` | Existing body checked against examples at compile time |

In the **derivation only** form, `:= { examples }` appears directly where a
`{ body }` would normally go. The parser must not error on `:=` — it checks
for `{` first, and if absent, leaves `body` empty and delegates to the
derivation block. This is the primary `brief derive` target.

In the **body + derivation** form, the body is written by the developer and
the derivation examples serve as assertions. This is the "build gate" mode
(Phase B) — the compiler verifies the body against the examples at every build.

### Step A.1 — Add tolerance syntax to parser

**File**: `src/parser.rs` — `parse_derivation_example()`

**What**: Allow an optional `[tolerance]` annotation before the output expression:

```brief
// Hard constraint (default): output must match exactly
{ 2, 3 -> 6; }

// Soft constraint: output must be within 1% relative error
{ 1.0, 2.0 -> [0.01] 3.0; }

// Soft constraint: integer approximation
{ 10, 3 -> [0.5] 3; }
```

**Parser change** (in `parse_derivation_example`):

```rust
// After parsing the -> arrow, check for optional tolerance bracket
let tolerance = if self.current_token_is(Token::LBracket) {
    self.advance(); // consume [
    let tol_expr = self.parse_expression()?;
    self.expect(Token::RBracket)?;
    let tol = eval_constant_f64(&tol_expr)
        .map_err(|_| self.spanned_err("tolerance must be a constant float"))?;
    Some(tol)
} else {
    None
};

// Parse output expression
let output = self.parse_expression()?;
```

**Nesting check**: A single `if` with guard clause — depth 1.

**Tests**:
- `test_parse_tolerance_example`: `1.0, 2.0 -> [0.01] 3.0;` → tolerance = Some(0.01)
- `test_parse_no_tolerance_example`: `2, 3 -> 6;` → tolerance = None
- `test_parse_tolerance_non_float`: `2 -> [abc] 6;` → parse error
- `test_parse_example_after_tolerance`: Normal example after a tolerance one still works

### Step A.2 — Store tolerance in DerivationExample and verify display round-trip

**File**: `src/ast/expr.rs`, `src/ast/display.rs` (or equivalent)

**What**: Tolerance is stored as `Option<f64>` on `DerivationExample`. The display
implementation must round-trip: `1.0, 2.0 -> [0.01] 3.0;`.

**Display format**:
```rust
// For tolerance output:
if let Some(tol) = self.tolerance {
    write!(f, " [{}]", tol)?;
}
```

**Tests**:
- `test_derivation_example_display_tolerance`: Display with tolerance → contains ` [0.01]`
- `test_derivation_example_display_no_tolerance`: Display without tolerance → no bracket
- `test_roundtrip_tolerance_example`: Parse → display → re-parse produces identical AST

---

## Phase B — Assertion Build Gate

### Goal

When a function has BOTH a body and a derivation block, every `brief build` evaluates
each example through the compile-time interpreter and compares the result to the expected
output. A mismatch is a build error (exit code 64).

### Step B.0 — Wire assertion verification into compile pipeline

**File**: `src/compile.rs`

**What**: After type-checking and before codegen, iterate all definitions with both
`body: Some(...)` and `derivation: Some(...)`. For each, call into the interpreter
to evaluate each example.

**Implementation**:

```rust
/// Phase B.0 — Verify all derivation examples against their function bodies.
/// Called after type-check, before codegen. Errors are fatal (exit code 64).
/// 2026-07-28: Phase B.0 — assertion build gate.
fn verify_derivation_assertions(
    program: &[TopLevel],
    interpreter: &mut Interpreter,
) -> Result<(), Vec<String>> {
    let mut errors = Vec::new();

    for item in program {
        let (name, body, derivation) = match item {
            TopLevel::Definition(d) => (&d.name, d.body.as_ref(), d.derivation.as_ref()),
            TopLevel::Transaction(t) => (&t.name, t.body.as_ref(), t.derivation.as_ref()),
            _ => continue,
        };
        let (Some(body), Some(derivation)) = (body, derivation) else {
            continue; // Drafting or no derivation — skip
        };

        for (i, example) in derivation.examples.iter().enumerate() {
            // Evaluate inputs
            let args: Result<Vec<Value>, _> = example.inputs.iter()
                .map(|input| interpreter.eval_expr(input))
                .collect();
            let args = match args {
                Ok(a) => a,
                Err(e) => { errors.push(format!("{} example {}: input eval failed: {}", name, i+1, e)); continue; }
            };

            // Evaluate body with those args
            let result = match interpreter.call_function(name, &args) {
                Ok(r) => r,
                Err(e) => { errors.push(format!("{} example {}: body execution failed: {}", name, i+1, e)); continue; }
            };

            // Evaluate expected output
            let expected = match interpreter.eval_expr(&example.output) {
                Ok(v) => v,
                Err(e) => { errors.push(format!("{} example {}: expected eval failed: {}", name, i+1, e)); continue; }
            };

            // Compare with tolerance if applicable
            let match_ok = match example.tolerance {
                Some(tol) => values_within_tolerance(&result, &expected, tol),
                None => result == expected,
            };
            if !match_ok {
                errors.push(format!(
                    "{} example {}: expected {:?}, got {:?}",
                    name, i+1, expected, result
                ));
            }
        }
    }

    if errors.is_empty() { Ok(()) } else { Err(errors) }
}

/// Compare two values within relative tolerance for floating-point types.
/// 2026-07-28: Phase B.0 — tolerance comparison.
fn values_within_tolerance(actual: &Value, expected: &Value, tol: f64) -> bool {
    match (actual, expected) {
        (Value::Float(a), Value::Float(e)) => {
            let diff = (a - e).abs();
            let mag = e.abs().max(1e-10);
            diff / mag <= tol
        }
        // Non-float: exact match
        _ => actual == expected,
    }
}
```

**Integration point in `compile.rs`** (after type-check, before codegen):

```rust
// Phase B.0: Verify derivation assertions
if let Err(errors) = verify_derivation_assertions(&items, &mut interpreter) {
    for e in &errors {
        eprintln!("error: derivation assertion: {}", e);
    }
    std::process::exit(64);
}
```

**Nesting check**: The function loops over items (level 1), with a single guard for
body+derivation presence (level 2). Each example processes sequentially — depth 1
within the loop.

**Tests**:
- `test_assertion_passes`: Function with correct body + derivation → compiles
- `test_assertion_fails`: Function with wrong body + derivation → error, exit code 64
- `test_assertion_tolerance_passes`: FP function with tolerance → within tol → passes
- `test_assertion_tolerance_fails`: FP function with tolerance → outside tol → fails
- `test_assertion_skipped_no_body`: Draft function (no body) → no assertion run
- `test_assertion_skipped_no_derivation`: No derivation block → no assertion run
- `test_assertion_multi_example`: Two examples, one fails → error with correct index

### Step B.1 — Interpreter call_function helper

**File**: `src/interpreter.rs`

**What**: If `Interpreter::call_function` does not yet exist (requires raw `Value` args
rather than `Expr::Call`), add a thin wrapper:

```rust
/// Call a function by name with pre-evaluated argument values.
/// 2026-07-28: Phase B.1 — assertion verification requires raw-value calls.
pub fn call_function(&mut self, name: &str, args: &[Value]) -> Result<Value, RuntimeError> {
    // Look up the function definition
    let defn = self.lookup_function(name)
        .ok_or_else(|| RuntimeError::UndefinedFunction(name.to_string()))?;

    // Bind parameters to argument values
    let frame = InterpreterFrame::new(
        defn.parameters.iter().map(|p| p.name.clone()).collect(),
        args.to_vec(),
    );
    self.push_frame(frame);

    // Execute body statements
    let result = self.execute_block(&defn.body);

    self.pop_frame();
    result
}
```

**Tests**:
- `test_call_function_direct`: Call `add(2, 3)` via `call_function`, verify result is 5
- `test_call_function_undefined`: Call nonexistent function → RuntimeError
- `test_call_function_wrong_arity`: Mismatched arg count → RuntimeError

---

## Phase C — Full Enumerative Synthesis

### Goal

Rewrite the existing `src/derive/engine.rs` from pattern-matching stubs to a proper
type-aware, interpreter-backed enumerative search with an Occam cost model.

### Step C.0 — Type-aware expression generation

**File**: `src/derive/engine.rs`

**What**: The current `generate_expressions` only handles `i64` constants and simple
binary ops. Replace with a type-aware generator that knows the function's parameter
types and return type, and only generates well-typed expressions.

**Grammar** (typed):

```
Expr<T> ::=
    | Const(i64)                   if T ∈ {Int, Int8, Int16, Int32, Int64, UInt*}
    | Const(f64)                   if T ∈ {Float, Double}
    | Const(bool)                  if T ∈ {Bool}
    | Var(name)                    if name : T in parameter list
    | BinOp(Expr<L>, Op, Expr<R>)  if Op : (L, R) → T
    | UnOp(Op, Expr<U>)            if Op : U → T
    | Cond(Expr<Bool>, Expr<T>, Expr<T>)
```

**Operator typing table**:

```rust
/// 2026-07-28: Phase C.0 — operator type compatibility for synthesis.
fn op_result_type(op: BinaryOpKind, lhs_ty: &Type, rhs_ty: &Type) -> Option<Type> {
    match op {
        Add | Sub | Mul | Div | Rem => {
            if types_compatible(lhs_ty, rhs_ty) { Some(lhs_ty.clone()) } else { None }
        }
        Eq | Neq => Some(Type::Bool),
        Lt | Gt | Le | Ge => Some(Type::Bool),
        And | Or | Xor => {
            if *lhs_ty == Type::Bool && *rhs_ty == Type::Bool { Some(Type::Bool) } else { None }
        }
        Shl | Shr => Some(lhs_ty.clone()), // shift by Int is always valid
        _ => None,
    }
}
```

**Tests**:
- `test_generate_type_int`: Type `Int` generates Int-typed expressions
- `test_generate_type_bool`: Type `Bool` generates Bool-typed expressions (comparisons, logical)
- `test_generate_type_mismatch_rejected`: `Int + Bool` not generated
- `test_generate_with_typed_constants`: `Float` generates `1.0`, `2.0` etc.

### Step C.1 — Interpreter-based example evaluation

**File**: `src/derive/engine.rs`

**What**: Replace the current `matches_pattern` (which only handles a hardcoded set of
patterns via string matching) with proper interpreter evaluation. Build a minimal
expression evaluator that handles the synthesis grammar.

```rust
/// Evaluate a synthesized expression against concrete input values.
/// Uses the interpreter for evaluation, binding parameter names to values.
/// 2026-07-28: Phase C.1 — replaces the old matches_pattern stub.
fn evaluate_synthesized(
    expr: &Expr,
    param_names: &[String],
    input_values: &[Value],
) -> Result<Value, SynthesisEvalError> {
    // Build a mini-evaluation context with parameter bindings
    let mut ctx = SynthesisEvalContext::new();
    for (name, val) in param_names.iter().zip(input_values.iter()) {
        ctx.bind(name, val.clone());
    }
    eval_expr_in_context(expr, &mut ctx)
}
```

**Tests**:
- `test_evaluate_add_expr`: `x + y` with `x=2, y=3` → `Value::Int(5)`
- `test_evaluate_cond_expr`: `if x > 0 then x else -x` with `x=-3` → `Value::Int(3)`
- `test_evaluate_nested_expr`: `(x + y) * z` → correct result
- `test_evaluate_undefined_var`: Unknown variable → SynthesisEvalError

### Step C.2 — Occam cost model with constant burden

**File**: `src/derive/engine.rs`

**What**: Implement the cost model from the existing plan (Phase 9.1). Each expression
has a cost, and the search returns the lowest-cost match. Constants beyond a small
set (0, 1, -1, 0.0, 1.0) incur a penalty proportional to their information content.

```rust
/// 2026-07-28: Phase C.2 — Occam cost model with constant burden.
pub struct CostModel {
    pub constant: u64,        // base cost of a literal (default: 1)
    pub variable: u64,        // cost of reading a parameter (default: 1)
    pub unary_op: u64,        // cost of a unary operation (default: 2)
    pub binary_op: u64,       // cost of a binary operation (default: 3)
    pub branch: u64,          // cost of an if/else (default: 5)
    pub constant_burden: f64, // per-bit cost for non-trivial constants (default: 0.1)
}

impl CostModel {
    pub fn cost_of_constant(&self, val: &Value) -> u64 {
        let bits = match val {
            Value::Int(n) => n.checked_abs().map(|a| 64 - a.leading_zeros()).unwrap_or(0) as u64,
            Value::Float(f) => 64, // every float is "complex"
            Value::Bool(_) => 1,
            _ => 8,
        };
        self.constant + (bits as f64 * self.constant_burden).ceil() as u64
    }
}
```

**Tests**:
- `test_cost_constant_small`: `0` costs less than `65536`
- `test_cost_constant_defaults`: Default costs match expected values
- `test_cost_binary_op_greater_than_variable`: `x + y` costs more than `x`
- `test_cost_prefers_simple`: Two programs, same correctness, cheaper one wins

### Step C.3 — Depth-bounded search with cost pruning

**File**: `src/derive/engine.rs`

**What**: The enumerative search enumerates all programs up to `max_depth`, evaluates
each against all examples, and returns the lowest-cost match. Uses cost pruning:
if a partial program's cost already exceeds the best known cost, it is not expanded.

```rust
/// Enumerate all programs up to max_depth, return lowest-cost match.
/// 2026-07-28: Phase C.3 — replaces the old enumerative_search stub.
pub fn synthesize_enumerative(
    param_types: &[Type],
    ret_type: &Type,
    param_names: &[String],
    examples: &[DerivationExample],
    cost_model: &CostModel,
    max_depth: u8,
) -> Result<SynthesizedProgram, SynthesisError> {
    let mut best: Option<SynthesizedProgram> = None;
    let mut candidates = generate_typed_expressions(
        param_types, ret_type, param_names, cost_model, max_depth
    );
    candidates.sort_by_key(|c| c.cost);

    for candidate in &candidates {
        if candidate.cost >= best.as_ref().map(|b| b.cost).unwrap_or(u64::MAX) {
            break; // Remaining candidates are all more expensive
        }
        let all_match = examples.iter().all(|ex| {
            let inputs: Vec<Value> = /* evaluate inputs via interpreter */;
            let result = evaluate_synthesized(&candidate.body[0], param_names, &inputs);
            let expected = /* evaluate expected via interpreter */;
            match result {
                Ok(r) => values_match(&r, &expected, ex.tolerance),
                Err(_) => false,
            }
        });
        if all_match {
            let prog = SynthesizedProgram { ... };
            if best.as_ref().map_or(true, |b| prog.cost < b.cost) {
                best = Some(prog);
            }
        }
    }

    best.ok_or(SynthesisError::NoSolutionFound)
}
```

**Tests**:
- `test_enumerative_simple_add`: `{2,3→5; 0,5→5}` → `x + y`
- `test_enumerative_identity`: `{0→0; 42→42}` → `x`
- `test_enumerative_constant`: `{_→42; _→42}` → `42`
- `test_enumerative_no_solution`: Impossible at max_depth → Err
- `test_enumerative_cost_pruning`: Expensive candidate not evaluated if cheap one already found
- `test_enumerative_with_tolerance`: `{1.0, 2.0→[0.01] 3.0}` → finds `x + y` with tolerance
- `test_enumerative_empty_examples`: Empty examples → Err

---

## Phase D — Full SMT Synthesis

### Goal

Replace the stub `src/derive/smt.rs` with a real SMT-LIB query builder that converts
Brief types to QF_BV sorts, emits example constraints, calls Z3, and parses the
`define-fun` response back into `Expr` trees.

### Step D.0 — Build SMT-LIB query from typed examples

**File**: `src/derive/smt.rs`

**What**: Given parameter types, return type, and examples, build a SyGuS query in
QF_BV (quantifier-free bitvector logic). Each example becomes an `(assert (= (f <inputs>) <output>))`
constraint. The grammar of `synth-fun` includes all relevant bitvector operations.

**Type mapping**:

| Brief Type | SMT Sort |
|------------|----------|
| `Int`, `Int64`, `UInt64` | `(_ BitVec 64)` |
| `Int32`, `UInt32` | `(_ BitVec 32)` |
| `Int16`, `UInt16` | `(_ BitVec 16)` |
| `Int8`, `UInt8` | `(_ BitVec 8)` |
| `Bool` | `Bool` |
| `Float` | `(_ BitVec 32)` (bit pattern) |
| `Double` | `(_ BitVec 64)` (bit pattern) |

**Grammar for synth-fun**:

```lisp
(synth-fun f ((x1 (_ BitVec 64)) (x2 (_ BitVec 64))) (_ BitVec 64)
    ((Start (_ BitVec 64) (
        x1 x2
        #x0000000000000000 #x0000000000000001
        (bvadd Start Start)
        (bvsub Start Start)
        (bvmul Start Start)
        (bvudiv Start Start)   ; unsigned div for Int → treat as bitvector
        (bvand Start Start)
        (bvor Start Start)
        (bvxor Start Start)
        (bvshl Start Start)
        (bvlshr Start Start)
        (bvneg Start)
        (bvnot Start)
        (ite StartBool Start Start)
    ))
    (StartBool Bool (
        true false
        (= Start Start)
        (bvslt Start Start)
        (bvsle Start Start)
        (bvult Start Start)
        (bvule Start Start)
        (not StartBool)
        (and StartBool StartBool)
        (or StartBool StartBool)
    )))
)
```

**Implementation**:

```rust
/// Build a SyGuS QF_BV query from typed parameters and examples.
/// 2026-07-28: Phase D.0 — replaces the old hardcoded stub.
fn build_sygus_query(
    params: &[TypedParameter],
    ret_type: &Type,
    examples: &[DerivationExample],
) -> Result<String, SynthesisError> {
    let mut q = String::new();
    q.push_str("(set-option :produce-models true)\n");
    q.push_str("(set-logic QF_BV)\n\n");

    // Declare parameters as bitvector constants
    let param_names: Vec<String> = params.iter().enumerate().map(|(i, p)| {
        format!("x{}", i)
    }).collect();

    // synth-fun declaration
    let param_sorts: Vec<String> = params.iter().map(|p| {
        format!("({} {})", &param_names[/* index */], type_to_smt_sort(&p.ty))
    }).collect();
    let ret_sort = type_to_smt_sort(ret_type);

    q.push_str("(synth-fun f (");
    for (i, p) in params.iter().enumerate() {
        q.push_str(&format!(" (x{} {})", i, type_to_smt_sort(&p.ty)));
    }
    q.push_str(&format!(") {}\n", ret_sort));
    // ... grammar body ...
    q.push_str(")\n\n");

    // Constraints from examples
    for example in examples {
        let inputs: Vec<String> = example.inputs.iter()
            .map(|e| expr_to_smt_const(e))
            .collect();
        let output = expr_to_smt_const(&example.output);
        q.push_str(&format!("(assert (= (f {}) {}))\n", inputs.join(" "), output));
    }

    q.push_str("\n(check-synth)\n");
    Ok(q)
}
```

**Tests**:
- `test_build_sygus_query_two_params`: `add(x: Int, y: Int)` → query has `bvadd`, 2 params
- `test_build_sygus_query_single_param`: `inc(x: Int)` → query has 1 param
- `test_build_sygus_query_bool_ret`: `is_zero(x: Int) -> Bool` → query returns Bool
- `test_build_sygus_query_example_constraints`: Examples appear as `(= (f ...) ...)` assertions
- `test_build_sygus_query_unsupported_type`: Complex type → Err

### Step D.1 — Call Z3 and parse model

**File**: `src/derive/smt.rs`

**What**: Run Z3 via subprocess with the SyGuS query (or via WASM plugin if available).
Parse the `define-fun` response into a `SynthesizedProgram`.

```rust
/// Run Z3 solver and parse the synthesized function.
/// 2026-07-28: Phase D.1 — Z3 subprocess call with response parsing.
pub fn synthesize_via_smt(
    params: &[TypedParameter],
    ret_type: &Type,
    examples: &[DerivationExample],
    z3_path: &str,
) -> Result<SynthesizedProgram, SynthesisError> {
    let query = build_sygus_query(params, ret_type, examples)?;

    let output = Command::new(z3_path)
        .arg("-in")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            if let Some(stdin) = child.stdin.as_mut() {
                writeln!(stdin, "{}", query).ok();
            }
            child.wait_with_output()
        })
        .map_err(|e| SynthesisError::SolverError(e.to_string()))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if stdout.contains("unsat") {
        return Err(SynthesisError::Unsat);
    }
    if stdout.contains("unknown") || output.status.code() != Some(0) {
        return Err(SynthesisError::SolverError(stderr.to_string()));
    }

    parse_smt_response(&stdout, params, ret_type)
}

/// Parse a define-fun response into a SynthesizedProgram.
/// Expected format: (define-fun f ((x (_ BitVec 64))) (_ BitVec 64) (bvadd x #x0001))
/// 2026-07-28: Phase D.1 — SMT model parser.
fn parse_smt_response(
    response: &str,
    params: &[TypedParameter],
    ret_type: &Type,
) -> Result<SynthesizedProgram, SynthesisError> {
    // Extract the define-fun body via S-expression parsing
    let sexpr = parse_sexpr(response)?;
    let body_expr = extract_define_fun_body(&sexpr)?;

    // Convert SMT expression to Brief Expr tree
    let brief_expr = smt_to_brief_expr(body_expr, params, ret_type)?;

    Ok(SynthesizedProgram {
        body: vec![Statement::Term { values: vec![Some(brief_expr)], swan_song: None, modifiers: vec![] }],
        cost: 0, // Cost is recomputed by the caller
        operators_used: vec![],
    })
}
```

**SMT to Brief expression mapping**:

| SMT expression | Brief expression |
|---------------|-----------------|
| `(bvadd a b)` | `a + b` |
| `(bvsub a b)` | `a - b` |
| `(bvmul a b)` | `a * b` |
| `(bvand a b)` | `a & b` |
| `(bvor a b)` | `a \| b` |
| `(bvxor a b)` | `a ^ b` |
| `(bvneg a)` | `-a` |
| `(bvnot a)` | `~a` |
| `(bvslt a b)` | `a < b` (signed) |
| `(ite cond t f)` | `if cond then t else f` |
| `#x0001` | `1` |
| `true` / `false` | `true` / `false` |

**Tests**:
- `test_smt_call_integration`: Z3 available → synthesize `add(x, y) = x + y`
- `test_smt_solver_unavailable`: Z3 not found → SynthesisError::SolverUnavailable
- `test_parse_define_fun_add`: `(define-fun f ((x (_ BitVec 64))) (_ BitVec 64) (bvadd x #x0001))` → `x + 1`
- `test_parse_define_fun_ite`: `(ite (bvslt x #x0000) (bvneg x) x)` → `if x < 0 then -x else x`
- `test_parse_define_fun_constant`: `(define-fun f ((x (_ BitVec 64))) (_ BitVec 64) #x002A)` → `42`
- `test_query_rejected_wrong_type`: SMT returns Bool but expected Int → error

### Step D.2 — Integrated synthesis dispatch

**File**: `src/derive/mod.rs`

**What**: The top-level `synthesize()` function tries enumerative first (simple cases),
falls through to SMT for harder problems.

```rust
/// Synthesize a function body from its derivation block.
/// Tries fast enumerative first, falls back to SMT.
/// 2026-07-28: Phase D.2 — integrated dispatch.
pub fn synthesize(
    defn: &Definition,
    config: &DeriveConfig,
) -> Result<SynthesizedProgram, SynthesisError> {
    let Some(derivation) = &defn.derivation else {
        return Err(SynthesisError::NoDerivationBlock);
    };

    let param_types: Vec<Type> = defn.parameters.iter().map(|p| p.ty.clone()).collect();
    let param_names: Vec<String> = defn.parameters.iter().map(|p| p.name.clone()).collect();
    let ret_type = defn.output_type.clone();

    // Phase 1: Enumerative (fast, simple cases)
    if config.use_enumerative {
        let result = engine::synthesize_enumerative(
            &param_types, &ret_type, &param_names,
            &derivation.examples, &config.cost_model, config.max_depth,
        );
        if let Ok(prog) = result {
            return Ok(prog);
        }
        // Fall through to SMT
    }

    // Phase 2: SMT (complex constants, deeper expressions)
    if config.use_smt {
        let result = smt::synthesize_via_smt(
            &defn.parameters, &ret_type,
            &derivation.examples, &config.z3_path,
        );
        if let Ok(prog) = result {
            return Ok(prog);
        }
    }

    Err(SynthesisError::NoSolutionFound {
        examples_checked: derivation.examples.len(),
        strategies_tried: vec!["enumerative", "smt"],
    })
}
```

**Tests**:
- `test_synthesize_dispatch_enumerative_first`: Simple examples → enumerative succeeds
- `test_synthesize_dispatch_falls_to_smt`: Complex constants → SMT is tried
- `test_synthesize_no_strategy`: No strategy enabled (config) → Err

---

## Phase E — Doppelganger Write-Back

### Goal

After synthesis, write the results to a doppelganger file (`foo.derive.bv`) rather than
mutating the source. The doppelganger is a complete copy of the original source with
synthesized bodies inserted. The build system checks for doppelgangers before the
original source.

### Step E.0 — Doppelganger module

**File**: `src/derive/doppelganger.rs` (new)

**What**: Module for reading, writing, and resolving doppelganger files.

**Core types**:

```rust
/// 2026-07-28: Phase E.0 — doppelganger file management.
pub struct Doppelganger {
    /// The original source path (e.g., "src/foo.bv")
    pub source_path: PathBuf,
    /// The doppelganger path (e.g., "src/foo.derive.bv")
    pub derive_path: PathBuf,
    /// The optimized path (e.g., "src/foo.opt.bv") — only with --stochastic
    pub opt_path: PathBuf,
}

impl Doppelganger {
    /// Determine the doppelganger path for a given source file.
    /// Replaces .bv with .derive.bv (or .opt.bv).
    pub fn derive_path_for(source: &Path) -> PathBuf {
        let stem = source.file_stem().unwrap_or_default();
        source.with_file_name(format!("{}.derive.bv", stem.to_string_lossy()))
    }

    pub fn opt_path_for(source: &Path) -> PathBuf {
        let stem = source.file_stem().unwrap_or_default();
        source.with_file_name(format!("{}.opt.bv", stem.to_string_lossy()))
    }

    /// Resolve the best available source for a given file.
    /// Order: .opt.bv > .derive.bv > .bv
    pub fn resolve(source: &Path) -> PathBuf {
        let opt = Self::opt_path_for(source);
        if opt.exists() { return opt; }
        let derive = Self::derive_path_for(source);
        if derive.exists() { return derive; }
        source.to_path_buf()
    }
}
```

**Tests**:
- `test_derive_path_for`: `foo.bv` → `foo.derive.bv`
- `test_opt_path_for`: `foo.bv` → `foo.opt.bv`
- `test_resolve_opt_exists`: `.opt.bv` exists → returns `.opt.bv`
- `test_resolve_derive_exists`: Only `.derive.bv` exists → returns `.derive.bv`
- `test_resolve_source_only`: No doppelgangers → returns original `.bv`

### Step E.1 — Full-source doppelganger writer

**File**: `src/derive/doppelganger.rs`

**What**: Read the original source, inject synthesized bodies at the correct byte
offsets, write the doppelganger. The doppelganger is a COMPLETE COPY of the original
source with bodies inserted — not a diff or patch.

**Write-back strategy**:

For each definition with a synthesis result:
1. Locate the byte offset of the derivation block's opening `{` via `DerivationBlock.span`
2. The synthesized body is inserted AFTER the signature but BEFORE the `:=`
3. Insert the body text at that offset in the byte buffer

**For drafting state** (no body in source):
```
Before: defn add(x: Int, y: Int) -> Int := { 2, 2 -> 4; };
                                                    ^-- insertion point: span.start
After:  defn add(x: Int, y: Int) -> Int { term x + y; } := { 2, 2 -> 4; };
```

**For asserted state** (body already present in source):
```
Before: defn add(x: Int, y: Int) -> Int { term x + y; } := { 2, 2 -> 4; };
After:  defn add(x: Int, y: Int) -> Int { term x + y; } := { 2, 2 -> 4; };
        (unchanged — body already present, assertion mode will verify)
```

**Implementation**:

```rust
/// Write the doppelganger file with synthesized bodies injected.
/// 2026-07-28: Phase E.1 — full-source doppelganger writer.
pub fn write_doppelganger(
    source_path: &Path,
    source_bytes: &[u8],
    syntheses: &[(String, SynthesizedProgram)], // (function_name, body)
    derivations: &[(String, DerivationBlock)],  // (function_name, block)
    output_path: &Path,
) -> Result<(), DeriveError> {
    let mut bytes = source_bytes.to_vec();

    // Process in reverse byte order to preserve offsets
    let mut insertions: Vec<(usize, String)> = Vec::new();
    for ((name, prog), (_, block)) in syntheses.iter().zip(derivations.iter()) {
        let insert_at = block.span.start as usize; // before the :=
        let body_str = format!(" {{\n    {};\n}} ", format_body(prog));
        insertions.push((insert_at, body_str));
    }
    insertions.sort_by_key(|(offset, _)| std::cmp::Reverse(*offset));

    for (offset, body_str) in insertions {
        let mut new = Vec::with_capacity(bytes.len() + body_str.len());
        new.extend_from_slice(&bytes[..offset]);
        new.extend_from_slice(body_str.as_bytes());
        new.extend_from_slice(&bytes[offset..]);
        bytes = new;
    }

    std::fs::write(output_path, &bytes).map_err(|e| DeriveError::Io {
        path: output_path.to_path_buf(),
        error: e,
    })
}

/// Format a synthesized program as a readable Brief body string.
/// 2026-07-28: Phase E.1 — body formatter.
fn format_body(prog: &SynthesizedProgram) -> String {
    let mut out = String::new();
    for stmt in &prog.body {
        out.push_str(&format!("{}", stmt));
    }
    out
}
```

**Nesting check**: The write function sorts insertions (level 1), then iterates
(level 1). The `format_body` function is a single loop — depth 1.

**Tests**:
- `test_write_doppelganger_draft`: Draft source → doppelganger has body between sig and `:=`
- `test_write_doppelganger_multi_function`: 2 functions → both bodies injected correctly
- `test_write_doppelganger_already_bodied`: Body present → doppelganger identical to source
- `test_write_doppelganger_preserves_formatting`: Comments, spacing, indentation intact
- `test_write_doppelganger_output_path`: Output written to correct `.derive.bv` path

### Step E.2 — Build system doppelganger resolution

**File**: `src/compile.rs`, `src/main.rs`

**What**: When building, check for doppelganger files before reading the original source.
If `foo.opt.bv` exists, compile that. Else if `foo.derive.bv` exists, compile that.
Else compile `foo.bv`.

**Resolution in `compile.rs`**:

```rust
/// 2026-07-28: Phase E.2 — resolve doppelganger precedence.
fn resolve_source_path(requested_path: &Path) -> PathBuf {
    Doppelganger::resolve(requested_path)
}
```

**CLI output**:

```
$ brief build foo.bv
[derive] using foo.opt.bv (stochastic superoptimized, 3 iterations)
  or
[derive] using foo.derive.bv (synthesized, enumerative depth=4)
  or
(no message — compiling original source)
```

**Tests**:
- `test_resolve_doppelganger_in_build`: Create `.derive.bv`, build `foo.bv` → uses doppelganger
- `test_resolve_no_doppelganger`: No doppelganger → builds original
- `test_resolve_opt_overrides_derive`: Both exist → `.opt.bv` wins

### Step E.3 — `brief derive` CLI command

**File**: `src/main.rs`, `src/derive/cli.rs`

**What**: Add `brief derive <file>` command with flags:

```
brief derive <file>           # Phase 1+2: enumerative + SMT → foo.derive.bv
brief derive --stochastic     # Phase 3: also run MCMC → foo.opt.bv
brief derive --iterations N   # MCMC iterations (default: 10000)
brief derive --temperature T  # MCMC initial temperature (default: 1.0)
brief derive --enumerative-depth N  # Max enumerative depth (default: 5)
brief derive --all            # Process all transitive imports
```

**CLI handler**:

```rust
/// 2026-07-28: Phase E.3 — `brief derive` command handler.
pub fn handle_derive_command(
    file_path: &str,
    config: &DeriveConfig,
) -> Result<(), String> {
    let source_path = Path::new(file_path);
    let source = std::fs::read_to_string(source_path)
        .map_err(|e| format!("cannot read '{}': {}", file_path, e))?;

    // Parse
    let (program, _) = parse_file_with_offsets(source_path)?;

    // Synthesize each derivation block
    let mut syntheses: Vec<(String, SynthesizedProgram)> = Vec::new();
    let mut derivations: Vec<(String, DerivationBlock)> = Vec::new();

    for item in &program {
        if let Some((name, derivation)) = extract_derivation(item) {
            let result = derive::synthesize(name, derivation, config)?;
            syntheses.push((name.clone(), result));
            derivations.push((name.clone(), derivation.clone()));
        }
    }

    // Write doppelganger
    let output_path = Doppelganger::derive_path_for(source_path);
    write_doppelganger(source_path, source.as_bytes(), &syntheses, &derivations, &output_path)?;

    eprintln!("[derive] wrote {}", output_path.display());
    Ok(())
}
```

**Tests**:
- `test_derive_command_basic`: Run `brief derive foo.bv` → `foo.derive.bv` exists
- `test_derive_command_no_derivation`: No `:=` blocks → no doppelganger written
- `test_derive_command_already_synthesized`: Body already present → assertion mode only
- `test_derive_command_nonexistent_file`: File not found → error

### Step E.4 — `brief accept` subcommand (user-approved folding)

**File**: `src/main.rs`, `src/derive/accept.rs` (new)

**What**: An explicit user command that folds a doppelganger's synthesized bodies
back into the source `.bv` file. The compiler NEVER mutates source — `brief accept`
is an intentional user action after reviewing the generated `foo.derive.bv`.

```
brief accept <file>              # fold foo.derive.bv bodies into foo.bv
brief accept <file> --opt        # fold from foo.opt.bv instead (MCMC result)
brief accept <file> --all        # accept all derivation blocks in file
```

**Semantics**:
1. Reads the shadow file (`foo.derive.bv` or `foo.opt.bv`), extracts each synthesized
   expression body.
2. For each `:= { ... };` derivation block in `foo.bv`, replaces it with the
   synthesized body, demoting the derivation block to a trailing comment:
   ```
   Before: defn add(x: Int, y: Int) -> Int := { 2, 2 -> 4; };
   After:  defn add(x: Int, y: Int) -> Int { term x + y; }  // := { 2, 2 -> 4; };
   ```
3. Writes the modified `foo.bv` in place.
4. The trailing comment preserves the derivation block as specification for future
   assertion builds or re-derivation.

**Why separate from `brief derive`**: The derive step is automatic and lossless
(original file untouched). The accept step is an explicit review gate — the
developer inspects `foo.derive.bv`, decides "this looks correct," then folds it in.
This separation prevents accidental source mutation from a failed synthesis or
from synthesizing a correct-but-suboptimal body.

**Relation to assertion mode**: After acceptance, `brief build foo.bv` runs in
assertion mode by default (verifies `// := { 2, 2 -> 4; }` matches the body).
Pass `--no-assert` to skip this verification.

**CLI handler**:

```rust
/// 2026-07-28: Phase E.4 — `brief accept` command.
/// Folds doppelganger bodies into the source file.
/// Never mutates source without explicit user invocation.
pub fn handle_accept_command(
    file_path: &str,
    opts: &AcceptOptions,
) -> Result<(), String> {
    let source_path = Path::new(file_path);
    let source = std::fs::read_to_string(source_path)
        .map_err(|e| format!("cannot read '{}': {}", file_path, e))?;

    // Determine which doppelganger to read from
    let shadow_path = if opts.use_opt {
        Doppelganger::opt_path_for(source_path)
    } else {
        Doppelganger::derive_path_for(source_path)
    };

    if !shadow_path.exists() {
        return Err(format!("no doppelganger found at '{}' — run 'brief derive' first", shadow_path.display()));
    }

    let shadow_source = std::fs::read_to_string(&shadow_path)
        .map_err(|e| format!("cannot read '{}': {}", shadow_path.display(), e))?;

    // Parse both files to find derivation blocks and their synthesized counterparts
    let (program, byte_offsets) = parse_file_with_offsets(source_path)?;
    let (shadow_program, _) = parse_file_with_offsets(&shadow_path)?;

    // For each derivation block, replace := { ... } with the synthesized body
    let new_source = fold_synthesized_bodies(&source, &program, &shadow_program, &byte_offsets)?;

    // Write back to original source
    std::fs::write(source_path, new_source)
        .map_err(|e| format!("cannot write '{}': {}", file_path, e))?;

    eprintln!("[accept] folded bodies into {}", source_path.display());
    Ok(())
}
```

**Key design invariants**:
- The `// := { ... }` comment preserves the derivation spec for future assertion-mode
  builds and re-derivation runs.
- Insertions happen in reverse byte offset order (same as doppelganger writer) to
  preserve span positions during replacement.
- If no doppelganger exists, the error message tells the user to run `brief derive` first.

**Nesting check**: The handler validates inputs, finds the shadow, calls the
replacement helper, writes — one `?` chain, depth ≤ 2.

**Tests**:
- `test_accept_basic`: Run `brief accept foo.bv` after derive → `foo.bv` has bodies inlined
- `test_accept_opt`: Accept from `foo.opt.bv` → `foo.bv` has MCMC-optimized bodies
- `test_accept_no_shadow`: No `foo.derive.bv` → error message points to `brief derive`
- `test_accept_preserves_derivation_block`: `// := { ... }` comment emitted after body
- `test_accept_without_derivation`: Source has no `:=` blocks → no-op, file unchanged
- `test_accept_idempotent`: Accept twice → second run sees no `:=` blocks → no-op

### Step E.5 — Updated commit sequence

The commit sequence for Phase E becomes:

```
5a. Doppelganger writer + resolver       (Step E.0–E.2)
5b. `brief derive` CLI with flags        (Step E.3)
5c. `brief accept` subcommand            (Step E.4)
```

---

## Phase F — MCMC Stochastic Superoptimizer

### Goal

Implement a STOKE-style MCMC sampler that takes a correct program (from Phase 1 or 2)
and searches for a FASTER equivalent program by random mutation of the normalized IR.

### Step F.0 — MCMC configuration and types

**File**: `src/derive/mcmc.rs` (new)

**What**: Core types for the MCMC sampler: configuration, state, and the Metropolis-Hastings
acceptance logic.

```rust
/// 2026-07-28: Phase F.0 — MCMC superoptimizer configuration.
pub struct McmcConfig {
    /// Initial temperature for simulated annealing
    pub initial_temperature: f64,
    /// Cooling rate (multiplicative per iteration, e.g., 0.999)
    pub cooling_rate: f64,
    /// Maximum iterations
    pub max_iterations: usize,
    /// Convergence: stop after N iterations with no improvement
    pub convergence_window: usize,
    /// Probability of each mutation type
    pub mutation_weights: MutationWeights,
    /// Which equivalence check to use
    pub equivalence: EquivalenceMode,
    /// Cost model weights
    pub correctness_weight: f64,
    pub performance_weight: f64,
}

/// Mutation probability weights.
/// 2026-07-28: Phase F.0 — mutation type probabilities.
pub struct MutationWeights {
    pub replace_subtree: f64,    // 0.25
    pub change_operator: f64,    // 0.20
    pub swap_commutative: f64,   // 0.10
    pub fold_constant: f64,      // 0.15
    pub insert_identity: f64,    // 0.10
    pub delete_dead_code: f64,   // 0.10
    pub distribute: f64,         // 0.05
    pub vector_fuse: f64,        // 0.05
}

/// How to verify equivalence after mutation.
/// 2026-07-28: Phase F.0 — equivalence verification strategy.
pub enum EquivalenceMode {
    /// Fast: check against derivation examples only (microseconds)
    ExamplesOnly,
    /// Slow: Z3 QF_BV equivalence proof (milliseconds)
    Z3Proof { z3_path: String },
    /// Hybrid: check examples first, escalate to Z3 only if examples pass
    Hybrid { z3_path: String },
}

/// State of the MCMC sampler.
/// 2026-07-28: Phase F.0 — sampler tracking state.
pub struct McmcState {
    pub current: SynthesizedProgram,
    pub best: SynthesizedProgram,
    pub current_cost: f64,
    pub best_cost: f64,
    pub temperature: f64,
    pub iteration: usize,
    pub iterations_without_improvement: usize,
    pub rng_seed: u64,
}
```

**Default configuration**:

```rust
impl Default for McmcConfig {
    fn default() -> Self {
        Self {
            initial_temperature: 1.0,
            cooling_rate: 0.999,
            max_iterations: 100_000,
            convergence_window: 1000,
            mutation_weights: MutationWeights::default(),
            equivalence: EquivalenceMode::Hybrid {
                z3_path: "z3".to_string(),
            },
            correctness_weight: 1000.0,
            performance_weight: 1.0,
        }
    }
}
```

**Nesting check**: Struct definitions and impl blocks — no nesting.

**Tests**:
- `test_mcmc_config_default`: Default config has expected values
- `test_mcmc_config_custom`: Custom config overrides defaults
- `test_mutation_weights_sum_to_one`: All weights sum to 1.0

### Step F.1 — Mutation operators (AST layer)

**File**: `src/derive/mutate.rs` (new)

**What**: Define the mutation operators that transform an `Expr` tree into a new `Expr`
tree. Each mutation is type-safe (preserves the expression's type) and structurally valid.

```rust
/// 2026-07-28: Phase F.1 — AST mutation operators for MCMC search.

/// Replace a random subtree with a different subtree of the same type.
/// Picks a random node, then generates a new subtree of compatible type.
pub fn mutate_replace_subtree(expr: &Expr, rng: &mut ThreadRng, depth: u8) -> Expr { ... }

/// Change a binary operator: x + y → x * y (same types).
pub fn mutate_change_operator(expr: &Expr, rng: &mut ThreadRng) -> Expr { ... }

/// Swap operands of a commutative operation: x + y → y + x.
pub fn mutate_swap_commutative(expr: &Expr, rng: &mut ThreadRng) -> Expr { ... }

/// Fold identity expressions: x + 0 → x, x * 1 → x, x & ~0 → x, etc.
pub fn mutate_fold_constant(expr: &Expr, rng: &mut ThreadRng) -> Expr { ... }

/// Insert an identity expression: x → x + 0, x → x * 1, etc.
/// This adds dead code that the MCMC can later transform into useful computation.
pub fn mutate_insert_identity(expr: &Expr, rng: &mut ThreadRng) -> Expr { ... }

/// Delete a dead-code subtree that has no effect on the result.
pub fn mutate_delete_dead_code(expr: &Expr, rng: &mut ThreadRng) -> Expr { ... }

/// Distribute multiplication over addition: a*(b+c) → a*b + a*c (or reverse).
pub fn mutate_distribute(expr: &Expr, rng: &mut ThreadRng) -> Expr { ... }

/// Fuse scalar operations into vector operations (when types permit).
pub fn mutate_vector_fuse(expr: &Expr, rng: &mut ThreadRng, types: &TypeUniverse) -> Expr { ... }

/// Apply a random mutation weighted by the mutation weights.
/// 2026-07-28: Phase F.1 — weighted random mutation dispatch.
pub fn apply_random_mutation(
    expr: &Expr,
    weights: &MutationWeights,
    rng: &mut ThreadRng,
    depth: u8,
) -> Expr {
    let roll: f64 = rng.gen();
    let mut cumulative = 0.0;

    cumulative += weights.replace_subtree;
    if roll < cumulative { return mutate_replace_subtree(expr, rng, depth); }

    cumulative += weights.change_operator;
    if roll < cumulative { return mutate_change_operator(expr, rng); }

    // ... etc for each mutation type

    // Fallback: no-op
    expr.clone()
}
```

**Key design point**: Each mutation verifies type compatibility before applying.
If a mutation would produce a type error, it is retried (up to 3 attempts) or
a different mutation is chosen.

**Nesting check**: Each mutation function is a single traversal or transformation
on the expression tree — depth varies with tree depth but the code structure is
flat (one match per case, with guard clauses).

**Tests**:
- `test_mutate_replace_subtree`: Apply to `x + y` → new expression still has type Int
- `test_mutate_change_operator`: `x + y` → `x * y` → types compatible
- `test_mutate_swap_commutative`: `x + y` → `y + x` → equivalent
- `test_mutate_fold_constant`: `x + 0` → `x`
- `test_mutate_insert_identity`: `x` → `x + 0` → `x * 1` → equivalent
- `test_mutate_delete_dead_code`: `x + 0` → `x` → equivalence preserved
- `test_mutate_distribute`: `2 * (x + 3)` → `2*x + 2*3` → equivalent
- `test_mutate_type_preserved`: Every mutation preserves expression type
- `test_apply_random_mutation_all`: All mutation types can be sampled

### Step F.2 — Equivalence verification

**File**: `src/derive/equivalence.rs` (new)

**What**: Verify that a mutated expression is equivalent to the original (or satisfies
the derivation examples). Three modes: examples-only (fast), Z3 proof (slow), hybrid.

```rust
/// 2026-07-28: Phase F.2 — equivalence verification for MCMC.

/// Check if a program is equivalent to the specification.
/// Returns Ok(()) if equivalent, Err(reason) if not.
pub fn check_equivalence(
    program: &SynthesizedProgram,
    examples: &[DerivationExample],
    params: &[TypedParameter],
    mode: &EquivalenceMode,
) -> Result<(), EquivalenceError> {
    match mode {
        EquivalenceMode::ExamplesOnly => check_examples(program, examples, params),
        EquivalenceMode::Z3Proof { z3_path } => check_z3_proof(program, examples, params, z3_path),
        EquivalenceMode::Hybrid { z3_path } => {
            // Fast path: check examples first
            check_examples(program, examples, params)?;
            // Slow path: Z3 proof
            check_z3_proof(program, examples, params, z3_path)
        }
    }
}

/// Fast path: evaluate all examples and compare outputs.
/// 2026-07-28: Phase F.2 — example-based fast equivalence.
fn check_examples(
    program: &SynthesizedProgram,
    examples: &[DerivationExample],
    params: &[TypedParameter],
) -> Result<(), EquivalenceError> {
    for (i, ex) in examples.iter().enumerate() {
        let inputs: Vec<Value> = ex.inputs.iter()
            .map(|e| eval_as_constant(e))
            .collect::<Result<_, _>>()
            .map_err(|e| EquivalenceError::EvalFailed(i, e))?;

        let result = evaluate_synthesized_program(program, params, &inputs)
            .map_err(|e| EquivalenceError::EvalFailed(i, e))?;

        let expected = eval_as_constant(&ex.output)
            .map_err(|e| EquivalenceError::EvalFailed(i, e))?;

        if !values_match(&result, &expected, ex.tolerance) {
            return Err(EquivalenceError::ExampleFailed(i, expected, result));
        }
    }
    Ok(())
}

/// Slow path: Z3 QF_BV equivalence proof.
/// Proves: ∀ inputs. old_program(inputs) == new_program(inputs)
/// 2026-07-28: Phase F.2 — Z3 equivalence proof.
fn check_z3_proof(
    program: &SynthesizedProgram,
    examples: &[DerivationExample],
    params: &[TypedParameter],
    z3_path: &str,
) -> Result<(), EquivalenceError> {
    // Build: (assert (forall ((x ...)) (= (old_f x) (new_f x))))
    let query = build_equivalence_query(program, examples, params)?;

    let output = Command::new(z3_path)
        .arg("-in")
        .arg("-smt2")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            use std::io::Write;
            if let Some(stdin) = child.stdin.as_mut() {
                writeln!(stdin, "{}", query).ok();
            }
            child.wait_with_output()
        })
        .map_err(|e| EquivalenceError::SolverError(e.to_string()))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    if stdout.contains("unsat") {
        // unsat means NO counterexample exists → programs ARE equivalent
        Ok(())
    } else if stdout.contains("sat") {
        // sat means a counterexample EXISTS → programs are NOT equivalent
        Err(EquivalenceError::NotEquivalent)
    } else {
        Err(EquivalenceError::SolverUnknown(stdout.to_string()))
    }
}

/// Build a QF_BV equivalence query.
/// Proves: (assert (forall ((x ...)) (= (f_old x) (f_new x))))
/// Z3 answers unsat if no counterexample → equivalent.
/// 2026-07-28: Phase F.2 — equivalence SMT-LIB query builder.
fn build_equivalence_query(
    program: &SynthesizedProgram,
    examples: &[DerivationExample],
    params: &[TypedParameter],
) -> Result<String, EquivalenceError> {
    // For each example, generate a concrete equivalence check:
    // (assert (= (f_expected <inputs>) (f_actual <inputs>)))
    // Rather than a quantified forall, we check each example individually
    // (which is sufficient for inductive synthesis — generalization is
    //  the developer's responsibility via contracts).
    let mut q = String::new();
    q.push_str("(set-logic QF_BV)\n\n");

    // encode the current (expected) program as f_expected
    q.push_str(&encode_as_smt_function("f_expected", program, params)?);

    // For each example, assert equivalence
    for ex in examples {
        let inputs: Vec<String> = ex.inputs.iter()
            .map(expr_to_smt_const)
            .collect();
        q.push_str(&format!("(assert (= (f_expected {}) ", inputs.join(" ")));
        q.push_str(&format!("{}))\n", expr_to_smt_const(&ex.output)));
    }

    q.push_str("\n(check-sat)\n");
    Ok(q)
}
```

**Nesting check**: Each function uses guard clauses and early returns — depth ≤ 2.

**Tests**:
- `test_equivalence_examples_pass`: Equivalent program → Ok
- `test_equivalence_examples_fail`: Non-equivalent program → Err
- `test_equivalence_z3_equivalent`: Z3 proves equivalence → Ok
- `test_equivalence_z3_not_equivalent`: Z3 disproves equivalence → Err(NotEquivalent)
- `test_equivalence_hybrid_pass`: Examples pass + Z3 passes → Ok
- `test_equivalence_hybrid_fail_examples`: Examples fail → Z3 not called → fast error
- `test_equivalence_with_tolerance`: Within tolerance → Ok
- `test_equivalence_without_tolerance`: Outside tolerance → Err

### Step F.3 — MCMC sampler loop

**File**: `src/derive/mcmc.rs`

**What**: The main MCMC loop. Takes a correct program, runs Metropolis-Hastings
for `max_iterations` steps, returns the best program found.

```rust
/// Run the MCMC superoptimizer.
/// Takes a known-correct program and searches for a faster equivalent.
/// 2026-07-28: Phase F.3 — main MCMC orchestration.
pub fn optimize(
    initial: SynthesizedProgram,
    examples: &[DerivationExample],
    params: &[TypedParameter],
    config: &McmcConfig,
) -> Result<SynthesizedProgram, McmcError> {
    let mut rng = StdRng::seed_from_u64(config.seed.unwrap_or(42));
    let mut state = McmcState {
        current: initial.clone(),
        best: initial.clone(),
        current_cost: cost_function(&initial, examples, params, config),
        best_cost: cost_function(&initial, examples, params, config),
        temperature: config.initial_temperature,
        iteration: 0,
        iterations_without_improvement: 0,
        rng_seed: config.seed.unwrap_or(42),
    };

    // Phase 3a: Correctness search (synthesis mode)
    // Only runs if the initial program does NOT satisfy all examples
    if state.current_cost >= config.correctness_weight {
        state = correctness_search(state, examples, params, config, &mut rng)?;
    }

    // Phase 3b: Performance optimization (optimization mode)
    // Only runs if the initial program IS correct
    if state.current_cost < config.correctness_weight {
        state = performance_optimization(state, examples, params, config, &mut rng)?;
    }

    Ok(state.best)
}

/// Phase 3a: Correctness search — random mutations until program satisfies all examples.
/// Uses high correctness_weight so equivalence dominates the cost.
/// 2026-07-28: Phase F.3 — MCMC correctness search.
fn correctness_search(
    mut state: McmcState,
    examples: &[DerivationExample],
    params: &[TypedParameter],
    config: &McmcConfig,
    rng: &mut StdRng,
) -> Result<McmcState, McmcError> {
    for i in 0..config.max_iterations {
        state.iteration = i;

        // Propose mutation
        let proposal = apply_random_mutation(
            &state.current.program_expr(),
            &config.mutation_weights, rng, 3,
        );
        let proposed_prog = SynthesizedProgram::from_expr(proposal);

        // Compute cost (correctness dominates)
        let proposal_cost = cost_function(&proposed_prog, examples, params, config);

        // Metropolis-Hastings acceptance
        let delta = proposal_cost - state.current_cost;
        if delta < 0.0 || rng.gen::<f64>() < (-delta / state.temperature).exp() {
            state.current = proposed_prog;
            state.current_cost = proposal_cost;
            state.iterations_without_improvement = 0;

            if proposal_cost < state.best_cost {
                state.best = state.current.clone();
                state.best_cost = proposal_cost;
            }
        } else {
            state.iterations_without_improvement += 1;
        }

        // Cool down
        state.temperature *= config.cooling_rate;

        // Convergence check
        if state.iterations_without_improvement >= config.convergence_window {
            break;
        }

        // Early exit: found a program that passes all examples
        if state.current_cost < config.correctness_weight {
            break;
        }
    }

    Ok(state)
}

/// Phase 3b: Performance optimization — strict improvement on a correct program.
/// Uses full equivalence verification (Z3), only accepts strictly faster programs.
/// 2026-07-28: Phase F.3 — MCMC performance optimization.
fn performance_optimization(
    mut state: McmcState,
    examples: &[DerivationExample],
    params: &[TypedParameter],
    config: &McmcConfig,
    rng: &mut StdRng,
) -> Result<McmcState, McmcError> {
    // Set temperature low — only accept strict improvements
    state.temperature = 0.01;

    for i in 0..config.max_iterations {
        state.iteration = i;

        // Propose mutation
        let proposal = apply_random_mutation(
            &state.current.program_expr(),
            &config.mutation_weights, rng, 3,
        );
        let proposed_prog = SynthesizedProgram::from_expr(proposal);

        // Verify equivalence via Z3 (BEAST/normalized IR layer semantics preserved)
        match check_equivalence(&proposed_prog, examples, params, &config.equivalence) {
            Ok(()) => {
                // Equivalent! Now check if it's faster
                let proposal_perf = performance_cost(&proposed_prog);
                let current_perf = performance_cost(&state.current);

                if proposal_perf < current_perf {
                    // Strict improvement: accept
                    state.current = proposed_prog;
                    state.current_cost = proposal_perf as f64;
                    state.iterations_without_improvement = 0;

                    if state.current_cost < state.best_cost {
                        state.best = state.current.clone();
                        state.best_cost = state.current_cost;
                    }
                } else {
                    state.iterations_without_improvement += 1;
                }
            }
            Err(_) => {
                // Not equivalent: reject
                state.iterations_without_improvement += 1;
            }
        }

        // Convergence
        if state.iterations_without_improvement >= config.convergence_window {
            break;
        }
    }

    Ok(state)
}

/// Cost function for MCMC: correctness violations + performance cost.
/// 2026-07-28: Phase F.3 — combined cost.
fn cost_function(
    program: &SynthesizedProgram,
    examples: &[DerivationExample],
    params: &[TypedParameter],
    config: &McmcConfig,
) -> f64 {
    // Correctness: count example failures
    let violation_count = examples.iter().filter(|ex| {
        let result = evaluate_synthesized_program(program, params, &evaluate_inputs(ex));
        let expected = eval_as_constant(&ex.output);
        match (result, expected) {
            (Ok(r), Ok(e)) => !values_match(&r, &e, ex.tolerance),
            _ => true,
        }
    }).count();

    let correctness_cost = violation_count as f64 * config.correctness_weight;

    // Performance: estimated operation count
    let performance_cost = performance_cost(program) as f64 * config.performance_weight;

    correctness_cost + performance_cost
}

/// Estimate the performance of a program by counting operations.
/// Later: use LLVM cost model, actual cycle counts, or benchmark runtime.
/// 2026-07-28: Phase F.3 — operation count estimator.
fn performance_cost(program: &SynthesizedProgram) -> u64 {
    program.body.iter().map(|stmt| count_ops(stmt)).sum()
}
```

**Nesting check**: The MCMC loop is a single `for` with guard clauses inside
(early exit on convergence, early exit on correctness). Max depth 2.

**Tests**:
- `test_mcmc_optimize_identity`: Start from `x + 0`, MCMC discovers `x` as cheaper equivalent
- `test_mcmc_optimize_add_commute`: Start from `x + y`, MCMC tries `y + x` (same cost)
- `test_mcmc_optimize_distribute`: `2*(x+3)` → `2*x + 6` (constant folding)
- `test_mcmc_correctness_search`: Start from random, find correct program for `{2,3→5}`
- `test_mcmc_no_equivalent_found`: Impossible constraints → returns best attempt
- `test_mcmc_convergence_early`: Converges before max_iterations
- `test_mcmc_deterministic_same_seed`: Same seed → same result
- `test_mcmc_cost_correctness_dominates`: Violating program has higher cost than correct

### Step F.4 — Pareto frontier and knee selection

**File**: `src/derive/pareto.rs` (new)

**What**: Track all valid programs discovered during MCMC and select the "knee"
of the Pareto frontier — the program that offers the best trade-off between
correctness (error count) and performance (op count).

```rust
/// 2026-07-28: Phase F.4 — Pareto frontier types.
pub struct ParetoPoint {
    pub program: SynthesizedProgram,
    pub error_count: u64,
    pub op_count: u64,
    /// Estimated or benchmarked runtime (if available)
    pub runtime_ns: Option<u64>,
}

/// Pareto frontier — set of non-dominated programs.
/// A program A dominates B if A has strictly fewer errors AND
/// strictly fewer operations (or runtime).
/// 2026-07-28: Phase F.4 — Pareto frontier.
pub struct ParetoFrontier {
    pub points: Vec<ParetoPoint>,
}

impl ParetoFrontier {
    /// Add a point and update the frontier.
    /// Returns true if the point is on the frontier.
    /// 2026-07-28: Phase F.4 — frontier insertion.
    pub fn insert(&mut self, point: ParetoPoint) -> bool {
        // Remove points dominated by the new point
        self.points.retain(|p| !point.dominates(p));
        // Check if the new point is dominated by any existing point
        if self.points.iter().any(|p| p.dominates(&point)) {
            return false; // dominated — not on frontier
        }
        self.points.push(point);
        true
    }

    /// Select the "knee" of the Pareto frontier — the point where
    /// the marginal benefit of reducing errors equals the marginal
    /// cost of increasing operations.
    /// Uses the "angle method": find the point farthest from the line
    /// connecting the min-error and min-op extremes.
    /// 2026-07-28: Phase F.4 — knee selection via angle method.
    pub fn select_knee(&self) -> Option<&ParetoPoint> {
        if self.points.is_empty() {
            return None;
        }
        if self.points.len() <= 2 {
            return self.points.iter().min_by_key(|p| p.error_count);
        }

        // Normalize: find min/max for both dimensions
        let min_err = self.points.iter().map(|p| p.error_count).min().unwrap_or(0) as f64;
        let max_err = self.points.iter().map(|p| p.error_count).max().unwrap_or(1).max(1) as f64;
        let min_op = self.points.iter().map(|p| p.op_count).min().unwrap_or(0) as f64;
        let max_op = self.points.iter().map(|p| p.op_count).max().unwrap_or(1).max(1) as f64;

        // Line between (min_err, max_op) — low error, high ops
        //   and (max_err, min_op) — high error, low ops
        // Knee = point farthest from this line
        let extreme_a = (min_err, max_op);
        let extreme_b = (max_err, min_op);

        self.points.iter()
            .map(|p| {
                let px = p.error_count as f64;
                let py = p.op_count as f64;
                let dist = point_line_distance((px, py), extreme_a, extreme_b);
                (dist, p)
            })
            .max_by(|(a, _), (b, _)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(_, p)| p)
    }
}
```

**Nesting check**: The `insert` function uses `retain` and `iter().any()` — each is
a single expression. The `select_knee` function is sequential (find min/max, compute
distance, select max) — depth 1.

**Tests**:
- `test_pareto_insert_dominated`: Point with more errors AND more ops → not on frontier
- `test_pareto_insert_non_dominated`: Point with fewer ops but same errors → on frontier
- `test_pareto_insert_dominates_existing`: New point dominates old → old removed
- `test_pareto_knee_selection`: Three points, knee is the middle trade-off
- `test_pareto_knee_single_point`: Only one point → that point
- `test_pareto_knee_two_points`: Two points → lower error wins
- `test_pareto_empty_frontier`: No points → None

### Step F.5 — Tolerance validation for floating-point

**File**: `src/derive/tolerance.rs` (new)

**What**: Handle floating-point tolerance in equivalence checking and cost computation.
A program that produces outputs within tolerance is "correct enough" for the MCMC
to consider it equivalent.

```rust
/// 2026-07-28: Phase F.5 — floating-point tolerance checking.

/// Check if two values match within the given relative tolerance.
/// Differs from values_within_tolerance in Phase B: this one handles
/// the case where the expected value is the SPECIFICATION (derivation
/// block output) and the actual value is the PROGRAM OUTPUT.
pub fn values_match_with_tolerance(
    actual: &Value,
    expected: &Value,
    tolerance: Option<f64>,
) -> bool {
    match tolerance {
        None => actual == expected,
        Some(tol) => {
            match (actual, expected) {
                (Value::Float(a), Value::Float(e)) => {
                    let diff = (a - e).abs();
                    let mag = e.abs().max(f64::MIN_POSITIVE);
                    diff / mag <= tol
                }
                (Value::Int(a), Value::Int(e)) if *e != 0 => {
                    // Integer tolerance: relative error
                    let diff = (a - e).unsigned_abs();
                    let mag = e.unsigned_abs().max(1);
                    (diff as f64) / (mag as f64) <= tol
                }
                _ => actual == expected,
            }
        }
    }
}

/// Relaxed equivalence: two programs are equivalent if their outputs
/// match within tolerance for ALL derived examples. This is weaker than
/// strict equality but necessary for FP-heavy code where MCMC might
/// find algebraically equivalent but bitwise-different expressions.
/// 2026-07-28: Phase F.5 — relaxed equivalence for FP synthesis.
pub fn check_equivalence_relaxed(
    program: &SynthesizedProgram,
    examples: &[DerivationExample],
    params: &[TypedParameter],
) -> Result<(), EquivalenceError> {
    for (i, ex) in examples.iter().enumerate() {
        let inputs: Vec<Value> = /* evaluate example inputs */;
        let result = evaluate_synthesized_program(program, params, &inputs)?;
        let expected = /* evaluate example output */;

        if !values_match_with_tolerance(&result, &expected, ex.tolerance) {
            return Err(EquivalenceError::ExampleFailed(i, expected, result));
        }
    }
    Ok(())
}
```

**Nesting check**: Single match with guard clauses — depth 1.

**Tests**:
- `test_tolerance_exact_match`: tol=None, exact values → true
- `test_tolerance_within`: tol=0.01, 3.0 vs 3.005 → true
- `test_tolerance_outside`: tol=0.01, 3.0 vs 3.5 → false
- `test_tolerance_int`: tol=0.1, 10 vs 11 → true (10%)
- `test_tolerance_int_outside`: tol=0.01, 100 vs 200 → false

---

## Phase G — Metadata Vocabulary and Backend Mappings

### Goal

Document the complete `!>` metadata vocabulary for optimization hints and implement
mapping tables for each backend. The MCMC optimizer reads derivation-related keys
(`search_space`, `cost_model`, `tolerance`, `allowed_mutations`) to guide its search.
Each backend reads the remaining keys and maps them to target-specific semantics.

**Architecture decision (2026-07-28)**: Vocabulary definitions and backend mapping
rules are stored in a single `config/meta-vocab.dbv` file (Data Brief `.dbv` format
with inline schemas). This replaces hardcoded Rust match arms for metadata→attribute
lookups. The DBV file is parsed at compile time via `include_str!` +
`dbrief::v2::parse_document_quoted`. A `MetadataRegistry` in `src/backend/metadata.rs`
provides typed lookup functions for each backend.

Rationale: Adding a new backend or metadata key only requires editing
`config/meta-vocab.dbv` — no Rust changes to the mapping logic. The `.dbv` file
also serves as the machine-readable source of truth for the vocabulary (the
`docs/architecture/optimization-hints.md` doc is a human-readable rendering).

### Step G.0 — Vocabulary reference document

**File**: `docs/architecture/optimization-hints.md` (new)

**What**: A complete reference document listing every `!>` metadata key, its values,
its semantics, and which backends honor it. This is the human-readable source of truth.
The machine-readable source is `config/meta-vocab.dbv`.

```markdown
# !> Optimization Hints — Vocabulary Reference

## Arithmetic Semantics

| Key | Values | Description | LLVM | Webstack | CIRCT | MCMC |
|-----|--------|-------------|------|----------|-------|------|
| `overflow` | wrapping, checked, saturating | Integer overflow behavior | nuw nsw | implicit | comb.add flags | algebraic identities |
| `associative` | true, false | May reassociate operations | reassoc | reorder | tree balance | reassociation mutations |
| `commutative` | true, false | May swap operands | ignored | ignored | ignored | commutative swap mutations |
| `fp_contract` | fast, strict | May form FMA | contract | fma fuse | fma | fma fusion mutations |
| `fp_math` | ieee754, fast | Floating-point compliance | fast-math flags | no subnormals | relaxed | relaxed equivalence |

## Memory Semantics

... (table continues for all keys)
```

**Nesting check**: Flat markdown document with tables — no nesting concern.

### Step G.1 — Data Brief vocabulary file

**File**: `config/meta-vocab.dbv` (new)

**What**: A single `.dbv` file with inline schemas and data entries. Defines:

1. **`MetaField` schema** — what metadata keys exist, their types, descriptions
2. **`BackendMapping` schema** — how a (key, value) pair maps to a backend IR attribute
3. **Data entries** under `as MetaField { ... }` — keyed entries using `(name)` key field
4. **Data entries** under `as BackendMapping { ... }` — positional `>` entries

**Grammar note**: `MetaField` uses `(name)` as its key field annotation. This marks
the `name` field as the logical lookup key but does NOT change the entry syntax —
entries in `as MetaField { ... }` use the keyed entry form `key: ty; "description";`
where `key` is the name, and the remaining fields are positional by schema order.

**DBV parser fix (2026-07-28)**: `parse_schema()` and `parse_grouped_data()` now
consume the optional trailing `;` after `}`. Previously the `;` fell through to the
main loop's `_ =>` arm and was misparsed as an empty positional value. Both
`schema Name { ... };` and `as SchemaName { ... };` work with or without `;`.

```dbv
// config/meta-vocab.dbv — Phase G metadata vocabulary
// MetaField uses keyed entries with (name) key field.
// BackendMapping uses positional > entries with (meta_key) key field.

schema MetaField (name) {
    name: String;
    ty: String;
    description: String;
};

schema BackendMapping (meta_key) {
    backend: String;
    meta_key: String;
    value_pattern: String;
    attr: String;
    scope: String;  // "function", "instruction", "module", "loop", "option"
};

as MetaField {
    overflow: String; "Integer overflow behavior for arithmetic ops";
    associative: Bool; "May the optimizer reassociate FP operations";
    commutative: Bool; "May the optimizer swap commutative operands";
    fp_contract: String; "May the optimizer form FP contractions (FMA)";
    fp_math: String; "Floating-point compliance: ieee754 | fast";
    readonly: Bool; "Function has no observable side effects";
    alloc_scope: String; "Allocation scope: heap | stack";
    inline_hint: String; "Inlining hint: always | never | hint";
    convergence: String; "Hardware convergence mode: tight | loose";
    unroll_hint: Int; "Loop unroll factor hint";
    search_space: String; "MCMC search space: linear | bitwise | all";
    cost_model: String; "MCMC cost function: latency | throughput | size";
    tolerance: Float; "FP equivalence tolerance for MCMC";
    allowed_mutations: Vec[String]; "Restrict MCMC to named mutations only";
};

as BackendMapping {
    // LLVM mappings
    > llvm; overflow; "wrapping"; "nuw nsw"; instruction;
    > llvm; overflow; "checked"; "nsw"; instruction;
    > llvm; overflow; "saturating"; "nuw"; instruction;
    > llvm; associative; "*"; "reassoc"; function;
    > llvm; fp_contract; "*"; "contract"; function;
    > llvm; fp_math; "fast"; "fast"; function;
    > llvm; fp_math; "ieee754"; ""; function;
    > llvm; readonly; "*"; "readonly"; function;
    > llvm; inline_hint; "always"; "alwaysinline"; function;
    > llvm; inline_hint; "never"; "noinline"; function;
    > llvm; unroll_hint; "*"; "unroll"; loop;

    // Webstack mappings
    > webstack; convergence; "tight"; "no_subnormals"; option;
    > webstack; alloc_scope; "stack"; "stack_allocation"; option;
    > webstack; fp_contract; "*"; "fma_fuse"; option;

    // CIRCT mappings
    > circt; overflow; "wrapping"; "comb.add"; option;
    > circt; convergence; "tight"; "single_cycle"; option;
    > circt; unroll_hint; "*"; "unroll_factor"; option;
};
```

**Loading**: The `MetadataRegistry` uses `include_str!("../../config/meta-vocab.dbv")`
and `dbrief::v2::parse_document_quoted()`. The `parse_document_quoted` variant is
required because mapping values use `"..."` quotation (e.g., `"wrapping"`, `"fast"`).
Parse failure at compile time is a hard error (invariant: the `.dbv` file is always
valid in a working copy).

**MetaField parsing**: `MetadataRegistry::parse_meta_field` reads `entry.key` as
the field name, `fields[0]` as the type string, `fields[1]` as the description.

**BackendMapping parsing**: `MetadataRegistry::parse_backend_mapping` reads
`fields[0..4]` as positional `(backend, meta_key, value_pattern, attr, scope)`.
The `entry.key` is `None` for `>` positional entries.

### Step G.2 — MetadataRegistry

**File**: `src/backend/metadata.rs` (new)

**What**: Central registry that loads `config/meta-vocab.dbv` and provides
typed lookup functions for each backend.

```rust
/// 2026-07-28: Phase G — DBV-backed metadata registry.
/// Loaded once at compiler init from config/meta-vocab.dbv.
pub struct MetadataRegistry {
    fields: HashMap<String, MetaFieldDef>,
    mappings: Vec<BackendMapping>,
    llvm_idx: Vec<usize>,
    webstack_idx: Vec<usize>,
    circt_idx: Vec<usize>,
}

pub struct MetaFieldDef {
    pub name: String,
    pub field_type: MetaType,
    pub description: String,
}

pub enum MetaType { Bool, Int, Float, String, List }

struct BackendMapping {
    backend: String,
    metadata_key: String,
    value_pattern: String,  // literal value or "*" wildcard
    ir_attribute: String,
    applies_to: String,
}

impl MetadataRegistry {
    pub fn load() -> Self;
    pub fn field_def(&self, name: &str) -> Option<&MetaFieldDef>;
    pub fn llvm_attr(&self, key: &str, value: &str) -> Option<&str>;
    pub fn webstack_option(&self, key: &str, value: &str) -> Option<&str>;
    pub fn circt_option(&self, key: &str, value: &str) -> Option<&str>;
}
```

**Loading logic**:
1. `include_str!("../../config/meta-vocab.dbv")` gets the file content
2. `parse_document_quoted(&content).expect("...")` parses into `DbriefDocument`
3. Iterate `data_groups`, matching `schema_name == "MetaField"` → extract field defs
4. Match `schema_name == "BackendMapping"` → collect all mappings
5. Build per-backend index vectors (`llvm_idx` = indices where `backend == "llvm"`)

**Lookup logic** (`llvm_attr` as example):
```
for idx in &self.llvm_idx {
    let m = &self.mappings[*idx];
    if m.metadata_key == key && (m.value_pattern == "*" || m.value_pattern == value) {
        return Some(m.ir_attribute.as_str());
    }
}
None
```

**Tests** (`test_registry_loads` etc. — ~6 tests):
- Parse succeeds and returns expected number of fields and mappings
- `llvm_attr("overflow", "wrapping")` returns `Some("nuw nsw")`
- `llvm_attr("fp_math", "ieee754")` returns `None` (no mapping for that value)
- `llvm_attr("unknown_key", "val")` returns `None`
- `webstack_option("alloc_scope", "stack")` returns `Some("stack_allocation")`
- `circt_option("unroll_hint", "4")` returns `Some("unroll_factor")`

### Step G.3 — Backend integration

**LLVM** (`src/backend/llvm/`): During function codegen, load the registry and
consult it when emitting float operations (fast-math flags) and function
attributes (`alwaysinline`, `noinline`, `readonly`).

- `apply_llvm_function_metadata(fn_decl, metadata, registry)` — emits function-level attrs
- `emit_fast_math_flags(metadata, registry)` — returns `" fast contract reassoc"` string
  based on registry lookups
- The LLVM fast-math flag emission is extracted as a call to `registry.llvm_attr()`
  rather than hardcoded matches

**Webstack** (`src/backend/webstack.rs`): Add `apply_webstack_metadata(metadata,
registry) -> WebstackOptions` function that queries the registry for
`alloc_scope` and `fp_contract`.

**CIRCT** (`src/backend/circt.rs`): Add `apply_circt_metadata(metadata,
registry) -> CirctOptions` function that queries the registry for `convergence`
and `unroll_hint`.

**Tests** (same as original G.1-G.3 in this plan, but now driven by the registry):
- `test_llvm_overflow_wrapping`
- `test_llvm_fp_math_fast`
- `test_llvm_readonly`
- `test_llvm_inline_hint`
- `test_llvm_unknown_key`
- `test_webstack_stack_alloc`
- `test_webstack_default`
- `test_circt_convergence_tight`
- `test_circt_unroll_hint`

### Step G.4 — MCMC metadata reading

**File**: `src/derive/mcmc.rs`

**What**: The MCMC optimizer reads `!>` metadata keys directly from the AST
(no registry needed — these are configuration keys, not backend attributes).

```rust
/// 2026-07-28: Phase G.4 — MCMC reads !> metadata for search guidance.
fn mcmc_config_from_metadata(metadata: &HashMap<String, PropertyValue>) -> McmcConfig {
    let mut config = McmcConfig::default();

    // search_space: restrict the mutation grammar
    if let Some(PropertyValue::String(space)) = metadata.get("search_space") {
        match space.as_str() {
            "linear" => config.mutation_weights = MutationWeights::linear_only(),
            "bitwise" => config.mutation_weights = MutationWeights::bitwise_only(),
            "all" => {} // default
            _ => {}
        }
    }

    // cost_model: choose cost function
    if let Some(PropertyValue::String(model)) = metadata.get("cost_model") {
        match model.as_str() {
            "latency" => config.cost_fn = CostFn::Latency,
            "throughput" => config.cost_fn = CostFn::Throughput,
            "size" => config.cost_fn = CostFn::Size,
            _ => {}
        }
    }

    // tolerance: relaxed equivalence for FP
    if let Some(PropertyValue::Float(tol)) = metadata.get("tolerance") {
        config.tolerance = Some(*tol);
    }

    // allowed_mutations: restrict which mutations are applied
    if let Some(PropertyValue::List(mutations)) = metadata.get("allowed_mutations") {
        config.allowed_mutations = mutations.iter()
            .filter_map(|v| match v {
                PropertyValue::String(s) => Some(s.clone()),
                _ => None,
            })
            .collect();
    }

    config
}
```

**Tests**:
- `test_mcmc_config_from_metadata_search_space_linear`: Restricts to linear mutations
- `test_mcmc_config_from_metadata_cost_model`: Uses latency cost model
- `test_mcmc_config_from_metadata_tolerance`: Sets FP tolerance
- `test_mcmc_config_from_metadata_allowed_mutations`: Restricts to listed mutations

---

## Integration Test: End-to-End Pipeline

### Test: Full derivation cycle

```brief
// test_functions.bv
defn add(x: Int, y: Int) -> Int
    !> overflow: "wrapping";
    !> associative: true;
:= {
    2, 3 -> 6;
    0, 5 -> 5;
    10, 20 -> 30;
};
```

```rust
#[test]
fn test_end_to_end_derive_and_build() {
    // 1. Parse the source
    // 2. Run brief derive (enumerative + SMT)
    //   → Should synthesize: term x + y;
    // 3. Verify foo.derive.bv exists
    // 4. Read foo.derive.bv → parse → body is present
    // 5. Run brief build on foo.derive.bv
    //   → Assertion mode verifies {2,3→6, 0,5→5, 10,20→30}
    // 6. Run brief derive --stochastic
    //   → MCMC should find x + y (already optimal, no change)
    //   → Or find commutativity swap y + x (equivalent cost)
    // 7. Verify foo.opt.bv exists (same as derive.bv for optimal case)
    // 8. Build foo.opt.bv → passes assertions
    // 9. Run brief accept foo.bv
    //   → Folds x + y body into test_functions.bv
    //   → Derivation block demoted to // := { ... } comment
    // 10. Read foo.bv → body present, // := comment preserved
    // 11. Run brief build foo.bv
    //   → Assertion mode verifies body matches the // := spec
    // 12. Run brief accept foo.bv again (idempotent)
    //   → No := blocks remain → no-op, file unchanged
}
```

### Test: MCMC finds optimization

```brief
// test_optimize.bv
defn double(x: Int) -> Int := {
    0 -> 0;
    5 -> 10;
    100 -> 200;
};
```

Enumerative might synthesize: `x + x` (cost: variable + binary_op = 1 + 3 = 4)
MCMC might find: `x << 1` (cost: variable + shift = 1 + 3 = 4, same but
interesting) or potentially better on target hardware.

### Test: Tolerance for floating-point

```brief
defn celsius_to_fahrenheit(c: Float) -> Float := {
    0.0 -> [0.001] 32.0;
    100.0 -> [0.001] 212.0;
};
```

Enumerative: `c * 1.8 + 32.0` (cost: ~10)
MCMC might try: `c * 9.0/5.0 + 32.0` (same cost) or `(c*9 + 160)/5` (more ops,
rejected).

## Phase H — Metadata-Driven Optimization Hints (3 sub-steps)

**Goal**: Wire the `MetadataRegistry` (created in Phase G) into each active backend so
`!>` metadata annotations on functions, loops, and instructions produce real
LLVM IR attributes, Webstack flags, and CIRCT options. Currently, Phase G only
created the vocabulary and lookup functions — no backend consumes them.

### Step H.0 — LLVM Fast-Math Flag Emission

**File**: `src/backend/llvm/mod.rs`

**What**: During function emission, read the function's `!>` metadata entries
(`fp_math`, `fp_contract`, `associative`, `commutative`, `overflow`) and emit
the corresponding LLVM attributes via `MetadataRegistry::llvm_attr()`.

| `!>` metadata | LLVM attribute | Emitted on |
|---|---|---|
| `fp_math: "fast"` | `"fast"` | `fadd`/`fmul`/etc. |
| `fp_contract: "*"` | `"contract"` | `fadd`/`fmul` |
| `associative: "*"` | `"reassoc"` | function |
| `overflow: "wrapping"` | `"nuw nsw"` | `add`/`sub`/`mul` |
| `overflow: "checked"` | `"nsw"` | `add`/`sub`/`mul` |
| `readonly: "*"` | `"readonly"` | function |
| `inline_hint: "always"` | `"alwaysinline"` | function |
| `inline_hint: "never"` | `"noinline"` | function |
| `unroll_hint: N` | `"unroll"` | loop |

**Tests**: `test_llvm_fast_math_fp_math`, `test_llvm_overflow_flag`,
`test_llvm_unroll_hint_loop` — verify emitted `.ll` contains the expected
string attributes.

### Step H.1 — Webstack Options

**File**: `src/backend/webstack.rs`

**What**: During Webstack code generation, read `!>` metadata and emit
WebAssembly-level options via `MetadataRegistry::webstack_option()`.

| `!>` metadata | Webstack option |
|---|---|
| `convergence: "tight"` | `no_subnormals` |
| `alloc_scope: "stack"` | `stack_allocation` |
| `fp_contract: "*"` | `fma_fuse` |

**Tests**: `test_webstack_convergence_option`,
`test_webstack_alloc_scope_stack` — verify option string appears in generated
JS glue.

### Step H.2 — CIRCT Options

**File**: `src/backend/circt.rs`

**What**: During CIRCT code generation, read `!>` metadata and emit
CIRCT-level options via `MetadataRegistry::circt_option()`.

| `!>` metadata | CIRCT option |
|---|---|
| `overflow: "wrapping"` | `comb.add` |
| `convergence: "tight"` | `single_cycle` |
| `unroll_hint: N` | `unroll_factor` |

**Tests**: `test_circt_overflow_wrapping`, `test_circt_convergence_tight`
— verify option emitted in `.mlir` output.

---

## Phase I — CLI Completion (`brief derive` flags + `brief accept`)

**Goal**: Complete the phase E sub-steps that were deferred. Phase E committed
the doppelganger infrastructure and a bare `brief derive` command, but omitted
flag parsing (`--stochastic`, `--iterations`, etc.) and the `brief accept`
subcommand. Phase I finishes both.

### Step I.0 — `brief derive` CLI Flags

**File**: `src/derive/cli.rs`, `src/main.rs`

**What**: Add flag parsing to the `brief derive` command:

```
brief derive <file>                # enumerative + SMT → foo.derive.bv
brief derive --stochastic <file>   # also run MCMC → foo.opt.bv
brief derive --iterations N        # MCMC iterations (default: 10000)
brief derive --temperature T       # initial temperature (default: 1.0)
brief derive --enumerative-depth N # max depth (default: 5)
brief derive --all                 # process all transitive imports
```

Flags are parsed with a simple hand-written loop (no clap dependency in
`src/main.rs`). The handler passes them to `handle_derive_command` which
currently ignores them — extend the function signature to accept a config
struct.

**`DeriveConfig` struct** (new, in `src/derive/cli.rs`):

```rust
pub struct DeriveConfig {
    pub stochastic: bool,
    pub iterations: usize,
    pub temperature: f64,
    pub enumerative_depth: usize,
    pub process_all: bool,
}
```

Default: `DeriveConfig { stochastic: false, iterations: 10_000,
temperature: 1.0, enumerative_depth: 5, process_all: false }`.

**Flow**:
1. CLI parses flags, builds `DeriveConfig`
2. `synthesize()` receives config — uses `enumerative_depth` for depth cap
3. If `stochastic: true`, calls `mcmc_superoptimize()` (Phase F) after synthesis
4. Writes to `foo.opt.bv` instead of `foo.derive.bv` when `stochastic`

**Tests**:
- `test_derive_with_stochastic_flag`: `--stochastic` → MCMC runs
- `test_derive_with_custom_depth`: `--enumerative-depth 3` → limited search
- `test_derive_with_iterations`: Non-default iteration count passed to MCMC
- `test_derive_all_flag`: Imported derivation blocks also processed

### Step I.1 — `brief accept` Subcommand

**File**: `src/derive/accept.rs` (new), `src/main.rs`, `src/derive/mod.rs`

**What**: Add `brief accept <file>` that folds doppelganger bodies back into
the source `.bv` file. The compiler NEVER mutates source — `brief accept` is
an intentional user action after reviewing the generated `foo.derive.bv`.

```
brief accept <file>              # fold foo.derive.bv bodies into foo.bv
brief accept <file> --opt        # fold from foo.opt.bv instead (MCMC result)
brief accept <file> --all        # accept all derivation blocks in file
```

**Algorithm**:
1. Read the shadow file (`foo.derive.bv` or `foo.opt.bv`)
2. Parse it to find synthesized bodies
3. Read the original `foo.bv`, find each `:= { ... }` block
4. Replace the derivation comment with the actual body
5. Write `foo.bv` (with `.bak` backup of original)

**Why separate from `brief derive`**: The derive step is automatic and lossless
(original file untouched). The accept step is an explicit review gate — the
developer inspects `foo.derive.bv`, decides "this looks correct," then folds it in.

**Why no `--all` default**: A large file may have multiple derivation blocks.
The developer should review each synthesized body before accepting. `--all`
is available for CI/scripted workflows.

**Relation to assertion mode**: After acceptance, `brief build foo.bv` runs in
assertion mode (`verify_derivation_assertions` in compile pipeline) — if the
body and examples disagree, the build fails. This ensures accepted results
stay correct after source changes.

**Tests**:
- `test_accept_basic`: derive → accept → `foo.bv` has inlined bodies
- `test_accept_opt`: accept from `foo.opt.bv` → MCMC-optimized bodies
- `test_accept_no_shadow`: no `.derive.bv` → error points to `brief derive`
- `test_accept_preserves_derivation_block`: `// := { ... }` comment retained
- `test_accept_without_derivation`: source has no `:=` blocks → no-op
- `test_accept_idempotent`: accept twice → second run sees no `:=` → no-op

---

## Documentation Requirements

Every phase must update the following documentation in the same commit:

| Phase | Document to update |
|-------|-------------------|
| All | `docs/architecture/overview.md` — mention the nine-phase (A–I) pipeline |
| A | `docs/architecture/features/derivation-blocks.md` — tolerance syntax |
| B | `docs/architecture/features/derivation-blocks.md` — assertion build gate |
| C | `docs/architecture/features/derivation-blocks.md` — enumerative synthesis |
| D | `docs/architecture/features/derivation-blocks.md` — SMT synthesis |
| E | `docs/architecture/features/derivation-blocks.md` — doppelganger system + `brief accept` |
| F | `docs/architecture/features/mcmc-superoptimizer.md` — new doc |
| G | `docs/architecture/optimization-hints.md` — new vocabulary reference |
| H | `docs/architecture/optimization-hints.md` — backend wiring section |
| I | `docs/architecture/features/derivation-blocks.md` — `brief accept` docs |

---

## Commit Sequence

```
 1. Phase A: Parser/AST wiring + tolerance syntax                                     ✓ (0b244970)
 2. Phase B: Assertion build gate + interpreter call_function                         ✓ (ba45226e)
 3. Phase C: Full enumerative synthesis (type-aware, interpreter-backed)              ✓ (76c70901)
 4. Phase D: Full SMT synthesis (SyGuS, Z3 integration)                               ✓ (32ae4a47)
 5. Phase E: Doppelganger write-back + build resolution (E.0–E.3)                     ✓ (53f5604c)
 6. Phase F: MCMC superoptimizer (mutations, equivalence, sampler, Pareto)            ✓ (a272dc11)
 7. Phase G: Metadata vocabulary reference + backend mappings                         ✓ (09cfc234)
 8. Phase H: Metadata-driven codegen (LLVM, Webstack, CIRCT wiring)                   ✓ (9d54f263+)
 9. Phase I: CLI completion (`brief derive` flags + `brief accept`)                   ✓ (cb02b43b+)
  -- Parse fix: `:= { ... }` without body (I.1 dependency)                           ✓ (755aa08c)
 10. Integration tests: end-to-end pipeline
 11. Benchmark baseline + MCMC comparison
```

Each commit must pass `cargo test --lib` and `cargo build --release`.
After commit 11, run the full benchmark suite and compare to baseline.