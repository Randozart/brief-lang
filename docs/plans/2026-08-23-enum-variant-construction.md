# Enum Variant Construction — Sub-Plan

**Date:** 2026-08-23
**Parent:** `docs/plans/2026-08-22-spec-conformance.md` (surfaced by the Phase 10 sweep)
**Status:** active

## Problem

Enum variant PATTERNS exist (`Result::Ok(v) =>` parses and matches), but
variant CONSTRUCTION does not: `term Ok(x)` is a call to an undefined fn.
This single gap blocks ~40 sweep files — the entire Result/Option error-
handling ecosystem (lib/std/result.bv, option.bv, json.bv, process.bv's
`.is_ok()` chain, examples/error-handling.bv) is dead surface written in
the removed `uni` dialect because modern construction was never available.

## Design (binding)

| Question | Decision |
|---|---|
| Grammar | **Bare variant calls**: `Ok(v)`, `Err(e)`, `Some(v)`, `None`. Reuses `Expr::Call` — zero new AST. Qualified `Result::Ok(v)` deferred until `::` path syntax exists (lexer lacks DoubleColon). |
| Resolution | Typechecker registry `variant_defs: variant_name → enum_name`, built from every TypeDef carrying `__variant_*` slots. A Call whose name resolves to a variant CONSTRUCTS; if two enums declare the same variant name → ambiguity error at use. User fns shadow variants (fn lookup wins). |
| Typing | Result type = `Applied(enum_name, type_args)`. Payload types bind leading type params positionally (`Ok(5)` under `Result<T,E>` ⇒ T=Int); remaining params unify against `ctx.current_output_type` when present (nearly all stdlib sites return a fully-known Result); otherwise they default to the payload's own inference or Void with a diagnostic naming the fix. Zero-payload variants (`None`) take ALL params from context; bare `None` outside any Result-typed context errors naming it. |
| Interpreter | `eval_call` fallback (after closures/functions): variant registry hit → `Value::sum(variant_name, evaluated)`. Registry mirrors into load_program like objs. Pattern side already matches `Sum.name == variant_name` ✓. |
| LLVM | STAGED via capability matrix: new flag `enum_constructors` (false everywhere incl. LLVM) until lowering lands — same precedent as ports/cells. Enum VALUES already have no compiled lowering either, so nothing regresses. BUGS.md tracks 5d. |
| Migrations unlocked | result.bv + option.bv: rewrite `uni` blocks as exhaustive match arms using native constructors; delete the corrupted fragments (option.bv has truncated defs). json.bv/process.bv chains follow. error-handling.bv example re-checked. |

## Work items

1. Registry + typechecker interception (+ ambiguity + context-unification).
2. Interp construction.
3. Capability staging (5d).
4. Migrations: result.bv, option.bv, then sweep-driven follow-ups.
5. Tests: construct+match round-trip (interp), typing checks (contextual
   params, ambiguity, None-outside-context), migrated stdlib files pass.

## Acceptance

- examples/error-handling.bv typechecks+runs (interp).
- Sweep count drops materially; suite green per commit.
