# BinaryOp — Unified Binary Operations

**Date:** 2026-06-20  
**Phase:** 3  
**Status:** Implemented (parser produces BinaryOpExpr, interpreter evaluates, LLVM backward-compat shim, webstack direct emit)

## Design

The old `Expr::Add(l, r)`, `Expr::Sub(l, r)`, etc. variants (18 total) have been unified into a single `Expr::BinaryOp(Box<BinaryOpExpr>)` variant. `BinaryOpExpr` carries a `BinaryOpKind` enum (Add, Sub, Mul, Div, Mod, Eq, Ne, Lt, Le, Gt, Ge, And, Or, BitAnd, BitOr, BitXor, Shl, Shr) plus `left` and `right` `Expr` fields.

## Motivation

The old approach used 18 separate `Expr` variants, making pattern matching repetitive and preventing uniform handling. The unified struct is the "Pattern B" design — a single feature struct with a kind discriminator that all backends handle via one match arm.

## Evaluation

The interpreter dispatches on `Value` variant:

| Kind | Int | Float | Bool | Regex | Error |
|------|-----|-------|------|-------|-------|
| Add/Sub/Mul/Div/Mod | arithmetic | float op | — | — | TypeMismatch |
| Eq/Ne | comparison | comparison | comparison | pattern match | TypeMismatch |
| Lt/Le/Gt/Ge | comparison | comparison | — | — | TypeMismatch |
| And/Or | — | — | boolean | — | TypeMismatch |
| BitAnd/BitOr/BitXor/Shl/Shr | bitwise | — | — | — | TypeMismatch |

## Backward Compatibility

The old `Expr::Add`, `Expr::Sub`, etc. variants remain in the `Expr` enum for backward compatibility. Tests and analysis passes that construct old variants directly continue to work. The interpreter has shims that delegate old variants to `BinaryOpExpr::evaluate()`.

The parser now produces `Expr::BinaryOp` directly — no more old variants from parsing.

## Backend Support

| Backend | Status |
|---------|--------|
| **Interpreter** | Real evaluation, all 18 kinds handled |
| **LLVM** | Backward-compat shim: reconstructs old Expr variant, delegates to existing emit_expr arms |
| **Webstack** | Direct emit: `expr_to_ts` matches `Expr::BinaryOp(bop)` and dispatches on kind to TS operators |
| **CIRCT** | No match arms — dead backend |

## Files

| File | Responsibility |
|------|---------------|
| `src/features/binary_op.rs` | `BinaryOpExpr` struct, `BinaryOpKind` enum, evaluate/typecheck/emit_llvm/emit_js |
| `src/ast.rs` | `Expr::BinaryOp` variant |
| `src/parser.rs` | Parser produces `Expr::BinaryOp` for all operator tokens |
| `src/interpreter.rs` | Shims for old `Expr::Add` etc. → `BinaryOpExpr` |
| `src/backend/llvm/emit_expr.rs` | `Expr::BinaryOp` dispatches to `BinaryOpExpr::emit_llvm` |
| `src/backend/webstack.rs` | `Expr::BinaryOp` dispatches on kind to JS operators |
| `src/proof_engine.rs` | Normalizes BinaryOp to old variants via `normalize_to_old()` |
