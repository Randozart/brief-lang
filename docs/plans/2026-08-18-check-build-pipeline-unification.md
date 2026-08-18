# Plan: unify the `brievc check` pipeline with `build` + verify-fixed follow-ups

**Date:** 2026-08-18
**Head commit:** `4541bc0f` (both pooled-member-foreach and defn-param boxing shipped).
**Build/test:** `cargo build --release` / `cargo test --lib` /
`bash benchmarks/build_and_bench.sh --runtime` / `./target/release/brievc check <file>` /
`./target/release/brievc build <file> --out <dir>`.

## Motivation

`brievc check` and `brievc build` silently diverge: `brievc check` on a program
that imports `std/collections.bv` (or `std/hashmap.bv`) reports three spurious
type errors on the HashMap's GENERIC scans (`acc <- keys[i]` in `keys()`,
`acc <- vals[i]` in `values()`, `acc <- (keys[i], vals[i])` in `entries()` —
"expected List<K> for arrow assignment, found K"). `brievc build` on the same
program is clean and the runtime is correct. Documented as
`BUGS.md` "`brievc check` over-reports on imported generic collection scans".

A stale `BUGS.md` entry claims a String-param library-export codegen bug
(`%ac0` i64-vs-ptr); read-only A/B (2026-08-18) shows it is NOT reproducible
with the current binary — see Item B.

## Item A — unify the check pipeline with build

### A/B evidence (2026-08-18, black-box)

- `brievc check tests/tier1/test_hashmap_surface.bv` → 3 type errors;
  `brievc build` (same file, `--out /tmp/…`) → OK, correct runtime.
- Both paths call the SAME `check_types` → `typechecker::check_program`
  (`compile.rs:2457`). Build enforces type errors generally
  (`let x: Int = "hello"` fails in build), so the two paths hand `check_program`
  DIFFERENT items.
- Ruled OUT as the cause: the prelude plugin (`--disable-plugin prelude`
  build still passes), the print plugin, an explicit `import { Int } from
  "std/types/bootstrap.bv"`, and `resolve_comptime_refs` (touches only
  Constants/Triggers/init seeding, never TypeDefs).

### Pipeline divergence

`compile_source` (build) runs BEFORE `check_types`:
1. `extract_inline_stage_blocks` (pre-import)
2. `evaluate_pending_comptime` (pre-import)
3. `pm.run_ast(StageKind::Parsed)`
4. `resolve_imports`
5. `extract_inline_stage_blocks` (post-import)
6. `evaluate_pending_comptime` (post-import)
7. `pm.run_ast(StageKind::Resolved)`
8. `resolve_comptime_refs`

`parse_and_check` (check) runs: parse → `resolve_imports` → `check_types`.
No plugin stages, no comptime evaluation.

The typechecker has a built-in `List` name special-case
(`typechecker/mod.rs:1308`, `list_literal_accepted_by`) that lets `List<T>`
typecheck even when the real `obj List<T>` op-bindings are not collected —
masking the missing registration in the check path.

### Step 0 — instrumented A/B (identify the exact missing registration)

Temporary `eprintln!` in the `ArrowAssign` handler (or `push_element_type`)
dumping, for the failing `acc <- keys[i]`: `type_members["List"]`,
`regular_bindings["List"]`, the resolved `acc`/`keys[i]` types — run under
BOTH `brievc check` and `brievc build` on `test_hashmap_surface.bv`. Record the
diff in the plan. Hypothesis: `regular_bindings["List"]` (the `op InsertAt:
push` binding) is missing in the check path, so `push_element_type(acc)`
returns `None` and the arrow falls to the plain-assignment mismatch.

### Step 1 — unify

`parse_and_check` runs the SAME pipeline stages as `compile_source` before
`check_types`, in the same order: build the plugin manager (prelude etc.),
`extract_inline_stage_blocks`, `evaluate_pending_comptime`, plugin Parsed,
`resolve_imports`, post-import stage-block extraction + comptime eval, plugin
Resolved, `resolve_comptime_refs`. Then `check` and `build` cannot diverge —
the whole bug class dies (the check path is a stale lean path that predates
the plugin stages).

Keep the check path's extra analyses (watchdog contracts, termination
diagnostics, spawn-count, unconstrained-literal) unchanged.

Threading: `parse_and_check` currently takes `(file_path, source, opts)` and
builds no plugin manager. It must construct one (mirroring `compile_source`'s
`build_plugin_manager`) with the same lockfile/allow-* handling, or factor the
shared pre-check pipeline into a helper used by both paths (DRY — the two
paths must not drift again).

### Step 2 — regression

- A unit test asserting `parse_and_check` on an import-bearing source
  (imports `collections.bv`, calls `m.keys()` and `ks.Count#()`) returns `Ok`.
- `brievc check tests/tier1/test_hashmap_surface.bv` clean;
  `brievc check lib/std/collections.bv` and `brievc check lib/std/hashmap.bv`
  clean (the hashmap.bv check error from 2026-08-18 was THIS divergence).
- `cargo test --lib` green; runtime suite all MATCH.

### Risks

- `parse_and_check` is shared by `brievc check` (both `.bv` and `.dbv` paths —
  the `.dbv` dispatch happens before it in `run_check`). The plugin manager
  wiring and the watchdog/termination steps must be threaded correctly.
- The instrumented A/B may surface a SECOND cause beyond the hypothesis; the
  fix narrows accordingly before unification.
- Plugin runs have side effects (lockfile validation, `--allow-*`); the check
  path must respect the same flags.

## Item B — String-param library export `%ac0` (verify-fixed)

Read-only A/B (2026-08-18):
- `export defn greet(name: String) -> String` (plain) and
  `let saved: String = "hi"; export defn state_str(name: String) -> String`
  (stateful) both emit clean `define ptr @…(ptr %arg0)` with `--library`.
- `opt -O2` on the generated `.ll` (the ORIGINAL failure — "`%ac0` defined
  with type 'i64' but expected 'ptr'") now passes.
- The only clang link error is "undefined reference to `main`" — expected for
  a library.

The fix landed earlier: struct/String params are `ptr` at the ABI
(`emit_toplevel.rs:2631-2638`: `ptrtoint` conversion), and the SSO String-param
branch was retired (`emit_toplevel.rs:2563-2565`).

### Steps

1. Run the ORIGINAL driver shape: `cargo test --test c_driver_needs_state`
   (the `--library` + C driver that surfaced the bug) — confirm green.
2. Mark the BUGS.md entry **FIXED 2026-08-18** with the evidence (clean `ptr`
   signature + `opt -O2` pass + driver green).
3. Leave `lib/compiler/needs_state.bv`'s `CStr` + `cstr_to_briev` workaround
   in place — a legitimate ABI choice for a C boundary; no churn.

## Housekeeping (same pass)

Stale BUGS.md statuses where the RESOLVED/FIXED body supersedes an OPEN
title/status:
- countdown/fold path (`Inlined member with a foreach + a nested foreach`, ~157-160/191).
- Iterable-protocol slice-6 deletions (~335).
- Float ABI/opcode corruption (~4094, body marks FIXED 2026-08-03).
- Item B's String-param entry (this pass).

## Execution order

Plan doc → A Step 0 (instrumented A/B) → A Step 1 (unify) → A Step 2
(regression) → B verify → housekeeping. Commit after each logical step that
leaves the suite green. `cargo test --lib` before every commit; runtime suite
before the A commit; Praetor on changed dirs (`src/backend/llvm`,
`src/compile.rs`); update BUGS.md + this plan in the same commits.
