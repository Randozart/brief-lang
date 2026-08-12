# Phase 9 (Slice 1) — Wire the garbage scheduler: Free# emission + redundant-keep warnings

**Date:** 2026-08-06
**Status:** Implementation plan
**Source:** `docs/plans/2026-08-05-implement-normative-language-spec.md` §15 (§15.3 Scheduler)
**Design:** `docs/plans/2026-08-01-global-lifetime-design.md` (garbage scheduler)
**Pin:** `benchmarks/global_lifetime.bv` (+ `global_lifetime_c.c` which calls `free`)

---

## 0. Executive Summary

`analysis/global_lifetime.rs` — the compile-time garbage scheduler that PROVES
each heap-backed state field's last consumer in reactor order and plans a
`Free#` exactly after it — is complete and unit-tested but **never wired into
the compile pipeline**. Briev programs with heap state (ring_buffer, hash_ops,
linked_list, global_lifetime) leak: the C references call `free`, Briev does
not. This slice wires the scheduler: surface `redundant_keep` warnings and
inject the scheduled `Free#` statements before each last-consumer transaction's
trailing term. Output-identical (frees are not observable), so the benchmark
suite must stay 36/36 MATCH; the `global_lifetime` benchmark pins the behavior.

## 1. Investigation findings

- `global_lifetime::analyze(items, field_initializers, node_order)` →
  `GlobalLifetime { free_after: HashMap<txn, Vec<field>>, redundant_keeps }`,
  fully implemented + 4 unit tests. Detects heap-backed fields
  (`contains_heap_alloc`: `Malloc#`/`Alloc#` initializers), excludes
  manually-freed fields, computes per-txn touch sets, and schedules the free
  after the last ordered consumer.
- `ReactorTransitionGraph.nodes: Vec<ReactorNode>` is the reactor's
  deterministic firing order (`node_order`).
- `Free#` is a first-class intrinsic (interpreter intrinsics.rs:128, backend
  intrinsics.rs:31 → `__briev_free`). A statement
  `Statement::Expression(Call("Free#", [Identifier(field)], None))` lowers
  correctly.
- `benchmarks/global_lifetime.bv` ends with `term;` — a naive append would
  make the free unreachable; the injection must insert BEFORE a trailing
  `Term`/`ExitProgram` (or append when none).
- Dangling pointers are ALREADY hard errors in compile.rs (line 563-583) —
  §15.2's hard-error requirement is done. The ownership algebra keywords
  (`borrow`/`owned`/`borrowed<source>`/`shared`) do NOT parse yet — separate
  slice.
- compile.rs surfaces warnings via `eprintln!("warning: {w}")`.

## 2. Design

### 2.1 Backend fold-path free emission (NOT statement injection)

Investigation corrected the plan: the scheduler is ALREADY wired end-to-end —
`analyze_program` computes `GlobalLifetime` (backend/mod.rs:124) into
`AnalysisResults.global_lifetime`, the LLVM backend copies it to
`ctx.global_free_after` (mod.rs:1729), and the **countdown** fold path
(emit_countable_countdown_main, counter.rs) and the **non-loop** txn path
(emit_toplevel.rs:1824) already emit the `__briev_free` after the loop/body.
The `global_lifetime` benchmark runs the countdown path and was already
freeing (free_count == 1).

What this slice adds:

1. **`emit_scheduled_frees` helper** (counter.rs) — the shared
   load-handle → inttoptr → `__briev_free` emission, replacing the duplicated
   inline blocks (DRY, 4 sites).
2. **PerFieldPhi path** (`emit_countable_main`) — previously did NOT free;
   now emits the scheduler's frees at the loop exit (`.cm_end` block), so a
   free never fires inside an iterating body.
3. **version-DAG path** (`emit_version_dag_main`) — gained a `free_after`
   param + emission before `ret`, so the single-runtime-guard fold path also
   frees. (Both fold paths were silent leaks for heap-backed last-consumer
   fields.)
4. **Automated test** — the countdown shape (benchmark's shape, with a
   non-plugin `when` guard) asserts the `__briev_free` call is present and
   precedes a `ret` (post-loop, not inside the body).

Known limitation (pre-existing, sound): the non-loop reactive path skips the
free when the body ends in `term` (`emit_toplevel.rs:1818` gates on
`!fun.terminated`). A multi-firing reactive node MUST NOT free inside its body
(use-after-free), so the skip is the conservative choice; such nodes leak
rather than crash. Making them free requires forcing the countable-loop shape
or a post-reactor hook — separate work.

## 3. Tests

- `global_lifetime` unit tests: `inject_frees` inserts before a trailing
  term; appends when no term; multiple fields ordered; term-not-last handled.
- End-to-end: `global_lifetime.bv` still builds and its printed output
  matches (the suite A/B below); `__briev_free_count` becomes 1 (matches the
  C reference's `free`).
- Full `cargo test --lib` green.

## 4. Benchmark Baseline (rule 11)

Measured at commit `431bf003` (Phase 5 slice 1): 36/36 runtime benchmarks
MATCH, `bridge_glue` SKIP, `bridge_multi` PASS. Expectation: unchanged output;
timing for the heap-state benchmarks may shift by one O(1) free — verify via
the full A/B run, never excuse as noise.

## 5. Docs to update

- `docs/plans/2026-08-06-phase9-ownership-lifetime.md` tracker (this doc).
- `docs/plans/2026-08-05-spec-implementation-status.md` §14 row → In progress
  (scheduler wired; ownership algebra + `.s` enforcement pending).
- `docs/architecture/overview.md` memory note if it claims the scheduler is
  unwired.

## 6. Risks

| Risk | Mitigation |
|---|---|
| Injected frees break a benchmark (use-after-free if scheduler unsound) | Scheduler excludes non-heap + manually-freed fields; full A/B suite; `global_lifetime` pin |
| Free placed unreachable (after term) | `insert_before_trailing_term` |
| Timing regression on heap benchmarks | One O(1) free per field; A/B compare, document |
| node_order mismatch (graph built with `&None` exit) | Same call shape as `analyze_program` uses |

## 7. Tracker

- [x] Fold-path free coverage: PerFieldPhi (`emit_countable_main`) +
  version-DAG (`emit_version_dag_main`) + `emit_scheduled_frees` helper — 2026-08-06
- [x] Automated countdown-shape test (free after loop, before ret) — 2026-08-06
- [x] E2E: `global_lifetime` benchmark free_count == 1, output matches C — 2026-08-06
- [x] Benchmarks + Praetor + commit — 2026-08-06

## 8. Delivered (2026-08-06)

- `emit_scheduled_frees(out, fields)` helper in counter.rs — shared
  load-handle → inttoptr → `__briev_free`; used by countdown, PerFieldPhi,
  version-DAG (DRY, replaces duplicated inline blocks).
- PerFieldPhi + version-DAG fold paths now free scheduler-proven fields at
  the loop exit (were silent leaks for heap-backed last consumers).
- countdown path refactored onto the helper (behavior unchanged).
- New backend test locks the countdown free emission (post-loop, pre-ret).
- Verified E2E: `global_lifetime.bv` emits `__briev_free` once after the loop
  (`.cde_` exit block), `__briev_free_count == 1`, output identical to the C
  reference.
- Discovery: the scheduler was already wired for the countdown + non-loop
  paths (analyze_program → AnalysisResults.global_lifetime →
  ctx.global_free_after); the compile.rs statement-injection approach in the
  original plan was unnecessary and would have been UNSOUND inside an
  iterating loop — the backend loop-exit emission is the correct mechanism.
- Remaining Phase 9 items (separate slices): ownership algebra keywords
  (`borrow`/`owned`/`borrowed<source>`/`shared`) do not parse yet; `.s`
  profile enforcement; `src/lifetime.rs`. Pre-existing sound limitation:
  reactive multi-firing nodes leak (freeing inside the body would be
  use-after-free; the `!terminated` gate conservatively skips).
