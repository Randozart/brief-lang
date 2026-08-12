# Fix emit_expr: last_val_temps ordering & float type width

## Root Causes

Three remaining benchmark failures after the 2026-07-17 fixes share two root
causes in `src/backend/llvm/emit_expr.rs`:

### Issue 1: Identifier resolution ignores `last_val_temps` (const_heavy)

`emit_expr` for `Expr::Identifier(name)` checks `phi_field_regs` before
`last_val_temps`.  In a per-field phi loop body, after `count = count + 1`
updates `last_val_temps["count"]`, a subsequent guard `[count % 5000000 == 0]`
reads the **phi register** (start-of-iteration value) instead of the **updated
value** — producing wrong guard evaluation and wrong print values.

**Fix**: Check `self.fun.last_val_temps.get(name)` before `phi_field_regs`.

### Issue 2: Constant fallthrough always loads as `i64` (iir_filter, float_math)

The `else` branch at line 102-105 does `load i64, ptr @<name>` for all
constants regardless of their type.  Float constants declared as
`constant float ...` in the IR are loaded as `i64`, producing type mismatches
when fed to `fmul double` or `fadd double`.

**Fix**: Look up `self.ctx.constants.get(name)` and emit the correct LLVM load:
`load float` for Float, `load double` for Float64/Double.

### Issue 3: Float width confusion — all Float ops emit double (iir_filter, float_math)

Two sub-issues:

**3a — `Expr::Float` literal (line 38-41)**: emits `fadd double 0.0, f` and
returns `Type::float64()`.  But the typechecker assigns `Type::float()` (32-bit)
to float literals.  The emitted IR width disagrees with the type annotation.

**3b — `emit_binary_op` (line 436-438)**: `ty_str = if is_float { "double" } else { "i64" }`.
All float operations use `double` irrespective of whether the operand type is
`Type::float()` (32-bit) or `Type::float64()` (64-bit).

**Fix 3a**: `Expr::Float(f)` → `fadd float 0.0, f`, return `Type::float()`.

**Fix 3b**: `emit_binary_op` — check actual widths:
- `float64` operand → `"double"`, return `Type::float64()`
- `float` operand → `"float"`, return `Type::float()`
- else → `"i64"`, return `Type::int()`

## Type Convention

| Briev Type | LLVM Type | Width |
|-----------|-----------|-------|
| `Float`   | `float`   | 32-bit |
| `Float64` | `double`  | 64-bit |

The constant emitter (`mod.rs:1891-1892`) and `lower_type` (`types.rs:35-36`)
already follow this convention.  The fixes bring `emit_expr` into alignment.

## Files Changed

- `src/backend/llvm/emit_expr.rs` — all four fixes

## Regression Prevention

- `cargo test --lib` — 913 tests must pass
- `precompute_sum` must still produce `249500` output
- `const_heavy BOUND=5` prints `21000` (matching C)
- `iir_filter` compiles (no LLVM type error)
- `float_math` compiles (no LLVM type error)
