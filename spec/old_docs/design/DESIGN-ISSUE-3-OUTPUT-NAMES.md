# Design Document: Output Variable Name Binding
**Issue:** #3 - Proof Engine Enhancement  
**Date:** 2026-04-05  
**Status:** Ready for Implementation  
**Complexity:** Medium (80-120 lines)  
**Time Estimate:** 2 hours

---

## 1. Overview

### 1.1 Problem Statement

Postconditions currently fail verification when they reference output variable names:

```brief
defn sufficient_funds(amount: Int) [amount > 0][result == true] -> Bool {
  term true;
};
```

**Error:**
```
error[P008]: contract verification failed
= definition 'sufficient_funds': post-condition not satisfied on path 0
= proof:
=   • 1. Pre-condition: amount > 0
=   • 3. Post-condition: result == true
```

The compiler doesn't understand that `result` in the postcondition maps to the Bool value returned by `term true`.

### 1.2 Root Cause

The proof engine tracks postconditions as arbitrary expressions but doesn't track the relationship between:
- Output variable names declared in the defn signature
- The values produced by `term` statements
- References to those names in postconditions

### 1.3 Solution

Implement output variable name binding in the proof engine:
1. Parse and store output variable names from defn declaration
2. Build mapping: output_name → output_slot_index
3. When verifying postcondition, substitute output names with their symbolic values

### 1.4 Benefits

1. **Intuitive postconditions:** Write `[result == true]` instead of `[true]`
2. **Self-documenting:** Postcondition clearly states what's being verified
3. **Type safety:** Output names document what each output slot represents
4. **Correctness:** Enables verification of output-dependent contracts

---

## 2. Semantics

### 2.1 Output Variable Name Syntax

```bnf
# Current (no names):
definition ::= "defn" identifier parameters contract "->" type ("," type)* "{" body "}"

# NEW (with optional names):
definition ::= "defn" identifier parameters contract "->" output_type ("," output_type)* "{" body "}"

output_type ::= (identifier ":")? type
# Examples:
#   Bool               (no name, just type)
#   result: Bool       (with name)
#   Int, success: Bool (mixed)
```

### 2.2 Name Binding Rules

1. **Names are optional:** Both `-> Bool` and `-> result: Bool` are valid
2. **Names are scoped to postconditions:** Only usable in `[post]` condition
3. **Names can be repeated:** `-> Bool, Bool` has two Bools (unnamed)
4. **Names must be unique:** `-> x: Bool, x: Int` is invalid
5. **Names cannot shadow parameters:** `defn f(x: Int) ... -> x: Bool` is invalid
6. **Slot mapping:** First output is slot 0, second is slot 1, etc.

### 2.3 Postcondition Substitution

When verifying postcondition, the proof engine:

1. Identifies all identifiers in postcondition
2. For each identifier:
   - Check if it's in the output_name map
   - If yes: substitute with the value from corresponding term statement
   - If no: treat as variable reference (existing behavior)

**Example:**

Defn declaration:
```brief
defn divide(a: Int, b: Int) [b != 0][result == a / b] -> Int
```

Mapping: `{result: 0}`

Postcondition: `result == a / b`

Substitution: `(value_from_term_stmt) == a / b`

Verification: Use symbolic executor to check this expression is satisfiable

### 2.4 Edge Cases

**Multiple outputs with names:**
```brief
defn pair() [true][first > 0 && second > 0] -> first: Int, second: Int {
  term 10;
  term 20;
};
```

Mapping: `{first: 0, second: 1}`  
Substitution: `(value_from_term_0) > 0 && (value_from_term_1) > 0`

**Mixed named and unnamed:**
```brief
defn complex() -> result: Bool, Int, success: Bool {
  term true;
  term 42;
  term true;
};
```

Mapping: `{result: 0, success: 2}`  
Postcondition can use `result` and `success`, but not refer to slot 1

**No names (backward compat):**
```brief
defn old_style() -> Bool, Int {
  term true;
  term 42;
};
```

Mapping: `{}` (empty)  
Postcondition cannot reference output names

---

## 3. Implementation

### 3.1 AST Changes

**File:** `src/ast.rs`

Current `Definition` struct:
```rust
pub struct Definition {
    pub name: String,
    pub parameters: Vec<(String, Type)>,
    pub output_types: Vec<Type>,
    pub contract: Contract,
    pub body: Vec<Statement>,
}
```

New `Definition` struct:
```rust
pub struct Definition {
    pub name: String,
    pub parameters: Vec<(String, Type)>,
    pub output_types: Vec<Type>,
    pub output_names: Vec<Option<String>>,  // NEW: parallel to output_types
    pub contract: Contract,
    pub body: Vec<Statement>,
}
```

**Rules:**
- `output_names.len() == output_types.len()` (always)
- `output_names[i] == Some(name)` means output slot i has a name
- `output_names[i] == None` means output slot i is unnamed

### 3.2 Parser Changes

**File:** `src/parser.rs`

Current `parse_output_types()` pseudocode:
```rust
fn parse_output_types(&mut self) -> Result<Vec<Type>> {
    let mut types = Vec::new();
    loop {
        types.push(self.parse_type()?);
        if self.peek() != "," { break; }
        self.consume(",")?;
    }
    Ok(types)
}
```

New `parse_output_types()`:
```rust
fn parse_output_types(&mut self) -> Result<(Vec<Type>, Vec<Option<String>>)> {
    let mut types = Vec::new();
    let mut names = Vec::new();
    
    loop {
        // Check for optional name before type
        let name = if self.peek_ahead_two() == ("identifier", ":") {
            let n = self.advance().unwrap_text().to_string();
            self.consume(":")?;
            
            // Validation: name not a parameter
            if self.is_parameter(&n) {
                return Err(ParseError::OutputNameShadowsParameter(n));
            }
            
            Some(n)
        } else {
            None
        };
        
        types.push(self.parse_type()?);
        names.push(name);
        
        if self.peek() != "," { break; }
        self.consume(",")?;
    }
    
    // Validation: output names are unique
    let mut seen = HashSet::new();
    for name in names.iter().filter_map(|n| n.as_ref()) {
        if seen.contains(name) {
            return Err(ParseError::DuplicateOutputName(name.clone()));
        }
        seen.insert(name.clone());
    }
    
    Ok((types, names))
}
```

Integration into `parse_definition()`:
```rust
fn parse_definition(&mut self) -> Result<Definition> {
    self.consume("defn")?;
    let name = self.parse_identifier()?;
    let parameters = self.parse_parameters()?;
    let contract = self.parse_contract()?;
    self.consume("->")?;
    
    // NEW: Returns tuple
    let (output_types, output_names) = self.parse_output_types()?;
    
    self.consume("{")?;
    let body = self.parse_body()?;
    self.consume("}")?;
    
    Ok(Definition {
        name,
        parameters,
        output_types,
        output_names,  // NEW
        contract,
        body,
    })
}
```

### 3.3 Proof Engine Changes

**File:** `src/proof_engine.rs`

New helper function:
```rust
fn build_output_name_map(defn: &Definition) -> HashMap<String, usize> {
    let mut map = HashMap::new();
    
    for (index, name) in defn.output_names.iter().enumerate() {
        if let Some(n) = name {
            map.insert(n.clone(), index);
        }
    }
    
    map
}
```

Updated `verify_definition()`:
```rust
fn verify_definition(defn: &Definition) -> Result<(), Error> {
    let pre = &defn.contract.pre_condition;
    let post = &defn.contract.post_condition;
    
    // NEW: Build output name mapping
    let output_names = build_output_name_map(defn);
    
    // Enumerate paths
    let paths = enumerate_paths(&defn.body);
    
    for path in paths {
        // Initialize symbolic state
        let mut state = SymbolicState::new(pre);
        
        // Walk through path
        for stmt in &path {
            match stmt {
                Statement::Assignment { target, expr } => {
                    state.assign(target, expr);
                }
                Statement::Guarded { condition, .. } => {
                    state.guard(condition, true);
                }
                Statement::Term(outputs) => {
                    // NEW: Collect term values
                    let term_values = collect_term_values(outputs, &defn.output_types);
                    
                    // NEW: Substitute output names in postcondition
                    let substituted_post = 
                        substitute_output_names(post, &output_names, &term_values);
                    
                    // Verify substituted postcondition
                    if !satisfies_postcondition(&substituted_post, &state) {
                        return Err(Error::ContractViolation {
                            path: path.clone(),
                            reason: format!(
                                "postcondition not satisfied: {}",
                                format_expr(&substituted_post)
                            ),
                        });
                    }
                }
                _ => {}
            }
        }
    }
    
    Ok(())
}

fn collect_term_values(
    outputs: &[Option<Expr>],
    output_types: &[Type],
) -> Vec<SymbolicValue> {
    outputs
        .iter()
        .zip(output_types.iter())
        .map(|(expr_opt, _ty)| {
            match expr_opt {
                Some(expr) => eval_symbolic(expr),
                None => SymbolicValue::Unknown,
            }
        })
        .collect()
}

fn substitute_output_names(
    expr: &Expr,
    output_names: &HashMap<String, usize>,
    term_values: &[SymbolicValue],
) -> Expr {
    match expr {
        Expr::Identifier(name) => {
            if let Some(index) = output_names.get(name) {
                // Replace with term value
                symbolic_value_to_expr(&term_values[*index])
            } else {
                expr.clone()
            }
        }
        Expr::Binary(op, left, right) => {
            Expr::Binary(
                *op,
                Box::new(substitute_output_names(left, output_names, term_values)),
                Box::new(substitute_output_names(right, output_names, term_values)),
            )
        }
        // Recursively handle other expression types
        _ => expr.clone(),
    }
}

fn symbolic_value_to_expr(val: &SymbolicValue) -> Expr {
    match val {
        SymbolicValue::Literal(v) => Expr::Literal(v.clone()),
        SymbolicValue::Identifier(name) => Expr::Identifier(name.clone()),
        SymbolicValue::Previous(name) => Expr::Prior(name.clone()),
        // For complex symbolic values, convert to approximate expression
        _ => Expr::Identifier("_unknown".to_string()),
    }
}
```

---

## 4. Testing Strategy

### 4.1 Unit Tests

**File:** `tests/proof_engine_tests.rs`

```rust
#[test]
fn test_output_name_binding_simple() {
    let code = r#"
        defn test() [true][result == true] -> Bool {
            term true;
        };
    "#;
    let result = compile(code);
    assert!(result.is_ok(), "Output name binding should work");
}

#[test]
fn test_output_name_multiple() {
    let code = r#"
        defn pair() [true][first > 0 && second < 10] -> first: Int, second: Int {
            term 5;
            term 3;
        };
    "#;
    let result = compile(code);
    assert!(result.is_ok());
}

#[test]
fn test_output_name_mixed() {
    let code = r#"
        defn mixed() [true][x == 10] -> Int, x: Int {
            term 5;
            term 10;
        };
    "#;
    let result = compile(code);
    assert!(result.is_ok());
}

#[test]
fn test_output_name_shadows_parameter_error() {
    let code = r#"
        defn bad(x: Int) [true][true] -> x: Bool {
            term true;
        };
    "#;
    let result = compile(code);
    assert!(result.is_err(), "Output name cannot shadow parameter");
}

#[test]
fn test_output_name_duplicate_error() {
    let code = r#"
        defn bad() [true][true] -> x: Bool, x: Int {
            term true;
            term 5;
        };
    "#;
    let result = compile(code);
    assert!(result.is_err(), "Output names must be unique");
}

#[test]
fn test_backward_compat_no_names() {
    let code = r#"
        defn old() [true][true] -> Bool, Int {
            term true;
            term 42;
        };
    "#;
    let result = compile(code);
    assert!(result.is_ok(), "No-name syntax still works");
}

#[test]
fn test_output_name_in_complex_postcondition() {
    let code = r#"
        defn complex() [x > 0][result == x + 1] -> result: Int {
            term x + 1;
        };
    "#;
    let result = compile(code);
    // Should verify result (from term) equals x+1
    assert!(result.is_ok());
}
```

### 4.2 Integration Tests

Create `examples/output_names.bv`:

```brief
defn divide(a: Int, b: Int) [b != 0][result == a / b] -> result: Int {
  let result: Int = a / b;
  term result;
};

defn pair() [true][first > 0 && second > 0] -> first: Int, second: Int {
  term 10;
  term 20;
};

defn status(id: Int) [true][success == (id > 0)] -> success: Bool {
  [id > 0] term true;
  [id <= 0] term false;
};

txn use_output_names [true][true] {
  let x = divide(10, 2);
  let (f, s) = pair();
  let ok = status(1);
  term;
};
```

### 4.3 Regression Tests

```bash
cargo test --release
# All 8 existing stress tests should still pass
```

---

## 5. Edge Cases

### 5.1 No Output Names (Backward Compat)

```brief
defn old_style() -> Bool, Int {
  term true;
  term 42;
};
```

✅ Works as before; postcondition cannot use output names

### 5.2 Output Name Not Used in Postcondition

```brief
defn unused_name() [true][true] -> result: Bool {
  term true;
};
```

✅ Name is declared but not used; that's OK

### 5.3 Reference to Non-Output Name

```brief
defn test() [true][foo == 5] -> result: Bool {  // foo is not an output
  term true;
};
```

✅ `foo` is treated as variable reference, not output name

### 5.4 Output Name in Nested Expression

```brief
defn nested() [true][(result + 1) > 5] -> result: Int {
  term 10;
};
```

✅ Substitution handles nested expressions

---

## 6. Interaction with Multi-Output Feature

This feature is a **prerequisite** for multi-output functions (Feature A). When Feature A is implemented, output names enable:

```brief
defn safe_fetch(url: String) -> json: JSON | Bool {
  // Can now reference json in postcondition
};
```

---

## 7. Implementation Checklist

- [ ] Update `Definition` struct to include `output_names`
- [ ] Update parser to accept optional names before types
- [ ] Add validation for name uniqueness
- [ ] Add validation for parameter shadowing
- [ ] Implement `build_output_name_map()` helper
- [ ] Update `verify_definition()` to use name mapping
- [ ] Implement `substitute_output_names()` function
- [ ] Handle substitution in proof engine verification
- [ ] Write unit tests for all cases
- [ ] Create `examples/output_names.bv`
- [ ] Run regression tests
- [ ] Verify all 8 stress tests still pass
- [ ] Commit with message

---

## 8. Success Criteria

- ✅ Optional names in output declarations parse correctly
- ✅ `result == true` in postcondition verifies with `term true`
- ✅ Multiple output names work: `first: Int, second: Int`
- ✅ Mixed named/unnamed outputs work
- ✅ Duplicate output names produce error
- ✅ Output names shadowing parameters produce error
- ✅ Backward compatibility: no-name syntax still works
- ✅ Substitution happens before proof verification
- ✅ All 8 existing stress tests still pass
- ✅ New `examples/output_names.bv` compiles

---

*End of Design Document: Output Variable Name Binding*
