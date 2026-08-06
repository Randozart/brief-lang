# `endprogram` + `beginprogram` — Process Boundary Keywords

**Date:** 2026-08-06
**Status:** Active implementation plan
**Branch:** `feat/accel-gpu` (worktree `../briv-compiler-accel`)
**Companion SPEC change:** `spec/SPEC.md` §11.5 + a new entry-loop subsection,
`src/vocab.rs`, `syntax-highlighter/syntaxes/briv.tmLanguage.json`.

---

## 0. Executive Summary

Two language-semantics fixes discovered while validating the accel benchmark:

1. **`endprogram`** — the `exit program` statement is systematically conflated
   with `term` (transaction end) in every backend: LLVM and VM emit a plain
   `ret`, the interpreter and reactor treat it as `TermReturn`/`TermSuccess`.
   A node whose precondition stays true therefore re-fires forever — the
   reactor only ends by convergence, never by an actual process exit. SPEC
   §11.5 says `exit program` should "complete the process boundary" — the
   implementation diverges. History: the removed `term!` keyword used to close
   the program; its role passed to `exit program`, implemented as `term`
   (vocab.rs `VocabStatus::Removed`), and `display.rs` still renders the
   statement as `term!`. This plan renames the keyword to **`endprogram`**
   (a single token, killing the two-word `exit program` + the legacy `term!`
   render) and restores the real process-exit semantics.

2. **`beginprogram`** — a new, optional precondition keyword marking the
   **program entry**: true exactly once at program start. A `beginprogram`
   node is an **entry loop**: entered once when its state conditions hold
   (the precondition is evaluated once, never re-checked), then the body
   loops until the postcondition (goal) is met. Termination (goal
   reachability) and entry conflict (at most one beginprogram node's
   conditions hold at start) are **compile-time proof obligations** —
   unprovable ⇒ compile error.

The accel work drives the motivation: `nbody_newton_accel.bv` needs an
explicit entry (`init_bodies`), which today is a phase-0 hack; with
`beginprogram` the entry is declared, and the entry-loop is exactly the
work-item-map shape the accel analysis already proves.

---

## 1. Motivation

### 1.1 The `exit program` bug (found via the accel benchmark)

`benchmarks/nbody_newton_accel.bv` printed its observable forever. Root
cause: the `report` node's precondition (`count == bound`) stays true after
the bound is reached, so the reactor fires it every tick. The `exit program`
inside it does NOT exit the process — it is emitted as a plain transaction
`ret` (LLVM emit_stmt.rs:466 `Term | ExitProgram` → one path; VM
emit_stmt.rs:106 → `emit_ret`; interpreter eval.rs:851 → `TermReturn`;
reactor.rs:284 → `TermSuccess`). The program only ends by reactor convergence.

The conflation is a legacy artifact: `term!` (removed) used to "close the
program"; `exit program` inherited the role but was implemented as `term`.
`display.rs:313` still renders `Statement::ExitProgram` as `term!`.

### 1.2 The `beginprogram` motivation

The accel benchmark's entry node uses a `phase == 0` flag to mark "the node
that starts the program". That is implicit and hacky. `beginprogram` makes
the entry explicit and — crucially — the entry-loop semantics (enter once,
loop until the goal) is exactly the counted-loop/work-item-map shape the accel
analysis already proves.

---

## 2. Locked Decisions

| # | Decision |
|---|---|
| D1 | Keyword: `endprogram` replaces `exit program` (single token). `exit` is removed as a keyword |
| D2 | `endprogram` genuinely exits the process: LLVM emits `call void @__exit(i64 code)` (briv_rt.c wrapper, runs atexit cleanup) |
| D3 | Exit code = the statement's value via `adapt_to_i64`; bare `endprogram;` exits 0; `endprogram 5;` exits 5; `endprogram println!(x);` prints then exits 0 (Print# returns i64 0) |
| D4 | `beginprogram` is a keyword usable as a conjunct in a node precondition: `[beginprogram && <state-condition>]` |
| D5 | `beginprogram` is a PURE marker — true exactly once at program start. It takes no conditions itself; the node's other precondition terms are ordinary STATE expressions (top-level `let` bindings seeded from env at startup, e.g. `let startingnumber: Int = get_env_int!("env_var")`) |
| D6 | A `beginprogram` node is an ENTRY LOOP: entered once when its state conditions hold; the precondition is evaluated ONCE at entry, NEVER re-checked during the loop; the body loops until the postcondition (goal) is met |
| D7 | Termination proof (compile-time): the body must provably progress toward the goal; unprovable ⇒ compile error. Reuses the accel counter-increment verification, generalized |
| D8 | Entry-conflict proof (compile-time): at most one beginprogram node's state conditions can hold at program start; unprovable overlap ⇒ compile error. The entry dispatch is a RUNTIME evaluation of the state conditions (the compiler cannot statically select — the conditions depend on env-seeded state) |
| D9 | `[true]` goal on a beginprogram node ⇒ the loop runs exactly once (goal immediately satisfied) |
| D10 | `beginprogram` scoped to `node` declarations (an entry concept) |
| D11 | The plan doc is written first; SPEC is updated after, in the same implementation commits (SPEC is authoritative) |

---

## 3. Research Findings

- `Token::Exit` (`src/lexer.rs:99`) is used in exactly three places: the
  lexer, `parser/helpers.rs:389` (keyword map), and
  `parser/statements.rs:52` (`parse_exit_program_statement`, which
  `eat_identifier("program")`-s the second word, lines 175-185).
- `Statement::ExitProgram(val)` — AST variant, handled in ~12 files
  (parser, ast/display, ast/canonical, interpreter/eval, reactor,
  backend/{vm,llvm,webstack}/emit_stmt, backend/normalizer,
  derive/verify_smt, fuzzing/ast_generator).
- `display.rs:313` renders `ExitProgram` as `term!` — a removed keyword.
- `term!` is `VocabStatus::Removed` (`src/vocab.rs:194`); the parser errors
  direct the user to `term;` / `term!` closes the program (definitions.rs:271,
  statements.rs:89).
- `briv_rt.c` provides `void __exit(int64_t code)` (lib/runtime/briv_rt.c:578)
  — always linked via the archive step.
- SPEC §11.5: `term expression;` (complete current callable/transition),
  `exit program;` (complete the process boundary), `exit program code;`
  (exit code). §11.5 also states "A program converges (and exits) when no
  node can fire."
- Preconditions parse as `Expr` inside `[ ... ]`
  (`parse_single_contract_condition`, definitions.rs:1101). A keyword token
  inside a precondition needs explicit translation to a recognized predicate.
- `exit program` appears in 11 benchmarks + 7 examples + SPEC + 3 plan docs
  (2 historical — records, untouched) + BUGS.md + 4 src files (comments) +
  the active accel plan.
- The accel analysis (`src/analysis/accel.rs`) already verifies the counter
  increments toward the bound — this is the termination/progress proof
  primitive for `beginprogram` entry-loops.

---

## 4. Part 1 — `endprogram`

### 4.1 Keyword rename

- **Lexer**: `#[token("endprogram")] EndProgram` replaces
  `#[token("exit")] Exit` (`src/lexer.rs:99`). Display `"endprogram"`.
- **Parser** (`src/parser/statements.rs:52,175`): `Some(Token::EndProgram)`
  → parse `endprogram [value];` (single token; drop `eat_identifier("program")`).
- **AST**: `Statement::ExitProgram` → `Statement::EndProgram` (mechanical
  sweep of the ~12 files). `display.rs` renders `endprogram <value>;` /
  `endprogram;`.
- **Sources**: `endprogram` in 11 benchmarks + 7 examples.
- **vocab.rs**: `kw("endprogram", Canonical, Statement)`; drop `kw("exit", ...)`
  and the `"exit program"` list entry.
- **highlighter**: add `endprogram` keyword to
  `syntax-highlighter/syntaxes/briv.tmLanguage.json`.
- **helpers.rs**: keyword map.

### 4.2 Real exit semantics

- **LLVM** (`src/backend/llvm/emit_stmt.rs:466`): split `Term(val)` from
  `EndProgram(val)`.
  - `Term(val)`: unchanged (ret; transaction end, reactor continues).
  - `EndProgram(val)`: emit the value's side effects (the print), then
    `call void @__exit(i64 <code>)`; `<code>` = the value via
    `adapt_to_i64`, or `0` for the bare form. Declare `declare void
    @__exit(i64)` (briv_rt.c, already linked; runs atexit cleanup).
- **Interpreter** (`src/interpreter/eval.rs:851`): `EndProgram(val)` →
  a distinct `RuntimeError::ProgramExit(value)` (not `TermReturn`).
- **Reactor** (`src/reactor.rs:284`): `EndProgram` → distinct
  `StmtResult::ProgramExit(code)`; the reactor's top firing loop breaks on
  it and returns the exit code.
- **VM** (`src/backend/vm/emit_stmt.rs:106`): `EndProgram` emits the value +
  an exit opcode (the tamer loop treats it as process exit).
- **webstack** (`src/backend/webstack.rs:516,830`): `EndProgram` → a real
  exit call in the JS shim (or `process.exit`, per the target), not `return`.

### 4.3 Tests

- A node `[true][done]` with `done = 1; endprogram println!(x);` prints once
  and exits 0 (no infinite loop) — a regression for the benchmark bug.
- Existing benchmarks (nbody, fannkuch, precompute_sum) unchanged — the
  `when`-guarded `endprogram` fires once and exits cleanly.
- Interpreter: a reactive program with `endprogram` stops the reactor with
  the exit code.

---

## 5. Part 2 — `beginprogram`

### 5.1 Language

```briv
let startingnumber: Int = get_env_int!("env_var");

node entry1 [beginprogram && startingnumber == 1][done] {
    ...
    done = 1;
    term;
};
```

- `beginprogram` is a keyword, a conjunct in a node's precondition. It is a
  PURE marker: true exactly once at program start; it takes no conditions.
  The node's other precondition terms are ordinary STATE expressions
  (top-level `let` bindings seeded from env/compile-time at startup).
- Multiple nodes may carry `beginprogram`; the entry-conflict proof (D8)
  guarantees at most one can be entered.

### 5.2 Semantics (entry loop)

1. Entered exactly once when `beginprogram` fires AND the node's state
   conditions hold.
2. The node ITSELF is a loop: the body runs repeatedly until the
   postcondition (goal) is met.
3. The precondition is evaluated ONCE at entry — never re-checked during
   the loop.
4. Termination is a compile-time proof (D7): the body must provably
   progress toward the goal. Unprovable ⇒ compile error with evidence.
5. `[true]` goal ⇒ the loop runs exactly once (D9).
6. Entry dispatch is RUNTIME (D8): at startup the state is initialized
   (env lets resolved), then the beginprogram nodes' state conditions are
   evaluated; the conflict proof guarantees exactly one holds.

### 5.3 Compiler surfaces

- **Parser**: `Token::BeginProgram`; inside a precondition bracket,
  `beginprogram` translates to a recognized predicate; a node-level
  `is_begin` marker is extracted (cleaner than scattering the keyword into
  the precondition Expr).
- **Transition graph / analysis**: recognize beginprogram nodes → entry-loop
  shape (goal-driven exit, not pre-driven). 
  - **Termination proof**: generalize the accel counter-increment
    verification — the body must advance the state named by the goal toward
    the goal. Unprovable ⇒ compile error.
  - **Entry-conflict proof**: the beginprogram nodes' state conditions are
    mutually exclusive at startup (satisfiability check, like the
    concurrency gate's XOR). Unprovable overlap ⇒ compile error.
- **Backend**: the entry-loop compiles as a loop entered once at startup
  (entry check → body → until goal), not a per-tick reactive node.
  - **Accel integration**: an accel entry-loop is the work-item map; the GPU
    dispatch (one launch of N) replaces the loop; the accel eligibility's
    increment verification IS the termination proof.
- **vocab.rs / highlighter**: `beginprogram` keyword.

### 5.4 Tests

- `node entry [beginprogram && flag == 1][done]` enters once, loops until
  `done`, halts; a second beginprogram node with a contradictory condition
  does not conflict; an overlapping one is a compile error.
- Termination proof: a beginprogram node whose body does not advance toward
  the goal is a compile error.
- `[true]` goal: runs exactly once.
- Accel entry-loop: `[beginprogram && i < nb][i == nb]` compiles, runs nb
  iterations on CPU, and the accel analysis proves the map.

---

## 6. Benchmark Changes

`benchmarks/nbody_newton_accel.bv`:

- **Revert the `done`-flag workaround** (Phase 8 session): the `report` node
  returns to `[count == bound][true]` with `endprogram println!(px[0]);` —
  the real exit now terminates the process before the precondition can
  re-fire.
- **Entry via `beginprogram`**: `init_bodies [beginprogram && i < nb][i == nb]`
  replaces the `phase == 0` entry hack (the phase flag remains only for the
  init→step sequencing).

---

## 7. Execution Order

1. Write this plan + commit.
2. Revert the `done` workaround in `nbody_newton_accel.bv`.
3. Part 1 code: lexer/parser/AST rename + display + sources + vocab +
   highlighter + helpers.
4. Part 1 semantics: LLVM `__exit`, interpreter `ProgramExit`, reactor
   `StmtResult::ProgramExit`, VM, webstack.
5. Part 1 tests + commit (`cargo test --lib` green, Praetor on changed dirs).
6. Part 2 code: parser `beginprogram` + `is_begin` marker; analysis
   (entry-loop, termination proof, conflict proof); backend entry-loop
   emission + accel integration.
7. Part 2 tests + commit.
8. SPEC §11.5 + entry-loop subsection + vocab + highlighter (same commits as
   the code they document — SPEC is authoritative, updated after the plan).
9. Update `AGENTS.md` if command/reference tables change.
10. Full suite + Praetor + final commit.

---

## 8. Open Items

- The reactor's exact propagation of `StmtResult::ProgramExit` (where the
  top loop breaks and returns the code) — traced during implementation.
- `beginprogram` + `sync<group>` / `async` interaction — the entry node is a
  one-shot loop; verify it does not need concurrency classification.
- `endprogram` inside an accel body's host statements (the GPU wrapper skips
  host statements — a pre-existing gap; the exit would run on the CPU path
  only).
- **Backend entry-loop semantics (the core beginprogram gap).** The keyword,
  typechecker proofs (goal reachability + entry conflict), and the
  `[i == nb]` goal validation are in. The backend still emits
  `Expr::BeginProgram` as constant `true`, so a beginprogram node fires like a
  normal node (pre re-checked every tick) and must be gated by a phase flag in
  the benchmark. The full semantic — beginprogram true only until the node's
  goal is met, the precondition evaluated once at entry and never re-checked,
  entered once at startup — requires a per-node begin flag
  (`@briv_begin_<name> = global i1 1`, read in the precondition, cleared when
  the goal is met in the reactor's post check). Then `[beginprogram && i < N]`
  alone drives an entry loop with no phase gate.
