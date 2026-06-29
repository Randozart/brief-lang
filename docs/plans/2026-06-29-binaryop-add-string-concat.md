# BinaryOpKind::Add Missing is_string_chain Check

Date: 2026-06-29

## Root Cause

The parser creates `Expr::BinaryOp(BinaryOpExpr)` (new-style packed variants) for ALL binary operations. The old-style variants (`Expr::Add`, `Expr::Sub`, etc.) are only used through `expr.normalize_to_old()`.

The old-style `Expr::Add` handler at `emit_expr.rs:404` correctly checks `is_string_chain` before deciding to emit string concatenation vs integer addition. But the new-style `BinaryOpKind::Add` handler at `binary_op.rs:132` calls `ctx.emit_binop` DIRECTLY, **without** checking `is_string_chain`.

This means any `a + b` where both operands are strings, but where the parser generated new-style `Expr::BinaryOp`, produces `add i64` instead of inline string concatenation. This adds the tagged pointer addresses as integers, producing a garbage address that crashes when dereferenced.

## Crash in officina

`draw_top_bar` computes: `" officina [" + target_os + "]"`. The new-style binary op generates:
```llvm
%t35 = add i64 %t32, %t34    ; @str.101_addr + target_os_tagged_ptr  → garbage
%t39 = add i64 %t35, %t37    ; + @str.110_addr                       → bigger garbage
```
Passing this address to `fprintf` reads from unmapped memory → SIGSEGV at `0xc1fab1`.

## Fix

Add `is_string_chain` check to `BinaryOpKind::Add` arm in `binary_op.rs:132`:

```rust
BinaryOpKind::Add => {
    if ctx.is_string_chain(&self.left) || ctx.is_string_chain(&self.right) {
        let a = ctx.emit_expr(out, &self.left, "");
        let b = ctx.emit_expr(out, &self.right, "");
        return ctx.emit_inline_concat(out, "", &a, &b);
    }
    ctx.emit_binop(out, "", &self.left, &self.right, "add", "fadd")
}
```

## Testing

- `cargo test --lib` must pass
- officina should boot without SIGSEGV in draw_top_bar
