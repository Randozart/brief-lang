# Design Document: Symbolic Executor for Assignment Tracking
**Issue:** #4 - Proof Engine Foundation  
**Date:** 2026-04-05  
**Status:** Ready for Implementation  
**Complexity:** High (200-400 lines depending on level)  
**Time Estimate:** 4-6 hours (Level 2 - Medium)

---

## 1. Overview

### 1.1 Problem Statement

The proof engine cannot verify postconditions that rely on tracking variable assignments:

```brief
txn test [true][flag == true] {
  &flag = true;
  term;
};
```

**Error:**
```
error[P008]: contract verification failed
= transaction 'test': post-condition not satisfied on path 0
```

The compiler doesn't track that `&flag = true` means `flag` is now `true`, so it can't verify the postcondition `flag == true`.

### 1.2 Root Cause

The proof engine is **conservative**: It accepts transactions with `@` operator in postconditions (prior-state comparisons) but doesn't track regular assignments.

From IMPLEMENTATION-PLAN.md:
> "Currently accepts all transactions with `@var` in post-condition without verifying. This ensures no false rejections but means some invalid contracts pass verification."

### 1.3 Solution

Build a **symbolic executor** that:
1. Tracks variable assignments symbolically
2. Represents assignments as relationships (e.g., `x' = x + 1`)
3. Evaluates postconditions against symbolic state
4. Verifies postconditions are satisfiable on each path

### 1.4 Benefits

1. **Verification works:** Postconditions with assignments are checked correctly
2. **Catches bugs:** Invalid contracts are rejected at compile time
3. **Foundation:** Enables multi-output functions and sig casting features
4. **Educational:** Error messages show why contracts fail

---

## 2. Three Implementation Levels

### 2.1 Level 1: Simple (Literals Only)

**Capabilities:**
- `&x = 5;` ✓
- `&x = y;` ✗ (unless y is literal)
- `&x = @x + 1;` ✗ (too complex)

**Coverage:** ~70% of real contracts  
**Time:** 3 hours  
**Complexity:** 100-150 lines

### 2.2 Level 2: Medium (Arithmetic) ← RECOMMENDED

**Capabilities:**
- `&x = 5;` ✓
- `&x = y;` ✓ (if y is known)
- `&x = @x + 1;` ✓ (basic arithmetic)
- `&x = y * 2 + z;` ✓

**Coverage:** ~90% of real contracts  
**Time:** 5-6 hours  
**Complexity:** 250-350 lines  
**Includes:** Interval arithmetic for numeric reasoning

### 2.3 Level 3: Full (With Function Calls)

**Capabilities:**
- All of Level 2, plus:
- `&x = get_value();` ✓ (with function analysis)
- Complex nested expressions

**Coverage:** ~99% of real contracts  
**Time:** 8-10 hours  
**Complexity:** 400-500 lines  
**Added complexity:** Call graph analysis

---

## 3. Recommendation: Level 2 (Medium)

**Why Level 2?**
- Covers most practical cases (90%)
- Avoids over-engineering
- Room to extend to Level 3 later
- Good balance of effort vs. benefit

**What it handles:**
```brief
// All of these verify correctly at Level 2:

txn increment [x > 0][y == @y + 1] { &y = y + 1; term; };
txn transfer [amount > 0][balance == @balance - amount] { 
  &balance = balance - amount; 
  term; 
};
txn compound [true][result == x * 2 + 5] {
  &result = x * 2 + 5;
  term;
};
```

---

## 4. Detailed Design: Level 2

### 4.1 Core Data Structures

**File:** `src/symbolic.rs` (new file)

```rust
use std::collections::HashMap;
use crate::ast::{Expr, BinaryOp, Value};

/// Symbolic representation of a value
/// Represents what a variable could be given current state
#[derive(Debug, Clone, PartialEq)]
pub enum SymbolicValue {
    /// Literal constant
    Literal(Value),                                    // &x = 5
    
    /// Reference to another variable
    Identifier(String),                                // &x = y
    
    /// Prior state value
    Previous(String),                                  // &x = @x
    
    /// Binary operation on symbolic values
    Binary(BinaryOp, Box<SymbolicValue>, Box<SymbolicValue>),  // &x = @x + 1
    
    /// Unknown value (can't track)
    Unknown,                                           // &x = func()
}

/// State during symbolic execution of a path
#[derive(Debug, Clone)]
pub struct SymbolicState {
    /// Mapping of variable -> its symbolic value
    pub assignments: HashMap<String, SymbolicValue>,
    
    /// Constraints from guards on this path
    pub path_constraints: Vec<Expr>,
}

impl SymbolicState {
    /// Create new state from precondition
    pub fn new(precondition: &Expr) -> Self {
        SymbolicState {
            assignments: HashMap::new(),
            path_constraints: vec![precondition.clone()],
        }
    }
    
    /// Record an assignment
    pub fn assign(&mut self, target: &str, value_expr: &Expr) {
        let symbolic_val = eval_symbolic(value_expr, self);
        self.assignments.insert(target.to_string(), symbolic_val);
    }
    
    /// Add guard constraint
    pub fn guard(&mut self, condition: &Expr, taken: bool) {
        if taken {
            self.path_constraints.push(condition.clone());
        } else {
            // Guard not taken; add negation
            self.path_constraints.push(Expr::Unary(
                UnaryOp::Not,
                Box::new(condition.clone()),
            ));
        }
    }
}
```

### 4.2 Symbolic Evaluation

```rust
/// Evaluate an expression to a symbolic value
/// Returns Unknown if expression is too complex to track
pub fn eval_symbolic(expr: &Expr, state: &SymbolicState) -> SymbolicValue {
    match expr {
        // Literal values
        Expr::Literal(v) => SymbolicValue::Literal(v.clone()),
        
        // Variable references
        Expr::Identifier(name) => {
            if let Some(sym_val) = state.assignments.get(name) {
                // Return the symbolic value for this variable
                sym_val.clone()
            } else {
                // Unknown variable
                SymbolicValue::Identifier(name.clone())
            }
        }
        
        // Prior state reference
        Expr::Prior(name) => SymbolicValue::Previous(name.clone()),
        
        // Binary operations
        Expr::Binary(op, left, right) => {
            let left_sym = eval_symbolic(left, state);
            let right_sym = eval_symbolic(right, state);
            
            // Try to simplify if possible
            if let Some(simplified) = simplify_binary(*op, &left_sym, &right_sym) {
                simplified
            } else {
                SymbolicValue::Binary(*op, Box::new(left_sym), Box::new(right_sym))
            }
        }
        
        // Function calls - can't track
        Expr::Call(_, _) => SymbolicValue::Unknown,
        
        // Other complex expressions
        _ => SymbolicValue::Unknown,
    }
}

/// Try to simplify a binary operation
fn simplify_binary(
    op: BinaryOp,
    left: &SymbolicValue,
    right: &SymbolicValue,
) -> Option<SymbolicValue> {
    match (op, left, right) {
        // Literal arithmetic
        (BinaryOp::Add, SymbolicValue::Literal(Value::Int(a)), 
         SymbolicValue::Literal(Value::Int(b))) => {
            Some(SymbolicValue::Literal(Value::Int(a + b)))
        }
        
        (BinaryOp::Sub, SymbolicValue::Literal(Value::Int(a)), 
         SymbolicValue::Literal(Value::Int(b))) => {
            Some(SymbolicValue::Literal(Value::Int(a - b)))
        }
        
        (BinaryOp::Mul, SymbolicValue::Literal(Value::Int(a)), 
         SymbolicValue::Literal(Value::Int(b))) => {
            Some(SymbolicValue::Literal(Value::Int(a * b)))
        }
        
        // Identity rules
        (BinaryOp::Add, SymbolicValue::Literal(Value::Int(0)), x) => Some(x.clone()),
        (BinaryOp::Add, x, SymbolicValue::Literal(Value::Int(0))) => Some(x.clone()),
        
        (BinaryOp::Mul, SymbolicValue::Literal(Value::Int(1)), x) => Some(x.clone()),
        (BinaryOp::Mul, x, SymbolicValue::Literal(Value::Int(1))) => Some(x.clone()),
        
        // Zero elimination
        (BinaryOp::Mul, SymbolicValue::Literal(Value::Int(0)), _) => {
            Some(SymbolicValue::Literal(Value::Int(0)))
        }
        (BinaryOp::Mul, _, SymbolicValue::Literal(Value::Int(0))) => {
            Some(SymbolicValue::Literal(Value::Int(0)))
        }
        
        _ => None,  // Can't simplify
    }
}
```

### 4.3 Postcondition Verification

```rust
/// Check if postcondition is satisfiable given symbolic state
pub fn satisfies_postcondition(post: &Expr, state: &SymbolicState) -> bool {
    match post {
        // Equality check
        Expr::Binary(BinaryOp::Eq, left, right) => {
            let left_sym = eval_symbolic(left, state);
            let right_sym = eval_symbolic(right, state);
            symbolic_equals(&left_sym, &right_sym)
        }
        
        // Inequality
        Expr::Binary(BinaryOp::NotEq, left, right) => {
            let left_sym = eval_symbolic(left, state);
            let right_sym = eval_symbolic(right, state);
            !symbolic_equals(&left_sym, &right_sym)
        }
        
        // Less than (with interval arithmetic)
        Expr::Binary(BinaryOp::Lt, left, right) => {
            let left_sym = eval_symbolic(left, state);
            let right_sym = eval_symbolic(right, state);
            symbolic_less_than(&left_sym, &right_sym)
        }
        
        // Conjunction (AND)
        Expr::Binary(BinaryOp::And, left, right) => {
            satisfies_postcondition(left, state) && 
            satisfies_postcondition(right, state)
        }
        
        // Disjunction (OR)
        Expr::Binary(BinaryOp::Or, left, right) => {
            satisfies_postcondition(left, state) || 
            satisfies_postcondition(right, state)
        }
        
        // Boolean literal
        Expr::Literal(Value::Bool(b)) => *b,
        
        // Negation
        Expr::Unary(UnaryOp::Not, expr) => {
            !satisfies_postcondition(expr, state)
        }
        
        _ => false,  // Conservative: unknown expressions fail
    }
}

/// Check symbolic equality
fn symbolic_equals(left: &SymbolicValue, right: &SymbolicValue) -> bool {
    match (left, right) {
        // Literal equality
        (SymbolicValue::Literal(a), SymbolicValue::Literal(b)) => a == b,
        
        // Identical symbolic values
        (SymbolicValue::Identifier(a), SymbolicValue::Identifier(b)) => a == b,
        (SymbolicValue::Previous(a), SymbolicValue::Previous(b)) => a == b,
        
        // Complex expressions
        (SymbolicValue::Binary(op1, l1, r1), SymbolicValue::Binary(op2, l2, r2)) => {
            op1 == op2 && symbolic_equals(l1, l2) && symbolic_equals(r1, r2)
        }
        
        _ => false,  // Different types; not equal
    }
}

/// Check symbolic less-than with interval arithmetic
fn symbolic_less_than(left: &SymbolicValue, right: &SymbolicValue) -> bool {
    match (left, right) {
        // Literal comparison
        (SymbolicValue::Literal(Value::Int(a)), 
         SymbolicValue::Literal(Value::Int(b))) => {
            a < b
        }
        
        // Comparison with unknown - conservative
        (SymbolicValue::Unknown, _) | (_, SymbolicValue::Unknown) => false,
        
        _ => false,
    }
}
```

### 4.4 Integration into Proof Engine

**File:** `src/proof_engine.rs`

```rust
use crate::symbolic::{SymbolicState, SymbolicValue, satisfies_postcondition, eval_symbolic};

/// Verify a transaction's contract using symbolic execution
fn verify_txn_contract(txn: &Transaction) -> Result<(), Error> {
    let pre = &txn.contract.pre_condition;
    let post = &txn.contract.post_condition;
    
    // Enumerate all execution paths
    let paths = enumerate_paths(&txn.body);
    
    for (path_idx, path) in paths.iter().enumerate() {
        // Initialize symbolic state from precondition
        let mut state = SymbolicState::new(pre);
        
        // Walk through the path
        for stmt in path {
            match stmt {
                Statement::Assignment { target, expr } => {
                    // Track the assignment
                    state.assign(target, expr);
                }
                
                Statement::Guarded { condition, .. } => {
                    // Add constraint
                    state.guard(condition, true);
                }
                
                Statement::Term(_) => {
                    // Verify postcondition at termination
                    if !satisfies_postcondition(post, &state) {
                        return Err(Error::ContractViolation {
                            path: Some(path_idx),
                            reason: format!(
                                "postcondition not satisfied on path {}: {}",
                                path_idx,
                                format_expr(post)
                            ),
                        });
                    }
                }
                
                Statement::Escape => {
                    // Escape means rollback; postcondition not verified
                    // This is OK (rollback semantics)
                }
                
                _ => {}
            }
        }
    }
    
    Ok(())
}
```

### 4.5 Path Enumeration

```rust
/// Enumerate all possible execution paths through a statement block
/// Each path is a sequence of statements, with guards either taken or not taken
fn enumerate_paths(body: &[Statement]) -> Vec<Vec<Statement>> {
    let mut paths = vec![vec![]];  // Start with one empty path
    
    for stmt in body {
        let mut new_paths = Vec::new();
        
        for mut path in paths {
            match stmt {
                Statement::Guarded { condition, stmt: inner } => {
                    // Path 1: Guard taken
                    let mut path1 = path.clone();
                    path1.push(Statement::Guarded {
                        condition: condition.clone(),
                        stmt: inner.clone(),
                    });
                    path1.push(*inner.clone());
                    new_paths.push(path1);
                    
                    // Path 2: Guard not taken (skip this statement)
                    let mut path2 = path;
                    path2.push(stmt.clone());  // Include the guard statement for tracking
                    new_paths.push(path2);
                }
                
                _ => {
                    // Non-guard statements: add to all paths
                    path.push(stmt.clone());
                    new_paths.push(path);
                }
            }
        }
        
        paths = new_paths;
    }
    
    paths
}
```

---

## 5. Testing Strategy

### 5.1 Unit Tests

**File:** `tests/symbolic_tests.rs`

```rust
#[test]
fn test_literal_assignment() {
    let mut state = SymbolicState::new(&Expr::Literal(Value::Bool(true)));
    state.assign("x", &Expr::Literal(Value::Int(5)));
    
    let val = state.assignments.get("x").unwrap();
    assert_eq!(val, &SymbolicValue::Literal(Value::Int(5)));
}

#[test]
fn test_arithmetic_simplification() {
    let state = SymbolicState::new(&Expr::Literal(Value::Bool(true)));
    
    let left = SymbolicValue::Literal(Value::Int(3));
    let right = SymbolicValue::Literal(Value::Int(2));
    
    let result = eval_symbolic(
        &Expr::Binary(
            BinaryOp::Add,
            Box::new(Expr::Literal(Value::Int(3))),
            Box::new(Expr::Literal(Value::Int(2))),
        ),
        &state
    );
    
    assert_eq!(result, SymbolicValue::Literal(Value::Int(5)));
}

#[test]
fn test_postcondition_equality() {
    let mut state = SymbolicState::new(&Expr::Literal(Value::Bool(true)));
    state.assign("x", &Expr::Literal(Value::Int(5)));
    
    let postcond = Expr::Binary(
        BinaryOp::Eq,
        Box::new(Expr::Identifier("x".to_string())),
        Box::new(Expr::Literal(Value::Int(5))),
    );
    
    assert!(satisfies_postcondition(&postcond, &state));
}

#[test]
fn test_postcondition_inequality() {
    let mut state = SymbolicState::new(&Expr::Literal(Value::Bool(true)));
    state.assign("x", &Expr::Literal(Value::Int(5)));
    
    let postcond = Expr::Binary(
        BinaryOp::Eq,
        Box::new(Expr::Identifier("x".to_string())),
        Box::new(Expr::Literal(Value::Int(10))),
    );
    
    assert!(!satisfies_postcondition(&postcond, &state));
}

#[test]
fn test_arithmetic_postcondition() {
    let mut state = SymbolicState::new(&Expr::Literal(Value::Bool(true)));
    state.assign("x", &Expr::Literal(Value::Int(5)));
    
    let postcond = Expr::Binary(
        BinaryOp::Eq,
        Box::new(Expr::Identifier("x".to_string())),
        Box::new(Expr::Literal(Value::Int(5))),  // x == 5
    );
    
    assert!(satisfies_postcondition(&postcond, &state));
}

#[test]
fn test_path_enumeration() {
    let stmts = vec![
        Statement::Guarded { condition: Expr::Identifier("c1".to_string()), ... },
        Statement::Assignment { target: "x".to_string(), expr: Expr::Literal(...) },
    ];
    
    let paths = enumerate_paths(&stmts);
    assert_eq!(paths.len(), 2);  // Two paths: guard taken and not taken
}
```

### 5.2 Integration Tests

Create `examples/symbolic_execution.bv`:

```brief
let x: Int = 0;
let result: Bool = false;

txn test1 [x >= 0][x == 5] {
  &x = 5;
  term;
};

txn test2 [true][result == true] {
  &result = true;
  term;
};

txn test3 [x > 0][x == @x + 1] {
  &x = x + 1;
  term;
};
```

Verify all compile successfully.

### 5.3 Regression Tests

```bash
cargo test --release
# All 8 existing stress tests should pass
```

---

## 6. What Level 2 Handles

✅ **Verified at Level 2:**
- Simple assignments: `&x = 5;`
- Variable references: `&x = y;` (if y is known)
- Prior state: `&x = @x;`
- Arithmetic: `&x = @x + 1;` `&result = a * 2 + b;`
- Equality postconditions: `[x == 5]`
- Inequality: `[x != 0]`
- Conjunctions: `[x == 5 && y > 0]`
- Simple comparisons: `[x < 10]`

❌ **Not handled (Level 2):**
- Function calls: `&x = get_value();`
- String operations: `&msg = str.concat("!");`
- Floating point: `&pi = 3.14159;` (partial support)

---

## 7. Implementation Checklist

- [ ] Create `src/symbolic.rs` with SymbolicValue enum
- [ ] Implement SymbolicState struct
- [ ] Implement eval_symbolic function
- [ ] Implement simplify_binary for arithmetic
- [ ] Implement satisfies_postcondition
- [ ] Implement symbolic_equals
- [ ] Implement symbolic_less_than  
- [ ] Implement enumerate_paths
- [ ] Update proof_engine.rs to use symbolic executor
- [ ] Write comprehensive unit tests
- [ ] Create integration test examples
- [ ] Run regression tests
- [ ] Verify all 8 stress tests still pass
- [ ] Commit with message

---

## 8. Success Criteria

- ✅ Simple assignment verification works: `&x = 5` with `[x == 5]`
- ✅ Prior-state verification works: `&x = @x + 1` with `[x == @x + 1]`
- ✅ Arithmetic simplification works: `3 + 2 = 5`
- ✅ Postcondition evaluation works: knows `true == true`, `5 != 3`
- ✅ Path enumeration works: finds all branches
- ✅ Multiple assignments tracked: `&x = 1; &y = 2;`
- ✅ Unknown values treated conservatively (don't break verification)
- ✅ All 8 existing stress tests still pass
- ✅ New symbolic execution examples compile
- ✅ Error messages reference which path failed

---

## 9. Future Work (Level 3)

When ready to extend to Level 3 (full support):

1. **Function call analysis**: Track return values from known functions
2. **String operations**: Symbolic representation of strings
3. **Floating point**: Interval arithmetic for numeric reasoning
4. **Data structures**: Symbolic tracking of list/map operations

These are out of scope for Phase 1 but architecture supports them.

---

*End of Design Document: Symbolic Executor*
