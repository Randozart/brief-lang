# Extensible Types — Comprehensive Implementation Plan

**See also**: `docs/plans/2026-07-11-extensible-types-vision.md` for the
design thesis, end-state picture, and philosophical rationale. This plan is
the executable counterpart — step-by-step implementation with exact file
paths, code snippets, and test lists.

## Overview

This plan covers the full transformation from Brief's current type system
(hardcoded `ResolvedType` fields, string-name dispatch in codegen, no codec
system, no plugin architecture) to the complete vision: a self-describing,
user-extensible type system where every property is metadata, every backend
is a property consumer, and the compiler is a platform for plugins written
in Brief itself.

The plan is organized into 9 sequential phases (Phase 0, 1A, 1B, 2–7). Each
phase is independently testable and commit-able. No phase may break the
previous phase's tests.

**Key architectural decision**: Metadata travels with the item, not above
it. The `<~` (Annotation Arrow) is the single universal metadata attachment
mechanism — valid inside type bodies, definition bodies, transaction bodies,
and guard branches. The `~>` prefix annotation syntax `(name: val) ~> item`
is **removed entirely** in Phase 1A.

## Golden Rules (All Phases)

1. **Flat control flow**: All new/modified code must nest at most 2 levels
   deep. Arrowhead code is forbidden. Use `?`, guard clauses, early returns,
   and extracted helper functions.

2. **Contract-first**: Never weaken contract guarantees. If a type had
   `[result > 0]` before the refactor, it must still have it after.

3. **Additive only**: Existing optimization paths must NOT be modified. New
   match arms only. The `_ => return None;` fallthrough must remain
   unchanged.

4. **Tests or it doesn't exist**: Every new code path, every match arm,
   every feature must have corresponding tests. `cargo test --lib` before
   every commit.

5. **Doc comments on every definition**: Every `fn`, `struct`, `enum`,
   `trait` added or modified must have a `///` doc comment.

6. **Rationale comments at every change site**: Format:
   `// YYYY-MM-DD: <phase> — <what and why>`

7. **HashMap iteration determinism**: All HashMap iterations that produce
   IR instructions must be sorted by key before the loop.

8. **Do not ask "shall I commit?" — just commit**: After every logical step
   where tests pass, commit. No amend, no squash. One commit per step.

---

## Phase 0 — Lexer Cleanup & Type Body Syntax Simplification

### Goal

Remove the `~>` token from the lexer, delete dead parser tests, and
simplify type body syntax: `=` is removed as a property-binding delimiter;
`<~` is the sole syntax for metadata in type bodies. Projections with
parameters still use `=`.

### Step 0.0 — Remove `TildeArrowRight` (`~>`) from the lexer

**What**: The `~>` token (`Token::TildeArrowRight`) is removed entirely.
This token was used only by the prefix annotation syntax `(name: val) ~> item`,
which is being removed in Phase 1A.

**Files**: `src/lexer.rs`

**Changes**:
1. Remove `TildeArrowRight` from the `Token` enum (line 253)
2. Remove its `Display` impl: `write!(f, "~>")` (line 592)
3. Remove the lexer's character-sequence match for `~>`
4. Remove any test that checks for `TildeArrowRight` lexing

**Search before deleting** — verify no other code references `TildeArrowRight`:
```bash
grep -rn 'TildeArrowRight' src/
```

Expected results:
- `src/lexer.rs`: enum variant, Display impl, lexer match — remove all
- `src/parser.rs`: verify the `self.expect(Token::TildeArrowRight)` call
  at line 641 is ONLY inside `parse_prefix_annotation()`, which is being
  removed in Phase 1A — safe.

**Nesting check**: Token removal — no nesting concern.

**Tests**: `cargo test --lib` must still pass. The `TildeArrowRight` tests
in the lexer must be removed or updated.

### Step 0.1 — Delete dead parser tests

**What**: Remove `#[test]` functions at the `parser` module level (lines
12247–12305) that are invisible to the test harness (they're outside the
`parser_tests` module).

**Files**: `src/parser.rs`

**Functions to remove**:
- `test_parse_async_expr`
- `test_parse_async_await_expr`
- `test_parse_async_await_let`
- `test_parse_typedef_underscore_bitrange`
- `test_parse_expr_bitrange_after_identifier`
- `test_parse_typedef_bits_bitrange_again` (duplicate of the one inside `parser_tests`)
- `test_parse_typedef_slot_syntax_proper` (duplicate)
- Any other `#[test]` at `parser` module level after line 11759

**How to identify**: At the end of `parser.rs`, search for `#[test]` after
the `parser_tests` module closing brace (line 11759).

```bash
# Find all #[test] functions outside parser_tests module
awk '/^mod parser_tests/,/^}' src/parser.rs | tail -1
grep -n '#\[test\]' src/parser.rs | tail -20
```

**Nesting check**: Straight deletions — no nesting concern.

**Tests**: `cargo test --lib` must report approximately 1464 tests (from
1470, removing ~6 dead tests).

### Step 0.2 — Remove `=` as metadata delimiter in type bodies

**What**: In `parse_type_def()`, the `=` sign is no longer accepted as a
binding delimiter for metadata. `name = expr;` without parentheses emits a
parse error: `"use '<~' for metadata in type bodies"`. The `=` is reserved
for parameterized projections only: `name(params) = expr;`.

**Files**: `src/parser.rs` (function `parse_type_def`, line 3596)

**Before** (current logic at line 3594–3609):
```rust
// Accept `=` or `<~` (Annotation Arrow) as the binding separator
if matches!(self.current_token(), Some(Ok(Token::TildeArrow))) {
    self.advance(); // consume <~
} else {
    self.expect(Token::Eq)?;
}
let value = self.parse_expression()?;
// Create binding — always goes to TypeBinding
bindings.push(TypeBinding { name: item_name, params, value: Box::new(value), ... });
```

**After**:
```rust
if matches!(self.current_token(), Some(Ok(Token::TildeArrow))) {
    // <~ binding — compile-time metadata property
    self.advance();
    let value = self.parse_expression()?;
    // ... consume semicolon ...
    if params.is_empty() {
        // Constant metadata: name <~ value;
        metadata.insert(item_name, property_value_from_expr(&value)?);
    } else {
        // Parameterized metadata: name(a, b) <~ value;
        projections.push(TypeBinding {
            name: item_name, params,
            value: Box::new(value), ...
        });
        // Also store as metadata with param-qualified key
        metadata.insert(format!("{}({})", item_name, params.join(",")),
            property_value_from_expr(&value)?);
    }
} else if matches!(self.current_token(), Some(Ok(Token::Eq))) {
    // = binding — ONLY valid with params (projection)
    if params.is_empty() {
        return self.spanned_err("use '<~' for metadata in type bodies");
    }
    self.advance();
    let value = self.parse_expression()?;
    // ... consume semicolon ...
    projections.push(TypeBinding {
        name: item_name, params,
        value: Box::new(value), ...
    });
} else {
    return self.spanned_err("expected '<~' or '=' in type body binding");
}
```

**Helper function**:
```rust
/// Convert a compile-time-constant expression to a PropertyValue.
/// Returns an error if the expression is not a compile-time constant.
/// 2026-07-11: Phase 0.2.
fn property_value_from_expr(expr: &Expr) -> Result<PropertyValue, SyntaxError> {
    match expr {
        Expr::Int(n, _) => Ok(PropertyValue::Int(*n)),
        Expr::Float(f, _) => Ok(PropertyValue::Float(*f)),
        Expr::String(s) => Ok(PropertyValue::String(s.clone())),
        Expr::Bool(b) => Ok(PropertyValue::Bool(*b)),
        Expr::Identifier(name) => Ok(PropertyValue::Identifier(name.clone())),
        Expr::List(list) => {
            let mut vals = Vec::new();
            for elem in list {
                vals.push(property_value_from_expr(elem)?);
            }
            Ok(PropertyValue::List(vals))
        }
        _ => Err(SyntaxError::new("metadata value must be a compile-time constant "
            "(literal, identifier, or list of literals)")),
    }
}
```

**Nesting check**: The new logic uses guard clauses (first check `<~`, then
check `=`, then error) — depth 2 max. The helper function is flat.

**Tests**:
- `test_type_body_tilde_arrow_metadata`: `type F { bytes <~ 8; };` → metadata map has `"bytes" → Int(8)`
- `test_type_body_eq_rejected`: `type F { bytes = 8; };` → parse error
- `test_type_body_eq_with_params_valid`: `type F { ptr: Ptr<UInt8>; At(i) = ptr[i]; };` → projection
- `test_type_body_tilde_with_params`: `type F { ptr: Ptr<UInt8>; At(i) <~ ptr[i]; };` → parameterized metadata
- `test_type_body_slot_still_works`: `type F { x: Int; };` → slot, unchanged
- `test_type_body_op_still_works`: `type F { op Add(F) -> F = __add#; };` → operator, unchanged
- `test_type_body_constraint_still_works`: `type F { [x > 0]; };` → constraint, unchanged
- `test_hashtag_pragma_still_works`: `#native type F { };` → pragma, unchanged

---

## Phase 1A — Metadata Everywhere (Syntax + Parser + AST)

### Goal

Unify all annotation and metadata syntax into inline `<~` statements inside
body blocks. Remove the `~>` prefix annotation syntax and the `<~(...)`
structured annotation form from `parse_hashtag_modifiers()`. Add metadata
storage to `Definition`, `Transaction`, and `GuardBranch`. Add `#?` as a
diagnostic qualifier on annotations.

### Step 1A.0 — Remove prefix annotation infrastructure

**What**: Three pieces of code are removed:
1. `parse_prefix_annotation()` method — no longer needed
2. The `~>` prefix annotation handler in `parse_statement()` (lines 6393–6417)
3. The `<~ (name: expr, ...)` handler inside `parse_hashtag_modifiers()` (lines 573–592)

**Files**: `src/parser.rs`

**Changes**:

**1A.0a — Remove `parse_prefix_annotation()`** (lines 599–642):

Remove the entire method. Its caller at lines 6393–6417 also goes:

```rust
// BEFORE (lines 6393-6417):
// 2026-07-07: Phase 1 — check for prefix annotations before expression
if let Some(prefix_mods) = self.parse_prefix_annotation()? {
    if !prefix_mods.is_empty() {
        // Prefix annotations before a guarded statement
        if let Some(Ok(Token::LBracket)) = self.current_token() {
            self.advance();
            let condition = self.parse_expression()?;
            self.expect(Token::RBracket)?;
            let statements = if let Some(Ok(Token::LBrace)) = self.current_token() {
                // ... parse body ...
            };
            return Ok(Statement::Guarded { condition, statements });
        } else {
            return self.spanned_err("Expected '[condition]' after prefix annotation".to_string());
        }
    }
}
```

**AFTER**: Remove the entire block. Fall through to normal expression
parsing.

If a user writes `(name: val) ~> [cond]`, the parser will now hit the `(`
as the start of a parenthesized expression, then `name` as an identifier,
then `:` which is not valid after an identifier in expression context → a
normal parse error. This is fine — the old syntax is simply gone.

**1A.0b — Remove `<~ (...)` from `parse_hashtag_modifiers()`** (lines 573–592):

Remove this arm from the match in `parse_hashtag_modifiers()`:

```rust
// REMOVE THIS ENTIRE ARM:
Some(Ok(Token::TildeArrow)) => {
    self.advance();
    self.expect(Token::LParen)?;
    loop {
        let name = self.expect_identifier()?;
        self.expect(Token::Colon)?;
        let value = self.parse_expression()?;
        mods.push(Annotation { name, value, mode: AnnotationMode::Advisory });
        if matches!(self.current_token(), Some(Ok(Token::Comma))) {
            self.advance();
        } else {
            break;
        }
    }
    self.expect(Token::RParen)?;
}
```

The `TildeArrow` token is still valid in type bodies (Phase 0.2) and in
defn/txn/guard bodies (Phase 1A.1 below). It's only removed from
`parse_hashtag_modifiers()`.

**After removal**, `parse_hashtag_modifiers()` only handles:
- `#name(val)` → `AnnotationMode::Advisory`
- `#!name(val)` → `AnnotationMode::Mandatory`
- `#[scope]` → scoped grouping
- Bare `<~` in this context → falls through to `_ => return Ok(mods)`

**1A.0c — Also remove `SigModifier::Annotation` and `SigModifier::PrefixAnnotation`**:

If `SigModifier` has an `Annotation` variant for storing the old `<~ (...)`
annotations, remove it. (Check `src/ast.rs:2363` first to see if this
variant exists. The current `SigModifier` at that line only has `Out`,
`Inline`, `Export(Option<String>)` — no annotation variant. If no such
variant exists, skip this sub-step.)

**Nesting check**: Code removal — no nesting concern.

**Tests**:
- `test_prefix_annotation_rejected`: `(name: 2) ~> [cond] { term 0; };` → parse error (not prefix annotation error, just regular expression parse error)
- `test_hashtag_annotations_still_work`: All existing `#tag` and `#!tag` tests still pass
- All existing tests must still pass

### Step 1A.1 — Add `PropertyValue` enum and metadata to Definition/Transaction/GuardBranch

**What**: A new `PropertyValue` enum holds all possible metadata values.
`Definition`, `Transaction`, and `GuardBranch` get a `metadata` HashMap
field. The parser recognizes `<~ expr;` inside these body blocks.

**File**: `src/ast.rs`

**1A.1a — Define `PropertyValue`** (shared with Phase 1B):

```rust
/// A compile-time-constant metadata value.
/// 2026-07-11: Phase 1A — shared across all item metadata.
#[derive(Debug, Clone, PartialEq)]
pub enum PropertyValue {
    /// Integer literal: `24`, `-1`
    Int(i64),
    /// Float literal: `3.14`
    Float(f64),
    /// String literal: `"%String"`
    String(String),
    /// Boolean literal: `true`, `false`
    Bool(bool),
    /// Bare identifier (symbol): `Native`, `C`, `LittleEndian`
    Identifier(String),
    /// List of property values: `[1, 2, 3]`
    List(Vec<PropertyValue>),
}
```

**1A.1b — Add metadata fields**:

```rust
pub struct Definition {
    // ... existing fields ...
    /// Inline metadata declared via `<~ expr;` inside the body.
    /// 2026-07-11: Phase 1A.
    pub metadata: HashMap<String, PropertyValue>,
}

pub struct Transaction {
    // ... existing fields ...
    /// Inline metadata declared via `<~ expr;` inside the body.
    /// 2026-07-11: Phase 1A.
    pub metadata: HashMap<String, PropertyValue>,
}

pub struct GuardBranch {
    pub condition: Expr,
    pub statements: Vec<Statement>,
    /// Metadata scoped to this specific guard branch.
    /// 2026-07-11: Phase 1A.
    pub metadata: HashMap<String, PropertyValue>,
}
```

**1A.1c — Parser for `<~` in body blocks**:

Inside `parse_definition_body()`, `parse_txn_body()`, and
`parse_guard_body_inner()`: before parsing regular statements, check for
the pattern:

```rust
// At the top of a body block, after {:
loop {
    // Check for metadata statement: ident <~ expr ;
    if let Some(Ok(Token::Identifier(name))) = self.current_token() {
        // Peek ahead: is the next token <~ ?
        let saved = self.pos;
        self.advance();
        if matches!(self.current_token(), Some(Ok(Token::TildeArrow))) {
            self.advance();
            let value = self.parse_expression()?;
            self.expect(Token::Semicolon)?;
            let prop_value = property_value_from_expr(&value)?;
            metadata.insert(name.clone(), prop_value);
            // Store the rest of the annotation name if this is a
            // chained annotation (see step 1A.1d)
            continue;
        }
        self.pos = saved;  // backtrack — not metadata, parse as statement
        break;
    }
    break;
}
// Now parse normal statements...
```

Note: This backtracking is only necessary if identifiers can start both
metadata and regular statements. An alternative is to check for `<~`
BEFORE committing — peek at the second token. The backtrack approach
is simpler but slightly less efficient.

**Optimized version** (peek without consuming):

```rust
loop {
    match self.current_token() {
        Some(Ok(Token::Identifier(name))) => {
            // Peek: check if followed by <~ (metadata) or something else (statement)
            if self.peek_token(1).map_or(false, |t| matches!(t, Ok(Token::TildeArrow))) {
                // Metadata statement
                let name = name.clone();
                self.advance(); // consume ident
                self.advance(); // consume <~
                let value = self.parse_expression()?;
                self.expect(Token::Semicolon)?;
                let prop_value = property_value_from_expr(&value)?;
                metadata.insert(name, prop_value);
                continue;
            }
        }
        _ => {}
    }
    break;
}
```

This requires a `peek_token(n: usize) -> Option<&Result<Token, ...>>`
method on the parser. If it doesn't exist, add it:

```rust
/// Peek at the nth token ahead without consuming.
/// 2026-07-11: Phase 1A — needed for metadata vs statement disambiguation.
fn peek_token(&self, n: usize) -> Option<&Result<Token, SyntaxError>> {
    self.tokens.get(self.pos + n)
}
```

**1A.1d — Chained annotations**: If a user wants multiple annotations,
they can just write multiple `<~` statements:

```brief
defn foo() -> Int {
    jira <~ "FIN-8422";
    priority <~ 2;
    reviewed <~ true;
    term 42;
};
```

This is equivalent to metadata map: `{"jira": String("FIN-8422"),
"priority": Int(2), "reviewed": Bool(true)}`.

**Nesting check**: The peek-then-consume pattern is depth 2 (match inside
loop). The helper `property_value_from_expr` is a flat match.

**Tests**:
- `test_metadata_in_defn_body`: `defn foo() -> Int { priority <~ 2; term 0; };` → `defn.metadata["priority"] == Int(2)`
- `test_metadata_in_txn_body`: `txn foo [i < N][i == N] { origin <~ "calc"; term i; };` → `txn.metadata["origin"] == String("calc")`
- `test_metadata_in_guard`: `defn foo() -> Int { [true] { priority <~ 1; term 0; }; };` → guard branch has metadata
- `test_metadata_non_constant_rejected`: `defn foo(x: Int) -> Int { val <~ x; term 0; };` → parse error "metadata value must be a compile-time constant"
- `test_metadata_list_value`: `targets <~ ["a", "b", "c"];` → `metadata["targets"] == List(...)`
- `test_metadata_identifier_value`: `storage <~ Native;` → `metadata["storage"] == Identifier("Native")`
- `test_multiple_metadata`: `jira <~ "x"; priority <~ 2;` → both present
- `test_metadata_does_not_conflict_with_let`: `let x <~ 5;` → still a valid let binding (note: `let` keyword makes it unambiguous — `let x` is not `identifier <~`)

### Step 1A.1b — `#?` as diagnostic qualifier on annotations

**What**: `#?` before a tag means "advisory with diagnostic output." A new
`diagnostic: bool` field on `Annotation` signals that the compiler should
emit a compile-time explanation of its decision for that pass.

**Files**: `src/ast.rs`, `src/parser.rs`

**1A.1b-1 — Add `diagnostic` field to `Annotation`**:

```rust
#[derive(Debug, Clone)]
pub struct Annotation {
    pub name: String,
    pub value: Expr,
    pub mode: AnnotationMode,
    /// When true, the compiler emits verbose diagnostic output explaining
    /// WHY it chose or rejected the annotated decision.
    /// 2026-07-11: Phase 1A — set by `#?` prefix.
    pub diagnostic: bool,
}
```

**1A.1b-2 — Parser logic for `#?`**:

In `parse_hashtag_modifiers()`, the `Token::HashQuestion` arm (line 425)
is updated:

```rust
Some(Ok(Token::HashQuestion)) => {
    self.advance();
    let diagnostic = true;  // #? always sets diagnostic
    // Check if followed by ! (mandatory diagnostic)
    let mode = if matches!(self.current_token(), Some(Ok(Token::Bang))) {
        self.advance();
        AnnotationMode::Mandatory
    } else {
        AnnotationMode::Advisory
    };
    // Check if followed by an identifier (scoped diagnostic)
    if let Some(Ok(Token::Identifier(n))) = self.current_token() {
        let name = n.clone();
        self.advance();
        let value = parse_annotation_value(self); // reuses existing value parser
        mods.push(Annotation {
            name,
            value: value_expr,
            mode,
            diagnostic: true,  // NEW
        });
    } else {
        // Bare #? — diagnostic all passes for this item
        mods.push(Annotation {
            name: "?diagnostic-all".to_string(),
            value: Expr::Bool(true),
            mode,
            diagnostic: true,
        });
    }
}
```

Also update the `Annotation { ... }` construction at ALL existing `#tag`
and `#!tag` sites to include `diagnostic: false`:

```rust
// Existing #tag (line 505) → add diagnostic: false:
mods.push(Annotation {
    name,
    value: value_expr,
    mode: AnnotationMode::Advisory,
    diagnostic: false,  // NEW
});

// Existing #!tag (line 555) → add diagnostic: false:
mods.push(Annotation {
    name,
    value: value_expr,
    mode: AnnotationMode::Mandatory,
    diagnostic: false,  // NEW
});
```

**1A.1b-3 — Compiler diagnostic output**:

When a definition/transaction has annotations with `diagnostic: true`, the
backend checks these during codegen and emits explanations:

| Annotation | Pass/domain diagnosed | Example output |
|------------|----------------------|----------------|
| `#?gpu` | GPU offloading | `[foo] gpu: deferred to GPU (kernel size > threshold of 64)` |
| `#?inline` | Inlining decision | `[foo] inline: NOT inlined (function body > 100 instructions)` |
| `#?vectorize` | Loop vectorization | `[foo] vectorize: NOT vectorized (trip count < 4)` |
| bare `#?` | ALL passes | All of the above for the annotated item |

Implementation sketch (in the backend):

```rust
/// Check if a pass-level diagnostic is enabled for a given item.
/// 2026-07-11: Phase 1A.
fn diagnostic_enabled(annotations: &[Annotation], pass_name: &str) -> bool {
    for ann in annotations {
        if ann.diagnostic {
            if ann.name == "?diagnostic-all" || ann.name == pass_name {
                return true;
            }
        }
    }
    false
}

/// Emit a diagnostic message to stderr.
/// 2026-07-11: Phase 1A.
fn emit_diagnostic(item_name: &str, pass: &str, message: &str) {
    eprintln!("[{}] {}: {}", item_name, pass, message);
}
```

Usage in codegen passes:

```rust
// Inside loop vectorization pass
let diag = diagnostic_enabled(item.annotations, "vectorize");
if trip_count < 4 {
    if diag {
        emit_diagnostic(item.name, "vectorize",
            "NOT vectorized (trip count < 4)");
    }
    // don't vectorize
} else {
    if diag {
        emit_diagnostic(item.name, "vectorize",
            "vectorized (trip count >= 4)");
    }
    // vectorize
}
```

**Diagnostics are purely additive** — they produce zero IR changes.
Tests verify IR equivalence with and without `#?`.

**Tests**:
- `test_hash_question_diagnostic_field`: `#?gpu` → `Annotation { diagnostic: true, mode: Advisory, name: "gpu" }`
- `test_hash_question_bang_diagnostic`: `#?!gpu` → `Annotation { diagnostic: true, mode: Mandatory, name: "gpu" }`
- `test_bare_hash_question`: `#?` → `Annotation { name: "?diagnostic-all", diagnostic: true }`
- `test_hash_annotations_have_diagnostic_false`: `#gpu` → `Annotation { diagnostic: false }`
- `test_hash_bang_annotations_have_diagnostic_false`: `#!gpu` → `Annotation { diagnostic: false }`
- `test_diagnostic_no_ir_change`: defn with `#?gpu` produces same IR as defn with `#gpu`
- All existing annotation tests still pass (with `diagnostic: false` added)

### Design Note: `!` and `?` stay in annotation land — `<~` metadata stays clean

`!` (mandatory) and `?` (diagnostic) are qualifiers on **compiler directives** —
they modify what the compiler does with an annotation ("obey it" / "explain your
reasoning"). Metadata via `<~` is **declarative data**, not a directive.

Examples where the distinction matters:
```brief
#!gpu                    // "MUST defer to GPU" — compiler directive
#?vectorize              // "Explain vectorization decision" — compiler diagnostic

jira <~ "FIN-8422";     // "This item references JIRA ticket FIN-8422" — data
priority <~ 2;           // "Priority is 2" — data
```

What would `!` on metadata mean? `!jira <~ "FIN-8422"` — "the metadata jira
MUST be FIN-8422"? It's already `FIN-8422` by declaration; there's nothing
to enforce. `!` would be noise.

What would `?` on metadata mean? `?jira <~ "FIN-8422"` — "explain why this
metadata is FIN-8422"? That's a question about a source-code value, not
about the compiler's reasoning. The `?` diagnostic is specifically about
pass-level decisions ("why did you vectorize?"), not provenance of values.

**Therefore**: `!` and `?` are annotation-only. `<~` body metadata uses
neither. A `#!` annotation and a `<~` metadata with the same key do not
conflict — they occupy different namespaces on different AST nodes
(`annotations` on Signature vs `metadata` on Definition/Transaction).

This keeps a clean separation: "what the compiler must do" (annotations,
with `!`/`?`) vs "what this item is" (metadata, with `<~`).

**Additional constraint**: `<~` is reserved for compile-time metadata. It
is never a runtime operator. All runtime data movement uses other tokens:
- Channel/stream writes: `<-` (ArrowLeft)
- Assignment: `=` (Eq)
- Transfer: `->` (Arrow)

This means the lookahead in Step 1A.1c is always safe — `identifier <~`
can only mean "metadata key" in every context.

### Step 1A.2 — Restructure TypeDefBody: metadata vs projections

**What**: `TypeDefBody.bindings` is split into `metadata` and `projections`.
This is the cleanup after Phase 0.2's syntax change — `TypeDefBody` now
reflects the clean split between compile-time metadata (`<~`) and lazy
projections (`name(params) = expr`).

**File**: `src/ast.rs`

**Before**:
```rust
pub struct TypeDefBody {
    pub slots: Vec<TypeSlot>,
    /// All bindings: both metadata properties and parameterized projections.
    pub bindings: Vec<TypeBinding>,
    pub operators: Vec<OpDeclaration>,
    pub constraints: Vec<Expr>,
}
```

**After**:
```rust
pub struct TypeDefBody {
    /// Slot declarations: `name: Type;` — structural bit partitions.
    /// 2026-07-11: Phase 1A cleaned from bindings.
    pub slots: Vec<TypeSlot>,
    /// Compile-time constant metadata: `name <~ expr;`.
    /// 2026-07-11: Phase 1A — split from bindings.
    pub metadata: HashMap<String, PropertyValue>,
    /// Parameterized projections: `name(param1, param2) = expr;`.
    /// These are lazy, may reference `self` slots.
    /// 2026-07-11: Phase 1A — split from bindings.
    pub projections: Vec<TypeBinding>,
    /// Operator declarations: `op Rune(Param) -> Ret = intrinsic;`
    pub operators: Vec<OpDeclaration>,
    /// Refinement constraints: `[ > 0 ]`.
    pub constraints: Vec<Expr>,
}
```

**How the parser populates these** (already handled by Phase 0.2 syntax
changes — just update the data flow):

In `parse_type_def()`, instead of pushing to `bindings`, push to
`metadata` or `projections`:

```rust
// Slot: push to slots
slots.push(TypeSlot { ... });

// <~ binding (metadata):
metadata.insert(name, property_value_from_expr(&value)?);

// = binding with params (projection):
projections.push(TypeBinding { name, params, value: Box::new(value), ... });

// operator declaration:
operators.push(op_decl);

// constraint:
constraints.push(expr);
```

**Rename all existing code that reads `type_def.body.bindings`**:

Search for `body.bindings` in the codebase:
```bash
grep -rn '\.body\.bindings' src/
```

Every site must be updated to read from `body.metadata`, `body.projections`,
or both depending on which kind of binding it needs.

Sites to update include:
- `src/type_universe.rs`: `resolve_type_def()` — iterate both `metadata`
  and `projections`
- `src/backend/llvm/mod.rs`: struct auto-registration scan — check
  `projections` for field access patterns
- Any other backend or analysis pass that reads type body bindings

**Nesting check**: Straightforward rename — no nesting concern.

**Tests**:
- Existing type body tests must still pass (adjusted for new field names)
- `test_type_body_metadata_map`: verify `type F { bytes <~ 8; };` produces `body.metadata["bytes"] == Int(8)`
- `test_type_body_projections`: verify `type F { At(i) = self[i]; };` produces `body.projections[0].name == "At"`

### Step 1A.3 — Codegen: metadata produces zero IR

**What**: When the codegen loop encounters a `<~ expr;` metadata statement
in a defn or txn body, it skips it entirely — no LLVM IR is emitted.

**Files**: `src/backend/llvm/emit_body.rs`, `src/backend/llvm/mod.rs`
(and equivalent paths in other backends)

**Approach**: Metadata statements are NOT stored as `Statement` variants.
They are collected during parsing into the item's metadata HashMap (Step
1A.1c). The codegen loop only sees regular statements (assignments, calls,
guarded blocks, term, etc.). Since metadata never enters the statement
list, it automatically produces zero IR.

But we must ensure backward compatibility: if metadata DOES enter the
statement list (e.g., through an older code path or a forgotten
refactoring), the codegen loop should skip it:

```rust
fn emit_statement(tx: &mut TransactionContext, stmt: &Statement, out: &mut String) {
    match stmt {
        Statement::Metadata { .. } => {
            // No-op — skip metadata statements entirely
            // These are collected during parsing and stored on the item
        }
        // ... existing arms
    }
}
```

However, the cleaner approach (design intent) is that metadata NEVER
reaches the statement list. The parser collects it directly into the
item's metadata field during parsing. If we find the parser creating
`Statement::Metadata` variants, remove that code and route to the item's
metadata HashMap instead.

**Verification**: Write a test that:
1. Parses a `defn` with `<~` metadata
2. Verifies the metadata HashMap is populated
3. Verifies the statement list does NOT contain `Statement::Metadata`
4. Compiles to IR and verifies no spurious instructions

**Tests**:
- `test_codegen_no_ir_for_metadata`: compile `defn foo() -> Int { jira <~ "x"; term 42; };` and
  `defn foo() -> Int { term 42; };` — verify emitted IR is identical
- `test_defn_body_statements_exclude_metadata`: verify the statement vector
  of a defn with `<~` does not contain any metadata statement

### Step 1A.4 — Full test sweep for Phase 1A

Run `cargo test --lib` after each sub-step. The full Phase 1A test
additions:

| Sub-step | New tests | File |
|----------|-----------|------|
| 1A.0 | `test_prefix_annotation_rejected`, `test_hashtag_annotations_still_work` | `src/parser.rs` |
| 1A.1a | `test_property_value_enum` | `src/ast.rs` or `src/type_universe.rs` |
| 1A.1c | `test_metadata_in_defn_body`, -txn, -guard, -non-constant, -list, -identifier, -multiple | `src/parser.rs` |
| 1A.1b | `test_hash_question_diagnostic_field`, -bang, -bare, `test_hash_*_diagnostic_false`, `test_diagnostic_no_ir_change` | `src/parser.rs` + backend test |
| 1A.2 | `test_type_body_metadata_map`, `test_type_body_projections` | `src/parser.rs` |
| 1A.3 | `test_codegen_no_ir_for_metadata`, `test_defn_body_statements_exclude_metadata` | Backend test |

**Target**: All existing tests pass + ~20 new tests.

### Step 1A.5 — Variable metadata (reserved, not implemented)

**What**: The `#?`-qualified syntax for variable metadata is reserved but
not yet implemented. The grammar recognizes the pattern and produces a
clear error.

**Syntax** (reserved):
```brief
let x: Int = 42;
x <~ (jira: "FIN-8422", range: [0, 100]);
```

The `<~ (name: expr, ...)` form is specifically for variables. Note: this
uses the OLD structured annotation syntax `<~ (name: expr, ...)` which was
removed from `parse_hashtag_modifiers()` in 1A.0. For variables, we
re-introduce it as a POST-STMT annotation instead of pre-sig.

**Parser**: In `parse_let_binding()` or the statement-level post-let parse,
after consuming the semicolon that ends a `let` binding, check if the next
token is `<~`:

```rust
// After parsing "let x: Type = expr ;"
// Check for variable metadata: x <~ (name: expr, ...)
if let Some(Ok(Token::Identifier(name))) = self.current_token() {
    if name == &var_name && self.peek_token(1).map_or(false,
        |t| matches!(t, Ok(Token::TildeArrow)))
    {
        return self.spanned_err(
            "variable metadata not yet implemented");
    }
}
```

**Error message**: `"variable metadata not yet implemented — use type-level or item-level annotations instead"`

**Rationale for reserving**: Variable metadata requires a side-table in
the SSA (`HashMap<RegisterId, HashMap<String, PropertyValue>>`), which
depends on the metadata pipeline architecture from Phase 1B+. Reserving
the syntax now prevents future ambiguity.

**Tests**:
- `test_variable_metadata_reserved`: `let x: Int = 42; x <~ (jira: "x");` → specific error message
- `test_variable_metadata_does_not_break_normal_let`: `let x: Int = 42; let y: Int = 0;` → no error

---

## Phase 1B — Generic Property System (was old Phase 1)

### Goal

Convert `ResolvedType`'s ~30 hardcoded fields into a generic
`HashMap<String, PropertyValue>`. This is the foundation for ALL downstream
phases: codec system, custom literals, WASM target, plugins.

The `PropertyValue` enum is already defined in Phase 1A (Step 1A.1a). This
phase wires it into the type system and provides accessors for migration.

### Step 1B.1 — Add `properties` HashMap to `ResolvedType`

**File**: `src/type_universe.rs`

Add to `ResolvedType`:

```rust
/// Generic property map. All typed properties are ALSO stored here
/// during the migration phase. After Phase 2, ONLY this map remains.
/// 2026-07-11: Phase 1B.
pub properties: HashMap<String, PropertyValue>,
```

Keep ALL existing hardcoded fields. During Phase 1B, we dual-write:
`apply_binding()` populates BOTH the hardcoded field (for backward compat)
and the `properties` map (for migration).

**Nesting check**: Single struct field addition.

### Step 1B.2 — Dual-write in `apply_binding()`

**File**: `src/type_universe.rs` (function `apply_binding`, line 639)

For EVERY match arm in `apply_binding()`, add a corresponding
`self.properties.insert()` call. Example:

```rust
"bytes" => {
    if let Some(n) = binding.value.as_integer() {
        rt.bytes = n as u64;
        rt.properties.insert("bytes".to_string(),
            PropertyValue::Int(n as u64 as i64));
    }
}
"alignment" => {
    if let Some(n) = binding.value.as_integer() {
        rt.alignment = n as u64;
        rt.properties.insert("alignment".to_string(),
            PropertyValue::Int(n as u64 as i64));
    }
}
"llvm" => {
    if let Some(s) = binding.value.as_string() {
        rt.llvm_type = s.to_string();
        rt.properties.insert("llvm".to_string(),
            PropertyValue::String(s.to_string()));
    }
}
```

Also update the "unknown name → projection" fallthrough:

```rust
_ => {
    rt.projections.insert(binding.name.clone(), binding.clone());
    // Also store in properties map for generic access
    // 2026-07-11: Phase 1B
    let prop_val = property_value_from_binding(binding);
    rt.properties.insert(binding.name.clone(), prop_val);
}
```

**Note**: This step assumes `apply_binding()` still receives
`TypeBinding` entries. In Phase 1A, `TypeDefBody.bindings` was split into
`metadata` and `projections`. The `metadata` entries go directly to the
`properties` map. The `projections` entries go through `apply_binding()`
as before. So `apply_binding()` now only processes `projections`, not
`metadata`.

**Update `resolve_type_def()`** to read from both `body.metadata` and
`body.projections`:

```rust
// Phase 1A: metadata entries go directly to properties
for (name, value) in &body.metadata {
    rt.properties.insert(name.clone(), value.clone());
}

// Phase 1A: projection entries go through apply_binding (for now)
for binding in &body.projections {
    self.apply_binding(&mut rt, binding, type_params, &mut errors);
}
```

**Nesting check**: Each arm is a single-level match.

**Tests**: Existing tests implicitly test this. Add one explicit test:
`test_property_map_populated_for_known_binding` — verify that after
`resolve_type_def("type F : Bits { bytes <~ 8; }")`, `rt.properties`
contains `("bytes", Int(8))`.

### Step 1B.3 — Update all `ResolvedType` constructors

**Files**:
- `src/type_universe.rs`: `default_primitive()` (line 343), `resolve_type_def()` (line 502)
- `src/backend/llvm/mod.rs`: struct auto-registration (line 1584)
- `src/backend/bindgen.rs`: `make_test_type()` (line 352)

Add `properties: HashMap::new()` to EVERY `ResolvedType { ... }` expression.
If the constructor sets hardcoded fields, also insert corresponding property
entries.

For `resolve_type_def()`, the `properties` map must be populated with
default values matching the hardcoded defaults:

```rust
let mut properties: HashMap<String, PropertyValue> = HashMap::new();
properties.insert("bytes".to_string(), PropertyValue::Int(0));
properties.insert("alignment".to_string(), PropertyValue::Int(1));
properties.insert("llvm".to_string(), PropertyValue::String("i64".to_string()));
properties.insert("storage".to_string(), PropertyValue::String("Boxed".to_string()));
properties.insert("tbaa".to_string(), PropertyValue::String("Int".to_string()));
// ... all ~30 defaults
```

Then at the end of `resolve_type_def()`, after bindings are applied:

```rust
rt.properties = properties;
```

**Nesting check**: Adding a field to a struct literal is depth 1.

**Tests**: Must compile. `cargo test --lib` must still pass.

### Step 1B.4 — Add accessor methods to `ResolvedType`

**File**: `src/type_universe.rs` (after the struct definition)

For each hardcoded field, add a getter that FIRST checks the `properties`
map (migration path) and FALLS BACK to the struct field (backward compat):

```rust
impl ResolvedType {
    /// Get the byte size. Prefers the properties map, falls back to struct field.
    /// 2026-07-11: Phase 1B.
    pub fn get_bytes(&self) -> u64 {
        self.properties.get("bytes")
            .and_then(|v| if let PropertyValue::Int(n) = v { Some(*n as u64) } else { None })
            .unwrap_or(self.bytes)
    }

    pub fn get_llvm_type(&self) -> &str {
        self.properties.get("llvm")
            .and_then(|v| if let PropertyValue::String(s) = v { Some(s.as_str()) } else { None })
            .unwrap_or(&self.llvm_type)
    }

    pub fn get_storage(&self) -> &str {
        self.properties.get("storage")
            .and_then(|v| if let PropertyValue::String(s) = v { Some(s.as_str()) } else { None })
            .unwrap_or(&self.storage)
    }

    pub fn get_tbaa_node(&self) -> &str {
        self.properties.get("tbaa")
            .and_then(|v| if let PropertyValue::String(s) = v { Some(s.as_str()) } else { None })
            .unwrap_or(&self.tbaa_node)
    }

    pub fn get_alignment(&self) -> u64 {
        self.properties.get("alignment")
            .and_then(|v| if let PropertyValue::Int(n) = v { Some(*n as u64) } else { None })
            .unwrap_or(self.alignment)
    }

    pub fn get_box_op(&self) -> Option<&str> {
        self.properties.get("box")
            .and_then(|v| if let PropertyValue::String(s) = v { Some(s.as_str()) } else { None })
            .or(self.box_op.as_deref())
    }

    pub fn get_unbox_op(&self) -> Option<&str> {
        self.properties.get("unbox")
            .and_then(|v| if let PropertyValue::String(s) = v { Some(s.as_str()) } else { None })
            .or(self.unbox_op.as_deref())
    }

    // ... one getter per hardcoded field (see old plan Step 1.5 for complete list)
}
```

Each getter follows the same pattern: `properties.get("key") → and_then →
unwrap_or(field)`.

**Nesting check**: Each getter is a 3-line chain — depth 2.

**Tests**: Add one test per getter:
- `test_get_bytes_via_property`: construct `ResolvedType` with `{"bytes": Int(8)}` in properties, set `bytes=0`, verify `get_bytes() == 8`
- `test_get_llvm_type_via_property`: same pattern

### Step 1B.5 — Verify dual-write coverage

**What**: After Step 1B.2 and 1B.3, every binding that sets a hardcoded
field ALSO sets the corresponding property map entry. Every accessor
prefers the map. All existing tests pass because:
1. Old code paths still read hardcoded fields directly (unchanged behavior)
2. New accessors return same values (dual-write ensures map == field)
3. Accessors not yet used anywhere — they're infrastructure for Phase 2

**Tests**: `cargo test --lib` — all existing tests must pass.

---

## Phase 2 — Eliminate Hardcoded Type-Name Matches in Codegen

(Same as the old plan — unchanged. Copy from the previous version.)

### Goal

Replace ALL `Type::Custom("Int")`, `Type::Custom("Float")`, etc. string
dispatch in backend codegen with universe lookups and property queries.

~370 sites across 20+ files. One module at a time.

### Strategy: Conservative Migration

Each codegen site follows a standard replacement pattern.

**Before**:
```rust
if ty == Type::Custom("Float".to_string()) { /* float path */ }
```

**After**:
```rust
if let Some(rt) = universe.get_by_type(&ty) {
    if rt.get_storage() == "Native" && rt.get_llvm_type() == "float" {
        /* float path */
    }
}
```

**CRITICAL**: Always keep the old hardcoded match as a fallback until the
file is fully migrated. `_ => {}` at the end. This prevents silent
regressions for types not yet registered in the universe.

### Step 2.1 — Add convenience methods to TypeUniverse

Add to `TypeUniverse`:

```rust
pub fn llvm_type_for(&self, ty: &Type) -> Option<&str>;
pub fn byte_size_for(&self, ty: &Type) -> Option<u64>;
pub fn is_native(&self, ty: &Type) -> bool;
pub fn tbaa_matches(&self, ty: &Type, expected: &str) -> bool;
```

(Full implementation in the old plan — copy verbatim.)

### Step 2.2 — Migrate ONE file at a time

Ordered by impact: `builder.rs` → `emit_toplevel.rs` → `mod.rs` → helpers → expr/* → other backends.

**For each file**:
1. Add `use` for `TypeUniverse`
2. Thread `&TypeUniverse` parameter into functions that do type dispatch
3. Replace each `Type::Custom("Foo")` match with universe query
4. Keep legacy fallback in same match arm
5. `cargo test --lib` after EACH file
6. `bash benchmarks/build_and_bench.sh --correctness` every 5 files

### Step 2.3 — Remove hardcoded ResolvedType fields

After ALL codegen sites have been migrated (no remaining `.bytes`,
`.llvm_type`, `.storage`, etc. field access outside of `type_universe.rs`):

1. Remove each hardcoded field from `ResolvedType`
2. Update constructors to only populate `properties` map
3. Simplify accessors to only read from the map (remove fallback)
4. Remove `default_primitive()`

### Step 2.4 — Clean up `byte_size()` and `alignment()` in TypeUniverse

Replace hardcoded `if __t == "Int" => Some(8)` with:
```rust
Type::Custom(name) => self.get(name).map(|rt| rt.get_bytes()),
```

Keep `LayoutPtr` and `Applied("Ptr", _)` special cases.

---

## Phase 3 — String Slot Migration

(Was old Phase 0.2, deferred until after Phase 2.)

### Goal

Now that codegen queries the property system instead of matching
`Type::Custom("String")`, change String's LLVM representation from `i8*`
to `%String*`.

### Step 3.1 — Update `bootstrap.bv`

**File**: `lib/std/types/bootstrap.bv` (line 270)

**Before**:
```brief
type String : Bits {
    bytes <~ 8;
    alignment <~ 8;
    llvm <~ "i8*";
    storage <~ "Boxed";
    tbaa <~ "String";
    box <~ "ptrtoint#";
    unbox <~ "inttoptr#";
    default_width <~ 64;
    default_codec <~ 0;
};
```

**After**:
```brief
type String {
    ptr: Ptr<UInt8>;
    len: Int;
    codec: UInt8;
    bytes <~ 24;
    alignment <~ 8;
    llvm <~ "%String";
    storage <~ "Native";
    tbaa <~ "String";
    box <~ "ptrtoint#";
    unbox <~ "inttoptr#";
};
```

### Step 3.2 — Update `validate_primitives()`

Change String's validation table:
```rust
("String", "llvm", "%String"),
("String", "storage", "Native"),
("String", "bytes", "24"),
```

### Step 3.3 — Verify all benchmarks

```bash
bash benchmarks/build_and_bench.sh --correctness
bash benchmarks/build_and_bench.sh --runtime
```

### Step 3.4 — FFI boundary handling for String

**What**: The String ABI changes from `i8*` (pointer-width boxed) to
`%String*` (pointer to `{ ptr, i64, i8 }` struct). Every FFI call site
that passes or receives a String must now unpack/pack the struct pointer,
because foreign code expects a `char*` / `i8*`, not a `%String*`.

**Affected files**:
- `src/backend/llvm/helpers.rs` — FFI parameter marshaling
- `src/glue/export.rs` — C type mapping for exported functions
- `src/backend/bindgen.rs` — FFI type mapping for imported functions
- Any `frgn` call site passing a String argument or receiving a String return

**Marshaling rules**:

*Calling a foreign function with a String argument* (`Boxed` → `%String*`):
- Before: emit `i8*` directly (String was already a pointer)
- After: emit `ptrtoint (%String* to i64)`, then check `storage` property:
  - If `storage == "Native"` (new): String is a struct pointer, so box it
    to `i64` for the state, then unbox to `%String*`, then GEP `.ptr` field,
    then bitcast to `i8*`, then pass to the foreign function
  - If `storage == "Boxed"` (legacy fallback during migration): String is
    already `i8*`, emit as before

*Receiving a String return from a foreign function* (`i8*` → `%String*`):
- Before: emit `i8*` directly
- After: receive `i8*`, then construct a `%String` struct: set `.ptr` to
  the received pointer, `.len` to `strlen` (via `__strlen#` intrinsic or
  equivalent), `.codec` to 0 (default). Then `alloca` the struct and store.

The GLUE-generated glue code must be updated to handle this conversion.
The `--library` flag's emitted bindings must also account for the change.

**Helper function** (add to `emit_toplevel.rs` or `helpers.rs`):

```rust
/// Emit FFI marshaling code for a String-typed parameter.
/// 2026-07-11: Phase 3 — String is now %String*, not i8*.
fn emit_string_ffi_param(builder: &mut LlvmBuilder, reg: &TypedRegister, out: &mut String) {
    if reg.ty == Type::Custom("String".to_string()) {
        let universe = builder.ctx.type_universe.as_ref().unwrap();
        let rt = universe.get("String").unwrap();
        if rt.get_storage() == "Native" {
            // New: String is %String* — extract .ptr field
            writeln!(out, "  %unboxed = call i64 @__unbox_string(i64 %{})", reg.name)?;
            writeln!(out, "  %str_ptr = inttoptr i64 %unboxed to %String*")?;
            writeln!(out, "  %c_str = getelementptr %String, %String* %str_ptr, i32 0, i32 0")?;
            writeln!(out, "  %c_char_ptr = bitcast i64* %c_str to i8*")?;
        } else {
            // Legacy: still i8*
            writeln!(out, "  %c_char_ptr = inttoptr i64 %{} to i8*", reg.name)?;
        }
    }
}
```

**Tests**:
- `test_string_ffi_param_native`: compile a `frgn` call passing String,
  verify emitted FFI marshaling extracts `.ptr`
- `test_string_ffi_param_legacy`: same with legacy `storage == "Boxed"`
- `test_string_ffi_return`: compile a `frgn` returning String, verify
  struct construction from `i8*`
- `test_glue_export_string`: exported function with String parameter,
  verify C header uses `char*`

---

## Phase 4 — Codec System

(Unchanged from the old plan. Copy verbatim.)

### Goal

Add the `codec` keyword for declaring codec implementations.

### Steps

4.1 — Add `CodecDeclaration` AST node
4.2 — Parse `codec` declarations
4.3 — Add `CodecRegistry` to TypeUniverse
4.4 — Wire codec → type linking in `resolve_type_def()`
4.5 — Codec-based validation

(Full implementation in the old plan — copy verbatim.)

---

## Phase 5 — Custom Literal Parsers

(Unchanged from the old plan. Copy verbatim.)

### Goal

Enable `let r: RomanNumeral = XIV;` with codec-based parse handlers.

### Steps

5.1 — Add `Expr::DeferredLiteral` variant
5.2 — Register literal parsers during codec ingestion
5.3 — Detect typed declarations and defer
5.4 — Resolve deferred literals after type resolution
5.5 — Wire into the compilation pipeline
5.6 — Evaluate the codec parse handler via interpreter

(Full implementation in the old plan — copy verbatim.)

---

## Phase 6 — WASM Target

(Unchanged from the old plan. Copy verbatim.)

### Goal

Compile Brief to WebAssembly via LLVM's `wasm32-unknown-wasi` target.

### Steps

6.1 — Add `--target wasm32` CLI flag
6.2 — Configure LLVM for WASM
6.3 — Handle address space in Pointer type
6.4 — Handle WASM calling convention
6.5 — WASI imports for I/O
6.6 — Test WASM output

(Full implementation in the old plan — copy verbatim.)

---

## Phase 7 — Plugin System

(Unchanged from the old plan. Copy verbatim.)

### Goal

Enable Brief to compile to WebAssembly and load those WASM modules as
compiler plugins.

### Steps

7.1 — Define the WIT interface
7.2 — Add `wasmtime` runtime dependency
7.3 — Create plugin loader
7.4 — Define plugin loading API
7.5 — Wire plugin hooks into compilation pipeline
7.6 — Example plugin in Brief
7.7 — Test plugin system

(Full implementation in the old plan — copy verbatim.)

---

## Testing Strategy Summary

| Phase | Focus | Test count delta |
|-------|-------|-----------------|
| 0 | Lexer cleanup + syntax simplification | ~-6 (dead tests removed), +10 (new type body syntax) |
| 1A | Metadata everywhere | +25 (parser, AST, codegen) |
| 1B | Generic property system | +30 (accessor tests) |
| 2 | Hardcoded type-name elimination | +50 (migration per module) |
| 3 | String migration execution | +10 (end-to-end) |
| 4 | Codec system | +20 (parser + resolve) |
| 5 | Custom literal parsers | +15 (deferred resolve) |
| 6 | WASM target | +10 (target triple, address space) |
| 7 | Plugin system | +15 (load, hooks, sandbox) |

---

## Documentation Updates

Every phase must update:

| Doc | Phase | What |
|-----|-------|------|
| `docs/architecture/features/annotations.md` | 1A | New annotation system: `#tag`, `#!tag`, `#?tag`, inline `<~` |
| `docs/architecture/features/extensible-types.md` | 1B | Generic property system design |
| `docs/architecture/codegen.md` | 2 | Universe-based type dispatch |
| `docs/architecture/type-universe.md` | 2, 4 | Property map + codec registry |
| `docs/architecture/features/ffi.md` | 3 | String as struct in FFI |
| `docs/architecture/features/codec.md` | 4 | Codec declaration syntax |
| `docs/architecture/features/literals.md` | 5 | Custom literal syntax |
| `docs/architecture/features/wasm.md` | 6 | WASM target specifics |
| `docs/architecture/features/plugins.md` | 7 | Plugin API and lifecycle |
| `docs/architecture/features/metadata.md` | 1A | Metadata storage model on items |

---

## Risk Register

| Risk | Phase | Mitigation |
|------|-------|------------|
| Phase 0.2 `=` removal breaks external .bv files | 0 | Add a clear error message: `"use '<~' for metadata in type bodies"`. No silent semantic change. |
| Phase 1A doesn't catch all `~>` usage | 1A | `grep -rn '~>' src/` after Phase 1A.0. If anything remains, it's dead code — remove it. |
| Phase 2 migration incomplete (missed a codegen site) | 2 | Keep legacy fallback in ALL replacement sites. `grep -rn 'Custom(' src/` after every commit |
| String migration breaks existing benchmarks | 3 | Benchmark before and after. Fix in concert with Phase 2 complete |
| Codec parse handler performance at compile time | 5 | Use interpreter only for simple codecs. Cache results. |
| WASM ABI mismatch with LLVM expectations | 6 | Start with trivial programs (no structs, no frgn). Add complexity incrementally. |
| Plugin API version skew | 7 | WIT interface is versioned. Compiler checks plugin `api_version` on load and rejects mismatches. |

---

## Deleted Syntax Summary

| Token / Syntax | Removed in | Replacement |
|----------------|------------|-------------|
| `~>` (`TildeArrowRight`) | Phase 0.0 | Remove entirely |
| `(name: val) ~> item` (prefix annotation) | Phase 1A.0 | `item { name <~ val; ... }` |
| `<~ (name: val, ...)` in `parse_hashtag_modifiers()` | Phase 1A.0 | `#?name(val)` or inline `<~` |
| `name = value;` in type bodies (no params) | Phase 0.2 | `name <~ value;` |
| `AnnotationMode::Speculative` (replaced by `diagnostic: true`) | Phase 1A.1b | `diagnostic: true` field on `Annotation` |

---

## Summary of New/Modified AST Fields

| AST node | New/Changed field | Source | Phase |
|----------|-------------------|--------|-------|
| `Annotation` | `diagnostic: bool` | `#?` prefix | 1A.1b |
| `Definition` | `metadata: HashMap<String, PropertyValue>` | `<~` statements in body | 1A.1 |
| `Transaction` | `metadata: HashMap<String, PropertyValue>` | `<~` statements in body | 1A.1 |
| `GuardBranch` | `metadata: HashMap<String, PropertyValue>` | `<~` statements in guard | 1A.1 |
| `TypeDefBody` | `metadata: HashMap<String, PropertyValue>` (was `bindings`) | `<~` bindings | 1A.2 |
| `TypeDefBody` | `projections: Vec<TypeBinding>` (split from `bindings`) | `=` bindings with params | 1A.2 |
| `TypeDefBody` | **removed** `bindings: Vec<TypeBinding>` | Split into metadata + projections | 1A.2 |
| `ResolvedType` | `properties: HashMap<String, PropertyValue>` | Resolved from TypeDefBody.metadata | 1B.1 |
| `PropertyValue` | New enum | Metadata values | 1A.1 / 1B.1 |
| `CodecDeclaration` | New struct | `codec` keyword | 4 |
| `Expr::DeferredLiteral` | New variant | Codec parse handler | 5 |

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
4. `bash benchmarks/build_and_bench.sh --correctness` before every phase boundary commit
5. Commit messages: `"YYYY-MM-DD: Phase N.M — <description>"`
6. Do not ask "shall I commit?" — the instructions from AGENTS.md say auto-commit

---

## Immediate Next Action

Phase 0, Step 0.0: Remove `TildeArrowRight` from the lexer.
Phase 0, Step 0.1: Delete dead parser tests at `parser.rs:12247-12305`.
Phase 0, Step 0.2: Remove `=` as metadata delimiter in type body parsing.

---

## Phase 8 and Beyond — Roadmap After Extensible Types

After Phases 0–7 are complete, the compilation pipeline is fully
metadata-driven: types carry properties, codecs handle custom
serialization, plugins extend the compiler at every stage. The next
two major efforts build directly on this foundation:

### Next: Derivation & Synthesis (Phases 8–8G–14)

**Plan**: `docs/plans/2026-07-11-derivation-synthesis-comprehensive.md`

Adds `:=` derivation blocks, compile-time assertions, SMT-guided program
synthesis, sad-path FFI error recovery, contract-guided deduction, the
`.dbvl` semantic archive for decoupled backends, and **Phase 8G** which
removes the `Intrinsic` enum, `inop` keyword, and `Expr::IntrinsicCall` —
completing the frontend/backend split by routing all operator dispatch
through metadata strings.

**Prerequisites from this plan**:
- Phase 1B (Generic Property System): synthesis uses operator cost models
  derived from type properties; 8G uses metadata properties for dispatch
- Phase 5 (Custom Literal Parsers): compile-time interpreter reuse for
  assertion execution
- Phase 7 (Plugin System): SMT solver integration via WASM plugin;
  `.dbvl` archive enables decoupled backends

### Parallel: Phases 16A–16F — Extension Modifiers, Entry Points, Scripting

**Plan**: `docs/plans/2026-07-12-modifiers-entry-scripting.md`

Adds `.s`/`.f`/`.c` filename modifiers (`main.sf.bv`), `[#]` entry
precondition with CLI dispatch, indentation-based layout parsing (`.f`),
cell-wrapped files (`.c`), `input`/`output` keywords, top-level scripting,
and the stdlib `cli.c.bv` CLI framework. All frontend-only.

**Prerequisites from this plan**:
- Phase 1A (metadata infrastructure): `[#]` uses the contract system
- Phase 7 (plugin system): `cli.c.bv` can discover entry points via plugin hooks

### Parallel: Alloc Metadata System

**Plan**: `docs/plans/2026-07-12-alloc-metadata.md`

Adds `alloc` as a `<~` metadata annotation on variable bindings for
stack, physical MMIO, arena, and placement allocation. Frontend validates
`"Stack"` and physical address constants; backends validate and emit the
correct IR. Known key + unparseable value → error; unknown key → silently
ignored.

**Prerequisites from this plan**:
- Phase 1B (property system): `<~` metadata infrastructure
- Phase 2 (codegen migration): backends query the universe for type sizes

### After That: Phase 15 — Library Mode Completion

**Plan**: `docs/plans/2026-07-11-library-mode-completion.md`

**Proposal by**: [@revred](https://github.com/revred) — reviewed Brief's
`--library` infrastructure and identified five gaps between the existing
`.ll`-level library mode and a consumable C-callable library.

Adds the `export` keyword, `.ll` → `.o` → `.a` packaging,
`__brief_init_state`/`__glue_release` in generated headers,
`Bool`/`String` marshaling at the FFI boundary, and an end-to-end
C driver test.

**Prerequisites from this plan**:
- Phase 1B (Generic Property System): type properties determine
  marshaling (e.g., `String.bytes == 24` triggers struct-to-pointer
  conversion at the boundary)
- Phase 2 (codegen migration to universe queries): the export wrapper
  emission queries the universe for type layout rather than hardcoding

### Then: GLUE v2 — FFI Metadata Pipeline

**Plan**: `docs/plans/2026-07-10-glue-v2-ffi-unification.md`

The `bridge-exports.dbvl` metadata format for foreign build systems.
Complementary to Phase 15: Phase 15 produces the linkable binary, GLUE
v2 produces the metadata that tells the foreign build system how to
call it.

### Then: Zero-Copy Meld and Cross-Language LTO

Detailed in `docs/plans/2026-07-10-zero-copy-glue-bridge-phases.md`.
Extends zero-copy from scalars to composites via LLVM struct signatures,
meld projections for non-LLVM targets, and cross-language LTO.
