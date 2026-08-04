# Remove Vestigial `return` Statement

**Date:** 2026-08-04
**Status:** Implemented — see Implementation Log.
**Branch:** `feat/term-termination-diagnostics` (worktree `../brief-compiler-term-diagnostics`)
**Related:**
- `docs/plans/2026-08-04-term-termination-diagnostics.md` (this plan's §5 and
  Out-of-Scope claimed the `return` divergence was "logged in BUGS.md" — it was
  not, and the divergence is resolved here by REMOVING the feature, not by
  aligning semantics).
- `AGENTS.md` Rule 4 (interpreter is reference) — moot once the statement
  cannot be written.
- `spec/SPEC.md` — never defined a `return` statement (only return *types*).

---

## The Problem

`return expr;` / `return;` is a **vestigial parser path**, not a Brief feature:

- **Parser** (`src/parser/statements.rs:54-55, 142-151`): dispatches on the
  identifier `return`, producing `Statement::Return(Option<Expr>)`. Introduced in
  the Phase 1 parser rewrite (`77836c35`).
- **Spec** (`spec/SPEC.md`): no `return` statement anywhere — "return" appears
  only as return *types* (`-> (T1, T2)`, FFI return values).
- **Usage**: zero `return` statements in any `.bv` in the compiler repo or
  `test-project-otto`; zero tests.
- **Semantics disagree across the pipeline**:
  - Interpreter (`src/interpreter/eval.rs:712`): evaluates the value and returns
    `Ok(Value)` — execution CONTINUES; the runner (`interpreter/mod.rs:129-149`)
    sets `result = value`, which the next statement overwrites. Mid-body
    `return x;` is effectively a no-op.
  - LLVM backend (`src/backend/llvm/emit_stmt.rs:568`): emits a real `ret` and
    sets `terminated = true` — a HARD function exit.
  - VM backend (`src/backend/vm/emit_stmt.rs:88`): treats `return` exactly like
    `term` (`emit_ret`).
  - So the two backends disagree with each other, and neither matches the
    interpreter. A user who writes `return` gets silently wrong codegen.

## Decision (maintainer)

**Remove the statement** AND raise a **helpful parse error** when `return` is
used, pointing the author at the Brief-native construct:

> Brief has no `return` statement. Use `term <value>` to return a value from a
> defn, bare `term;` as a convergence checkpoint, or `term!` to close the
> program.

Rejected alternatives: (B) leave it and fix the plan's false "logged" claim —
leaves a latent wrong-codegen hazard; (C) alias `return` ≡ `term <val>` in all
engines — more code, still unspecced, pointless since `term` exists.

## Changes

1. **Parser** (`src/parser/statements.rs`): replace the
   `check_identifier("return")` dispatch (lines 54-55) with a
   `SyntaxError::InvalidStatement` carrying the helpful message; delete
   `parse_return_statement` (lines 142-151). `return` becomes an ordinary
   identifier in non-statement positions (acceptable — it was never a reserved
   word and never used).
2. **AST** (`src/ast/top.rs`): remove the `Statement::Return(Option<Expr>)`
   variant (line 216) and its `PartialEq` arm (line 309).
3. **Display** (`src/ast/display.rs:291`): remove the Return arm.
4. **All `Statement::Return` match arms** (~50 across ~22 files): delete/merge.
   Includes `src/beast/serialize.rs:145` + `src/beast/deserialize.rs:353`
   (serialized programs cannot contain a statement that never parses).
5. **Termination analysis** (`src/analysis/termination.rs`): remove the
   `return`-in-test references (lines 173, 269) and any conservative-case
   handling — the variant no longer exists.
6. **Docs**: `BUGS.md` — note the vestigial feature was removed (resolves the
   dangling "logged" claim); the new plan's Implementation Log records it.

## Regression Tests

- Parser unit test: `return x;` / `return;` at statement position fails with the
  helpful message ("has no `return`"); a defn body using `return` fails `check`.
- `cargo test --lib` (1468 tests) stays green — proves no live program path
  depended on `return`.

## Benchmarks (rule #11)

Not required: no codegen path changes, no optimization-path changes. The only
behavior change is a parse error for previously-parseable-but-unspecced input.
Full runtime A/B was already run for the term work this branch; this commit
cannot affect benchmark IR.

## Verification

1. `cargo test --lib` green.
2. `cargo build --release` no new warnings.
3. Praetor on changed files — no NEW diagnostics.
4. `grep -rn "Statement::Return" src/` returns zero.
5. Manual: a `.bv` with `return x;` prints the helpful error; `--no-stdlib`
   build of an example still works.

---

## Implementation Log

- Parser: `return` at statement position and at top level now errors with the
  helpful message (`src/parser/statements.rs` dispatch +
  `src/parser/definitions.rs` top-level fallback); `parse_return_statement`
  deleted.
- AST: `Statement::Return` variant + PartialEq arm removed (`src/ast/top.rs`);
  display arm removed (`src/ast/display.rs`).
- All ~50 `Statement::Return` match arms removed across 37 files, including the
  LLVM `ret` emission (`src/backend/llvm/emit_stmt.rs`), the VM `emit_ret`
  alias (`src/backend/vm/emit_stmt.rs`), the interpreter
  (`src/interpreter/eval.rs`), the countdown-loop `ret i64`
  (`src/backend/llvm/loop_engine/counter.rs`), and the beast
  serialize/deserialize arms.
- Termination analysis: removed the `return`-specific unit tests (the
  "return is not a terminator" guarantee no longer applies).
- Tests: `parser::statements::tests::return_statement_errors_with_helpful_message`
  (return errors with the helpful message at top level, in a defn, and in a node
  body) + `term_statements_still_parse`. Full suite: 1469 lib + 5 integration +
  2 parser tests green.
- Docs: BUGS.md entry; plan reference from the term-termination plan (§5
  "logged" claim is resolved by removal).
- A `return` token still lexes as an ordinary identifier in non-statement
  positions (never a reserved word; zero usages to break).
