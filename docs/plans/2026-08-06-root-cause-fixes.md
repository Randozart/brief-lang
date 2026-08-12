# Root-Cause Fixes — interpreter user-fn support, pure-fold IR bug, scheduler planning, escaping closures

**Date:** 2026-08-06
**Status:** Implementation plan
**Source:** classification of the 2026-08-06 diagnostics sweep (three were genuine
compiler bugs, one mixed, two programmer-facing) + two new bugs surfaced by the
sweep.

---

## 0. Executive Summary

The diagnostics sweep fixed messages; these are the ROOT CAUSES. Four items:

1. **Interpreter cannot apply user-defined functions** — the reference
   evaluator's `eval_call` has no registry access; op-elaborated contracts and
   derivation blocks fail (misleadingly). Fix: thread the functions registry
   through the interpreter and apply `defn` bodies with dynamic scoping to the
   caller's state.
2. **Pure-counter-fold + derived exit condition → invalid IR.** `emit_exit_check`
   emits a bare `.continue:` label meant for a loop body; the pure fold has no
   loop, so `.continue:` is an empty, unterminated block and clang rejects the
   module. Fix: bridge `.continue` → `.end`. (DONE — commit in this plan.)
3. **Scheduler plans frees it cannot soundly emit.** `global_lifetime::analyze`
   schedules a `Free#` for any provably-last consumer, but a non-bounded
   reactive node has no sound emission point, so the plan is silently dropped.
   Fix: `analyze` only schedules when the last consumer has a `bounded_pre`
   (can fold); otherwise the field falls back to the documented "lives for the
   program".
4. **Escaping closures are not lowered.** `let f = x -> body;` used as a value
   is rejected (sweep #3); the real capability is fn_ptr + heap env + indirect
   calls. Slice 1: capture analysis + env allocation + closure function
   emission + indirect call.

## 1. Investigation findings

### 1.1 Interpreter user-fn gap

- `Interpreter.functions: HashMap<String, FunctionDef>` (FunctionDef has
  `body: Vec<Statement>`); `call_function` applies them, but the free
  `eval_call` (eval.rs:230) only checks `bindings` → falls back to
  `UndefinedFunction`. All external callers use `Interpreter::eval_expr`/
  `exec_stmt` (methods with `&self`), so a threaded registry is reachable.
- `Expr::Block(stmts)` is evaluated (eval_block). A `defn` body is statements.
- Dynamic scoping is correct for defns (they read caller state) — unlike
  lexical capture for lambdas. So user-fn application seeds a fresh env from
  the CALLER's bindings, not a captured snapshot.

### 1.2 Pure-fold IR bug

- generate() derives `ctx.exit_condition` from the program post/term
  (mod.rs:2742 `combined`). The pure-counter-fold path (mod.rs:3669-3680)
  calls `emit_exit_check` (loop_engine/mod.rs:214-224) which emits
  `br i1 %c, label %.end, label %.continue` + a bare `.continue:` label. The
  pure fold has no loop, so nothing follows `.continue:` → empty unterminated
  block → clang "expected instruction opcode" at `.end:`. `emit_main` is
  unaffected (its `.continue` is filled by the loop setup alloca/br).
- Fix verified: const-bound `life` (Malloc# + `[sum < N][sum == N]`) compiles.

### 1.3 Scheduler planning

- `global_lifetime::analyze` schedules a free for the last ordered consumer
  regardless of foldability. The backend fold dispatch requires a
  `bounded_pre`; the transition-graph `nodes[].bounded_pre` is available to
  `analyze`'s caller (analyze_program builds it). A non-bounded last consumer
  never emits the free (reactive path has no sound point).
- Design says the scheduler is "sound but not complete" — a field it cannot
  free falls back to "lives for the program" (silent, documented). So the
  fix is to NOT schedule unemittable frees; the sweep's warning then covers
  only the rare "has bounded_pre but the backend still can't fold" edge.

### 1.4 Escaping closures

- Codegen `closure_lets` (per-function) + inline-at-call-site (Phase 8 slice).
  The value case is rejected (sweep #3). Full lowering needs: free-variable
  capture analysis, a heap env block, top-level closure-function emission, and
  an indirect call. `Call` is name-keyed; a closure VALUE as a callee needs the
  fn_ptr + env representation. Slice 1 = the capture+env+function-pointer
  machinery for closures bound to a `let` and passed/called dynamically.

## 2. Design

### 2.1 Interpreter user-fn support

- Thread `functions: &HashMap<String, FunctionDef>` through the free
  `eval_expr`/`eval_statement`/`eval_block`/`eval_call` and the recursive
  helpers (mechanical param addition). `Interpreter::eval_expr`/`exec_stmt`
  pass `&self.functions`. Test helpers pass `&HashMap::new()`.
- `eval_call` order: `#` intrinsic → bindings closure/value → **functions
  registry** (apply: `local = bindings.clone()` seeded with caller state,
  bind params, eval `Expr::Block(fn.body)`; `Term`/`ExitProgram` inside
  propagate via the existing TermReturn mechanism) → `UndefinedFunction`.
- This makes op-elaborated contracts + derivation blocks reference-verifiable.

### 2.2 Pure-fold IR bug (done)

Bridge `.continue` → `.end` in the pure-counter-fold path.

### 2.3 Scheduler planning

- `global_lifetime::analyze` gains a `foldable: &HashSet<String>` input (the
  txns with a `bounded_pre`, computed by the caller from the transition
  graph). A field whose last consumer is not foldable is NOT scheduled (lives
  for the program). The backend warning stays for the fold-attempted-but-failed
  edge.

### 2.4 Escaping closures (slice 1)

- A pre-pass collects every `Expr::Lambda` with its free variables (idents not
  bound by the lambda, minus `#` intrinsics/field names) → `ClosureDef {
  params, body, free_vars }` + a stable symbol `briev_closure_N`.
- `Expr::Lambda` at runtime: heap-allocate an env block `[cap1..capN]`, closure
  value = block address (i64). The `let` binding stores the address.
- The closure function `define i64 @briev_closure_N(ptr %env, i64 %p1..)`
  emitted after the enclosing function; params + captured vars bound, body
  emitted, return.
- `Call` on a closure-typed name: load the env address, call the symbol with
  the env + args.
- Escaping = the closure value can flow (passed, returned); slice 1 supports
  call-by-name + direct application; full escaping (stored in structs) is the
  follow-up.

## 3. Tests

- Interpreter: `defn` called from a contract/body applies with caller state;
  params bound; term return propagates. Update the `test_closure_call_unbound_name_errors` /
  add `test_user_function_call_applies`.
- Pure fold: backend test asserting the const-bound heap program generates
  valid IR (no empty `.continue`).
- Scheduler: `analyze` skips non-bounded last consumers (lives-for-program);
  the fold-failed warning still fires.
- Closures: escape test (closure bound + passed + called) matching the
  interpreter's by-value capture.

## 4. Baseline

Commit `1d09db2f` (diagnostics sweep). 36/36 runtime MATCH, `bridge_glue`
SKIP noise. Expectation: unchanged output; the pure-fold fix only affects
previously-broken IR; scheduler planning only stops planning (no behavior
change to correct programs); interpreter fn-support is additive; closure
lowering is new capability.

## 5. Docs to update

- `docs/plans/2026-08-06-diagnostics-sweep.md` §5 (notes the root causes here).
- `docs/architecture/overview.md` interpreter + scheduler notes.
- `docs/plans/2026-08-05-spec-implementation-status.md` §14/§18/§2 rows.
- `docs/plans/2026-08-06-phase5-op-elaboration.md` (interpreter user-fn
  follow-up now done).

## 6. Tracker

- [x] Plan doc
- [x] Fix 2: pure-fold `.continue` bridge — committed `4ef243d1`
- [x] Fix 3: scheduler never plans unemittable frees — committed `4ef243d1`
- [x] Fix 1: interpreter user-fn support — committed `4ef243d1`
- [x] Fix 4: escaping closures slice 1 — committed (see §8)
- [x] Tests + Praetor + benchmarks + commit

## 7. Fixes 1–3 delivered (commit `4ef243d1`)

- **Interpreter user-fn support**: `eval_expr`/`eval_statement`/`eval_call`
  thread a functions registry; `eval_call` applies a `FunctionDef` with
  dynamic scoping (body reads the caller's state) and catches `term`-as-return.
  `EvalScope` bundles bindings+functions so the four eval helpers stay under
  the Praetor 6-param gate. 3 new tests.
- **Pure-fold IR bug**: `.continue` → `.end` bridge. Verified the const-bound
  reactive heap loop compiles.
- **Scheduler planning**: `analyze` takes a foldable set (bounded_pre txns)
  and skips non-foldable last consumers ("lives for the program"). 1 new
  scheduler test; the leak-warning backend test now asserts the no-schedule
  behavior.
- 1616 lib tests; Praetor no new diagnostics (36 identical); 36/36 MATCH.

## 8. Fix 4 — escaping closures (delivered 2026-08-06)

- **Capture analysis**: `collect_free_vars` / `collect_free_expr` /
  `collect_free_stmts` (context.rs) — idents not bound by params/lets, `#`
  names excluded, deterministic order. Nested lambdas/blocks shadow.
- **Env allocation**: `let f = lambda` allocates `[fn_ptr, cap1..capN]`
  (8-byte slots), stores `ptrtoint @briev_closure_N` + the captured values;
  the closure VALUE is the block address (was the `add i64 0, 0` placeholder).
- **Closure function emission**: `emit_pending_closures` /
  `emit_one_closure` (mod.rs) emit `define i64 @briev_closure_N(ptr %env,
  i64 %p..)` at module end; captured vars loaded from env slots, params bound,
  body emitted, value returned.
- **Indirect call**: `emit_closure_indirect_call` (emit_expr.rs) loads the
  value → inttoptr env → load fn_ptr → indirect call. Replaces the inline
  lowering; the inline helper removed.
- **Alias flow**: `let g = f; g(x)` — g aliases f's env; calls to g go
  indirect too (emit_stmt.rs closure-alias arm).
- **Rejection lifted**: the diagnostics-sweep closure-as-value error is gone —
  closures are real values now; the typechecker still rejects a Function value
  where a non-Function is required.
- Verified natively: `f(2)=10, f(3)=15, c=25` (capture of k); `g(41)=42`
  (alias). 2 new backend tests (env+indirect, alias). 1618 lib tests.
  Praetor: no NEW diagnostics (the +1 on pre-existing `emit_statement`
  360→361 is from the necessary closure-alias arm on a 24x-over-limit
  function). 36/36 MATCH.

Known slice-1 boundaries (documented): closure values stored in structs /
passed as `defn` arguments need function-typed parameter plumbing (the
typechecker does not yet annotate untyped lets with their inferred Function
type); closure functions always return i64 (int closures).
