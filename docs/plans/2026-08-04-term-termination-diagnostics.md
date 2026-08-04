# Term Termination Diagnostics + Void-Term Checkpoint

**Date:** 2026-08-04
**Status:** Active plan
**Branch:** `feat/term-termination-diagnostics` (worktree `../brief-compiler-term-diagnostics`)
**Related:**
- `docs/plans/2026-07-31-frontend-driven-dispatch.md` (frontend-driven dispatch — the pass model this follows)
- `AGENTS.md` Rule 4 (interpreter is reference) and Rule 8 (tests or it doesn't exist)
- `docs/architecture/agent-reference.md` (term/guard semantics)

---

## The Goal

Close the compiler's silent acceptance of **misused `term` syntax**. The author
of the `test-project-otto` demo (`when !arg_present(...) { ...; term; };`
cascade printing 404/422/409/200) got no diagnostic, and neither would anyone
who writes statements after a terminating `term <val>` / `term! <val>` — code
that provably never runs per the interpreter. The compiler must:

1. **Reject unreachable statements** after a terminating `term <val>` /
   `term! <val>` (and after any statement that always terminates), on both the
   `check` and `build` paths.
2. **Warn** on the author's exact confusion — a `when` guard ending in bare
   `term;` with the enclosing body continuing — with the fix spelled out.
3. **Make the backend faithful to the interpreter** for bare `term;`: a void
   term is a convergence checkpoint, not a body-stopper, in every void emission
   path.

The semantic model (confirmed with the maintainer):

- `term;` = **convergence checkpoint**: the body may continue; the txn/node
  loops until the postcondition + return value hold. It does NOT hard-stop the
  body.
- `term <val>` / `term! <val>` = **terminate now** (early return with value /
  close the program). The interpreter returns `Err(TermReturn)` which unwinds
  the whole transaction — every statement after it never runs.
- `term!` closes the program; the swan-song hoist
  (`src/analysis/swan_song.rs`) already realizes this for trailing guards.

---

## Investigation Summary (why this is needed)

Reference semantics in the interpreter (`src/interpreter/eval.rs`):

- `Term(Some(v))` / `TermBang(Some(v))` → `Err(RuntimeError::TermReturn(v))`
  (`eval.rs:651`, `eval.rs:705`); the defn/txn runner catches it and breaks
  (`src/interpreter/mod.rs:133-136`). `Guarded` uses `?`, so TermReturn
  propagates out of the guard and out of the whole body.
- `Term(None)` / `TermBang(None)` → `Ok(Void)` — a checkpoint, execution
  continues (`eval.rs:646-657`, `eval.rs:707-709`).

Backend divergences found at HEAD `93d4c07e`:

- **Term-with-value inside a `when` falls through.** `emit_stmt.rs:447-531`
  in a void fn sets `terminated = true` without emitting a real terminator
  (`:527-529`); the `Guarded` handler then unconditionally emits
  `br label %guard.endN` and resets `terminated = false` (`:576-589`), so the
  statements after the guard always run. The SSA main loop (`loop_engine/ssa.rs:367-370`)
  has no `terminated` break at all. Result: `when c { term! -> Print#(x); }; more;`
  prints `more` in the binary but never does in the interpreter. (The maintainer's
  own `term!` repro in `scratch_verify/term_bang_test.bv` printed `FIRSTSECOND`
  per tick — fall-through, not the interpreter's early-return.)
- **Bare `term;` mid-body stops the loop** in the async
  (`emit_toplevel.rs:2613`), callable (`:2397`), and pre (`:1937`) paths because
  `Term(None)` also sets `terminated = true`. The interpreter continues.
- **`return <val>` divergence (mirror image, tracked, NOT diagnosed):**
  interpreter `Return` is `Ok`, so following statements run; the backend emits a
  real `ret` and sets `terminated = true`. Undiagnosed because flagging it would
  reject code the interpreter executes; logged in BUGS.md for a separate fix.

The author's cascade (claim #4 of the 2026-08-03 report) is **correct behavior**:
bare `term;` really does continue to the next `when`. No compiler bug — a
missing compiler error / missing guidance.

---

## Changes

### 1. New analysis pass `src/analysis/termination.rs` (template: `swan_song.rs`, `watchdog.rs`)

- `always_terminates(&Statement) -> bool`, **conservative and
  interpreter-faithful**:
  - `Term(Some(_))`, `TermBang(Some(_))` → `true`
  - `Guarded(_, body)` → `always_terminates(body)`
  - `If` → both branches always terminate; `Block` → always terminates
  - `Match`, `Foreach`, bare `term;`, `return` → `false` (conservative; no
    false positives)
- `find_unreachable(&[TopLevel]) -> Vec<String>` — walk every statement list
  (txn/node bodies, guard bodies, if branches, blocks, defn bodies); after an
  always-terminating statement, every sibling is unreachable — report the first
  with node/txn context and a fix hint.
- `hint_guard_ending_in_void_term(&[TopLevel]) -> Vec<String>` — warning for the
  author's exact pattern: a `when` guard body whose last statement is a bare
  `Term(None)`/`TermBang(None)` and whose enclosing body continues. Message:
  "bare `term;` is a convergence checkpoint — execution continues to the next
  statement. To close the program here use `term! ->`, to return a value use
  `term <value>`."
- Returns `(errors, warnings)`; unreachable code is a **hard error** (it
  provably never runs per the interpreter); the guard hint is a warning.
- Unit tests in-module (mirroring `swan_song.rs`'s `#[cfg(test)]` style).

### 2. Wire the pass so both `check` and `build` see it

- `parse_and_check` (`src/compile.rs:1654`): run after typecheck, alongside the
  watchdog check (`:1669`). Errors fail `check`; warnings print to stderr.
- `compile_source`: run right after `check_types` (`:382`), alongside
  `string_concat`/`boundary_marshalling`.
- This closes the claim-#1 `check` vs `build` gap **for this diagnostic**.

### 3. Backend fix — void `term;` is a true checkpoint

- `src/backend/llvm/emit_stmt.rs:530-531` void path: `Term(None)` /
  `TermBang(None)` must NOT set `terminated = true` (interpreter: continue).
  Value forms keep setting it (interpreter: stop).
- Effect: bare `term;` mid-body becomes legal and continuing in the async,
  callable, and pre paths. Fused paths already filter terms
  (`emit_toplevel.rs:2652`); the SSA main needs no change.
- The value-form-in-guard fall-through becomes unreachable for valid programs
  via the §1/§2 error (no codegen change needed for it).

### 4. Regression tests

- Termination-pass unit tests (§1).
- Integration: a `.bv` with `when c { term! -> Print#(x); }; more;` must fail
  `briefc check` with the unreachable-code message; same for a guard whose body
  is only `term <val>` followed by a trailing statement.
- Valid shapes still pass: `examples/swan-song.bv`, `examples/ptr-demo.bv`,
  `examples/error-handling.bv`, and all four `test-project-otto` CLI files
  (bare `term;` guards).
- A corrected `transition_validate` variant using `term! ->` that compiles and
  prints exactly one line.
- Interpreter-parity test for the §3 fix (bare `term;` mid-body continues).
- **Pre-scan risk gate** (before landing the error): scan examples/benchmarks/
  tests for any terminating-term-followed-by-statement; fix genuine hits (they
  are genuinely unreachable).

### 5. BUGS.md + docs

- Log the term-in-guard fall-through divergence, the §3 fix, and the `return`
  divergence (separate latent bug — see Investigation Summary).
- Note the bare-`term!;` decision: currently a checkpoint per the interpreter;
  if `term!` is to close the program even without a value, that is a language
  change requiring interpreter + spec + docs updates together (Rule 4).
- Update `docs/architecture/agent-reference.md` term semantics if the message
  wording lands.

---

## Benchmarks (rule #11 — baseline required, §3 touches codegen)

Baseline at the current commit (clean `cargo build --release` +
`bash benchmarks/build_and_bench.sh --runtime`), run in the worktree BEFORE the
§3 change; re-run AFTER and compare with `bash benchmarks/compare_baseline.sh`.
Table format follows `benchmarks/results/2026-08-01-plugin-rework-baseline.md`.

| Benchmark | Brief | C | Ratio | Winner | Correct |
|-----------|:-----:|:--:|:-----:|:------:|:-------:|
| _baseline TBD — filled in at implementation time_ | | | | | |

---

## Verification

1. `cargo test --lib` green.
2. `cargo build` no new warnings.
3. Praetor on changed directories (no NEW diagnostics in changed files).
4. `briefc check` + `briefc build --llvm` on the four `test-project-otto` CLI
   files: all pass (bare `term;` guards are valid); the `term! ->` corrected
   variant behaves as intended.
5. Benchmark baseline A/B per above; any regression must be explained, not
   excused (rule #11b).
6. Commit after each logical step; never `git checkout --`/`git restore`.

---

## Out of Scope

- Changing the interpreter's `term` semantics (it is the reference).
- Fixing the `return` divergence (logged, separate fix).
- The claim-#1 `check`/`build` pipeline gap in general (only this diagnostic is
  wired to both; the full pipeline-consistency work is separate).
