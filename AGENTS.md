# Brief Compiler - Agent Guidelines

This file is the condensed active guidelines (~330 lines). Historical context is in `AGENTS_HISTORY.md` and
the full snapshot backup at `AGENTS_HISTORY_2.md`.

## IMPORTANT CONSIDERATION

This is NOT some "go fast and break things" type SaaS. We are building a compiler. Whenever you think "This would be too much effort" or "This is too large a refactor, we should defer/drop this", DON'T. Patches are UNACCEPTABLE. We are going for code correctness for ALL programs written in Brief, not just the test case we happen to be working on. This is also why we MUST comment on EVERY code change we make. This makes it visible WHY the code is there, and prevents critical code from being removed.

## Philosophy

Brief's contract system (`[pre][post]`) is not a correctness tax — it is
information the compiler uses to optimize harder. Safety IS the
optimization enabler. Full machine access is available through contracts
proven at compile time, not `unsafe` blocks.

## Golden Rules

1. **CONTRACT-FIRST**: Contracts are the source of truth. Never weaken
   `[product > 0]` to `[true]` — fix the code, not the contract.

2. **NO MAGIC**: Never hardcode Rust string matches as built-in functions.
   `is_digit` → `import char from "std/char.bv"`. `None` → `import option from "std/option.bv"`.

3. **INTRINSICS BEFORE FRGN**: Before writing `frgn`, check if an `Intrinsic`
   variant exists. Print? `print_int#`. Input? `get_env_int#`. GPU? `get_global_id#`.
   If no suitable intrinsic exists, add one to `src/ast.rs` — never add `frgn`.

4. **INTERPRETER IS REFERENCE**: If the interpreter runs it correctly, the
   backend must compile it. Fix codegen, never the interpreter.

5. **ADDITIVE ONLY**: Never modify existing optimization paths. New match arms
   only. The `_ => return None;` fallthrough must remain unchanged.

6. **ALWAYS FINISH**: No `todo!()`, `unreachable!()`, `// TODO:`, or stubs in
   committed code. Every feature must be wired parser → AST → analysis → codegen → tests.

7. **NEVER DISCARD STAGED WORK**: The git index holds critical work-in-progress.
   Before `git checkout --` or `git restore`, inspect everything that will be
   destroyed. Stash instead of discard. Never `git stash drop` — resolve conflicts.

8. **TESTS OR IT DOESN'T EXIST**: Every new feature, every code path, every
   match arm must have tests. `cargo test --lib` before every commit.

9. **NO PROTOTYPING — BUILD CLEAN**: Every optimization is a first-class pass
   in its proper module. Never inline new analysis into codegen as a shortcut.

10. **EXECUTIVE REQUESTS ARE NOT OPTIONAL**: When told to fix a pattern, do
    the work. All of it. If unsure, ask — do not decide. If prereqs are missing,
    implement them first.

11. **PLAN WITH BENCHMARKS**: Every performance optimization plan MUST include
    a baseline table of ALL benchmark results (ratios, Brief times, C times,
    correctness status) at the current commit BEFORE any changes. After
    implementation, the plan MUST be updated with the new results for
    comparison. This prevents "optimizations" that fix one benchmark while
    silently regressing others. The baseline must be from a clean `cargo build
    --release` + `bash benchmarks/build_and_bench.sh --runtime` run.

12. **DOCUMENTATION MAINTENANCE IN PLANS**: Every optimization plan MUST
    include a "Documentation" section that specifies:
    - Which `///` doc comments need updating (function signatures, new params)
    - Which rationale comments (`// 2026-07-DD: ...`) need adding at each
      modified code site, explaining WHY the change exists and what pattern
      it targets
    - Which architecture docs (`docs/architecture/`) need updating if the
      optimization changes a dispatch decision or codegen strategy
    - How to preserve existing commentary when refactoring (never delete
      rationale comments — rewrite them to explain the new structure instead)
    Rationale comments are institutional memory. A plan without a documentation
    strategy will produce unmaintainable code.

## Coding Standards

### 1. Flat Control Flow — Max 2 Levels Deep

Never write arrowhead code. Indentation depth must not exceed 2 levels.

**Instead of:**
```rust
fn process(x: Option<Value>) -> Option<i64> {
    if let Some(val) = x {
        if let Some(result) = val.as_i64() {
            if result > 0 {
                return Some(result);
            }
        }
    }
    None
}
```

**Write:**
```rust
fn process(x: Option<Value>) -> Option<i64> {
    let val = x?;
    let result = val.as_i64()?;
    if result <= 0 {
        return None;
    }
    Some(result)
}
```

Use `?`, `if let`, `map`, `and_then`, and guard clauses to flatten code:
- `let val = opt else { return ... };`
- `if !eligible { return; }`
- `let Some(inner) = x else { return None; }`

If a function requires deeper nesting, extract the inner logic into a named helper function.

### 2. Doc Comments on Every Definition

Every `fn`, `struct`, `enum`, `trait`, `type`, `const`, and `mod` must have a `///` doc comment explaining intent, invariants, and usage.

- **Functions**: what it does, each parameter, return value, any panics or errors
- **Types**: what data they represent, valid invariants, field meanings
- **Traits**: what capability they abstract, expected implementer contract, required methods
- **Modules**: what the module provides, key types, relationship to other modules

Doc comments are read by every engineer touching the code. Write them as if the reader knows Rust but not the domain. This is non-negotiable — code with missing doc comments must be rejected in review.

### 3. Input Validation and Defensive Checks

Every function must validate its inputs before use:
- Check array/vector bounds before indexing
- Assert struct invariants hold after construction or mutation
- Print diagnostic context (function name, relevant values, expected vs actual) when validation fails
- Check for NaN/Inf in floating-point parameters at FFI boundaries

Use `debug_assert!` on hot paths, `assert!` for safety-critical invariants. Validation failures must produce messages that identify the function, file, and relevant state so bugs can be diagnosed from logs alone.

### 4. Early Returns Over else-if

Beyond a simple `if/else`, use guard clauses and early returns. `else if` chains deeper than one level are forbidden.

```rust
// Forbidden:
if a { A }
else if b { B }
else if c { C }
else { D }

// Write:
if a { return A; }
if b { return B; }
if c { return C; }
D
```

### 5. Continuous Git Commits

- Commit after each logical step, not at end of day
- `git add` only intended files — inspect `git status` and `git diff` first
- Write concise commit messages that state what and why (reference plan file if applicable)
- Never amend commits — create new ones
- The repo must always be in a state you can roll back to a working checkpoint
- **The user explicitly requires auto-committing between checkpoints.** Do not ask
  "shall I commit?" — just commit when a logical step is complete and tests pass.

### 6. Need-to-Know Dependency Injection

Functions should receive only the data they need, not large context structs. When a function needs specific fields from a large state object, pass those fields explicitly.

```rust
// Avoid:
fn emit_binop(ctx: &CompilerContext, state: &State) -> Result<()>;

// Prefer:
fn emit_binop(builder: &mut LlvmBuilder, op: BinOp, lhs: Type, rhs: Type) -> Result<String>;
```

This makes dependencies explicit, improves testability, and documents which data each function actually uses.

### 7. HashMap Iteration Determinism

Every HashMap iteration that produces LLVM IR instructions MUST be sorted by
key before the loop. Rust's `HashMap` uses SipHash with a random seed per
process — iteration order differs every compilation.

**Wrong** (non-deterministic IR — up to ~9% performance variation):
```rust
for (name, reg) in &self.fun.phi_field_regs {
    writeln!(out, "  {} = phi {} ...", reg, ty).ok();
}
```

**Right** (deterministic IR — same machine code every compilation):
```rust
let mut sorted: Vec<(String, String)> = self.fun.phi_field_regs.iter()
    .map(|(k, v)| (k.clone(), v.clone())).collect();
sorted.sort_by_key(|(k, _)| k.clone());
for (name, reg) in &sorted {
    writeln!(out, "  {} = phi {} ...", reg, ty).ok();
}
```

This applies to ALL HashMaps whose iteration order determines IR instruction
order: `field_index_map`, `phi_field_regs`, `backedge_field_regs`, `last_val_temps`,
`done_needs_fields`, `pending_phi_backedge`, `pending_phi_native_backedge`,
`vector_phi_groups`, `vector_phi_current`, etc. HashMaps used solely for O(1)
lookups (never iterated for emission) are fine.

Reference: commit `139c345`, `docs/plans/2026-07-06-ir-determinism-and-benchmark-strategy.md`,
and the warning comment at `src/backend/llvm/context.rs:223`.

## Commands

- **Build**: `cargo build`
- **Test**: `cargo test --lib`
- **Test backend registry**: `cargo test --lib -- backend::tests`
- **Compile RBV**: `./target/release/brief-compiler rbv <file.rbv>`
- **Benchmark**: `bash benchmarks/build_and_bench.sh` — always use this harness.
  Ad-hoc timing produces false hangs and imprecise numbers.

## Anti-Patterns (NEVER DO)

- Changing `[product > 0]` to `[true]` because code doesn't set product
- Using generic contracts like `[true]` that pass everything
- Adding postconditions that don't guarantee specific outcomes
- Adding Rust string-match built-ins when the standard library or import system should be used
- Pre-populating interpreter state with enum constants (None, Some, Ok, Err)
- Adding `x == x` self-references in preconditions to force liveness
- Adding synthetic exit-condition fields solely to prevent dead-field elimination
- **Hardcoded `from` strings**: `from "libruntime"` — use `from "c"` or omit
- **Hardcoded runtime declares**: `__rt_init` etc. must be `frgn` in `std/rt.bv`
- **Name-based interpreter dispatch**: Dispatch on `Value::HashMap`, not `fn_name == "insert"`
- **`"None"`/`"Err"` discriminant magic**: Use enum declaration order, not variant names
- **Runtime type tags for dispatch**: Type is determined statically by `TypedRegister.ty`, never at runtime
- **Implicit coercions**: All type reinterpretations must be explicit via `as` casts
- **Dynamic optimization path switching**: Choose layouts at compile time based on liveness evidence
- **Transitive compatibility inference**: Each compatibility must be explicitly declared
- **Weakening existing optimization paths**: Additional match arms only, never modify existing arms
- **Blaming regressions on "system noise" or "HashMap iteration order" without a controlled A/B experiment**:
  Every suspected regression must be investigated by running a controlled experiment
  (old compiler vs new compiler on the full benchmark suite, same machine, same load).
  Document the results in a plan or fix document before any corrective action.
  "System noise" is not an excuse — if benchmarks are noisy, increase iterations or
  switch to statistical comparison. Always refer to existing documentation
  (`docs/plans/`, `docs/architecture/`, BUGS.md) when a regression is suspected.
- **Old-style Expr match without BinaryOp normalization**: The parser creates `Expr::BinaryOp`/`Expr::UnaryOp`
  (new-style packed variants) for all operations. Any function matching `Expr::Add`, `Expr::Mul`, etc.
  (old-style variants) must first normalize via `expr.normalize_to_old()`. Missing this produces
  silent wrong output — e.g., `try_eval_cfloat` returned `None` for `4.0 * pi * pi`, causing all
  nbody mass constants to be `constant float 0.0` in the IR. **Always normalize before matching.**

## For OpenCode

1. Read CLAUDE.md and this file for full context
2. Follow Contract-First Philosophy — never weaken contracts
3. Test with `cargo test --lib` before committing
4. Document bugs and root causes in BUGS.md
5. Never add Rust built-ins for things the standard library provides
6. **No prototyping**: Every optimization is a first-class pass in its proper module
7. **Never weaken C benchmarks**: Fix Brief to match or beat C, never hobble C with `volatile`
8. **Interpreter IS the reference**: Add to interpreter first, then codegen
9. **Benchmarks on our own terms**: End-to-end results. Features must add language value
10. Write `docs/plans/YYYY-MM-DD-<topic>.md` before starting plan-driven work
11. Update `docs/architecture/` in the same commit as structural changes
12. Add Kani proof harnesses for all new safety-critical code
13. Run Praetor on new/changed files: complexity ≤ 15, lines ≤ 100, params ≤ 6

## Per-Commit Checklist

Before every commit:
1. `cargo test --lib` — all tests pass
2. `cargo build` — no warnings
3. Run Praetor on new/changed files
4. Update architecture docs if API contracts changed
5. Log bugs/gotchas in BUGS.md or `docs/architecture/praetor-log.md`
6. Add Kani harnesses for all newly written or modified functions

### Regression Guard Checklist (every refactoring)

Before every refactoring change:
7. **Inspect every match arm** in the function being refactored.
   Silent regressions come from removed arms, not added ones.
8. **Verify optimized IR** — not just that tests pass. Run the relevant
   benchmarks and compare against the pre-refactoring numbers.
9. **Update architecture comments** to reflect the new structure. Delete
   no rationale comments; rewrite them to explain the current design.

### LLVM Diagnostic Commands (when optimizer fails)

```bash
# SROA failures (struct not decomposed into scalars)
opt -O3 -pass-remarks-missed=sroa unopt.ll -disable-output 2>&1
# Loop vectorization failures
opt -O3 -pass-remarks-missed=loop-vectorize unopt.ll -disable-output 2>&1
# Alias analysis / GVN failures
opt -O3 -pass-remarks-missed=gvn unopt.ll -disable-output 2>&1
# All optimization remarks at once
opt -O3 -pass-remarks-missed=sroa,gvn,licm,loop-vectorize unopt.ll -disable-output 2>&1
# Inspect IR before/after
opt -S -O3 unopt.ll -o opt.ll
diff <(grep -v '^;' unopt.ll | grep -v '^$') <(grep -v '^;' opt.ll | grep -v '^$')
# Check if %State struct survived SROA
grep '%State' opt.ll
```

## Observability as Liveness

A program that produces no observable effect IS dead code. The compiler is
correct to eliminate it. **A value is live if an FFI call consumes it.**

If the compiler folded your hot loop to `store i64 N`, **the compiler is
right.** Your program produced no observable output. The fix is NOT liveness
hacks (`x == x`, synthetic exit fields). The fix IS `frgn __print_int(result)`.

### `term! -> swan_song` is the correct liveness pattern for terminal programs

```brief
term! -> __print_int(result);   // swan song runs before ret — structurally live
```

**Do NOT:**
- Use `io_pending` or other opaque triggers purely to prevent fold elimination
- Add `#!exit` pragmas when `term!` already terminates the program
- Add synthetic exit-condition fields or `x == x` self-references
- Complain that `main` is just `ret` — the compiler is RIGHT. Fix your program.

**The correct pattern:**
```brief
frgn __get_env_int(name: Ptr<Byte>) -> Int ;
frgn __print_int(n: Int) -> Bool ;

let N: Int = __get_env_int("BOUND");   // runtime-determined

rct txn compute [done < N][done == N] {
    [done == N - 1] {
        term! -> __print_int(result);
    };
    &done = done + 1;
    term;
};
```

### Precomputation is Correct, Not a Bug

If the compiler folds your entire hot loop — it had all information at compile
time. Do NOT fight it with hacks. Make the bound runtime-determined:
```
let N: Int = __get_env_int("BOUND");  // ✓ not precomputable
const N: Int = 50000000;              // ✗ precomputable
```

The `--optimize-budget` flag (default 256) controls simulation depth. Increase
it or use runtime bounds — never weaken contracts or add hacks.

## Benchmark Philosophy (Condensed)

### Semantic goals, not syntax

Brief benchmarks answer: **"Can Brief compute X with competitive performance
vs C?"** — not "Does Brief have feature Y?" Implement the semantic goal using
Brief's idioms, not a line-by-line port.

### Benchmarks exist to find flaws

A benchmark that fails tells you something is missing. A benchmark that is
"too good to be true" (0.001s for real work) tells you the compiler folded
dead code. Both are diagnostic signals.

### When a benchmark can't be implemented as-is: find the isomorphism

| C pattern | Brief-idiomatic equivalent |
|-----------|---------------------------|
| `malloc` + pointer navigation | Contract-proven struct arrays + index traversal |
| `double u[N]` (runtime-sized) | Contract-proven compile-time bound + `<-` push |
| `HashMap<String, Int>` | Integer-encoded keys + flat field lookup |
| `for (i=0; i<N; i++)` loop | Convergent contract `[count < N][count == N]` |
| `while (true)` + `break` | Reactive transaction with natural death |
| Recursive `enum Tree` | Flat struct pool with index navigation |

### Symmetric by default

Every Brief benchmark must compute the **same output** as its C reference for
the same input. If approaches differ fundamentally, create two benchmarks:

| Variant | Intent |
|---------|--------|
| **Symmetric** (`_sym`) | Mirrors C step-for-step. Answers: "Does Brief's throughput match C?" |
| **Idiomatic** (`_idio`) | Uses Brief-native patterns. Answers: "Can Brief's optimizer find a better path?" |

Both get `-O3 -ffast-math` from the same clang. No `volatile`, no unused
variables. Any asymmetry is a signal of a missing Brief optimization — fix
the compiler, not the C code.

### Two benchmark categories

| Category | Tag | What it measures | Criteria |
|----------|-----|------------------|----------|
| **Runtime** | `--runtime` | Throughput of compiled code | FFI call in hot loop body |
| **Optimizer** | `--optimizer` | Compile-time folding power | All const inputs, no FFI in hot loop |

A benchmark cannot be both. The harness detects precomputed binaries by
`.text` size ratio (< 25% of C → skip timing). Correctness is checked for all.

`bash benchmarks/build_and_bench.sh --runtime` | `--optimizer` | `--correctness`

### Useful utilities become stdlib functions

When a benchmark produces a general-purpose helper (rolling hash, vector math,
frequency counting), extract it into `lib/std/`.

## Iteration Pattern

**Iteration requires `txn` with `[pre][post]` convergence, NOT `defn` + `[guard]`:**

`Statement::Guarded` is a **one-shot conditional** — evaluates the guard once,
executes the body zero or one times. A `defn` body is straight-line with no
implicit transaction wrapping.

The correct pattern is a **callable `txn`** (not `rct txn`). A regular `txn`
takes parameters and returns values like `defn`, but its body executes in a
convergence loop: precondition → body → postcondition → repeat if precondition
still holds.

```brief
txn iter_map<T, U>(list: List<T>, f: T -> U, result: List<U>, i: Int)
    [i < list :> Size][i == list :> Size] -> List<U>
{
    &result = result.append(f(list[i]));
    &i = i + 1;
    term result;
};

defn iter_map<T, U>(list: List<T>, f: T -> U) -> List<U> {
    term iter_map_loop(list, f, [], 0);
};
```

| Construct | Semantics | When to use |
|-----------|-----------|-------------|
| `defn` | Pure function, straight-line | Stateless computations, wrappers |
| `txn params [pre][post] -> Ret` | Callable convergent loop | Iteration, accumulation, recursion |
| `rct txn [pre][post]` | Reactive, reactor-driven | State machines, event-driven |
| `[guard] { body }` | One-shot conditional | If/else inside a `txn` body |

## Key Backend Rules

### Three canonical backends (only these matter)

| Backend | Target | Status |
|---------|--------|--------|
| **LLVM** (`src/backend/llvm/`) | Native binary (`.ll` + `llc`) | **Active** |
| **Webstack** (`src/backend/webstack.rs`) | WASM + JS glue | **Active** |
| **CIRCT** (`src/backend/circt.rs`) | Hardware (`.mlir` → Verilog) | **Active** |

### Dead backends — zero fixes

`verilog.rs`, `vhdl.rs`, `c.rs`, `rust.rs`, `cobol.rs`, `x86_64.rs`,
`aarch64.rs`, `wasm.rs`, `tcl_generator.rs`

Do not modify for any reason. If a shared API change mechanically breaks a
dead backend, use `#[allow(unused_variables)]`, `_ => {}`, or `todo!()` with
a comment `// dead backend` — do not implement the feature.

### Never weaken optimizations for new features

Existing optimization paths MUST NOT regress. All additions are additive —
new match arms only, no touching existing fold/precompute/dispatch paths.
The `_ => return None;` fallthrough must remain unchanged.

### Contracts enable optimizations

Preserve contract information in codegen so the optimizer can reason about
it. The more LLVM knows, the more aggressively it can optimize.

### Contract Rules

1. **`defn` needs no contract** — body is linear, translation from inputs
   to outputs is inherently provable. Add contracts only when you need
   the optimization leverage they unlock.

2. **`txn` must have at least one contract side** — either `[pre][post]`,
   `[pre]]`, or `[[post]`. Convergence must be provable.

3. **`inop` should have meaningful contracts** — BILD body is opaque to
   the proof engine; the contract IS the specification.

4. **`[true][true]` is rejected** — parser enforces at least one
   meaningful constraint. Use `[[post]` or `[pre]]` sugar instead.

5. **`[[post]` = `[true][post]`** — postcondition-only.
   **`[pre]]` = `[pre][true]`** — precondition-only.

6. **`[true][term == true \|\| term == false]` is a useless tautology** —
   the type system already guarantees the return type. Write a contract
   that actually constrains behavior.

7. **Single-bracket `[expr]` is ambiguous** — parser rejects it.
   Must be `[[expr]` or `[expr]]`.

### Inop Conventions

8. **Inops follow intrinsic naming**, not private-underscore:
   `sl_insert#` not `_sl_insert`. The `_` prefix convention does not
   exist in Brief.

9. **`(%state)` marker comes AFTER the contract**:
   `inop! foo() -> Int [pre][post] (%state) { BILD }`.

### Common Syntax Traps

10. **`<-` is statement-level** — it breaks the expression parser.
    You cannot write `let x = &list <- val`. Use standalone
    `&list <- val;` or `let x = &list <- ;` (pop only).

11. **`Byte` is defined in `lib/std/types.bv`** — do not assume it
    exists without importing. If the type isn't needed, use `Int`.

### Type System

12. **`type Foo <: List { ... }` creates a TypeDef** — but `Foo<Int>`
    is NOT automatically assignable to `List<Int>` in the type checker.
    Projections like `:> Size` and index `foo[i]` may fail on `Foo<Int>`
    even though the runtime representation is identical.

13. **No implicit `Copy` on enums with `String`** — `InsertStrategy::Custom(String)`
    requires removing `Copy` and adjusting comparison code.

## Commenting Mandate (Backend Updates)

**Never delete rationale comments when refactoring.** When consolidating
repeated code (match arms, type dispatch, etc.) into a shared helper, every
rationale comment from the original sites must be preserved — they are the
project's institutional memory. Rewrite them at the helper's definition or at
each call site. If a comment no longer applies after refactoring, rewrite it
to explain the new structure. Comments explaining why specific types are
handled, what edge cases exist, and what bugs were fixed are precious —
never delete them silently.

**Every backend code change must include a comment explaining why it was made
and what it fixes or enables.** The comment format is:
```
// YYYY-MM-DD: <short description of why this exists>
// <what problem it solves, what pattern it targets>
```
Comments must be placed at the site of the change, not in a commit message.
If the change has trade-offs (faster path A but slower path B), the comment
must document them and explain why the chosen approach is optimal for the
targeted situation.

## Optimization Philosophy

### 1. Long-Term Best Optimization

Always emit the IR that produces the BEST FINAL CODE after LLVM's full
optimization pipeline (SROA + GVN + DSE + LICM + vectorizer + backend),
not the IR that looks cleanest before optimization. If a more complex
emission pattern unlocks a downstream LLVM pass (e.g., struct-SSA enables
SROA where per-field GEPs do not), use the complex pattern.

This means:
- Think about what `opt -O3` + `llc -O2` will produce, not just what
  the initial `.ll` looks like
- Prefer patterns that LLVM's optimizer is designed to recognize and
  simplify (phi + icmp + add for induction variables, extractvalue/
  insertvalue for struct decomposition)
- Avoid patterns that produce "already clean" IR at the cost of
  blocking later optimization (e.g., dead stores that DSE must labor
  through a call barrier)
- When in doubt, check the optimized IR (`opt -O3 -S unopt.ll`) and
  count the remaining instructions — that is the true cost
- If you see a way to make the generated IR produce better final code
  after all LLVM passes, implement it — even if the initial IR looks
  more complex

### 2. Regression Prevention

Every optimization decision must leave a comment documenting:
- What pattern it targets
- What it gains (specific benchmarks, expected improvement)
- What it costs (IR bloat, compile time, edge cases)
- Why the trade-off is optimal for the targeted pattern
- What happens if this optimization is removed (exact regressions)

When refactoring, inspect ALL match arms and code paths that the
refactoring touches. A refactoring that accidentally removes an
optimization (like the A005c body stores eliminated on 2026-07-04)
causes silent regressions that may not be caught by correctness
tests. The architecture comments are the primary defense — they
tell the next engineer WHY each pattern exists.

Before every commit:
1. Check: "Does this change affect any existing optimization path?"
2. If yes, verify the optimization still fires (check IR, benchmark)
3. Update comments to reflect the new structure
4. Run full test suite AND benchmark suite
5. Document any trade-off decisions in the commit message

The cost of a missed optimization is measured in months — a pattern
broken today may not be rediscovered until a benchmark regresses, and
the regression may be blamed on "noise" rather than root-caused.

## Regression Watch & Trade-Off Analysis

**Every optimization must consider its effect on ALL code paths, not just the
one it targets.** Before committing an optimization:

1. **Identify the pattern** the optimization targets (e.g., "reactive txn with
   3-5 state fields and a cheap body").

2. **Identify when it would hurt** — what workloads pay more under the new
   codegen? (e.g., "adds a `br` that forces a new basic block, which LLVM
   must then merge back — ~0.1% overhead on 3-field txns").

3. **Eliminate trade-offs where possible**: If the code can detect at compile
   time which path is better, emit different IR for each situation. The default
   answer is NOT "pick one" — it is "detect and branch in the compiler."
   Only settle for a single strategy when runtime detection is impossible
   (e.g., property of the input data, not the program structure).

4. **Always consult existing documentation** before attributing a regression.
   Check `docs/plans/`, `docs/architecture/`, `BUGS.md`, and `git log` for
   prior analysis. A regression that looks like "random noise" often has a
   documented root cause from a previous investigation. Never blame "system
   noise" or "HashMap iteration order" without first checking what changed
   between the two compiler versions and running a controlled A/B experiment
   on the full benchmark suite.

5. **Benchmark both paths** before and after. Compare against C baseline. If
   the optimization helps benchmark A by 2× but hurts benchmark B by 0.1×,
   it may still be worth it — but the comment must explicitly state the cost.

6. **Add a regression check**: When a heuristic chooses between two codegen
   strategies, store a `bool` field on `LlvmBackend` that records which
   strategy was chosen per transaction. The field must be documented and the
   choice must be logged in `report_lines` so benchmark output shows which
   path was taken for each transaction. This makes regressions diagnosable.

## Testing Mandate

**Every new feature, every code path, every match arm must have corresponding
tests.** No exceptions.

- **Interpreter changes**: Add direct AST-construction tests in `src/interpreter.rs`
- **Parser changes**: Add source-text parsing tests in `src/parser.rs`
- **Backend changes**: Ensure existing tests pass (`cargo test --lib`)
- **Legacy code**: Changing old code paths in backends does not require new tests
  for each backend — but the compiler must build and all tests must pass

Run `cargo test --lib` before every commit. **If a change has no test, it does not exist.**

## Backend Architecture Rules (Post-Refactoring)

### 1. Decoupled, Context-Driven Architecture

The backend state must remain strictly stratified into three distinct lifetimes
to prevent state leakage and the fragile "save/restore" anti-pattern:

1. **CompilerContext (Global):** Read-only during code generation. Contains
   AST-level definitions, FFI signatures, target specs, and layout properties.
2. **FunctionContext (Per-Function):** Instantiated per-function/transaction.
   Tracks local variables, types, and the SSA register counter. Must never
   outlive the function it compiles.
3. **LLVMBuilder (Instruction Builder):** The sole writer of LLVM IR instructions.
   Direct `writeln!` formatting to raw strings is forbidden for standard instructions.

**Rules:**
- **No Global State Pollution:** Never add transient, function-scoped compilation
  variables (temporary register caches, back-edge trackers) to the global backend
  struct.
- **Single-Source Registry:** All registers must be requested via
  `builder.gen_reg()`. Manual string-based register arithmetic
  (`format!("%t{}", counter)`) outside the builder is prohibited.

### 2. Strict Defensive Code Generation & Validation

Textual code generation must not bypass the compiler's semantic type checks.

**Rules:**
- **No Untyped Casts:** Every type coercion (`trunc`, `zext`, `bitcast`,
  `ptrtoint`) must be explicitly handled by a centralized type-conversion helper.
  Never assume sizes or inject raw cast strings inline.
- **Memory Safety & Thread Safety:**
  - When generating temporary files for compiler-driven external tools (like
    `llc`), always generate unique temporary filenames (e.g., using process/thread
    IDs or UUIDs) to prevent parallel build collisions.
  - Verify that any pointer-tagging assumptions (such as masking off the lower
    2 bits of string pointers) are strictly validated against target platform
    alignments.
- **Explicit FFI Type Declarations:** Every foreign function called by the
  compiler must have an explicit LLVM declaration. Mismatches between C-type
  return sizes (like `bool` or `int32_t`) and the LLVM return declaration must
  be explicitly resolved using truncation/extension to prevent ABI register
  corruption.

### 3. Mandatory Trade-Off Documentation

We do not write code without documenting *why* a specific pattern was chosen
over its alternatives.

**Rules:**
- Every significant optimization, structural file separation, or custom logic
  block must begin with a comment block starting with:
  ```rust
  // ── [Feature Name] ──────────────────────────────────────────────────
  //
  // Why [Architectural Choice] over [Alternative]:
  // [Detailed explanation of trade-offs, register pressure, memory, or CPU benefits]
  //
  ```
- This comment must explicitly outline the trade-off (e.g., compile-time budget
  vs. binary size, loop-unrolling factor vs. stack spilling, etc.).

### 4. Dual-Path / Adaptive Optimizations (Dynamic Dispatch)

We do not choose compiler design patterns dogmatically. If a feature can be
implemented in two ways — where each excels under different workloads — **both
must be supported**, and a static decision tree must select the optimal path at
compile-time.

**Rules:**
When implementing or modifying a backend subsystem, evaluate if a hybrid model
is required:

- **Memory Allocations:**
  - *Path A (Stack/Arena):* Short-lived, temporary collections must use scoped
    bump arena allocation.
  - *Path B (Heap):* Escape-analyzed, persistent collections must use safe,
    tracking-enabled heap allocations.
- **Loop Execution:**
  - *Path A (Folded/O(1)):* Bounded loops with pure bodies and constant limits
    must be collapsed into single-instruction compile-time updates.
  - *Path B (Vectorized/Pipeline):* Bounded loops with side-effects or variable
    limits must be compiled using pipeline-friendly SSA register phi nodes.
- **Control-Flow Dispatch:**
  - *Path A (Enum/Switch):* Triggers with value sets within the
    `--optimize-budget` must be lowered to high-performance, switch-dispatched
    case blocks.
  - *Path B (Sequential Reactor):* Complex or unbounded trigger networks must
    fall back to the sequential state-tick evaluation loop.

For every hybrid subsystem, implement a clear, testable cost-model function
(e.g., `optimal_unroll_factor` or `is_fully_precomputable`) to cleanly divide
the execution paths.

## Key References

| Resource | Location |
|----------|----------|
| **Historical context (pre-2026-06-25)** | `AGENTS_HISTORY.md`, `AGENTS_HISTORY_2.md` (full snapshot) |
| **Bug diagnoses** | `BUGS.md` |
| **Architecture docs** | `docs/architecture/overview.md` |
| **Feature docs** | `docs/architecture/features/` |
| **Channel map** | `docs/architecture/channel-map.md` |
| **Optimization decision tree** | `docs/design/optimization-decision-tree.md` |
| **Backend dispatch** | `docs/architecture/features/backend-dispatch.md` |
| **Benchmark strategy** | `docs/architecture/benchmark-strategy.md` |
| **Kani harnesses** | `docs/architecture/kani-harnesses.md` |
| **Plan documents** | `docs/plans/` |
