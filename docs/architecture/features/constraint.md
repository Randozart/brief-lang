# Constraint Unification — `<: [expr]` Runtime Enforcement

**Date:** 2026-06-21  
**Phase:** B1/B2/B3  
**Status:** Complete

## Design

Constraint unification replaces the old `RangeConstraint` and `Type::ContractBound`
with a single `Option<Box<Expr>>` on `Statement::Let` and `StateDecl`. The implicit
variable `_` is bound to the value at runtime. Constraints come from two sources:

| Source | Syntax | Storage | Enforcement |
|--------|--------|---------|-------------|
| **Inline** | `let x <: [expr]` | `Statement::Let.constraint` | `eval_constraint()` / `emit_guard_check()` |
| **TypeDef guard** | `type Foo <: Int { [expr]; }` | `ResolvedType.guards` | `check_type_guards()` / guard iteration |

## Syntax

```brief
// Inline constraint on let binding (no type annotation required)
let x <: [_ > 0] = expr;

// Inline constraint with type annotation
let x: Int <: [_ > 0] = expr;

// Range sugar (desugars to _ >= lo && _ <= hi)
let x <: [0..100] = expr;

// TypeDef body guard (enforced on all variables of that type)
type Positive <: Int {
  [_ > 0];
};
let p: Positive = 42;
```

## Typechecking

- Constraints are arbitrary expressions of type `Bool`
- `_` is typed as the value's declared type
- TypeDef body constraints are validated during `TypeUniverse::build()`
- No new syntax in strict tiers (`.sbv`, `.srbv`) — `[pre][post]` remains

## Evaluation (Interpreter)

### B2: `eval_constraint(&mut self, value, constraint)`
- File: `src/interpreter.rs:1428`
- Temporarily binds `_` to `value` in `self.state`
- Evaluates the constraint expression
- Restores prior `_` binding
- Returns `Ok(())` if result is `Value::Bool(true)`, else `Err(TypeMismatch)`

### B3: `check_type_guards(&mut self, ty, value)`
- File: `src/interpreter.rs:1394`
- Looks up `ResolvedType.guards` from `TypeUniverse` for the annotated type
- Clones guards list to avoid borrow conflicts
- Iterates calling `eval_constraint()` for each guard

### Call sites in `Statement::Let` handler:
```rust
// Inline constraint (line 1211)
if let Some(constraint_expr) = constraint {
    self.eval_constraint(&value, constraint_expr)?;
}
// TypeDef guards (line 1215)
if let Some(ann_ty) = ty {
    self.check_type_guards(ann_ty, &value)?;
}
```

## Codegen (LLVM Backend)

### B2: `emit_guard_check(out, indent, var_name, guard)`
- File: `src/backend/llvm/emit_stmt.rs:718`
- Looks up LLVM register for `var_name` from `let_bindings`
- Temporarily binds `_` to that register
- Evaluates guard expression via `emit_expr()`
- Converts result to `i1` via `as_bool_reg()`
- Emits `br i1 %result, label %ccN, label %cpN`
- `cpN` block: `call void @llvm.trap()` + `unreachable`
- `ccN` block: continues execution
- Restores prior `_` binding

### B3: TypeDef guard iteration in `Statement::Let` handler
- File: `src/backend/llvm/emit_stmt.rs:259`
- Looks up `type_universe.types.get(type_name)?.guards`
- Clones guards to avoid borrow conflict with mutable `self`
- Calls `emit_guard_check()` for each guard

## Kani Formal Verification

Full-group only (uses formatting, heap, loops). See `src/interpreter.rs`
and `src/backend/llvm/emit_stmt.rs` for `#{cfg(kani)}` harness blocks.

## Files Touched

| File | Change |
|------|--------|
| `src/ast.rs` | `RangeConstraint` removed; `Type::ContractBound` removed; `Statement::Let.constraint: Option<Box<Expr>>`; `StateDecl.constraint: Option<Box<Expr>>` |
| `src/parser.rs` | `parse_constraint_expr()`; range sugar desugaring; `Token::Underscore` → `Expr::Identifier("_")` |
| `src/type_universe.rs` | `ResolvedType.guards: Vec<Expr>` |
| `src/interpreter.rs` | `eval_constraint()`; `check_type_guards()`; enforcement in `Statement::Let` |
| `src/backend/llvm/emit_stmt.rs` | `emit_guard_check()`; TypeDef guard iteration |
| `src/backend/llvm/emit_toplevel.rs` | `@llvm.trap()` declared |
| `src/proof_engine.rs` | Removed `ContractBound` match arms |
| `src/memory_spec.rs` | Removed `ContractBound` match arms |
| `src/annotator.rs` | Removed `ContractBound` match arms |
| `src/analysis/region.rs` | `extract_range_from_constraint()` helper |
