# Plan: Restore the Object Layer, Re-establish Type Validation, and Align the Docs

**Date:** 2026-07-31
**Author:** session lead (frontend-driven-dispatch workstream)
**Status:** Approved — execution in progress
**Required reading:** `docs/handoff-methodology.md` (the rigorous loop), `AGENTS.md` (rules), `docs/2026-07-31-session-report.md` (session findings)

---

## 1. Goal

Briev's compiler accepts programs that are not well-typed, mis-parses its own
object layer (field access, reflection, method calls), emits void garbage for
unimplemented expressions, and documents syntax it cannot parse. This plan
restores the object layer and the type system to a state where **`brievc check`
is a real gate** — it rejects every malformed program — and the docs describe
exactly what the compiler implements.

The plan is contract-first: nothing below weakens a language feature or a
benchmark. Where a subsystem is genuinely not yet implementable (generic obj
instantiation), the compiler reports a **precise error**, and the SPEC states
the boundary — it does not silently emit garbage.

## 2. Baseline (Golden Rule 11)

**Compiler:** current `main` (`e9adc57b`) + uncommitted parser contract-position
fix (1279→1285 tests, all green). Benchmarks are flat-state programs and do not
exercise the object layer, so their numbers are unaffected by this plan's
compiler phases; the suite guards against regressions.

**Runtime benchmark ratios vs C (`BOUND=50M`, `-O3 -flto`, zero MISMATCH),
from `benchmarks/results/2026-07-31-countdown-loop.md` + session harness:**

| Benchmark | Ratio (Briev/C) | Direction |
|-----------|----------------:|:---------:|
| kalman_filter_runtime | 0.85× | Briev faster |
| float_math_nonzero | 0.94× | Briev faster |
| float_math | 0.62× | Briev faster |
| print_loop | 0.64× | Briev faster |
| queue_drain (×3) | 0.47× / 0.62× / 0.57× | Briev faster |
| matrix_pipeline | 0.66× | Briev faster |
| accumulator_flush | 0.71× | Briev faster |
| telemetry_stream | 0.99× | parity |
| pid_control | 0.97× | parity |
| sweep_sparse / mid / dense | 1.40× / 1.10× / 1.49× | C faster (known gap, Phase-3 dispatch follow-up) |
| all original benchmarks | within noise of Phase 3 | — |

**Test suite:** `cargo test --lib` = 1285 passing, 0 failing at plan start.

## 3. Investigation — the verified defects (evidence, not opinion)

Every item below was reproduced with `brievc check` at the plan-start commit;
`file:line` anchors are cited.

### Group A — the postfix receiver subsystem is broken end-to-end

Root cause: `src/parser/expressions.rs:241-264` — both `.` and `.#` replace the
receiver expression with `Expr::PropertyGet(String)`, **discarding it**.
`Expr::Field(Box<Expr>, String)` exists in `src/ast/expr.rs:31` but is **never
constructed** by any code path. Consequences:

| # | Feature | Repro | Result |
|---|---------|-------|--------|
| A1 | struct field access | `p.name` where `p: Person` | `undefined variable 'name'` (receiver lost) |
| A2 | tuple index | `coords.0` | same bug (`PropertyGet("0")`) |
| A3 | `.#` property in a body | `let n: Int = items .#Size;` | `undefined variable 'Size'` |
| A4 | method call | `s.trim()` | parse error `only named functions can be called` (`expressions.rs:238`) |
| A5 | generic `obj` + members | `obj Stack<T, N> { txn push… }` | parse error `expected identifier, found '<'` (`definitions.rs:1584` `parse_obj_like` handles only simple slots) |
| A6 | collections stdlib | `lib/std/collections.bv` | fails to parse (depends on A4+A5) |
| A7 | backend `PropertyGet` | — | emits `add i64 0, 0` with `Type::void()` — silent wrong codegen (`emit_expr.rs:762-768`) |

The benchmark suite is green because every benchmark uses flat scalar state;
none exercise `.`, `.#`, or method calls.

### Group B — type validation is absent

| # | Gap | Repro | Site |
|---|-----|-------|------|
| B1 | `let x: Int = "hello"` (body) | OK | `infer_statement` infers the initializer but never compares it to `ty` (`typechecker/mod.rs:749-755`) |
| B2 | top-level `let` initializer | OK | `check_top_level` falls through `_ => Ok(())` (`typechecker/mod.rs:1040`) |
| B3 | `defn f() -> Int { term "hello"; }` | OK | `Term` arm only infers the value (`:773`) |
| B4 | `takes_int("hello")` | OK | call args inferred, never compared to param types |
| B5 | `[true][true]`, `0 == 0` | accepted | no tautology detection anywhere |

### Group C — codegen stubs and panics

| # | Site | Today |
|---|------|-------|
| C1 | `emit_expr.rs:762` | `PropertyGet` → void stub |
| C2 | `emit_expr.rs:777` | `Expr::Exists(_) => unreachable!("fn? only in stage eval")` |
| C3 | `emit_expr.rs:771` | `Expr::PluginIntercept { .. } => panic!("unresolved plugin-intercept call reached codegen")` |
| C4 | `typechecker/mod.rs:464` | `Expr::Exists(_) => unreachable!()` |

### Group D — documented but unparseable

Watchdog `?[5000ms]` / `![1000ms]` is a parse error
(`expected identifier, found '?'`). `Contract.watchdog: Option<WatchdogSpec>`
exists (`ast/top.rs:140`); `parse_contract` always sets `None`
(`definitions.rs:844-891`); only the legacy COBOL backend consumes it
(`backend/cobol.rs:404`). SPEC §2.5 and `learn-briev/02-contracts.md` §7
document the syntax as implemented. **Decision (user): add it back in.**

### Group E — the gate

`brievc check` accepts every item in Group B and most of Group A. It is not a
real typecheck gate.

## 4. Design decisions (final, locked this session)

### D1 — Reflection: `expr.^Meta` (runtime) and `expr.^^Meta` (compile-time)

- `^` remains bitwise XOR (`a ^ b`) — **unchanged**; the `.` disambiguates.
- `.^` and `.^^` are new postfix operators (tokens `DotCaret`, `DotCaretCaret`),
  binding like field access (tightest precedence).
- Targets are **PascalCase compiler-known identifiers**, explicitly marked by
  the operator — satisfying the "no hidden magic" rule.
- `expr.^Meta` = runtime reflection (value-derived). `expr.^^Meta` =
  compile-time reflection (type-derived, **foldable**, usable in `const`
  initializers and contract expressions).
- The operator is the static/runtime guarantee: `.^^` results are compile-time
  constants by construction; `.^` results are runtime values. No per-target
  foldability analysis is needed.

**Target table:**

| Target | Kind | Result | Foldable | Semantics |
|--------|:----:|--------|:--------:|-----------|
| `Len` | `.^` | `Int` | no | runtime length of a String/List value |
| `Ptr` | `.^` | `Ptr<T>` | no | address-of; `&x` is primary |
| `Size` | `.^^` | `Int` | yes | fixed-size element count (`Int[8].^^Size` → 8) |
| `Bytes` | `.^^` | `Int` | yes | storage size of the type |
| `Alignment` | `.^^` | `Int` | yes | alignment of the type |
| `Type` | `.^^` | type token | yes | type identity, usable in `as`/cast position |

Errors: `.^` before a compile-time-only target (and vice versa) →
`reflection target 'Size' is compile-time; use '.^^'`. Unknown target →
`reflection target 'X' is unknown`.

**History:** supersedes the `. #` DotHash property access (removed), the
intermediate `.^`-operator-with-`#`-suffix idea, and the `Len^()`-suffix idea.
The final form is the least syntax with the clearest static/runtime signal.

### D2 — Field access: `Expr::Field(Box<Expr>, String)`

Already in the AST (`ast/expr.rs:31`); wire the parser to construct it.
Numeric field names (`.0`, `.1`) are tuple indices.

### D3 — Method calls: `Expr::MethodCall(Box<Expr>, String, Vec<Expr>, Option<usize>)`

New AST variant. `obj` members (txn/defn) receive an implicit `self` binding
bound to the receiver. Args validated against the member signature. Rejected
alternative: desugaring `a.m(x)` → `Call("m", [a, x])` — would require
uniform-call semantics and blur the method namespace.

### D4 — Generic `obj`: `obj Stack<T, N> { … }`

`parse_obj_like` extended to accept type params + member `txn`/`defn` bodies
(self-parameterized), collected into `TypeDefBody`. **Boundary (explicit):**
parse + member collection + concrete-receiver method dispatch this phase;
generic **instantiation** (monomorphization) is a scheduled follow-up — the
compiler reports a precise `generic obj 'Stack' requires type instantiation;
not yet supported — use a concrete obj` error at build time, never garbage.
SPEC documents this boundary.

### D5 — Watchdogs

`?[expr]` (optional) / `![expr]` (required) parsed back into
`Contract.watchdog`. LLVM emits a deadline check in the loop engines (required →
error-exit path; optional → note). COBOL already consumes it.

### D6 — Tautologies at proof time

`prove_contract` / `check_satisfiable` flag functionally-always-true contracts
(`[true][true]`, `0 == 0`, `x == x`) with the `[[post]`/`[pre]]` hint. The
parser stays permissive (contracts are parsed structurally; proof is where
"does this constrain anything" is decided).

### D7 — Contracts in both positions + explicit/implicit return (already implemented)

`parse_output_and_contract` (`definitions.rs`) accepts the return type and
contract in either order; the return type may be omitted (inferred from
`term`). Array-size `[N]` parsing is non-greedy (a non-integer `[` after a
return type is a contract). **Uncommitted at plan start; banked in Phase 0.**

### D8 — `.#` is removed

The historical runtime `.#Size` (collection/string length) becomes `.^Len`; the
compile-time metadata becomes `.^^Size`/`.^^Bytes`/`.^^Alignment`/`.^^Type`;
`.#Ptr`/`.#Ptr!` become `.^Ptr`/`&`. Migration is **semantic, not mechanical** —
each `.#X` maps by its static/runtime nature. No alias retained.

## 5. Phases (each independently verifiable, committed separately)

### Phase 0 — Bank the green state
Commit the uncommitted contract-position parser fix + 6 tests (`definitions.rs`
`tests::test_defn_contract_*`, `test_txn_contract_after_return_type`,
`test_array_type_still_parses_with_contract_after`,
`test_non_integer_bracket_left_for_contract`) + tutorial edits 00–07
(1285 pass, Praetor clean). **Then update the baseline worktree to this commit.**

### Phase 1 — Rebuild the postfix layer (fields, reflection, methods)
1. **Lexer** (`src/lexer.rs`): add `DotCaret` (`#[token(".^")]`) and
   `DotCaretCaret` (`#[token(".^^")]`); remove `DotHash` (`.#`). Update the
   token `Display` impls and any token tests.
2. **Parser** (`src/parser/expressions.rs:241-264`): `.` → `Expr::Field(receiver,
   name)` (preserve the `$`-nav-call branch); `.^`/`.^^` → `Expr::Reflect(receiver,
   name, kind)`; delete the `.#` branch. Verify the receiver is preserved in
   each.
3. **Parser** (`src/parser/definitions.rs:1584`): `parse_obj_like` accepts
   `<T, N>` params and member `txn`/`defn` bodies (self-parameterized) into
   `TypeDefBody`.
4. **AST** (`src/ast/expr.rs`): add `ReflectKind { Runtime, CompileTime }`,
   `Expr::Reflect(Box<Expr>, String, ReflectKind)`,
   `Expr::MethodCall(Box<Expr>, String, Vec<Expr>, Option<usize>)`; remove
   `Expr::PropertyGet(String)`. Update `collect_vars_into` and `Display`.
5. **Consumers** (all 17 `PropertyGet` sites): `typechecker/mod.rs`,
   `backend/llvm/{emit_expr,helpers,mod}.rs`, `backend/mod.rs`,
   `interpreter/eval.rs`, `analysis/{dataflow,dependency_graph,licm,allocation}.rs`,
   `annotator.rs`, `symbolic.rs`, `plugin/env_plugin.rs`, `macros/eval.rs`.
   Each gets Field/Reflect/MethodCall arms (the compiler enumerates missing
   arms during the build).
6. **Typechecker**: `Field` → struct-field / tuple-index lookup → field type;
   `Reflect` → the D1 table (kind validation, result type, foldability);
   `MethodCall` → member lookup on the receiver's obj type, `self` + params
   bound, args validated.
7. **LLVM emission**: `Field` → GEP/extractvalue; `Reflect` → constants
   (`Size`/`Bytes`/`Alignment`/`Type`) and runtime (`Len` intrinsic, `Ptr`
   address-of); `MethodCall` → member body with `self` bound to the receiver
   storage pointer.
8. **`.#` migration** (same commit): stdlib `.#Size`/`.#Bytes`/`.#Ptr` →
   `.^Len`/`.^^Size`/`.^Ptr` (semantic mapping per D8).
9. **Tests** (regression per A1–A6): field access on `obj`/tuple, `var.^Len`
   in a body, `var.^^Size` foldable in a contract, `.^Ptr`, `s.trim()`,
   concrete `obj` with a `txn push` called via method, `collections.bv`
   parses. Assert `brievc check` fails on the old broken inputs.

### Phase 2 — Re-establish type validation
1. `let` (`typechecker/mod.rs:749`): compare inferred vs declared → `TypeMismatch`
   (coercion-aware: `as` casts and cross-type overloads honored — reuses the
   Int×Float machinery).
2. Top-level `let` (`:1040`): route `TopLevel::Statement(Let)` through
   `infer_statement`.
3. `Term`/`TermBang` (`:773`): validate against declared `output_type`.
4. Call args: validate each against the callee's param types (defn/txn/method).
5. Group C: `unreachable!()`/`panic!`/void-stub → `BackendError` with spans.
6. **Tests** per B1–B4 and C1–C4; assert `brievc check` fails on each.

### Phase 3 — Watchdogs
1. `parse_contract` (`definitions.rs:844`): parse `?[expr]`/`![expr]` after
   `[post]` → `Contract.watchdog`.
2. LLVM: deadline check in the countdown/version-DAG loop engines.
3. **Tests**: required/optional parse + round-trip + LLVM emission.

### Phase 4 — Proof-time tautology detection
1. `prove_contract`/`check_satisfiable` (`proof_engine/mod.rs:20,249`): flag
   functionally-always-true contracts with the sugar hint.
2. **Tests**: `[true][true]`, `0 == 0`, `x == x` rejected; real contracts
   accepted.

### Phase 5 — Docs alignment
1. Normalize `var.^Len` / `var.^^Size` style everywhere; no `.#`, no `:>`.
2. `05-data-types.md` §10 Ptr: lead with `&x`/`*p`; `.^Ptr` as reflection.
3. Finish node/txn/defn reclassification (00–12) with the "called by a txn or
   node" and defn-no-external-mutation wording.
4. `15-custom-types.md`: RHS-only op overloads (`op Add(Float): func(#L, #R)`;
   never `op Add(#Int, #Int)`).
5. SPEC: de-dup §2.3–2.6 and §3.x; new **Reflection** section (`.^`/`.^^`
   static/runtime contract, PascalCase rule, `.^` vs `.m()` boundary, the
   `xor` note that `^` is unchanged); `meld <:>` → `->`; RHS-only op grammar;
   node/txn/defn section; §1.4 architecture refresh; watchdogs updated to
   implemented.
6. `13-projections.md` → **Reflection chapter** (`Len`/`Size`/`Bytes`/
   `Alignment`/`Type` via `.^`/`.^^`).
7. `04-functions.md`: "contracts in both positions + explicit/implicit return"
   section verified against the implemented parser.

## 6. Verification (per AGENTS.md)

- `cargo test --lib` green after every phase; one regression test per A/B/C/D
  item.
- `brievc check` **must fail** on every previously-accepted-invalid input —
  the "gate is real" assertion.
- `cargo build` no new warnings; Praetor (one `--target` per invocation) on
  changed files; Kani if the `self`-binding member dispatch introduces unsafe
  indexing.
- Benchmark suite still green (flat-state programs unaffected by Phase 1 —
  verify no ratio shift against §2).
- Architecture docs updated in the same commit as structural changes.

## 7. Risks and mitigations

| Risk | Mitigation |
|------|-----------|
| AST ripple (17 `PropertyGet` consumers + new arms) | build-per-phase; the compiler enumerates missing arms |
| `.#` migration is semantic | explicit mapping table (D8); each migration verified against a compile |
| Method-dispatch + `self` codegen (largest unknown) | isolated in Phase 1; concrete receivers first, generic boundary explicit |
| Generic obj instantiation deferred | precise build error + SPEC boundary + scheduled follow-up — never garbage |
| Tamer/vm files (other agents') | only XOR lines are already correct (`^` unchanged); no edits unless a Phase-1 `.`/`.#` occurrence exists there (none does) |
| Benchmarks regress | §2 baseline + `compare_baseline.sh` after Phase 1 |

## 8. Documentation maintenance (Golden Rule 12)

- `docs/architecture/agent-reference.md`: update the operator taxonomy, the
  `.#`→`.^`/`.^^` mapping, the `PropertyGet` removal, the reflection table.
- `docs/architecture/backend-type-dispatch.md`: Field/Reflect/MethodCall
  emission notes.
- `spec/SPEC.md`: grammar + Reflection section (Phase 5).
- `learn-briev/13-projections.md` → Reflection chapter (Phase 5).
- `docs/handoff-methodology.md`: this plan becomes a second worked example
  reference after completion.

## 9. Results (filled after each phase)

- **Phase 0** — committed (`dea31ae3`); baseline worktree updated to it.
- **Phase 1** — committed (`4706845a`). Postfix layer rebuilt: `.` → `Field` /
  `MethodCall` (receiver preserved), `.^`/`.^^` → `Reflect` (DotCaret/
  DotCaretCaret tokens, `#` DotHash removed), `Expr::PropertyGet` deleted
  (it was dropping the receiver — field access, tuple index, and method calls
  were all broken). `parse_obj_like` accepts `<T,N>` params + member txn/defn
  (self-parameterized). Typechecker resolves Field slots/tuple indices,
  the D1 reflection table, and MethodCall member dispatch with type-arg
  substitution. Backend emits Field GEP-loads, compile-time reflection
  constants, `Ptr`→address-of; `Len`-on-dynamic and MethodCall codegen are
  documented Phase-1b boundaries (clear panics, never garbage). `.#`→`.^`/
  `.^^` migration across lib/std (~400 sites). Tests 1292 (+7).
  **Boundary:** generic obj instantiation and MethodCall/Len emission deferred;
  collection stdlib (collections.bv) needs generic structs + instantiation.
- **Phase 2** — committed (`2084086c`). The gate is real: `let x: T = expr`,
  top-level `let`, `term` vs declared return, and call args are all validated
  (no implicit coercion; literal Parse-ops, numeric-protocol members, and
  `op Init` construction remain sanctioned). Known plugin-intercepts typed for
  the plugin-free check path. `Expr::Exists` unreachable → proper error.
  Generic arrays `T[N]` parse. Latent benchmark errors fixed (PrintLn void
  annotation, GetEnvInt#→!, Malloc cast); queue_drain removed from the harness
  (depends on the D4 collections stdlib). Tests 1297 (+5).
- **Phase 3** — committed (`86978508`). Watchdogs parse back in: `?[expr]`
  optional / `![expr]` required → `Contract.watchdog`, with ms/cyc/seconds/
  minute units. LLVM deadline check is a documented follow-up (needs a time
  source in the loop engines). Tests 1300 (+3).
- **Phase 4** — committed (`d8d81b88`). Proof-time tautology detection:
  `is_vacuously_true` catches `true`, `0 == 0`, `x == x`; `detect_tautology`
  is the txn/node gate (explicit contracts only — `Contract.explicit` added).
  frgn return types registered so `term frgn_foo(x)` typechecks. Tests 1305
  (+5).
- **Phase 5** — committed. SPEC de-duplicated (3728→3483 lines; the doubled
  §2.3–2.6 and §3.x are single), `:>` projection and `<:` lens sections
  replaced with the Reflection `.^`/`.^^` spec + real meld, op grammar is
  RHS-only, §1.4 architecture refreshed, Ptr section leads with `&`/`*`,
  `async node` parses. Tutorials: `13-projections.md` is now the Reflection
  chapter, `15-custom-types.md` op syntax is RHS-only, `05-data-types.md` Ptr
  leads with `&`/`*`, all `. #`/`:>` swept to `.^`/`.^^`.
