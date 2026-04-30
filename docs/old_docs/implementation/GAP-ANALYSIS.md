# Brief Language Gap Analysis: Spec vs Implementation

**Document Version:** 3.0  
**Date:** 2026-04-08  
**Status:** DECISIONS RECORDED - READY FOR IMPLEMENTATION

---

## Executive Summary

This document captures discrepancies between the Brief Language Specification and the actual compiler implementation, along with decisions on how to resolve each gap.

---

## DECISIONS LOG

### 1. Keyword Aliases (def/defn/definition, etc.)

**Decision:** Support multiple declaration keywords as aliases for better ergonomics.

| Concept | Aliases to Support |
|---------|-------------------|
| Definition | `def`, `defn`, `definition` |
| Transaction | `txn`, `transact`, `transaction` |
| Signature | `sig`, `sign`, `signature` |
| Constant | `const`, `constant` |
| Render | `render` (already correct, spec was wrong) |

**Implementation Required:** Update lexer to accept all aliases, map to single token type.

---

### 2. Lambda/Bodyless Syntax

**Decision:** SHOULD BE IMPLEMENTED.

**Rules:**
- Contracts are declared at the TOP (before body)
- Transaction/defn parameters are OPTIONAL
- `txn name [pre][post] { body }` - with body (current behavior)
- `txn name [pre][post];` - without body (lambda-style, NEW)
- **RCT/REACTIVE transactions CANNOT have parameters** - enforce at compile time
- Lambda-style transactions must have provable postcondition

**Lambda Defn Example:**
```brief
// Full syntax
defn double(n: Int) -> Int [true][result == n * 2] {
    term n * 2;
};

// Lambda syntax which mutates an out of scope var called "result"(no body needed)
defn double(n: Int) -> Int [true][result == n * 2];
// OR with implicit return:
// defn double(n: Int) -> Int [true][n * 2];
```

**Implementation Required:**
1. Parser: Accept `;` as transaction/defn termination (currently requires `}`)
2. Desugarer: Handle lambda-style transactions and defns

---

### 3. `view` vs `render` Keyword

**Decision:** MUST be `render`. `<view>` is specifically a HTML region in .rbv files.

| Keyword | Use |
|---------|-----|
| `render` | Top-level render block definition |
| `<view>...</view>` | HTML region in .rbv files |

**Implementation Required:** Fix SPEC.md grammar (spec was wrong).

---

### 4. rstruct HTML Syntax

**Decision:** SPEC should match implementation (inline `<tag>` syntax).

**Correct Syntax:**
```brief
rstruct Counter {
    count: Int;
    
    txn increment [count < 100][count == @count + 1] {
        &count = count + 1;
        term;
    };

    <div>
        <span b-text="count">{count}</span>
    </div>
}
```

**Implementation Required:** Update SPEC.md examples.

---

### 5. Transaction Parameters

**Decision:** Add to grammar as OPTIONAL. RCT/REACTIVE transactions should NEVER have parameters.

**Grammar:**
```bnf
transaction ::= ("async")? "txn" identifier "(" parameters? ")" contract "{" body "}" ";"
               | ("async")? "rct" "txn" identifier contract "{" body "}" ";"  # No params for reactive
```

**Validation Required:**
- If `rct` keyword present, reject parameters with compiler error
- Only passive transactions can have parameters

---

### 6. `txc` Transactions

**Decision:** REMOVE. It should not exist.

**Implementation Required:**
1. Remove `Token::Txc` from lexer.rs
2. Remove `parse_tx_c_transaction()` from parser.rs
3. Update any test files using `txc`

---

### 7. `sig` (Signature) - Complete Redesign

**Decision:** Redefine `sig` as an explicit constraint on a defn's output type path.

**Definition:** A `sig` is a **constrained view** of a defn that limits which output type path is valid. The compiler must PROVE the constrained path is reachable in all cases.

**Syntax:**
```brief
defn complex(x: Bool) -> Bool | Int | String {
    [x == true] term true;       // Path 1: Bool
    [x == false] term 42;        // Path 2: Int  
    term "default";               // Path 3: String
};

// Sig explicitly binds to a defn, constrains to String path only
sig toString: Bool -> String = complex();

// Usage - compiler verifies String path is always reachable
txn use_sig [true] {
    let s: String = toString(true);  // Calls complex(), but only String path allowed
    term;
};
```

**Semantics:**
- `sig name: InputType -> OutputType = defn;` binds a sig to a defn
- Compiler verifies: For all inputs, the defn can reach the specified output type
- `frgn sig` imports sigs from FFI bindings (already handled separately)

**Current Implementation Issues:**
- `sig` exists in parser but grammar doesn't document it
- `-> true` syntax not properly enforced
- Need to implement sig-to-defn binding and path verification

**Implementation Required:**
1. Update grammar to include sig declaration syntax
2. Implement sig-to-defn binding
3. Add path reachability verification
4. Document in LANGUAGE-REFERENCE.md

---

### 8. `result` Variable - Complete Redesign

**Decision:** `result` is NOT magic. It is an **out-of-scope variable** that is auto-created at the parent scope.

**IMPORTANT - NOT A RETURN VALUE:**
- `result == n * 2` does NOT mean "the return value equals n * 2"
- It means "the out-of-scope variable `result` has been mutated to equal n * 2"
- `term x;` desugars to `&result = x; term;`

**Syntax:**
```brief
// Compiler sees "result == n * 2" → infers: let result: Int; at parent scope
// Type inference: result == n * 2 implies result is Int

defn double(n: Int) -> Int [true][result == n * 2] {
    term n * 2;  // Desugars to: &result = n * 2; term;
};
```

**Desugared Form:**
```brief
let result: Int;  // Auto-created at parent scope (type inferred from postcondition)

defn double(n: Int) -> Int [true][result == n * 2] {
    &result = n * 2;  // Write access to outer scope variable
    term;
};
```

**`@` Semantics:**
- `@result` = value at function START
- `result` = value after function completes (current value)

**Example:**
```brief
let result: Int = 5;  // Outer scope

defn double(n: Int) -> Int [true][result == n * 2] {
    term n * 2;  // &result = n * 2; term;
};

// Call double(3)
// Before: result = 5, @result = 5
// Write: &result = 3 * 2
// After: result = 6
// Postcondition check: result == 3 * 2 → 6 == 6 ✓
```

**Sharing:**
- Multiple defns CAN share the same `result` variable
- Borrow checker enforces: only one defn can write to `result` at a time
- If two defns both write to `result` simultaneously → borrow checker error

**Type Conflicts:**
- If `result == n * 2` (implies Int) and `result == true` (implies Bool) → compiler error
- Type inference catches conflicts at compile time

**Implementation Required:**
1. Update parser/desugarer to handle `result` as real variable
2. Type inference from postcondition expression
3. Auto-create `result` variable at parent scope
4. Desugar `term x;` to `&result = x; term;`
5. Remove "magic" `result` handling from typechecker
6. Document in LANGUAGE-REFERENCE.md

---

### 9. `void` vs `Void`

**Decision:** Use `Void` for type correctness.

**Action:** Update all examples and docs to use `Void` instead of `void`.

---

### 10. Async Transaction Order Flexibility

**Decision:** Note flexibility in spec.

**Behavior:** Parser accepts both `rct async txn` and `async rct txn`.

**Documentation:** "Transaction modifiers may appear in any order."

---

### 11. Keywords List Update

**Decision:** Update keywords list.

**REMOVE:** `txc` (being removed)

**Updated Keywords:**
```
defn    def     definition    # All aliases for definition
txn     transact transaction # All aliases for transaction
sig     sign    signature    # All aliases for signature
rct     async   # Transaction modifiers
let     const              # Variable declarations
term    escape            # Control flow
from    import            # Imports
struct  rstruct           # Struct types
render                   # Render block (NOT view!)
frgn                      # Foreign functions
as                         # Import aliasing
true    false             # Literals
```

---

### 12. Comment Syntax

**Decision:** ONLY use `//`. Remove `#` from all references.

**Action:**
1. Update QUICK-REFERENCE.md
2. Update SPEC.md
3. Update LANGUAGE-REFERENCE.md
4. Remove `#` from lexer if present

---

## NEW: Trivial Contract Errors (P009/P010)

**Recently Added:** Compiler now rejects trivial `[true]` contracts with errors.

| Code | Error |
|------|-------|
| P009 | Trivial precondition - `[true]` is not allowed |
| P010 | Trivial postcondition - `[true]` is not allowed |

**Documentation Required:** Add to LANGUAGE-REFERENCE.md in Contract Verification section.

---

## Implementation Plan

### Phase 1: Grammar & Parser Fixes (2-3 hours)

| # | Task | Files to Modify |
|---|------|----------------|
| 1.1 | Add keyword aliases (`def`, `definition`, etc.) | `lexer.rs` |
| 1.2 | Remove `txc` keyword | `lexer.rs`, `parser.rs` |
| 1.3 | Add optional transaction parameters to grammar | `parser.rs` |
| 1.4 | Add parameter validation (reject params for rct) | `typechecker.rs` or `parser.rs` |
| 1.5 | Accept lambda-style transactions/defns (no body) | `parser.rs`, `desugarer.rs` |
| 1.6 | Add `sig` declaration syntax to grammar | `parser.rs`, `ast.rs` |

### Phase 2: Semantic Redesign - `result` Variable (3-4 hours)

| # | Task | Files to Modify |
|---|------|----------------|
| 2.1 | Remove magic `result` handling | `typechecker.rs` |
| 2.2 | Implement `result` type inference from postcondition | `parser.rs` or `desugarer.rs` |
| 2.3 | Auto-create `result` variable at parent scope | `desugarer.rs` |
| 2.4 | Desugar `term x;` to `&result = x; term;` | `desugarer.rs` |
| 2.5 | Update proof engine for new `result` semantics | `proof_engine.rs` |
| 2.6 | Add tests for `result` behavior | `tests/` |

### Phase 3: Semantic Redesign - `sig` (3-4 hours)

| # | Task | Files to Modify |
|---|------|----------------|
| 3.1 | Implement sig-to-defn binding | `parser.rs`, `resolver.rs` |
| 3.2 | Implement path reachability verification | `proof_engine.rs` |
| 3.3 | Handle `frgn sig` imports from FFI | `ffi.rs` |
| 3.4 | Add error for unprovable sig assertions | `proof_engine.rs` |

### Phase 4: Documentation Updates (2 hours)

| # | Task | Files to Modify |
|---|------|----------------|
| 4.1 | Fix keyword aliases in SPEC.md grammar | `spec/SPEC.md` |
| 4.2 | Fix rstruct syntax examples | `spec/SPEC.md`, `spec/LANGUAGE-REFERENCE.md` |
| 4.3 | Fix `view` → `render` in grammar | `spec/SPEC.md` |
| 4.4 | Document `sig` syntax and semantics | `spec/LANGUAGE-REFERENCE.md` |
| 4.5 | Document `result` semantics | `spec/LANGUAGE-REFERENCE.md` |
| 4.6 | Document keyword aliases | `spec/LANGUAGE-REFERENCE.md` |
| 4.7 | Fix comment syntax (# → //) | `spec/QUICK-REFERENCE.md` |
| 4.8 | Update keywords list | `spec/LANGUAGE-REFERENCE.md` |
| 4.9 | Add trivial contract errors to docs | `spec/LANGUAGE-REFERENCE.md`, `spec/SPEC.md` |

### Phase 5: Examples & Tests (1-2 hours)

| # | Task | Files to Modify |
|---|------|----------------|
| 5.1 | Remove `txc` from any test files | `tests/*.bv` |
| 5.2 | Fix `void` → `Void` in examples | `examples/*.bv`, `tests/*.bv` |
| 5.3 | Add tests for `result` and `sig` | `tests/` |

---

## Summary Table

| # | Original Issue | Decision | Effort | Priority |
|---|---------------|----------|--------|----------|
| 1 | Keyword mismatch | Support aliases | Medium | HIGH |
| 2 | Lambda syntax | IMPLEMENT | Medium | HIGH |
| 3 | view vs render | Spec wrong | Low | HIGH |
| 4 | rstruct syntax | Update spec | Low | MEDIUM |
| 5 | Txn params missing | Add (optional, no rct) | Medium | HIGH |
| 6 | txc undocumented | REMOVE | Low | HIGH |
| 7 | sig incomplete | REDESIGN: explicit defn constraint | High | HIGH |
| 8 | result magic | REDESIGN: out-of-scope variable | High | HIGH |
| 9 | void vs Void | Use Void | Low | LOW |
| 10 | Async order | Note flexibility | Low | LOW |
| 11 | Keywords list | Update | Low | LOW |
| 12 | # comments | Remove # | Low | LOW |
| NEW | P009/P010 docs | Document | Low | MEDIUM |

---

## Remaining Implementation Work

| # | Item | Status | Priority |
|---|------|--------|----------|
| 1 | Lambda-style verification | ✅ Complete | HIGH |
| 2 | `result` variable redesign | ✅ Complete | HIGH |
| 3 | Two failing tests (ContractBound) | ✅ Complete | MEDIUM |
| 4 | Struct/RStruct/Render instances | ✅ Implemented | HIGH |
| 5 | clone() function | ✅ Implemented | HIGH |
| 6 | List<T> generic type | ✅ Already Supported | HIGH |

---

## Design Decisions for Struct/RStruct Instances

### Syntax

```brief
// Single instance (default values)
let counter = Counter {};

// Single instance (partial init)
let counter = Counter { count: 5 };

// List of instances
let counters = [Counter {}, Counter {}];

// Default for empty struct literal uses struct's default values
```

### Method Resolution

| Syntax | Behavior |
|--------|----------|
| `Counter.increment()` | Call on global state (current) |
| `counter.increment()` | Call on instance (requires init) |
| `counters[0].increment()` | Call on list element (requires bounds check) |

### Safety Rules

1. **Compile error** if calling method on uninitialized instance
2. **Compile error** if index out of bounds
3. **Type error** if calling method on wrong type

### Transaction Field Access

- Inside instance transaction: `&count` = instance field
- Unless external variable passed: `txn process(data)` - data is external input

### Implementation Status

✅ **Phase 1**: AST + Parser (StructInstance, List<T>) - COMPLETE
✅ **Phase 2**: Type System (instance types, method resolution) - COMPLETE
✅ **Phase 3**: Instance methods + field access - COMPLETE
✅ **Phase 4**: Runtime (interpreter/WASM) - COMPLETE

**Implementation Details:**
- `Value::Instance { typename, fields }` replaces `Value::Struct(fields)`
- `clone()` function implemented - clones any value
- Instance method resolution: `counter.increment()` → `Counter.increment(counter)`
- Field access: `counter.count` works via `Expr::FieldAccess`
- Parser fix: struct definitions now correctly consume trailing semicolon

**Test File:** `tests/instances_test.bv`

---

## References

- **SPEC.md:** `spec/SPEC.md`
- **LANGUAGE-REFERENCE.md:** `spec/LANGUAGE-REFERENCE.md`
- **QUICK-REFERENCE.md:** `spec/QUICK-REFERENCE.md`
- **RENDERED-BRIEF-GUIDE.md:** `spec/RENDERED-BRIEF-GUIDE.md`
- **Lexer:** `src/lexer.rs`
- **Parser:** `src/parser.rs`
- **Proof Engine:** `src/proof_engine.rs`
- **Desugarer:** `src/desugarer.rs`
- **Examples:** `examples/`, `tests/`

---

*End of Gap Analysis v3.0*
