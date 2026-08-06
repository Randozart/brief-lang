# Phase 5 (Slice 1) — Op Elaboration: declared `op` bindings lower to their functions

**Date:** 2026-08-06
**Status:** Implementation plan
**Source:** `docs/plans/2026-08-05-implement-normative-language-spec.md` §11 (Phase 5, §11.2)

---

## 0. Executive Summary

The typechecker resolves every binary operator to an `OpBinding` (`Intrinsic`
or `Function`) — a declared `op Add: my_add(#L, #R)` on a type body authorizes
`T + T` — but **discards the resolved binding**. Lowering (codegen + interpreter)
re-dispatches by operand type, so a declared custom op produces wrong/garbage
native code instead of calling `my_add`. This plan closes that gap: when a
`BinaryOp` resolves to `OpBinding::Function(fn)`, a frontend pass rewrites it
into `Expr::Call(fn, [l, r])` so both codegen (`emit_user_call`) and any later
consumer invoke the declared function.

Protocol intrinsics (`AddI64#` etc.) are **not** rewritten: their existing
category-dispatch lowering is already correct and benchmark-stable. The rewrite
is additive — no existing program (no custom `op` declarations in
`lib/`/`benchmarks/`) changes behavior.

## 1. Investigation findings

- `type_universe/operators.rs::protocol_binding(category, op)` → the ONLY
  hardcoded operator knowledge, keyed by protocol category (never type name) —
  rule 18 compliant.
- `TypecheckContext::{type_declares_op, has_cross_type_overload,
  protocol_binding_for}` authorize mixed/custom ops during inference but return
  only `bool`/`Option<OpBinding>` whose target is dropped at the call site
  (`arithmetic_result_ty` at `typechecker/mod.rs:1101` matches `Some(_)` and
  returns `lhs_ty`).
- `regular_bindings[type]` holds `OperatorBinding { name, protocol_variant,
  fn_name, ... }` — the declared op's target function — parsed from type-body
  `op Name: fn(#L, #R)` (`parser/definitions.rs:1476`).
- Existing mutating rewrite passes set the pattern:
  `analysis/string_concat.rs::rewrite_plus_concat(items, universe)` and
  `analysis/boundary_marshalling.rs::rewrite_boundary_marshalling(...)`.
- The interpreter's `eval_call` applies closures (Slice E) but not user `defn`s
  (pre-existing "simplified Call model"); a rewritten `Call(my_add, ...)`
  through the reactor will hit `UndefinedVariable`. This is a PRE-EXISTING
  interpreter limitation (any user-fn call today fails the same way), not a
  regression from this slice — tracked as follow-up work.

## 2. Design

### 2.1 Resolution (typechecker)

Add to `TypecheckContext`:

```rust
/// Resolve a binary op to its semantic OpBinding (the chain
/// arithmetic_result_ty used, but returning the binding instead of dropping
/// it). Declared ops (type-body `op Name: fn(...)`) return
/// `OpBinding::Function(fn)`; protocol bindings return the intrinsic.
fn resolve_binary_op_binding(
    &self, kind: &BinaryOpKind, lhs: &Type, rhs: &Type,
) -> Option<OpBinding>
```

Chain (same as today, in order):
1. `type_declares_op_binding(lhs, op, rhs)` → the declared `Function` target
   (NEW helper returning the fn name, walking `type_parents`).
2. `protocol_binding_for(op, lhs)`.
3. `get_operator_intrinsic(universe, rune, lhs)`.
4. `protocol_binding_for(op, rhs)` + `get_operator_intrinsic(universe, rune, rhs)`.

`arithmetic_result_ty` refactored to use it (keeps one resolution chain — DRY).

### 2.2 Elaboration pass (typechecker post-check)

```rust
/// Rewrite every BinaryOp whose resolved binding is a declared Function into
/// `Call(fn, [l, r])`. Intrinsic bindings stay as BinaryOp (existing lowering
/// is category-correct). Runs after check_program so types are known.
pub fn elaborate_ops(items: &mut [TopLevel], ctx: &TypecheckContext)
```

- Mutating statement/expr walker (models `rewrite_stmt_idents` +
  `string_concat::rewrite_expr`). Replaces a node's `Expr::BinaryOp` with
  `Expr::Call(fn_name, vec![l, r], None)` when the binding is a Function.
- Recurses into block bodies, if/guard/match arms, foreach, struct literals,
  method args, lambda bodies, `Within`, etc.
- No pipeline change needed: `check_program` (or its caller) invokes it once
  the typed AST is available; codegen + interpreter receive the rewritten form.

## 3. Tests

- **Typechecker unit**: `resolve_binary_op_binding` on a declared `op Add`
  returns `Function("my_add")`; on `Int + Int` returns `Intrinsic("AddI64#")`.
- **Backend IR**: a program with a custom type + declared `op Add` emits
  `call i64 @my_add(...)` and no native `add` for the custom op.
- **Native end-to-end**: a `.bv` program with `MyNum` + `op Add: my_add`
  compiles and prints `my_add`'s result (via scratch build + run).
- **Interpreter**: documented limitation — custom-op programs need user-fn call
  support (follow-up). Existing 1603 tests must stay green (additive rewrite).

## 4. Benchmark Baseline (rule 11)

Measured at commit `75f02aef` (Phase 7/8 pull-in; identical to the Phase 17
baselines): 36/36 runtime benchmarks **MATCH**, `bridge_glue` SKIP,
`bridge_multi` PASS. Expectation: unchanged — no benchmark declares a custom
`op`, so the rewrite never fires on them.

## 5. Docs to update (same commit)

- `docs/plans/2026-08-05-spec-implementation-status.md` §15 row: note
  declared-op lowering shipped.
- `docs/architecture/agent-reference.md` or `overview.md` op-resolution note:
  declared ops lower to their function; protocol intrinsics lower by category.
- This plan's tracker below.

## 6. Risks

| Risk | Mitigation |
|---|---|
| Rewrite perturbs benchmark emission | Only `Function` bindings rewrite; benchmarks declare none — verify 36/36 after |
| Mutating walker misses a nesting site | Mirror the statement/expr shapes `string_concat::rewrite_expr` covers; test a nested case |
| Interpreter sees `Call(my_add)` and errors | Pre-existing user-fn limitation, documented; not a regression |
| Duplicate resolution chain drifts | `resolve_binary_op_binding` becomes the single chain; `arithmetic_result_ty` delegates |

## 7. Tracker

- [x] `resolve_binary_op_binding` + `type_declares_op_binding` (typechecker) — 2026-08-06
- [x] `elaborate_ops` mutating pass wired after check_program — 2026-08-06
- [x] Tests (typechecker / backend IR / native e2e) — 2026-08-06
- [x] Benchmarks + Praetor + commit — 2026-08-06

## 8. Delivered (2026-08-06)

- `TypecheckContext::resolve_binary_op_binding` — the single resolution chain
  (declared variant op → `Function(fn)`; protocol → intrinsic), shared by
  `arithmetic_result_ty` and the elaboration pass (was split + target dropped).
- `TypecheckContext::type_declares_op_binding` — returns the declared op's fn
  name; coverage mirrors `type_declares_op` (variant/param forms only). A
  colon-form `op Add: add(#L, #R)` is documentation and is NOT rewritten (the
  bootstrap `Int` binding never becomes a call to the undefined `add`).
- `elaborate_ops(items, universe, env)` — post-typecheck pass that rewrites
  `BinaryOp` → `Expr::Call(fn, [l, r])` when the resolved binding is a
  declared Function; tracks let bindings so operand types resolve (mirrors
  `infer_statement`). Intrinsic bindings stay as BinaryOp.
- `CheckEnv` struct + `make_typecheck_context` helper — bundle the 14
  pre-collected maps shared by `check_top_level` and `elaborate_ops`
  (Praetor rule 4).
- `check_program` now takes `&mut [TopLevel]` and runs elaboration when the
  program typechecks; call sites in compile.rs / library.rs updated.
- Native verification: `op Add(Int): my_add(#L, #R)` on `MyNum` compiles
  `MyNum + Int` to `call i64 @my_add` → prints 14 (my_add(4,2)=4*3+2) instead
  of the previous plain Int add (6).
- Follow-up (pre-existing, not a regression): the interpreter's `eval_call`
  cannot apply user `defn`s (simplified Call model) — a custom-op program
  evaluated by the reactor would hit `UndefinedVariable`. Interpreter user-fn
  dispatch is tracked as separate work.
