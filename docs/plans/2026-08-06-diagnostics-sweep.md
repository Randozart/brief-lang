# Diagnostics Sweep — six missing/misleading error messages

**Date:** 2026-08-06
**Status:** Implementation plan
**Source:** audit of error messages during Phase 17/7/8/5/9 work

---

## 0. Executive Summary

Six cases where the compiler stayed silent or gave a misleading error during
the recent phases. All are additive diagnostics — no behavior change to correct
programs. Each has a test.

## 1. The six fixes

| # | Symptom | Fix | Site |
|---|---|---|---|
| 1 | A scheduler-planned `Free#` is silently skipped (reactive/term-ending last consumer) → heap leak, zero diagnostic | Warn when a `free_after` field's last-consumer txn does not fold (no LoopShape → the free has no sound emission point) | `backend/llvm/mod.rs` generate() after `global_free_after` set |
| 2 | `op Add(Int): nonexistent(#Lh,#Rh)` typechecks, lowers to `call @nonexistent`, dies at link with an undefined-symbol clang error | Typecheck error: "op 'Add' target 'nonexistent' is not a defined function" | `elaborate_ops` (validate the Function binding target against the program's defined fns) |
| 3 | `let f = x -> body;` then `Print#(f)`/passing `f` silently yields the 0 placeholder register | Error: "closure 'f' can only be applied as a call" | `emit_expr` Identifier arm (closure_lets contains the name) |
| 4 | Interpreter `Call(my_add)` (from op elaboration) → misleading "undefined variable 'my_add'" (my_add IS defined) | Truthful message covering both cases | `interpreter/eval.rs` eval_call fallback |
| 5 | `GetEnvInt#("BOUND")` → "unknown intrinsic 'GetEnvInt#'" (the rename guidance for `GetEnvInt` exists but misses the `#` forms) | Route `GetEnvInt#`/`GetEnv#`/`GetEnvOrDefault#` through the rename guidance | typechecker intrinsic-lookup error path |
| 6 | `x => body` (lambda with the match arrow) → "expected Semicolon, found '=>'" | Hint: match arms use `=>`; lambda parameters use `->` | parser error_at_current special-case for FatArrow |

## 2. Baseline

Commit `b291cbb4` (Phase 9 slice 1). 36/36 runtime MATCH, `bridge_glue` SKIP
noise, `bridge_multi` PASS. Expectation unchanged — all six are diagnostics.

## 3. Docs to update

- This plan's tracker.
- `docs/plans/2026-08-05-spec-implementation-status.md` §23 row if it lists
  diagnostics; house-style note in `src/errors.rs` comments where touched.

## 4. Tracker

- [x] #5 GetEnvInt# rename guidance — 2026-08-06 (typechecker infer_call)
- [x] #6 lambda arrow parse hint — 2026-08-06 (parser expect() FatArrow hint)
- [x] #2 undefined op target — 2026-08-06 (elaborate_ops validates against defined_fns)
- [x] #3 closure-as-value error — 2026-08-06 (elaborate_expr Identifier × Function binding; backend panic as defense)
- [x] #1 reactive-leak warning — 2026-08-06 (fold-site None/Some arms + emit_transaction no-bounded-pre gate; benchmark verified no false positive)
- [x] #4 interpreter user-fn message — 2026-08-06 (eval_call None → UndefinedFunction, not UndefinedVariable)
- [x] Tests + Praetor + benchmarks + commit — 2026-08-06

## 5. Delivered (2026-08-06)

All six diagnostics implemented + tested (5 new tests, 1 updated; 1612 total).
Verified end-to-end: each fires on the target program and stays silent on
correct programs (the `global_lifetime` benchmark folds and frees with NO
false "will leak" warning; none of the 36 benchmarks trigger any new
diagnostic). Praetor: no new diagnostics (13 identical at HEAD). 36/36
runtime benchmarks MATCH; bridge_glue SKIP noise identical to baseline.

Key implementation notes:
- #1's warning is placed at the fold-decision point (mod.rs:2828, both the
  no-shape `None` arm and the `!folded` case) plus emit_transaction gated on
  "no bounded pre" — this avoids false positives for folding txns (the free
  IS emitted in the fold) while flagging the genuinely-unemittable cases.
- #2 reuses the new `defined_fns` set on `TypecheckContext` (populated once in
  check_program); `elaborate_ops` now returns `Vec<TypeError>`.
- #3 fires in `elaborate_expr` when an `Expr::Identifier` resolves to a
  Function-typed binding (a closure used as a value); the codegen panic is a
  defense-in-depth guard.
- #4 reuses the existing (previously unused for calls) `UndefinedFunction`
  RuntimeError variant.
