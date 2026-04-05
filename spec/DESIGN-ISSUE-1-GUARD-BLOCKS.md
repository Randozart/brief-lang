# Design Document: Guard Block Syntax `[c] { statements }`
**Issue:** #1 - Parser Enhancement  
**Date:** 2026-04-05  
**Status:** Ready for Implementation  
**Complexity:** Low (20-30 lines)  
**Time Estimate:** 30 minutes

---

## 1. Overview

### 1.1 Problem Statement

Brief currently supports only flat guard syntax:
```brief
[condition] statement;
```

This works for single statements but becomes verbose for multiple related operations:
```brief
[amount > 100] &large_transfers = large_transfers + 1;
[amount > 100] &large_total = large_total + amount;
[amount > 100] &large_count = large_count + 1;
```

### 1.2 Proposed Solution

Add block guard syntax that allows multiple statements in braces:
```brief
[amount > 100] {
  &large_transfers = large_transfers + 1;
  &large_total = large_total + amount;
  &large_count = large_count + 1;
};
```

Both syntaxes should be valid and equivalent. Users choose based on readability.

### 1.3 Benefits

1. **Readability**: Guards that protect multiple statements are visually grouped
2. **Maintainability**: Clear relationship between guard condition and actions
3. **LLM-friendly**: More intuitive for AI-assisted development (looks like C/Rust)
4. **Backward Compatible**: Flat syntax still works; this is purely additive

---

## 2. Semantics

### 2.1 Grammar Changes

```bnf
# CURRENT:
guarded_stmt ::= "[" expression "]" statement

# NEW:
statement ::= ... | guarded_stmt | guarded_block | ...

guarded_stmt ::= "[" expression "]" statement
guarded_block ::= "[" expression "]" "{" statement* "}"
```

### 2.2 Execution Semantics

Both syntaxes are semantically identical:

**Flat syntax:**
```brief
[condition] stmt1;
[condition] stmt2;
```

**Block syntax:**
```brief
[condition] {
  stmt1;
  stmt2;
};
```

**Equivalence:** The compiler desugars both to the same AST.

### 2.3 Nesting and Combinations

Blocks can contain any statements, including more guards:

```brief
[x > 0] {
  &counter = counter + 1;
  [counter > 10] {
    &large_count = large_count + 1;
  };
};
```

Multiple guards in sequence:
```brief
[x > 0] { &a = a + 1; };
[x < 0] { &b = b + 1; };
[x == 0] { &c = c + 1; };
```

### 2.4 Edge Cases

**Empty block:** Not allowed (compiler error)
```brief
[x > 0] { };  // ERROR: Empty guarded block
```

**Nested identical guards:** Allowed but redundant
```brief
[x > 0] {
  [x > 0] &a = 1;  // Redundant inner guard, allowed
};
```

**Guard with term statement:** Allowed
```brief
[success] {
  &done = true;
  term;
};
```

---

## 3. Implementation

### 3.1 Current Parser Structure

**File:** `src/parser.rs`

Current `parse_statement()` pseudocode:
```rust
fn parse_statement(&mut self) -> Result<Statement> {
    if self.peek() == "[" {
        self.advance();  // consume [
        let condition = self.parse_expr()?;
        self.consume("]")?;
        
        let stmt = self.parse_statement()?;  // Recursively parse next statement
        return Ok(Statement::Guarded {
            condition,
            stmt: Box::new(stmt),
        });
    }
    
    // ... other statement types ...
}
```

### 3.2 Required Changes

**Step 1: Check current AST representation**

In `src/ast.rs`, find the `Statement` enum:
```rust
pub enum Statement {
    Assignment { target: String, value: Expr },
    Guarded { condition: Expr, stmt: Box<Statement> },
    Term(Vec<Option<Expr>>),
    Escape(Option<Expr>),
    // ...
}
```

**Option A:** Extend existing `Guarded` to support multiple statements
```rust
pub enum Statement {
    // ...
    Guarded { 
        condition: Expr, 
        stmts: Vec<Statement>,  // Changed from single stmt to vec
    },
    // ...
}
```

**Option B:** Add new `GuardedBlock` variant
```rust
pub enum Statement {
    // ...
    Guarded { condition: Expr, stmt: Box<Statement> },
    GuardedBlock { condition: Expr, stmts: Vec<Statement> },
    // ...
}
```

**Recommendation:** Use Option A (simpler, less duplication)

**Step 2: Update parser to handle both syntaxes**

```rust
fn parse_statement(&mut self) -> Result<Statement> {
    if self.peek() == "[" {
        self.advance();  // consume [
        let condition = self.parse_expr()?;
        self.consume("]")?;
        
        // NEW: Check for block vs flat
        if self.peek() == "{" {
            // Block guard
            self.advance();  // consume {
            let mut stmts = Vec::new();
            
            while self.peek() != "}" {
                stmts.push(self.parse_statement()?);
            }
            
            if stmts.is_empty() {
                return Err(ParseError::EmptyGuardedBlock);
            }
            
            self.consume("}")?;
            
            return Ok(Statement::Guarded {
                condition,
                stmts,  // Flat vec of statements
            });
        } else {
            // Flat guard - convert to vec with single statement
            let stmt = self.parse_statement()?;
            return Ok(Statement::Guarded {
                condition,
                stmts: vec![stmt],  // Wrap in vec
            });
        }
    }
    
    // ... rest of parser ...
}
```

**Step 3: Update AST to use `Vec<Statement>`**

```rust
pub enum Statement {
    Assignment { target: String, value: Expr },
    Guarded { 
        condition: Expr, 
        stmts: Vec<Statement>,
    },
    Term(Vec<Option<Expr>>),
    Escape(Option<Expr>),
    // ...
}
```

**Step 4: Update desugarer/interpreter**

The desugarer/interpreter already handles sequences of statements, so minimal changes:

**Old code:**
```rust
Statement::Guarded { condition, stmt } => {
    // Handle single guarded statement
}
```

**New code:**
```rust
Statement::Guarded { condition, stmts } => {
    // Handle multiple guarded statements
    // Desugarer can expand to sequence:
    // for each stmt in stmts:
    //   execute as [condition] stmt;
}
```

### 3.3 Code Changes Summary

| File | Lines | Change |
|------|-------|--------|
| `src/ast.rs` | ~5 | Update `Statement::Guarded` to use `Vec<Statement>` |
| `src/parser.rs` | ~30 | Add block parsing logic in `parse_statement()` |
| `src/desugarer.rs` | ~5 | Update to handle vec of statements |
| `src/interpreter.rs` | ~5 | Update to handle vec of statements |

**Total:** ~45 lines (within estimate)

---

## 4. Desugaring Strategy

The compiler can desugar block guards to flat guards:

**Input:**
```brief
[condition] {
  stmt1;
  stmt2;
  stmt3;
};
```

**Desugared:**
```brief
[condition] stmt1;
[condition] stmt2;
[condition] stmt3;
```

This ensures the existing interpreter logic works without changes.

---

## 5. Testing Strategy

### 5.1 Unit Tests

**File:** Add to `tests/parser_tests.rs`

```rust
#[test]
fn test_flat_guard() {
    let code = "[x > 0] &a = a + 1;";
    let ast = parse(code).unwrap();
    // Verify: Statement::Guarded with single stmt
}

#[test]
fn test_block_guard() {
    let code = "[x > 0] { &a = a + 1; &b = b + 2; }";
    let ast = parse(code).unwrap();
    // Verify: Statement::Guarded with 2 stmts
}

#[test]
fn test_nested_guards() {
    let code = "[x > 0] { [x < 10] &a = 1; };";
    let ast = parse(code).unwrap();
    // Verify: Nested guarded statements
}

#[test]
fn test_empty_block_error() {
    let code = "[x > 0] { }";
    let result = parse(code);
    assert!(result.is_err());
}

#[test]
fn test_block_with_term() {
    let code = "[success] { &done = true; term; }";
    let ast = parse(code).unwrap();
    // Verify: Both assignment and term in block
}
```

### 5.2 Integration Tests

Run existing stress tests to ensure backward compatibility:

```bash
cargo test --release
# Verify all 8 existing .bv files still compile
```

### 5.3 New Examples

Create `examples/guard_blocks.bv`:

```brief
let amount: Int = 0;
let large_count: Int = 0;
let large_total: Int = 0;

let small_count: Int = 0;
let small_total: Int = 0;

txn categorize_transfer [true][large_count + small_count > 0] {
  [amount > 100] {
    &large_count = large_count + 1;
    &large_total = large_total + amount;
  };
  
  [amount <= 100] {
    &small_count = small_count + 1;
    &small_total = small_total + amount;
  };
  
  term;
};
```

---

## 6. Edge Cases and Error Handling

### 6.1 Empty Blocks

```brief
[x > 0] { }  // ERROR: Expected at least one statement
```

**Implementation:** Check `stmts.is_empty()` after parsing block, return error.

### 6.2 Unclosed Blocks

```brief
[x > 0] {
  &a = 1;
  // Missing }
```

**Implementation:** Existing parser error handling (unclosed brace).

### 6.3 Guard at Block End

```brief
txn test [true][true] {
  &x = 1;
  [success] { &done = true; }
  // Missing semicolon after guard block
  term;
};
```

**Current behavior:** Semicolon is part of some statements. Guard blocks need semicolon:
```brief
[success] { &done = true; };
```

This is consistent with existing syntax.

### 6.4 Multiple Consecutive Guards

```brief
[x > 0] { &a = 1; };
[x < 0] { &b = 1; };
[x == 0] { &c = 1; };
```

This is allowed and expected. Each guard is independent.

---

## 7. Backward Compatibility

✅ **Fully backward compatible**

- Existing flat syntax `[c] stmt;` still works
- Parser treats it as block with single statement
- No breaking changes to AST structure (just Vec instead of Box)
- All existing .bv files continue to compile

---

## 8. Related Languages

### How Other Languages Handle This

**Rust:**
```rust
if condition {
    stmt1;
    stmt2;
}
```

**C/C++:**
```c
if (condition) {
    stmt1;
    stmt2;
}
```

**Brief:**
```brief
[condition] {
  stmt1;
  stmt2;
};
```

Brief follows the familiar pattern but with guard syntax instead of `if`.

---

## 9. Implementation Checklist

- [ ] Understand current AST `Statement::Guarded` representation
- [ ] Update `Statement` enum to use `Vec<Statement>`
- [ ] Implement block parsing in `parse_statement()`
- [ ] Update desugarer to handle multiple statements
- [ ] Update interpreter to handle multiple statements
- [ ] Add error handling for empty blocks
- [ ] Write unit tests for parser
- [ ] Write integration tests (existing .bv files)
- [ ] Create `examples/guard_blocks.bv`
- [ ] Verify all tests pass
- [ ] Update SPEC-v6.0.md (already done)
- [ ] Commit with message

---

## 10. Success Criteria

- ✅ Flat syntax `[c] stmt;` compiles (backward compat)
- ✅ Block syntax `[c] { stmts }` compiles
- ✅ Multiple statements in block work correctly
- ✅ Nested guards in blocks work
- ✅ Empty blocks produce error
- ✅ All 8 existing stress tests still pass
- ✅ New `examples/guard_blocks.bv` compiles
- ✅ Error messages clear for invalid syntax

---

*End of Design Document: Guard Block Syntax*
