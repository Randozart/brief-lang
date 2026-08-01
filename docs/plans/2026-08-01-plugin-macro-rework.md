# Plugin / Macro Syntax Rework — `entry!`, `args!`, `print!`/`println!`, `[#]` Removal, FFI Audit

**Date:** 2026-08-01
**Status:** Approved — awaiting execution start
**Worktree:** `../brief-compiler-plugin-rework` (new, from `main` `d6c6c818`)
**Baseline worktree:** `../brief-compiler-baseline` — synced to `d6c6c818` on 2026-08-01

---

## 1. Goal

Rework the plugin / macro surface of Brief to match the language's disclosure
principle (AGENTS.md #2):

1. **Remove `[#]`** as a special-cased entry precondition in the parser. It is
   dead weight — parsed into `Contract.is_entry` but consumed by no backend or
   analysis pass.
2. **Replace it with `entry!("command")`** — a user-facing macro, unwrapped by a
   compiler plugin, that inserts a CLI-command precondition, creates top-level
   bindings/helpers to fetch that command, and composes a one-shot firing guard.
3. **Add `args!("flag")`** — the companion macro for CLI flags (Bool presence and
   typed value retrieval).
4. **Standardize macro naming**: user-facing `!` macros are lowercase /
   snake_case (`println!`, `print!`, `get_env!`, `get_env_int!`, `entry!`,
   `args!`). PascalCase is reserved for compiler-knowns (`Sqrt#`, `Tag$`, `#Int`,
   `.^`). PascalCase `!` intercepts become compile errors with a rename hint.
5. **`println!` / `print!`** gain Rust-style curly-brace substitution (`{}`,
   `{n}`) with positional arguments, rewritten at compile time to typed print
   intrinsic calls — zero runtime formatting.
6. **Flat-scripting plugin** synthesizes a one-shot opening node. **No generated
   node ever uses `[true]` as a precondition** (continuous-fire eligibility).
   Instead a top-level `Bool` guard is flipped at the end of the inserted node.
7. **Enforce the concurrency gate** (AGENTS.md #21) — an eligible-to-fire pair
   with no XOR read-write overlap and no `async`/`sync<group>` classification is
   a hard compile error.
8. **Research the FFI "native performance" regression** — full audit of the
   `frgn` Inline path, print/env intercept resolution, and the bridge path, with
   a documented baseline.

---

## 2. Operating contract

Every step honors: contract-first (#1), no hidden special treatment (#2),
interpreter-is-reference (#4), additive-only match arms (#5), ALWAYS FINISH (#6),
never discard uncommitted work (#7), tests-or-it-doesn't-exist (#8), no
prototyping (#9), plan-with-benchmarks (#11), baseline worktree A/B (#11b),
stdlib-is-the-extension-mechanism (#13), no compiler knowledge of specific types
(#14), full provenance tracking (#15), DRY (#16), migrate-when-touched (#17), no
type-name matching (#18), measure-before-build (#19), delimiter semantic load
(#20), no implicit concurrency (#21), and the Performance Recovery Protocol.

---

## 3. Current-state research findings (verified 2026-08-01)

| Area | Finding | Location |
|------|---------|----------|
| `[#]` parsing | Special-cased in `parse_contract`; sets `Contract.is_entry` | `src/parser/definitions.rs:872-959` (branch at `:880-908`) |
| `is_entry` consumption | **None.** Only display, beast serialization, tests | `src/ast/display.rs:476`, `src/beast/serialize.rs:52,81-83,365`, `src/ast/top.rs:137`, `src/ast/mod.rs:8` |
| `is_entry` constructor sites | ~50 `is_entry: false` (mechanical sweep) | `backend/llvm/tests.rs`, `backend/mod.rs:645`, `backend/circt.rs:795`, `backend/spirv/mod.rs:66`, `backend/webstack.rs:1108`, `fuzzing/*`, `assertion_verify.rs`, `reactor.rs`, `plugin/intrinsics.rs`, `analysis/*`, `hardware_validator.rs` |
| `defn main` | Emitted as `brief_main` (`emit_toplevel.rs:1133`) but **never called** by the runtime — reactor only fires reactive `txn`/`node` | `src/backend/llvm/emit_toplevel.rs:1133` |
| Implicit entry wrap | `wrap_implicit_entry` is an empty placeholder | `src/parser/definitions.rs:1210-1213` |
| Plugin intercept syntax | `name!(args)` postfix → `Expr::PluginIntercept` | `src/parser/expressions.rs:319-339`, `src/ast/expr.rs:80-86` |
| Intercept rewriting | Rust plugins at Parsed stage: `Print`/`PrintLn` → `PrintInt#`/`PrintStr#`/`PrintFloat#`/`PrintChar#`; `GetEnv`/`GetEnvInt` → stdlib `get_env`/`get_env_int` | `src/plugin/print_plugin.rs`, `src/plugin/env_plugin.rs` |
| Plugin registration | `EnvPlugin` + `PrintPlugin` hard-registered | `src/compile.rs:861-862`; `config/targets.toml [".bv"].plugins = ["prelude","env","print"]` |
| Typechecker intercept arm | Recognizes `GetEnvInt`, `GetEnv`, `GetEnvOrDefault`, `PrintLn`, `println`; unknown intercepts → error | `src/typechecker/mod.rs:544-557` |
| Interpreter on intercepts | `Expr::PluginIntercept` → runtime error (no plugin pass before eval) | `src/interpreter/eval.rs:136` |
| CLI args | **None.** `main` is `define i32 @main()` with no args in all loop-engine paths | `loop_engine/counter.rs` (×5), `ssa.rs` (×4), `mod.rs` (×1) |
| Env vars | brief_rt.c provides `__getenv_brief` / `__getenv_int`; `frgn` wrappers in `lib/std/ffi/env.bv` | `lib/runtime/brief_rt.c:127-157`, `lib/std/ffi/env.bv` |
| FFI dispatch | `frgn` `.c/.cpp/.rs` → Inline (compile+link+LTO); `#System`/`#Link<x>` → Inline direct `-l`; GLUE-mapped ext → Bridge; native `.o/.so/.a` → Inline | `src/analysis/frgn_dispatch.rs:143-219` |
| Print codegen | `PrintInt#` → `call i64 @__print_int` (brief_rt.c, `always_inline` + LTO) | `src/backend/llvm/intrinsics.rs:65-90`, `lib/runtime/brief_rt.c:178` |
| Concurrency gate | Documented, **not enforced** — only auto-selects Sequential/Parallel | `src/backend/llvm/strategy.rs:50-102`, `docs/architecture/concurrency-and-modifiers.md` |
| XOR helpers | `collect_assigned_identifiers` / `collect_read_identifiers` | `src/backend/mod.rs` |
| SAT check | `check_satisfiable(a, b) -> bool` | `src/proof_engine/mod.rs:291` |
| Print format today | No formatting — `PrintLn!(x)` prints a single value | `src/plugin/print_plugin.rs:227-250` |

**Critical consequence:** removing `[#]` is behavior-neutral (nothing consumes
`is_entry`), but the flat-scripting opening node is **genuinely unimplemented**
and `defn main` is **dead code** — Phase 4 must make scripts actually runnable.

---

## 4. Architecture decisions (locked)

### 4.1 Naming convention

| Category | Convention | Examples |
|----------|-----------|----------|
| User-facing `!` macros | lowercase / snake_case | `println!`, `print!`, `get_env!`, `get_env_int!`, `entry!`, `args!` |
| Compiler-known intrinsics | PascalCase + `#` | `Sqrt#`, `PrintInt#`, `Malloc#` |
| Compile-time `$` intrinsics | PascalCase + `$` | `Tag$`, `Insert$`, `StrReplace$` |
| Hashwords | `#PascalCase` | `#Int`, `#String<UTF8>`, `#System` |
| Reflection | `.^` / `.^^` | `x.^Len`, `x.^^Size` |
| Compile-time fn definitions | lowercase + `$` prefix | `$defn`, `$txn`, `$let`, `$const` (unchanged) |

**Enforcement:** any `Expr::PluginIntercept` whose name is not in the known
lowercase set is a compile error at the typechecker. If the name is
PascalCase (`PrintLn`, `GetEnvInt`, ...), the error includes a rename hint
(`PrintLn!` → `println!`). This is the migration path; no transitional alias is
kept.

### 4.2 `println!` / `print!` — Rust-style formatting (Phase 1)

Grammar (format literal):

```
format      ::= ( text | placeholder | escape )*
text        ::= any char except '{' '}'
escape      ::= '{{' | '}}'                    // literal { }
placeholder ::= '{' index? '}'
index       ::= decimal                        // {0}, {1}, ...
```

Expansion (compile-time, `print_plugin.rs`): a `println!("...", a0, a1, ...)` /
`print!("...", a0, a1, ...)` intercept is rewritten to a `Statement::Block` of:

1. For each leading/interspersed literal segment (non-empty): `PrintStr#(seg)`.
2. For each placeholder: the corresponding argument, printed by type:
   `PrintInt#` / `PrintFloat#` / `PrintStr#` / `PrintChar#` (protocol-derived
   dispatch via `TypeUniverse`, rule #18 — the current name-based
   `kind_from_type`/`kind_from_expr` is replaced).
3. `println!` appends `PrintChar#(10)`.

Errors (compile-time):
- Placeholder index `{n}` with no argument `n` → "format argument {n} out of
  range in println!".
- More arguments than placeholders → allowed (Rust-compatible) — unused
  trailing args are a compile warning.
- `{` not followed by `}` or digits → malformed format error.

No runtime formatting machinery — the block IS the output. `println!()` with no
args emits only `PrintChar#(10)`.

### 4.3 `entry!` and `args!` — placement and expansion (Phase 3)

**Placement:** inside contract brackets, as a Bool expression:

```brief
node build [entry!("build")][result == 0] { ... }
txn  serve [entry!("serve")][running == false] { ... }
```

**`entry!("<cmd>")` expansion** (for the decorated node/defn `N`):

1. Inject a top-level one-shot guard (deduped per command; `__` prefix is
   compiler-reserved):
   ```brief
   let __entry_<cmd>_done: Bool = false;
   ```
2. Rewrite the `entry!` expression in the contract to:
   ```brief
   entry_cmd() == "<cmd>" && !__entry_<cmd>_done
   ```
   composed into `N`'s existing precondition with `&&` (precedence: parenthesize).
   **`[true]` is never used.**
3. Append to the end of `N`'s body: `__entry_<cmd>_done = true;` — the node fires
   at most once. **One-shot by default.** A deliberately persistent node declares
   its own explicit contract (its own counter/state guard) alongside `entry!`.
4. If `N` is a `defn` (non-reactive), the plugin also injects a reactive wrapper:
   ```brief
   let __entry_<cmd>_done: Bool = false;
   node __entry_<cmd> [entry_cmd() == "<cmd>" && !__entry_<cmd>_done][__entry_<cmd>_done] {
       <call to N>;
       __entry_<cmd>_done = true;
   };
   ```
   This is the "helper node" path — CLI-addressable defns become subcommands.

**`args!("--flag")` expansion:**

```brief
let arg_flag: Bool = __argv_has("--flag");
```

Inserted as a top-level state field initializer, and the `args!` expression
rewrites to the identifier `arg_flag`. **`args!` reads snapshot state only** — no
guard, no flip (the enclosing node's one-shot guard governs firing).

**`args!("--flag", T)` expansion (typed value):**

```brief
let arg_flag: T = __argv_value_as::<T>("--flag");
```

The type argument `T` is parsed from the second intercept argument (an
`Expr::Identifier` naming the type). The plugin type-checks the conversion
(Int/Float/String/Bool) and rewrites the expression to `arg_flag`.

**Top-level binding naming / collisions:** helper names are
`arg_<sanitized-flag>` where `<sanitized-flag>` = flag with leading `-` stripped
and remaining `-`→`_` (`--out` → `arg_out`). If a user binding already exists,
the plugin errors (no silent shadowing).

**Stdlib (rule #13):** `lib/std/cli.bv` (new) provides the FFI + helper surface:

```brief
frgn __argv_count() -> Int        from "lib/runtime/brief_rt.c" fallback 0;
frgn __argv_get(i: Int) -> String from "lib/runtime/brief_rt.c" fallback "";
frgn __argv_has(flag: String) -> Bool   from "lib/runtime/brief_rt.c" fallback false;
frgn __argv_value(flag: String) -> String from "lib/runtime/brief_rt.c" fallback "";

defn entry_cmd() -> String { term __argv_command(); };
defn arg_present(flag: String) -> Bool { term __argv_has(flag); };
```

The entry plugin ensures `import "std/cli.bv"` exists (like the prelude injects
stdlib imports) before rewriting intercept expressions.

**Command semantics (precise):** `__argv_command()` scans `argv[1..]`, skips
tokens beginning with `-`, and returns the first remaining token; `""` if none.
So `<prog> --verbose build` → `"build"`.

**Env-var fallback:** `entry_cmd()` also honors `$BRIEF_ENTRY_CMD` if set (test /
embedded path without argv); documented in the runtime helper. This is the sole
environment dependency and is additive.

### 4.4 CLI runtime capture (Phase 3)

- Emitted `main` changes from `define i32 @main()` to
  `define i32 @main(i32 %argc, ptr %argv)` in **every** loop-engine main
  emission site (`counter.rs` ×5, `ssa.rs` ×4, `mod.rs` ×1).
- At the top of `main`, store into module globals:
  `@__brief_argc = internal global i32 0`, `@__brief_argv = internal global ptr null`,
  via `store i32 %argc, ptr @__brief_argc` / `store ptr %argv, ptr @__brief_argv`.
- brief_rt.c gains: `__argv_count`, `__argv_get`, `__argv_has`, `__argv_value`,
  `__argv_command` (reading the globals; `extern int64_t __brief_argc; extern void* __brief_argv;`).
- **Scope:** native (LLVM) targets only. Non-native backends (WASM/SPIR-V/Webstack)
  receive a compile-time warning if `entry!`/`args!` are used on a target without
  argv support, and the helpers degrade to their fallbacks (documented behavior,
  not silent).

### 4.5 One-shot script node (Phase 4)

New `src/plugin/script_plugin.rs` (Parsed stage). When a `.bv` has **bare
top-level statements** (`TopLevel::Statement`), `TopLevel::Constant`, or
`TopLevel::Let`) and **zero** explicit `defn`/`txn`/`node`:

```brief
let __script_done: Bool = false;
node __script_main [__script_done == false][__script_done] {
    <script statements, in order>
    __script_done = true;
};
```

- Precondition `[__script_done == false]` is true exactly once; the final flip
  makes it false afterward. **`[true]` is never emitted.**
- The guard is read by the reactor's per-tick precondition check → live, no DCE.
- `defn main` wiring: if a `defn main()` exists (no explicit `entry!`), the
  plugin synthesizes the same one-shot node calling `brief_main()` once, fixing
  the current dead-code gap.
- Naming: `__script_main`, `__script_done` are compiler-reserved; collision with
  a user top-level binding is a compile error (not silent shadowing).

### 4.6 Concurrency gate (Phase 3)

New `src/analysis/concurrency_gate.rs` (frontend-computed per the
frontend-driven-dispatch pillar; invoked from `compile.rs` after typechecking).

For every unordered pair of **reactive** txns `(A, B)`:

1. `sat = check_satisfiable(pre_A, pre_B)` (`src/proof_engine/mod.rs:291`).
2. `xor_overlap` = `(A.writes ∩ (B.reads ∪ B.writes)) ≠ ∅` OR
   `(B.writes ∩ A.reads) ≠ ∅` (via `collect_assigned_identifiers` /
   `collect_read_identifiers`).
3. If `!sat` OR `xor_overlap` → pair is safe without classification (mutually
   exclusive, or sequential-by-dependency). Continue.
4. Else (eligible to fire together): the pair must be classified —
   both `async` (explicit simultaneous firing) or both `sync<group>` (same
   group barrier). Otherwise → **hard compile error**:

   ```
   error: nodes A and B can fire together; declare 'async' on both or
   'sync<group>' on both.
   ```

Generated entry/script nodes are **never** `async` and **never** `sync<group>`.
Consequences:
- Two `entry!` nodes with mutually exclusive commands (`cmd == "a"` vs
  `cmd == "b"`) → `pre_A ∧ pre_B` UNSAT → legal subcommand dispatch.
- An entry/script node overlapping a user node with no XOR dependency →
  gate demands classification; since generated nodes cannot be classified, the
  program is **denied** unless the developer restructures (the intended behavior:
  no implicit concurrency).
- **Existing programs** (examples/benchmarks) with multiple auto-firing nodes are
  audited in this phase and reclassified with explicit `async`/`sync<group>`
  where concurrent firing is intended, or restructured. This is a first-class
  part of the phase (no silent breakage; every change is reviewed).

### 4.7 Additive-only rule (#5)

No existing optimization match arm is modified. New behavior is added as new
match arms / new plugin passes. The `_ => return None;` / `_ => Err(...)`
fallthroughs remain unchanged except where a *diagnostic* is improved (error
messages, never semantics).

---

## 5. Phase plan

### Phase 0 — FFI full audit (research; deliverable = documented findings + regression target)

**Baseline (rule #11):** clean `cargo build --release`, then
`bash benchmarks/build_and_bench.sh --runtime`. Record the full ratio table for
all runtime benchmarks at `d6c6c818` in `benchmarks/results/2026-08-01-plugin-rework-baseline.md`.

**Audit protocol (Performance Recovery Protocol §1-6):**
1. Inspect `benchmarks/*.ll` at baseline for every FFI shape:
   - `frgn` `.c` / `#System` / `#Link<x>` / native `.o/.so/.a`: `call @sym(...)`
     direct + LTO-inline evidence.
   - `PrintInt#`/`PrintStr#`/`PrintFloat#`/`PrintChar#` → `call @__print_*`.
   - `get_env_int` defn → inlined `frgn__getenv_int` → `call @__getenv_int`.
   - Bridge path (`emit_bridge_frgn_call`) — verify it is never selected for
     `.c/.rs` on the LLVM backend (ext dispatch ordering in `frgn_dispatch.rs`).
   - SSO string shims (`extractvalue {i64,i64} ..., 0` + `inttoptr`) and
     `i64↔ptr` coercions (`coerce_to_param_type`) — quantify any added
     instructions in hot loops.
   - `frgn!`/`frgn?!` fire-and-forget codegen (dead-skip vs call).
2. A/B against `../brief-compiler-baseline` (now at `d6c6c818` = current state,
   so A/B isolates only our subsequent changes) via `compare_baseline.sh`.
3. `git log --oneline` over `src/backend/llvm/emit_expr.rs`,
   `emit_toplevel.rs`, `intrinsics.rs`, `src/plugin/{print,env}_plugin.rs` to
   identify the syntax change that introduced any indirection between the
   "native" era and now; cross-check `benchmarks/results/` history.
4. **Deliverable:** `docs/plans/2026-08-01-ffi-audit-findings.md` with the exact
   IR evidence, the identified regression (if any), and a regression target
   benchmark + a guard test asserting plugin/print rewrites do not change the
   emitted FFI call sequence.

### Phase 1 — Lowercase macros, `println!`/`print!` formatting

| File | Change |
|------|--------|
| `src/plugin/print_plugin.rs` | Handle `print`/`println`; format-string parse + positional substitution; protocol-derived type dispatch via `TypeUniverse`; newline for `println!`; compile-time errors for bad placeholders |
| `src/plugin/env_plugin.rs` | Rename `GetEnv`→`get_env`, `GetEnvInt`→`get_env_int` |
| `src/typechecker/mod.rs:544-557` | Recognize `print`/`println`/`get_env`/`get_env_int`; PascalCase intercept → rename-hint error |
| `src/interpreter/eval.rs:136` | Evaluate `print`/`println`/`get_env`/`get_env_int` natively (interpreter-is-reference) |
| `benchmarks/*.bv` (~40) | `PrintLn!`→`println!`, `GetEnvInt!`→`get_env_int!` |
| `lib/std/{io,env,ffi/io,ffi/env}.bv` | Update comments/defn doc strings |
| `examples/*.bv` | Same rename sweep |
| `learn-brief/*`, `spec/SPEC.md` | Tutorial + spec sweep |
| `syntax-highlighter/` | Grammar: lowercase macro tokens |

**Tests:** format expansion (literal/`{}`/`{0}`/`{1}`/`{{}}`/out-of-range/
trailing-args-warning), newline semantics, PascalCase rename-hint error,
interpreter parity (rule #4), FFI call-sequence guard (Phase 0 #4).

### Phase 2 — Remove `[#]`

| File | Change |
|------|--------|
| `src/parser/definitions.rs:880-908` | Delete the `[#]` branch in `parse_contract` |
| `src/ast/top.rs` | Remove `Contract.is_entry`; delete `:137` comment |
| `src/ast/mod.rs:8` | Remove comment line |
| `src/ast/display.rs:476` | Remove `[#]` display branch |
| `src/beast/serialize.rs:52,81-83,365` | Remove `is_entry` serialize/round-trip + test |
| ~50 constructor sites | `git grep -n "is_entry"` sweep → remove the field from every initializer |
| `src/main.rs:731` | Init template `defn main() -> Int [#]` → script form (Phase 4 output), or `defn main() -> Int { term 0; }` |

**Tests:** `[#]` is a syntax error; a script compiles to a one-shot node; BEAST
round-trip still passes without `is_entry`.

### Phase 3 — CLI runtime, `entry!`/`args!`, concurrency gate

| File | Change |
|------|--------|
| `src/backend/llvm/loop_engine/{mod,counter,ssa}.rs` | `main(i32 %argc, ptr %argv)` + global capture stores (all 10 sites) |
| `lib/runtime/brief_rt.c` | `__argv_count/__argv_get/__argv_has/__argv_value/__argv_command` |
| `lib/std/cli.bv` (new) | frgn declarations + `entry_cmd`/`arg_present` defns (§4.3) |
| `src/plugin/entry_plugin.rs` (new) | `entry!`/`args!` expansion, guard injection, `std/cli.bv` import, collision checks |
| `src/plugin/mod.rs`, `src/compile.rs:861` | Register `EntryPlugin` |
| `config/targets.toml [".bv"]` | `plugins = ["prelude","env","print","entry"]` |
| `src/analysis/concurrency_gate.rs` (new) | Gate algorithm (§4.6) |
| `src/analysis/mod.rs`, `src/compile.rs` | Invoke gate after typechecking |
| examples/benchmarks with multi-auto-fire | Audit + explicit `async`/`sync<group>` classification |

**Tests:** expansion to precondition + guard + flip; one-shot (no re-fire);
`args!` Bool + typed value; collision errors; target-without-argv warning;
gate deny/allow matrix (UNSAT, XOR-overlap, async, sync<group>, unclassified);
`entry!`-vs-`entry!` subcommand dispatch.

### Phase 4 — Flat-scripting plugin (one-shot opening node)

| File | Change |
|------|--------|
| `src/plugin/script_plugin.rs` (new) | §4.5 synthesis; `defn main` wiring to `brief_main` |
| `src/plugin/mod.rs`, `src/compile.rs`, `config/targets.toml` | Register `ScriptPlugin` (priority after entry) |
| `src/parser/definitions.rs:1210` | Remove the placeholder `wrap_implicit_entry` (replaced by the plugin) |

**Tests:** bare-statement script compiles to one-shot node; runs exactly once;
`[true]` never present in generated preconditions (assert on emitted IR);
`defn main` runs once; collision with `__script_done` errors.

### Phase 5 — Docs, SPEC, highlighter, full-suite verification

| File | Change |
|------|--------|
| `spec/SPEC.md` | §3.24 → `entry!`/`args!` (one-shot semantics); §3.28 scripting (one-shot node); macro-naming table |
| `docs/architecture/macro-system.md` | `!` macro naming convention, `entry!`/`args!` reference |
| `docs/architecture/concurrency-and-modifiers.md` | Gate is now enforced; update "Implementation notes" |
| `docs/architecture/agent-reference.md` | Naming convention + plugin surface |
| `docs/architecture/hash-words.md` | If it references `[#]` |
| `syntax-highlighter/` | Grammar for `entry!`/`args!`/`print!`/`println!`/`get_env!`/`get_env_int!` |
| `learn-brief/` | Tutorials: scripting, CLI, printing |
| `benchmarks/results/2026-08-01-plugin-rework-final.md` | Post-change full runtime table (rule #11) |

**Final verification:** `cargo test --lib` green; `cargo build` no new warnings;
Praetor on changed files (complexity ≤ 15, lines ≤ 100, params ≤ 6);
`compare_baseline.sh` on the FFI regression target; no benchmark regressed
without a documented A/B result.

---

## 6. Commit order (continuous commits, rule "Continuous commits")

1. `docs/plans/2026-08-01-plugin-macro-rework.md` (this file) — on `main`.
2. Phase 0: baseline results + audit findings.
3. Phase 1: print/env lowercase + formatting (parser-less; tests green).
4. Phase 2: `[#]`/`is_entry` removal (mechanical; tests green).
5. Phase 3: CLI runtime → stdlib → entry plugin → gate (each step green).
6. Phase 4: script plugin.
7. Phase 5: docs/highlighter/learn-brief + final benchmark run.

Each commit: `git add` only intended files; `cargo test --lib` before commit;
`cargo build` no new warnings; Praetor on changed files.

## 7. Undo / rollback

- Every phase is a self-contained commit on `feat/plugin-macro-rework` (new
  branch in the worktree). Rollback = `git revert <commit>` (never
  `git checkout --` / `git restore` — rule #7).
- `is_entry` removal is preceded by a commit that archives the field's semantics
  in this plan; nothing consumes it, so reversion is trivial.
- The baseline worktree at `d6c6c818` remains the controlled A/B reference; it
  is not modified further during execution.

## 8. Risks

| Risk | Mitigation |
|------|-----------|
| `main` signature change (10 sites) breaks a backend path | Phase 3 is isolated; additive global capture; full `cargo test --lib` + benchmark suite per step |
| Concurrency gate denies existing valid programs | Explicit audit + reclassification step with per-change review (Phase 3) |
| `println!` formatting changes emitted IR → FFI benchmark deltas | Phase 0 guard test pins the FFI call sequence; compare_baseline.sh on target |
| Interpreter drift (rule #4) | Interpreter gains native handling in the same phase as the plugin |
| Stale `benchmarks/*.ll` in repo confuse verification | Rebuild from clean source; never trust committed `.ll` |
