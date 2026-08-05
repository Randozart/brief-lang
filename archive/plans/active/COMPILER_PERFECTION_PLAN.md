# Compiler Perfection Plan — Consolidated

**Created:** 2026-05-27
**Tests baseline:** 269 passing

---

## Problem Summary

The Briv self-hosted compiler (`lib/compiler/`) has **6 files** that fail to parse due to Rust-isms that leaked in during porting. The Rust bootstrap parser (`src/parser.rs`) short-circuits on the first error, forcing a fix-compile-repeat cycle. The LSP inherits the same limitation.

---

## Part A: Multi-Error Parser Recovery (~1 session)

### Current Behavior
```rust
fn parse_something(&mut self) -> Result<Expr, SyntaxError> {
    // On first error, returns Err immediately
    // Entire program aborts
}
```

### Target Behavior
```rust
fn parse_statements(&mut self) -> (Vec<Statement>, Vec<SyntaxError>) {
    let mut stmts = Vec::new();
    let mut errors = Vec::new();
    while !self.is_at_end() {
        match self.parse_one_statement() {
            Ok(stmt) => stmts.push(stmt),
            Err(e) => {
                errors.push(e);
                self.sync_to_next_stmt(); // skip tokens until ; or }
            }
        }
    }
    (stmts, errors)
}
```

### Changes Required

**In `src/parser.rs`:**
1. Add `sync_to_next_stmt()` — skips tokens until `;`, `}`, or EOF
2. Modify `parse_body()`, `parse_program()` and top-level dispatch to collect errors instead of returning on first `?`
3. Change `parse_program()` return type to include `Vec<SyntaxError>` for all collected errors
4. Add error-count output to CLI: "Found X errors"

**In `src/main.rs`:**
5. Update `run_check` to use new parser signature
6. Print all errors, not just the first

**In `src/lsp.rs`:**
7. Wire multi-error collection into `publishDiagnostics`

---

## Part B: Fix Briv Source Rust-isms (10 changes, ~15 min)

### Files and exact fixes

| # | File | Line | Change |
|---|------|------|--------|
| 1 | proof_engine.bv | 102-103 | `SymFloat(l+r)` in ExprVar → `SymUnknown` |
| 2 | proof_engine.bv | 156 | `uni (a,b) = (C,d)` → nested `uni a(C) { uni b(d)` |
| 3 | proof_engine.bv | 164-179 | Identity ops: `uni (op,a,b)=(pat..)` → `[op==.. && a==..]` |
| 4 | proof_engine.bv | 185 | `Box(operand_simp)` → `operand_simp` |
| 5 | typechecker.bv | 405-413 | `items[i] = Pattern` → `items[i](Pattern)` |
| 6 | typechecker.bv | 458 | `Ok((body_ty, _))` → `Ok(pair)` + `.0` access |
| 7 | typechecker.bv | 470 | `Ok(())` → `Ok(1)` |
| 8 | call_graph.bv | 115 | `[true][true]` → meaningful contract |
| 9 | parser.bv | 183 | `uni tok(..)` inside guard → `[tok != ..]` |
| 10 | option.bv | 107 | `result.is_some() -> pred()` contract → `true` |

---

## Part C: Add list concat `++` to parser (~30 min)

The `++` operator for list concatenation is used in `call_graph.bv` (`called ++ collect_call_names(expr)`). Currently parsed as prefix `+` operator on a list literal.

### Changes
1. **`src/lexer.rs`**: Add `PlusPlus` token (before `Plus` for longest-match)
2. **`src/parser.rs`**: In `parse_additive()`, match `Token::PlusPlus` → `Expr::Concat`
3. **`src/ast.rs`**: Add `Expr::Concat(Box<Expr>, Box<Expr>)` variant, or reuse `Expr::Add` with type-aware backends

---

## Implementation Order

```
Session 1 (this session):
  Part C: Add ++ to lexer + parser (30 min)
  Part B: Fix all 10 source Rust-isms (15 min)
  └─ Verify all 9/9 core Briv files parse

Session 2 (next session):
  Part A: Multi-error parser recovery (1 session)
  └─ Run end-to-end transpilation test
  └─ Backend stubs fixup
  └─ LSP error collection
```
