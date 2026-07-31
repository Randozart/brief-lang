# Brief Compiler — Agent Guidelines

**2026-07-31:** This is the condensed operating manual (~300 lines). The full
pre-rewrite document is preserved in `AGENTS.md.archive`; reference material
(language syntax, contracts, coding standards, backend architecture) lives in
`docs/architecture/agent-reference.md`. Historical context: `AGENTS_HISTORY.md`,
`AGENTS_HISTORY_2.md`.

## Operating Contract

You are building a compiler that must be correct for **all programs** written
in Brief, not just the test case in front of you. Zero tolerance: "probably
fine" is a critical failure. Every edge case, undefined behavior, or bug in a
file you touch is solved completely NOW — never deferred, never "out of scope,"
never "pre-existing."

Every decision passes three questions:

1. **Does this make the compiler more general, or special-case one pattern?**
   A match arm for `"ring_push"` solves today's benchmark; tomorrow's
   `MyQueue<T>` with `InsertAt <~ my_push(#L, #R)` demands the same treatment.
2. **Does this add knowledge the compiler must carry forever, or push it into
   configuration/stdlib where it can evolve?** The dividing line is
   `--no-stdlib`: if it must work without stdlib, it's an intrinsic; everything
   else belongs in config or `.bv` files.
3. **If this were the only rule left, would the architecture still hold?**
   Removing any one rule must not break the others.

Patches are unacceptable. There is no "go fast and break things."

## Golden Rules

1. **CONTRACT-FIRST**: Contracts are the source of truth. Never weaken
   `[product > 0]` to `[true]` — fix the code, not the contract.
2. **NO MAGIC**: Never hardcode Rust string matches as built-in functions.
   `is_digit` → `import char from "std/char.bv"`. Primitive types (Int, Float,
   Bool, Ptr, Void) are the sole bootstrap exceptions.
3. **INTRINSICS BEFORE FRGN**: Check `get_intrinsic_signature()` before writing
   `frgn`. All intrinsic names are PascalCase + `#` suffix (`Sqrt#`).
4. **INTERPRETER IS REFERENCE**: If the interpreter runs it correctly, the
   backend must compile it. Fix codegen, never the interpreter.
5. **ADDITIVE ONLY**: Never modify existing optimization paths — new match arms
   only. The `_ => return None;` fallthrough must remain unchanged.
6. **ALWAYS FINISH**: No `todo!()`, `unreachable!()`, `// TODO:`, or stubs in
   committed code. Every feature wired parser → AST → analysis → codegen → tests.
7. **NEVER DISCARD UNCOMMITTED WORK**: `git checkout --`, `git restore`, and
   `git checkout .` DESTROY work permanently — never use them. Commit your own
   changes with targeted `git add`; never stash others' work. `git reset HEAD`
   is safe (unstaging only).
8. **TESTS OR IT DOESN'T EXIST**: Every feature, code path, and match arm needs
   tests. `cargo test --lib` before every commit.
9. **NO PROTOTYPING**: Every optimization is a first-class pass in its proper
   module — never inline analysis into codegen as a shortcut.
10. **EXECUTIVE REQUESTS ARE NOT OPTIONAL**: Told to fix a pattern? Do all of
    it. If prereqs are missing, implement them first.
11. **PLAN WITH BENCHMARKS**: Every performance plan MUST include a baseline
    table of ALL benchmark results at the current commit BEFORE changes, and the
    new results AFTER. Baseline from a clean `cargo build --release` +
    `bash benchmarks/build_and_bench.sh --runtime`.
11b. **PERSISTENT BASELINE WORKTREE**: `../brief-compiler-baseline` holds the
    baseline commit for controlled A/B regression detection
    (`bash benchmarks/compare_baseline.sh <name>`). Never excuse a regression as
    "noise" without this experiment.
12. **DOCUMENTATION MAINTENANCE IN PLANS**: Every plan must specify which doc
    comments, rationale comments, and architecture docs need updating, and how
    to preserve existing commentary when refactoring.
13. **STDLIB IS THE EXTENSION MECHANISM**: New functionality goes in `.bv`
    files, not new Rust match arms. The compiler teaches; stdlib learns.
14. **NO KNOWLEDGE OF SPECIFIC TYPES**: The compiler must never check for
    `Type::string()` or match `"ring_push"`. Type-specific logic lives in config
    and stdlib. Sole exception: the bootstrap primitives.
15. **FULL PROVENANCE TRACKING**: Every rationale comment carries *when, why,
    what pattern it targets, and how to undo it*. `// TEMP: YYYY-MM-DD:` flags
    temporary solutions with a path to permanence.
16. **DRY**: A pattern appearing 3+ times becomes a centralized helper. Grep ALL
    call sites when changing a helper's behavior.
17. **MIGRATE WHEN TOUCHED**: When you modify a file, migrate its hand-rolled
    instances to the centralized helpers at the same time.
18. **NO TYPE NAME MATCHING**: Never match Brief type names (`t == "Int"`) in
    Rust. Derive LLVM type, protocol category, and ABI width from the
    `TypeUniverse` (via `universe_key()`/`Cast.#` properties) + `CastingGraph`.
    Exceptions: `Type::Ptr(_)`/`Type::Vector`/`Type::Bits(N)` (compiler
    constructs) and `tbaa_node` (operates on LLVM IR type strings). A `git
    grep` for `Type::Custom.*==` in `src/backend/llvm/` and `src/glue/` must
    return zero.
19. **MEASURE BEFORE YOU BUILD**: Before implementing any performance fix, run a
    pre-build A/B experiment on the ACTUAL generated IR (see Performance
    Recovery Protocol). A refuted hypothesis blocks the fix. A regression caused
    by removing a fragile-but-correct optimization is fixed by REBUILDING it on
    the current architecture — never accepted, never re-added as heuristics.

## Performance Recovery Protocol

When a benchmark is at/above parity but a mechanism made it faster before:

1. **Find the fast era.** Read `benchmarks/results/` ratio history for the
   benchmark. Identify the commit/era where it was at or below parity.
2. **Isolate the regression window.** `git log --oneline <fast>..<slow>` over
   the codegen files. Don't assume — verify which commits changed the emission.
3. **Read the removal plan.** The plan that removed the mechanism documents WHY
   and usually the principled alternative. The removal reason decides the
   response: *fragility* ⇒ rebuild on current analysis; *wrongness* ⇒ reject.
   The current plan may describe the intended end-state better than the current
   code implements it.
4. **Derive the principled version** in terms of the CURRENT frontend analysis
   (LoopShape, `node_decompose` segments, CastingGraph) — never re-add the
   removed heuristics verbatim.
5. **Experiment before building.** Transform the actual generated `.ll` when the
   hypothesis is an IR property; use a hand-peeled `.bv` variant only when the
   structure requires it. Link with the harness's exact command. Verify output
   equality at a BOUND that crosses a print boundary before timing. Interleave
   reference/experiment/C timings ×N (`LC_ALL=C /usr/bin/time -f "%e"`) and
   compare averages. Record the full protocol + results in the plan.
6. **The LTO lesson.** `llc -O2` / raw `.ll` inspection does NOT reflect the
   `-O3 -flto` pipeline used by the benchmark harness. Verify every codegen
   claim (loads, hoisting, folding) against the actual linked binary before
   acting on it.

Experiment link command (match the harness exactly):

```bash
clang -O3 -flto -march=native -ffast-math -fdata-sections -ffunction-sections \
    -Wl,--gc-sections "<name>.ll" "lib/runtime/brief_rt.c" -o "<name>"
```

## Architecture Pillars

- **Types are protocol + metadata.** Nothing else: no cached LLVM type, no
  precomputed layout, no name-based lookup.
  `type Int32: #Int { !> bits: 32; };` is the complete definition. Everything
  else is derived from `(protocol, metadata)` by the casting graph at codegen
  time.
- **The casting graph is the single source of truth.** Cast paths
  (`find_path()`), LLVM type resolution (`resolve_llvm_type()`), and protocol
  variant membership all live there. Every codegen site asks
  `self.ctx.casting_graph.resolve_llvm_type(universe, ty, int_bits)` — no
  exceptions, no `rt.properties["llvm_type"]` fallbacks.
- **The normalizer's one job** is registering types in the universe. It does
  NOT resolve LLVM types, inject `Cast.#`, or compute layouts.
- **Frontend-driven dispatch.** The backend CONSUMES decisions; it does not make
  them. Loop shapes, swan-song hoists, density, modulo partitions, inline
  decisions, and unguarded-FFI sets are computed once in the frontend
  (`AnalysisResults`) and read by the backend. Tunables live in
  `config/targets.toml` + `config/ir-lowering.toml`. See
  `docs/plans/2026-07-31-frontend-driven-dispatch.md`.
- **`#Category` hashwords** (`#Int`, `#Float`, `#String`, …) are backend
  directives in op signatures; `#Link<name>` emits `-l<name>`; `#System` is the
  sole bare protocol hashword. See `docs/architecture/hash-words.md`.
- **Intrinsics vs stdlib**: `rm -rf lib/std && briefc --no-stdlib` still
  type-checks `let x: Int = 5` ⇒ intrinsic; else stdlib.

## Observability as Liveness

A program with no observable effect IS dead code — the compiler is right to
eliminate it. **A value is live if an FFI call consumes it.** The fix for a
folded loop is NOT liveness hacks (`x == x`, synthetic exit fields) — it's
`term! -> __print_int(result)` (structurally live swan song) or a
runtime-determined bound (`GetEnvInt#("BOUND")`, never `const N`).
Precomputation is correct, not a bug. `--optimize-budget` (default 256)
controls simulation depth; increase it or use runtime bounds — never weaken
contracts.

## Benchmarks

- **Semantic goals, not syntax**: "Can Brief compute X competitively vs C?" —
  not "Does Brief have feature Y?"
- **Benchmarks exist to find flaws**: a failing benchmark means something is
  missing; a "too good to be true" time means the compiler folded dead code.
- **Symmetric by default**: same output as the C reference. When approaches
  differ fundamentally, create `_sym` (mirrors C step-for-step) and `_idio`
  (idiomatic, Brief-native patterns) variants. Never hobble C with `volatile` —
  fix Brief to match or beat C.
- **Two categories**: `--runtime` (throughput, FFI in hot loop) vs
  `--optimizer` (compile-time folding). The harness detects precomputed
  binaries by `.text` ratio.
- **Useful utilities become stdlib functions.**
- When a C pattern can't be ported directly, find the isomorphism (see
  `docs/architecture/benchmark-strategy.md`).

## Plans & Documentation

1. Write `docs/plans/YYYY-MM-DD-<topic>.md` before starting plan-driven work.
2. Update `docs/architecture/` in the SAME commit as structural changes.
3. Outdated docs are bugs. Update the tutorial, `spec/SPEC.md`, and the syntax
   highlighter when syntax changes.
4. Behavioral tests, not literal tests — a test must pass after refactoring if
   the behavior is preserved. Test the contract, not the implementation.
5. Timestamped records (`docs/plans/`, `benchmarks/results/`, milestones) are
   historical — never retroactively edit them; reference them.

## Working Rules

- **Flat control flow** — max 2 nesting levels. Use `?`, `if let`, guard
  clauses, early returns. Deeper logic goes in named helpers. `else if` chains
  deeper than one level are forbidden.
- **HashMap iteration determinism** — every HashMap iteration producing LLVM IR
  MUST be sorted by key (SipHash seed varies per process, up to ~9% perf
  variation). See `docs/architecture/agent-reference.md` §4.
- **Continuous commits** — commit after each logical step; auto-commit when a
  step is complete and tests pass (do not ask). `git add` only intended files;
  never amend; never use `git checkout --`/`git restore`.
- **Per-commit checklist**: `cargo test --lib` green; `cargo build` no new
  warnings; Praetor on changed files (complexity ≤ 15, lines ≤ 100, params ≤ 6;
  one `--target` per invocation); Kani harnesses for safety-critical code;
  update architecture docs if API contracts changed; log bugs in BUGS.md.
- **Regression guard**: inspect every match arm (silent regressions come from
  removed arms); verify optimized IR, not just tests; update architecture
  comments; never delete rationale comments — rewrite them.
- **System-level changes**: trace the full data flow; verify claims in source
  (file:line), not memory; check `git diff --stat` between eras; map ALL
  benchmarks not just the regressed one; identify every gate on the path and the
  single decision point that matters; state the hypothesis AND its verification
  test, then RUN it.
- **Interpretation of benchmark numbers**: never blame "noise" or "HashMap
  iteration order" without a controlled A/B (old vs new compiler, full suite,
  same machine). Document results before corrective action.

## Commands

- **Build**: `cargo build` · **Test**: `cargo test --lib`
- **Test backend registry**: `cargo test --lib -- backend::tests`
- **Compile RBV**: `./target/release/briefc rbv <file.rbv>`
- **Benchmark**: `bash benchmarks/build_and_bench.sh` — always use this harness.
  Ad-hoc timing produces false hangs and imprecise numbers.
- **Compare against baseline**: `bash benchmarks/compare_baseline.sh <name>`

## Reference Index

| Resource | Location |
|----------|----------|
| **Language syntax, contracts, coding standards, backend rules** | `docs/architecture/agent-reference.md` |
| **Full pre-rewrite guidelines** | `AGENTS.md.archive` |
| **Historical context** | `AGENTS_HISTORY.md`, `AGENTS_HISTORY_2.md` |
| **Bug diagnoses** | `BUGS.md` |
| **Architecture overview** | `docs/architecture/overview.md` |
| **Backend type dispatch** | `docs/architecture/backend-type-dispatch.md` — read first before backend type code |
| **LLVM backend architecture** | `docs/architecture/backend-architecture.md` — read first before LLVM backend changes |
| **Casting protocol** | `docs/architecture/casting-protocol.md` |
| **Hash words** | `docs/architecture/hash-words.md` |
| **Benchmark strategy** | `docs/architecture/benchmark-strategy.md` |
| **Intrinsics vs stdlib** | `docs/architecture/intrinsics-vs-stdlib.md` |
| **Frontend-driven dispatch (active plan)** | `docs/plans/2026-07-31-frontend-driven-dispatch.md` |
| **kalman/float_math parity (active plan)** | `docs/plans/2026-07-31-regain-kalman-float-math-parity.md` |
| **Spec / tutorial** | `spec/SPEC.md`, `learn-brief/` |

## For OpenCode

1. Read CLAUDE.md and this file for full context.
2. Follow Contract-First Philosophy — never weaken contracts.
3. Test with `cargo test --lib` before committing.
4. Document bugs and root causes in BUGS.md.
5. Never add Rust built-ins for things the standard library provides.
6. **No prototyping**: every optimization is a first-class pass in its module.
7. **Never weaken C benchmarks**: fix Brief to match or beat C.
8. **Interpreter IS the reference**: add to interpreter first, then codegen.
9. Write `docs/plans/YYYY-MM-DD-<topic>.md` before plan-driven work.
10. Update `docs/architecture/` in the same commit as structural changes.
11. Add Kani proof harnesses for all new safety-critical code.
12. Run Praetor on new/changed files.
