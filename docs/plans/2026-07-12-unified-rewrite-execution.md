# Unified Rewrite Execution Plan

**Date:** 2026-07-12
**Status:** Plan — ready for execution
**Design review:** [✓] Architecture validated against Bits thesis, metadata dispatch, and intrinsic/type separation of concerns
**Supersedes:** All prior partial plans:
- `2026-07-11-big-rewrite-execution.md`
- `2026-07-12-intrinsic-architecture.md`
- `2026-07-11-derivation-synthesis-comprehensive.md`
- `2026-07-12-modifiers-entry-scripting.md`
- `2026-07-12-alloc-metadata.md`
- `2026-07-11-library-mode-completion.md`

**See also:**
- `docs/architecture/bits-thesis.md` — the three axioms
- `docs/architecture/features/metadata-dispatch.md` — distributed validation
- `AGENTS.md` — coding standards, flat control flow, commenting mandate

---

## Table of Contents

1. [Mandate — What "Rewrite" Means](#1-mandate)
2. [Architecture Risks — Documented Before We Start](#2-architecture-risks)
3. [File Size Budget — Hard Limit: 3000 Lines, Target: 2000](#3-file-size-budget)
4. [Flat Control Flow — The Non-Negotiable Rule](#4-flat-control-flow)
5. [Testing Mandate — Behavior, Not Implementation](#5-testing-mandate)
6. [Dependency Graph](#6-dependency-graph)
7. [Phase 0 — Foundation](#7-phase-0)
8. [Phase 1 — Parser + Backend Interface Design](#8-phase-1)
9. [Phase 2 — Type System + Type Lowering](#9-phase-2)
10. [Phase 3 — Interpreter + Intrinsics](#10-phase-3)
11. [Phase 4 — Backend Implementation](#11-phase-4)
12. [Phase 5 — Proof Engine + Analysis](#12-phase-5)
13. [Phase 6 — Derivation Module](#13-phase-6)
14. [Phase 7 — Main + CLI + Library Mode](#14-phase-7)
15. [Phase 8 — Archive + Tests + Stdlib](#15-phase-8)
16. [Verification — Does This Plan Match the Intended Design?](#16-verification)

---

## 1. Mandate — What "Rewrite" Means

### 1.1 Every File Is Written From Scratch

There is no such thing as "modifying" or "updating" or "patching" a file in this plan.
Every file in the rewrite scope is:

1. **Deleted** (if it should not exist in the new architecture — e.g., `intrinsic_dispatch.rs`),
2. **Discarded and replaced** with a new file that provides the same external behavior
   but is written from scratch with flat code, clean structure, and all required features,
   OR
3. **Kept as-is** only if it is already clean, flat, and follows the architecture
   (explicitly listed as KEEP in each phase).

**No find-replace. No surgical edit. No "while we're here" patching.**
If a file is in the rewrite scope, its entire contents are replaced.
The old file is not used as a reference for line-by-line translation — it is
used only to understand what behavior must be preserved.

### 1.2 The Onion Principle

Files are written from the outside in, starting at the dependency leaves.
A file at level N may ONLY import from levels < N. This ensures:

- No circular dependencies
- No forward references to types that haven't been defined yet
- Every phase can be built and tested independently of later phases

### 1.3 Behavioral Fidelity

The rewritten compiler must produce IDENTICAL results for all existing
test cases and benchmarks. The only intentional differences are:

- Removal of `inop` keyword (becomes a `defn` with `interpreter_impl` metadata)
- Removal of `Intrinsic` enum / `Expr::IntrinsicCall` / old variant names
- Intrinsic names renamed to PascalCase`#` (e.g., `__add_i64#` → `AddI64#`)
- New type errors for things the old compiler silently accepted
  (e.g., calling a `[#]` function from internal code)
- Enforced `alloc` validation (previously unchecked metadata passed through)

### 1.4 What is NOT Changing

- The `Value::Bits` representation — already correct
- The `Expr` enum (modulo deleted variants and renamed forms) — shape is stable
- The parser's overall structure — token stream → top-level items → expressions
- The LLVM IR output format — same `.ll` file structure, same optimization passes
- The contract/proof system — same SMT-LIB queries
- The `.dbvl` archive format (when implemented) — same tagged-line format

---

## 2. Architecture Risks — Documented Before We Start

These risks were identified during design review and are ACCEPTED, but must
be addressed explicitly during implementation. Every phase that touches the
risk area must include mitigations.

### Risk 1: `op` Resolution Is the Single Point of Failure

**What:** Every operation in every program (`a + b`, `x[i]`, `term v`) routes
through the typechecker's `op` binding resolution: resolve the type of the
left operand → look up `op Add` on that type → resolve the binding to either
an intrinsic (`AddI64#`) or a user function → verify signature compatibility.

**Why it's risky:** If this function is slow, buggy, or incomplete, **every**
program fails. There is no bypass path. One missing binding cascades to every
expression of that type.

**Mitigation in implementation:**
- `get_operator_intrinsic(rune: &str, ty: &Type) -> Option<OpBinding>`
  must be the most thoroughly tested function in the compiler.
- Test matrix: every built-in type × every operator = combinatorial coverage.
- Returns `None` if the type has no binding — the caller produces a clear
  error: `"no op Add for type MyCustomType"`.
- Performance: this is a HashMap lookup by type ID + rune string. It must
  not do type traversal or unification at query time.

### Risk 2: `Bits`-Only Values Make Debugging Harder

**What:** `Value::Bits(Vec<u8>)` is the only representational value. A debug
printer cannot tell if `[0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x28, 0x40]`
is an Int (value 40), a Float (value 12.0), or a pointer (address 0x4028).

**Why it's risky:** When a test fails, the error message shows hex bytes.
Without type context, diagnosing "why did this addition produce the wrong
value" becomes archaeology.

**Mitigation in implementation:**
- `Value::display_typed(ty: &Type)` — displays value with type interpretation
- Error messages MUST include the type annotation, not just the raw bytes
- Assertions in tests use typed helpers: `assert_eq_int(a, 42)` not
  `assert_eq!(a, Value::Bits(vec![42, 0, 0, ...]))`
- The `PartialEq` impl for `Value` compares bytes only — this is correct
  (two values of different types but same bytes are equal as bits).

### Risk 3: VirtualHeap Maturity

**What:** The compile-time heap (`VirtualHeap`) enables List/HashMap/Box
operations in the interpreter via virtual addresses. It replaces the old
`Value::List`, `Value::HashMap` special-cased variants.

**Why it's risky:** If VirtualHeap has bugs (bounds checking, address reuse,
GC, alignment), every compile-time collection operation silently corrupts
state. A list append that silently overwrites adjacent heap memory will
produce wrong results that are nearly impossible to diagnose.

**Mitigation in implementation:**
- VirtualHeap operations MUST bounds-check every read/write
- Every `allocate` returns a unique address; `free` marks as unusable
- `debug_assert!` on every heap access in test builds
- Kani proof harness for VirtualHeap safety-critical operations
- `VirtualHeap` is a separate file with its own test suite before any
  interpreter code uses it

### Risk 4: `formatting <~` Validation at Type-Check Time

**What:** The parser produces untyped literal tokens (`Expr::Decimal(42)`,
`Expr::Quoted(bytes)`). The typechecker must validate that the target type's
`formatting` property accepts the literal's form. This pushes work from
parse time (simple) to type-check time (complex).

**Why it's risky:** Error messages can become worse. Instead of "expected
integer literal" at the parser, the user gets "type `Color` does not accept
Decimal literals" at the typechecker. The mapping from "I wrote `42`" to
"my type doesn't accept decimals" is not obvious to new users.

**Mitigation in implementation:**
- The error message MUST show the source text, the literal form, and the
  accepted forms: `"literal '42' is a Decimal, but 'Color' only accepts
  Bare identifiers. Use '@42' to force a Quoted literal, or define
  'formatting <~ Decimal' on 'Color'."`
- The `formatting` lookup is a simple property get — no traversal needed

### Risk 5: `observable <~ true` Metadata Must Prevent DCE of Side-Effecting Calls

**What:** When an intrinsic like `PrintInt#` or `Malloc#` is called, the
compiler's dead-code elimination must NOT eliminate the call if it has
observable side effects. The mechanism is the `observable` metadata
property: `observable <~ true` marks a call as not eligible for DCE.

**Why it's risky:** If the optimizer eliminates a `PrintInt#` call because
its return value is unused, the program produces no output. This is
silent corruption — no compile error, no runtime crash, just missing output.
This is the precise failure mode described in AGENTS.md: "A program that
produces no observable effect IS dead code. The compiler is correct to
eliminate it."

**Mitigation in implementation:**
- `observable` is a frontend-intrinsic metadata key (PascalCase identifier).
  The typechecker reads it and sets a flag on the definition.
- The interpreter checks `observable` before folding a call: if true,
  the call is always executed at compile time.
- The LLVM backend reads `observable` and emits `sideeffect` on the call
  (or omits `readnone`), preventing LLVM from DCE'ing it.
- The DCE pass (Phase 5) checks the `observable` flag: if true, the call
  is never eliminated.
- Every intrinsic with external side effects (`PrintInt#`, `PrintFloat#`,
  `PrintString#`, `Malloc#`, `Free#`, `Memcpy#`, `Memset#`) MUST have
  `observable <~ true` in its metadata in `bindings.dbvl`.

---

## 3. File Size Budget — Hard Limit: 3000 Lines, Target: 2000

| Category | Limit | Rationale |
|----------|-------|-----------|
| Hard maximum | 3000 lines | No file may exceed this under any circumstance |
| Target | 2000 lines | Split at natural module boundaries to stay near this |
| Reason for limit | 3000 lines | A file over 3000 lines cannot be held in working memory |
| Exception | None | The rewrite is the opportunity to get this right |

**When a file exceeds 2000 lines during writing:** stop and extract a
submodule with a clean conceptual boundary. Do NOT push to 2999 lines
and promise to split later — the split happens during writing or it
doesn't happen.

---

## 4. Flat Control Flow — The Non-Negotiable Rule

### 4.1 The Rule

No function may have more than 2 levels of indentation depth (not counting
the function definition line and the opening brace).

### 4.2 The Techniques

| Pattern | Bad (arrow code) | Good (flat) |
|---------|------------------|-------------|
| Nested optionals | `if let Some(a) = x { if let Some(b) = a.y { ... } }` | `let a = x?; let b = a.y?;` |
| Nested results | `match x { Ok(val) => { match val.f() { Ok(r) => ... } } }` | `let val = x?; let r = val.f()?;` |
| Guard clauses | `if cond { long body A } else { long body B }` | `if !cond { return B; }; long body A` |
| Match in match | `match a { X => { match b { Y => ... } } }` | Extract inner match to helper `fn handle_xy(...)` |
| Loop in match | `match a { X => { for item in list { ... } } }` | Extract loop body to helper `fn process_item(item)` |

### 4.3 When Extraction Is Required

If you are writing a function and the third level of indentation is
needed, you MUST:

1. Stop writing
2. Extract the inner logic into a named helper function
3. Write the helper function with flat code
4. Call the helper from the original function

There is no "just this once, it's simpler inline." Every level of nesting
beyond 2 is a new function.

### 4.4 Commenting Requirement

Every helper function extracted for flattening MUST have:

```rust
/// [description of what this helper does]
/// 2026-07-12: Extracted from [parent_function] to flatten nesting.
fn extracted_helper(...) -> ... {
```

---

## 5. Testing Mandate — Behavior, Not Implementation

### 5.1 Coverage Target: 100% of Functions

Every function in every rewritten file MUST have at least one test.
"100% unit test coverage" means: for every `pub fn` and every non-trivial
`fn` (more than 5 lines), there is a test that exercises it.

Exception: trivial getters, single-expression delegations, and Display
impls do not require dedicated tests (but they must be covered by
integration tests that exercise them indirectly).

### 5.2 Behavior Tests, Not Implementation Tests

**Good** (tests behavior):
```rust
#[test]
fn test_op_resolution_int_add() {
    // Given: type Int has op Add = AddI64#
    let result = resolve_op("Add", &int_type());
    assert_eq!(result, Some(OpBinding::Intrinsic("AddI64#")));
}
```

**Bad** (tests implementation):
```rust
#[test]
fn test_get_operator_intrinsic_calls_lookup() {
    // This tests HOW the function works, not WHAT it produces
    let mock = MockTypeUniverse::new();
    mock.expect_lookup().times(1).returning(...);
    let result = get_operator_intrinsic("Add", &int_type(), &mock);
    // ...
}
```

**Guidelines:**
- Test input → expected output, not internal call chains
- Use real types (construct them in the test), not mocks
- If a function needs a `TypeUniverse`, build a minimal one with only
  the types the test needs
- Each test covers one logical scenario (one "what if")
- The test name describes the scenario: `test_<fn>_<condition>`

### 5.3 Test File Placement

Tests go in:
- A `#[cfg(test)] mod tests { ... }` block at the bottom of the source file
  for unit tests
- `src/tests/` for cross-module integration tests
- `tests/` (top-level) for end-to-end compiler tests

### 5.4 Regression Test for Every Bug

When a bug is found during implementation:
1. Write a test that FAILS on the buggy code
2. Fix the code
3. The test now PASSES
4. The test stays: it is now a regression guard

### 5.5 Flat Code Tests

Every test file must also follow the flat control flow rule. Test helpers
are subject to the same constraints as production code.

---

## 6. Dependency Graph

```
                          ┌──────────────────────────────────────────────┐
                          │              Phase 0: Foundation             │
                          │  errors.rs  lexer.rs  ast/ (5 files)         │
                          │  intrinsic_signatures.rs                     │
                          └──────────────────┬───────────────────────────┘
                                             │
                                             ▼
                ┌────────────────────────────────────────────────────────┐
                │              Phase 1: Parser                           │
                │  parser/ (7 files)  layout.rs  bindings.dbvl          │
                │  **Backend interface design decisions made here**      │
                └──────────────────┬─────────────────────────────────────┘
                                   │
                                   ▼
        ┌────────────────────────────────────────────────────────────────┐
        │              Phase 2: Type System + Type Lowering              │
        │  type_universe/ (4 files)  typechecker/ (2 files)              │
        │  backend/llvm/types.rs  backend/llvm/abi.rs                   │
        └──────────────────┬─────────────────────────────────────────────┘
                           │
                           ▼
        ┌────────────────────────────────────────────────────────────────┐
        │              Phase 3: Interpreter + Intrinsics                 │
        │  interpreter/ (5 files + DELETE intrinsic_dispatch.rs)         │
        └──────────────────┬─────────────────────────────────────────────┘
                           │
                           ▼
        ┌────────────────────────────────────────────────────────────────┐
        │              Phase 4: Backend Implementation                   │
        │  LLVM (12+ files)  webstack.rs  circt.rs  bindgen.rs          │
        └──────────────────┬─────────────────────────────────────────────┘
                           │
                           ▼
        ┌────────────────────────────────────────────────────────────────┐
        │              Phase 5: Proof Engine + Analysis                  │
        │  proof_engine/ (2 files)  features/  desugarer  normalize      │
        └──────────────────┬─────────────────────────────────────────────┘
                           │
                           ▼
        ┌────────────────────────────────────────────────────────────────┐
        │              Phase 6: Derivation Module (NEW)                  │
        │  derive/ (4 files)                                            │
        └──────────────────┬─────────────────────────────────────────────┘
                           │
                           ▼
        ┌────────────────────────────────────────────────────────────────┐
        │              Phase 7: Main + CLI                               │
        │  main.rs  compile.rs  library.rs                               │
        └──────────────────┬─────────────────────────────────────────────┘
                           │
                           ▼
        ┌────────────────────────────────────────────────────────────────┐
        │              Phase 8: Archive + Tests + Stdlib                 │
        │  archive/  test updates  stdlib inop→defn  docs               │
        └────────────────────────────────────────────────────────────────┘
```

---

## 7. Phase 0 — Foundation

**Dependency: none.** These files define types and tokens that everything
else imports.

### 7.1 `src/errors.rs` — NEW, ~500 lines

**Purpose:** Single source of truth for error types used across the compiler.

**What it must contain:**
- `CompilerError` enum — top-level catch-all for compilation errors
- `SyntaxError` — parse failures (span, message)
- `TypeError` — type mismatch, missing operator, unknown type
- `AllocError` — alloc annotation validation failures
- `DeriveError` — synthesis failures
- `BackendError` — codegen failures (LLVM, CIRCT, Webstack variants)
- `LlvmError`, `CirctError`, `WebstackError` — backend-specific errors
- All errors implement `Display` + `Debug`
- Each error type has a `span: Option<Span>` field for source location
- `Into<CompilerError>` for each specific error type

**Test coverage:**
- Each error variant is constructable
- Display output includes the message (not just the variant name)
- From impls chain correctly

**Flat code requirement:**
- `Display` impls use flat `write!` per variant, no nested logic
- `From` impls are one-liners

### 7.2 `src/lexer.rs` — REWRITE, ~800 lines

**Purpose:** Tokenize Briev source text into a token stream.

**What it must contain:**
- `Token` enum with ALL token variants:
  - Keywords: `Defn`, `Txn`, `Rct`, `Cell`, `Trg`, `Term`, `Frgn`, `Let`,
    `State`, `Import`, `When`, `If`, `Else`, `Guard`, `In`, `From`, `As`
  - **NEW keywords** (from modifier/entry/derivation plans):
    - `ColonEq` (`:=`) — for derivation blocks
    - `When` — for guard statements (already a keyword, ensure it maps
       to the correct token)
    - `Input` — for `.c.bv` cell parameter declaration
    - `Output` — for `.c.bv` cell output declaration
    - `Export` — for `export defn`
  - Literals: `Decimal(i64)`, `Quoted(Vec<u8>)`, `Identifier(String)`
  - Punctuation: `LBrace`, `RBrace`, `LParen`, `RParen`, `LBracket`,
    `RBracket`, `Semicolon`, `Colon`, `Comma`, `Dot`, `DotDot`,
    `Arrow` (`->`), `Pipe` (`|`), `At` (`@`), `Hash` (`#`), `Tilde`
  - Operators: `Plus`, `Minus`, `Star`, `Slash`, `Percent`, `Ampersand`,
    `Pipe`, `Caret`, `Tilde`, `Less`, `Greater`, `Equal`, `Bang`,
    `LessEqual`, `GreaterEqual`, `EqualEqual`, `BangEqual`, `ShiftLeft`,
    `ShiftRight`, `AndAnd`, `OrOr`
  - `Eof`
- `#` is a valid identifier character (not a standalone token) —
  `foo#` lexes as `Identifier("foo#")`
- `inop` is NOT a keyword — removed entirely from the lexer
- `struct Lexer<'a>` with `pos: usize`, `source: &'a str`, `tokens: Vec<Token>`
- `fn next_token(&mut self) -> Result<Token>` — main lexing loop
- `fn peek(&self) -> Option<Token>` — lookahead
- Comments: `//` single-line, `/* */` block (nested?)
- String escape handling in Quoted literals: `\n`, `\t`, `\\`, `\"`, `\xNN`

**What is removed from the old lexer:**
- `Hash` as a standalone token — `#` is now only valid as part of identifiers
- `inop` keyword — no longer exists
- Any special-cased intrinsic parsing — removed

**Key changes from old lexer:**
```
Old: `Sqrt#` → lexes as Identifier("Sqrt"), then Hash token
New: `Sqrt#` → lexes as Identifier("Sqrt#") — single token

Old: `#` standalone → valid token
New: `#` standalone → LEX ERROR (must be part of identifier, or `[#]` uses
     the Hash+Punctuation path? Wait — `[#]` needs a `#` token.
```

**Important: `[#]` entry syntax.** The `[#]` syntax must still work:
```
[#] → LBracket, Hash, RBracket
```
This means `#` is BOTH an identifier character AND a standalone token.
The rule is:
- `#` followed by an identifier character → part of identifier
- `#` followed by `]` → standalone Hash token (for `[#]`)
- `#` anywhere else → error

Actually, let me revise. Looking at the intrinsic architecture plan more carefully:
- `#` is a valid identifier character: `Sqrt#`, `AddI64#`
- `#` is also the entry marker in `[#]`
- After `[`, `#` followed by `]` is the entry marker

The simplest approach: `#` is always a valid identifier character.
When the parser sees `[#]`, it parses `[`, then the expression `#`.
The expression `#` by itself is an identifier named `#`.
The parser's `parse_contract()` method checks for `Token::Identifier(s)` where `s == "#"`.

OR: keep `#` as a standalone token for `[#]`, but allow it in identifiers too.
The simplest: `#` in the middle of an identifier (like `Sqrt#`) makes it
part of the identifier. `#` standing alone is a hash token.

Let me just make it simple:
- Lex rule: if we see `#` and it's followed by an identifier character or
  another `#`, it's part of an identifier. If `#` appears standalone
  (followed by whitespace, `]`, or EOF), it's a Hash token.
- This handles both `Sqrt#(x)` and `[#]` correctly.

**Test coverage (behavior tests):**
- `test_lex_identifier_hash`: `Sqrt#` → `Identifier("Sqrt#")`
- `test_lex_entry_hash`: `[#]` → `LBracket`, `Hash`, `RBracket`
- `test_lex_hash_standalone_error`: `#foo` → token `Hash` then `Identifier("foo")` (hash is standalone before ident char? no — see rule above)
- Actually: `#foo` should lex as `Identifier("#foo")` if `#` at start of identifier is valid.
  But `#` as first char of an identifier is unusual. Let me check...

You know what, let me just keep it straightforward:
- `#` is always a valid identifier character (any position, including first)
- `[#]` → LBracket, Identifier("#"), RBracket
- The parser matches `Identifier("#")` in `parse_contract()`

This is the cleanest approach. No special token needed.

**Test coverage:**
- `test_lex_identifier_hash_suffix`: `AddI64#` → `Identifier("AddI64#")`
- `test_lex_identifier_hash_only`: `#` → `Identifier("#")` (for `[#]`)
- `test_lex_hash_in_identifier_middle`: `foo#bar` → `Identifier("foo#bar")`
- `test_lex_keywords_new`: `export` → `Export`, `input` → `Input`,
  `output` → `Output`
- `test_lex_inop_removed`: `inop` → `Identifier("inop")` (not a keyword)
- `test_lex_comments_and_strings`: existing comment/string tests pass

### 7.3 `src/ast/mod.rs` — NEW, ~50 lines

**Purpose:** Re-export all AST types from submodules.

```rust
pub use types::*;
pub use expr::*;
pub use top::*;
pub use display::*;
```

### 7.4 `src/ast/types.rs` — NEW, ~1500 lines

**Purpose:** Type definitions for the Briev type system.

**What it must contain:**
- `Type` enum:
  - `Bits(usize)` — arbitrary-width Bits (Axiom 1)
  - `Custom(String, Vec<Type>)` — user-defined named type with type args
  - `Ptr(Box<Type>)` — pointer to type
  - `Tuple(Vec<Type>)` — tuple type
  - `TypeVar(usize)` — type variable (for generics)
  - `Function(Vec<Type>, Box<Type>)` — function type
- `TypeKind` enum — how a type is defined:
  - `Struct(Vec<Field>)` — struct with fields
  - `Enum(Vec<Variant>)` — tagged union
  - `Codec { formatting: Formatting, parse: Option<String> }` — codec
  - `Alias(Box<Type>)` — type alias
- `Field { name: String, ty: Type, metadata: HashMap<String, PropertyValue> }`
- `Variant { name: String, fields: Vec<Field>, metadata: HashMap<String, PropertyValue> }`
- `Formatting` enum — token form acceptance:
  - `Quoted` — accepts `"..."` literals only
  - `Decimal` — accepts numeric literals only
  - `Bare` — accepts bare identifiers only
  - `Any` — accepts all three
  - `None` — no literal syntax (constructor functions only)
- `PropertyValue` enum — metadata values:
  - `Int(i64)`, `Float(f64)`, `Bool(bool)`, `String(String)`,
    `Identifier(String)`, `List(Vec<PropertyValue>)`
- `OpBinding` enum:
  - `Intrinsic(String)` — `#`-named compiler intrinsic
  - `Function(String)` — user-defined Briev function
- `Constraint` — type constraint expression (for `[pre][post]`):
  - `Eq(Expr, Expr)`, `Neq`, `Lt`, `Gt`, `Le`, `Ge`, `And`, `Or`, `Not`,
    `Implies`, `Forall`, `Exists`
  - (This is the constraint language for SMT — may overlap with Expr but
    is kept separate to avoid confusion)

**Test coverage:**
- Each enum variant is constructable and matchable
- `Formatting` parse from string + display round-trip
- `PropertyValue` type-checking (string is valid, int is valid, etc.)

### 7.5 `src/ast/expr.rs` — NEW, ~1500 lines

**Purpose:** Expression AST node types.

**What it must contain:**
- `Expr` enum — ALL expression variants:
  - `Quoted(Vec<u8>)` — raw quoted bytes (was `Expr::String`)
  - `Decimal(i64)` — numeric literal (was `Expr::Integer`)
  - `Bool(bool)` — boolean literal
  - `Float(f64)` — floating-point literal
  - `Identifier(String)` — bareword reference
  - `Call(String, Vec<Expr>)` — function/intrinsic call (including `#`)
  - `BinaryOp(BinaryOpKind, Box<Expr>, Box<Expr>)` — binary operator
  - `UnaryOp(UnaryOpKind, Box<Expr>)` — unary operator
  - `Field(Box<Expr>, String)` — field access
  - `Index(Box<Expr>, Box<Expr>)` — index expression
  - `Block(Vec<Statement>)` — block expression
  - `If(Box<Expr>, Box<Expr>, Option<Box<Expr>>)` — if/else
  - `Match(Box<Expr>, Vec<MatchArm>)` — pattern match
  - `Tuple(Vec<Expr>)` — tuple literal
  - `List(Vec<Expr>)` — list literal
  - `Lambda(Vec<String>, Box<Expr>)` — anonymous function
  - `Within(Box<Expr>, Box<Expr>)` — scope/within expression
  - `Cast(Box<Expr>, Type)` — type cast
  - `IsType(Box<Expr>, Type)` — type check
  - `PropertyGet(String)` — property access on metadata
  - `FormattingAnnotation(Formatting)` — `formatting <~` value
  - `DerivationBlock { ... }` — derivation block (see below)
- `BinaryOpKind` enum:
  `Add`, `Sub`, `Mul`, `Div`, `Mod`, `Eq`, `Neq`, `Lt`, `Gt`, `Le`, `Ge`,
  `And`, `Or`, `BitAnd`, `BitOr`, `BitXor`, `Shl`, `Shr`, `Concat`
- `UnaryOpKind` enum:
  `Neg`, `Not`, `BitNot`
- `MatchArm { pattern: Pattern, guard: Option<Expr>, body: Expr }`
- `Pattern` enum:
  `Wildcard`, `Literal(Expr)`, `Binding(String)`, `EnumVariant(String, Vec<Pattern>)`,
  `Tuple(Vec<Pattern>)`, `Range(Expr, Expr)`
- `DerivationBlock` struct:
  - `examples: Vec<DerivationExample>`
  - `synthesized: Option<Expr>` — the synthesized expression (filled by Phase 6)
- `DerivationExample` struct:
  - `inputs: Vec<(String, Value)>` — named input values
  - `output: Value` — expected output

**NOT in this file (removed from the old AST):**
- `Expr::IntrinsicCall` — deleted. `Sqrt#(x)` is `Expr::Call("Sqrt#", [x])`.
- `Expr::String` — renamed to `Quoted`
- `Expr::Integer` — renamed to `Decimal`
- Any `Intrinsic` enum references

**Testing:**
- Each Expr variant is constructable
- Expr equality works as expected
- DerivationBlock round-trips through serialization

### 7.6 `src/ast/top.rs` — NEW, ~1200 lines

**Purpose:** Top-level declaration AST nodes.

**What it must contain:**
- `TopLevel` enum:
  - `Definition(Definition)` — `defn` or `export defn`
  - `Transaction(Transaction)` — `txn` or `node`
  - `Cell(CellDef)` — `cell` declaration
  - `Import(Import)` — import statement
  - `Export(Export)` — `export defn` wrapper
  - `Meld(Meld)` — meld declaration
  - `Trigger(Trigger)` — `trg` binding
- `Definition` struct:
  - `name: String`
  - `type_params: Vec<String>`
  - `parameters: Vec<(String, Type)>`
  - `output_type: Option<Type>`
  - `contract: Contract`
  - `body: Block` — function body
  - `metadata: HashMap<String, PropertyValue>` — `<~` annotations
  - `derivation: Option<DerivationBlock>` — `:=` block (NEW)
  - `span: Option<Span>`
- `Transaction` struct:
  - `name: String`
  - `type_params: Vec<String>`
  - `parameters: Vec<(String, Type)>`
  - `contract: Contract`
  - `is_reactive: bool`
  - `is_async: bool`
  - `body: Vec<Statement>`
  - `metadata: HashMap<String, PropertyValue>`
  - `derivation: Option<DerivationBlock>` (NEW)
  - `span: Option<Span>`
- `Contract` struct:
  - `pre_condition: Expr`
  - `post_condition: Expr`
  - `is_entry: bool` — `[#]` marker (NEW)
  - `watchdog: Option<Watchdog>`
  - `span: Option<Span>`
- `Export` struct:
  - `inner: Box<TopLevel>` — the exported definition
  - `export_name: Option<String>` — optional external name
- `CellDef` struct:
  - `name: String`
  - `type_params: Vec<String>`
  - `parameters: Vec<(String, Type)>` — cell parameters (includes `input`)
  - `output_type: Option<OutputType>` — cell output (from `output`)
  - `fields: Vec<Field>` — state fields
  - `transactions: Vec<Transaction>` — cell transactions
  - `definitions: Vec<Definition>` — cell definitions
  - `internal_triggers: Vec<Trigger>` — cell triggers
  - `is_persistent: bool`
  - `metadata: HashMap<String, PropertyValue>`
  - `span: Option<Span>`
- `OutputType` enum:
  - `Single(Type)` — unnamed output
  - `Named(String, Box<OutputType>)` — named output (for `.c.bv`)
- `Statement` enum:
  - `Let(String, Option<Type>, Expr)` — `let x: T = expr;`
  - `Assign(Expr, Expr)` — `dest = expr;`
  - `Expr(Expr)` — expression statement
  - `Term(Option<Expr>)` — `term` or `term expr;`
  - `Return(Option<Expr>)` — `return expr;`
  - `Guarded(Expr, Box<Statement>)` — `[when expr] { body }` (NEW)
  - `If(Expr, Vec<Statement>, Vec<Statement>)`
  - `Block(Vec<Statement>)`
  - `MetadataAssignment(String, PropertyValue)` — `key <~ value;`
  - `Guard(Expr)` — `[expr]` contract guard
- `Import` struct: `path: String`, `symbols: Vec<String>`, source info
- `Meld` struct: meld declaration data
- `Trigger` struct: trigger binding data
- `Watchdog` struct: timing watchdog configuration
- `Span` struct: `start: usize`, `end: usize`, `file: Option<String>`

**NOT in this file (removed):**
- `TopLevel::Inop` — deleted entirely
- `Intrinsic` enum — deleted entirely
- `InopDeclaration` — deleted entirely

**Test coverage:**
- Each TopLevel variant is constructable
- Contract with `is_entry = true` round-trips correctly
- Export wrapping a Definition round-trips
- Guarded statement constructs correctly

### 7.7 `src/ast/display.rs` — NEW, ~500 lines

**Purpose:** Display implementations for all AST types.

**What it must contain:**
- `Display` for `Expr` — produces valid Briev source text
- `Display` for `Type` — produces valid Briev type syntax
- `Display` for `Statement` — produces valid Briev statement syntax
- `Display` for `TopLevel` — produces valid Briev top-level syntax
- `Display` for `Contract` — produces `[pre][post]`, `[#]`, `[[post]`, etc.
- `Display` for `Pattern`, `MatchArm`, `BinaryOpKind`, `UnaryOpKind`
- `Display` for `DerivationBlock` — produces `:= { ... }` syntax
- `Display` for all error types in `errors.rs`

**Test coverage:**
- Display round-trip: parse → display → parse → same AST
- Each variant produces syntactically valid output
- Edge cases: empty blocks, no contract, derivation blocks

### 7.8 `src/intrinsic_signatures.rs` — NEW, ~400 lines

**Purpose:** Central registry of ALL `#` intrinsic function signatures.

**What it must contain:**
- `fn get_intrinsic_signature(name: &str) -> Option<Signature>`
- `Signature` struct:
  - `name: String`
  - `parameters: Vec<(String, Type)>`
  - `return_type: Option<Type>`
- One flat match arm per intrinsic name, alphabetically sorted:

```rust
pub fn get_intrinsic_signature(name: &str) -> Option<Signature> {
    match name {
        "AddI64#" => Some(Signature {
            name: "AddI64#".into(),
            parameters: vec![("a".into(), Type::int()), ("b".into(), Type::int())],
            return_type: Some(Type::int()),
        }),
        "FAddF64#" => Some(Signature {
            name: "FAddF64#".into(),
            parameters: vec![("a".into(), Type::float()), ("b".into(), Type::float())],
            return_type: Some(Type::float()),
        }),
        // ... every intrinsic, alphabetically sorted ...
        _ => None,
    }
}
```

**Why this works:** The match on string name replaces the old `Intrinsic` enum.
Adding a new intrinsic is one new arm. No enum update, no match exhaustiveness
check to update. The `_ => None` fallthrough is unchanged.

**Complete list of intrinsics (from all plans):**

| Intrinsic | Parameters | Returns | Purpose |
|-----------|-----------|---------|---------|
| `AddI64#` | a: Int, b: Int | Int | Integer addition |
| `SubI64#` | a: Int, b: Int | Int | Integer subtraction |
| `MulI64#` | a: Int, b: Int | Int | Integer multiplication |
| `DivI64#` | a: Int, b: Int | Int | Integer division |
| `RemI64#` | a: Int, b: Int | Int | Integer remainder |
| `EqI64#` | a: Int, b: Int | Bool | Integer equality |
| `LtI64#` | a: Int, b: Int | Bool | Integer less-than |
| `FAddF64#` | a: Float, b: Float | Float | Float addition |
| `FSubF64#` | a: Float, b: Float | Float | Float subtraction |
| `FMulF64#` | a: Float, b: Float | Float | Float multiplication |
| `FDivF64#` | a: Float, b: Float | Float | Float division |
| `FEqF64#` | a: Float, b: Float | Bool | Float equality |
| `FLtF64#` | a: Float, b: Float | Bool | Float less-than |
| `EqI1#` | a: Bool, b: Bool | Bool | Bool equality |
| `EqI32#` | a: Char, b: Char | Bool | Char equality |
| `Sqrt#` | x: Float | Float | Square root |
| `Malloc#` | size: Int | Ptr<Byte> | Heap allocation |
| `Free#` | ptr: Ptr<Byte> | Void | Heap deallocation |
| `PrintInt#` | n: Int | Void | Print integer to stdout |
| `PrintFloat#` | f: Float | Void | Print float to stdout |
| `PrintString#` | s: String | Void | Print string to stdout |
| `GetEnvInt#` | name: String | Int | Get environment variable as int |
| `GetEnvString#` | name: String | String | Get environment variable as string |
| `GetGlobalId#` | dim: Int | Int | GPU work-item ID |
| `GetGlobalSize#` | dim: Int | Int | GPU work-group size |
| `GetLocalId#` | dim: Int | Int | GPU local ID |
| `Memcpy#` | dst: Ptr<Byte>, src: Ptr<Byte>, n: Int | Void | Memory copy |
| `Memset#` | ptr: Ptr<Byte>, val: Int, n: Int | Void | Memory set |
| `ListInsert#` | list: Ptr<List>, index: Int, val: Bits | Void | List insertion |
| `ListGet#` | list: Ptr<List>, index: Int | Bits | List element access |
| `ListAppend#` | list: Ptr<List>, val: Bits | Void | List append |
| `StringConcat#` | a: String, b: String | String | String concatenation |
| `StringLength#` | s: String | Int | String length |
| `StringEq#` | a: String, b: String | Bool | String equality |
| `FloatToInt#` | f: Float | Int | Float-to-int conversion |
| `IntToFloat#` | n: Int | Float | Int-to-float conversion |
| `IntToString#` | n: Int | String | Int-to-string conversion |
| `FloatToString#` | f: Float | String | Float-to-string conversion |
| `CharToInt#` | c: Char | Int | Char-to-int conversion |
| `IntToChar#` | n: Int | Char | Int-to-char conversion |

(This list covers all intrinsics from the existing codebase, all plans,
and the GPU/intrinsic architecture documents.)

**Test coverage:**
- `test_signature_exists` for every intrinsic — no missing arms
- `test_signature_nonexistent` — unknown name returns None
- `test_signature_parameter_count` — each signature has correct arity

---

## 8. Phase 1 — Parser + Backend Interface Design

**Dependency:** Phase 0 (lexer, AST types, error types, intrinsic signatures).

### 8.1 Backend Interface Design Decision (Documented Here, Implemented in Phase 4)

Before writing the parser, we MUST decide how the backend will consume parser
output. This ensures the parser produces the right data structures.

**Decision: Backend consumes Definition metadata HashMap.**

The path is:
1. Parser produces `Definition { name, params, return_type, contract, body,
   metadata: HashMap<String, PropertyValue> }`
2. Parser validates: metadata keys are valid identifiers, values are valid
   property types (Int, Float, Bool, String, Identifier, List)
3. Parser does NOT validate: key names, string contents, target relevance
4. Typechecker adds `is_entry: true` flag for `[#]` contracts, validates
   alloc annotations, validates intrinsic signatures
5. Backend reads metadata keys matching its prefix (`llvm_*`, `circt_*`,
   `wasm_*`, `interpreter_*`)
6. Unknown keys are silently ignored by all backends
7. Known key + unsupported value → backend error

**What this means for the parser:**
- `key <~ value;` is parsed as `Statement::MetadataAssignment(key, value)`.
- `key <~` values can be: identifiers (`Quoted`, `Decimal`), strings
  (`"i64"`, `"add nsw i64"`), integers (`42`, `0x4000_2000`), bools
  (`true`, `false`), or lists (`["Arena", ptr]`).
- The parser does NOT interpret any of these — it just stores them.

**What this means for Phase 2 (typechecker):**
- The typechecker reads specific metadata keys it understands:
  `formatting`, `bytes`, `op X`, `alloc`
- It validates and expands what it can
- All other keys are passed through unchanged

### 8.2 `src/parser/mod.rs` — NEW, ~500 lines

**Purpose:** Parse pipeline entry point, delegate to sub-parsers.

```rust
pub struct Parser<'a> {
    tokens: Vec<Token>,
    pos: usize,
    source: &'a str,
}

impl Parser<'_> {
    pub fn new(tokens: Vec<Token>, source: &str) -> Parser;
    pub fn parse_program(&mut self) -> Result<Program, SyntaxError>;
    // Delegate methods:
    fn parse_top_level(&mut self) -> Result<TopLevel, SyntaxError>;
    fn parse_definition_or_transaction(&mut self) -> Result<TopLevel, SyntaxError>;
}
```

### 8.3 `src/parser/expressions.rs` — NEW, ~2500 lines

**Purpose:** Parse ALL expression forms.

**Functions:**
- `parse_expression(&mut self) -> Result<Expr, SyntaxError>` — entry point
- `parse_primary()` — literals, identifiers, parenthesized, `@` prefix
- `parse_call_suffix()` — `f(x, y)` after identifier
- `parse_field_suffix()` — `.field` after expression
- `parse_index_suffix()` — `[i]` after expression
- `parse_binary_op()` — operator precedence climbing
- `parse_unary_op()` — prefix operators
- `parse_if_expression()` — `if cond { ... } else { ... }`
- `parse_match_expression()` — `match expr { ... }`
- `parse_lambda()` — `params => body`
- `parse_tuple_or_parens()` — `(a, b)` vs `(a)`
- `parse_list_literal()` — `[a, b, c]`
- `parse_within()` — `expr within { ... }`
- `parse_cast()` — `expr as Type`
- `parse_is_type()` — `expr is Type`
- `parse_derivation_block()` — `:= { examples }` (NEW)

**Key design:**
- `@` prefix: `@expr` converts any expression to `Expr::Quoted(raw_text)`.
  The parser captures the raw source bytes of `expr` and wraps them.
- Operator precedence: standard C-like precedence table.
- No special handling for `#` in expressions — `Sqrt#(x)` parses as
  `Identifier("Sqrt#")` then `Call("Sqrt#", [x])` via call suffix.

**Test coverage:**
- Every expression form parses correctly
- Operator precedence: `a + b * c` → `+(a, *(b, c))`
- `@` prefix: `@42` → `Quoted("42".as_bytes())`
- Call with `#`: `Sqrt#(x)` → `Call("Sqrt#", [Identifier("x")])`
- Derivation block: `:= { 1, 2 -> 3; 4, 5 -> 9 }` → DerivationBlock with
  two examples
- Error recovery: malformed expressions produce span-accurate errors

### 8.4 `src/parser/definitions.rs` — NEW, ~2000 lines

**Purpose:** Parse top-level declarations.

**Functions:**
- `parse_definition(&mut self) -> Result<Definition, SyntaxError>`
  - `defn name<T>(params) -> RetType [pre][post] { body }`
  - `export defn ...` — calls `parse_definition()` then wraps in `Export`
  - Handles `is_entry: true` for `[#]` contract
  - Handles derivation block `:= { ... }` after body
- `parse_transaction(&mut self) -> Result<Transaction, SyntaxError>`
  - `txn name<T>(params) [pre][post] { body }`
  - `node name [pre][post] { body }`
  - Handles `is_entry: true` for `[#]` contract
- `parse_cell(&mut self) -> Result<CellDef, SyntaxError>`
  - `cell name<T>(params) -> RetType { fields txn ... }`
- `parse_contract(&mut self) -> Result<Contract, SyntaxError>`:
  - `[#]` → `is_entry: true`
  - `[pre][post]` → standard contract
  - `[[post]` → `pre: true`, post as given
  - `[pre]]` → `[pre][true]`
  - `[#][post]` → `is_entry: true`, post as given
  - `[true][true]` → REJECTED (parser error: "useless contract")
- `parse_import(&mut self) -> Result<Import, SyntaxError>`
- `parse_meld(&mut self) -> Result<Meld, SyntaxError>`
- `parse_trigger(&mut self) -> Result<Trigger, SyntaxError>`
- `fn wrap_implicit_entry(program: &mut Program)` — when file has no explicit
  declarations, wraps top-level statements in implicit `txn` with `[#]`
  (Phase 16E). Rejects if both explicit declarations and top-level statements exist.

**Test coverage:**
- `test_parse_entry_only`: `defn main() -> Int [#] { term 0; };` → entry=true
- `test_parse_entry_with_post`: `defn f() -> Int [#] [result == 0]`
- `test_parse_entry_on_txn`: `txn run() [#] [bal' == bal - amt]`
- `test_parse_export_defn`: `export defn add(a: Int, b: Int) -> Int`
- `test_parse_derivation_block`: `defn add(a: Int, b: Int) -> Int := { 1, 2 -> 3 }`
- `test_parse_cell_input_output`: `.c.bv` with `input`/`output` keywords
- `test_parse_implicit_entry`: File with only top-level statements → implicit txn
- `test_parse_implicit_and_explicit_conflict`: Both explicit defn and top-level
  statements → error
- `test_parse_entry_rejects_pre`: `[#][x > 0]` → error (no pre with `[#]`)
- `test_parse_useless_contract_rejected`: `[true][true]` → error

### 8.5 `src/parser/statements.rs` — NEW, ~1500 lines

**Purpose:** Parse statement-level constructs.

**Functions:**
- `parse_statement(&mut self) -> Result<Statement, SyntaxError>`
- `parse_let_statement()` — `let x: T = expr;`
- `parse_assignment()` — `dest = expr;`
- `parse_term()` — `term;` or `term expr;`
- `parse_return()` — `return expr;`
- `parse_guard_statement()` — `[when expr] { body }` (NEW, Phase 16C-ish)
  Actually: `[expr] { body }` is a guarded statement (the `[` is the guard,
  the `{` is the body). The `when` keyword is for the derivation plan's
  `when` guard statement.
- `parse_if_statement()` — `if expr { ... } else { ... }`
- `parse_block()` — `{ stmt; stmt; ... }`
- `parse_metadata_assignment()` — `key <~ value;`

### 8.6 `src/parser/types.rs` — NEW, ~1000 lines

**Purpose:** Parse type annotations.

**Functions:**
- `parse_type(&mut self) -> Result<Type, SyntaxError>`
- `parse_type_params()` — `<T, U>` or nothing
- `parse_codec_declaration()` — `codec Name { formatting <~ X; parse <~ Y; }`
- `parse_projection_type()` — `list :> Element` (subtype projection)

### 8.7 `src/parser/metadata.rs` — NEW, ~800 lines

**Purpose:** Parse metadata/value annotations inside function bodies.

**Functions:**
- `parse_body_metadata(&mut self) -> Result<HashMap<String, PropertyValue>, SyntaxError>`
- Recognizes: `key <~ value;` where value is one of:
  - identifier: `formatting <~ Quoted`
  - string: `llvm <~ "i64"`
  - integer: `bytes <~ 8`
  - bool: `volatile <~ true`
  - list: `alloc <~ "Arena", local_pool`

### 8.8 `src/parser/helpers.rs` — NEW, ~1000 lines

**Purpose:** Shared parser utilities.

**Functions:**
- `expect(&mut self, token: TokenKind) -> Result<(), SyntaxError>`
- `expect_identifier(&mut self) -> Result<String, SyntaxError>`
- `peek(&self) -> Option<&Token>`
- `advance(&mut self) -> Token`
- `check(&self, kind: TokenKind) -> bool`
- `span(&self) -> Span`
- `error_at_current(&self, msg: &str) -> SyntaxError`
- `error_at_previous(&self, msg: &str) -> SyntaxError`
- `synchronize(&mut self)` — error recovery (skip to next statement boundary)

### 8.9 `src/layout.rs` — NEW, ~400 lines

**Purpose:** Indentation-to-brace pre-processor for `.f.bv` files.

**Activated by:** `CompilationJob.layout_parser == true` (from `.f` modifier).

**Algorithm:**
1. Split source into lines
2. For each line, count leading whitespace
3. Track indentation stack (start with level 0)
4. Indent increase → emit `{`
5. Indent decrease → emit `}` for each level dropped
6. At end of each line → emit `;` (unless line is a block opener)
7. Detect mixed tabs and spaces → error

```rust
pub struct LayoutPreprocessor;

impl LayoutPreprocessor {
    pub fn process(source: &str) -> Result<String, LayoutError> {
        // Flat: process each line once, track stack
    }
}
```

**Test coverage:**
- Basic function indentation
- Nested blocks (if inside function)
- Empty lines and comments preserved
- Mixed indentation → error

### 8.10 `src/backend/llvm/bindings.dbvl` — NEW, ~100 lines

**Purpose:** Template mappings from intrinsic names to LLVM IR snippets.

**Format:**
```dbvl
# Intrinsic bindings for the LLVM backend
# Each entry: intrinsic_name -> LLVM IR template
# %0, %1, ... refer to arguments

AddI64#    -> add nsw i64 %0, %1
SubI64#    -> sub nsw i64 %0, %1
MulI64#    -> mul nsw i64 %0, %1
FAddF64#   -> fadd double %0, %1
Sqrt#      -> call double @llvm.sqrt.f64(double %0)
PrintInt#  -> call i32 (ptr, ...) @printf(ptr @format_int, i64 %0)
PrintString# -> call i32 (ptr, ...) @printf(ptr @format_str, ptr %0)
GetGlobalId# -> call i32 @__get_global_id(i32 %0)
```

**Why a separate file:** Backend engineers can add new intrinsic mappings
without touching Rust code. The `.dbvl` file is parsed at compile time and
its entries are merged with the Rust-side intrinsic signature registry.

---

## 9. Phase 2 — Type System + Type Lowering

**Dependency:** Phase 0 (AST types, intrinsic signatures), Phase 1 (parser produces
typed `Definition`/`Transaction` nodes).

### 9.1 `src/type_universe/mod.rs` — NEW, ~500 lines

**Purpose:** The `TypeUniverse` — central type definition registry.

```rust
pub struct TypeUniverse {
    /// Registered type definitions, keyed by fully-qualified name.
    types: HashMap<String, TypeDef>,
    /// Known formatting bindings: type name -> Formatting
    formatting_cache: HashMap<String, Formatting>,
}

impl TypeUniverse {
    pub fn new() -> Self;
    pub fn get(&self, name: &str) -> Option<&TypeDef>;
    pub fn register(&mut self, def: TypeDef) -> Result<(), TypeError>;
    pub fn get_formatting(&self, ty: &Type) -> Formatting;
    pub fn resolve_type_def(&self, ty: &Type) -> Result<&TypeDef, TypeError>;
    pub fn contains(&self, name: &str) -> bool;
}
```

### 9.2 `src/type_universe/resolve.rs` — NEW, ~800 lines

**Purpose:** Type resolution: `Custom("List", [Int])` → resolved struct with
field types.

- `fn resolve_type_def(universe: &TypeUniverse, ty: &Type) -> Result<&TypeDef, TypeError>`
- `fn apply_binding(type_params: &[String], args: &[Type], target: &Type) -> Type`
- `fn instantiate_generic(def: &TypeDef, args: &[Type]) -> TypeDef`

**Test coverage:**
- Resolve `Int` → `TypeDef { name: "Int", bytes: 8, ... }`
- Resolve `List<Int>` → struct with `ptr: Ptr<Int>`, `len: Int`, `cap: Int`
- Resolve unknown type → error
- Generic substitution: `T → Int` in `List<T>` → `List<Int>`

### 9.3 `src/type_universe/validate.rs` — NEW, ~500 lines

**Purpose:** Validate type definitions for internal consistency.

- `fn validate_primitives(universe: &TypeUniverse) -> Result<(), Vec<TypeError>>`
- Checks: all referenced types exist, byte widths are consistent, no circular
  inheritance, operator bindings resolve to known intrinsics or functions
- `fn validate_bytes(type_def: &TypeDef) -> Result<(), TypeError>`
- `fn validate_no_cycles(universe: &TypeUniverse, name: &str, visited: &mut HashSet<String>) -> Result<(), TypeError>`

### 9.4 `src/type_universe/operators.rs` — NEW, ~800 lines

**Purpose:** The CRITICAL `op` resolution function (Risk #1).

- `fn get_operator_intrinsic(universe: &TypeUniverse, rune: &str, ty: &Type) -> Option<OpBinding>`
  - Maps rune to op name: `"+"` → `"Add"`, `"=="` → `"Eq"`, `"[]"` → `"ExtractFrom"`
  - Looks up type's metadata for `op <name>`
  - Returns `OpBinding::Intrinsic("AddI64#")` or `OpBinding::Function("my_add")`
  - Returns `None` if no binding found (caller produces error)
- `fn get_formatting_type(universe: &TypeUniverse, ty: &Type) -> Formatting`
  - Looks up `formatting` property on the type's codec
  - Returns `Formatting::None` as default (no literal syntax)

**Test coverage — THE MOST IMPORTANT TESTS IN THE COMPILER:**
- Matrix test: every built-in type × every valid operator → correct OpBinding
- Missing operator: `Int + String` → None (Int has `op Add`, String doesn't)
- User-defined operator: `MyType + MyType` → `OpBinding::Function("my_add")`
- `Formatting::Quoted` on `String` → accepts `"..."` literals
- `Formatting::Bare` on enum → accepts bare identifiers
- `Formatting::None` → any literal assignment is an error

### 9.5 `src/typechecker/mod.rs` — REWRITE, ~2000 lines

**Purpose:** Type-check programs after parsing.

**Functions:**
- `fn infer_expression(expr: &Expr, ctx: &TypecheckContext) -> Result<Type, TypeError>`
  - Flat dispatch on `Expr` variant
  - For `Expr::Call(name, args)` where `name.ends_with('#'`)`: look up
    `get_intrinsic_signature(name)`, verify argument types match parameter types,
    return the declared return type
  - For `Expr::Call(name, args)` without `#`: look up user definition, check args
  - For binary/unary ops: resolve `op` via `get_operator_intrinsic()`, check args
  - For literals: check `formatting` compatibility via `get_formatting_type()`
- `fn infer_statement(stmt: &Statement, ctx: &TypecheckContext) -> Result<(), TypeError>`
- `fn infer_block(block: &[Statement], ctx: &TypecheckContext) -> Result<Type, TypeError>`
- `fn check_program(program: &Program, universe: &TypeUniverse) -> Result<TypecheckResult, Vec<TypeError>>`

**Test coverage:**
- `test_infer_int_add`: `1 + 2` → `Int`
- `test_infer_float_add`: `1.0 + 2.0` → `Float`
- `test_infer_intrinsic_call`: `Sqrt#(4.0)` → `Float`
- `test_infer_intrinsic_wrong_args`: `Sqrt#("hello")` → type error
- `test_infer_missing_operator`: `true + 1` → "no op Add for type Bool"
- `test_infer_formatting_quoted`: `let s: String = "hello"` → OK
- `test_infer_formatting_mismatch`: `let x: Int = "hello"` → error
- `test_infer_formatting_bare`: `let c: Color = Red` → OK (Color accepts Bare)

### 9.6 `src/typechecker/validate.rs` — NEW, ~1500 lines

**Purpose:** Post-type-checking validation passes (from alloc, derivation,
entry-point plans).

**Functions:**
- `fn validate_alloc_annotations(program: &mut Program) -> Result<(), Vec<AllocError>>`
  - Iterates all bindings with `alloc` metadata
  - `alloc("Stack")`: verifies escape analysis proves no-escape, sets `alloca` metadata
  - `alloc(0x4000_2000)`: verifies address is compile-time constant, expands to
    `volatile`, `observable`, `fixed_addr` metadata
  - Unknown values: pass through to backend
  - **Design:** `expand_alloc()` is extracted as a named helper with flat dispatch
- `fn check_entry_call_graph(program: &Program) -> Result<(), Vec<TypeError>>`
  - Build call graph
  - If any function calls a `[#]`-marked function: error
  - `[#]` functions can call each other (all CLI-addressable)
- `fn check_derivation(program: &Program) -> Result<(), Vec<TypeError>>`
  - Validates that derivation example types match function signature
  - Inputs match parameter types, output matches return type
- `fn check_implicit_entry_conflict(program: &Program) -> Result<(), Vec<TypeError>>`
  - If file has both explicit `[#]` functions and implicit scripting statements: error

**Test coverage:**
- `test_alloc_stack_no_escape`: variable stays in scope → passes
- `test_alloc_stack_does_escape`: variable returned → error
- `test_alloc_physical_constant`: `alloc(0x4000_2000)` → expands to volatile+observable
- `test_alloc_unknown_value`: `alloc("CustomRegion")` → pass through, no error
- `test_entry_call_graph_violation`: internal code calls `[#]` fn → error
- `test_entry_call_graph_isolated`: no internal calls → ok
- `test_derivation_example_types_ok`: example inputs/output match signature
- `test_derivation_example_types_wrong`: example output doesn't match → error
- `test_implicit_entry_with_explicit`: file has defn AND top-level stmts → error

### 9.7 `src/backend/llvm/types.rs` — NEW, ~1000 lines

**Purpose:** Lower Briev types to LLVM types.

**Functions:**
- `fn lower_type(ty: &Type, universe: &TypeUniverse) -> String`
  - Checks `llvm` property: `Int` → `"i64"`, `Float` → `"double"`, etc.
  - Falls back to `iN` where `N = bytes * 8` if no `llvm` property
  - Structs: `{ i64, double, i8 }` from field types
  - Enums: `{ i8, { ... } }` (discriminant + variant data)
  - `Ptr<T>` → `ptr` (opaque pointer)
- `fn type_size(ty: &Type, universe: &TypeUniverse) -> u64`
  - Byte width of the type in LLVM layout
- `fn type_alignment(ty: &Type, universe: &TypeUniverse) -> u64`

**Test coverage:**
- Int → `"i64"`
- Float → `"double"`
- Struct with 3 fields → `"{ i64, double, i8 }"`
- Unknown type without `llvm` property → `"i24"` (for 3 bytes)

### 9.8 `src/backend/llvm/abi.rs` — NEW, ~1000 lines

**Purpose:** ABI marshaling between C calling convention and Briev internal types.

**Functions:**
- `fn marshal_param_to_briev(param_ty: &Type, param_reg: &str) -> String`
  - Bool: `trunc i8 %param to i1`
  - Others: identity (pass through)
- `fn marshal_briev_to_return(ret_ty: &Type, ret_reg: &str) -> String`
  - Bool: `zext i1 %ret to i8`
  - String: `getelementptr %String, ptr %ret, i32 0, i32 0` → `load i64` → `inttoptr`
  - Others: identity
- `fn marshal_export_wrapper(defn: &Definition, export_name: &str) -> String`
  - Generates the LLVM IR for the export wrapper function:
    - Trunc parameters (Bool i8 → i1)
    - Call inner function
    - Zext return (Bool i1 → i8)
    - Return

**Test coverage:**
- Bool parameter: `trunc i8 %b to i1`
- Bool return: `zext i1 %result to i8`
- String return: extracts `.ptr` field as `i8*`
- Int passthrough: no marshaling needed

---

## 10. Phase 3 — Interpreter + Intrinsics

**Dependency:** Phase 0 (AST types, Value enum), Phase 2 (type universe).

### 10.1 `src/interpreter/mod.rs` — REWRITE, ~800 lines

**Purpose:** `Value` enum, `VirtualHeap`, basic value utilities, re-exports.

```rust
pub enum Value {
    /// The ONLY representational storage cell for program data.
    Bits(Vec<u8>),

    // Compiler-internal meta-objects (never reach user code):
    Defn(String),
    Void,
    Ref(Box<Value>),
    Expr(Box<Expr>),
    Stmt(Box<Statement>),
    Block(Vec<Statement>),
    Items(Vec<TopLevel>),
    Type(Type),
    Regex(RegexPattern),
    DbvlTable(Arc<DbvlTableInner>),
}
```

**Key helpers:**
- `fn bits_to_i64(val: &Value) -> Option<i64>` — first 8 bytes as little-endian i64
- `fn bits_to_f64(val: &Value) -> Option<f64>` — first 8 bytes as f64
- `fn bits_to_bool(val: &Value) -> Option<bool>` — first byte as bool
- `fn bits_is_true(val: &Value) -> bool` — any non-zero byte
- `fn i64_to_bits(n: i64) -> Vec<u8>` — little-endian encoding
- `fn f64_to_bits(f: f64) -> Vec<u8>` — IEEE 754 encoding
- `fn bits_zero(size: usize) -> Vec<u8>` — zero-filled byte array

**`VirtualHeap`:**
```rust
pub struct VirtualHeap {
    allocations: HashMap<u64, Vec<u8>>,
    next_address: u64,
    freed: HashSet<u64>,
}

impl VirtualHeap {
    pub fn allocate(&mut self, size: usize) -> u64;
    pub fn read(&self, addr: u64, size: usize) -> Option<&[u8]>;
    pub fn write(&mut self, addr: u64, data: &[u8]) -> Result<(), HeapError>;
    pub fn free(&mut self, addr: u64) -> Result<(), HeapError>;
    pub fn contains(&self, addr: u64) -> bool;
}
```

**Test coverage (critical — this is Risk #3):**
- Allocate then read → correct data
- Allocate bounds check: write beyond allocation → error
- Double free → error
- Free then read → error (freed address)
- Address collision: sequential allocations give different addresses
- Kani proof harness for bounds checking

### 10.2 `src/interpreter/eval.rs` — REWRITE, ~2000 lines

**Purpose:** Expression evaluation — compile-time interpretation.

```rust
pub struct EvalContext<'a> {
    pub universe: &'a TypeUniverse,
    pub heap: &'a mut VirtualHeap,
    pub bindings: HashMap<String, Value>,
}

pub fn eval_expr(expr: &Expr, ctx: &mut EvalContext) -> Result<Value, RuntimeError>;
```

**Dispatch structure — flat, one arm per Expr variant:**
```rust
pub fn eval_expr(expr: &Expr, ctx: &mut EvalContext) -> Result<Value, RuntimeError> {
    match expr {
        Expr::Quoted(bytes) => Ok(Value::Bits(bytes.clone())),
        Expr::Decimal(n) => Ok(Value::Bits(i64_to_bits(*n))),
        Expr::Bool(b) => Ok(Value::Bits(vec![if *b { 1 } else { 0 }])),
        Expr::Float(f) => Ok(Value::Bits(f64_to_bits(*f))),
        Expr::Identifier(name) => eval_identifier(name, ctx),
        Expr::Call(name, args) => eval_call(name, args, ctx),
        Expr::BinaryOp(kind, lhs, rhs) => eval_binary_op(kind, lhs, rhs, ctx),
        Expr::UnaryOp(kind, expr) => eval_unary_op(kind, expr, ctx),
        Expr::Field(obj, name) => eval_field_access(obj, name, ctx),
        Expr::Index(obj, index) => eval_index(obj, index, ctx),
        Expr::Block(stmts) => eval_block(stmts, ctx),
        Expr::If(cond, then, else_) => eval_if(cond, then, else_, ctx),
        // ... remaining variants as flat arms ...
    }
}
```

**Critical path — `eval_call`:**
```rust
fn eval_call(name: &str, args: &[Expr], ctx: &mut EvalContext) -> Result<Value, RuntimeError> {
    if name.ends_with('#') {
        // Intrinsic call — evaluate args, then dispatch
        let evaluated: Vec<Value> = args.iter()
            .map(|a| eval_expr(a, ctx))
            .collect::<Result<Vec<_>, _>>()?;
        return execute_intrinsic(name, &evaluated, ctx.heap);
    }
    // User function call — look up definition, evaluate body
    eval_user_call(name, args, ctx)
}
```

**Extraction for flat code:**
- `eval_identifier` — look up in bindings, or error
- `eval_call` — dispatch: `#` suffix → intrinsic, else user call
- `eval_binary_op` — resolve `op` via `get_operator_intrinsic()`, then
  either `execute_intrinsic()` or `eval_user_call()`
- `eval_field_access` — struct field offset computation in Bits
- `eval_index` — resolve `op ExtractFrom`, then dispatch
- `eval_block` — sequential statement evaluation
- `eval_if` — condition + branch
- `eval_within` — scope with temporary bindings (fix existing nesting violations)

**Nesting violation fix (current `eval_subtype_projection` at 4 levels):**
The old `eval_subtype_projection` function has 4 levels of nesting
(regex captures → for ops → if let Match → for caps). The rewrite
extracts:
- `try_regex_match(regex, value) -> Option<Captures>`
- `try_op_match(op, captures) -> Option<Value>`
- `try_extract_key_eq(eq_expr, captures) -> Option<(String, Value)>`

Each extracted helper is flat (≤2 levels) and independently testable.

**Test coverage:**
- Every literal variant evaluates to correct `Value::Bits`
- `+` on Ints → `AddI64#` → correct bitwise addition
- `+` on Floats → `FAddF64#` → correct IEEE 754 addition
- `#` call: `Sqrt#(4.0)` → `Value::Bits` representing 2.0
- User function call: evaluates body with parameter bindings
- Block evaluation: sequential execution, last value is block value
- Within expression: scoped bindings
- Error: undefined variable → runtime error with name
- Error: type mismatch in intrinsic call → runtime error

### 10.3 `src/interpreter/intrinsics.rs` — REWRITE, ~2000 lines

**Purpose:** `execute_intrinsic(name, args, heap)` — one flat match arm per `#`.

```rust
pub fn execute_intrinsic(
    name: &str,
    args: &[Value],
    heap: &mut VirtualHeap,
) -> Result<Value, RuntimeError> {
    match name {
        "AddI64#" => {
            let a = bits_to_i64(&args[0]).ok_or(RuntimeError::expected_int(args[0]))?;
            let b = bits_to_i64(&args[1]).ok_or(RuntimeError::expected_int(args[1]))?;
            Ok(Value::Bits(i64_to_bits(a.wrapping_add(b))))
        }
        "FAddF64#" => {
            let a = bits_to_f64(&args[0]).ok_or(RuntimeError::expected_float(args[0]))?;
            let b = bits_to_f64(&args[1]).ok_or(RuntimeError::expected_float(args[1]))?;
            Ok(Value::Bits(f64_to_bits(a + b)))
        }
        "PrintInt#" => {
            let n = bits_to_i64(&args[0]).ok_or(RuntimeError::expected_int(args[0]))?;
            eprintln!("{}", n); // compile-time print
            Ok(Value::Void)
        }
        "Malloc#" => {
            let size = bits_to_i64(&args[0]).ok_or(RuntimeError::expected_int(args[0]))?;
            let addr = heap.allocate(size as usize);
            Ok(Value::Bits(i64_to_bits(addr as i64)))
        }
        "Free#" => {
            let ptr = bits_to_i64(&args[0]).ok_or(RuntimeError::expected_ptr(args[0]))?;
            heap.free(ptr as u64)?;
            Ok(Value::Void)
        }
        // ... every intrinsic, alphabetically sorted ...
        _ => Err(RuntimeError::unknown_intrinsic(name)),
    }
}
```

**This file absorbs ALL of `intrinsic_dispatch.rs`** — every arm from that
file moves here. `intrinsic_dispatch.rs` is deleted.

**Test coverage — every single intrinsic:**
- Arithmetic: AddI64#, SubI64#, MulI64#, DivI64#, RemI64#
- Float: FAddF64#, FSubF64#, FMulF64#, FDivF64#
- Comparison: EqI64#, LtI64#, FEqF64#, EqI1#
- Math: Sqrt#
- Memory: Malloc#, Free#
- IO: PrintInt#, PrintFloat#, PrintString#, GetEnvInt#
- GPU: GetGlobalId#, GetGlobalSize#, GetLocalId#
- Collections: ListInsert#, ListGet#
- Strings: StringConcat#, StringLength#
- Conversions: FloatToInt#, IntToFloat#
- Error: unknown intrinsic name → RuntimeError

### 10.4 `src/interpreter/cells.rs` — REWRITE, ~2000 lines

**Purpose:** Cell/thread execution logic.

**Changes from current code:**
- Replace all `Intrinsic::TtyReadKey`, `Intrinsic::Sleep`, etc. with
  string-based dispatch to `execute_intrinsic()`
- Cell tick evaluation: for each transaction, check precondition, execute body
- Thread scheduling for `async` cells
- Trigger dispatch for `trg` bindings

**Test coverage:**
- Cell tick with satisfied precondition → body executes
- Cell tick with unsatisfied precondition → body skipped
- State field updates persist across ticks
- Trigger fires when condition becomes true

### 10.5 `src/interpreter/ffi.rs` — REWRITE, ~1500 lines

**Purpose:** FFI dispatch for `frgn` calls during compile-time evaluation.

**Changes from current code:**
- Remove any remaining `Intrinsic` enum references
- FFI functions are dispatched by name string (not enum variant)
- For each `frgn` declaration, look up the native function via `libloading`
- Marshal parameters from `Value::Bits` to C types and back

**Test coverage:**
- FFI call with no arguments
- FFI call with Int argument → correct native type
- FFI call returns Int → correct Value::Bits
- Unknown FFI function → runtime error

### 10.6 `src/interpreter/casts.rs` — KEEP, 268 lines

Already clean and flat. No changes needed.

### 10.7 DELETE `src/interpreter/intrinsic_dispatch.rs`

All 1423 arms migrated into `src/interpreter/intrinsics.rs`.

---

## 11. Phase 4 — Backend Implementation

**Dependency:** Phase 0 (AST), Phase 2 (type_universe, typechecker),
Phase 3 (interpreter as reference), Phase 1 (bindings.dbvl).

### 11.1 `src/backend/llvm/mod.rs` — REWRITE, ~2000 lines

**Purpose:** Top-level LLVM compilation orchestration.

```rust
pub fn compile_module(program: &Program, universe: &TypeUniverse) -> Result<String, BackendError>;
```

- Loads `bindings.dbvl` for intrinsic templates
- Iterates top-level items, dispatches to sub-emitters
- Emits `_start` or `main` (if `[#]` entry points exist)
- Runs `opt -O3` on the generated IR (via optimizer.rs)

### 11.2-11.5: LLVM backend modules

See the file list in the Phase 4 section of the dependency graph. Each is
a clean rewrite or split from the current monoliths.

Key points:
- `function.rs`: per-function SSA state, register management
- `phi.rs`: phi node emission with deterministic (sorted-by-key) iteration
- `loop_engine.rs`: transaction convergence codegen (remaining after split)
- `emit_expr.rs`: expression codegen, `#` calls look up `bindings.dbvl`
- `emit_toplevel.rs`: `export defn` wrapper emission, `[#]` entry `main` emission
- `intrinsics.rs`: `#` intrinsic codegen via `.dbvl` bindings (clean rewrite!)

### 11.6-11.8: Other backends

- `webstack.rs`: WASM `#` intrinsic codegen, alloc metadata
- `circt.rs`: CIRCT `#` intrinsic codegen, alloc metadata
- `bindgen.rs`: `__briev_init_state`/`__glue_release` in C/Rust/Python headers

---

## 12. Phase 5 — Proof Engine + Analysis

**Dependency:** Phase 0 (AST), Phase 2 (type universe).

### 12.1 `src/proof_engine/mod.rs` — REWRITE, ~2000 lines

**Purpose:** Contract verification via SMT solving.

**Changes:**
- Add `#` → Z3 mapping: `Value::Bits(bytes)` → `(_ BitVec N)`
- Contract checking pipeline: pre/post conditions become SMT-LIB constraints
- `fn prove_contract(defn: &Definition, universe: &TypeUniverse) -> Result<(), Vec<ProofError>>`

### 12.2 `src/proof_engine/smt.rs` — REWRITE, ~1500 lines

**Purpose:** SMT-LIB query construction and solver invocation.

### 12.3-12.5: Minor cleanup files

- `features/traits.rs`: remove `IntrinsicCall` arms (tiny diff)
- `desugarer.rs`: remove `IntrinsicCall` arm, keep existing desugaring
- `normalize_types.rs`: remove `IntrinsicCall` arm (tiny diff)
- `analysis/equality_saturation.rs`: remove `IntrinsicCall` arm (tiny diff)

---

## 13. Phase 6 — Derivation Module (NEW)

**Dependency:** Phase 0 (AST definitions for `DerivationBlock`),
Phase 2 (type checker for example validation).

Four new files (`derive/mod.rs`, `engine.rs`, `smt.rs`, `cli.rs`) totaling
~3500 lines. This is the synthesis engine that uses `:=` derivation blocks
to synthesize function bodies from input/output examples.

---

## 14. Phase 7 — Main + CLI + Library Mode

**Dependency:** Everything up to Phase 4 (backends).

### 14.1 `src/main.rs` — REWRITE, ~2000 lines

**Purpose:** CLI entry point.

- `CompilationJob` struct with `variant`, `layout_parser`, `strict_mode`, `cell_wrapper`
- `analyze_file_pipeline()` — parses filename for `.sf.bv` etc.
- `--library` flag dispatch
- `derive` subcommand dispatch

### 14.2 `src/compile.rs` — NEW (from main.rs split), ~2000 lines

**Purpose:** Compilation pipeline orchestration.

### 14.3 `src/library.rs` — NEW (from main.rs split), ~1000 lines

**Purpose:** `--library` mode: `.ll` → `llc` → `.o` → `ar` → `.a`.

---

## 15. Phase 8 — Archive + Tests + Stdlib

**Dependency:** Everything above.

- `src/archive/mod.rs`: basic `.dbvl` writer
- Test updates: replace old `Intrinsic::Foo` with `"Foo#"`, `Value::Int(42)` with `Value::Bits(...)`
- Stdlib: rewrite `inop` → `defn` with `interpreter_impl` metadata, rename
  intrinsics to PascalCase`#`

---

## 16. Verification — Does This Plan Match the Intended Design?

### 16.1 Cross-Reference: All 6 Source Plans

| Source Plan | Where Addressed | Status |
|---|---|---|
| **Big Rewrite Execution** (Phases 0-10) | Phases 0-7 cover all Big Rewrite phases. Type_universe split done in Phase 2. FFI registry split done in Phase 1 (parser metadata). | ✅ |
| **Intrinsic Architecture** (Phase 8G) | Phase 0.7 (`intrinsic_signatures.rs`), Phase 3.2 (`intrinsics.rs` DELETE old dispatch), no `Intrinsic` enum, `#` as ident char in lexer. | ✅ |
| **Derivation & Synthesis** (Phases 8-14) | Phase 6 (NEW derive/ module). Parser handles `:=` blocks in Phase 1.2. | ✅ |
| **Modifiers/Entry/Scripting** (Phases 16A-16F) | Phase 7 (`CompilationJob`/`analyze_file_pipeline`), Phase 0.5 (`Contract.is_entry`), Phase 1.2 (`[#]` parsing), Phase 1.7 (`layout.rs`), Phase 2.5 (call graph isolation). | ✅ |
| **Alloc Metadata** (A.0-A.6) | Phase 1.5/1.7 (parser metadata for `alloc`), Phase 2.5 (validate/expand alloc), Phase 4.9 (backend validate.rs). | ✅ |
| **Library Mode** (Phase 15) | Phase 7.3 (`library.rs`), Phase 4.6 (emit_toplevel.rs export wrappers), Phase 4.8 (bindgen.rs). | ✅ |

### 16.2 Architecture Design Principles

| Principle | How the plan enforces it |
|---|---|
| **Bits is the sole primitive** | Phase 3.0: `Value::Bits(Vec<u8>)` only. No `Value::Int`, `Value::Float`, etc. |
| **`#` is an identifier char** | Phase 0.1 (lexer): `#` in identifiers. Phase 0.7: `get_intrinsic_signature("AddI64#")` |
| **No `Intrinsic` enum** | Phase 0.2-0.6: AST has no `Intrinsic` or `Expr::IntrinsicCall`. Phase 3.1: `eval_call` checks `name.ends_with('#')` |
| **op bindings resolve rune→fn** | Phase 2.4: `get_operator_intrinsic("+", &Int) → OpBinding::Intrinsic("AddI64#")` |
| **Metadata is opaque to frontend** | Phase 1.5: parser stores metadata as `HashMap<String, PropertyValue>`, no key interpretation. Phase 2.5: typechecker reads only known keys (`alloc`, `formatting`, `op`). |
| **Distributed validation** | Phase 2.5: frontend validates `alloc("Stack")` escape analysis. Phase 4.9: backend validates alloc address in memory map. |
| **Flat code (max 2 nesting)** | Every file spec includes flat code requirement. Phase sections document specific extraction points. |
| **Every file ≤ 3000 lines** | File size budget enforced at every split point. No file exceeds 2500 lines during rewrite. |

### 16.3 Risk Mitigation Status

| Risk | Mitigated in | How |
|---|---|---|
| `op` resolution is SPOF | Phase 2.4 | Most-tested function; matrix coverage; returns `None` (not panic) for missing bindings |
| Bits-only debugging | Phase 3.0 | `Value::display_typed(ty)`, typed test helpers (`assert_eq_int`), error messages include type |
| VirtualHeap maturity | Phase 3.0 | Separate test suite before any interpreter use; Kani proof harness; bounds-check every access |
| `formatting <~` errors | Phase 2.4 | Error messages show source text, accepted forms, and remediation hint |

---

## Appendix A: Per-Phase Commit Checklist

Before committing each phase's work:

- [ ] All new files exist at correct paths
- [ ] `cargo build` succeeds with zero warnings
- [ ] `cargo test --lib` passes (all tests, including existing ones)
- [ ] Every new `fn` has `///` doc comment
- [ ] Every change site has `// YYYY-MM-DD: <why` rationale comment
- [ ] No file exceeds 3000 lines
- [ ] No function exceeds 2 nesting levels
- [ ] No `else if` chains beyond one level
- [ ] No `if let Some(x) { if let Some(y) { ... } }` patterns
- [ ] All old `Intrinsic`/`IntrinsicCall` references in the file are removed
- [ ] `git add` only intended files
- [ ] Commit message references phase number and plan document

---

## Appendix B: Intrinsic Name Migration Table

| Old name | New name |
|---|---|
| `__add_i64` | `AddI64#` |
| `__sub_i64` | `SubI64#` |
| `__mul_i64` | `MulI64#` |
| `__div_i64` | `DivI64#` |
| `__rem_i64` | `RemI64#` |
| `__eq_i64` | `EqI64#` |
| `__lt_i64` | `LtI64#` |
| `__add_f64` | `FAddF64#` |
| `__sub_f64` | `FSubF64#` |
| `__mul_f64` | `FMulF64#` |
| `__div_f64` | `FDivF64#` |
| `__eq_f64` | `FEqF64#` |
| `__lt_f64` | `FLtF64#` |
| `__eq_bool` | `EqI1#` |
| `__eq_i32` | `EqI32#` |
| `print_int` | `PrintInt#` |
| `print_float` | `PrintFloat#` |
| `print_string` | `PrintString#` |
| `get_env_int` | `GetEnvInt#` |
| `get_env_string` | `GetEnvString#` |
| `sqrt` | `Sqrt#` |
| `malloc` | `Malloc#` |
| `free` | `Free#` |
| `memcpy` | `Memcpy#` |
| `memset` | `Memset#` |
| `get_global_id` | `GetGlobalId#` |
| `get_global_size` | `GetGlobalSize#` |
| `get_local_id` | `GetLocalId#` |
| `list_insert` | `ListInsert#` |
| `list_get` | `ListGet#` |
| `string_concat` | `StringConcat#` |
| `string_length` | `StringLength#` |
| `float_to_int` | `FloatToInt#` |
| `int_to_float` | `IntToFloat#` |

---

*End of plan.*
