# Verification Chain — Phase B: Parser + AST

Date: 2026-07-29
Status: Plan
Parent: `docs/plans/2026-07-29-verification-chain.md`

## Scope

Add `asm<target> name(args) -> T { body }` to the AST and parser, and
extend the `:=` chain parser to support multiple segments.

## 1. AST: `TopLevel::AsmFn` Variant

File: `src/ast/top.rs`

### 1.1 The Struct

```rust
/// 2026-07-29: Assembly function declaration.
/// asm<x86_64> name(params) -> ReturnType { "instruction"; "instruction"; };
/// Target-annotated inline assembly body. Cross-verified against other
/// bodies in the := chain at compile time (see verification-chain plan).
#[derive(Debug, Clone, PartialEq)]
pub struct AsmFn {
    pub target: String,                    // "x86_64", "aarch64", etc.
    pub name: String,
    pub params: Vec<(String, Type)>,
    pub ret_type: Type,
    pub body: Vec<String>,                 // instruction strings, pre-substitution
    pub span: Span,
}
```

### 1.2 TopLevel Enum Addition

Add to `pub enum TopLevel`:

```rust
pub enum TopLevel {
    // ... existing variants ...
    /// 2026-07-29: Inline assembly function (asm<target> ... { ... }).
    AsmFn(AsmFn),
}
```

### 1.3 Exports

Ensure `AsmFn` is re-exported from `src/ast/mod.rs` for use by the parser
and codegen modules.

## 2. Parser: `asm` Declaration

File: `src/parser/top_level.rs` (or equivalent file where `defn` is parsed)

### 2.1 Grammar

```
asm_fn := 'asm' '<' target_id '>' name '(' params ')' '->' type '{' asm_body '}' ';'
target_id ::= identifier    // e.g., x86_64, aarch64, riscv64
name      ::= identifier
params    ::= (param (',' param)*)?
param     ::= name ':' type
asm_body  ::= string_literal (';' string_literal)* ';'?
```

### 2.2 Parser Function

```rust
/// 2026-07-29: Parse asm<target> name(args) -> T { "instr"; }.
fn parse_asm_fn(&mut self) -> Result<TopLevel> {
    // expect 'asm'
    self.expect_keyword("asm")?;
    // expect '<'
    self.expect_operator("<")?;
    // expect target identifier
    let target = self.expect_identifier()?;
    // expect '>'
    self.expect_operator(">")?;
    // expect function name
    let name = self.expect_identifier()?;
    // expect '('
    self.expect_operator("(")?;
    // parse params
    let params = self.parse_fn_params()?;
    // expect ')'
    self.expect_operator(")")?;
    // expect '->'
    self.expect_operator("->")?;
    // parse return type
    let ret_type = self.parse_type()?;
    // expect '{'
    self.expect_operator("{")?;
    // parse asm body (string literals separated by semicolons)
    let body = self.parse_asm_body()?;
    // expect '}'
    self.expect_operator("}")?;
    // expect ';'
    self.expect_semicolon()?;
    Ok(TopLevel::AsmFn(AsmFn { target, name, params, ret_type, body, span: self.current_span() }))
}

/// Parse the body of an asm block: a sequence of string literals separated
/// by semicolons. Returns the strings before template substitution.
fn parse_asm_body(&mut self) -> Result<Vec<String>> {
    let mut strings = Vec::new();
    loop {
        // Check for closing brace
        if self.peek() == Some(Token::CloseBrace) {
            break;
        }
        // Expect a string literal
        let s = self.expect_string_literal()?;
        strings.push(s);
        // Optional semicolon after each string
        if self.peek() == Some(Token::Semicolon) {
            self.advance();
        }
    }
    Ok(strings)
}
```

### 2.3 Integration into Top-Level Parse

In the main top-level parse loop (the function that dispatches to
`parse_defn`, `parse_struct`, etc.), add an `asm` case:

```rust
fn parse_top_level(&mut self) -> Result<TopLevel> {
    match self.peek() {
        Some(Token::Ident(ref name)) if name == "asm" => self.parse_asm_fn(),
        Some(Token::Ident(ref name)) if name == "defn" => self.parse_defn(),
        // ... existing cases ...
    }
}
```

This must come BEFORE the generic identifier branch so `asm` is recognized
as a keyword-like construct (not a general identifier).

## 3. Parser: Multi-Segment `:=` Chain

File: `src/parser/derivation.rs` (or wherever `DerivationBlock` is parsed)

### 3.1 Current Derivation Block Structure

The existing `DerivationBlock` has:
- `examples: Vec<DerivationExample>` (from `{ input -> output; ... }`)
- `postcondition: Option<Expr>` (from `[[ expr ]]`)
- `precondition: Option<Expr>` (from `[ expr ]]` or `[[ expr ]`)
- `ref_name: Option<String>` (from `:= name`)

### 3.2 Extension

Add a chain field:

```rust
/// 2026-07-29: A segment in the := verification chain.
/// Each segment is one body: an asm function name, a derivation block,
/// a reference function name, or inline examples.
#[derive(Debug, Clone)]
pub enum ChainSegment {
    /// Reference to an asm function or defn: := name
    Ref(String),
    /// Derivation block with examples and optional reference: := { ex } or := { ex } := ref
    Derivation(DerivationBlock),
    /// Inline examples only (no reference): := { ex }
    Examples(Vec<DerivationExample>),
}
```

The `DerivationBlock` struct gets a new field:

```rust
pub struct DerivationBlock {
    pub examples: Vec<DerivationExample>,
    pub postcondition: Option<Expr>,
    pub precondition: Option<Expr>,
    pub ref_name: Option<String>,
    /// 2026-07-29: Multi-segment verification chain.
    /// If non-empty, the body is selected by cross-verification at compile time.
    pub chain: Vec<ChainSegment>,
}
```

### 3.3 Parser Extension

After parsing the first `:= body`, continue parsing additional `:= body`
segments:

```rust
/// 2026-07-29: Parse a derivation segment: { examples } optionally := ref.
/// This is extracted from the chain parser for flat control flow.
fn parse_derivation_segment(&mut self) -> Result<ChainSegment> {
    let examples = self.parse_examples()?;
    let ref_name = self.peek().filter(|t| *t == Token::ColonEquals).and_then(|_| {
        self.advance();
        self.expect_identifier().ok()
    });
    Ok(ChainSegment::Derivation(DerivationBlock {
        examples, postcondition: None, precondition: None,
        ref_name, chain: vec![],
    }))
}

fn parse_derivation_block(&mut self) -> Result<Option<DerivationBlock>> {
    let mut chain: Vec<ChainSegment> = Vec::new();

    while self.peek() == Some(Token::ColonEquals) {
        self.advance();
        let segment = match self.peek() {
            Some(Token::OpenBrace) => self.parse_derivation_segment()?,
            Some(Token::Ident(_)) => ChainSegment::Ref(self.expect_identifier()?),
            _ => break,
        };
        chain.push(segment);
    }

    let (postcondition, precondition) = self.parse_contracts()?;
    Ok(Some(DerivationBlock {
        examples: vec![], postcondition, precondition,
        ref_name: None, chain,
    }))
}
```

### 3.4 Compatibility

Existing single-segment forms produce a one-element chain:

| Old syntax | Chain produced |
|------------|---------------|
| `:= { ex }` | `chain = [Derivation(examples)]` |
| `:= ref_fn` | `chain = [Ref("ref_fn")]` |
| `:= { ex } := ref_fn` | `chain = [Derivation(examples + ref)]` |

New multi-segment syntax:

| Syntax | Chain produced |
|--------|---------------|
| `:= a := b := c` | `chain = [Ref("a"), Ref("b"), Ref("c")]` |
| `:= a := { ex } := c` | `chain = [Ref("a"), Derivation(ex), Ref("c")]` |

## 4. Typechecker Changes

File: `src/typechecker/mod.rs` or `src/typechecker/definitions.rs`

### 4.1 AsmFn Validation

```rust
/// 2026-07-29: Typecheck an AsmFn declaration.
/// Validates: params have valid types, return type is valid,
/// target string matches a known architecture.
fn check_asm_fn(&mut self, asm_fn: &AsmFn) -> Result<()> {
    // Check target is non-empty
    if asm_fn.target.is_empty() {
        return Err(TypeError::new("asm target cannot be empty", asm_fn.span));
    }
    // Check params
    for (name, ty) in &asm_fn.params {
        self.check_type(ty)?;
    }
    // Check return type
    self.check_type(&asm_fn.ret_type)?;
    Ok(())
}
```

## 5. Tests

### 5.1 Parser Tests

File: `src/parser/tests.rs`

| Test | What it verifies |
|------|-----------------|
| `parse_asm_fn_simple` | `asm<x86_64> foo(x: Int) -> Int { "nop" };` produces correct AST |
| `parse_asm_fn_multi_instr` | Multi-instruction body with semicolons |
| `parse_asm_fn_params` | Multiple params, complex types |
| `parse_asm_fn_empty_body` | Empty body `{ }` |
| `parse_verification_chain_multi` | `:= a := b := c` produces 3-segment chain |
| `parse_verification_chain_mixed` | `:= a := { ex } := c` produces mixed chain |
| `parse_verification_chain_single` | `:= ref_fn` produces single-segment (backward compat) |
| `parse_verification_chain_derivation` | `:= { ex } := ref_fn` produces derivation segment (backward compat) |

### 5.2 AST Tests

File: `src/ast/tests.rs`

| Test | What it verifies |
|------|-----------------|
| `asm_fn_debug_display` | AsmFn implements Debug |
| `asm_fn_partial_eq` | AsmFn implements PartialEq |

## 6. Files Changed

| File | Change |
|------|--------|
| `src/ast/top.rs` | Add `AsmFn` struct + `TopLevel::AsmFn` variant |
| `src/ast/mod.rs` | Re-export `AsmFn` |
| `src/parser/top_level.rs` | Add `parse_asm_fn()`, integrate into top-level dispatch |
| `src/parser/derivation.rs` | Add `ChainSegment` enum, extend `DerivationBlock`, multi-segment parsing |
| `src/typechecker/definitions.rs` | Add `check_asm_fn()` validation |

## 7. Implementation Order

1. `AsmFn` struct + `TopLevel` variant
2. `parse_asm_fn()` in parser
3. Integration into top-level parse loop
4. `ChainSegment` enum + `DerivationBlock.chain` field
5. Multi-segment `:=` parser
6. Typechecker validation
7. Tests: parser round-trip, chain parsing, backward compat
8. `cargo test --lib` — all pass
