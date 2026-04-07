# Design Document: Comment Handling with `//`
**Issue:** #2 - Lexer Enhancement  
**Date:** 2026-04-05  
**Status:** Ready for Implementation  
**Complexity:** Low (5-15 lines)  
**Time Estimate:** 15 minutes

---

## 1. Overview

### 1.1 Problem Statement

Comments currently break when used inside transaction and definition bodies:

```brief
txn transfer [pre][post] {
  &balance = balance - 10;  // Perform transfer
  term;
};
```

**Error:**
```
Parse error: Unexpected token in expression: Ok(Comment("// Perform transfer"))
```

### 1.2 Root Cause

The lexer returns `Token::Comment`, but the parser's expression parser doesn't handle comment tokens. It assumes all tokens are syntax.

### 1.3 Solution

Update the lexer to **skip comments entirely** instead of emitting `Token::Comment` tokens. Comments are never sent to the parser.

### 1.4 Benefits

1. **Correctness**: Comments work anywhere in the code
2. **UX**: Programmers expect comments to be transparent
3. **LLM-friendly**: AI-generated code can include explanatory comments
4. **Maintainability**: Code becomes self-documenting
5. **Consistency**: Follows standard language practice (C, Rust, Go, etc.)

---

## 2. Semantics

### 2.1 Comment Syntax

```bnf
comment ::= "//" [^\n]*

# A comment starts with // and extends to end of line
# The newline is NOT part of the comment
```

### 2.2 Where Comments Are Allowed

Comments can appear **anywhere** except inside string literals:

**Allowed:**
```brief
// Top-level comment
let x: Int = 5;  // Inline comment

txn test [pre][post] {  // Comment after guard
  &x = x + 1;           // Inline assignment comment
  term;                 // Comment before term
};

defn add(a: Int, b: Int) [true][true] -> Int {  // Function comment
  term a + b;  // Return sum
};
```

**Not allowed (inside strings):**
```brief
let msg: String = "This is not // a comment";  // But this is
```

### 2.3 Lexer Behavior

**Current:** Lexer returns `Token::Comment("...")` which parser tries to parse as expression

**New:** Lexer silently skips comments (never emits tokens)

Result: Parser never sees comments; they're transparent.

### 2.4 Newline Handling

Newlines are already ignored by the lexer (Brief doesn't use newlines for statement boundaries). Comments extend to the newline and are skipped with it:

```brief
let x = 5;        // Comment 1
let y = 10;       // Comment 2
// Standalone comment
let z = 15;
```

All comments are skipped; execution is unaffected.

---

## 3. Implementation

### 3.1 Current Lexer Structure

**File:** `src/lexer.rs`

The lexer uses the `logos` crate for tokenization:

```rust
use logos::Logos;

#[derive(Logos, Debug, Clone, PartialEq)]
pub enum Token {
    // ... other tokens ...
    
    #[regex(r"//.*$")]
    Comment(String),  // Currently emits comment tokens
    
    // ...
}
```

### 3.2 Solution Approach

**Option A: Use logos skip pattern (RECOMMENDED)**

```rust
#[derive(Logos, Debug, Clone, PartialEq)]
#[logos(skip r"//.*")]  // Skip comments entirely
pub enum Token {
    // ... tokens without Comment variant ...
    
    // Remove the #[regex(r"//.*$")] Comment(String) line
}
```

**Pros:**
- Simplest approach
- Logos handles it natively
- No Token::Comment variant needed
- Comments never reach parser

**Cons:**
- Need to verify logos version supports skip directive

**Option B: Use skip callback (fallback)**

If logos doesn't support skip patterns:

```rust
#[derive(Logos, Debug, Clone, PartialEq)]
pub enum Token {
    // ... other tokens ...
    
    #[regex(r"//[^\n]*", skip_comment)]
    Comment,  // Skipped; never emitted
    
    // ...
}

#[logos(ignore = r"[ \t\n]+")]
fn skip_comment(_: &mut Lexer<Token>) {
    // Callback: do nothing, effectively skipping the match
}
```

**Option C: Manual skip in parser (not recommended)**

Create helper in parser to skip comment tokens. More complex, error-prone.

### 3.3 Code Changes

**Choose Option A if possible:**

**File:** `src/lexer.rs`

**Change 1: Add skip directive**
```rust
#[derive(Logos, Debug, Clone, PartialEq)]
#[logos(skip r"//.*")]  // NEW: Skip comments
pub enum Token {
    // ... existing tokens ...
}
```

**Change 2: Remove Comment variant**
```rust
// REMOVE THIS:
// #[regex(r"//.*$")]
// Comment(String),
```

**Total changes:** ~3 lines

---

## 4. Testing Strategy

### 4.1 Unit Tests

**File:** Add to `tests/lexer_tests.rs`

```rust
#[test]
fn test_comment_is_skipped() {
    let mut lexer = Token::lexer("x = 5; // comment");
    let tokens: Vec<_> = lexer.collect();
    
    // Should NOT contain comment token
    for token in tokens {
        assert!(!matches!(token, Token::Comment(_)));
    }
}

#[test]
fn test_standalone_comment_line() {
    let mut lexer = Token::lexer("// This is a comment\nlet x = 5;");
    let tokens: Vec<_> = lexer.collect();
    
    // Should start with 'let' token, comment skipped
    assert!(matches!(tokens[0], Token::Let));
}

#[test]
fn test_multiple_comments() {
    let code = "
        // Comment 1
        let x = 5;  // Comment 2
        // Comment 3
        term;
    ";
    let mut lexer = Token::lexer(code);
    let tokens: Vec<_> = lexer.collect();
    
    // No comment tokens
    for token in tokens {
        assert!(!matches!(token, Token::Comment(_)));
    }
}

#[test]
fn test_comment_in_string_not_skipped() {
    // String literal containing // should not be treated as comment
    let code = r#"let msg: String = "This is // not a comment";"#;
    let mut lexer = Token::lexer(code);
    let tokens: Vec<_> = lexer.collect();
    
    // Should parse successfully; string contains //
    // This is already handled by string parsing logic
}

#[test]
fn test_comment_with_special_chars() {
    let code = "let x = 5; // !@#$%^&*() special chars";
    let mut lexer = Token::lexer(code);
    let tokens: Vec<_> = lexer.collect();
    
    // Should skip entire comment including special chars
    for token in tokens {
        assert!(!matches!(token, Token::Comment(_)));
    }
}
```

### 4.2 Integration Tests

**File:** `examples/comments.bv`

```brief
// Bank transfer system with comments
let alice_balance: Int = 1000;  // Alice's starting balance
let bob_balance: Int = 500;     // Bob's starting balance

txn transfer_funds [alice_balance >= 10]
  [alice_balance == @alice_balance - 10 && bob_balance == @bob_balance + 10]
{
  // Perform the transfer
  &alice_balance = alice_balance - 10;
  &bob_balance = bob_balance + 10;
  
  // Record transfer
  term;
};

// Verify balances are updated
defn check_transfer() [true][true] -> Bool {
  // If transfer succeeded, balances should change
  term true;
};
```

**Verify:** `cargo run -- examples/comments.bv` succeeds

### 4.3 Regression Tests

Run existing test suite:

```bash
cargo test --release
# Verify all 8 stress tests compile
```

All existing .bv files should continue to compile (most don't have comments, so no change).

---

## 5. Edge Cases

### 5.1 Comments Before Statement End

```brief
txn test [true][true] {
  &x = 1; // Comment
  term;   // Comment
};
```

✅ **Handled:** Comment is skipped before semicolon is seen.

### 5.2 Multiple Comments on Same Line

```brief
let x = 5; // Comment 1 // Comment 2
```

✅ **Handled:** First `//` starts comment to end of line (includes "Comment 2")

### 5.3 Comment After Guard

```brief
[x > 0] // Comment
{ &a = 1; };
```

✅ **Handled:** Comment skipped, then `{` parsed normally.

### 5.4 Comment in Expression (unlikely but possible)

```brief
let y = (
  x + 1  // Comment in expression
);
```

✅ **Handled:** Comment skipped, expression continues.

### 5.5 Comments in Contracts

```brief
txn test
  [x > 0]  // Pre-condition comment
  [x == @x + 1]  // Post-condition comment
{
  &x = x + 1;
  term;
};
```

✅ **Handled:** Comments skipped during expression parsing.

### 5.6 Comment with Escape Sequences

```brief
&msg = "test"; // Comment with \n \t \r
```

✅ **Handled:** Comments are raw strings; `\n` is just two characters, no special handling.

---

## 6. Interaction with Other Lexer Features

### 6.1 String Literals

Comments should NOT apply inside strings:

```brief
let msg: String = "This is // not a comment";
```

**Status:** String parsing happens before comment detection, so this is safe. The string lexer pattern will match and consume the entire string before comment pattern is checked.

### 6.2 Multi-line Strings (if supported)

If Brief ever supports multi-line strings, comments must not affect them.

**No change needed:** This design only affects single-line comments (`//`).

### 6.3 Other Whitespace

Comments should be treated like whitespace:

```brief
let x=5;/*no comment*/ let y=10;
```

The regex `r"//.*"` only matches `//` syntax, so `/* */` would not be skipped. This is fine; Brief uses `//` only.

---

## 7. Related Languages and Prior Art

### Rust

```rust
// Single-line comment
/* Multi-line comment */
```

Brief uses `//` only (no multi-line).

### Go

```go
// Comment
/* Multi-line */
```

Brief uses `//` only.

### Python

```python
# Comment
```

Brief uses `//` (more mainstream than `#`).

### Why `//` over `#`?

1. **Consistency:** C-family languages use `//`
2. **Clarity:** No conflict with Brief syntax (no `#var` would be confusing)
3. **LLM preference:** AI models trained on C/Rust/Go suggest `//`
4. **Familiarity:** Most programmers recognize `//`

---

## 8. Implementation Checklist

- [ ] Check logos version in `Cargo.toml`
- [ ] Verify logos supports skip directive
- [ ] Add skip directive to Token enum
- [ ] Remove Comment variant from Token enum
- [ ] Test with `cargo build`
- [ ] Write unit tests for comment skipping
- [ ] Create `examples/comments.bv`
- [ ] Run regression tests (existing 8 stress tests)
- [ ] Verify no compiler warnings
- [ ] Commit with message

---

## 9. Success Criteria

- ✅ Comments in transaction bodies compile
- ✅ Comments in defn bodies compile
- ✅ Comments in expressions are skipped
- ✅ Comments at end of statements work
- ✅ Multiple comments in code work
- ✅ No `Token::Comment` tokens reach parser
- ✅ Strings containing `//` are not treated as comments
- ✅ All 8 existing stress tests still pass
- ✅ New `examples/comments.bv` compiles
- ✅ Error messages unchanged (comments not visible in errors)

---

## 10. Potential Issues and Mitigations

### Issue: Logos doesn't support skip directive

**Mitigation:** Use Option B (skip callback). Simple fallback.

### Issue: Comments in error messages

**Mitigation:** Comments are removed before parsing, so error messages won't show them anyway.

### Issue: Performance impact

**Mitigation:** Negligible. Skipping is a lexer operation (same cost as ignoring whitespace).

---

*End of Design Document: Comment Handling*
