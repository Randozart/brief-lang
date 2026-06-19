# Macro System — Remaining Gaps

**Date:** 2026-06-18  
**Updated:** 2026-06-19  
**Status:** ✅ All gaps resolved  
**Context:** Post-implementation audit after M1–M6 of the macro/template system.

---

## Resolution Notes (2026-06-19)

| Gap | Resolution | Details |
|-----|-----------|---------|
| 1 — Statement-level expansion | ✅ Fixed | `expand_template_in_stmt` handles all return types (Block/Stmt/Expr) in `expand.rs:89-113` |
| 2 — Nested macro calls | ✅ Fixed | `expand_macro_calls_in_items` recurses into Definition/Transaction bodies via `expand_macro_in_stmts` (added `expand_macro_in_stmts` at `expand.rs:291-305`). Macro calls inside guarded blocks, let bindings, term values all resolved. |
| 3 — Template control flow | ✅ Fixed | `expand_template` does AST @-substitution + interpreter execution (guards evaluated, term return captured) in `template.rs:31-44` |
| 4 — `macro_.rs` empty | No action needed | Cosmetic placeholder, no bug. |
| 5 — Integration tests | ✅ Fixed | Three `.bv` source-parsing tests added: `test_integration_macro_expansion_from_source`, `_in_defn_body`, `_in_txn_body` at `expand.rs:673-723` |

---

## Gap 1: Template expansion at statement level, not expression level

### Symptom

When `$template` returns a `Stmt` or `Block` value, the expander wraps the result in
`Expr::QuoteBlock` and replaces the expression inside the original
`Statement::Expression`. Downstream passes (typechecker, codegen) treat
`QuoteBlock` as `unreachable!()` → **panic**.

### Root Cause

`expand_template_call_in_expr` in `expand.rs:186` is an expression-level
function — it replaces `Expr::TemplateCall` with a new `Expr`. But templates
that emit statements (e.g., `template unless(cond, body) -> Stmt`) need to
expand at the _statement_ level: the whole `Statement::Expression(Expr::TemplateCall)`
should be replaced by the emitted statements.

### Fix

Add a new function `expand_template_call_in_stmt` that handles
`Statement::Expression(Expr::TemplateCall { .. })` by:

1. Evaluating the template to get a `Value`
2. If `Value::Block(stmts)` or `Value::Stmt(stmt)`: replace the entire
   `Statement` at the `TopLevel::Statement(stmt)` level (not just the inner `Expr`)
3. Lift substitution up to `expand_template_calls` in the TopLevel walker

The `expand_template_call_in_expr` fallback handles templates that return
`Expr` values (used inside `let x = $template(...);`).

### Files
- `src/features/macros/expand.rs` — add `expand_template_call_in_stmt`, update `expand_template_calls`

---

## Gap 2: Macro calls inside nested statements not expanded

### Symptom

Only top-level `Statement::Expression(Expr::MacroCall)` is expanded.
A macro call inside a guarded block like `[cond] { $!foo(); }` or in a let
binding `let x = $!bar();` is silently ignored.

### Root Cause

`has_macro_call_in_stmt` in `expand.rs` only checks
`Statement::Expression(Expr::MacroCall { .. })`. It does not recurse into
`Guarded`, `Let`, `Foreach`, `SyncBlock`, `Term`, etc. The walker in
`expand_macro_calls` uses this check to decide whether to expand.

### Fix

Make `expand_macro_calls` recursive:

1. Replace `has_macro_call_in_stmt` with a recursive walk that descends into
   `Guarded { statements }`, `Let { expr }`, `Term { values }`, etc.
2. When a macro call is found at depth, remove that statement, expand, and
   splice the result at the correct position in the parent statement list.
3. Handle `MacroCall` inside expressions (e.g., `let x = $!foo();`) by
   evaluating the call and substituting the result expression.

### Files
- `src/features/macros/expand.rs` — recursive traversal in `expand_macro_calls`

---

## Gap 3: Template body control flow not executed

### Symptom

`expand_template` does @-substitution on the template body but does not
evaluate guards or control flow. Both branches of `[cond] { ... }` are
substituted and both appear in the output.

### Root Cause

The `expand_template` function in `template.rs` walks the AST and substitutes
`@`-interpolation markers, but every `[guard] { body }` is copied verbatim
without evaluating the guard condition.

### Fix

Switch `expand_template` from AST-substitution to interpreter execution,
matching how `expand_macro` already works:

1. Create a sandboxed Interpreter
2. Bind arguments as state variables (same as `expand_macro`)
3. Execute each statement through `interpreter.exec_stmt()`
4. Check `return_value` after each statement
5. The interpreter naturally handles `[guard] { body }` by evaluating the
   guard condition

This unifies the template and macro execution paths — both use the
interpreter. The `@`-interpolation markers become proper `Expr::Interpolate`
values that the interpreter evaluates to their bound argument AST.

### Files
- `src/features/macros/template.rs` — replace `expand_template`
- `src/features/macros/expand.rs` — templates now matched same as macros
- `src/features/macros/hygiene.rs` — verify still correct after change

---

## Gap 4: `macro_.rs` is empty

### Status

Not a bug. `macro_.rs` was created in M1 as a placeholder for future
refactoring. All macro expansion logic currently lives in `template.rs`
and `expand.rs`.

### When to fill

If the macro expansion logic grows beyond what fits in the existing files
(respecting Praetor's 100-line limit), `macro_.rs` would hold the
macro-specific execution path (separate from template's interpreter-based
path).

Currently, no action needed.

---

## Gap 5: No integration-level end-to-end test

### Symptom

All tests are unit-level (AST construction, parser round-trips, span
propagation). There is no test that:
- Parses a `.bv` file containing a `template` or `macro` definition + call
- Runs the full compilation pipeline (Phase 1a/1b → TypeChecker → analyze)
- Verifies the expanded AST matches expected output

### Fix

Add an integration test in `src/features/macros/expand.rs` or a new
`tests/` directory that:

1. Creates a `Program` with a `TemplateDef` and a `TemplateCall`
2. Calls `expand_templates` + `validate_no_compile_time_intrinsics`
3. Asserts the resulting program has no `TemplateCall` nodes
4. Asserts the expanded AST contains the expected generated code

Same for macros: define a `MacroDef` whose body uses `compile#()` and
`gensym#()`, call `expand_macros`, verify no `MacroCall` nodes remain.

### Files
- `src/features/macros/expand.rs` — add integration-style tests

---

## Prioritization

| Gap | Impact | Effort | Priority |
|-----|--------|--------|----------|
| 1 — Statement-level expansion | Compiler panic on Stmt-returning templates | 1 day | **Critical** |
| 3 — Template control flow | Wrong output for guarded templates | 1 day | **High** |
| 2 — Nested macro calls | Macros inside guards silently skipped | 0.5 day | **High** |
| 5 — Integration tests | No regression safety net | 0.5 day | **Medium** |
| 4 — `macro_.rs` empty | Cosmetic | 0 | None |
