# Async Phase C — Compiled Task Concurrency (Segmented Continuations)

**Date:** 2026-08-26
**Builds on:** Phase B (wake/block scheduler) + B2 (cells) — interpreter
reference complete. This phase gives the LLVM backend the same observable
task semantics.

## Reference semantics pinned first (rule: interpreter IS reference)

Probe (interpreter): `defn job(n: Int) -> Int { let acc = n * 2; yield;
term acc; }` → **`undefined variable 'acc'`**. Locals do NOT survive a
yield; only parameters carry into later segments. This is the A3 segment
model already shipped in the interpreter, and it makes the compiled form
exactly faithful WITHOUT stack switching:

> **Each segment is a plain function of the task's parameters.**
> `@briev_yield`/`@briev_resume` fibers from the original sketch are
> unnecessary machinery for these semantics. Substitution recorded here per
> rule 20 (refuted-hypothesis / simpler-isomorphism discipline).

## Current LLVM state (baseline)

- `spawn defn(args)` → inline eager call; handle IS the result
  (emit_expr.rs:4083)
- `await t` → pass-through of inner (:150)
- `yield;` → no-op (emit_stmt.rs:1863)
- `free t` → FreeHint local removal
- Observable consequence: compiled programs run tasks EAGERLY — no
  interleaving, cancellation is meaningless after the fact. The interpreter
  and backend disagree on any program with ≥2 segments or a freed task.

## Design

### 1. Frontend pass (frontend-driven dispatch pillar)

New `src/analysis/task_segments.rs`:
- `split_task_body(body, param_names) -> Vec<Vec<Statement>>` — the ONE
  splitter, extracted from the interpreter's `register_pending_task`
  (yield boundaries + Phase B port-read presplit via the shared
  root-identifier walk). Interpreter calls it; analysis calls it. DRY.
- `collect_spawn_targets(items) -> HashSet<String>` — defns referenced by
  any `Expr::Spawn`.
- `AnalysisResults.task_segments: HashMap<String, Vec<Vec<Statement>>>`
  populated in `analyze_program` for spawn targets only.

### 2. C runtime (`lib/runtime/briev_rt.c`)

```c
typedef struct { long long value; int finished; } BrievSegOut;
typedef BrievSegOut (*BrievSegFn)(long long* argv);
typedef struct {
    int status;            /* READY/YIELDED/DONE/CANCELLED */
    int current_segment, segment_count, arg_count;
    long long args[BRIEV_TASK_MAX_ARGS];   /* 8; params <= 6 house gate */
    long long result;
    const BrievSegFn* segments;
} BrievTask;

long long briev_task_spawn(const BrievSegFn* segs, int nseg, int nargs,
                           const long long* argv);
long long briev_await(long long handle);      /* round-robin driver */
void briev_task_cancel(long long handle);
```

`briev_await` mirrors the interpreter's Await handler exactly: loop over
READY/YIELDED tasks in id order, run one segment each, stop when the target
is DONE; empty runnable pool with target unfinished returns the handle
(deadlock posture). Fixed table of 64 tasks — spawn beyond it aborts.

### 3. Backend emission

Per spawned defn `f` with N segments:
- `__task_f_seg<k>(ptr %argv) -> {i64, i32}` — loads params by index from
  `%argv`, runs segment statements through the existing statement emitter.
  `term expr;` → `{value, 1}`; final-segment fall-through → `{last_value,
  1}`; otherwise `{0, 0}`. Intermediate Expression values are evaluated and
  dropped except in terminal position (matches "last evaluated wins" only
  where observable).
- `__task_f_segments` private constant array of fn pointers.
- Spawn site: adapt args to i64 ABI slots (Int/Bool raw; Ptr ptrtoint —
  same boxing family as unions), alloca+store argv, call spawn → i64 handle
  adapted back to Task<R>.
- Await site: call `@briev_await(h)` → i64 → adapt to R. R restricted to
  i64-representable (Int/Bool/Ptr); anything else is a capability rejection
  with what/why/fix (v1 scope, disclosed).
- `free t`: `call @briev_task_cancel(t)`.

Non-spawned defns lower unchanged — fully additive (rule 6).

### 4. Capability flag

`async` flips TRUE in LLVM's CAPABILITIES; programs using spawn/await now
compile to the concurrent form. The eager-inline path remains as the
degenerate case of N=1 segment... NO — removed: one code path, always the
segmented runtime (two paths = heuristic tree; accidental complexity).

## Tests

1. Splitter parity: `split_task_body` output == interpreter registration
   segments for the same bodies (unit, both call sites pinned).
2. Runtime unit (C-level, via rust test linking? no — exercised e2e).
3. End-to-end: new `examples/async-tasks.bv` — two tasks yielding between
   prints; deterministic interleaved output asserted against expected text;
   binary built with the harness link command and executed.
4. Cancellation e2e: freed task never prints.
5. Await-result e2e: `await` returns computed value through segments.
6. Full suite green; sweep green.

## Non-goals (disclosed)

- Port events in compiled code: EventQ/obj-port lowering does not exist on
  the LLVM surface yet — blocking reads stay interpreter-only this phase.
  Cells likewise (B2 was interpreter-side by design).
- Float-returning tasks (i64-slot restriction above).
- Verification-mode interleaving exploration.

## Benchmarks

No existing benchmark spawns tasks (grep confirms) — zero regression
surface on --runtime/--optimizer suites. Verified by grep + suite run;
rule 12b experiment unnecessary (no hot-path change).

## Doc maintenance

SPEC §12.2 gains the locals-die-at-yield sentence (a real language rule
discovered by probe, previously undocumented). Plan records the fiber→
segment substitution rationale.

## Undo

Revert AnalysisResults field + new module + emission arms + runtime block;
the eager-inline behavior returns. No shared arm changes meaning.
