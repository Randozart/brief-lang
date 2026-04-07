# Design Document: Sig Casting with Type Projection
**Feature:** Feature B - Sig Casting  
**Date:** 2026-04-05  
**Prerequisite:** Feature A (Multi-Output Types)  
**Complexity:** Medium (100-150 lines)  
**Time Estimate:** 2 hours

---

## 1. Overview

### 1.1 Problem Statement

With multi-output functions, callers must handle all declared types. But sometimes you only need one type from a union:

```brief
defn api_call() -> JSON | Error {
  term json;  // or Error
};

// Caller forced to handle both:
let json: JSON;
let error: Error = api_call();  // Exhaustive

// But we only care about JSON!
```

### 1.2 Solution

Use **sig casting** to project specific output types:

```brief
sig json_only: void -> JSON;

// Now we can guarantee JSON output and reject the Error path
let data = json_only();  // Only gets JSON, Error is filtered
```

The compiler verifies that the projected type is **actually producible** by at least one execution path.

### 1.3 Benefits

1. **Selective handling:** Extract only needed types from unions
2. **Type safety:** Compiler verifies projection is valid
3. **Polymorphism:** One function serves multiple interfaces
4. **Clarity:** Sig name documents the intent (e.g., `json_only` vs `error_handler`)

---

## 2. Semantics

### 2.1 Sig Casting Syntax

```bnf
signature ::= "sig" identifier ":" type_spec "->" result_type ";"

# Examples:
sig json_data: String -> JSON;           # Project JSON from union
sig both_outputs: void -> Bool, Int;     # Project tuple
sig status: Int -> Bool | String;        # Project union (if defn allows)
```

### 2.2 Projection Rules

**Type Projection:** Extract specific type from union

```brief
defn complex() -> Bool | String, Int {
  [ok] { term true; term 42; };
  [fail] { term "error"; term 0; };
};

sig status_bool: void -> Bool;           // Takes Bool from slot 0 union
sig status_msg: void -> String;          // Takes String from slot 0 union
sig status_code: void -> Int;            // Takes Int from slot 1
sig status_tuple: void -> Bool, Int;     // Takes both slots 0 and 1
```

**Verification Rule:** For each declared output type in sig:
- At least one execution path in defn must produce that type
- All other paths can produce different types (will be ignored)

### 2.3 Sig Casting at Call Site

```brief
let data = json_only();  // Implicitly casts to sig -> JSON

// Only one type bound:
let json: JSON = json_only();
```

When a defn is called as a sig:
1. Compiler validates sig projection is achievable
2. Runtime executes defn
3. Result is projected to requested type
4. Other outcomes are discarded

---

## 3. Implementation

### 3.1 Type Checker Changes

**File:** `src/typechecker.rs`

```rust
fn verify_sig_casting(
    sig: &Signature,
    defn: &Definition,
) -> Result<(), Error> {
    // Verify: Can defn produce sig's output type?
    
    let paths = enumerate_paths(&defn.body);
    let sig_output_type = &sig.output_type;
    
    for path in paths {
        // What type does this path produce?
        let produced = get_path_output_type(&path, &defn.output_types);
        
        // Does it match sig requirement?
        if type_matches(&produced, sig_output_type) {
            return Ok(());  // Found satisfying path
        }
    }
    
    Err(Error::SigCastingFailed {
        sig: sig.name.clone(),
        requested: sig_output_type.clone(),
        available: defn.output_types.clone(),
    })
}

fn get_path_output_type(
    path: &[Statement],
    declared: &OutputType,
) -> Type {
    // Walk path, find what type the term statement produces
    for stmt in path {
        if let Statement::Term(outputs) = stmt {
            if !outputs.is_empty() {
                if let Some(expr) = &outputs[0] {
                    return infer_type(expr);
                }
            }
        }
    }
    Type::Void
}

fn type_matches(produced: &Type, required: &OutputType) -> bool {
    match (produced, required) {
        (t, OutputType::Single(req)) => t == req,
        (t, OutputType::Union(types)) => types.contains(t),
        _ => false,
    }
}
```

### 3.2 Proof Engine Integration

**File:** `src/proof_engine.rs`

Register sig-to-defn mappings and verify them:

```rust
fn verify_all_sig_casts(program: &Program) -> Result<(), Error> {
    for sig in &program.signatures {
        if let Some(defn) = find_defn_for_sig(program, sig) {
            verify_sig_casting(sig, defn)?;
        }
    }
    Ok(())
}

fn find_defn_for_sig(program: &Program, sig: &Signature) -> Option<Definition> {
    // Find defn with matching name
    program.items.iter().find_map(|item| {
        if let TopLevel::Definition(defn) = item {
            if defn.name == sig.name {
                return Some(defn.clone());
            }
        }
        None
    })
}
```

### 3.3 Runtime Type Projection

**File:** `src/interpreter.rs`

```rust
fn project_to_type(
    value: Value,
    target_type: &Type,
) -> Result<Value, RuntimeError> {
    match (value, target_type) {
        // Direct match
        (v, t) if v.get_type() == t => Ok(v),
        
        // Union projection
        (Value::Union(actual, val), target) => {
            if actual == target {
                Ok(*val)
            } else {
                Err(RuntimeError::TypeMismatch {
                    expected: target.clone(),
                    got: actual,
                })
            }
        }
        
        // Mismatch
        (v, t) => {
            Err(RuntimeError::TypeMismatch {
                expected: t.clone(),
                got: v.get_type().clone(),
            })
        }
    }
}
```

---

## 4. Testing Strategy

### 4.1 Unit Tests

**File:** `tests/sig_casting_tests.rs`

```rust
#[test]
fn test_union_projection_valid() {
    let code = r#"
        defn get() -> Bool | String { term true; };
        sig bool_only: void -> Bool;
    "#;
    let result = compile(code);
    assert!(result.is_ok());
}

#[test]
fn test_tuple_projection_partial() {
    let code = r#"
        defn pair() -> Bool, Int, String { 
            term true; 
            term 42; 
            term "msg"; 
        };
        sig first_two: void -> Bool, Int;
    "#;
    let result = compile(code);
    assert!(result.is_ok());
}

#[test]
fn test_invalid_projection() {
    let code = r#"
        defn get() -> Bool | String { term true; };
        sig get_int: void -> Int;  // Int not producible
    "#;
    let result = compile(code);
    assert!(result.is_err());
}

#[test]
fn test_projection_multiple_paths() {
    let code = r#"
        defn multi() -> Bool | Error {
            [ok] term true;
            [fail] term Error("msg");
        };
        sig success_only: void -> Bool;
    "#;
    let result = compile(code);
    assert!(result.is_ok());
}
```

### 4.2 Integration Tests

Create `examples/sig_casting.bv`:

```brief
defn fetch_data(url: String) -> JSON | Error {
  [valid] term parse_json(url);
  [invalid] term Error("Bad URL");
};

sig json_safe: String -> JSON;
sig error_handler: String -> Error;

txn load [true][data_loaded] {
  let data = json_safe("https://api.example.com");
  &data_loaded = true;
  term;
};
```

---

## 5. Error Messages

### Invalid Projection

```
[E005] Sig casting failed for 'get_int'

Function 'get_data' declares:
  -> Bool | String

Sig 'get_int' requests:
  -> Int

Problem:
  Type Int is not producible by any path in get_data

To fix:
  Either:
  1. Change sig to valid type: sig get_int: void -> Bool;
  2. Change defn to produce Int
  3. Use different defn that produces Int
```

---

## 6. Implementation Checklist

- [ ] Implement `verify_sig_casting()` in proof engine
- [ ] Implement `get_path_output_type()` helper
- [ ] Implement `type_matches()` for projections
- [ ] Add sig-to-defn mapping in program verification
- [ ] Implement runtime `project_to_type()` in interpreter
- [ ] Write comprehensive unit tests
- [ ] Create integration test examples
- [ ] Update error messages
- [ ] Verify with existing tests
- [ ] Commit with message

---

## 7. Success Criteria

- ✅ Union type projection works
- ✅ Tuple partial projection works
- ✅ Invalid projections rejected with clear error
- ✅ Multiple paths to requested type works
- ✅ Sig casting at call site works
- ✅ All 8 existing stress tests still pass

---

*End of Design Document: Sig Casting*
