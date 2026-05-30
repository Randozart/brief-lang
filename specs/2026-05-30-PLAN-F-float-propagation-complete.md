# PLAN F: Complete Float Type-Propagation for Blocks and Casts

**Date**: 2026-05-30
**Source**: External audit

## Background

Float type-propagation was implemented for basic math operators (`Expr::Add`, `Expr::Neg`, etc.) and literals, but two expression kinds were missed: `Expr::Block` and `Expr::Cast`. Compound expressions using these forms do not propagate float identity, so `is_float_expr` returns `false` for them, causing downstream operations to emit integer instructions on float bitcast registers.

## Root Cause

1. `is_float_expr` (llvm.rs:1430) has no arms for `Expr::Block` or `Expr::Cast`, falling through to `_ => false`.
2. No universal catch-all exists at the return of `emit_expr`, so any expression not explicitly annotated cannot propagate its float identity to `register_types`.

## Fix

### Fix 1: Add `is_float_expr` arms

Add two match arms to `is_float_expr` before `_ => false`:

```rust
Expr::Cast(_, ty) => ty == &Type::Float,
Expr::Block(_, last) => self.is_float_expr(last),
```

### Fix 2: Universal catch-all in `emit_expr`

At the end of `emit_expr`, after the match block closes and before `return v`, insert:

```rust
if self.is_float_expr(expr) {
    self.register_types.insert(v.to_string(), Type::Float);
}
```

This replaces the need for any per-expression float registration (the existing `Expr::Float`, `emit_binop`, and `Expr::Neg` registrations become redundant but harmless — we can clean them up or leave them as explicit documentation).

## Files Changed

- `src/backend/llvm.rs` only

## Verification

- `cargo build`
- `cargo test --lib` — all 294 tests pass