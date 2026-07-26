# Constraint Unification — Let-Bound and Type-Level Constraints via `: [expr]`

**Date:** 2026-06-20  
**Status:** Plan (ready for implementation)  
**Estimated effort:** 4–6 hours across ~8 files  
**Prerequisites:** Phase 4 (parser handles `_` as `Expr::Identifier("_")`), Phase 3.5 (TypeUniverse wired into pipeline)

---

## 1. The Problem

The codebase has **four separate constraint mechanisms** that all do the same thing — restrict the value space of a type using a boolean expression — but none of them enforce at runtime:

| # | Mechanism | Syntax | Location | Status |
|---|-----------|--------|----------|--------|
| 1 | `RangeConstraint` | `let x: Int : [0..100]` | `src/ast.rs:116` — `enum RangeConstraint` | ✅ parser, ❌ none enforce |
| 2 | `TypeDefBody.constraints` | `type Foo : Int { [ > 0 ]; }` | `src/ast.rs:376` — `constraints: Vec<Expr>` | ✅ parser stored as `ResolvedType.guards`, ❌ never evaluated |
| 3 | `Type::ContractBound` | `Int[product > 0]` | `src/ast.rs:148` — `Type::ContractBound` | ✅ parser, ❌ never enforced |
| 4 | (missing) | `let x: Int : [0..100] = 50;` | doesn't exist | ❌ not implemented, not parseable |

The primary goals of this plan:
1. **Unify** all four into one mechanism: `: [expr]` where `_` is the value placeholder
2. **Enforce** at runtime in both interpreter and LLVM
3. **Add** the `let x: Type : [expr] = val;` syntax
4. **Add** range sugar: `[lo..hi]` → `_ >= lo && _ <= hi`
5. **Add** CamelCase aliases for subtype query ops (`Filter`, `Map`, etc.)

---

## 2. Core Semantic Model

Every constraint is a boolean expression with `_` bound to the constrained value:

```brief
// Range sugar: [0..100]  →  _ >= 0 && _ <= 100

// Regex: [@"^[a-z]+@"]  →  _ :> Match pattern
//   (where Match is the existing string-regex projection)

// Explicit: [_ >= 0 && _ <= 100]

// Universal regex (works on any type via stringification):
//   @"123" on an Int means "the decimal representation must not contain 123"
```

**Principle:** `<:` changes the semantics of `[]` from "partition/access" to "constrain/restrict". Without `<:`, bare `[expr]` is always index/slice/partition.

---

## 3. Implementation Phases

### Phase A — Unify existing constraint mechanisms

**Goal:** Eliminate `RangeConstraint` and `Type::ContractBound`, folding them into the general constraint expression model.

#### A1. Remove `RangeConstraint` from AST

**File:** `src/ast.rs`

- Delete `pub enum RangeConstraint` (currently around line 116-122)
- Delete `RangeConstraint` from any match sites in parser, typechecker, interpreter

**Affected match sites** (search for `RangeConstraint::`):
- `src/parser.rs` — state declaration parsing (around line 3059, 4620) — replace with general `parse_constraint_expr()`
- `src/interpreter.rs` — if any eval arms exist (likely none — no runtime eval)
- `src/backend/llvm/emit_expr.rs` — if any codegen arms exist (likely none)
- `src/typechecker.rs` — if any type-checking arms exist

#### A2. Remove `Type::ContractBound` from AST

**File:** `src/ast.rs`

- Delete `ContractBound(Box<Type>, Box<Expr>)` from `pub enum Type`
- Find ALL match sites for `Type::ContractBound` and delete/redirect

**Affected match sites** (grep for `ContractBound`):
- `src/parser.rs` — `parse_type_inner` — redirect `Type[expr]` to produce `TypeDefBody { constraints: [expr] }` or reject
- `src/typechecker.rs` — type inference arms, return `src_ty` (strip the bound)
- `src/interpreter.rs` — skip/error for contract-bound
- `src/backend/llvm/mod.rs` — codegen arms
- `src/proof_engine.rs` — extraction arms
- `src/analysis/*.rs` — analysis arms
- `src/symbolic.rs` — symbolic evaluation arms

**Replacement:** `Type[expr]` in source can be desugared at the parser level to a reference to a compiler-synthesized TypeDef with the constraint, or simply produce an error suggesting `: [expr]` syntax instead.

#### A3. Keep `TypeDefBody.constraints` as the canonical form

**File:** `src/ast.rs` — no changes needed. `constraints: Vec<Expr>` stays.

**File:** `src/type_universe.rs` — `ResolvedType.guards: Vec<Expr>` already mirrors constraints. No changes needed.

---

### Phase B — `_`-binding constraint evaluation engine

**Goal:** A reusable constraint evaluation function in both interpreter and LLVM.

#### B1. Interpreter constraint eval

**File:** `src/interpreter.rs`

Add a method to `Interpreter`:

```rust
/// Evaluate a constraint expression with `_` bound to the given value.
/// Returns Ok(()) if the constraint passes, Err if it fails.
pub fn eval_constraint(&mut self, value: &Value, constraint: &Expr) -> Result<(), RuntimeError> {
    // 1. Bind `_` in a temporary scope
    let prior = self.state.insert("_".to_string(), value.clone());
    // 2. Evaluate the constraint expression
    let result = self.eval_expr(constraint)?;
    // 3. Restore prior `_` binding (or remove if none)
    match prior { Some(v) => { self.state.insert("_".to_string(), v); } None => { self.state.remove("_"); } }
    // 4. Check truthiness
    match result {
        Value::Bool(true) => Ok(()),
        _ => Err(RuntimeError::TypeMismatch("constraint violated".into())),
    }
}
```

#### B2. LLVM constraint codegen

**File:** `src/backend/llvm/emit_stmt.rs`

After emitting the store for a constrained let variable, evaluate the constraint:

```rust
fn emit_constraint_check(&mut self, out: &mut String, val_reg: &str, constraint: &Expr, indent: &str) {
    // 1. Emit the constraint expression with `_` replaced by val_reg
    //    (or better: bind `_` in the IR by adding a store to the alloca)
    // 2. Branch on false → call @llvm.trap
}
```

Simplest approach: store the value into an alloca `%_`, then evaluate the constraint expression normally (since `_` will resolve as an identifier in the expression context). After evaluation, a branch on false goes to a `call void @llvm.trap()` basic block.

#### B3. TypeUniverse guard enforcement

**File:** `src/interpreter.rs` + `src/typechecker.rs`

When a value is typed as a type with `ResolvedType.guards` non-empty:
- Interpreter: after the let eval, call `eval_constraint` for each guard
- The let-binding's type annotation provides the type name to look up

---

### Phase C — `let x: Type : [expr] = val;` syntax

**Goal:** Parse and store inline constraints on let statements.

#### C1. AST changes

**File:** `src/ast.rs`

Find the let-binding struct or the `Statement::Let` variant. Add:

```rust
pub constraint: Option<Box<Expr>>,
```

If `Statement::Let` directly carries fields (instead of a sub-struct), add it directly.

#### C2. Parser changes

**File:** `src/parser.rs`

In `parse_let_statement()` (find around line 4390-4480):

After parsing optional `: Type`, check for `: [expr]`:

```rust
// After: let x: Type
if let Some(Ok(Token::LtColon)) = self.current_token() {
    self.advance();
    if let Some(Ok(Token::LBracket)) = self.current_token() {
        self.advance();
        let constraint = self.parse_expression()?;
        self.expect(Token::RBracket)?;
        // Store in let binding struct
    }
}
// Then optionally parse = initializer
// Then expect ;
```

**Handling range sugar:** During `parse_expression` for the constraint, `0..100` would parse as a Slice expression. To detect the range sugar, after parsing the first expression, check for `Token::DotDot`:

```rust
let first = self.parse_expression()?;
if matches!(self.current_token(), Some(Ok(Token::DotDot))) {
    self.advance();
    let second = self.parse_expression()?;
    // Desugar: [_ >= first && _ <= second]
    let constraint = Expr::And(
        Box::new(Expr::Ge(Box::new(Expr::Identifier("_".into())), Box::new(first))),
        Box::new(Expr::Le(Box::new(Expr::Identifier("_".into())), Box::new(second))),
    );
    // Store constraint
}
```

#### C3. Full grammar

```
let_stmt ::= "let" ident (":" type_expr)? ("<:" "[" constraint_expr "]")? ("=" expr)? ";"
```

All four parts are independent. Valid forms:

```brief
let x: Int = 5;                  // existing — type annotation only
let x: Int : [0..100];          // new — type + constraint, no initializer
let x = 50;                      // existing — initializer only
let x: Int : [0..100] = 50;     // new — type + constraint + initializer
let x : [0..100];               // new — constraint only (type inferred)
```

---

### Phase D — Range sugar `[lo..hi]`

**File:** `src/parser.rs`

Add a helper function `parse_constraint_expr()` that:

1. Tries to parse `lo..hi` pattern
   - Parse first expression
   - If `DotDot` follows → parse second expression → desugar to `[_ >= lo && _ <= hi]`
2. Otherwise, parse as a normal expression

**Desugaring rule:**

```rust
"lo..hi"  →  _ >= lo && _ <= hi
```

Implemented as:

```rust
Expr::And(
    Box::new(Expr::Ge(Box::new(Expr::Identifier("_".to_string())), Box::new(lo))),
    Box::new(Expr::Le(Box::new(Expr::Identifier("_".to_string())), Box::new(hi))),
)
```

This reuses the existing `Expr::Ge`/`Expr::Le`/`Expr::And` variants (which still exist in the enum for backward compat, even though the parser produces `Expr::BinaryOp` — the old variants work through `normalize_to_old()` shims if needed).

---

### Phase E — CamelCase query operation aliases

**File:** `src/parser.rs`, function `parse_single_subtype_op()` (around line 615-660)

Each match arm that matches an uppercase name should also match the CamelCase version:

```rust
"FILTER" | "Filter" => SubtypeOp::Filter(expr),
"MAP" | "Map" => SubtypeOp::Map(expr),
"SORT" | "Sort" => SubtypeOp::Sort(expr),
"LIMIT" | "Limit" => SubtypeOp::Limit(expr),
"SKIP" | "Skip" => SubtypeOp::Skip(expr),
"UNIQUE" | "Unique" => SubtypeOp::Unique(expr),
"JOIN" | "Join" => SubtypeOp::Join(expr),
"GROUP" | "Group" => SubtypeOp::Group(expr),
"COUNT" | "Count" => SubtypeOp::Count,
"SUM" | "Sum" => SubtypeOp::Sum,
"AVG" | "Avg" => SubtypeOp::Avg,
"MIN" | "Min" => SubtypeOp::Min,
"MAX" | "Max" => SubtypeOp::Max,
"MATCH" | "Match" => SubtypeOp::Match(expr),
```

Each arm receives `expr` (if the op takes an expression) or returns it directly. Check the actual match arm to see if it already destructures `expr` or produces it.

Full list of 14 ops (from `src/ast.rs:1493-1524`):
- `Filter`, `Map`, `Sort`, `Limit`, `Skip`, `Unique`, `Join`, `Group` (take expression)
- `Count`, `Sum`, `Avg`, `Min`, `Max` (take no expression)
- `Match` (takes string expression)

---

## 4. Files and Changes Summary

| File | Phase | Change | Lines |
|------|-------|--------|-------|
| `src/ast.rs` | A | Remove `RangeConstraint` enum | ~10 |
| `src/ast.rs` | A | Remove `Type::ContractBound` variant | ~3 |
| `src/ast.rs` | C | Add `constraint: Option<Box<Expr>>` to let-binding struct | ~3 |
| `src/parser.rs` | A | Remove `RangeConstraint` parse paths in state/struct decl | ~20 |
| `src/parser.rs` | A | Redirect `Type[expr]` parsing | ~10 |
| `src/parser.rs` | C | Add `: [expr]` parsing in `parse_let_statement` | ~20 |
| `src/parser.rs` | D | Add `parse_constraint_expr()` with `lo..hi` sugar | ~25 |
| `src/parser.rs` | E | Add CamelCase aliases in `parse_single_subtype_op()` | ~14 |
| `src/typechecker.rs` | A | Remove `RangeConstraint`/`ContractBound` handling | ~10 |
| `src/typechecker.rs` | B | Validate constraint expression returns Bool | ~10 |
| `src/interpreter.rs` | A | Remove any `RangeConstraint` eval | ~5 |
| `src/interpreter.rs` | B | Add `eval_constraint()` method | ~15 |
| `src/interpreter.rs` | C | Call `eval_constraint()` after let eval | ~10 |
| `src/interpreter.rs` | B3 | Wire `ResolvedType.guards` into type construction | ~15 |
| `src/backend/llvm/emit_stmt.rs` | B | Add `emit_constraint_check()` | ~20 |
| `src/backend/llvm/emit_stmt.rs` | C | Call after constrained let store | ~10 |
| `src/backend/llvm/emit_toplevel.rs` | B3 | Wire `guards` into function prologue | ~15 |
| `src/proof_engine.rs` | A | Remove `ContractBound` match arms | ~5 |
| `src/analysis/*.rs` | A | Remove `ContractBound` match arms | ~5 each |
| `src/symbolic.rs` | A | Remove `ContractBound` match arms | ~3 |

---

## 5. Tests to Add

| Test | What it verifies | Phase |
|------|-----------------|-------|
| `test_let_constraint_range_sugar` | `let x: Int : [0..100] = 50;` parses and evaluates | C+D |
| `test_let_constraint_regex` | `let e: String : [@"@"];` parses | C |
| `test_let_constraint_explicit` | `let x: Int : [_ > 0] = 5;` parses and eval | C |
| `test_let_constraint_violation` | `let x: Int : [0..100] = 200;` → RuntimeError | B+C |
| `test_camelcase_subtype_ops` | `Filter(` works same as `FILTER(` | E |
| `test_typedef_constraint_enforced` | TypeDef with constraint fails on invalid runtime value | B3 |
| `test_range_constraint_removed` | No `RangeConstraint` match sites remain | A |
| `test_contract_bound_removed` | No `Type::ContractBound` match sites remain (except error msg) | A |

---

## 6. Dependencies and Ordering

```
Phase A (unify)
  ↓
Phase B (eval engine) ←─ Phase B3 (guards) can run concurrently
  ↓
Phase C (let binding) ←─ depends on B for eval
  ↓
Phase D (range sugar) ←─ depends on C for integration point
  ↓
Phase E (CamelCase)   ←─ independent, can run at any point after A
```

All existing tests must pass after each phase. No commit should break the build.

---

## 7. Edge Cases

1. **`_` collision**: If `_` is already bound in the current scope, the constraint eval temporarily shadows it. After eval, restore the prior binding.

2. **Multiple constraints on one let**: A single constraint expression with `&&` covers multiple conditions. No need for `Vec<Expr>`.

3. **Type inference without annotation**: `let x : [0..100]` — the type is inferred from the constraint's expected type (Int for range, String for regex). This requires the typechecker to walk `_`'s usage.

4. **RegEx on non-String types**: `let x: Int : [@"123"]` — `@"123"` evaluates to `Value::Regex`. In constraint context, a `Value::Regex` should auto-match against the stringified `_`. Desugaring: `_ :> Str :> Match pattern`. This may require a new helper or explicit desugaring at parse time.

5. **`ContractBound` replacement**: `Int[product > 0]` should either produce an error (suggesting `type T : Int { [product > 0]; }`) or desugar to a synthesized TypeDef. For simplicity, **produce a clear error message**.

---

## 8. Verification

After all phases:
- `cargo test --lib` — all existing + new tests pass
- `cargo build --release` — not required for functionality but good to verify
- All match sites for removed AST variants (`RangeConstraint`, `Type::ContractBound`) are eliminated
- `let x: Type : [expr] = val;` works in both interpreter and LLVM
- TypeDef body constraints enforce at runtime
