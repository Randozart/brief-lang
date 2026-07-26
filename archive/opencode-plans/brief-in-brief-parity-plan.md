# Brief-in-Brief Parity Plan

## Goal
Bring `parser.bv`, `typechecker.bv`, `ast.bv`, `token.bv`, and `proof_engine.bv` in the self-hosted Brief compiler up to parity with the Rust compiler's `Expr::Term` handling.

## Background

The Rust compiler now has:
- `Expr::Term` AST node
- `token::Term` → `Expr::Term` in expression parsing
- `expr_has_result()` matching both `Expr::Term` and `Expr::Identifier("result")`
- Symbolic verification (not type-checking) of postconditions at call sites via `satisfies_postcondition`

The Brief-in-Brief compiler has none of this. It does pure type inference, not symbolic execution.

## Changes

### 1. `lib/compiler/ast.bv`
Add `ExprTerm` variant to the `Expr` enum.

### 2. `lib/compiler/token.bv`
Add `KeywordTerm` to `is_keyword()`.

### 3. `lib/compiler/parser.bv` — `parse_primary_expr()`
Add `KeywordTerm` → `ExprTerm` case. This is the critical parsing change that lets `term` appear inside `[...]` contract brackets.

### 4. `lib/compiler/typechecker.bv`
Two changes:
- `infer_expr()`: add `uni expr(ExprTerm)` that looks up `"term"` from the type context (same pattern as `ExprVar`)
- `check_definition()` and `check_transaction()`: bind `"term"` with the function's return type into the type context before checking the postcondition

### 5. `lib/compiler/proof_engine.bv`
`collect_identifiers()`: add `ExprTerm` to the leaf-expression catch-all (same as Rust `proof_engine.rs:2076` does).

### 6. Tests
- `cargo build` (Rust compiler compiles with any `.bv` changes)
- `cargo test --lib` (all 269 tests pass)

## Files to modify (5 files)
1. `lib/compiler/ast.bv`
2. `lib/compiler/token.bv`
3. `lib/compiler/parser.bv`
4. `lib/compiler/typechecker.bv`
5. `lib/compiler/proof_engine.bv`
