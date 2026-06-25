# Brief Compiler - Agent Guidelines

See CLAUDE.md for complete documentation. This file is the condensed active
guidelines (~330 lines). Historical context is in `AGENTS_HISTORY.md` and
the full snapshot backup at `AGENTS_HISTORY_2.md`.

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

## Testing Mandate

**Every new feature, every code path, every match arm must have corresponding
tests.** No exceptions.

- **Interpreter changes**: Add direct AST-construction tests in `src/interpreter.rs`
- **Parser changes**: Add source-text parsing tests in `src/parser.rs`
- **Backend changes**: Ensure existing tests pass (`cargo test --lib`)
- **Legacy code**: Changing old code paths in backends does not require new tests
  for each backend — but the compiler must build and all tests must pass

Run `cargo test --lib` before every commit. **If a change has no test, it does not exist.**

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
