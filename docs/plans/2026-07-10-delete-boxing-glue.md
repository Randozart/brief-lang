# Phase 7: Delete the Boxing Glue — Type-Aware Store Utility

## Problem

The compiler's LLVM backend uses `i64` as a universal boxing type for all values.
Floats, pointers, chars, bools — all are stored as `i64` with bitcast/trunc/zext
conversion at use sites. This creates a recurring class of bugs where new code
paths emit `store i64` without checking the actual type, producing invalid IR
like:

```
store i64 %t30, ptr %ap_31, align 8    ; %t30 has type 'float' — i64 mismatch
```

This is not a one-off — it's a structural fragility. Every new codegen path
that writes to a state field must independently remember to:
1. Look up the field's LLVM type from `ctx.field_types[idx]`
2. Convert the value to that type via `ensure_typed_value`
3. Use the correct LLVM type in the `store` instruction
4. Use the correct alignment for that type

If any path forgets any of these four steps, the IR is invalid.

## Root Cause

The `Expr::AddrOf` and `Expr::Deref` LHS handlers in `emit_stmt.rs:704,716`
hardcode `store i64`. They were added in Phase 2 (commit `cf1842f`) and never
went through the type-awareness path that the `TupleDestructure` handler uses
at line 791.

The function `expr_has_call` in `region.rs:1510` also missed `Expr::IntrinsicCall`,
causing `getenv_int#` to be treated as a constant and all precomputation analysis
to produce false fold decisions.

## Fixes

### Fix 1: Type-aware store in AddrOf/Deref handlers (emit_stmt.rs:704,716)

Replace the hardcoded `store i64` with the type-aware pattern already used at
line 791:

```
1. Extract field name from inner expression
2. Look up idx in field_index_map
3. Get ty = field_types[idx] (e.g., "float", "i8", "i32", "i64")
4. Get briev_ty = field_briev_types[idx] (for universe unboxing)
5. Call ensure_typed_value(out, indent, &ty, &val.name, briev_ty)
6. Emit store with correct ty, typed_value, and align_of(&ty)
```

**Flat control flow** (max 2 levels nesting):

```rust
let Some(name) = inner.as_var_name() else { return; };
let Some(&idx) = self.ctx.field_index_map.get(name) else {
    writeln!(out, "{}; assign to unknown field '{}'", indent, name).ok();
    return;
};
let ty = self.ctx.field_types[idx].clone();
let sr = self.fun.state_reg_name.clone();
let p = self.emit_state_gep(out, indent, "ap", &sr, idx);
let briev_ty = self.ctx.field_briev_types.get(idx).cloned();
let tv = self.ensure_typed_value(out, indent, &ty, &val.name, briev_ty);
writeln!(out, "{}store {} {}, ptr {}, align {}", indent, ty, tv, p, self.align_of(&ty)).ok();
```

### Fix 2: Recognize IntrinsicCall in expr_has_call (region.rs:1510)

```rust
Expr::Call(_, _) | Expr::IntrinsicCall { .. } => return true,
```

This prevents the precomputation analyzer from treating `getenv_int#` calls as
constant expressions. Fixes fannkuch_redux, mandelbrot, queue_drain, interval_step
false positives, AND the queue_drain_idio duplicate `%t10` register (which was a
symptom of the same root cause).

### Fix 3: Remove dead code (loop_engine.rs:2294-2296)

The first `let mut stmts` declaration at line 2294 is immediately shadowed by
the declaration at line 2303. Remove the dead declaration.

### Fix 4: Phase 7 — Centralized type-aware store utility

Create a `pub(crate) fn emit_typed_store` that encapsulates the four-step pattern
from Fix 1, so every LHS store path (Identifier, AddrOf, Deref, ListIndex,
TupleDestructure) routes through the same code.

```rust
/// Store `val` (a TypedRegister) to a state field identified by `name`.
/// Looks up the field's LLVM type, converts the value via ensure_typed_value,
/// and emits the correctly-typed store instruction.
pub(crate) fn emit_typed_store(
    &mut self,
    out: &mut String,
    indent: &str,
    name: &str,
    val: &TypedRegister,
) {
    let Some(&idx) = self.ctx.field_index_map.get(name) else { return; };
    let ty = self.ctx.field_types[idx].clone();
    let sr = self.fun.state_reg_name.clone();
    let p = self.emit_state_gep(out, indent, "ts", &sr, idx);
    let briev_ty = self.ctx.field_briev_types.get(idx).cloned();
    let tv = self.ensure_typed_value(out, indent, &ty, &val.name, briev_ty);
    writeln!(out, "{}store {} {}, ptr {}, align {}", indent, ty, tv, p, self.align_of(&ty)).ok();
}
```

All LHS paths in `emit_stmt.rs` (Identifier at line 698, AddrOf at line 699,
Deref at line 712, TupleDestructure at lines 791-797, and ListIndex at lines
740-743) call this instead of duplicating the pattern.

## Benchmark Impact

| Benchmark | Before | After |
|-----------|--------|-------|
| float_math | broken (`store i64 %float`) | correct float store |
| float_math_nonzero | broken | correct |
| nbody_newton | broken | correct |
| nbody_sqrt | broken | correct |
| nbody_sqrt_idio | broken | correct |
| kalman_filter_runtime | broken | correct |
| fannkuch_redux | 0.01x (false, precomputed) | proper runtime loop |
| mandelbrot | 0.07x (false, precomputed) | proper runtime loop |
| queue_drain | precomputed | proper runtime loop |
| queue_drain_idio | duplicate %t10 | correct |
| interval_step | 0x (false, precomputed) | proper runtime loop |

## Test Plan

1. `cargo test --lib` — all 1444+ tests pass
2. `bash benchmarks/build_and_bench.sh --runtime` — all benchmarks compile to valid IR
3. `bash benchmarks/build_and_bench.sh --correctness` — all correctness checks pass
