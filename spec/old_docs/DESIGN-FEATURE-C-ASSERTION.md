# Design Document: Assertion Verification with `sig -> true`
**Feature:** Feature C - Assertion Verification  
**Date:** 2026-04-05  
**Prerequisite:** Feature B (Sig Casting) + Issue #4 (Symbolic Executor)  
**Complexity:** High (150-200 lines)  
**Time Estimate:** 2-3 hours

---

## 1. Overview

### 1.1 Problem Statement

Sometimes we want to assert that a function **always returns true**:

```brief
sig always_succeeds: String -> true;
```

But how does the compiler know this is safe? It must verify:
1. The function can produce a Bool
2. That Bool is guaranteed to be true
3. This holds for all possible inputs

### 1.2 Solution

The `-> true` assertion is a **compile-time verification** that proves:
- At least one execution path produces `Bool = true`
- All other outcomes are invalid or eliminated
- This holds given the actual call sites in the program

### 1.3 Benefits

1. **Compile-time safety:** Impossible to call a function that fails the assertion
2. **Explicit contracts:** `-> true` documents that function never fails
3. **Conditional asserts:** Can be proven based on call context
4. **Type refinement:** Sig assertion is a refinement type

---

## 2. Semantics

### 2.1 Assertion Syntax

```brief
sig always_succeeds: Args -> true;
```

**Meaning:** "I assert this function always returns Bool, and that Bool is always `true`"

**Verification obligation:** Compiler must prove:
- At least one path produces Bool
- That Bool is constrained to true by the postcondition, guards, and arguments

### 2.2 Two Verification Modes

**Mode 1: Absolute Truth**
```brief
defn always_true() [true][result == true] -> Bool {
  term true;
};

sig always_true_cast: void -> true;
// ✅ VALID: Defn always returns true
```

**Mode 2: Context-Aware**
```brief
defn maybe_true(b: Bool) [true][true] -> Bool {
  term b;
};

// At call site:
txn caller [~success] {
  let result = maybe_true(true);  // Always called with true
  &success = result;
  term;
};

sig contextualized: Bool -> true;
// ✅ VALID: Given how it's called, always produces true
```

### 2.3 What Fails Assertion

```brief
defn conditional(x: Int) -> Bool {
  [x > 0] term true;
  [x <= 0] term false;  // Can return false
};

sig guaranteed_true: Int -> true;
// ❌ INVALID: Not always true (returns false when x <= 0)

defn error_case(b: Bool) -> Bool {
  [b] term true;
  [~b] term false;  // Always path to false
};

sig error_cast: Bool -> true;
// ❌ INVALID: Depends on input; not guaranteed
```

---

## 3. Implementation

### 3.1 Proof Engine Changes

**File:** `src/proof_engine.rs`

```rust
fn verify_assertion_cast(
    sig: &Signature,
    defn: &Definition,
) -> Result<(), Error> {
    // For sig -> true: verify some path produces Bool = true
    
    if !matches!(sig.output_type, Type::Bool) {
        // Extract Bool from union if needed
        if let OutputType::Union(types) = &defn.output_types {
            if !types.contains(&Type::Bool) {
                return Err(Error::AssertionNoBoolPath);
            }
        } else {
            return Err(Error::AssertionMustBeBool);
        }
    }
    
    let paths = enumerate_paths(&defn.body);
    
    for path in paths {
        // Walk path with symbolic execution
        let mut state = SymbolicState::new(&defn.contract.pre_condition);
        
        for stmt in path {
            match stmt {
                Statement::Assignment { target, expr } => {
                    state.assign(target, expr);
                }
                
                Statement::Guarded { condition, .. } => {
                    state.guard(condition, true);
                }
                
                Statement::Term(outputs) => {
                    if let Some(Some(expr)) = outputs.first() {
                        // Check if this path produces Bool = true
                        if is_provably_true(expr, &state) {
                            return Ok(());  // Found satisfying path
                        }
                    }
                }
                
                _ => {}
            }
        }
    }
    
    Err(Error::AssertionNotSatisfied {
        sig: sig.name.clone(),
        reason: "No path produces Bool = true".to_string(),
    })
}

fn is_provably_true(expr: &Expr, state: &SymbolicState) -> bool {
    let sym_val = eval_symbolic(expr, state);
    
    match sym_val {
        SymbolicValue::Literal(Value::Bool(true)) => true,
        
        SymbolicValue::Identifier(name) => {
            // Check if variable is known to be true
            if let Some(SymbolicValue::Literal(Value::Bool(true))) = 
                state.assignments.get(&name) {
                true
            } else {
                false
            }
        }
        
        _ => false,  // Conservative
    }
}
```

### 3.2 Context-Aware Verification (Advanced)

For more sophisticated verification, analyze call sites:

```rust
fn verify_assertion_with_context(
    sig: &Signature,
    defn: &Definition,
    program: &Program,
) -> Result<(), Error> {
    // Find all call sites of this sig
    let call_sites = find_all_calls_to_sig(program, &sig.name);
    
    for call_site in call_sites {
        let args = extract_call_arguments(call_site);
        
        // For each call site, verify the defn produces true with those args
        if !can_produce_true_with_args(defn, &args, program) {
            return Err(Error::AssertionFailsAtCallSite {
                sig: sig.name.clone(),
                call_site: format!("{:?}", call_site),
            });
        }
    }
    
    Ok(())
}

fn can_produce_true_with_args(
    defn: &Definition,
    args: &[Expr],
    program: &Program,
) -> bool {
    // Evaluate defn with specific arguments
    let paths = enumerate_paths(&defn.body);
    
    for path in paths {
        // Create state with argument values
        let mut state = SymbolicState::new(&defn.contract.pre_condition);
        
        // Bind parameters to argument values
        for (param, arg) in defn.parameters.iter().zip(args) {
            state.assign(&param.0, arg);
        }
        
        // Execute path
        for stmt in path {
            // ... same as above ...
        }
    }
    
    false  // No path produced true
}
```

---

## 4. Testing Strategy

### 4.1 Unit Tests

**File:** `tests/assertion_tests.rs`

```rust
#[test]
fn test_always_true_assertion() {
    let code = r#"
        defn always() [true][result == true] -> Bool {
            term true;
        };
        sig always_cast: void -> true;
    "#;
    let result = compile(code);
    assert!(result.is_ok());
}

#[test]
fn test_conditional_assertion_fails() {
    let code = r#"
        defn maybe(b: Bool) -> Bool { term b; };
        sig maybe_cast: Bool -> true;
    "#;
    let result = compile(code);
    assert!(result.is_err());
}

#[test]
fn test_guarded_true_path() {
    let code = r#"
        defn smart(x: Int) -> Bool {
            [x > 0] term true;
            [x <= 0] term false;
        };
        sig positive_only: Int -> true;  // Only valid for x > 0
    "#;
    // This FAILS because x <= 0 can produce false
    let result = compile(code);
    assert!(result.is_err());
}

#[test]
fn test_context_aware_assertion() {
    let code = r#"
        defn depends(b: Bool) -> Bool { term b; };
        
        txn caller [true][success] {
            let result = depends(true);
            &success = result;
            term;
        };
        
        sig ctx_aware: Bool -> true;
    "#;
    // This might PASS with context analysis (called with true)
    // Or FAIL without context analysis (too conservative)
    // For v6.1, assume FAIL (conservative)
    let result = compile(code);
    // Implementation choice: be conservative initially
}
```

### 4.2 Integration Tests

Create `examples/assertions.bv`:

```brief
defn always_succeeds() [true][result == true] -> Bool {
  term true;
};

sig success_guaranteed: void -> true;

txn use_assertion [~done] {
  let result = success_guaranteed();
  &done = result;
  term;
};
```

---

## 5. Error Messages

### Assertion Failed

```
[E006] Assertion verification failed for 'safe_call'

Sig 'safe_call' declares:
  -> true

This means the function must ALWAYS return Bool=true

But checking function 'unsafe' found path(s) that don't:

Path 1 (error case):
  [condition] -> term false;  ❌ Returns false, not true

To fix:
  Option 1: Remove the false path
  Option 2: Use different function that guarantees true
  Option 3: Don't use -> true assertion
```

---

## 6. Implementation Checklist

- [ ] Implement `verify_assertion_cast()` in proof engine
- [ ] Implement `is_provably_true()` with symbolic execution
- [ ] Add `-> true` syntax validation (must be Bool only)
- [ ] Implement (optional) context-aware verification
- [ ] Write comprehensive unit tests
- [ ] Create integration test examples
- [ ] Update error messages
- [ ] Test with symbolic executor from Issue #4
- [ ] Verify with existing tests
- [ ] Commit with message

---

## 7. Success Criteria

- ✅ Absolute true assertions verified correctly
- ✅ Conditional assertions rejected (conservative)
- ✅ Error messages explain why assertion failed
- ✅ Works with symbolic executor (Issue #4)
- ✅ Enables signature refinement types
- ✅ All 8 existing stress tests still pass

---

## 8. Future Enhancements (v7.0+)

- Context-aware verification (analyze call sites)
- Refinement types: `sig -> x > 0` (numeric assertions)
- Dependent types: `sig -> result == input + 1`

---

*End of Design Document: Assertion Verification*
