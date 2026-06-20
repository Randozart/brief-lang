# UnaryOp — Unified Unary Operations

**Date:** 2026-06-20  
**Phase:** 3  
**Status:** Implemented (parser produces UnaryOpExpr, interpreter evaluates, LLVM backward-compat shim, webstack direct emit)

## Design

The old `Expr::Neg(e)`, `Expr::Not(e)`, `Expr::BitNot(e)` variants have been unified into `Expr::UnaryOp(Box<UnaryOpExpr>)`. `UnaryOpExpr` carries a `UnaryOpKind` enum (Neg, Not, BitNot) and an `operand` `Expr`.

## Motivation

Same as BinaryOp — the Pattern B unification reduces variant count and enables uniform dispatch across backends.

## Evaluation

| Kind | Int | Bool | Error |
|------|-----|------|-------|
| Neg | -a | — | TypeMismatch |
| Not | — | !a | TypeMismatch |
| BitNot | !a | — | TypeMismatch |

## Backward Compatibility

Old `Expr::Neg`, `Expr::Not`, `Expr::BitNot` remain in the enum. Interpreter shims delegate to `UnaryOpExpr::evaluate()`. Parser now produces `Expr::UnaryOp` directly.

## Backend Support

| Backend | Status |
|---------|--------|
| **Interpreter** | Real evaluation, all 3 kinds handled |
| **LLVM** | Backward-compat shim: reconstructs old Expr variant, delegates to existing emit_expr arms |
| **Webstack** | Direct emit: `expr_to_ts` matches `Expr::UnaryOp(uop)` and dispatches on kind to JS operators |
| **CIRCT** | No match arms — dead backend |

## Files

| File | Responsibility |
|------|---------------|
| `src/features/unary_op.rs` | `UnaryOpExpr` struct, `UnaryOpKind` enum, evaluate/typecheck/emit_llvm/emit_js |
| `src/ast.rs` | `Expr::UnaryOp` variant |
| `src/parser.rs` | Parser produces `Expr::UnaryOp` for `!`, `-`, `~` |
| `src/interpreter.rs` | Shims for old `Expr::Not` etc. → `UnaryOpExpr` |
| `src/backend/llvm/emit_expr.rs` | `Expr::UnaryOp` dispatches to `UnaryOpExpr::emit_llvm` |
| `src/backend/webstack.rs` | `Expr::UnaryOp` dispatches on kind to JS operators |
| `src/proof_engine.rs` | Normalizes UnaryOp to old variants via `normalize_to_old()` |
