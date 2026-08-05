# Remaining Work: Example Files, Type-Checker Integration, Submodule Cleanup

**Date:** 2026-06-29
**Status:** Draft
**Prerequisite:** Phases 0-7B complete (back-end restructured, type system data-driven)

## Overview

The refactoring has produced a clean, data-driven backend with zero type-specific
match arms. However, three categories of work remain:

1. **Make example files work end-to-end** — the operator→intrinsic syntax parses
   and stores in the universe, but the type-checker and codegen don't use it yet
2. **Submodule cleanup** — `expr/rest.rs` (~2,400 lines) should be split into
   focused submodules; two `v.clone()` warnings remain
3. **Documentation** — mark Phase 7B as complete in the main plan

This plan covers all three.

---

## Part 1: Make Example Files Actually Compile

### Problem

The operator→intrinsic pipeline is partially wired:

```
Parser:  op Add(Float4) -> Float4 = my_add;    → TypeDefBody.operators ✅
Universe: resolves operators → ResolvedType.operators                  ✅
Type-checker: binary_op_type_scalar() ignores universe                 ❌
Codegen: emit_binop() ignores universe                                 ❌
```

When a user writes `let z = x + y` where both are `Float4`:

1. **Type-checker** (`infer_expression` at typechecker.rs:2038):
   - Old-style: calls `binary_op_type_scalar()` which has a `match (l_ty, r_ty)`
     — falls through to `_ => Type::Custom("unknown")` for custom types
   - New-style: calls `infer_expression` then falls through to `Type::Int`

2. **Codegen** (`emit_binop` at helpers.rs:858):
   - Checks `a.ty == Type::Float64`, `a.ty == Type::Float`,
     `a.ty.is_integral()` — none match custom types
   - Falls through to generic `i64` path → treats custom type as opaque i64

### Fix: Three Integration Points

#### 1a. Type-checker: `binary_op_type_scalar` (typechecker.rs:3044)

**Change:** In the `_ =>` fallthrough arm, before returning `Type::Custom("unknown")`,
check if either operand type is registered in the universe:

```rust
_ => {
    // Phase 7B: If either operand is a type registered in the universe,
    // preserve its type through inference. The codegen resolves the
    // operator→intrinsic mapping during emit_binop.
    if let Some(ref universe) = self.type_universe {
        let l_key = l_ty.universe_key();
        if universe.types.contains_key(l_key) {
            return l_ty.clone();
        }
        let r_key = r_ty.universe_key();
        if universe.types.contains_key(r_key) {
            return r_ty.clone();
        }
    }
    Type::Custom("unknown".to_string())
}
```

**Why this works:** Universe lookup is O(1) HashMap. The key string (`"Float4"`)
is the same one used in `ResolvedType.name`. `types.contains_key()` is a single
hash lookup — negligible cost.

#### 1b. Type-checker: `Expr::BinaryOp` path (typechecker.rs:2794)

**Change:** In the final `else` clause, before falling back to `Type::Int`,
check the universe:

```rust
else if let Some(ref universe) = self.type_universe {
    let l_key = l_ty.universe_key();
    if universe.types.contains_key(l_key) { l_ty }
    else {
        let r_key = r_ty.universe_key();
        if universe.types.contains_key(r_key) { r_ty }
        else { Type::Int }
    }
}
```

#### 1c. Codegen: `emit_binop` (helpers.rs:858)

**Change:** Before the type-specific dispatch (Float64, Float, integral checks),
add a universe query for custom types:

```rust
pub(crate) fn emit_binop(&mut self, out: &mut String, indent: &str,
                          l: &Expr, r: &Expr, int_op: &str, float_op: &str) -> TypedRegister {
    // ── Phase 7B: Custom type operator dispatch ──────────────
    // If either operand has a universe-registered type with operator
    // mappings, emit the operator's implementation expression.
    let a = self.emit_expr(out, l, indent);
    if let Type::Custom(type_name) = &a.ty {
        if let Some(ref universe) = self.ctx.type_universe {
            let rune = op_str_to_rune(int_op, float_op);
            let r_expr = self.emit_expr(out, r, indent);
            let r_key = r_expr.ty.universe_key();
            if let Some(op) = universe.resolve_operator(type_name, rune, Some(r_key)) {
                return self.emit_operator_call(out, indent, &a, &r_expr, op);
            }
        }
    }
    let b = if int_op != "add" { /* ... original code ... */ };
    // ... rest of emit_binop unchanged ...
}
```

#### 1d. New helper: `emit_operator_call` (helpers.rs, new function)

Takes a resolved `OpDeclaration` and emits its implementation:

```rust
fn emit_operator_call(&mut self, out: &mut String, indent: &str,
                       a: &TypedRegister, b: &TypedRegister,
                       op: &OpDeclaration) -> TypedRegister {
    match &op.implementation.as_ref() {
        // Simple intrinsic call: call the intrinsic with both operands
        Expr::IntrinsicCall { intrinsic, args } => {
            emit_intrinsic_unary(self, out, indent, &a.name, intrinsic, &b.name)
        }
        // Identifier → function call: call i64 @name(i64, i64)
        Expr::Identifier(name) => {
            let v = format!("%t{}", self.fun.next_reg());
            writeln!(out, "{}{} = call i64 @{}(i64 {}, i64 {})",
                     indent, v, name, a.name, b.name).ok();
            TypedRegister { name: v, ty: a.ty.clone() }
        }
        // Defn call: emit inline body
        _ => {
            // Fall back to standard i64 operation
            let v = format!("%t{}", self.fun.next_reg());
            writeln!(out, "{}{} = add i64 {}, {}", indent, v, a.name, b.name).ok();
            TypedRegister { name: v, ty: a.ty.clone() }
        }
    }
}
```

#### 1e. Map `int_op`/`float_op` strings to `OpRune`

```rust
fn op_str_to_rune(int_op: &str, _float_op: &str) -> OpRune {
    match int_op {
        "add" => OpRune::Add, "sub" => OpRune::Sub,
        "mul" => OpRune::Mul, "sdiv" | "udiv" => OpRune::Div,
        "srem" | "urem" => OpRune::Mod,
        _ => OpRune::Add, // fallback
    }
}
```

### Verification

After the four changes above, the example file `examples/inop-float4.bv` should:
1. ✅ Parse without errors
2. ✅ Pass type-checking (custom types preserved through inference)
3. ✅ Generate LLVM IR with the correct function calls

---

## Part 2: Submodule Cleanup and Warnings

### 2a. Fix `v.clone()` warnings (rest.rs:1883, 1919)

`v` is `&str`, calling `.clone()` copies the reference (no-op). Fix: `v.to_string()`.

```rust
// BEFORE:
let popped = v.clone();
return TypedRegister { name: popped.to_string(), ty: Type::Int };

// AFTER:
let popped = v.to_string();
return TypedRegister { name: popped, ty: Type::Int };
```

### 2b. Split `expr/rest.rs` into focused submodules (deferred)

`rest.rs` is ~2,400 lines containing ~15+ handler groups. The handlers use
fallthrough returns (no explicit `return` before the function's final
`TypedRegister { name: v, ty: Type::Int }`), which prevents clean extraction.
Before splitting, each handler needs explicit `return` statements.

| Submodule | Handlers | Lines | Status |
|-----------|----------|-------|--------|
| `call.rs` | Call, CellCall | ~300 | Blocked: fallthrough |
| `field.rs` | FieldAccess, StructInstance, ObjectLiteral | ~200 | Blocked: fallthrough |
| `control.rs` | Match, PatternMatch, Within | ~500 | Blocked: fallthrough |
| `arrow.rs` | ArrowMut, ArrowDiscard, ArrowTransfer | ~400 | Blocked: fallthrough |
| `slice.rs` | Slice, MultiSlice, ListIndex | ~300 | Blocked: fallthrough |
| `projection.rs` | Projection, SubtypeProjection | ~150 | Blocked: fallthrough |
| `misc.rs` | Cast, IsType, FromCheck, Like, Block, Concat | ~200 | Blocked: fallthrough |

**Strategy:** For each extraction, first audit the handler, then replace all
fallthrough paths with explicit `return`, then extract into a submodule.

### 2c. Documentation update

- Mark Phase 7B as complete in `docs/plans/2026-06-29-llvm-backend-refactoring.md`
- Update the success criteria table

---

## Part 3: Future Work (After Cleanup)

### 3a. Example files with full end-to-end compilation

Once the type-checker and codegen changes are applied, test all three examples:

```
cargo run --bin briv-compiler -- examples/inop-float4.bv
cargo run --bin briv-compiler -- examples/inop-custom-types.bv
```

Expected: generates LLVM IR with proper type-aware operations.

### 3b. Parser support for complex LLVM type expressions

Currently `LLVMType = <4 x float>;` fails to parse because `<` is not a valid
identifier start. Options:
1. Allow string values: `LLVMType = "<4 x float>";` (already supported)
2. Allow angle-bracket expressions in `parse_type_expr_for_typedef`
3. Store LLVM type names as metadata strings (simplest)

---

## Timeline

| Task | Est. Time | Risk | Verification |
|------|-----------|------|-------------|
| 1a. Type-checker type preservation | 15 min | Low | `cargo test --lib` — 1328 pass |
| 1b. Type-checker BinaryOp path | 15 min | Low | `cargo test --lib` — 1328 pass |
| 1c. emit_binop universe query | 30 min | Medium | Generate IR, check output |
| 1d. emit_operator_call helper | 30 min | Medium | Example files compile |
| 1e. op_str_to_rune mapping | 10 min | Low | Match coverage |
| **Total Part 1** | **~2 hours** | | |
| 2a. v.clone() warnings | 5 min | None | `cargo build` no warnings |
| 2c. Documentation update | 15 min | None | `git diff` |
| **Total Part 2** | **~20 min** | | |
| 3. Example file verification | 20 min | Low | `cargo run` |
