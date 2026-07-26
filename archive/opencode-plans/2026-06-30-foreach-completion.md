# Foreach Statement — Backend Completion

**Date**: 2026-06-30
**Author**: OpenCode

## Background

The `foreach(item in list) { body }` statement was implemented with full
parser, AST, desugaring, interpreter, and LLVM backend support, but three
gaps remained:

1. **Type checker**: `check_statement` has no `Foreach` arm — list expression
   is never validated, item variable never bound in scope.
2. **CIRCT backend**: `Foreach` silently falls through to `_ => {}`.
3. **LLVM codegen**: Item type hardcoded to `Type::Int`.
4. **StmtTypecheck trait**: Stub `Ok(())` no-op.

## Work Items

### 1. Type Checker (`src/typechecker.rs`)

Add `Statement::Foreach { item, list, body, modifiers }` arm in
`check_statement` (~line 1771):

- Infer type of `list` expression via `self.infer_expression(list)`
- Assert it's `Type::List(Some(element_type))` — emit type error if not
- Declare `item: element_type` in local scope (`self.locals.insert(...)`)
- Recursively `self.check_statement(s)` for each body statement
- Remove `item` from scope after body

### 2. CIRCT Backend (`src/backend/circt.rs`)

Hardware can't do dynamic iteration. Two cases:

- **Constant list**: evaluate at compile time → unroll body N times
- **Dynamic list**: emit MLIR comment warning, skip body

Add `try_eval_list_size(expr)` helper that returns `Option<usize>` for
literal lists and compile-time-constant expressions.

### 3. LLVM Codegen — Item Type Generalization (`src/features/stmt/foreach.rs`)

Line 85: `let elem_ty = Type::Int;` → determine dynamically from the
list expression's type. Use the `TypedRegister`'s element type and
`TypeConverter` to get the correct LLVM type.

### 4. StmtTypecheck Trait (`src/features/stmt/foreach.rs:14-18`)

Replace `Ok(())` stub with actual validation that delegates to the type
checker's infrastructure.

### 5. Tests

- Type checker: foreach with `List<Int>` (pass), foreach with non-list (fail)
- CIRCT: foreach with constant list (unrolled output), dynamic list (warn)
- LLVM: foreach with `List<Float>` (correct GEP in generated IR)

## Verification

- `cargo test --lib` — all tests pass
- `cargo build` — no warnings
- Existing foreach examples still compile and run
