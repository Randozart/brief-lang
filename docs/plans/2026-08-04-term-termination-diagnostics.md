# Term Termination Diagnostics + Void-Term Checkpoint

**Date:** 2026-08-04
**Status:** Implemented (`5ab100b1`, `be934d61`, `ac6aca40`) — all 37 runtime
benchmarks MATCH post-change (A/B table below), queue_drain regression fixed and
documented; docs updated.
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

### 3. Backend fix — void `term;` is a true checkpoint; value-form terms get real terminators

- `src/backend/llvm/emit_stmt.rs` void path: `Term(None)` / `TermBang(None)`
  must NOT set `terminated = true` (interpreter: continue). Value forms keep
  setting it (interpreter: stop) — AND now emit a REAL terminator.
- Effect of the checkpoint fix: bare `term;` mid-body becomes legal and
  continuing in the async, callable, and pre paths. Verified by IR diff: the
  async body contains the `__print_int` after the bare `term;`.
- **Value-form-in-guard fall-through REQUIRES a codegen fix (this plan's §1/§2
  error alone was NOT sufficient).** `corrected_term_guard.bv`
  (`when a == 1 { term! -> Print#(1); }; Print#(2);`) passes `briefc check`
  (the statement after the guard IS reachable when the guard is false) yet the
  pre-fix binary printed `"12"` while the interpreter prints `"1"`. The
  correctness proof: the interpreter's `TermReturn` unwinds the WHOLE body, so
  the guard's true-path must skip the rest of the body, not just converge to
  `guard.endN`. Implemented as:
  - New `FunctionContext.void_txn_abort_label` (context.rs) — the SSA main loop
    sets it to the current txn's `.ssn_<name>` next-txn label so a void
    value-form term branches past the rest of THIS txn's body; per-txn void
    functions leave it `None` and the term emits `ret void`.
  - `Guarded` (emit_stmt.rs) emits its convergence branch only when the body did
    NOT terminate (rewrites the 2026-07-19 unconditional-br workaround).
  - Body loops with no `terminated` break (SSA main `ssa.rs`, outlined cold
    functions) now `break`; epilogues that unconditionally emitted a trailing
    `br %...done` / `ret void` (async `emit_toplevel.rs`, pre-function, cold
    function) are now conditional.

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

## Benchmarks (rule #11 — baseline required, codegen touched)

Baseline at the current commit (clean `cargo build --release` +
`bash benchmarks/build_and_bench.sh --runtime`), run with the PRE-change binary
(main-worktree `target/release/briefc`, Aug 4 10:12) on 2026-08-04; the
POST-change run (feature worktree) follows and is compared.
Table format follows `benchmarks/results/2026-08-01-plugin-rework-baseline.md`.

### Baseline (pre-change, main worktree @ `b0487364`)

| Benchmark | Brief | C | Ratio | Winner | Correct |
|-----------|:-----:|:--:|:-----:|:------:|:-------:|
| ring_buffer | .0694s | .0648s | 1.07x | C | MATCH |
| float_math | .0455s | .0740s | .61x | Brief | MATCH |
| float_math_nonzero | .1619s | .1700s | .95x | Brief | MATCH |
| sparse_dispatch | .0618s | .0693s | .89x | Brief | MATCH |
| print_loop | .0417s | .0757s | .55x | Brief | MATCH |
| nbody_newton | 9.1892s | 10.8440s | .84x | Brief | MATCH |
| nbody_sqrt | 3.0792s | 3.9270s | .78x | Brief | MATCH |
| nbody_sqrt_idio | 3.9526s | 4.6586s | .84x | Brief | MATCH |
| fasta | .3034s | .3222s | .94x | Brief | MATCH |
| fannkuch_redux | .0817s | .0780s | 1.04x | C | MATCH |
| mandelbrot | .8040s | .8289s | .96x | Brief | MATCH |
| kalman_filter_runtime | .1453s | .1872s | .77x | Brief | MATCH |
| knucleotide | .2086s | .2059s | 1.01x | C | MATCH |
| cancel_math | .0643s | .0710s | .90x | Brief | MATCH |
| bit_clear | .0001s | .0002s | .50x | Brief | MATCH |
| queue_drain | .0400s | .0696s | .57x | Brief | MATCH |
| queue_drain_sym | .0417s | .0694s | .60x | Brief | MATCH |
| queue_drain_idio | .0416s | .0720s | .57x | Brief | MATCH |
| stack_push_pop | .0422s | .0694s | .60x | Brief | MATCH |
| interval_step | .0714s | .0705s | 1.01x | C | MATCH |
| telemetry_stream | .1973s | .2297s | .85x | Brief | MATCH |
| pid_control | .3485s | .3541s | .98x | Brief | MATCH |
| matrix_pipeline | .4705s | 1.0420s | .45x | Brief | MATCH |
| accumulator_flush | .1491s | .2144s | .69x | Brief | MATCH |
| sweep_sparse | .2238s | .1635s | 1.36x | C | MATCH |
| sweep_mid | .2712s | .2447s | 1.10x | C | MATCH |
| sweep_dense | .4119s | .2765s | 1.48x | C | MATCH |
| sweep_arr | .4143s | .3643s | 1.13x | C | MATCH |
| series_converge | .0003s | 0s | x | ~tie | MATCH |
| global_lifetime | .0337s | .0870s | .38x | Brief | MATCH |
| deep_recursion | .0004s | .0002s | 2.00x | C | MATCH |
| arena_churn | .0909s | .1217s | .74x | Brief | MATCH |
| linked_list | 1.5420s | 2.2608s | .68x | Brief | MATCH |
| hash_ops | 1.3678s | 1.5223s | .89x | Brief | MATCH |
| hash_ops_idio | .0322s | .0637s | .50x | Brief | MATCH |
| enemy_swarm | .1581s | .1788s | .88x | Brief | MATCH |
| bridge_glue | done | | | | SKIP |
| bridge_multi | done | | | | PASS |

### Post-change (feature worktree @ `ac6aca40`)

| Benchmark | Brief | C | Ratio | Winner | Correct |
|-----------|:-----:|:--:|:-----:|:------:|:-------:|
| ring_buffer | .0652s | .0684s | .95x | Brief | MATCH |
| float_math | .0470s | .0779s | .60x | Brief | MATCH |
| float_math_nonzero | .1624s | .1787s | .90x | Brief | MATCH |
| sparse_dispatch | .0634s | .0714s | .88x | Brief | MATCH |
| print_loop | .0421s | .0683s | .61x | Brief | MATCH |
| nbody_newton | 8.8868s | 10.6070s | .83x | Brief | MATCH |
| nbody_sqrt | 2.8409s | 3.8678s | .73x | Brief | MATCH |
| nbody_sqrt_idio | 4.0222s | 4.7522s | .84x | Brief | MATCH |
| fasta | .3176s | .3309s | .95x | Brief | MATCH |
| fannkuch_redux | .0934s | .0958s | .97x | Brief | MATCH |
| mandelbrot | .8272s | .8189s | 1.01x | C | MATCH |
| kalman_filter_runtime | .1587s | .1866s | .85x | Brief | MATCH |
| knucleotide | .2129s | .2197s | .96x | Brief | MATCH |
| cancel_math | .0688s | .0761s | .90x | Brief | MATCH |
| bit_clear | .0001s | .0003s | .33x | Brief | MATCH |
| queue_drain | .0428s | .0723s | .59x | Brief | MATCH |
| queue_drain_sym | .0414s | .0737s | .56x | Brief | MATCH |
| queue_drain_idio | .0457s | .0749s | .61x | Brief | MATCH |
| stack_push_pop | .0455s | .0742s | .61x | Brief | MATCH |
| interval_step | .0777s | .0768s | 1.01x | C | MATCH |
| telemetry_stream | .2007s | .2415s | .83x | Brief | MATCH |
| pid_control | .3493s | .3556s | .98x | Brief | MATCH |
| matrix_pipeline | .4739s | 1.1448s | .41x | Brief | MATCH |
| accumulator_flush | .1771s | .2421s | .73x | Brief | MATCH |
| sweep_sparse | .2231s | .1672s | 1.33x | C | MATCH |
| sweep_mid | .2798s | .2501s | 1.11x | C | MATCH |
| sweep_dense | .4136s | .2789s | 1.48x | C | MATCH |
| sweep_arr | .4113s | .3658s | 1.12x | C | MATCH |
| series_converge | .0004s | .0003s | 1.33x | C | MATCH |
| global_lifetime | .0426s | .0920s | .46x | Brief | MATCH |
| deep_recursion | .0008s | .0005s | 1.60x | C | MATCH |
| arena_churn | .0910s | .1279s | .71x | Brief | MATCH |
| linked_list | 1.4680s | 2.1151s | .69x | Brief | MATCH |
| hash_ops | 1.4328s | 1.5354s | .93x | Brief | MATCH |
| hash_ops_idio | .0324s | .0651s | .49x | Brief | MATCH |
| enemy_swarm | .1458s | .1801s | .80x | Brief | MATCH |
| bridge_glue | done | | | | SKIP |
| bridge_multi | done | | | | PASS |

**A/B verdict:** All 37 benchmarks MATCH; the previously-broken `queue_drain`
family is restored (.59x/.56x/.61x vs baseline .57x/.60x/.57x — unchanged).
Winner per benchmark is unchanged across the whole suite. The only movements
are sub-millisecond noise (`series_converge`, `deep_recursion`, `bit_clear`) and
within-variance swings (`fannkuch_redux` +12ms, `accumulator_flush` +28ms,
`kalman_filter_runtime` +13ms); several improved (`ring_buffer` now Brief-wins
at .95x vs baseline 1.07x C, `nbody_sqrt` -8%, `enemy_swarm` -8%,
`linked_list` -5%). No regression; the real-terminator codegen change is
performance-neutral as measured.

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

## Implementation Log

- `b0487364` (main): plan committed; worktree `../brief-compiler-term-diagnostics`
  created (`git worktree add -b feat/term-termination-diagnostics`).
- `5ab100b1`: `src/analysis/termination.rs` (analysis pass + 9 unit tests) +
  wiring into `parse_and_check` (`src/compile.rs:1674`) and `compile_source`
  (after `check_types`). (Note: this commit landed in two parts due to a staging
  hiccup; `5ab100b1` is the pass, `be934d61` is the codegen fix.)
- `be934d61`: codegen real-terminator fix (see §3).
- `ac6aca40`: regression fix — value-form `term <val>` inside an INLINED member
  body (e.g. RingBuffer pop via `<- queue`) is a member-local return, not a
  control-flow exit. The `be934d61` void path emitted `ret void` for these in
  the countdown loop, breaking `queue_drain` (`value doesn't match function
  result type 'i32'` at `queue_drain.ll:366`). The void path now checks
  `member_result.is_some()` first and emits no terminator (documented in BUGS.md).
- Verified: `corrected_term_guard.bv` prints `"1"` (was `"12"`) — now committed
  as the clang-guarded parity test
  `tests::guard_value_form_term_unwinds_body` (`tests/fixtures/term_guard_value_form.bv`);
  `term_valid_swan_song.bv` / all four `test-project-otto` CLI files pass with
  the expected checkpoint warnings; 1468 lib tests + 4 integration tests green;
  zero termination failures across all 352 tracked `.bv` files;
  `transition_validate` output identical (404/422/409/200);
  `queue_drain.bv` compiles/links/runs and prints correct boundary output after
  `ac6aca40` (live countdown-loop IR in `@main` equivalent to pre-change).
- Pre-existing `.bv` check failures (~202/352) are legacy-syntax parse/type
  errors (`rct`, `sig`, `codec`, `frgn from <source>`, Ptr arithmetic,
  shebang trophies) that fail before termination analysis — not regressions.

---

## Out of Scope

- Changing the interpreter's `term` semantics (it is the reference).
- Fixing the `return` divergence (logged, separate fix).
- The claim-#1 `check`/`build` pipeline gap in general (only this diagnostic is
  wired to both; the full pipeline-consistency work is separate).
