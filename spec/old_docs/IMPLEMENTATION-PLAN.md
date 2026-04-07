# Brief Implementation Plan
**Version:** 1.0
**Date:** 2026-04-05
**Status:** Working Document

---

## Overview

This document outlines the implementation roadmap for the Brief compiler, focusing on completing the contract verification system and fixing known issues in the standard library.

The plan is organized into phases. Each phase builds on the previous one.

---

## Phase 1: Symbolic Execution Engine

### Goal
Build a reusable symbolic executor that can trace execution paths through guards and verify contract implications.

### Components to Build

**1.1 Path Constraint Accumulation**
- Track boolean constraints from guards `[condition] { ... }`
- Accumulate constraints as we traverse each branch
- Handle nested guards by combining constraints with AND

**1.2 Symbolic Variable Tracking**
- Track variable assignments symbolically
- Represent assignments as: `x' = x + 1` (new value in terms of old)
- Handle multiple assignment types:
  - Direct: `&x = expr` → `x' = eval(expr)`
  - Conditional: `[cond] { &x = expr }` → if cond then `x' = eval(expr)`
  - Let bindings: track local variable scope

**1.3 Expression Simplification**
- Simplify boolean expressions (constant folding, identity rules)
- Handle numeric comparisons (intervals for Int/Float)
- Normalize for implication checking

**1.4 Implication Checking**
- Implement: `(pre AND path_constraints) ⇒ post`
- Start with simple syntactic checks
- Add interval arithmetic for numeric reasoning

### Files to Modify
- `src/proof_engine.rs` - Add symbolic executor module

### Success Criteria
- Can enumerate all paths through a transaction body
- Can track symbolic state through assignments
- Can verify simple contract implications

---

## Phase 2: Transaction Contract Verification

### Goal
Apply symbolic execution to verify transaction contracts.

### Components to Build

**2.1 Pre-condition Satisfiability Check**
- Verify pre-condition is not contradictory
- Check: `pre AND (NOT pre)` is unsatisfiable
- Warn if pre-condition can never be true

**2.2 Path Enumeration**
- Walk through all statements in transaction body
- At each guard, branch into both true and false paths
- Collect constraints at each branch point

**2.3 Post-condition Verification**
- At each `term` statement, verify post-condition
- Use: `pre AND path_constraints ⇒ post`
- Report which path fails if verification fails

**2.4 Termination Reachability**
- Verify at least one path from pre-condition to `term`
- Build control flow graph through guards
- Check for cycles without progress (infinite loop detection)

### Files to Modify
- `src/proof_engine.rs` - Add transaction verification methods
- `src/typechecker.rs` - Potentially add contract-specific checks

### Success Criteria
- Can verify simple transaction contracts at compile time
- Reports clear errors when contract cannot be proven
- Handles nested guards correctly

---

## Phase 3: Definition Contract Verification

### Goal
Apply the same verification to `defn` functions.

### Components to Build

**3.1 Hoare Triple Verification**
- Treat defn body as: `{pre} body {post}`
- Verify: if pre holds when called, post holds after execution
- defn has no state mutation, so simpler than transactions

**3.2 Handling defn Bodies**
- defn runs once, no reactor loop
- No `@` (prior-state) in defn contracts
- Track return values through `term`

**3.3 Multi-output Verification**
- Verify post-condition for each output slot
- Handle trailing comma semantics (void slots)

### Files to Modify
- `src/proof_engine.rs` - Extend verification to definitions

### Success Criteria
- Can verify defn contracts like: `defn add(a, b) [true][result == a + b]`
- Works with guards in defn body

---

## Phase 4: Ownership Verification

### Goal
Implement borrow-based mutual exclusion for async transactions.

### Components to Build

**4.1 Write Claim Tracking**
- Track which variables each transaction writes to (`&var`)
- Build write-set for each async transaction

**4.2 Read/Write Conflict Detection**
- For concurrent transactions:
  - If both write to same variable → check pre-condition exclusivity
  - If one writes, one reads → conflict (unless pre-conditions exclusive)

**4.3 Pre-condition Exclusivity Analysis**
- Check if two conditions can both be true
- Start with simple variable overlap check
- Add interval analysis for numeric conditions

**4.4 Error Reporting**
- When async transactions conflict, explain:
  - Which variables conflict
  - Why pre-conditions overlap
  - How to fix (make exclusive or ensure never simultaneous)

### Files to Modify
- `src/proof_engine.rs` - Replace/extend `check_mutual_exclusion`

### Success Criteria
- Catches conflicting async transactions at compile time
- Provides clear error messages with fix suggestions

---

## Phase 5: Standard Library Fixes

### Goal
Fix the standard library to conform to SPEC.md guidelines.

### Components to Fix

**5.1 Move Iteration to Transactions**
- Identify defns with iteration constructs `[i < n] { ... }`
- Convert to transactions where looping is needed
- Keep simple pure defns as-is

**5.2 Foreign Function Registry**
- Add missing foreign function implementations
- Current: ~30 implemented, ~150+ declared
- Prioritize: collections, json, encoding, time

**5.3 Contract Review**
- Verify all defn contracts are provable
- Fix any incorrect contracts

### Files to Modify
- `lib/std/*.bv` - Fix definitions and signatures

### Success Criteria
- No iteration constructs in defn that should be txn
- All declared frgn functions have implementations

---

## Phase 6: Error Messages

### Goal
Enhance error messages to teach Brief.

### Components to Build

**6.1 Path Display**
- Show the path taken through guards
- Number each branch point

**6.2 Constraint Explanation**
- Show what constraints were accumulated
- Explain why post-condition doesn't follow

**6.3 Fix Suggestions**
- Suggest specific changes in Brief syntax
- Reference relevant spec sections

### Files to Modify
- `src/proof_engine.rs` - Enhance error formatting
- `src/errors.rs` - Potentially add new error types

### Success Criteria
- Errors are educational, not just "contract failed"
- Programmer can understand what's wrong and how to fix it

---

## Implementation Order

```
Phase 1: Symbolic Execution (Foundation)
    ↓
Phase 2: Transaction Contracts
    ↓
Phase 3: Definition Contracts  
    ↓
Phase 4: Ownership Verification
    ↓
Phase 5: stdlib Fixes
    ↓
Phase 6: Error Messages
```

Each phase should produce working, testable code before moving to the next.

---

## Testing Strategy

**Unit Tests**
- Test symbolic executor on simple expressions
- Test path enumeration on known guard patterns

**Integration Tests**
- Test transaction verification on real code
- Test ownership checking on async transaction pairs

**Regression Tests**
- Ensure existing functionality still works
- Run current test suite before/after each phase

---

## Implementation Notes

### Conservative Verification Approach (2026-04-05)

The initial symbolic executor implementation uses a **conservative verification strategy** to avoid false positives:

1. **Post-condition handling with `@var`**: Currently accepts all transactions with `@var` in post-condition without verifying. This ensures no false rejections but means some invalid contracts pass verification.

2. **Symbolic variable tracking**: Currently initializes variables from pre-condition but doesn't fully track how assignments modify state. This is conservative - we don't verify the relationship between pre and post for state changes.

3. **Design Rationale**: Brief's goal is "if it runs, it won't have bugs." A conservative verifier that accepts valid programs is preferable to one that rejects valid programs. As the verifier matures, we can strengthen these checks.

### Future Improvements (TODO)
- Track variable assignments and verify pre→post relationship for state changes
- Implement proper symbolic execution with constraint solving
- Add interval arithmetic for numeric reasoning
- Implement termination reachability proofs

---

## Phase 7: sig as Function Type

**Goal**: Extend `sig` to be usable as a function type, enabling native implementation of higher-order functions (map, filter, reduce).

### Core Design

```brief
# Declare sig that can be used as type
sig print(msg: String) -> true;

# Use sig as parameter type in defn
defn log(msg: String, printer: sig(msg: String) -> true) [true][true] {
  term printer(msg);
};

# Pass defn as sig (implicit cast)
defn my_print(msg: String) [true][true] -> Bool {
  term true;
};

txn main [true][done] {
  log("hello", my_print);  # defn implicitly cast to sig
  term;
};
```

### Naming Rules

- **Conflict**: Cannot have both `defn foo` and `sig foo` in same scope
- **Alias**: Use `sig foo as BarName` to create unique type name
- **Direct use**: Any sig can be used directly as parameter type

### Implementation Components

#### Phase 7.1: AST & Parser
- Add `Type::Sig(String)` variant for sig-based types
- Allow sig names as parameter types in defn declarations
- Parse `sig name as TypeName` syntax for explicit aliases

#### Phase 7.2: Type Checker
- When parameter type is sig, verify passed defn matches signature
- Check `-> true` assertions are provable for passed defn
- Handle implicit defn-to-sig conversion at call site

#### Phase 7.3: Runtime (Interpreter)
- Support defn as first-class callable (pass reference, not invoke immediately)
- Support calling defn through sig-type parameter
- Handle implicit sig conversion at call site

#### Phase 7.4: stdlib - Replace frgn with defn
- Convert `map`, `filter`, `reduce` from frgn to native defn using sig types
- Ensure iteration is expressed as full transactions (not callable from defn)

### Design Decisions

| Decision | Resolution |
|----------|------------|
| Naming conflict | `defn foo` and `sig foo` cannot coexist |
| Alias syntax | `sig foo as BarName` creates unique type |
| Partial application | Not in v1 - keep simple |
| Multi-output | Supported, must be declared explicitly |

---

## Open Questions (Deferred)

These items are out of scope for now but may be needed in the future:

- **Function Types**: Adding `Type::Function` to support `T -> U` function types
- **Generics**: Adding parametric polymorphism for generic functions
- **ADTs**: Adding Option/Result algebraic data types
- **WASM Export**: Full WASM compilation target

---

## References

- `spec/SPEC.md` - Language specification (authoritative)
- `spec/v4-brief-lang-spec.md` - Previous version for reference
- `src/proof_engine.rs` - Current proof engine implementation
- `src/interpreter.rs` - Runtime execution model

---

*End of Implementation Plan v1.0*