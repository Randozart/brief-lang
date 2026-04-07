# Design Document: Multi-Output Functions with Union Exhaustiveness
**Feature:** Feature A - Multi-Output Type Declaration  
**Date:** 2026-04-05  
**Status:** Ready for Implementation  
**Complexity:** Medium (150-200 lines across 4 files)  
**Time Estimate:** 3 hours

---

## 1. Overview

### 1.1 Problem Statement

Currently, defn can only return a single output type:

```brief
defn get_status(id: Int) -> Bool {
  [id > 0] term true;
  [id <= 0] term false;
};
```

This forces callers to handle single outcomes. Real functions often have multiple possible results:
- Success/failure pairs
- Multiple data slots
- Multiple interpretations of the same result

### 1.2 Solution

Enable defn to declare **multiple output types** that callers **must handle exhaustively**:

```brief
// Union: can produce any ONE of these
defn fetch(url: String) -> JSON | Error {
  [valid_url(url)] term fetch_json(url);
  [~valid_url(url)] term Error("Invalid URL");
};

// Tuple: produces ALL of these  
defn process() -> Bool, String, Int {
  term true;
  term "success";
  term 200;
};

// Mixed: union in first slot, then tuple
defn complex() -> Bool | String, Int {
  [success] { term true; term 42; };
  [failure] { term "error"; term 0; };
};
```

### 1.3 Benefits

1. **Exhaustiveness checking:** Compiler forces handling all cases
2. **Type safety:** Union types prevent accidental misuse
3. **Polymorphism:** One function serves multiple purposes
4. **Clarity:** Output types document what the function can produce

---

## 2. Semantics

### 2.1 Output Type Declarations

```bnf
output_types ::= output_union ("," output_union)*
output_union ::= type ("|" type)*

# Examples:
-> Bool                          # Single output
-> Bool | Error                  # Union: one type
-> Bool, String, Int             # Tuple: three types in sequence
-> Bool | String, Int            # Mixed: (Bool or String), then Int
```

**Precedence:** Comma binds tighter than pipe (comma groups first)
- `A | B, C` parses as `(A | B), C` (not `A | (B, C)`)

### 2.2 Union vs Tuple Semantics

**Union (`|`):**
- Execution produces exactly ONE of these types
- Caller must handle ALL possible types
- Caller doesn't know which until runtime

```brief
defn choose(x: Int) -> Bool | String {
  [x > 0] term true;
  [x <= 0] term "negative";
};

// MUST bind both Bool and String:
let b: Bool;
let s: String = choose(5);

// INVALID - missing String:
let result: Bool = choose(5);  // Compiler error
```

**Tuple (`,`):**
- Execution produces ALL of these types in sequence
- Multiple `term` statements fill slots
- Caller must bind variables for all slots

```brief
defn pair() -> Bool, String {
  term true;      // Fills slot 0 (Bool)
  term "message"; // Fills slot 1 (String)
};

// MUST bind both slots:
let (b, s) = pair();

// INVALID - missing slot 1:
let b: Bool = pair();  // Compiler error
```

### 2.3 Union Exhaustiveness

When a defn returns a union type, the **caller must handle all outcomes**:

```brief
defn api_call() -> JSON | Error | Timeout {
  [success] term parse_json(...);
  [error] term Error(...);
  [timeout] term Timeout(...);
};

// VALID - handles all three:
let json: JSON;
let error: Error;
let timeout: Timeout = api_call();

// INVALID - missing Timeout:
let json: JSON;
let error: Error = api_call();  // Compiler error: Missing Timeout
```

**Compiler error message:**
```
[E003] Incomplete union handling for 'api_call'

Function declares:
  -> JSON | Error | Timeout

Your code handles:
  ✓ JSON
  ✓ Error
  ✗ Timeout (missing)

To fix:
  Add binding for Timeout type:
  let timeout: Timeout = api_call();
```

### 2.4 Output Variable Names

Multi-output functions support optional names (from Issue #3):

```brief
defn safe_divide(a: Int, b: Int)
  [b != 0]
  [success == true && result == a / b]
  -> success: Bool, result: Int
{
  term true;
  term a / b;
};
```

The postcondition can reference `success` and `result` to verify outputs.

---

## 3. Implementation

### 3.1 AST Changes

**File:** `src/ast.rs`

```rust
#[derive(Debug, Clone)]
pub enum OutputType {
    Single(Type),                          // Bool
    Union(Vec<Type>),                      // Bool | String | Int
    Tuple(Vec<OutputType>),                // (Bool | String), Int
}

pub struct Definition {
    pub name: String,
    pub parameters: Vec<(String, Type)>,
    pub output_types: OutputType,          // NEW: replaces Vec<Type>
    pub output_names: Vec<Option<String>>, // from Issue #3
    pub contract: Contract,
    pub body: Vec<Statement>,
}
```

### 3.2 Parser Changes

**File:** `src/parser.rs`

```rust
fn parse_output_types(&mut self) -> Result<OutputType> {
    let first = parse_single_output()?;
    
    // Check for union (|) or tuple (,)
    if self.peek() == "|" {
        // Union type
        let mut types = vec![first];
        while self.peek() == "|" {
            self.consume("|")?;
            types.push(parse_single_output()?);
        }
        
        // Check if there's a comma after (mixed type)
        if self.peek() == "," {
            let mut tuple_elements = vec![OutputType::Union(types)];
            while self.peek() == "," {
                self.consume(",")?;
                tuple_elements.push(parse_output_types()?);
            }
            Ok(OutputType::Tuple(tuple_elements))
        } else {
            Ok(OutputType::Union(types))
        }
    } else if self.peek() == "," {
        // Tuple type
        let mut elements = vec![OutputType::Single(first)];
        while self.peek() == "," {
            self.consume(",")?;
            elements.push(parse_output_types()?);
        }
        Ok(OutputType::Tuple(elements))
    } else {
        Ok(OutputType::Single(first))
    }
}

fn parse_single_output(&mut self) -> Result<Type> {
    // Check for optional name
    let _name = if self.peek_ahead_two() == ("identifier", ":") {
        let n = self.advance().unwrap_text().to_string();
        self.consume(":")?;
        Some(n)
    } else {
        None
    };
    
    parse_type()
}
```

### 3.3 Type Checker Changes

**File:** `src/typechecker.rs`

```rust
fn check_defn_call(
    defn: &Definition,
    call_context: &CallContext,
) -> Result<(), Error> {
    match &defn.output_types {
        OutputType::Single(ty) => {
            // Single output: caller must bind to matching type
            if !call_context.binding_matches(ty) {
                return Err(TypeError::OutputTypeMismatch {
                    expected: ty.clone(),
                    got: call_context.binding_type().clone(),
                });
            }
        }
        
        OutputType::Union(types) => {
            // Union: caller must bind ALL types
            for ty in types {
                if !call_context.binds_type(ty) {
                    return Err(TypeError::IncompleteUnionHandling {
                        missing: ty.clone(),
                        declared: types.clone(),
                    });
                }
            }
        }
        
        OutputType::Tuple(tuple_types) => {
            // Tuple: must bind all slots
            if call_context.binding_count() != tuple_types.len() {
                return Err(TypeError::TupleArityMismatch {
                    expected: tuple_types.len(),
                    got: call_context.binding_count(),
                });
            }
            
            // Check each slot matches expected type
            for (i, (binding, expected)) in call_context
                .bindings()
                .iter()
                .zip(tuple_types.iter())
                .enumerate()
            {
                let binding_type = binding.get_type();
                if !type_matches(binding_type, expected) {
                    return Err(TypeError::OutputSlotTypeMismatch {
                        slot: i,
                        expected: expected.clone(),
                        got: binding_type.clone(),
                    });
                }
            }
        }
    }
    
    Ok(())
}
```

### 3.4 Proof Engine Changes

**File:** `src/proof_engine.rs`

Verify all output types are reachable:

```rust
fn verify_all_outputs_reachable(defn: &Definition) -> Result<(), Error> {
    let paths = enumerate_paths(&defn.body);
    
    match &defn.output_types {
        OutputType::Single(_) => {
            // At least one path must reach term
            if paths.iter().all(|p| !path_reaches_term(p)) {
                return Err(Error::NoTerminationPath);
            }
        }
        
        OutputType::Union(types) => {
            // Each type must be producible by at least one path
            for ty in types {
                let reachable = paths.iter().any(|p| {
                    path_produces_type(p, ty)
                });
                
                if !reachable {
                    return Err(Error::UnreachableOutputType(ty.clone()));
                }
            }
        }
        
        OutputType::Tuple(tuple_types) => {
            // All slots must be fillable
            // (More complex - requires tracking term value types)
            for (idx, slot_type) in tuple_types.iter().enumerate() {
                let reachable = paths.iter().any(|p| {
                    path_fills_slot(p, idx, slot_type)
                });
                
                if !reachable {
                    return Err(Error::UnreachableOutputSlot {
                        slot: idx,
                        type_: slot_type.clone(),
                    });
                }
            }
        }
    }
    
    Ok(())
}
```

---

## 4. Testing Strategy

### 4.1 Unit Tests

**File:** `tests/multi_output_tests.rs`

```rust
#[test]
fn test_single_output() {
    let code = r#"
        defn get() [true][true] -> Bool {
            term true;
        };
    "#;
    let result = compile(code);
    assert!(result.is_ok());
}

#[test]
fn test_union_output() {
    let code = r#"
        defn get() [true][true] -> Bool | String {
            term true;
        };
    "#;
    let result = compile(code);
    assert!(result.is_ok());
}

#[test]
fn test_tuple_output() {
    let code = r#"
        defn pair() [true][true] -> Bool, Int {
            term true;
            term 42;
        };
    "#;
    let result = compile(code);
    assert!(result.is_ok());
}

#[test]
fn test_mixed_output() {
    let code = r#"
        defn complex() [true][true] -> Bool | String, Int {
            term true;
            term 42;
        };
    "#;
    let result = compile(code);
    assert!(result.is_ok());
}

#[test]
fn test_union_exhaustiveness_error() {
    let code = r#"
        defn get() -> Bool | String { term true; };
        txn test [true][true] {
            let b: Bool = get();
            term;
        };
    "#;
    let result = compile(code);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), CompileError::IncompleteUnionHandling(_)));
}

#[test]
fn test_tuple_arity_error() {
    let code = r#"
        defn pair() -> Bool, Int { term true; term 42; };
        txn test [true][true] {
            let b: Bool = pair();
            term;
        };
    "#;
    let result = compile(code);
    assert!(result.is_err());
}

#[test]
fn test_union_handled_correctly() {
    let code = r#"
        defn get() -> Bool | String { term true; };
        txn test [true][true] {
            let b: Bool;
            let s: String = get();
            term;
        };
    "#;
    let result = compile(code);
    assert!(result.is_ok());
}
```

### 4.2 Integration Tests

Create `examples/multi_output.bv`:

```brief
let json: JSON;
let error: String;

defn fetch_api(url: String) -> JSON | String {
  [valid_url(url)] term parse_json(url);
  [~valid_url(url)] term "Invalid URL";
};

txn load_data [true][true] {
  let (j, e) = fetch_api("https://api.example.com");
  &json = j;
  &error = e;
  term;
};
```

---

## 5. Error Messages

### Union Exhaustiveness

```
[E003] Incomplete union handling for 'fetch_api'

Function 'fetch_api' declares:
  -> JSON | Error | Timeout

Your code handles:
  ✓ JSON
  ✓ Error
  ✗ Timeout (missing)

To fix:
  Add handler for Timeout type:
  let timeout: Timeout = fetch_api(...);
```

### Tuple Arity

```
[E004] Output arity mismatch for 'pair'

Function 'pair' declares:
  -> Bool, Int, String (3 outputs)

Your code binds:
  2 variables

To fix:
  Bind all 3 outputs:
  let (b, i, s) = pair();
```

---

## 6. Implementation Checklist

- [ ] Update OutputType enum in AST
- [ ] Update Definition struct with OutputType
- [ ] Implement OutputType parsing (union vs tuple precedence)
- [ ] Implement type checking for union exhaustiveness
- [ ] Implement type checking for tuple arity
- [ ] Implement verification that all outputs reachable
- [ ] Write union exhaustiveness unit tests
- [ ] Write tuple arity unit tests
- [ ] Create integration test example
- [ ] Verify with existing stress tests
- [ ] Update error messages
- [ ] Commit with message

---

## 7. Success Criteria

- ✅ Single output still works (backward compat)
- ✅ Union types parse correctly
- ✅ Tuple types parse correctly
- ✅ Mixed types parse with correct precedence
- ✅ Union exhaustiveness checking works
- ✅ Tuple arity checking works
- ✅ Compiler rejects incomplete union handling
- ✅ Compiler rejects arity mismatches
- ✅ Error messages clearly explain what's missing
- ✅ All 8 existing stress tests still pass

---

*End of Design Document: Multi-Output Functions*
