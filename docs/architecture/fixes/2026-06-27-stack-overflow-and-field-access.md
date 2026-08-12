# 2026-06-27: Stack Overflow & Struct Field Access Fixes

## Bug 1: Debug Build Stack Overflow

### Symptoms

`briev build officina.bv` in debug mode crashes with `fatal runtime error: stack overflow` at `build_budget_plan` entry.

### Root Cause

The default Linux stack is 2MB. Debug builds generate ~4x larger stack frames
than release builds (debug info, no inlining, no tail-call optimization). The
cumulative call chain depth from the compilation pipeline (parser → imports →
typechecker → analysis → LLVM backend → LLVM toolchain) exceeds 2MB for large
projects like officina-cli (14 modules, ~30 state variables, ~10 transactions,
complex expression ASTs).

This is NOT caused by any single recursive function — it's the cumulative depth
of the entire pipeline. The deepest individual recursion was in the region
analyzer's `substitute_expr` (converted to iterative in this session).

### Fixes Applied

1. **`.cargo/config.toml`**: Set linker stack size to 8MB for debug builds via
   `-C link-args=-Wl,-z,stack-size=8388608`. This matches the Linux `ulimit -s`
   default and gives headroom for any project.

2. **8 recursive functions converted to iterative** in `src/analysis/region.rs`:
   - `substitute_expr` — post-order traversal with explicit work/results stacks
   - `collect_identifiers` — boolean walk with `Vec<&Expr>` work stack
   - `collect_var_ids` — identifier collection with `Vec<&Expr>` work stack
   - `expr_has_call` — boolean walk with early return on `Expr::Call`
   - `count_statements_recursive` — counting walk with `Vec<&Statement>` stack
   - `has_ffi_or_terminator_stmt` — boolean walk with `Vec<&Statement>` stack
   - `has_ffi_or_trigger_stmt` — boolean walk with `Vec<&Statement>` stack
   - `eval_expr_simple` — arithmetic evaluation with two-phase `Frame` stack

### Verification

```bash
cargo build                     # default debug build
briev build officina.bv          # no stack overflow
cargo build --release           # release build
briev build officina.bv          # no stack overflow
cargo test --lib                 # 1300 tests pass
```

## Bug 2: Struct Field Access via `ListIndex`

### Symptoms

`briev build officina.bv` panics at `emit_expr.rs:2999`:
```
emit_expr: FieldAccess: field 'slot_count' not found on object
```

### Root Cause

`Expr::ListIndex` in the LLVM backend always returned `TypedRegister { ty: Type::Int }`,
even when the list's element type was a struct like `UnderstandRule`. When
`let rule = rules[i];` was emitted, `let_binding_types["rule"]` stored
`Type::Int` instead of `Type::Custom("UnderstandRule")`. Subsequent field access
`rule.slot_count` failed because `FieldAccess` couldn't find the struct type.

The issue is in `src/backend/llvm/emit_expr.rs:2672`: after emitting the
pointer arithmetic for `ListIndex`, the code returned `Type::Int` regardless
of the list's element type.

### Fix Applied

In `src/backend/llvm/emit_expr.rs:2690` (ListIndex handler):

After emitting the ListIndex GEP+load, extract the element type from the list
expression's type. Check both `list_val.ty` (the emitter's result type) and
the original variable's type in `let_binding_types` (which preserves the
original `List<T>` type before the backend transforms it to `Ptr`).

If the list type is `Applied("List", [el_ty])`, return the element type so
downstream `FieldAccess` can resolve struct fields.

### Verification

```bash
briev build officina.bv          # no field access panic
```
