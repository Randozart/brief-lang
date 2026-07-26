# Derivation & Synthesis — Comprehensive Implementation Plan

**Date:** 2026-07-11
**Status:** Plan — pre-implementation
**Depends on:** Completion of Extensible Types (Phases 0–7); Pure Bits
Interpreter Refactor (Phases 7.5, 8A–8G, `docs/plans/2026-07-11-pure-bits-refactor.md`); GLUE v2
**See also:** `docs/plans/2026-07-11-extensible-types-comprehensive.md` for
the prerequisite phases. This plan builds directly on the property system
(Phase 1B), plugin system (Phase 7), Pure Bits interpreter dispatch
(Phases 8A–8G), and the `.dbvl` archive format. Backend metadata dispatch
is documented in `docs/architecture/features/metadata-dispatch.md`.

---

## Overview

This plan adds **Derivation-as-a-First-Class-Feature** to Brief. The `:=`
operator introduces three capabilities from a single syntactic primitive:

| Capability | What | Phase |
|------------|------|-------|
| **Compile-Time Assertions** | `:= { 2, 2 -> 4 }` verifies function body against examples | Phase 8 |
| **Program Synthesis** | `:= { 2, 2 -> 4 }` with empty body infers the minimal formula | Phase 9 |
| **Sad-Path Derivation** | `:= { Err(e) -> fallback }` resolves FFI error types | Phase 11 |
| **Deductive Synthesis** | `[pre][post]` contracts synthesize provably correct bodies | Phase 10 |

### Core Design Principles

1. **The `:=` block is immortal** — never consumed or removed from source.
   It remains the permanent specification / single source of truth.

2. **No body overwriting** — `brief derive` only fills holes (`body: None`).
   An existing body is never modified by the tool.

3. **Additive-only optimization** — Existing optimization paths are never
   modified. New match arms only. The `_ => return None;` fallthrough must
   remain unchanged.

4. **Byte-offset surgical insertion** — Source write-back uses byte offsets,
   not AST pretty-printing. 100% of formatting, comments, and spacing is
   preserved.

5. **SMT solver is optional** — A fallback enumerative search (depth-bounded)
   ensures `brief derive` works offline. The SMT solver is a performance
   accelerator, not a hard dependency.

---

## Golden Rules (All Phases)

1. **Flat control flow**: All new/modified code must nest at most 2 levels
   deep. Arrowhead code is forbidden. Use `?`, guard clauses, early returns,
   and extracted helper functions.

2. **Contract-first**: Never weaken contract guarantees. If a function had
   `[result > 0]` before the refactor, it must still have it after.

3. **Additive only**: Existing optimization paths must NOT be modified. New
   match arms only. The `_ => return None;` fallthrough must remain unchanged.

4. **Tests or it doesn't exist**: Every new code path, every match arm,
   every feature must have corresponding tests. `cargo test --lib` before
   every commit.

5. **Doc comments on every definition**: Every `fn`, `struct`, `enum`,
   `trait` added or modified must have a `///` doc comment.

6. **Rationale comments at every change site**: Format:
   `// YYYY-MM-DD: Phase N.M — <what and why>`

7. **HashMap iteration determinism**: All HashMap iterations that produce
   IR instructions must be sorted by key before the loop.

8. **Do not ask "shall I commit?" — just commit**: After every logical step
   where tests pass, commit. No amend, no squash. One commit per step.

---

## Phase 8 — Lexer + Parser + AST for Derivation Blocks

### Goal

Add `:=` (`ColonEq`) as a new token, parse derivation blocks after function
signatures or bodies, and store them in the AST as `DerivationBlock` objects
that coexist with function bodies.

### Step 8.0 — Add `ColonEq` (`:=`) to the lexer

**File**: `src/lexer.rs`

**What**: Add a new token variant for the `:=` operator. Brief currently has
`Colon` (`:`), `ColonColon` (`::`), `ColonGreaterThan` (`:>`), and
`LtColon` (`<:`). `ColonEq` slots naturally between `ColonGreaterThan` and
`ColonColon` in the enum ordering.

**Changes**:

1. Add to the `Token` enum (around line 280):
```rust
/// `:=` — derivation / compile-time assertion block
/// 2026-07-11: Phase 8.0
ColonEq,
```

2. Add `Display` impl (around line 590):
```rust
Token::ColonEq => write!(f, ":="),
```

3. Add lexer match in the multi-character token sequence (around the colon
   handling at line 380):
```rust
Token::Colon => {
    // Check for :=, ::, :>, :  (longest match first)
    if self.peek_char() == '=' {
        self.advance();
        Token::ColonEq
    } else if self.peek_char() == ':' {
        self.advance();
        Token::ColonColon
    } else if self.peek_char() == '>' {
        self.advance();
        Token::ColonGreaterThan
    } else {
        Token::Colon
    }
}
```

4. Update any match on `Token` that needs a wildcard to handle the new
   variant (add `Token::ColonEq => { }` to dead-code branches).

**Search before adding** — verify no existing code assumes `:` and `=` are
adjacent tokens (which would now be lexed as a single token):
```bash
grep -rn 'Token::Colon' src/ | grep -v 'ColonGreaterThan\|ColonColon\|Display\|//'
```

Expected: All `Token::Colon` usages are for single `:` tokens (type
annotations, contracts, ternary). No code constructs `:=` as two separate
tokens.

**Nesting check**: Token variant addition — no nesting concern. Lexer match
uses guard clauses (`if/else if` chain) at depth 1.

**Tests**:
- `test_lexer_colon_eq`: `":="` → `Token::ColonEq`
- `test_lexer_colon_eq_in_context`: `"x := { 1 -> 2 };"` → tokens: Ident,
  ColonEq, LBrace, Int, Arrow, Int, RBrace, Semicolon
- `test_lexer_colon_not_confused`: `"x: Int"` → tokens: Ident, Colon,
  Ident (ColonEq not produced)
- `test_lexer_colon_colon_not_confused`: `"::"` → ColonColon (not ColonEq)

### Step 8.1 — Add `DerivationBlock` struct to AST

**File**: `src/ast.rs`

**What**: Define the AST node for derivation blocks and add an optional
`DerivationBlock` field to `Definition` and `Transaction`.

**Changes**:

Add new structs (after `TypeDefBody`, around line 627):

```rust
/// A single input-output pair in a derivation block.
/// `2, 2 -> 4` becomes `inputs: [2, 2], output: 4`.
/// 2026-07-11: Phase 8.1.
#[derive(Debug, Clone, PartialEq)]
pub struct DerivationExample {
    /// Input expressions (one per function parameter).
    pub inputs: Vec<Expr>,
    /// Expected output expression.
    pub output: Expr,
    /// Source span for error messages.
    pub span: Span,
}

/// A derivation block attached to a definition or transaction.
/// Contains input-output examples (inductive synthesis) or sad-path mappings.
/// 2026-07-11: Phase 8.1.
#[derive(Debug, Clone, PartialEq)]
pub struct DerivationBlock {
    /// List of example pairs: (inputs, expected_output).
    pub examples: Vec<DerivationExample>,
    /// Source span for error messages.
    pub span: Span,
}
```

Add field to `Definition` (around line 2501):
```rust
/// Optional derivation block `:= { ... }` attached after the body.
/// 2026-07-11: Phase 8.1.
pub derivation: Option<DerivationBlock>,
```

Add field to `Transaction` (around line 2520):
```rust
/// Optional derivation block `:= { ... }` attached after the body.
/// 2026-07-11: Phase 8.1.
pub derivation: Option<DerivationBlock>,
```

Update existing constructors and test fixtures for `Definition` and
`Transaction` to set `derivation: None` by default.

**Nesting check**: Two new structs, two new optional fields on existing
structs — no nesting concern.

**Tests**:
- `test_derivation_example_construct`: Build a DerivationExample, verify
  fields
- `test_definition_with_derivation`: Build a Definition with
  `derivation: Some(...)`, verify serialization
- `test_definition_without_derivation`: Default `derivation: None`

### Step 8.2 — Parse derivation blocks in `parse_definition()`

**File**: `src/parser.rs`

**What**: Modify `parse_definition()` to accept an optional derivation
block after the function body, and to accept `:=` immediately after the
signature when the body is omitted (drafting state).

**Two syntactic states**:

**State A — Drafting (body omitted)**:
```brief
defn add(x: Int, y: Int) -> Int := { 2, 2 -> 4; 3, 5 -> 8; };
```

**State B — Resolved (body present)**:
```brief
defn add(x: Int, y: Int) -> Int { term x + y; } := { 2, 2 -> 4; 3, 5 -> 8; };
```

**Changes to `parse_definition()`** (line 4718):

After parsing the signature `-> ReturnType`, the parser looks at the next
token:

```rust
/// Phase 8.2: After signature, check for body or derivation.
let body = if self.current_token_is(Token::LBrace) {
    // State B: Body is present — parse it
    let body = self.parse_body()?;
    Some(body)
} else if self.current_token_is(Token::Semicolon) {
    // Lambda-style terminator: `defn f(x) -> Int;` (no body, no derivation)
    self.advance();
    None
} else {
    // Expect either body or derivation
    return self.spanned_err("expected '{' or ':=' after function signature");
};

// Check for trailing derivation block
let derivation = if self.current_token_is(Token::ColonEq) {
    self.advance(); // consume :=
    Some(self.parse_derivation_block()?)
} else {
    None
};
```

New parser method:

```rust
/// Parse a derivation block: `{ 2, 2 -> 4; 3, 5 -> 8; }`
/// 2026-07-11: Phase 8.2
fn parse_derivation_block(&mut self) -> Result<DerivationBlock, SyntaxError> {
    let start = self.current_span().start;
    self.expect(Token::LBrace)?;
    let mut examples = Vec::new();

    while !self.check(Token::RBrace) {
        let example = self.parse_derivation_example()?;
        examples.push(example);

        // Semicolon between examples, but not after the last one
        if self.current_token_is(Token::Semicolon) {
            self.advance();
        } else if !self.check(Token::RBrace) {
            return self.spanned_err("expected ';' between derivation examples");
        }
    }
    self.expect(Token::RBrace)?;

    let end = self.current_span().end;
    Ok(DerivationBlock {
        examples,
        span: Span::new(start, end),
    })
}

/// Parse a single example: `2, 2 -> 4` or `0x1234 -> 0x3412`
/// 2026-07-11: Phase 8.2
fn parse_derivation_example(&mut self) -> Result<DerivationExample, SyntaxError> {
    let start = self.current_span().start;

    // Parse inputs (comma-separated expressions until `->`)
    let mut inputs = Vec::new();
    inputs.push(self.parse_expression()?);
    while self.current_token_is(Token::Comma) {
        self.advance();
        inputs.push(self.parse_expression()?);
    }

    // Expect `->` separator
    self.expect(Token::Arrow)?;

    // Parse expected output
    let output = self.parse_expression()?;

    let end = self.current_span().end;
    Ok(DerivationExample {
        inputs,
        output,
        span: Span::new(start, end),
    })
}
```

**Note on `Token::Semicolon` after derivation block**: When the function has
a body and a trailing derivation, the final `;` after `}` is the definition
terminator. When there is no body (State A), the `;` after `}` is also the
terminator. The parse flow:
- State A: `defn f() -> Ret := { ... };` → signature → no body → derivation
  block → semicolon (consumed by `parse_definition` after)
- State B: `defn f() -> Ret { ... } := { ... };` → signature → body →
  derivation block → semicolon

The existing terminator handling in `parse_definition()` already consumes
the final `;` — no change needed there.

**Nesting check**: The `parse_derivation_block` method has a single loop
with guard clauses — depth 2. `parse_derivation_example` is sequential
(push, push, expect, parse) — depth 1.

**Tests**:
- `test_parse_defn_with_body_and_derivation`: Body + derivation → both present
- `test_parse_defn_draft_with_derivation`: No body, derivation → body is None,
  derivation is Some
- `test_parse_defn_no_derivation`: Body, no derivation → existing behavior
- `test_parse_txn_with_derivation`: Transaction with derivation block
- `test_parse_derivation_multiple_examples`: Two examples with semicolons
- `test_parse_derivation_no_semicolon_after_last`: `{ a -> b; c -> d }` valid
- `test_parse_derivation_rejects_empty`: `:= {}` → parse error (no examples)
- `test_parse_derivation_example_syntax_error`: `2 -> -> 4` → parse error

### Step 8.3 — Parse `:=` in lambda-style definitions

**What**: Lambda-style definitions (`defn f(x) -> Int;`) can also have
derivation blocks. Extend the lambda parsing to check for `:=` before `;`:

```brief
defn f(x: Int) -> Int := { 0 -> 0; };
```

**File**: `src/parser.rs` (the lambda branch in `parse_definition`)

When the current token after the signature is `;`:
```rust
if self.current_token_is(Token::Semicolon) {
    // Lambda-style: check if there's a derivation before the semicolon
    // Actually, for lambda-style, derivation comes before ; too:
    // defn f(x) -> Int := { 0 -> 0; };
    // But here ; is the terminator. So we must check for := BEFORE the ;
    // This case is already handled by the general flow — the parser
    // checks for := before the body, which handles the no-body case.
    self.advance();
    return Ok(Definition { body: None, derivation: None, ... });
}
```

Actually, re-read: `defn f(x) -> Int := { 0 -> 0; };` — the `;` after `}`
terminates the definition. The `=` in `:=` means `=` is the second character
which is NOT a standalone `;`. So the `:=` case triggers before we'd see `;`.
The flow is already correct from Step 8.2.

**Tests**:
- `test_parse_lambda_with_derivation`: `defn f(x) -> Int := { 0 -> 0; };` →
  body: None, derivation: Some

### Step 8.4 — Validate derivation examples at type-check time

**File**: `src/typechecker.rs`

**What**: After type-checking the function signature and body (if present),
validate each derivation example:
1. Number of input expressions matches the function's parameter count
2. Each input expression type-checks against the corresponding parameter type
3. The output expression type-checks against the function's return type

**Changes**:

Add a new function `check_derivation()` called from the type-check pass:

```rust
/// Validate that all derivation examples are well-typed for this function.
/// 2026-07-11: Phase 8.4
fn check_derivation(
    derivation: &DerivationBlock,
    params: &[Parameter],
    ret_type: &Type,
    ctx: &TypeCheckContext,
) -> Result<(), Vec<TypeError>> {
    let mut errors = Vec::new();

    for (i, example) in derivation.examples.iter().enumerate() {
        // Check parameter count
        if example.inputs.len() != params.len() {
            errors.push(TypeError::new(
                &example.span,
                format!(
                    "derivation example {}: expected {} input(s), got {}",
                    i + 1,
                    params.len(),
                    example.inputs.len()
                ),
            ));
            continue;
        }

        // Check each input expression type
        for (j, (input_expr, param)) in example.inputs.iter().zip(params.iter()).enumerate() {
            let input_ty = infer_expr_type(input_expr, ctx)?;
            if !types_compatible(&input_ty, &param.ty) {
                errors.push(TypeError::new(
                    &input_expr.span(),
                    format!(
                        "derivation example {} input {}: expected type {:?}, got {:?}",
                        i + 1, j + 1, param.ty, input_ty
                    ),
                ));
            }
        }

        // Check output expression type
        let output_ty = infer_expr_type(&example.output, ctx)?;
        if !types_compatible(&output_ty, ret_type) {
            errors.push(TypeError::new(
                &example.output.span(),
                format!(
                    "derivation example {} output: expected type {:?}, got {:?}",
                    i + 1, ret_type, output_ty
                ),
            ));
        }
    }

    if errors.is_empty() { Ok(()) } else { Err(errors) }
}
```

**Nesting check**: The function has a loop (level 1) with two inner loops
(level 2) — the `continue` is a guard clause pattern. The inner loops use
`zip`, which is flat. Acceptable.

**Tests**:
- `test_derivation_example_count_mismatch`: 3 inputs for 2-param function →
  error
- `test_derivation_example_type_mismatch`: `"hello"` for Int param → error
- `test_derivation_example_output_type_mismatch`: Output is String but return
  is Int → error
- `test_derivation_examples_all_valid`: All examples well-typed → passes

### Step 8.5 — Execute derivation examples as compile-time tests

**File**: New function in `src/typechecker.rs` or `src/derive.rs`

**What**: When a definition has BOTH a body and a derivation block, execute
each example through the compile-time interpreter. This provides
**Compile-Time Assertions** — zero-runtime-cost verification that the
function body produces the expected outputs for the provided inputs.

**Approach**: Rather than inventing an external environment API, reuse the
interpreter's existing function call mechanism. For each example, evaluate
the input expressions to produce `Value` arguments, then call the function
with those arguments via `Interpreter::call_function`. The interpreter
already handles parameter binding internally via `InterpreterFrame`.

**Changes**:

```rust
use crate::derive::DeriveError;

/// Execute all derivation examples through the compile-time interpreter.
/// Each example's inputs are evaluated as expressions, then the function
/// is called with those values. The result is compared to the expected
/// output expression (also evaluated at compile time).
/// 2026-07-11: Phase 8.5
fn execute_derivation_tests(
    defn: &Definition,
    interpreter: &mut Interpreter,
) -> Result<(), Vec<DeriveError>> {
    let Some(derivation) = &defn.derivation else {
        return Ok(());
    };
    let Some(_body) = &defn.body else {
        return Ok(()); // No body yet — synthesis phase handles this
    };

    let mut errors = Vec::new();

    for (i, example) in derivation.examples.iter().enumerate() {
        // Step 1: Evaluate input expressions to produce argument values
        let args: Result<Vec<Value>, _> = example.inputs.iter()
            .map(|input| interpreter.eval_expr(input, &None))
            .collect();
        let args = match args {
            Ok(a) => a,
            Err(e) => {
                errors.push(DeriveError::EvalFailed {
                    example_index: i,
                    message: e.to_string(),
                });
                continue;
            }
        };

        // Step 2: Call the function with these arguments.
        // The interpreter bounds params via InterpreterFrame internally.
        let result = match interpreter.call_function(&defn.name, &args) {
            Ok(r) => r,
            Err(e) => {
                errors.push(DeriveError::EvalFailed {
                    example_index: i,
                    message: e.to_string(),
                });
                continue;
            }
        };

        // Step 3: Evaluate the expected output expression
        let expected = match interpreter.eval_expr(&example.output, &None) {
            Ok(v) => v,
            Err(e) => {
                errors.push(DeriveError::EvalFailed {
                    example_index: i,
                    message: format!("expected output: {}", e),
                });
                continue;
            }
        };

        // Step 4: Compare
        if result != expected {
            errors.push(DeriveError::AssertionFailed {
                example_index: i,
                expected,
                got: result,
                span: example.span,
            });
        }
    }

    if errors.is_empty() { Ok(()) } else { Err(errors) }
}
```

**Prerequisites:** The interpreter's `call_function` method must accept
a function name and a slice of argument values, push a frame with
parameter bindings, execute the body, and return the `Term` value's
result. If `call_function` does not yet exist (because it currently
requires an `Expr::Call` node rather than raw `Value` arguments), a
thin wrapper must be added to the interpreter:

```rust
// Thin addition to the interpreter (if not already present)
impl Interpreter {
    /// Call a function by name with pre-evaluated argument values.
    /// 2026-07-11: Phase 8.5
    pub fn call_function(&mut self, name: &str, args: &[Value]) -> Result<Value, RuntimeError> {
        // Bind parameter names → values via InterpreterFrame
        // Execute body statements sequentially
        // Return the term value
    }
}
```

This reuse of the existing interpreter's call mechanism means no new
environment API (`InterpreterEnv`) is needed — the interpreter already
manages parameter binding internally through its frame stack.

**Tests**:
- `test_compile_time_assertion_passes`: `add { 2, 2 -> 4; }` → compiles
- `test_compile_time_assertion_fails`: `add { 2, 2 -> 5; }` → assertion error
- `test_compile_time_assertion_multi_example`: Both pass → compiles
- `test_compile_time_assertion_one_fails`: Second example fails → error
  with correct example number
- `test_compile_time_no_body_skips`: No body → tests skipped (handled in
  synthesis phase)

### Step 8.6 — Add `when` keyword and guard statement syntax

**Goal:** Introduce `when` as a body-only keyword for compile-time verified
guards, alongside the existing `[condition]` bracket form. Both map to the
same `Statement::Guarded` AST node. New same-line enforcement: if a guard
and its effect are on the same line, braces are optional. If they span
multiple lines, braces are required.

#### Syntax rules

| Form | Arrow? | Same-line? | Scope |
|------|--------|-----------|-------|
| `when x > 0 -> term 0;` | ✅ Required | ✅ Required | Bodies only |
| `when x > 0 { term 0; };` | ❌ Not used | ❌ Not required | Bodies only |
| `[x > 0] term 0;` | ❌ Not used | ✅ Required (NEW) | Universal |
| `[x > 0] { term 0; };` | ❌ Not used | ❌ Not required | Universal |
| `when x > 0` on signature | — | — | ❌ Rejected (bodies only) |
| `when x > 0 -> { ... };` | ❌ Rejected | — | `->` never pairs with `{}` |

**Key principle:** `when` is syntactically forbidden on signatures and type
definitions — it only appears inside function and transaction bodies.
Contract brackets `[ ]` remain universal (signatures, type defs, and bodies).

**Breaking change:** `[x > 0]` across lines without braces (`[x > 0]\nstmt;`)
is now rejected. The effect must be on the same line when braces are omitted.

#### Step 8.6.0 — Add `when` keyword token

**File:** `src/lexer.rs`

Add a new keyword token:

```rust
/// `when` — compile-time verified guard (body-only).
/// 2026-07-12: Phase 8.6
#[token("when")]
When,
```

Also mark `when` as a reserved keyword in the identifier parser so it
cannot be used as a variable or function name.

**Tests:**
- `test_lexer_when_keyword`: `when` → `Token::When`
- `test_lexer_when_not_identifier`: `let when = 5;` → parse error (reserved)

#### Step 8.6.1 — Implement `parse_guard_statement()`

**File:** `src/parser.rs`

Add a unified parser method that handles both `[condition]` and
`when condition` guard forms, with `->` for the compact single-line
`when` form and same-line enforcement:

```rust
/// Parse a compile-time verified guard statement.
/// Handles both `when condition` and `[condition]` forms.
/// 2026-07-12: Phase 8.6
fn parse_guard_statement(&mut self) -> Result<Statement, SyntaxError> {
    // `when` is only valid inside bodies — enforced by parse_statement dispatch
    // Capture start line for same-line enforcement
    let start_line = self.current_span().unwrap_or_else(Span::dummy).line;
    let condition = self.parse_expression()?;

    // Form A: Multi-line block — braces required
    if matches!(self.current_token(), Some(Ok(Token::LBrace))) {
        let statements = self.parse_block()?;
        return Ok(Statement::Guarded {
            condition,
            statements,
            metadata: HashMap::new(),
        });
    }

    // Form B: Compact single-line — no braces
    // For `when` only: optional `->` separator before the effect statement
    if matches!(self.current_token(), Some(Ok(Token::Arrow))) {
        // `when x > 0 -> { ... };` is rejected — `->` never pairs with `{}`
        if self.peek_token_is(Token::LBrace) {
            return self.spanned_err(
                "Do not use '->' with braces. Write 'when x > 0 { ... };' directly."
            );
        }
        self.advance(); // consume ->
    } else if !matches!(self.current_token(), Some(Ok(Token::LBrace))) {
        // Bracket form without `->` — check it's actually a bracket guard,
        // not a bare expression misparsed. If the token after the expression
        // is a statement start, proceed.
    }

    let effect = self.parse_statement()?;
    let end_line = effect.span().unwrap_or_else(Span::dummy).line;

    // Enforce same-line rule: guard and effect must be on the same line
    if start_line != end_line {
        return self.spanned_err(
            "Braces '{}' are required for guards that spill to a new line."
        );
    }

    Ok(Statement::Guarded {
        condition,
        statements: vec![effect],
        metadata: HashMap::new(),
    })
}
```

**Dispatch integration:** In `parse_statement()`, add a check for
`Token::When` that routes to `parse_guard_statement()`. The existing
`[condition]` handling already routes through the same method (or is
replaced by it).

```rust
fn parse_statement(&mut self) -> Result<Statement, SyntaxError> {
    match self.current_token() {
        Some(Ok(Token::When)) => {
            self.advance();
            self.parse_guard_statement()
        }
        Some(Ok(Token::LBracket)) => {
            // Existing bracket guard parsing — replace with
            // parse_guard_statement() or keep inline
            self.parse_guard_statement()
        }
        // ... other statement types ...
    }
}
```

**Rejection on signatures:** In `parse_definition()` and
`parse_transaction()`, add a check in the signature section that
rejects `when`:

```rust
// After parsing parameter list, before contract brackets:
if matches!(self.current_token(), Some(Ok(Token::When))) {
    return self.spanned_err(
        "'when' is not valid on signatures. Use '[condition]' for contracts."
    );
}
```

**Nesting check:** `parse_guard_statement()` uses guard clauses (early
return for `{}` block form, then `if` for arrow, then same-line check)
— max depth 2.

**Tests:**
- `test_parse_when_compact`: `when x > 0 -> term 0;` → Guarded with
  condition `x > 0`, body `[term 0]`
- `test_parse_when_block`: `when x > 0 { term 0; };` → same AST node
- `test_parse_bracket_compact`: `[x > 0] term 0;` → same AST node
- `test_parse_bracket_block`: `[x > 0] { term 0; };` → same AST node
- `test_parse_when_rejected_on_signature`: `defn f() when x > 0;` → error
- `test_parse_when_arrow_with_braces`: `when x > 0 -> { term 0; };` → error
- `test_parse_guard_same_line_violation`: `when x > 0 ->\nterm 0;` → error
- `test_parse_bracket_same_line_violation`: `[x > 0]\nterm 0;` → error
- `test_parse_when_not_reserved_as_keyword`: `when` used as identifier → error

#### Step 8.6.2 — Verify `Statement::Guarded` handles both forms

**File:** `src/ast.rs`

**What:** Verify that the existing `Statement::Guarded { condition, statements,
metadata }` node (Phase 1A) already serves both `when` and `[condition]`
forms. No changes to the AST struct itself — the parser simply populates it
from either syntactic form.

If `Statement::Guarded` does not yet exist (Phase 1A may not have been
committed when this is implemented), add it:

```rust
/// A compile-time verified guard condition.
/// Parsed from either `[condition] stmt;` or `when condition -> stmt;`.
/// The SMT verifier proves the condition before the body executes.
/// 2026-07-12: Phase 8.6
Guarded {
    condition: Expr,
    statements: Vec<Statement>,
    metadata: HashMap<String, PropertyValue>,
},
```

**Check:** Ensure the interpreter's eval loop, DCE pass, and SMT verifier
all handle `Statement::Guarded` identically regardless of whether it was
parsed from brackets or `when`. No changes needed — they operate on the
AST node, not the source syntax.

**Tests:**
- Existing `test_interpreter_guard` passes with both `when` and `[ ]` forms
- No interpreter/backend changes needed — the AST node is consumed identically

#### Step 8.6.3 — Add `when` to the syntax documentation

**File:** `docs/architecture/` (update existing syntax doc)

**What:** Document the complete guard statement model:

```brief
// Guards are compile-time verified — the SMT solver proves the
// condition before the body executes.

// Two syntactic forms, identical semantics:

// Form 1: Bracket (universal — signatures, type defs, bodies)
[x > 0] term 0;                  // compact, same-line
[x > 0] {                        // multi-line, braces required
    let adjusted = x + 1;
    term adjusted;
};

// Form 2: when (bodies only)
when x > 0 -> term 0;            // compact, same-line, arrow required
when x > 0 {                     // multi-line, braces required, no arrow
    let adjusted = x + 1;
    term adjusted;
};

// Error: spilling to a new line without braces
// [x > 0]          ← error: braces required for multi-line
//     term 0;

// Error: when on a signature
// defn f() when x > 0 -> Int { ... };  ← error: use [x > 0] instead
```

**Tests integration:**

```rust
#[test]
fn test_when_and_bracket_produce_same_ast() {
    let source_when = "defn f(x: Int) -> Int { when x > 0 -> term x; term 0; };";
    let source_bracket = "defn f(x: Int) -> Int { [x > 0] term x; term 0; };";
    let ast_when = parse(source_when).unwrap();
    let ast_bracket = parse(source_bracket).unwrap();
    // Both produce identical ASTs
    assert_eq!(ast_when, ast_bracket);
}
```

---

## Phase 8G — Remove Intrinsic/Inop, Modularize Frontend & Backend

**Proposal by:** [@revred](https://github.com/revred) — review identified
that the `Intrinsic` enum, `InopDeclaration`, and `Expr::IntrinsicCall` are
the last remaining compiler-level hardcoded special cases. Removing them
completes the frontend/backend split: the frontend emits only standard
`defn` entries with metadata, and backends dispatch on metadata strings.
See `docs/architecture/features/metadata-dispatch.md` for the full
architecture of metadata lifecycle and distributed backend verification.

**Depends on:** Pure Bits Refactor Phases 8A–8F (provides
`execute_intrinsic()`, `get_operator_intrinsic()`, property-based operator
dispatch). Phase 8G deletes the legacy intrinsic paths that 8A–8F built
alongside.

**Strategy:** Delete-and-compile. Remove the old enum/variant/AST node, let
the compiler report every match arm, replace each with
metadata-plus-`execute_intrinsic` dispatch. Zero new features — pure
deletion and rewriting.

### Goal

Remove four legacy components that block the frontend/backend modularization:

| Component | Replaced by |
|-----------|-------------|
| `Intrinsic` enum (~50 variants) | String-based `execute_intrinsic()` dispatch (Phase 8A) |
| `InopDeclaration` AST node | Standard `defn` with `llvm_instr`/`interpreter_impl` metadata |
| `Expr::IntrinsicCall` (~356 match arms) | Standard `Expr::Call` to metadata-decorated `defn` |
| `#` suffix syntax on identifiers | Standard function call `f(x)` not `f#(x)` |

### Step 8G.0 — Remove `Intrinsic` enum

**File:** `src/ast.rs` — `pub enum Intrinsic { ... }` line ~897

Delete the entire enum (~50 variants). All match arms on `Intrinsic` become
unreachable. The `Intrinsic::name()` method added in Phase 8A is also
deleted — its mapping is now only in `execute_intrinsic()`.

**Replacement:** Every site that previously matched on `Intrinsic::AddI64`
now calls `execute_intrinsic("__add_i64", args)`. The function already
exists (Phase 8A.2). The `execute_intrinsic` function is promoted from
interpreter-internal to a public API in `src/interpreter.rs`.

**Files affected:**
- `src/ast.rs` — delete `Intrinsic` enum, delete `intrinsic_name` list
- `src/interpreter.rs` — replace `Intrinsic::AddI64 =>` with
  `execute_intrinsic(name, args)` in all eval sites; promote
  `execute_intrinsic` to `pub`
- `src/features/binary_op.rs` — remove `Intrinsic` match, already
  replaced by property dispatch in Phase 8B
- `src/features/unary_op.rs` — same as binary_op
- `src/parser.rs` — remove any `Intrinsic`-related parsing
- `src/desugarer.rs` — replace intrinsic name resolution with
  string-based resolution

When `cargo build` succeeds, the `Intrinsic` enum is gone for good.

**Nesting check:** Each replacement is a one-line substitution at the call
site. No new nesting introduced.

**Tests:**
- `test_no_intrinsic_enum`: The type `Intrinsic` does not exist
- `test_execute_intrinsic_public`: `execute_intrinsic("__add_i64", &[a, b])`
  returns correct result

### Step 8G.1 — Remove `InopDeclaration` AST node + `inop` keyword

**File:** `src/ast.rs:1227`, `src/lexer.rs`, `src/parser.rs`

Delete `InopDeclaration` struct. Remove `TopLevel::Inop(InopDeclaration)`
variant from the `TopLevel` enum. Remove `inop` / `inop!` from the lexer
token set. Remove the `parse_inop_declaration()` parser path.

**Replacement:** All existing `inop` declarations in `lib/std/os/*.bv` are
rewritten as standard `defn` with metadata:

```brief
// Before (inop):
// inop! getpid() -> Int;

// After (defn with metadata):
defn getpid() -> Int {
    llvm_instr <~ "call i64 @getpid()";
    interpreter_impl <~ "posix_getpid";
}
```

**Migration:** The stdlib files in `lib/std/os/` are rewritten in the same
commit. The `inop` keyword is removed from the lexer — any old source files
using it will get a clear parse error.

**Files affected:**
- `src/ast.rs` — delete `InopDeclaration`, delete `TopLevel::Inop`
- `src/lexer.rs` — remove `inop`/`inop!` tokens
- `src/parser.rs` — remove `parse_inop_declaration()`, remove dispatch in
  `parse_top_level()`
- `lib/std/os/*.bv` — rewrite all `inop` declarations as `defn`s with
  metadata
- `lib/std/types/bootstrap.bv` — update any references

**Tests:**
- `test_inop_keyword_rejected`: `inop foo() -> Int;` → parse error
- `test_os_defn_has_llvm_instr`: `getpid` defn has `llvm_instr` metadata

### Step 8G.2 — Remove `Expr::IntrinsicCall` variant

**File:** `src/ast.rs` (Expr enum), all ~356 match sites

`Expr::IntrinsicCall { intrinsic, args }` is replaced by
`Expr::Call { name, args }`. The `#` suffix in the parser
(`sqrt#(x)` → `Expr::IntrinsicCall`) is removed — the parser produces
`Expr::Call` for all function calls.

**Replacement pattern** (in the interpreter and all eval sites):

```rust
// Before:
Expr::IntrinsicCall { intrinsic, args } => {
    let intrinsic_name = intrinsic.name();
    let evaluated_args: Vec<Value> = args.iter()
        .map(|a| ctx.eval_expr(a, &None))
        .collect::<Result<_, _>>()?;
    execute_intrinsic(intrinsic_name, &evaluated_args)
}

// After:
Expr::Call { name, args } => {
    let evaluated_args: Vec<Value> = args.iter()
        .map(|a| ctx.eval_expr(a, &None))
        .collect::<Result<_, _>>()?;
    // Check if the called defn has interpreter_impl metadata
    if let Some(impl_name) = ctx.lookup_interpreter_impl(&name) {
        execute_intrinsic(impl_name, &evaluated_args)
    } else {
        // Standard function call path (already exists)
        ctx.call_function(&name, evaluated_args)
    }
}
```

**Key change:** The `interpreter_impl` metadata lookup replaces the
`Intrinsic` enum dispatch. If a function has `interpreter_impl <~ "..."`,
the interpreter calls `execute_intrinsic` directly — no enum needed.
Otherwise, it falls through to the standard function call path.

**Files affected:**
- `src/ast.rs` — remove `IntrinsicCall` from `Expr` enum
- `src/interpreter.rs` — replace all `Expr::IntrinsicCall` match arms
  (~300 sites) with `Expr::Call` + metadata lookup
- `src/parser.rs` — remove `#` suffix in identifier parsing
- `src/desugarer.rs` — remove `intrinsic_name_from_expr`, replace with
  `operator_to_defn_name` (maps `Add` → `"add_i64"` etc.)
- `src/backend/llvm/*.rs` — replace `Expr::IntrinsicCall` with
  `Expr::Call` in codegen
- `src/backend/webstack.rs`, `src/backend/circt.rs` — same replacement

**Nesting check:** The replacement pattern has a single `if let` (metadata
lookup) with a guard clause — max 2 levels.

**Tests:**
- `test_intrinsic_call_becomes_call`: Parse `sqrt(x)` and verify AST has
  `Expr::Call` not `Expr::IntrinsicCall`
- `test_interpreter_dispatch_via_metadata`: Function with
  `interpreter_impl <~ "rust_add_i64"` dispatches through
  `execute_intrinsic` at runtime

### Step 8G.3 — Remove `#` suffix from lexer

**File:** `src/lexer.rs`, `src/parser.rs`

Remove the `Hash` token recognition at the end of identifiers (the `sqrt#`
lexer path). The `#` character remains for:
- `#[` (HashBracket) — attributes
- `#![` (HashBangBracket) — inner attributes
- `#` (Hash) — pragma prefix (`#no_derive`, `#export`)
- `#!` (HashBang) — shebang
- `#?` (HashQuestion) — diagnostic qualifier
- `#pragma` (Pragma) — pragma keyword

The `#` is no longer recognized as an intrinsic suffix on identifiers.

**Files affected:**
- `src/lexer.rs` — remove the identifier-ends-with-`#` check
- `src/parser.rs` — remove `IntrinsicCall` construction in call
  expression parsing
- `src/ast.rs` — verify `Expr::IntrinsicCall` is gone (caught by
  compile if missed)

**Tests:**
- `test_hash_no_longer_intrinsic_suffix`: `sqrt#(x)` is parsed as
  `sqrt # ( x )` (syntax error) or `#(x)` is a pragma-then-parens error,
  not an intrinsic call

### Step 8G.4 — Verify `.dbvl` archive carries metadata

**File:** (test addition — Phase 12 archive test suite)

Add a round-trip test that a `defn` with `llvm_instr`, `interpreter_impl`,
and `circt_op` metadata survives the archive serialization and
deserialization. A backend reading the archive can dispatch on these
strings without any shared enum with the frontend. The metadata dispatch
architecture is documented in `docs/architecture/features/metadata-dispatch.md`.

```rust
#[test]
fn test_archive_carries_metadata() {
    let source = r#"
        defn add_i64(a: Int, b: Int) -> Int {
            llvm_instr <~ "add nsw i64";
            interpreter_impl <~ "rust_add_i64";
            circt_op <~ "comb.add";
        };
    "#;
    // 1. Parse → write archive → read archive
    // 2. Verify defn entry has metadata strings intact
    // 3. Verify no Intrinsic enum or InopDeclaration in output
}
```

**This is the modularization proof:** the frontend emits only `defn`
entries with metadata strings; the backend consumes the archive and
dispatches on those strings. No shared `Intrinsic` enum between them.

**Tests:**
- `test_archive_carries_metadata`: Metadata round-trips correctly
- `test_archive_no_intrinsic_entries`: No `IntrinsicCall` or `Inop`
  references in archive output

### Step 8G.5 — Add `observable` metadata for liveness tracking

**What:** The `Intrinsic` enum currently carries implicit knowledge about
which operations have observable side effects (I/O, hardware access, etc.).
This knowledge is embedded in match arms across the dead-code elimination
pass and the LLVM backend. Removing the enum means this knowledge must be
explicit — declared as metadata on the function itself.

**The `observable` property:**

A boolean metadata key that marks a function as having side effects visible
outside the Brief program. The compiler's DCE pass must preserve calls to
`observable` functions even when the result is unused.

```brief
defn print_int(n: Int) -> Bool {
    observable <~ true;
    llvm_asm <~ "call @printf";
    interpreter_impl <~ "rust_print_int";
}

defn read_cycle_counter() -> UInt64 {
    observable <~ true;
    llvm_asm <~ "rdtsc";
}

defn add_i64(a: Int, b: Int) -> Int {
    llvm_instr <~ "add nsw i64";
    // No observable — this is a pure computation.
    // DCE may eliminate this call if result is unused.
}
```

**Impact by layer:**

| Layer | Without `observable` | With `observable` |
|-------|---------------------|-------------------|
| **DCE pass** (frontend) | Eliminates call if result is unused | Preserves call unconditionally |
| **LLVM backend** | Emits `readnone` — LLVM may reorder/eliminate | Emits `sideeffect` on asm, omits `readnone` on calls |
| **Interpreter** | May skip evaluation during compile-time folding | Executes call; side effects are observable |
| **SMT verifier** | Assumes deterministic (same inputs → same output) | Treats result as nondeterministic |

**Default:** `observable <~ false;` — functions are assumed pure unless
explicitly declared otherwise. This is the safe default for the synthesis
engine (Phase 9), which reasons about mathematical purity.

**Files affected:**
- `src/lifetime.rs` or DCE pass — check `observable` metadata before
  eliminating calls
- `src/backend/llvm/emit_stmt.rs` — check `observable` to decide
  `readnone`/`sideeffect` attributes
- `src/interpreter.rs` — check `observable` to decide fold eligibility
- `src/proof_engine.rs` — check `observable` to decide determinism
- `lib/std/os/*.bv`, `lib/std/types/bootstrap.bv` — mark I/O functions
  with `observable <~ true;`

**Tests:**
- `test_observable_dce_preserves`: Call to `observable` function with
  unused result → call preserved in IR
- `test_observable_dce_eliminates`: Call to pure function with unused
  result → call eliminated
- `test_observable_llvm_readnone`: Pure function gets `readnone`
  attribute in LLVM IR
- `test_observable_llvm_sideeffect`: Observable function gets
  `sideeffect` on asm call

| File | Before 8G | After 8G | Arms removed |
|------|-----------|----------|--------------|
| `src/ast.rs` | `Intrinsic` enum (50 vars) + `InopDeclaration` + `IntrinsicCall` | **Deleted** | ~60 enum variants/nodes |
| `src/interpreter.rs` | ~300 `Expr::IntrinsicCall` match arms | `Expr::Call` + metadata check | ~300 |
| `src/lexer.rs` | `#` suffix in identifier lexing | Standard lexer | ~5 lines |
| `src/parser.rs` | `inop` parse path + `#` call syntax | Removed | ~30 lines |
| `lib/std/os/*.bv` | `inop` declarations | Standard `defn` + metadata | All `inop` keywords |

### Gate: Phase 8G

```
cargo build            # clean (no Intrinsic, InopDeclaration, or
                       #        Expr::IntrinsicCall in the AST)
cargo test --lib       # 1497+ pass (all existing tests migrated)
bash benchmarks/build_and_bench.sh --correctness  # all benchmarks match
```

---

## Phase 9 — Synthesis Engine

**Depends on**: Phase 8 (AST + parsing), Phase 8G (intrinsic/inop removal,
metadata-dispatch for interpreted execution), Phase 7 (plugin system for
external SMT solver), Phase 1B (property system for operator cost model)

### Goal

When a definition has a derivation block but NO body (`body: None`), the
`brief derive` command invokes the synthesis engine to infer the minimal
formula satisfying all examples.

### Step 9.0 — Create `src/derive.rs` module

**File**: `src/derive.rs` (new)

**What**: Create the top-level derivation module with three sub-modules and
the shared error type used across all derivation phases:

```rust
//! Derivation & synthesis engine.
//! 2026-07-11: Phase 9.0

pub mod engine;   // Synthesis orchestration, cost model, enumerative search
pub mod smt;      // SMT solver interface (via WASM plugin)
pub mod cli;      // CLI command handlers for `brief derive`
```

**Shared error type** (used by all derivation phases):

```rust
/// Errors from derivation example execution, synthesis, and CLI operations.
/// 2026-07-11: Phase 8.5, 9.5, 9.6, 11.3, 13.1 — shared across all derive phases.
pub enum DeriveError {
    /// An expression could not be evaluated at compile time.
    EvalFailed { example_index: usize, message: String },
    /// The function body produced a different result than expected.
    AssertionFailed { example_index: usize, expected: Value, got: Value, span: Span },
    /// A fallback value violates the function's postcondition.
    FallbackViolatesContract { example_index: usize, variant: String, value: Value },
    /// IO error reading/writing a file.
    Io { path: PathBuf, error: io::Error },
    /// File is not valid UTF-8.
    InvalidUTF8 { path: PathBuf, error: FromUTF8Error },
    /// Parse failed on a file.
    ParseFailed { path: PathBuf, error: SyntaxError },
    /// Synthesis produced no valid program.
    SynthesisFailed(String),
    /// Multiple errors during directory-level derive.
    Multiple(Vec<DeriveError>),
}
```

Also create `src/derive/` directory with `mod.rs`, `engine.rs`, `smt.rs`,
and `cli.rs`.

Register the module in `src/lib.rs`:
```rust
pub mod derive;
```

**Nesting check**: Three sibling sub-modules — no nesting.

### Step 9.1 — Define DSL of primitive operations for synthesis

**File**: `src/derive/engine.rs`

**What**: Define the grammar of valid Brief expressions that the synthesizer
can generate. Each operator has a cost weight (Occam's Razor) — the
synthesizer searches for the lowest-cost program satisfying all examples.

**The DSL grammar**:

```
Program  ::= Term(Expr)
Expr     ::= Const(i64 | bool | float)
           | Var(String)
           | BinOp(Expr, BinOp, Expr)
           | UnOp(UnOp, Expr)
           | Cmp(Expr, CmpOp, Expr)
           | Cond(Expr, Expr, Expr)   // if-else
Const    ::= integer literal | float literal | true | false
BinOp    ::= + | - | * | / | % | & | | | ^ | << | >>
UnOp     ::= - (negate) | ! (not)
CmpOp    ::= == | != | < | > | <= | >=
```

**Data structures**:

```rust
/// Cost weights for the Occam's Razor heuristic.
/// Lower cost = preferred by the synthesizer.
/// 2026-07-11: Phase 9.1
#[derive(Debug, Clone)]
pub struct CostModel {
    pub constant: u64,   // cost of a literal (default: 1)
    pub variable: u64,   // cost of reading a parameter (default: 1)
    pub unary_op: u64,   // cost of a unary operation (default: 2)
    pub binary_op: u64,  // cost of a binary operation (default: 3)
    pub branch: u64,     // cost of an if/else (default: 5)
}

impl Default for CostModel {
    fn default() -> Self {
        Self {
            constant: 1,
            variable: 1,
            unary_op: 2,
            binary_op: 3,
            branch: 5,
        }
    }
}

/// A synthesized program — the minimal expression tree satisfying examples.
/// 2026-07-11: Phase 9.1
#[derive(Debug, Clone)]
pub struct SynthesizedProgram {
    pub body: Vec<Statement>,   // The synthesized function body
    pub cost: u64,              // Total cost of this program
    pub operators_used: Vec<String>,  // Which operators were used (for diagnostics)
}
```

**Nesting check**: Three data structures, one default impl — no nesting.

**Tests**:
- `test_cost_model_defaults`: Default costs match expected values
- `test_cost_model_constant_lower_than_binary`: constant(1) < binary_op(3)

### Step 9.2 — Enumerative search (fallback when SMT unavailable)

**File**: `src/derive/engine.rs`

**What**: Implement depth-bounded enumerative search over the DSL grammar.
This is the fallback when no SMT solver is available. It enumerates all
valid program trees up to a maximum depth and returns the lowest-cost match.

**Algorithm**:

```rust
/// Enumerate all programs up to `max_depth` and return the lowest-cost
/// match for the given examples.
/// 2026-07-11: Phase 9.2
pub fn synthesize_enumerative(
    param_names: &[String],
    examples: &[DerivationExample],
    cost_model: &CostModel,
    max_depth: u8,
) -> Result<SynthesizedProgram, SynthesisError> {
    // Generate candidate expressions up to max_depth
    let mut best: Option<SynthesizedProgram> = None;
    let mut candidates = generate_expressions(param_names, max_depth, cost_model);

    // Sort by cost (lowest first)
    candidates.sort_by_key(|c| c.cost);

    for candidate in &candidates {
        if matches_all_examples(candidate, examples) {
            return Ok(candidate.clone());
        }
    }

    Err(SynthesisError::NoSolutionFound {
        examples_checked: examples.len(),
        max_depth,
    })
}

/// Generate all valid expressions up to `depth`.
/// 2026-07-11: Phase 9.2
fn generate_expressions(
    params: &[String],
    depth: u8,
    cost: &CostModel,
) -> Vec<SynthesizedProgram> {
    if depth == 0 {
        return vec![];
    }
    let mut results = Vec::new();

    // Base: constants and variables
    // (In a full implementation, this enumerates a bounded set of
    //  constants like 0, 1, -1, and all parameter names)

    // Recursive: unary and binary ops
    // (Combine sub-expressions via BinOp, UnOp, Cond)

    results
}
```

**Key constraint**: The search for constants is bounded to a small set
(0, 1, -1, likely-relevant powers of 2). Full constant enumeration is
handled by the SMT solver path. This enumerative path is intended for
simple programs only.

**Nesting check**: The `generate_expressions` function builds results
sequentially — depth is variable but the code structure is flat (collect
base, collect recursive, return).

**Tests**:
- `test_synthesize_simple_add`: `{ 2, 2 -> 4; 3, 5 -> 8; }` with
  max_depth=2 → `x + y`
- `test_synthesize_constant_only`: `{ _ -> 42; _ -> 42 }` → constant
- `test_synthesize_no_solution`: Impossible constraints with low max_depth →
  Err
- `test_synthesize_prefers_lowest_cost`: Multiple valid programs, cheapest
  returned
- `test_synthesize_max_depth_increases_search`: depth=1 fails, depth=2
  succeeds

### Step 9.3 — SMT solver interface

**File**: `src/derive/smt.rs`

**What**: Interface to an external SMT solver (e.g., Z3) via the Phase 7
WASM plugin system. The plugin sends an SMT-LIB query in QF_BV
(quantifier-free bitvector logic) and receives a synthesized expression.

**Note**: The SMT solver may not be available at build time. The engine
gracefully falls back to enumerative search (Step 9.2).

**Interface**:

```rust
/// Result of an SMT synthesis query.
/// 2026-07-11: Phase 9.3
#[derive(Debug, Clone)]
pub enum SynthesisResult {
    /// A valid program was found.
    Program(SynthesizedProgram),
    /// No program exists satisfying the constraints (unsat).
    Unsat,
    /// The solver timed out or was unavailable.
    Unknown(String),
}

/// Attempt to synthesize via the SMT WASM plugin.
/// Falls back to enumerative if the plugin is unavailable.
/// 2026-07-11: Phase 9.3
pub fn synthesize_via_smt(
    params: &[TypedParameter],
    ret_type: &Type,
    examples: &[DerivationExample],
    plugin_host: &PluginHost,
) -> Result<SynthesisResult, SynthesisError> {
    // 1. Build the SMT-LIB query string
    let query = build_smt_query(params, ret_type, examples)?;

    // 2. Call the plugin (Phase 7 provides the WASM plugin interface)
    let response = match plugin_host.call("smt/synth", &query) {
        Ok(resp) => resp,
        Err(e) => return Ok(SynthesisResult::Unknown(format!(
            "SMT plugin unavailable: {}. Falling back to enumerative search.", e
        ))),
    };

    // 3. Parse the SMT response into a SynthesizedProgram
    parse_smt_response(&response)
}
```

**SMT-LIB query structure** (for `defn swap(x: UInt16) -> UInt16; { 0x1234 -> 0x3412 }`):

```lisp
; QF_BV synthesis query for swap
(declare-const x (_ BitVec 16))

; Constraints from examples
(assert (= (f #x1234) #x3412))
(assert (= (f #x00FF) #xFF00))

; Synthesis directive
(synth-fun f ((x (_ BitVec 16))) (_ BitVec 16)
    ((Start Symbol) (
        (Constant (_ BitVec 16))
        (Variable x)
        (bvadd Start Start)
        (bvsub Start Start)
        (bvmul Start Start)
        (bvand Start Start)
        (bvor Start Start)
        (bvxor Start Start)
        (bvshl Start Start)
        (bvlshr Start Start)
        (bvneg Start)
        (bvnot Start)
    ))
)

(check-synth)
```

**Building the query**: The function maps Brief types to SMT-LIB sorts:
- `UInt8`, `Int8` → `(_ BitVec 8)`
- `UInt16`, `Int16` → `(_ BitVec 16)`
- `UInt32`, `Int32`, `Float32` → `(_ BitVec 32)`
- `UInt64`, `Int64`, `Float64` → `(_ BitVec 64)`
- `Bool` → `Bool`

For each example, emit an `(assert (= (f <inputs>) <output>))` constraint.

**Parsing the response**: The SMT solver returns either:
- `(define-fun f (...) ...)` — a synthesized function definition
- `unsat` — no program exists (should not happen for inductive synthesis
  with consistent examples)
- `unknown` — solver could not decide

Parse the `define-fun` body back into a Brief `Vec<Statement>`.

**Nesting check**: Sequential logic: build query → try plugin → parse
response. Depth 1.

**Tests**:
- `test_build_smt_query_simple`: Build query for `add(x, y)`, verify
  constraints contain `(= (f #x02 #x02) #x04)`
- `test_build_smt_query_single_param`: Build for `swap(x)`
- `test_parse_smt_response_define_fun`: Parse `(define-fun f ((x (_ BitVec 16))) (_ BitVec 16) (bvadd x x))` → `term x + x;`
- `test_smt_plugin_unavailable_fallback`: Plugin returns error →
  SynthesisResult::Unknown
- `test_end_to_end_swap_synthesis`: `{ 0x1234 -> 0x3412; 0x00FF -> 0xFF00; }`
  → `term (x << 8) | (x >> 8);`

### Step 9.4 — `#no_derive` pragma handler

**What**: The `#no_derive` pragma tells the compiler to skip synthesis for
a specific definition, keeping the body empty during active drafting.

**File**: `src/derive/engine.rs`

```rust
/// Check whether a definition should be derived (synthesized) or skipped.
/// 2026-07-11: Phase 9.4
pub fn should_derive(defn: &Definition) -> bool {
    // Never overwrite an existing body
    if defn.body.is_some() {
        return false;
    }
    // Nothing to synthesize from
    if defn.derivation.is_none() {
        return false;
    }
    // Respect #no_derive pragma
    if has_pragma(&defn.annotations, "no_derive") {
        return false;
    }
    true
}

/// Check if the annotation list contains a specific pragma.
/// 2026-07-11: Phase 9.4
fn has_pragma(annotations: &[Annotation], name: &str) -> bool {
    annotations.iter().any(|a| a.name == name)
}
```

**Note**: The `#no_derive` pragma is parsed by the existing
`parse_hashtag_modifiers()` infrastructure (from Phase 1A). No parser
changes are needed — the pragma is simply a `#` annotation on the
definition.

**Usage in source**:
```brief
#no_derive
defn complex_fn(x: Int) -> Int := {
    0 -> 0; // Still drafting — leave the body empty
};
```

**Nesting check**: Three guard clauses — depth 1.

**Tests**:
- `test_no_derive_pragma_blocks_synthesis`: `#no_derive defn f(x) := {0->0;};`
  → should_derive returns false
- `test_no_derive_pragma_absent`: No pragma → should_derive returns true
- `test_no_derive_respects_existing_body`: Body present → should_derive false
  (regardless of pragma)
- `test_has_pragma_true`: Annotation list contains `"no_derive"` → true
- `test_has_pragma_false`: No match → false

### Step 9.5 — Orchestrate synthesis in the derive engine

**File**: `src/derive/engine.rs`

**What**: The top-level synthesis function that coordinates the SMT solver
and enumerative search, with proper fallback:

```rust
/// Synthesize a function body from its derivation block.
/// 2026-07-11: Phase 9.5
pub fn synthesize_body(
    defn: &Definition,
    universe: &TypeUniverse,
    plugin_host: Option<&PluginHost>,
    depth_limit: u8,
) -> Result<SynthesizedProgram, SynthesisError> {
    let Some(derivation) = &defn.derivation else {
        return Err(SynthesisError::NoDerivationBlock);
    };

    let param_names: Vec<String> = defn.parameters.iter()
        .map(|p| p.name.clone())
        .collect();

    // Try SMT first if plugin is available
    if let Some(host) = plugin_host {
        let result = smt::synthesize_via_smt(
            &defn.parameters,
            &defn.output_type,
            &derivation.examples,
            host,
        )?;
        match result {
            SynthesisResult::Program(prog) => return Ok(prog),
            SynthesisResult::Unsat => return Err(SynthesisError::Unsat),
            SynthesisResult::Unknown(_) => {
                // Fall through to enumerative
            }
        }
    }

    // Fallback: enumerative search
    engine::synthesize_enumerative(
        &param_names,
        &derivation.examples,
        &CostModel::default(),
        depth_limit,
    )
}
```

**Nesting check**: The function uses early returns for each decision point
— depth 1 throughout.

**Tests**:
- `test_synthesize_via_smt_first`: SMT available and succeeds → uses SMT
  result
- `test_synthesize_fallback_on_smt_failure`: SMT unavailable → enumerative
  search

### Step 9.6 — Surgical source-file write-back

**What**: After synthesis, insert the generated body into the source file
at the correct byte offset, preserving all existing formatting and comments.

**File**: `src/derive/cli.rs`

**The core challenge**: AST pretty-printing destroys formatting. Instead,
record byte offsets during parsing and use surgical byte-level insertion.

**Prerequisite: `parse_file_with_offsets`**:

Before write-back can happen, the derivation CLI needs the raw source bytes
alongside the parsed AST. The derivation block's `span` field (recorded
during parsing) provides the exact byte offset for insertion. The source
bytes are passed through unchanged alongside the AST.

```rust
/// Parse a Brief source file and return both the AST and the raw source bytes.
/// The source bytes are used for byte-offset surgical write-back.
/// 2026-07-11: Phase 9.6
fn parse_file_with_offsets(path: &Path) -> Result<(Program, Vec<u8>), DeriveError> {
    let source = std::fs::read(path).map_err(|e| DeriveError::Io {
        path: path.to_path_buf(),
        error: e,
    })?;
    let source_str = String::from_UTF8(source.clone())
        .map_err(|e| DeriveError::InvalidUTF8 {
            path: path.to_path_buf(),
            error: e,
        })?;
    let mut parser = Parser::new(&source_str);
    let program = parser.parse()
        .map_err(|e| DeriveError::ParseFailed {
            path: path.to_path_buf(),
            error: e,
        })?;
    Ok((program, source))
}
```

The byte offsets are stored in `DerivationBlock.span.end`, which points to
the byte AFTER the closing `}` of the derivation block. This is the correct
insertion point for the synthesized body.

**How it works**:

1. **During parsing**, the parser records the end offset of the signature
   (for State A / drafting) or the end offset of the derivation block's
   opening `{` ... `}` (for State B). This is stored in the
   `DerivationBlock.span`.

2. **During write-back**, the CLI:
   a. Reads the entire source file into a `Vec<u8>`
   b. Locates the insertion point (the byte offset of `end` in the
      derivation block's span — this is the character AFTER the closing `}`)
   c. Generates the body string: ` { term x + y; }`
   d. Splits the bytes at the insertion point, inserts the body string,
      and writes the result back
   e. For State A: Also replaces the final `;` handling — the body is
      inserted before the `;`

**State A — Drafting to Resolved**:

Before:
```
defn add(x: Int, y: Int) -> Int := { 2, 2 -> 4; 3, 5 -> 8; };
                                                              ^-- insertion point (after })
```

After:
```
defn add(x: Int, y: Int) -> Int := { 2, 2 -> 4; 3, 5 -> 8; } {
    term x + y;
};
```

Note: The original `}` was the closing brace of the derivation block. The
original `;` terminated the definition. After insertion, the `}` of the
derivation block is still present, followed by the synthesized body, then
`;` terminates the definition.

**State B — Already resolved** (no change, additive-only rule):
```
defn add(x: Int, y: Int) -> Int { term x + y; } := { 2, 2 -> 4; ... };
```
→ Write-back does nothing (body already present).

**Implementation**:

```rust
/// Write the synthesized body back into the source file.
/// Uses byte-level surgical insertion — does NOT pretty-print the AST.
/// 2026-07-11: Phase 9.6
fn write_back_body(
    source_path: &Path,
    derivation: &DerivationBlock,
    body_str: &str,
) -> Result<(), DeriveError> {
    let source = fs::read(source_path).map_err(|e| DeriveError::Io {
        path: source_path.to_path_buf(),
        error: e,
    })?;

    // Insertion point: byte offset after the derivation block's closing `}`
    let insert_at = derivation.span.end as usize;

    // Build the new content: before derivation + body + after derivation
    let mut new_source = Vec::with_capacity(source.len() + body_str.len() + 4);
    new_source.extend_from_slice(&source[..insert_at]);
    new_source.extend_from_slice(body_str.as_bytes());
    new_source.extend_from_slice(&source[insert_at..]);

    // Write back
    fs::write(source_path, &new_source).map_err(|e| DeriveError::Io {
        path: source_path.to_path_buf(),
        error: e,
    })?;

    Ok(())
}
```

**Important**: The insertion point must be computed so that:
- The derivation block's `}` is preserved
- The body is inserted between `}` and `;`
- The final `;` remains as the definition terminator

The span stored in `DerivationBlock` should point to the exact byte after
the closing `}`:

```
defn f() := { ... };
            ^         ^
            start     end (= position of ;)
```

If the derivation block has `span = (start_of_brace, end_of_brace)`, and
the closing `}` is at byte `end_of_brace`, then insertion at
`end_of_brace` + 1 (after the `}`) is correct. The `;` follows.

Actually, let's be precise:
```
defn f(x: Int) -> Int := { 0 -> 0; };
                         ^             ^
                         brace_open    brace_close = end
```
The span of the derivation block is `(brace_open, brace_close)`. After
`brace_close` comes `;`. We insert between `brace_close` and `;`.

So the insertion point is `derivation.span.end` (the byte after `}`).

Wait: `Span` in Rust is typically `(start, end)` where `start` is the byte
of the first token and `end` is the byte AFTER the last token. So
`derivation.span.end` is already past the `}`. Then we insert there, and
the `;` is at `derivation.span.end`.

This means `insert_at = derivation.span.end` is correct for insertion
between the `}` and `;`.

**Body string format**:
```rust
fn format_synthesized_body(program: &SynthesizedProgram) -> String {
    let mut out = String::new();
    out.push_str(" {\n");
    for stmt in &program.body {
        out.push_str("    ");
        out.push_str(&format!("{};\n", stmt));
    }
    out.push_str("}");
    out
}
```

**Nesting check**: The write-back function is sequential (read, split,
extend, write) — depth 1. The body formatter is a loop — depth 1.

**Tests**:
- `test_write_back_draft_to_resolved`: Draft source → after write-back,
  body is present between derivation and `;`
- `test_write_back_preserves_formatting`: Leading whitespace, comments,
  indentation all intact after write-back
- `test_write_back_no_change_already_bodied`: Body present → file unchanged
- `test_write_back_nonexistent_file`: Missing file → Io error

### Step 9.7 — Build import DAG for `--all`

**File**: `src/derive/cli.rs`

**What**: When `--all` is passed, `brief derive` must process all
transitive imports in dependency order (topological sort of the DAG).

**Algorithm**:

```rust
/// Derive all definitions in the given source file AND all its transitive
/// imports, processed in topological order (leaf modules first).
/// 2026-07-11: Phase 9.7
pub fn derive_all(source_path: &Path) -> Result<(), DeriveError> {
    // 1. Parse the source and build the import DAG
    let program = parse_file(source_path)?;
    let import_graph = build_import_graph(&program, source_path)?;

    // 2. Topological sort (Kahn's algorithm)
    let order = topological_sort(&import_graph)?;

    // 3. Process each file in order
    for file_path in &order {
        derive_file(file_path)?;
    }

    Ok(())
}
```

The import graph is a DAG where:
- Vertices = `.bv` files
- Edges = `import` statements

The existing `ImportResolver` (Phase 1A) already parses imports and
resolves them. This step reuses that infrastructure.

```rust
/// Build a DAG of imports for the given program.
/// Returns a list of (file_path, imports) pairs.
/// 2026-07-11: Phase 9.7
fn build_import_graph(
    program: &Program,
    source_path: &Path,
) -> Result<Vec<(PathBuf, Vec<PathBuf>)>, DeriveError> {
    let mut graph = Vec::new();
    let source_dir = source_path.parent().unwrap_or(Path::new("."));

    for import in &program.imports {
        let resolved = resolve_import_path(&import.path, source_dir)?;
        let sub_program = parse_file(&resolved)?;
        graph.push((resolved.clone(), Vec::new()));

        // Recurse into sub-imports
        let sub_graph = build_import_graph(&sub_program, &resolved)?;
        graph.extend(sub_graph);
    }

    graph.push((source_path.to_path_buf(), Vec::new()));
    Ok(graph)
}
```

**Nesting check**: The `build_import_graph` function iterates imports and
recurses — the recursion depth equals import depth, but the function body
is flat (push, recurse, push). Acceptable.

**Tests**:
- `test_derive_all_single_file`: No imports → single file processed
- `test_derive_all_simple_chain`: `A -> B -> C` → C processed first, then B,
  then A
- `test_derive_all_diamond`: `A -> B, A -> C, B -> D, C -> D` → D first,
  B and C in any order, A last
- `test_derive_all_cycle`: `A -> B -> A` → error (cycle detected)

### Step 9.8 — Directory scanning mode

**File**: `src/derive/cli.rs`

```rust
/// Derive all .bv files in a directory (recursive).
/// 2026-07-11: Phase 9.8
pub fn derive_directory(dir_path: &Path) -> Result<(), DeriveError> {
    let entries = walkdir::WalkDir::new(dir_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| {
            e.path().extension().map(|ext| ext == "bv").unwrap_or(false)
        })
        .collect::<Vec<_>>();

    // Process files in parallel (they may be independent)
    let results: Vec<Result<(), DeriveError>> = entries
        .par_iter()
        .map(|entry| derive_file(entry.path()))
        .collect();

    // Collect errors (if any)
    let errors: Vec<DeriveError> = results.iter()
        .filter_map(|r| r.as_ref().err().cloned())
        .collect();

    if errors.is_empty() {
        Ok(())
    } else {
        Err(DeriveError::Multiple(errors))
    }
}
```

**Note**: Directory scanning does NOT analyze inter-file dependencies. Each
file is processed independently. For dependency-aware derivation, use
`--all <entry_point>`.

**Nesting check**: The function collects entries (iterator chain) then
processes in parallel (iterator chain). Depth 1.

**Tests**:
- `test_derive_directory_empty`: Empty directory → ok (no files)
- `test_derive_directory_single_file`: One .bv file → derived
- `test_derive_directory_multiple_files`: Multiple .bv files → all derived
- `test_derive_directory_mixed_extensions`: `.bv` and `.txt` files → only
  `.bv` processed
- `test_derive_directory_nested`: Subdirectories → all `.bv` files found

---

## Phase 10 — Contract-Guided Synthesis (Deductive)

**Depends on**: Phase 9 (synthesis engine), existing proof engine

### Goal

Synthesize function bodies from contracts alone, without example pairs. The
SMT solver finds a body satisfying `∀x. P(x) ⇒ Q(x, f(x))`. This is
**Deductive Synthesis** — correctness for ALL valid inputs, not just the
provided examples.

### Step 10.0 — Translate contracts to SyGuS queries

**File**: `src/derive/engine.rs`

**What**: When a definition has contracts `[pre][post]` but neither body
nor examples (or an empty `:= {}`), synthesize a body from the contracts.

```brief
defn abs(val: Int) -> Int
    [true]
    [result >= 0]
    [result == val || result == -val]
:= {};  // Empty derivation — synthesize from contracts
```

**SyGuS (Syntax-Guided Synthesis) query**:

```lisp
; SyGuS query for abs
(set-logic LIA)

(declare-const val Int)

(define-fun pre ((val Int)) Bool
    true)

(define-fun post ((val Int) (result Int)) Bool
    (and (>= result 0) (or (= result val) (= result (- val)))))

(synth-fun f ((val Int)) Int
    ((Start Int (
        val
        0 1 (- 0) (- 1)
        (+ Start Start)
        (- Start Start)
        (* Start Start)
        (ite (<= Start Start) Start Start)
        (ite (< Start Start) Start Start)
        (ite (= Start Start) Start Start)
        (ite (>= Start Start) Start Start)
        (ite (> Start Start) Start Start)
    )))
)

(constraint (forall ((val Int))
    (=> (pre val) (post val (f val)))))

(check-synth)
```

**Implementation**:

```rust
/// Synthesize a body from contracts when no examples are provided.
/// 2026-07-11: Phase 10.0
pub fn synthesize_from_contracts(
    defn: &Definition,
    universe: &TypeUniverse,
    plugin_host: Option<&PluginHost>,
) -> Result<SynthesizedProgram, SynthesisError> {
    let Some(contract) = &defn.contract else {
        return Err(SynthesisError::NoContract);
    };

    // Build the SyGuS query from the contract
    let query = build_sygus_query(defn, contract)?;

    // Try SMT solver via plugin
    if let Some(host) = plugin_host {
        let response = host.call("smt/synth", &query)?;
        return parse_smt_response(&response);
    }

    // Without SMT solver, contract-guided synthesis is not possible
    Err(SynthesisError::SmtUnavailable)
}
```

**Nesting check**: Sequential logic — depth 1.

**Tests**:
- `test_synthesize_abs_from_contracts`: Contracts for `abs` with empty
  derivation → `if val < 0 { -val } else { val }`
- `test_synthesize_clamp_from_contracts`: `[result >= 0]` `[result <= 100]`
  → `if val < 0 { 0 } else if val > 100 { 100 } else { val }`
- `test_synthesize_no_contract`: No contract on definition → error
- `test_synthesize_no_smt_plugin`: No SMT available → SmtUnavailable error

### Step 10.1 — Contract + Example hybrid synthesis

**What**: When BOTH contracts and examples are present, use examples as
test oracles and contracts as the search-space boundary. The synthesizer
finds the cheapest program that:
1. Satisfies all input-output examples
2. Respects the precondition-postcondition for ALL inputs

This is strictly more powerful than examples alone — it acts as a formal
generalization guarantee.

```brief
defn clamp(val: Int) -> Int
    [result >= 0]
    [result <= 100]
:= {
    0 -> 0;
    200 -> 100;
};
```

**File**: `src/derive/engine.rs`

```rust
/// Synthesize from both contracts and examples.
/// The SMT query includes both constraints.
/// 2026-07-11: Phase 10.1
pub fn synthesize_hybrid(
    defn: &Definition,
    examples: &[DerivationExample],
    plugin_host: Option<&PluginHost>,
) -> Result<SynthesizedProgram, SynthesisError> {
    // Build a query with both example constraints AND contract constraints
    let query = build_hybrid_query(defn, examples)?;
    // ... send to SMT solver
}
```

**Tests**:
- `test_hybrid_synthesis_satisfies_both`: Output matches examples AND
  contracts
- `test_hybrid_synthesis_broken_contract`: Examples suggest a solution that
  violates the contract → solver finds different solution
- `test_hybrid_synthesis_unsat`: No program satisfies both examples and
  contracts → error

### Step 10.2 — LLVM optimization metadata from proven contracts

**File**: `src/backend/llvm/` (various emission sites)

**What**: When contracts are proven (either by the proof engine or by SMT
synthesis), emit them as LLVM metadata for optimizer leverage. This is the
"positive incentive loop": more contracts → more metadata → faster code.

**Supported metadata types**:

| Contract Pattern | LLVM Metadata | Effect |
|-----------------|---------------|--------|
| `[result >= min && result <= max]` | `!range !{min, max+1}` (max is **exclusive**) | GVN, jump threading, loop optimization |
| `[ptr != null]` | `nonnull` attribute | Null check elimination, register promotion |
| `[val % 2 == 0]` | `@llvm.assume` | Division → shift optimization |
| `[index < len]` | `!range` on index | Bounds check elimination, vectorization |

**Implementation** (in `emit_stmt.rs`, `emit_expr.rs`, or `mod.rs`):

```rust
/// Emit LLVM metadata for contract-proven invariants.
/// LLVM !range metadata uses exclusive upper bound (max+1).
/// 2026-07-11: Phase 10.2 — inclusive→exclusive conversion.
fn emit_contract_metadata(
    contract: &Contract,
    builder: &mut LlvmBuilder,
    function_context: &FunctionContext,
    out: &mut String,
) {
    // Extract range constraints from postcondition
    // extract_range_constraint returns INCLUSIVE bounds;
    // LLVM !range expects EXCLUSIVE upper bound.
    if let Some((min, max)) = extract_range_constraint(&contract.post) {
        writeln!(out, "  !range !{{ {} , {} }}", min, max.saturating_add(1)).ok();
    }

    // Extract non-null constraints
    if has_nonnull_constraint(&contract.post) {
        writeln!(out, "  !nonnull").ok();
    }
}
```

**Helper to extract range constraints** (returns **inclusive** bounds):

```rust
/// Extract a `[result >= min && result <= max]` pattern from a postcondition.
/// Returns `Some((min, max))` where BOTH min and max are INCLUSIVE.
/// The caller must convert to LLVM's exclusive upper bound via saturating_add(1).
/// 2026-07-11: Phase 10.2
fn extract_range_constraint(expr: &Expr) -> Option<(i64, i64)> {
    // Normalize to handle old-style vs new-style BinaryOp
    let expr = expr.normalize_to_old();

    match expr {
        // result >= min && result <= max
        Expr::And(left, right) => {
            let left_range = extract_single_bound(left)?;
            let right_range = extract_single_bound(right)?;
            // Combine: one must be lower bound, the other upper bound
            match (left_range, right_range) {
                ((a, Geq), (b, Leq)) if a <= b => Some((a, b)),
                ((a, Leq), (b, Geq)) if b <= a => Some((b, a)),
                _ => None,
            }
        }
        // Simple single bound
        _ => {
            let (val, op) = extract_single_bound(expr)?;
            match op {
                Geq => Some((val, i64::MAX)),
                Leq => Some((i64::MIN, val)),
                _ => None,
            }
        }
    }
}

enum BoundOp { Geq, Leq }
```

**Nesting check**: The `extract_range_constraint` function uses guard
clauses and match patterns — depth 1.

**Tests**:
- `test_contract_range_metadata`: `[result >= 0 && result <= 100]` →
  `!range !{ 0, 101 }` (exclusive upper bound)
- `test_contract_nonnull_metadata`: `[ptr != null]` → `nonnull` attribute
  on parameter
- `test_contract_metadata_emitted_in_ir`: Compile a function with contract,
  verify LLVM output contains the metadata
- `test_contract_no_metadata_for_tautology`: `[true]` → no metadata emitted

---

## Phase 11 — Sad-Path Derivation (FFI Error Recovery)

**Depends on**: Phase 8 (derivation AST), Phase 9 (synthesis engine),
existing FFI/frgn infrastructure

### Goal

When a `frgn` call returns `Result<T, E>` but the function signature
returns `T`, the derivation block provides the error-to-value mapping, and
the compiler synthesizes the `match` boilerplate.

### Background

Foreign function calls are inherently unpredictable — they can fail due to
network timeouts, missing files, or memory limits. Brief's `frgn` signature
returns a monadic `Result<T, E>` to model this. However, writing the
boilerplate `match` statement for every FFI call pollutes the happy path.

**Sad-path derivation** lets the developer write only the happy path in the
body, and declare the error-to-value mapping in a `:=` block.

### Step 11.0 — Detect monadic return type from `frgn` calls

**File**: `src/typechecker.rs` and `src/derive/sad_path.rs`

**What**: When type-checking a function body, if any `frgn` call returns
`Result<T, E>` and the function's declared return type is `T`, check for a
derivation block that provides the `E -> T` mapping.

```brief
frgn read_config_file(path: String) -> Result<Config, FileError>;

defn load_config(path: String) -> Config {
    let cfg = frgn read_config_file(path);
    term cfg;
} := {
    FileError::NotFound -> Config::Default();
    FileError::PermissionDenied -> Config::SecureDefault();
};
```

**Detection logic** (added to type-checking of `frgn` calls):

```rust
/// Check if a frgn call returns a Result type that needs sad-path handling.
/// Returns Some((ok_type, error_type)) if the return type is Result<T, E>.
/// 2026-07-11: Phase 11.0
fn check_frgn_result_type(
    call: &FrgnCall,
    return_type: &Type,
    defn_return_type: &Type,
) -> Option<(Type, Type)> {
    // Check: return_type is Result<ok_ty, err_ty>
    // AND defn_return_type == ok_ty
    if let Some((ok_ty, err_ty)) = extract_result_types(return_type) {
        if types_equivalent(defn_return_type, &ok_ty) {
            return Some((ok_ty, err_ty));
        }
    }
    None
}
```

**During type-checking**: If a sad-path is detected:
1. No derivation block → compile error:
   `"frgn call returns Result<T, E> but function expects T. Add a derivation
    block or handle the Result explicitly."`
2. Derivation block present → validate exhaustiveness (Step 11.1)

**Tests**:
- `test_sad_path_detected`: `frgn f() -> Result<Int, Err>` in `defn g() -> Int`
  → detection returns Some
- `test_sad_path_not_detected_no_result`: `frgn f() -> Int` → no detection
- `test_sad_path_not_detected_matching_result`: Function also returns
  `Result<Int, Err>` → no detection (handled normally)
- `test_sad_path_no_derivation_error`: Require derivation → compile error

### Step 11.1 — Validate exhaustiveness of sad-path mapping

**File**: `src/derive/sad_path.rs` (new)

**What**: Verify that the derivation block covers ALL variants of the error
type `E`.

```rust
/// Validate that the derivation block covers every variant of the error type.
/// 2026-07-11: Phase 11.1
fn validate_sad_path_exhaustiveness(
    error_type: &Type,
    examples: &[DerivationExample],
    universe: &TypeUniverse,
) -> Result<(), Vec<DeriveError>> {
    // 1. Get all variants of the error type
    let error_variants = get_enum_variants(error_type, universe)?;

    // 2. Collect which variants are covered by the derivation
    let covered: HashSet<&str> = examples.iter()
        .filter_map(|ex| extract_error_variant(&ex.inputs[0]))
        .collect();

    // 3. Check each variant is covered
    let mut uncovered = Vec::new();
    for variant in &error_variants {
        if !covered.contains(variant.as_str()) {
            uncovered.push(variant.clone());
        }
    }

    if uncovered.is_empty() {
        Ok(())
    } else {
        Err(vec![DeriveError::UncoveredErrorVariants(uncovered)])
    }
}
```

**Helper — extract error variant from input expression**:

```rust
/// Given `FileError::NotFound`, extract `"NotFound"`.
/// 2026-07-11: Phase 11.1
fn extract_error_variant(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Path(path) if path.segments.len() == 2 => {
            Some(path.segments[1].clone())
        }
        _ => None,
    }
}
```

**Tests**:
- `test_exhaustiveness_all_covered`: Error has 3 variants, derivation covers
  all 3 → Ok
- `test_exhaustiveness_missing_variant`: 3 variants, derivation covers 2 →
  error with uncovered variant name
- `test_exhaustiveness_no_error_type`: Not an enum type → Ok (no variants)
- `test_extract_error_variant`: `FileError::NotFound` → `"NotFound"`

### Step 11.2 — Synthesize sad-path branching from derivation

**File**: `src/derive/sad_path.rs`

**What**: Generate the conditional branching that wraps a `frgn` call result
with happy-path/sad-path handling. Uses the existing `Statement::Guarded`
AST node (one-shot conditional with a guard expression) and sequential
fallthrough — each guard is tried in order; the first match wins.

```rust
/// Generate a guarded if-else chain that wraps a frgn call result.
/// 2026-07-11: Phase 11.2
fn synthesize_sad_path(
    frgn_call: &ForeignSignature,
    ok_type: &Type,
    examples: &[DerivationExample],
) -> Vec<Statement> {
    let result_var = format!("%__result_{}", frgn_call.name);
    let ok_var = format!("%__ok_{}", frgn_call.name);

    let mut statements = Vec::new();

    // 1. Bind the frgn call result to a variable
    statements.push(Statement::Let {
        name: result_var.clone(),
        value: Box::new(Expr::Call {
            // The frgn call — left as an Expr::Call to the foreign function
            name: frgn_call.name.clone(),
            type_args: vec![],
            args: /* from the body context */ vec![],
        }),
        ty: Type::Applied("Result".to_string(), vec![ok_type.clone(), /* error type */]),
    });

    // 2. For each error variant, emit a Guarded check:
    // [result is Err(NotFound)] { term Config::Default(); }
    for example in examples {
        let error_variant = extract_error_variant(&example.inputs[0])
            .unwrap_or_default();
        let guard = build_is_err_check(&result_var, &error_variant);
        statements.push(Statement::Guarded {
            condition: guard,
            statements: vec![Statement::Term {
                values: vec![Some(example.output.clone())],
                swan_song: None,
                modifiers: vec![],
            }],
            metadata: HashMap::new(),
        });
    }

    // 3. Fallthrough to the happy path (no guard):
    // [true] { term ok_value; }
    let fallback_guard = Expr::Bool(true);
    statements.push(Statement::Guarded {
        condition: fallback_guard,
        statements: vec![Statement::Term {
            values: vec![Some(Expr::Identifier(ok_var))],
            swan_song: None,
            modifiers: vec![],
        }],
        metadata: HashMap::new(),
    });

    statements
}

/// Build a guard expression checking if result is Err(VariantName).
/// 2026-07-11: Phase 11.2
fn build_is_err_check(result_var: &str, variant: &str) -> Expr {
    // result is Err(VariantName) — simplified guard using Expr::Match
    // on the Result's discriminant.
    Expr::Match {
        expr: Box::new(Expr::Identifier(result_var.to_string())),
        arms: vec![
            MatchArm {
                pattern: MatchPattern::Variant {
                    name: "Err".to_string(),
                    fields: vec![Pattern::Wildcard],
                },
                guard: Some(Box::new(Expr::BinaryOp {
                    left: Box::new(Expr::Identifier("__discriminant".to_string())),
                    op: BinaryOp::Eq,
                    right: Box::new(Expr::Identifier(variant.to_string())),
                })),
                body: Box::new(Expr::Bool(true)),
            },
        ],
    }
}
```

**Note:** If `Expr::Match` is unavailable (pattern matching not yet
implemented), the fallback is to use the `is_err` intrinsic via metadata:

```rust
fn build_is_err_check(result_var: &str, variant: &str) -> Expr {
    // frgn __is_err_and_variant(result, variant)
    Expr::Call {
        name: "__is_err_and_variant".to_string(),
        type_args: vec![],
        args: vec![
            Expr::Identifier(result_var.to_string()),
            Expr::String(variant.to_string()),
        ],
    }
}
```

This call dispatches through the metadata-driven `interpreter_impl` /
`llvm_instr` mechanism established in Phase 8G. The function
`__is_err_and_variant` is defined in the prelude as a standard `defn`
with `interpreter_impl <~ "check_err_variant"`.

**Tests**:
- `test_sad_path_synthesize_if_else`: Generate guarded chain, verify all
  error variants present in order
- `test_sad_path_integration`: Full pipeline: frgn + derivation →
  synthesized body passes compile-time tests
- `test_sad_path_fallback_happy_path`: When result is Ok, fallthrough to
  happy path executed

### Step 11.3 — SMT verify fallback values satisfy downstream contracts

**File**: `src/derive/sad_path.rs`

**What**: If the function has a postcondition `[result.url != ""]`, verify
that ALL sad-path fallback values also satisfy the contract.

```rust
/// Verify that all sad-path fallback values satisfy the function's contract.
/// 2026-07-11: Phase 11.3
fn verify_fallbacks_satisfy_contract(
    function_return_type: &Type,
    examples: &[DerivationExample],
    postcondition: &Expr,
    interpreter: &mut Interpreter,
) -> Result<(), Vec<DeriveError>> {
    let mut violations = Vec::new();

    for (i, example) in examples.iter().enumerate() {
        // Evaluate the fallback value
        let fallback_value = interpreter.eval_expr(&example.output, &None)?;

        // Evaluate the postcondition with `result` bound to the fallback.
        // Uses the interpreter's eval_with_result helper (thin addition),
        // which temporarily binds `result` via the frame stack.
        let satisfied = interpreter.eval_with_result(postcondition, fallback_value.clone())?;

        if satisfied != Value::Bool(true) {
            violations.push(DeriveError::FallbackViolatesContract {
                example_index: i,
                variant: extract_error_variant(&example.inputs[0]).unwrap_or_default(),
                value: fallback_value,
            });
        }
    }

    if violations.is_empty() { Ok(()) } else { Err(violations) }
}
```

**Interpreter addition** (thin helper, added to `src/interpreter.rs`):

```rust
impl Interpreter {
    /// Evaluate an expression in a context where `result` is bound to a value.
    /// Used for postcondition verification of derivation fallbacks.
    /// 2026-07-11: Phase 11.3
    pub fn eval_with_result(&mut self, expr: &Expr, result: Value) -> Result<Value, RuntimeError> {
        // Save existing result binding (if any), push new one
        let prev = self.current_result.take();
        self.current_result = Some(result);

        // Evaluate the postcondition expression
        let out = self.eval_expr(expr, &None);

        // Restore previous binding
        self.current_result = prev;

        out
    }
}
```

This requires adding a `current_result: Option<Value>` field to the
`Interpreter` struct, which is initialized to `None` and set during
postcondition evaluation. The field is distinct from the frame stack —
it's a single-value convenience for quickly evaluating expressions
that reference `result`.

**Tests**:
- `test_fallback_satisfies_contract`: `Config::Default()` has non-empty URL
  → passes
- `test_fallback_violates_contract`: `Config::Default()` has empty URL
  → compile error
- `test_fallback_contract_mixed`: Some pass, some fail → error with indexes

---

## Phase 12 — `.dbvl` Semantic Archive and Decoupled Backend Architecture

**Depends on**: Phases 0–7 (especially Phase 7 for plugin system), Phase 5
for `.dbvl` format support

### Goal

Decouple the compiler front-end from backends via a serialized archive
in `.dbvl` format. The archive is just a `.dbvl` file with a specific tag
convention — any tool that already reads `.dbvl` (GLUE system, interpreter's
`dbvl_cache`, any backend) can consume it without changes. Backends become
independent executables that consume this archive.

### Step 12.0 — Define archive schema

**What**: The archive file is a `.dbvl` file following the same format
conventions as `glue.dbvl` and `bridge-exports.dbvl` — each line is a
**tagged, comma-separated entry** with `{ }` brace blocks for complex
structures (maps, lists). The first field is the tag, which dispatches the
entry type to the consumer. No new file extension or parsing infrastructure
is needed — any existing `.dbvl` reader can consume the archive.

**File**: `docs/architecture/archive.md` (new)

**Format** (inline, no `.dbvs` schema needed initially):

```dbvl
// Semantic archive — a .dbvl file with tagged entries.
// Tag is first field: type, defn, txn, frgn.
//
// Conventions:
//   - Lists of simple values:  `{item1 item2 item3}` (space-separated inside braces)
//   - Pairs/maps:              `{key1:val1 key2:val2}` (space-separated inside braces)
//   - Nested structures:       `{name1:{k:v} name2:{k:v}}`
//   - Strings with commas:     `"quoted, string"`
//
// Type entries:
type,String,{ptr:Ptr<UInt8> len:Int},{bytes:24 alignment:8 llvm:%String tbaa:String}
type,Config,{url:String retries:Int},{bytes:24 alignment:8}

// Definition entries (name, sig_params|pipe|separated, sig_ret, body, derivation,
// metadata, contracts):
defn,swap,x:UInt16,UInt16,"{term (x << 8) | (x >> 8)}",{ex:{inputs:0x1234 output:0x3412} ex:{inputs:0x00FF output:0xFF00}},{jira:SEC-42},{pre:true post:result==(x<<8)|(x>>8)}
defn,add,x:Int|y:Int,Int,"{term x + y}",{ex:{inputs:2|2 output:4} ex:{inputs:3|5 output:8}},,{pre:true post:result==x+y}

// Transaction entries:
txn,withdraw,amount:Int,Void,"{balance = balance - amount; term}",,{pre:amount>0&&balance>=amount post:balance'==balance-amount}

// Foreign function entries:
frgn,read_config_file,path:String,Result<Config,FileError>,libruntime
```

**Key formatting rules**:

1. **Body field**: Enclosed in `" "` double quotes when it contains commas
   or braces that should not be parsed as structure delimiters. The body is
   an opaque string — the backend parses it.
2. **Derivation field**: A `{ }` brace block containing space-separated
   example entries, each prefixed with `ex:` and containing `inputs:` and
   `output:` inline.
3. **Metadata/contracts**: Simple `{key:val key2:val2}` map format matching
   the existing `dbvl_reader::parse_map()` parser in `src/glue/dbvl_reader.rs`.
4. **Parameter lists**: Pipe-separated (`param:type|param2:type2`) to avoid
   ambiguity with the comma field separator.
5. **Inputs in examples**: Pipe-separated within the braces for the same
   reason: `inputs:2|2`.

**Existing infrastructure reused**: The `dbvl_reader.rs` parser already
supports all of this — no new parsing code needed. The archive consumer
splits by `,`, dispatches on `tag`, and uses `parse_map()` for structured
fields.

### Step 12.1 — Build `PackageWriter` and `PackageReader`

**New files**: `src/archive/mod.rs`, `src/archive/writer.rs`,
`src/archive/reader.rs`

**What**: The writer serializes the resolved `Program` into a `.dbvl` archive
file. The reader deserializes it for any backend.

**PackageWriter**:

```rust
/// Writes a fully resolved Program to a .dbvl archive file.
/// Emits tagged comma-separated lines matching dbvl_reader conventions.
/// 2026-07-11: Phase 12.1
pub struct ArchiveWriter {
    output_path: PathBuf,
}

impl ArchiveWriter {
    pub fn new(output_path: PathBuf) -> Self {
        Self { output_path }
    }

    /// Serialize the program to a .dbvl archive.
    pub fn write(&self, program: &Program) -> Result<(), ArchiveError> {
        let mut out = BufWriter::new(File::create(&self.output_path)?);

        // Optional: emit a comment header with version info
        writeln!(out, "// archive generated by briefc {}", env!("CARGO_PKG_VERSION"))?;

        // Write type entries: type,<name>,<slots_map>,<properties_map>
        for (name, resolved_type) in &program.type_universe.types {
            let slots = serialize_slots_map(resolved_type);
            let props = serialize_properties_map(resolved_type);
            writeln!(out, "type,{},{}", name, slots, props)?;
        }

        // Write definition entries:
        // defn,<name>,<params|pipe|sep>,<ret>,<body>,<derivation>,<metadata>,<contracts>
        for defn in &program.definitions {
            let params: Vec<String> = defn.parameters.iter()
                .map(|p| format!("{}:{}", p.name, p.ty))
                .collect();
            let sig_params = params.join("|");
            let body = serialize_body(&defn.body);
            let derivation = serialize_derivation(&defn.derivation);
            let meta = serialize_map(&defn.metadata);
            let contracts = serialize_contracts(&defn.contract);
            writeln!(
                out,
                "defn,{},{},{},{},{},{},{}",
                defn.name, sig_params, defn.output_type,
                body, derivation, meta, contracts
            )?;
        }

        // Write transaction entries (similar to defn)
        for txn in &program.transactions {
            let params: Vec<String> = txn.parameters.iter()
                .map(|p| format!("{}:{}", p.name, p.ty))
                .collect();
            let sig_params = params.join("|");
            let body = serialize_body(&txn.body);
            let contracts = serialize_contracts(&txn.contract);
            let derivation = serialize_derivation(&txn.derivation);
            writeln!(
                out,
                "txn,{},{},{},{},{},{}",
                txn.name, sig_params, "Void",
                body, contracts, derivation
            )?;
        }

        // Write frgn entries: frgn,<name>,<params|pipe|sep>,<ret>,<linking>
        for frgn in &program.frgns {
            let params: Vec<String> = frgn.params.iter()
                .map(|(n, t)| format!("{}:{}", n, t))
                .collect();
            let sig_params = params.join("|");
            writeln!(
                out,
                "frgn,{},{},{},{}",
                frgn.name, sig_params, frgn.return_type, frgn.linking
            )?;
        }

        Ok(())
    }
}
```

**PackageReader**:

```rust
/// Reads a .dbvl archive file into a consumable archive.
/// Uses the existing dbvl_reader to parse lines.
/// 2026-07-11: Phase 12.1
pub struct ArchiveReader {
    entries: Vec<ArchiveEntry>,
}

impl ArchiveReader {
    /// Parse a .dbvl archive file, reading line-by-line (streaming-friendly).
    pub fn read(path: &Path) -> Result<Self, ArchiveError> {
        let source = fs::read_to_string(path)
            .map_err(|e| ArchiveError::Io(path.to_path_buf(), e))?;

        // Reuse the existing dbvl_reader to split lines by commas
        // with proper { } brace and " " quote handling
        let dbvl = dbvl_reader::parse_dbvl(&source);
        let mut entries = Vec::new();

        for dbvl_entry in &dbvl.entries {
            let tokens = match dbvl_entry {
                DbvlEntry::Raw(tokens) => tokens,
                DbvlEntry::Validated { fields, .. } => fields,
            };
            if tokens.is_empty() {
                continue;
            }

            let entry = ArchiveEntry::from_tokens(tokens)?;
            entries.push(entry);
        }

        Ok(Self { entries })
    }

    /// Iterate over all entries of a specific type.
    pub fn entries_of_type(&self, entry_type: &str) -> Vec<&ArchiveEntry> {
        self.entries.iter()
            .filter(|e| e.tag() == entry_type)
            .collect()
    }

    /// Find a specific definition by name.
    pub fn find_definition(&self, name: &str) -> Option<&ArchiveEntry> {
        self.entries.iter().find(|e| e.matches_name(name))
    }
}

/// Parse a single comma-split token list into an ArchiveEntry.
/// Tag is tokens[0]: "type", "defn", "txn", "frgn".
/// 2026-07-11: Phase 12.1
impl ArchiveEntry {
    pub fn from_tokens(tokens: &[String]) -> Result<Self, ArchiveError> {
        let tag = tokens.first().ok_or(ArchiveError::EmptyLine)?;
        match tag.as_str() {
            "type" => {
                let name = tokens.get(1).ok_or(ArchiveError::MissingField("name"))?;
                Ok(ArchiveEntry::Type {
                    name: name.clone(),
                    slots_map: tokens.get(2).cloned().unwrap_or_default(),
                    properties_map: tokens.get(3).cloned().unwrap_or_default(),
                })
            }
            "defn" => {
                let name = tokens.get(1).ok_or(ArchiveError::MissingField("name"))?;
                Ok(ArchiveEntry::Defn {
                    name: name.clone(),
                    params: tokens.get(2).cloned().unwrap_or_default(),
                    ret: tokens.get(3).cloned().unwrap_or_default(),
                    body: tokens.get(4).cloned().unwrap_or_default(),
                    derivation: tokens.get(5).cloned().unwrap_or_default(),
                    metadata: tokens.get(6).cloned().unwrap_or_default(),
                    contracts: tokens.get(7).cloned().unwrap_or_default(),
                })
            }
            "txn" => { /* similar to defn */ }
            "frgn" => { /* frgn,<name>,<params>,<ret>,<linking> */ }
            _ => Err(ArchiveError::UnknownTag(tag.clone())),
        }
    }
}
```

**Nesting check**: Both Writer and Reader iterate with simple loops —
depth 1.

**Tests**:
- `test_archive_roundtrip`: Write program → read back → compare key fields
  (name, param count, return type)
- `test_archive_comments_skipped`: Lines starting with `//` are ignored
  (reuses dbvl_reader's existing behavior)
- `test_archive_tags_dispatched_correctly`: Each tag routes to the correct
  ArchiveEntry variant
- `test_archive_multiple_types`: Archive with types, defns, txns, frgns —
  all present after read
- `test_archive_quoted_body_field`: Body with `" "` quotes is parsed as
  single token despite containing commas
- `test_archive_missing_field`: Parse error on invalid entry

### Step 12.2 — Integrate archive emission into compile pipeline

**File**: `src/compile.rs` or `src/main.rs`

**What**: Add `--archive` flag to `brief compile` to produce the `.dbvl`
archive file instead of (or in addition to) final binary output.

```rust
/// CLI flag: --archive <path>
/// 2026-07-11: Phase 12.2
if let Some(archive_path) = matches.get_one::<String>("archive") {
    let writer = ArchiveWriter::new(PathBuf::from(archive_path));
    writer.write(&program)?;
}
```

**Tests**:
- `test_emit_archive_flag`: `brief compile main.bv --archive out.dbvl`
  → file exists, valid
- `test_emit_archive_roundtrip_compile`: Emit archive → read back → compile
  again → same binary

### Step 12.3 — Decoupled backend execution model

**What**: Backends become independent executables that receive a `.dbvl`
archive file path and produce output. The compiler frontend (Phase 12.2)
produces the archive; backends consume it. Because the archive IS a `.dbvl`
file, any language with a comma-split + `{ }` brace parser can consume it.

**CLI model**:

```bash
# 1. Frontend: Parse, run plugins, resolve derivations, emit archive
brief compile main.bv --archive build/main.dbvl

# 2. CPU backend (independent binary)
brief-llvm build/main.dbvl --output a.out

# 3. Hardware backend (independent binary)
brief-circt build/main.dbvl --output design.v

# 4. Documentation generator (independent script)
brief-doc build/main.dbvl --output docs/
```

**Backend interface** (backends read archive via `ArchiveReader` — can be
any language, not just Rust):

```rust
/// Trait for backends that consume .dbvl semantic archives.
/// Backends dispatch on the tag field of each entry:
///   "type"  → register type layout
///   "defn"  → compile function
///   "txn"   → compile transaction
///   "frgn"  → declare external symbol
/// 2026-07-11: Phase 12.3
pub trait ArchiveBackend {
    /// The name of this backend (e.g., "llvm", "circt").
    fn name(&self) -> &str;

    /// Process the archive and produce output.
    fn process(&self, archive: &ArchiveReader, output_path: &Path) -> Result<(), BackendError>;
}
```

**Migration path for existing backends**:

1. Add `--archive` to `brief compile` (Step 12.2)
2. Create `brief-llvm` wrapper binary that reads the archive and calls the
   existing LLVM codegen
3. Keep the old in-process path for backwards compatibility
4. After all users migrate, remove in-process linking — `brief compile`
   only produces the archive, backends handle codegen

**Dead backends**: `verilog.rs`, `vhdl.rs`, `c.rs`, `rust.rs`, `cobol.rs`,
`x86_64.rs`, `aarch64.rs`, `wasm.rs`, `tcl_generator.rs` — not migrated.
They continue to exist with their current (dead) status.

**Tests**:
- `test_backend_process_archive`: Create an archive, run `brief-llvm` on it,
  verify output binary
- `test_backend_unknown_entry_ignored`: Backend ignores entries it doesn't
  understand (forward compat)
- `test_backend_missing_type_error`: Backend reports if a required type is
  missing

---

## Phase 13 — CLI: `brief derive` Commands

**Depends on**: Phases 8, 9, 10, 11, 12

### Goal

Implement the `brief derive` subcommand with three modes (file, `--all`,
directory), all using the surgical write-back from Phase 9.6.

### Step 13.0 — Register `derive` subcommand in CLI

**File**: `src/main.rs`

**What**: Add the `derive` subcommand alongside existing commands (`check`,
`compile`, `build`, etc.).

```rust
// Phase 13.0: Add derive subcommand
. subcommand(
    Command::new("derive")
        .about("Synthesize function bodies from derivation blocks")
        .arg(Arg::new("input")
            .help("Source file or directory")
            .required(true))
        .arg(Arg::new("all")
            .long("all")
            .short('a')
            .help("Derive all transitive imports recursively"))
        .arg(Arg::new("depth")
            .long("depth")
            .short('d')
            .help("Maximum search depth for enumerative synthesis")
            .default_value("5"))
)
```

**Handler**:

```rust
// In main() dispatch:
Some(("derive", sub_m)) => {
    let input = sub_m.get_one::<String>("input").unwrap();
    let all = sub_m.get_flag("all");
    let depth: u8 = sub_m.get_one::<String>("depth")
        .unwrap()
        .parse()
        .unwrap_or(5);

    let path = Path::new(input);
    if path.is_dir() {
        derive::cli::derive_directory(path)?;
    } else if all {
        derive::cli::derive_all(path)?;
    } else {
        derive::cli::derive_file(path, depth)?;
    }
}
```

**Tests**:
- `test_cli_derive_file_exists`: Run `brief derive test.bv` → exit 0
- `test_cli_derive_all_flag`: Run `brief derive --all test.bv` → processes
  imports
- `test_cli_derive_directory`: Run `brief derive ./src` → processes all
  `.bv` files
- `test_cli_derive_nonexistent_file`: Run on missing file → exit 1 with
  error

### Step 13.1 — Single-file derive mode

**File**: `src/derive/cli.rs`

**What**: `brief derive <file>` — process a single file.

```rust
/// Derive all body-less definitions in a single file.
/// 2026-07-11: Phase 13.1
pub fn derive_file(path: &Path, depth: u8) -> Result<(), DeriveError> {
    // 1. Parse the file, recording byte offsets for each definition
    let (program, source_bytes) = parse_file_with_offsets(path)?;

    // 2. For each definition with derivation and no body, synthesize
    let mut modifications = Vec::new();
    for defn in &program.definitions {
        if !engine::should_derive(defn) {
            continue;
        }

        let synthesized = engine::synthesize_body(
            defn,
            &program.type_universe,
            plugin_host.as_ref(),  // Option<&PluginHost>
            depth,
        )?;

        let body_str = format_synthesized_body(&synthesized);
        modifications.push((defn.derivation.as_ref().unwrap(), body_str));
    }

    // 3. Apply all modifications (write-back)
    // Sort by descending offset to avoid offset invalidation
    modifications.sort_by(|a, b| b.0.span.end.cmp(&a.0.span.end));

    let mut source = source_bytes.clone();
    for (derivation, body_str) in &modifications {
        let insert_at = derivation.span.end as usize;
        let mut new_source = Vec::with_capacity(source.len() + body_str.len());
        new_source.extend_from_slice(&source[..insert_at]);
        new_source.extend_from_slice(body_str.as_bytes());
        new_source.extend_from_slice(&source[insert_at..]);
        source = new_source;
    }

    // 4. Verify: re-parse the modified program in memory and run compile-time tests.
    //    This must happen BEFORE writing to disk to prevent corrupting the file
    //    if verification fails.
    let temp_source = String::from_UTF8(source.clone())
        .map_err(|e| DeriveError::InvalidUTF8 {
            path: path.to_path_buf(),
            error: e,
        })?;
    let mut temp_parser = Parser::new(&temp_source);
    let reparsed = temp_parser.parse().map_err(|e| DeriveError::ParseFailed {
        path: path.to_path_buf(),
        error: e,
    })?;
    for defn in &reparsed.definitions {
        execute_derivation_tests(defn, &mut interpreter)?;
    }

    // 5. Write the modified source back (only after verification passes)
    fs::write(path, &source).map_err(|e| DeriveError::Io {
        path: path.to_path_buf(),
        error: e,
    })?;

    Ok(())
}
```

**Nesting check**: The function has sequential phases: parse → collect →
sort → verify (in memory) → write. Each phase is a loop with guard clauses —
depth 2 max. Verification before write ensures no file corruption on failure.

**Tests**:
- `test_derive_file_single_definition`: One definition with derivation →
  body filled
- `test_derive_file_multiple_definitions`: Multiple definitions → all filled
- `test_derive_file_skip_bodied`: Existing body → no modification
- `test_derive_file_skip_no_derive`: `#no_derive` → skipped
- `test_derive_file_verify_after`: Post-verify passes → file unchanged
- `test_derive_file_verify_fails`: Synthesized body fails verification →
  error, file unchanged

### Step 13.2 — Derive with `--all` (recursive imports)

**File**: `src/derive/cli.rs`

```rust
/// Derive all transitive imports in dependency order.
/// 2026-07-11: Phase 13.2
pub fn derive_all(source_path: &Path, depth: u8) -> Result<(), DeriveError> {
    // 1. Build import graph
    let import_order = build_import_dag(source_path)?;

    // 2. Process in topological order (leaf modules first)
    for file_path in &import_order {
        derive_file(file_path, depth)?;
    }

    Ok(())
}
```

**Topological sort**: Reuse the existing `ImportResolver` (from Phase 1A /
existing codebase).

**Tests**:
- `test_derive_all_dag_order`: A imports B, B has derivation → B processed
  before A
- `test_derive_all_circular`: Circular import → error
- `test_derive_all_no_imports`: Single file → same as derive_file

### Step 13.3 — Derive directory mode

**File**: `src/derive/cli.rs`

```rust
/// Derive all .bv files in a directory (non-recursive by default).
/// 2026-07-11: Phase 13.3
pub fn derive_directory(dir_path: &Path, depth: u8, recursive: bool) -> Result<(), DeriveError> {
    let walk_fn = if recursive {
        walkdir::WalkDir::new(dir_path)
    } else {
        walkdir::WalkDir::new(dir_path).max_depth(1)
    };

    let files: Vec<PathBuf> = walk_fn
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().map(|ext| ext == "bv").unwrap_or(false))
        .map(|e| e.path().to_path_buf())
        .collect();

    // Process files in parallel
    let results: Vec<Result<(), DeriveError>> = files.par_iter()
        .map(|path| derive_file(path, depth))
        .collect();

    // Report errors
    let errors: Vec<DeriveError> = results.iter()
        .filter_map(|r| r.as_ref().err().cloned())
        .collect();

    if errors.is_empty() {
        Ok(())
    } else {
        Err(DeriveError::Multiple(errors))
    }
}
```

**Tests**:
- `test_derive_directory_single_file`: Dir with one `.bv`
- `test_derive_directory_multi_file`: Multiple `.bv` files
- `test_derive_directory_nested`: Subdir with `.bv` file (recursive vs
  non-recursive)
- `test_derive_directory_non_bv`: `.txt`, `.rs` files → ignored

---

## Phase 14 — Derivation in the WASM Plugin Architecture

**Depends on**: Phase 7 (plugin system), Phase 9 (synthesis via plugin)

### Goal

Make the `brief derive` synthesis engine accessible as a WASM plugin, so
that the SMT solver (and eventually user-defined synthesis strategies) can
run as sandboxed plugins.

### Step 14.0 — Define the synthesis plugin WIT interface

**File**: `wit/synthesis.wit` (new)

```wit
// Synthesis plugin interface for Brief.
// 2026-07-11: Phase 14.0

/// A single derivation example.
record derivation-example {
    /// Input expressions as S-expressions.
    inputs: list<string>,
    /// Expected output as S-expression.
    output: string,
}

/// Result of a synthesis attempt.
variant synthesis-result {
    /// Synthesized body as a list of S-expressions.
    program(list<string>),
    /// No program exists (unsat).
    unsat,
    /// Solver could not decide.
    unknown(string),
}

/// The synthesis plugin.
resource synthesizer {
    /// Synthesize a function body from examples.
    /// Params: name, param-types, return-type, examples, depth-limit.
    synthesize: func(
        name: string,
        param-types: list<string>,
        return-type: string,
        examples: list<derivation-example>,
        depth-limit: u8,
    ) -> synthesis-result;

    /// Check if a program satisfies a set of examples.
    verify: func(
        body: list<string>,
        param-types: list<string>,
        return-type: string,
        examples: list<derivation-example>,
    ) -> bool;
}
```

**Tests**:
- `test_wit_interface_valid`: WIT file parses without error
- `test_wit_synthesize_call`: Call synthesizer plugin, verify result

### Step 14.1 — Add derive hooks to plugin lifecycle

**File**: `src/plugin/hooks.rs`

**What**: Add derivation hooks to the plugin lifecycle so synthesis
plugins are called during `brief derive`.

```rust
/// Plugin hooks for derivation.
/// 2026-07-11: Phase 14.1
pub enum DeriveHook {
    /// Called to synthesize a body from examples.
    Synthesize {
        defn: Definition,
        examples: Vec<DerivationExample>,
    },
    /// Called to verify a synthesized body against examples.
    Verify {
        body: Vec<Statement>,
        examples: Vec<DerivationExample>,
    },
}
```

**Tests**:
- `test_derive_hook_synthesize`: Plugin receives Synthesize hook, returns
  program
- `test_derive_hook_verify`: Plugin receives Verify hook, returns true/false

---

## Phase 15 and Beyond — Library Mode and FFI Export

After Phases 8–8G–14 are complete, the compiler has compile-time assertions,
program synthesis, sad-path derivation, intrinsic-free metadata dispatch,
and the `.dbvl` semantic archive. The next major efforts build the
**consumable C-callable library** path and the **frontend extension system**:

### Parallel: Phases 16A–16F — Extension Modifiers, Entry Points, Scripting

**Plan**: `docs/plans/2026-07-12-modifiers-entry-scripting.md`

Adds `.s`/`.f`/`.c` filename modifiers (`main.sf.bv` → strict + formatted),
`[#]` entry precondition with automatic CLI dispatch from function
signatures, `.f` layout pre-processor (indentation instead of braces),
`.c` cell wrapper (`server.c.bv` → `cell server { ... }`), `input`/`output`
keywords for cell files, top-level scripting with implicit `[#]` entry, and
the stdlib `cli.c.bv` extensible CLI framework.

All frontend-only — no backend, archive, or SMT changes.

### Parallel: Alloc Metadata System

**Plan**: `docs/plans/2026-07-12-alloc-metadata.md`

Adds `alloc` as a `<~` metadata annotation on variable bindings. Frontend
validates `"Stack"` (via escape analysis) and physical address literals
(`0x4000_2000`), expanding them to `alloca`, `volatile`, `observable`, and
`fixed_addr` metadata. Unknown values pass through to backends, which
validate what they can execute — known key + unparseable value is an error,
unknown keys are silently ignored.

Builds on the metadata dispatch infrastructure from Phase 8G and the
property system from Phase 1B.

### Next: Phase 15 — Library Mode Completion

**Plan**: `docs/plans/2026-07-11-library-mode-completion.md`

**Proposal by**: [@revred](https://github.com/revred) — reviewed the
existing `--library` infrastructure and identified five gaps between
Brief's `.ll`-level library mode and a linkable `.a`/`.so` with proper
headers and type marshaling.

Adds the `export` keyword (replacing `#export` pragma), `.ll` → `.o` →
`.a` packaging, `__brief_init_state`/`__glue_release` in generated
headers, `Bool`/`String` marshaling at the FFI boundary, and an
end-to-end C driver integration test.

**Prerequisites from this plan**:
- Phase 12 (`.dbvl` archive): the archive carries the resolved program
  that `--library` mode can consume as an alternative to direct AST input
- Phase 8 (derivation syntax): `export` parsing follows the same
  parser-extension pattern as `:=`
- Phase 8G (intrinsic/inop removal): backends now dispatch on metadata
  strings (`llvm_instr`, `interpreter_impl`) not hardcoded enums —
  `--library` mode's LLVM backend uses `llvm_instr` metadata
- Flat control flow, doc comments, and rationale comment conventions
  are carried forward

### After That: Zero-Copy Meld and Cross-Language LTO

Follows Phase 15. Extends zero-copy from scalars to composite types via
LLVM struct signatures at the FFI boundary, meld projections for non-LLVM
targets (Python PyObject*, Node v8::Value), and cross-language LLVM LTO.
Detailed in the existing plan at
`docs/plans/2026-07-10-zero-copy-glue-bridge-phases.md`.


| Phase | Focus | Test count delta |
|-------|-------|-----------------|
| 8 | Lexer + parser + AST for `:=` | ~+25 (lexing, parsing, type-check, compile-time assertions) |
| 8G | Remove Intrinsic/Inop, modularize | ~-60 (delete Intrinsic enum, InopDecl, IntrinsicCall; zero new code) |
| 9 | Synthesis engine + SMT bridge | ~+35 (enumerative search, SMT, `#no_derive`, write-back, DAG, directory) |
| 10 | Contract-guided synthesis | ~+15 (SyGuS, LLVM metadata emission) |
| 11 | Sad-path derivation | ~+15 (exhaustiveness, match synthesis, fallback verification) |
| 12 | `.dbvl` archive + decoupled backends | ~+25 (roundtrip, streaming, backend isolation, writer/reader) |
| 13 | CLI + derive commands | ~+15 (derive modes, write-back, rollback, parallel) |
| 14 | WASM plugin integration | ~+5 (WIT interface, plugin hooks) |

---

## Documentation Updates

| Doc | Phase | What |
|-----|-------|------|
| `docs/architecture/features/derivation.md` | 8 | Derivation block syntax, lifecycle, `:=` semantics, drafting vs resolved |
| `docs/architecture/features/intrinsic-removal.md` | 8G | Removal of `Intrinsic` enum, `inop`, `IntrinsicCall`, `#` suffix; metadata-dispatch architecture |
| `docs/architecture/features/synthesis.md` | 9 | Synthesis engine, DSL grammar, cost model, SMT bridge, enumerative fallback |
| `docs/architecture/features/sad-path.md` | 11 | FFI error recovery via derivation, exhaustiveness, contract verification |
| `docs/architecture/archive.md` | 8G, 12 | Archive schema, tagged `.dbvl` format, backend decoupling, ArchiveWriter/Reader; 8G adds metadata round-trip proof |
| `docs/architecture/features/contracts-synthesis.md` | 10 | Contract-guided synthesis, SyGuS, LLVM metadata emission |
| `docs/architecture/features/derive-cli.md` | 13 | `brief derive` CLI commands, modes, surgical write-back |
| `docs/architecture/features/derive-plugins.md` | 14 | WASM synthesis plugin WIT interface and hooks |
| `docs/plans/2026-07-11-derivation-synthesis-comprehensive.md` | All | This document |

---

## Risk Register

| Risk | Phase | Mitigation |
|------|-------|------------|
| SMT solver dependency unavailable | 9 | Fallback enumerative search (depth-bounded, works offline) |
| Synthesis produces wrong program from limited examples | 9 | Require ≥2 examples; user can always add more or write body manually; contracts add generalization guarantee |
| Source write-back corrupts formatting | 9, 13 | Byte-offset surgical insertion (not AST pretty-print); re-parse verification after write; rollback on failure |
| Sad-path derivation not exhaustive | 11 | Compile-time error listing uncovered variants |
| Contract synthesis too slow | 10 | Timeout + fallback to example-only synthesis or user writes body manually |
| Archive format drifts from AST | 12 | Roundtrip tests in CI; backwards-compat reader; single format (`.dbvl`) means no extension drift |
| Off-by-one errors in byte-offset insertion | 9, 13 | Test with files with BOM, mixed line endings, no trailing newline |
| Parallel directory derives conflict | 13 | File-level locking; or sequential fallback when lock unavailable |
| `Intrinsic` enum deletion misses a match arm | 8G | Delete-and-compile technique: remove the enum, let the compiler report every site. Fix each. |
| `inop` removal breaks existing stdlib | 8G | Rewrite all `lib/std/os/*.bv` in the same commit. CI tests verify before and after. |
| `Expr::IntrinsicCall` → `Expr::Call` breaks backend | 8G | Replace all match arms in lockstep. Every backend gets the same treatment. Gate with `cargo build` before `cargo test`. |
| `#` suffix removal confuses existing parsers for `#pragma` | 8G | Only remove the identifier-suffix-`#` path. `#` at token-start is unaffected. |

---

## Summary of New/Modified AST Fields

| AST node | New/Changed field | Source | Phase |
|----------|-------------------|--------|-------|
| `Definition` | `derivation: Option<DerivationBlock>` | `:=` block after body | 8.1 |
| `Transaction` | `derivation: Option<DerivationBlock>` | `:=` block after body | 8.1 |
| `DerivationBlock` | New struct | `:= { examples }` | 8.1 |
| `DerivationExample` | New struct | `inputs -> output` | 8.1 |
| `Token::ColonEq` | New token variant | `:=` | 8.0 |
| `Intrinsic` enum | **Deleted** | Replaced by `execute_intrinsic()` string dispatch | 8G |
| `InopDeclaration` | **Deleted** | Replaced by standard `defn` with metadata | 8G |
| `Expr::IntrinsicCall` | **Deleted** | Replaced by `Expr::Call` + metadata lookup | 8G |
| `TopLevel::Inop` | **Deleted** | No longer needed | 8G |
| `inop`/`inop!` tokens | **Deleted** | Removed from lexer | 8G |
| `#` intrinsic suffix | **Deleted** | Removed from identifier lexing | 8G |

---

## Flat Control Flow Check

Every function added or modified in this plan must be reviewed for nesting
depth. The standard pattern is:

```rust
// ACCEPTABLE (2 levels):
fn process(x: Option<Value>) -> Option<i64> {
    let val = x?;               // level 1: guard
    let result = val.as_i64()?; // level 1: guard
    if result <= 0 {            // level 1: guard
        return None;
    }
    Some(result)                // level 1: return
}

// ACCEPTABLE (2 levels):
fn process_opt(x: Option<Value>, y: Option<Value>) -> Option<i64> {
    let x = x?;
    let y = y?;
    helper(x, y)
}

fn helper(a: Value, b: Value) -> Option<i64> {
    let a = a.as_i64()?;
    let b = b.as_i64()?;
    Some(a + b)
}

// FORBIDDEN (3 levels):
fn process(x: Option<Value>) -> Option<i64> {
    if let Some(val) = x {           // level 1
        if let Some(result) = val.as_i64() { // level 2
            if result > 0 {          // level 3 ← FORBIDDEN
                return Some(result);
            }
        }
    }
    None
}
```

---

## Committing Strategy

1. Commit after EACH step within each phase
2. `cargo test --lib` before every commit
3. `cargo build` must produce no warnings
4. `bash benchmarks/build_and_bench.sh --correctness` before every phase
   boundary commit
5. Commit messages: `"YYYY-MM-DD: Phase N.M — <description>"`
6. Do not ask "shall I commit?" — the instructions from AGENTS.md say
   auto-commit

---

## Immediate Next Action

Phase 8, Step 8.0: Add `ColonEq` (`:=`) to the lexer token enum and
multi-character token matching in `src/lexer.rs`.
Phase 8, Step 8.1: Add `DerivationBlock` and `DerivationExample` structs
to `src/ast.rs`.
Phase 8, Step 8.2: Parse derivation blocks in `parse_definition()` in
`src/parser.rs`.
