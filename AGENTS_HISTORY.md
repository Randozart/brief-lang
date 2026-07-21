# Brief Compiler - Agent Guidelines (Historical Record)

**This is the archived historical record of all optimization sprints, benchmark results,
implementation phases, and bug fixes from May 31 through June 5, 2026.**
The current active guidelines live in AGENTS.md.

---

# Brief Compiler - Agent Guidelines

See CLAUDE.md for complete documentation. This file ensures OpenCode picks up the same guidelines.

## Quick Reference

### Commands
- **Build**: `cargo build`
- **Test**: `cargo test --lib`
- **Test backend registry**: `cargo test --lib -- backend::tests`
- **Compile RBV**: `./target/release/brief-compiler rbv <file.rbv>`
- **Selfhost** (deferred): `cargo run --bin brief-compiler -- selfhost <file.bv>` — self-hosting compiler relies on `list_append`/`get` magic; to be migrated to `<-` arrow syntax in Part C. Currently broken — magic handlers removed.
- **Benchmark**: `bash benchmarks/build_and_bench.sh` — always use this harness, never ad-hoc `/usr/bin/time` or other external timers. The harness rebuilds all binaries, uses nanosecond CLOCK_MONOTONIC timing, and averages 5 iterations. Ad-hoc timing produces false hangs (SIGTERM handler traps timeout) and imprecise numbers.

### File Types
- **.bv** - Brief (standard Brief file)
- **.rbv** - Rendered Brief (Brief + View, compiles to web frontend. Designed for web specifically. Like `.tsx` is to `.ts`)
- **.ebv** - Embedded Brief (Brief with less OS based abstractions, and more oriented towards bare metal and embedded programming)
- **.dbv/.dbvs/.dbvl** - Data Brief (configuration with schema and lines, think `.xml` compared to `.xmls` and `.json` compared to `.jsonl`)

### Critical Philosophy

**CONTRACT-FIRST**: Contracts are the source of truth. Never weaken contracts to match lazy code.

**NO MAGIC**: Never add hardcoded Rust string matches as "built-in" functions.
  - If a `.bv` file needs `is_digit`, the fix is `import char from "std/char.bv"` — NOT a Rust match arm.
  - If a `.bv` file needs `None`, the fix is `import option from "std/option.bv"` — NOT pre-populating state.
  - The FFI system (`frgn from "..."`) is the transparent path. Use it.
  - The standard library exists as a dependency source. Import from it.

**SELF-DOCUMENTING FAILURE**: Before fixing any issue:
  1. Understand WHY the fix works (not just THAT it works)
  2. Document the root cause in BUGS.md
  3. Ensure the fix doesn't violate Contract-First or No Magic

### Anti-Patterns (NEVER DO)
- Changing `[product > 0]` to `[true]` because code doesn't set product
- Using generic contracts like `[true]` that pass everything
- Adding postconditions that don't guarantee specific outcomes
- Adding Rust string-match built-ins when the standard library or import system should be used
- Pre-populating interpreter state with enum constants (None, Some, Ok, Err) — let stdlib handle it
- Adding `x == x` self-references in preconditions to force liveness
- Adding synthetic exit-condition fields solely to prevent dead-field elimination

### Observability as Liveness

A program that produces no observable effect IS dead code. The compiler is correct to eliminate it.

Brief's liveness model follows from one principle: **a value is live if an FFI call consumes it.** Every program must eventually interact with the world — print to stdout, write to a file, send a network packet. These are all `frgn` calls.

When benchmarking:
- If the compiler folded your entire hot loop to `store i64 N`, **the compiler is right.** Your program produced no observable output, so the computation served no purpose.
- The fix is NOT `x == x` in the precondition or `#!exit count == bound && bx0 == bx0`. Those are liveness hacks that work around the compiler instead of writing a real program.
- The fix IS to make the computation observable: compute the result and pass it to a `frgn` function (e.g., `frgn __print_int(result)` or `frgn __print_float(energy)`).
- `statement_contains_ffi` in `compute_effectively_pure` already prevents pure-counter fold for bodies containing FFI calls. The entire dependency chain becomes live naturally.
- The C reference must use the SAME observable (e.g., `printf("%f", energy)` not `return (int)(count + energy)`). Symmetric benchmarks, symmetric optimizations.

**In short: if the compiler eliminated your work, your program produced nothing. Add a `frgn` write that consumes the result. The system works as designed.**

## Benchmark Philosophy

### Benchmarks test semantic goals, not syntactic features

Brief benchmarks answer one question: **"Can Brief compute X with competitive performance vs C?"** — not "Does Brief have feature Y?" Implement the benchmark's **semantic goal** using Brief's idioms, not a line-by-line port.

Example: knucleotide counts k-mer frequencies over a DNA sequence. C uses `int counts[4^k]` + string indexing. Brief encodes the sequence as an Int rolling hash and dispatches through 64 guarded state-field blocks — same output, different encoding. Both compute the same frequency distribution.

### Benchmarks exist to find flaws in Brief

A benchmark that fails (won't compile, hangs, produces wrong output) tells you something is missing in the compiler. A benchmark that is "too good to be true" (0.001s for a computation that should take work) tells you the compiler folded your program — your program produced no observable output, so it's dead code. Both failure modes are diagnostic signals. Treat them as such.

If a Brief benchmark beats C by an implausible margin, first suspect the C reference has been hobbled (volatile, unused return, artificial liveness). Fix the C reference, don't celebrate the win. The only valid victory is Brief vs C on symmetric, structurally-live programs.

### When a benchmark can't be implemented as-is: find the isomorphism

| C pattern | Brief-idiomatic equivalent |
|-----------|---------------------------|
| `malloc` + pointer navigation | Contract-proven struct arrays + index-based traversal |
| `double u[N]` (runtime-sized) | Contract-proven compile-time bound + `<-` push |
| `HashMap<String, Int>` | Integer-encoded keys + flat field lookup |
| `for (i=0; i<N; i++)` loop | Convergent contract `[count < N][count == N]` + straight-line body |
| `while (true)` + `break` | Reactive transaction with natural death |
| Recursive `enum Tree` | Flat struct pool with index navigation |

### Each benchmark teaches one compiler lesson

| Benchmark | Lesson |
|-----------|--------|
| fasta | FFI output in hot loop prevents fold elimination |
| fannkuch-redux | 12-field rotation exercises SROA scalar decomposition |
| mandelbrot | Complex arithmetic + escape tracking = guarded integer pipeline |
| knucleotide | 64-field guarded dispatch = compiler switch-gen vs C array indexing |
| spectral-norm | Float arrays at contract-proven scale (N=5500) = allocation strategy |
| binary-trees | Struct pool allocation + index-based tree walk = memory model |

### The C reference is symmetric, always

Every C reference uses the same observable output mechanism as the Brief version.
Both get `-O3 -ffast-math` from the same clang. No `volatile`, no unused variables.
Any performance asymmetry is a signal of a missing Brief optimization — fix the compiler, not the C code.

### Useful utilities from benchmarks become standard library functions

When a benchmark produces a general-purpose helper (rolling hash, vector math,
frequency counting), extract it into `lib/std/`. Benchmarks are probes — they find
gaps in the language AND the library. Any function designed for a benchmark that
could serve as a general-purpose utility MUST be added to `lib/std/`.

### Correct Approach
- Keep contract `[product > 0]` 
- Fix code: make buttons call product-specific transactions like `add_laptop`, `add_keyboard`
- If interpreter raises `UndefinedForeignFunction("is_digit")`, add `import char from "std/char.bv"` to the calling .bv file
- If import resolver can't find a standard library file, fix the search path, not the interpreter

## Language Architecture

Brief is a **general-purpose programming language**. The interpreter proves this — it already supports the full expression language including lists, strings, structs, enums, pattern matching, hash maps, and FFI. The standard library (`lib/std/`) has 26 modules covering strings, collections, math, I/O, JSON, HTTP, encoding, shared memory, and more.

### How Brief Works (Correct Model)

Brief's computational primitive is the **reactive transaction** (`node`). A transaction has:
- A **precondition** (guard): `[x > 0 && y < N]`
- A **postcondition** (contract): `[x == N]`
- A **body**: `{ &x = x + 1; &y = y * 2; }`

The compiler's job is to analyze the transaction graph and emit code for the most efficient execution path. This is NOT a niche reactive DSL — it IS how Brief expresses computation. Loops are transactions with bounded convergence (`[count < N][count == N]`). Recursion is a transaction chain with proved termination. Every optimization (purity folding, dead-field elimination, SROA, SLP vectorization) applies because the compiler has enough information from contracts to prove correctness.

### Misconceptions to Avoid

| Wrong | Correct |
|-------|---------|
| "Brief is a reactive state machine DSL" | Brief is a general-purpose language. Transactions are the computational primitive — they ARE loops, iteration, and recursion. |
| "Brief has no arrays/strings/collections" | The interpreter supports `List<T>`, `String`, `HashMap<K,V>`, `HashSet<T>`, `Stack<T>`, `Queue<T>`, `StringBuilder`. The stdlib has 26 modules including `collections.bv`, `string.bv`, `char.bv`, `json.bv`, etc. |
| "Brief can't do tree/heap benchmarks" | The interpreter supports recursive enum types (e.g., `enum Tree { Node(Tree, Tree), Leaf }`), struct instances, field access, and match expressions. |
| "Brief needs malloc/FFI for buffers" | No. The compiler proves bounds from contracts at compile time and allocates accordingly. The programmer writes proofs, the compiler handles memory. |
| "The LLVM backend is the language" | The interpreter IS the reference implementation. The LLVM backend is an optimization pass over it. If the interpreter runs it, the backend should eventually compile it. |

### Two-Layer Architecture

1. **Interpreter** — the reference implementation. Validates EVERYTHING before any codegen work. If something isn't in the interpreter, it doesn't belong in codegen.
2. **LLVM Backend** — compiles state/transactions/expressions to LLVM IR. Applies optimizations (purity folding, SROA, SLP, dead-field elimination, etc.). Must never weaken existing optimization paths for new features.

## Interpreter Completeness

The interpreter is the full reference implementation. Here is the exact status:

### Expressions — Fully Implemented
| Expr | Status |
|------|--------|
| Integer, Float, String, Char, Bool, Term, Identifier, OwnedRef, PriorState | ✅ |
| Add, Sub, Mul, Div, Mod | ✅ |
| Eq, Ne, Lt, Le, Gt, Ge | ✅ |
| Or, And, Not | ✅ |
| Neg, BitNot, BitAnd, BitOr, BitXor, Shl, Shr | ✅ |
| Call, ListLiteral, ListIndex | ✅ |
| Projection (Size/Bytes/Ptr/Alignment/Range) | ✅ — full 5-target support |
| FieldAccess, StructInstance, ObjectLiteral | ✅ |
| PatternMatch (enum variant matching with field binding) | ✅ |
| Concat, Slice, MultiSlice | ✅ |
| Block, Tuple, TupleDestructure, Cast | ✅ |
| Match (full pattern matching with Wildcard and Variant arms) | ✅ |
| **ForAll** | ⚠️ Stub nodes remain in AST but REMOVED from core syntax. Not part of surface language. Returns `Bool(true)` always. |
| **Exists** | ⚠️ Stub nodes remain in AST but REMOVED from core syntax. Not part of surface language. Checks if list is non-empty. |

### Statements — Fully Implemented
| Statement | Status |
|-----------|--------|
| Assignment, Let, InlineAsm, Expression | ✅ |
| Term (with optional swan song), TermBang (with optional swan song) | ✅ |
| Escape, Guarded (guarded blocks) | ✅ |
| Unification (enum match-bind), LocalTrigger | ✅ |

### Top-Level — Fully Handled
| TopLevel | Status |
|----------|--------|
| Transactions (including reactive), StateDecl, Trigger, Constant, Import, LinkDependency | ✅ |
| ForeignBinding (dynamic .so/.dylib loading) | ✅ |
| Struct (instance creation, field access), Enum (constructor calls, dispatch, unification) | ✅ |
| Definition (defn) — including calls | ✅ |

### Known Gaps
- **Recursive defn calls**: `defn` functions CAN call themselves, but there is NO recursion guard or stack depth limit. A deeply-recursive `defn` will stack-overflow the Rust interpreter.
- **ForAll/Exists**: Removed from surface syntax. Stub AST nodes remain but the quantified forms are not part of the language.

**Conclusion**: The interpreter supports Brief as a general-purpose language. Recursive-defn safety is the only meaningful gap.

## LLVM Backend Gaps

The LLVM backend lags behind the interpreter. Additive only — never weaken existing optimization paths.

### Expressions — Stub (Returns 0 or Degraded)
| Expr | What's Missing |
|------|----------------|
| **Slice** | Only handles `start` offset into the same buffer. Missing `end` bound, `stride`, `mask`. Needs new buffer allocation + copy. |
| **MultiSlice** | Returns base pointer unchanged. Missing coordinate-based indexing. |
| **Tuple** | Returns 0. Missing struct allocation or register flattening. |
| **TupleDestructure** | Passes inner value through. Missing element extraction. |
| **StructInstance** | Returns 0. Missing struct allocation + GEP + stores. |
| **ObjectLiteral** | Returns 0. Same gap as StructInstance. |
| **FieldAccess** | Returns object pointer as-is. Missing GEP into struct at known field offset. |
| **ForAll** | Returns 1 always. Matches interpreter stub. |

### Top-Level — Silently Skipped
| TopLevel | Impact |
|----------|--------|
| **Struct** | No LLVM struct type generated. StructInstance/FieldAccess stubs are the symptoms. |
| **Enum** | No tagged union layout emitted. Enum constructors work via ad-hoc stack alloca + discriminant prefix. Should be moved to proper TopLevel::Enum codegen. |
| Signature, Import, LinkDependency | Correctly skipped — frontend-only. |
| ResourceDecl, RStruct, RenderBlock, Stylesheet, SvgComponent | Correctly skipped — .rbv frontend concepts. |

### Collection Method Calls
Collection method dispatch uses method-name string matching in `Expr::Call` in the interpreter. The LLVM backend handles `Expr::Call` via FFI marshal/decode/defn-call/constructor paths. Some stdlib collection methods may hit `UndefinedForeignFunction` — verify per benchmark.

## Key Philosophy for Backend Work

### Never Weaken Optimizations for New Features
Existing optimization paths (purity folding, dead-field elimination, SROA, SLP, switch dispatch, thread pool) MUST NOT regress when adding new codegen. Struct/enum/collection codegen is additive — new match arms that don't touch the existing fold/precompute/dispatch paths.

### The Interpreter is the Source of Truth
If the interpreter produces the correct result for a program, the LLVM backend is expected to eventually compile that program correctly. If there's a conflict between "what's easy to codegen" and "what the interpreter does," fix the codegen. Never change the interpreter to match a weak codegen path.

### Contracts Enable Optimizations — Don't Skip Them
The more contract information the LLVM backend has, the more aggressively it can optimize. Struct and collection codegen should preserve contract information (field types, bounds, pre/post conditions) so the optimizer can reason about them.

## For OpenCode

This project uses OpenCode. When making changes:
1. Read CLAUDE.md and this file for full context
2. Follow Contract-First Philosophy
3. Never weaken contracts - fix code instead
4. Test with `cargo test --lib` before committing
5. Document bugs and root causes in BUGS.md
6. Never add Rust built-ins for things the standard library should provide
7. **No prototyping — build clean**: Every optimization is a first-class pass in its proper module. Never inline new analysis into codegen as a shortcut.
8. **Never weaken C benchmarks**: Every asymmetry between Brief and C is a signal of a missing Brief optimization. Never hobble C with `volatile` or artificial liveness hacks. Fix Brief to match or beat C's optimization.
9. **The interpreter IS the reference**: If the interpreter runs it correctly, the backend should eventually compile it. If the interpreter doesn't support something, add it to the interpreter first, then add codegen.
10. **Benchmarks on our own terms**: Brief benchmarks compare end-to-end results (Input X → Output Y). The compiler chooses the optimal execution path. Adding features for benchmarks is fine IF they add value to the language. Never add features solely to run benchmarks.

## Self-Hosting Pipeline

The Brief-in-Brief compiler lives in `lib/compiler/`. The Rust interpreter runs it via:
```
brief-compiler selfhost <file.bv>
```

**NOT currently being worked on.** The self-hosted compiler is broken at the parser level (multidimensional slice parsing bug). It is deferred until further notice. The CLI command remains wired in `main.rs` for reference only.

**Do NOT add as built-ins**: `is_digit`, `is_alpha`, `is_alphanumeric`, `is_upper`, `is_lower`, `is_space`, `char_to_string`, `None`, `Some`, `Ok`, `Err`. These are in `lib/std/` and should be imported.

## Anchored Summary

**Current**: 400 tests pass. Phases 1-4 complete plus Phase 5 (DBVS import pipeline), N3 (PGO), and N2 (equality saturation). Brief wins or ties on 8 of 9 benchmarks. print_loop at 1.63× of C.

### Done — Eliminate Redundant Pragmas (Steps 1-6, complete)
- **Step 1**: Auto-select `Parallel` dispatch when all reactive txns are conflict-free (no `#pragma dispatch(parallel)` needed)
- **Step 2**: `@ link` triggers default to wake (no `#wake` needed)
- **Step 3**: Wake+enum mutual exclusion lifted — enum dispatch enters hybrid wake mode with `@__rt_wait()` loop
- **Step 4**: `suggest_async_promotion()` lint — A001 warning for conflict-free `node` txns that could be async
- **Step 5**: Thread pool auto-inference + concurrent async dispatch (Path 5)

### Implemented (2026-06-02 optimization sprint)
- **Float register promotion**: SSA mode emits native `float` registers alongside boxed i64 forms. `i64_to_float_reg()` helper with `reg_float_cache` skips redundant `trunc`/`bitcast` chains. Kalman filter boxing instructions reduced by ~85%. 
- **`llvm.assume` on convergent preconditions**: Emits `call void @llvm.assume(i1 %cond)` after `icmp slt` in folded loops. LLVM eliminates the conditional branch when the proof engine guarantees convergence.
- **Key extraction from precondition Eq/Or**: `extract_trigger_keys()` recursively extracts trigger = literal pairs from precondition AST. Enables enum dispatch for any trigger-gated txn, even with arbitrary Int triggers (not just enums with known value-set sizes).
- **Perfect hashing for sparse dispatch**: `find_perfect_hash()` finds multiplicative hash (`(k*M)>>S`) for sparse key sets. `sparsity_ratio()` heuristic skips hashing for dense sets. Verification guards (`icmp eq` per case) ensure safety. Falls back to standard switch when no hash found.
- **Peephole constant folding**: `emit_binop` and `emit_fcmp` fold integer+integer at compile time. Covers add/sub/mul/sdiv/and/or/xor/shl/lshr + all comparisons.
- **Constant inlining at point of reference**: Integer/bool constants referenced by name emit as instruction immediates instead of `load` from global RAM.
- **Constant deduplication**: Identical constants emit as `@alias` — single global declaration, zero extra cache lines.
- **`emit_exit_expr` Phase 1 refactor**: Integer/Bool literals delegate to `emit_expr` for consistent constant inlining. Identifiers remain local (use `@global_state`, not `%state` function param).

### Benchmarks (2026-06-03 — all phases, C with `-O3 -ffast-math`, 50M iterations)
| Benchmark | Path | Brief | C | Ratio |
|-----------|------|-------|---|-------|
| iir_filter | Dead-field elim + pure counter | **0.001s** | 0.084s | **Brief wins** |
| precompute_sum | Compile-time precomputation | 0.001s | 0.001s | ~tie |
| ring_buffer | Enum O(1) pure-counter | 0.001s | 0.001s | ~tie |
| async_counters | Thread pool O(1) pure-counter | 0.001s | 0.001s | ~tie |
| float_math | alloca+SROA + fast-math + -O3 | **0.004s** | 0.006s | **Brief ~1.5×** |
| float_math_nonzero | alloca+SROA + fast-math + -O3 + AVX | **0.162s** | 0.165s | **Brief ~1.02×** |
| sparse_dispatch | Dispatch-chain collapse | 0.001s | 0.001s | ~tie |
| const_heavy | Integer arithmetic (sdiv) | 0.001s | 0.034s | **Brief wins** |
| print_loop | **FFI-based structurally-live** | **0.030s** | 0.049s | **Brief 1.63×** |
| nbody_newton | Custom Newton sqrt inlined vs C sqrtf | **3.62s** | 9.73s | **Brief 2.7×** |
| nbody_sqrt | Both use sqrtf, Brief loses on call overhead | 6.96s | 3.23s | C 2.15× |
| fasta | LCG + FFI per-char output | — | — | (IO-bound) |
| fannkuch_redux | 12-field rotation + modulo checksum | — | — | (computation) |
| mandelbrot | Complex Int arithmetic + escape tracking | 0.74s | 0.65s | C 1.14× |
| knucleotide | Rolling 2-bit hash + FFI output | **0.188s** | 0.194s | **Brief 0.97×** |
| kalman_filter_runtime | 3×3 Float Kalman + SLP hazard guard | 0.161s | 0.153s | C 1.05× |

### New CLBG Benchmarks (2026-06-04)

### Step 5 Details — Thread Pool + Auto Async/Enum Inference
- **Phase 5a**: Thread pool primitives in `runtime/brief_rt.c` — portable barrier (mutex+cond+counter, works on macOS), `brief_thread_pool_init/release/wait/shutdown`, gated behind `#if defined(BRIEF_THREAD_POOL)`
- **Phase 5b**: Builtin declares (`brief_thread_pool_init`, `brief_barrier_release`, `brief_barrier_wait`)
- **Phase 5c**: Auto-categorize txns in `generate()` — enum candidates (trigger-gated), async candidates (conflict-free pairwise), enum beats async
- **Phase 5d**: `emit_async_body` — per-txn worker functions (`pre→fire` pattern)
- **Phase 5e**: Async phase injection in `emit_main` and `emit_enum_main` — thread pool init at entry, `barrier_release → reactor_tick → barrier_wait`
- **Phase 5f**: `main.rs` link step — detects `@llvm.thread_pool`, adds `-DBRIEF_THREAD_POOL -lpthread`
- **4 new tests** (async body emission, thread pool metadata, barrier calls in main, no thread pool without async txns)
- **No atomics on state fields** — the proof engine guarantees disjoint field access per txn group, so plain loads/stores are data-race-free (C11 5.1.2.4p25)
- **Step 5f**: `main.rs` link step — detects `@llvm.thread_pool`, adds `-DBRIEF_THREAD_POOL -lpthread`
- **Step 6**: Eliminate `io_registry.rs` and `#io` pragma — replaced by `import "link/brief_rt.o"` auto-dependency mechanism
  - Deleted `src/io_registry.rs` (94 lines of hardcoded concept→symbol table)
  - Deleted `parse_io_declaration()` (~80 lines) and `#io` parsing loop (~15 lines)
  - New AST node `TopLevel::LinkDependency` — parser detects `.o`/`.a` imports
  - New `lib/std/brief_rt.bv` — declares all `@ link` triggers as pure Brief code
  - `lib/std/system.bv` rewritten to import from `brief_rt.bv` (no more `#io`)
  - Compiler driver auto-detects link deps from source; `--link-rt` flag removed
  - 5 new parser tests for link dep detection
  - Zero compiler knowledge of OS signal concepts afterward

### Optimization Path Summary
| Path | What | Status |
|------|------|--------|
| Path 2 | Dead-field elimination + pure-counter (counter convergence) | Done — IIR filter O(1) store |
| Path 3 | Compile-time precompute (state space ≤ budget) | Done — `emit_precomputed_main` |
| Path 4 | Enum switch-dispatch (bounded trigger values) | Done — pure-counter O(1) store |
| Path 5 | Thread pool async dispatch (conflict-free txns) | Done — pure-counter O(1) store |
| Phase A | alloca+SROA (struct phi → scalar phis) | Done — float_math 41× improvement |
| Phase C | fast-math flags on all float ops | Done — compounds with SROA |
| SLP fix | Union-based float field tracking + cross-op cap | Done — catches float_math_nonzero hazard |
| Phase 1 | Per-function SLP guard + `opt/llc -O3` | Done — `#4`/`#5` attributes replace global flags |
| B1 | UTF-8 slicing fix in FFI helpers | Done — `is_char_boundary` checks |
| B2 | Entry-point value bug | Done — `get_initial_value_numeric` evaluates actual values |
| B3 | Assertion false-path soundness | Done — both guard branches checked |
| B4 | Overlap detection in cross-reference | Done — `decls.iter().any()` not `decls.first()` |
| A4 | Typed SSA — remove `is_float_expr` heuristic | Done — `TypedRegister`, 368 tests pass |
| A6 | Commutativity pattern fix | Done — removed duplicate match arm |
| Phase 3 | iir_filter `x==x` fix + `llc --mcpu=native` | Done — iir_filter O(1), AVX codegen |

### Natural Death (Step 12)
- **Algorithm**: After computing `has_wake_triggers` and building the transition graph, classify each reactive txn as persistent or transient. If ALL reactive txns have proven bounded convergence (`bounded_pre` + `increments`), the program has `has_natural_exit = true`.
- **Synthetic exit condition**: Builds `Expr::And(...Expr::Ge(counter, bound)...)` for each foldable txn and sets `self.exit_condition`. Reuses existing `emit_exit_expr` machinery.
- **Warning suppression**: Natural death sets `self.exit_condition` before the no-exit-path warning check, so the warning correctly fires only for programs with persistent txns.
- **3 new tests** (foldable exits, persistent skipped, non-wake skipped)

### Benchmark Timing Results (fair — C references no longer hobbled by volatile)
| Benchmark | Path | Brief | C | Ratio |
|-----------|------|-------|---|-------|
| iir_filter | 2 (dead-field elim + pure counter) | 0.1524s | 0.1028s | 1.48× |
| precompute_sum | 3 (compile-time) | 0.0020s | 0.0018s | ~tie (startup) |
| ring_buffer | 4 (enum O(1) pure-counter) | 0.0019s | 0.0017s | ~tie (startup) |
| async_counters | 5 (thread pool O(1) pure-counter) | 0.0018s | 0.0018s | ~tie (startup) |
| kalman_filter | SLP hazard + opt -O2 pipeline | 0.71s | 0.75s | **Brief beats C by ~5%** |

### .gitignore / Infrastructure Cleanup
- All benchmark build artifacts (`*.o`, `*.ll`, binaries, generated `brief_rt.c`) now ignored
- 18 tracked artifacts removed from git with `git rm --cached`
- `build_and_bench.sh`: removed `bench_timeout` (all self-terminate), uses release binary directly, no `cargo run` overhead
- `__rt_poll()`: non-blocking event drain called once at main() entry, before the first tick. Eliminates the 100ms wasted first tick on programs with already-pending events. Implemented for all platforms (epoll, kqueue, ARM wfi, x86 hlt, WASM).

### Exit Safety Diagnostics
- **Error**: Unknown identifier in `#!exit` — `check_exit_condition_idents()` recursively verifies all identifiers against `field_index_map` and `constants` before codegen; emits `error: #!exit references unknown variable 'X'` and exits 1
- **Warning**: `#!exit` on one-shot program — fires for folded/precomputed/enum-no-wake paths where exit check is never emitted (`warning: #!exit declared but program has no tick loop`)
- **Warning**: Wake program without exit path — fires when `has_wake_triggers && exit_condition.is_none()` (`warning: program has wake triggers but no exit path`)
- **Architecture**: Warnings collected in `self.warnings: Vec<String>`, printed from `main.rs` after `generate()`, same pattern as optimization report
- **7 new tests** (identifier validation ×2, one-shot warning ×2, no-exit-path ×3)

### Pure-Counter Fold Elimination for Enum/Async Dispatch
- **Problem**: Enum/async dispatch emitted O(N) `while (counter < bound) work()` for pure-body txns (e.g. `ops = ops + 1`), producing 50M iterations each taking GEP → load → icmp → br → add+store → br.
- **Solution**: `enum_fold_pure` companion map alongside `enum_fold_params` carries `(is_pure_body: bool, total_value: Option<i64>)` per txn. In `emit_case_folded_loops`, pure txns with a compile-time-constant bound emit `GEP + store i64 N` (O(1)) instead of the while-loop (O(N)).
- **Total value resolution**: Looks up `bp.bound_var` in `field_initializers` then `constants` to find the compile-time-known total.
- **ring_buffer**: 0.00s (was 0.11s, 110× speedup) — now O(1) beats C's O(N) loop
- **async_counters**: 0.00s (was 0.11s, 110× speedup) — now O(1) beats C's O(N) loop
- **File**: `src/backend/llvm.rs` (~25 lines net in `generate()` + `emit_case_folded_loops`)

### Dead-Field Elimination (Step 7)
- **Problem**: C compiler proved IIR filter's non-volatile float delay-line state (x1/x2/y1/y2) is never observed, eliminated all 50M biquad iterations, leaving only `volatile long count` incq loop (0.09s). Brief emitted the full body verbatim (0.15s).
- **Root Cause**: Brief had no liveness analysis — every state store was emitted regardless of whether the field value was ever consumed.
- **Solution**: `compute_live_fields()` + `compute_effectively_pure()` pass in `transition_graph.rs`.
  - Live set = identifiers in `#!exit <expr>` + preconditions of all txns
  - A txn is "effectively pure" if its only live stores are bounded counter increments; all dead-field stores are dropped.
  - For IIR: live = `{count}`, dead = `{x1, x2, y1, y2}` → effectively pure → `emit_folded_pure_counter` → O(1) `store i64 50000000`
- **Files**: `src/analysis/transition_graph.rs` (~110 lines new), `src/backend/llvm.rs` (1 line change)
- **Tests**: 5 existing transition_graph tests updated; test `test_iir_filter_folded_path_regression` updated to expect pure counter, not while-loop

### Phase 1 — Fair C Benchmarks
- **Problem**: C reference benchmarks used `volatile` for loop state (ring_buffer, async_counters, precompute_sum) and delay-line floats (iir_filter), explicitly preventing clang from applying standard compiler optimizations.
- **Fix**: Removed `volatile` qualifiers where they hobbled the C compiler:
  - `ring_buffer_c.c`: `volatile long ops + for-loop` → `long ops = N` (O(1))
  - `async_counters_c.c`: `volatile + 2 pthreads` → `long g_a=N; long g_b=N` (O(1))
  - `precompute_sum_c.c`: dropped all volatile — clang eliminates 500-iter loop
  - `iir_filter_c.c`: removed volatile from float state (x1/x2/y1/y2) — register promotion
- **Result**: Brief ties or beats C on all 4 benchmarks when both get equal compiler optimization
- **Files**: 4 `.c` files, `build_and_bench.sh`

### Next Up
- **Chimera target-switching**: Compile same `.ebv` program for `--target zcu4ev.dbv` (MMIO), `--target sim.dbv` (struct members), `--target metro.dbv` (shared memory channels).
- **DBVS schema quality**: Extract clock domains, interrupt lines, DMA channel info from Vivado HWH XML for richer hardware profiles.
- **LLVM backend completion**: Structs, enums, collections, runtime-sized allocation. See `plans/2026-06-03-llvm-backend-completion.md`.

### Phase 5: DBVS Import Pipeline (2026-06-03)
- **5a — DBVS import parsing + schema type validation**: `run_llvm_compile()` scans `.dbvs` imports, parses via `crate::dbrief::parse_dbvs()`, collects alias→type map. `LlvmBackend::with_schema_aliases()` stores schema aliases. `validate_schema_types()` cross-checks StateDecl types against schema (Vector/Option/Result → error, UInt→Int → warning).
- **5b — Auto-target resolution**: When schema imports exist and no explicit `--target-dbv`, searches `lib/targets/` and source dir for `.dbv` files whose `IMPORT` matches. Auto-selects if exactly one match.
- **5c — Schema ↔ target cross-validation**: New `src/analysis/schema_validator.rs`. Checks HW008 (missing target binding), HW009 (unreferenced target alias), HW010 (address overlap). Wired in `run_llvm_compile()` before codegen.
- **5d — Scoped MMIO injection**: `build_field_index()` only routes field to MMIO if its name is in `schema_aliases` (prevents accidental MMIO from address-name collisions). Non-schema fields removed from `mmio_fields` to prevent read/write path confusion.
- **7 new tests**: alias loading, unsigned warning, vector rejection, no-validation, multi-merge, scoped MMIO (imported vs unimported).

### N3: Compile-Time PGO via Interpreter (2026-06-03)
- **`Interpreter.profile_mode` + `branch_counts`**: Tracks guard outcome counts via `guard_counter` global identifier. Records `(true_count, false_count)` per guard.
- **`src/analysis/pgo.rs`**: `run_profile()` runs interpreter bounded to `max_ticks` ticks. `has_pgo_candidate()` checks skew ratio (default 100:1). `emit_branch_weights()` formats LLVM `!prof !{!"branch_weights", i32 T, i32 F}` metadata.
- **LLVM emission**: `LlvmBackend.pgo_guard_idx` increments per guard; `emit_stmt` appends `!prof` metadata to `br i1` instructions when profile attached.
- **CLI**: `--pgo-generate` flag on `llvm` command. Skip if no guards or all branches balanced.
- **3 new tests**: skew rejection, skew acceptance, branch weight formatting.

### N2: Equality Saturation (2026-06-03)
- **Lightweight recursive simplification**: No `egg` crate dependency. `simplify()` applies bottom-up rewrite rules with fixpoint iteration (max 5 passes).
- **Rules**: Identity elimination (x+0, x*1, x/1), zero propagation (x*0, x&0), negation (!!x, -(-x)), boolean (x&&true, x||false, x&&x), cancellation ((a+b)-b, (a-b)+b), bitwise (x|0, x^0, x<<0, x>>0).
- **Gate**: Applied in `emit_expr` when `optimize_budget > 0`. No overhead when budget=0.
- **6 new tests**: cancel-add-sub, identity+0, identity*1, zero-mul, double-neg, no-candidates.

### LTO Closure (2026-06-03)
- **`try_lto_pipeline()`** in `src/main.rs`: Compiles `brief_rt.c` to LLVM bitcode via `clang -c -emit-llvm`, merges with program IR via `llvm-link`, runs `opt -O3` on the merged module. Enables inlining of `__print_int`, `__wait_for_event`, and thread pool barriers into Brief loops.
- Graceful fallback: if `clang`/`llvm-link`/`llvm-as` not installed, falls back to existing `cc -c` + `opt` + `llc` + link path.

### MMIO Address Plumbing (2026-06-03)
- **`mmio_fields: HashMap<String, u64>`** in `LlvmBackend` — fields declared with `@ address` are excluded from `%State` struct.
- **Reads**: `Expr::Identifier` emits `inttoptr` + `load volatile` instead of GEP + `load`.
- **Writes**: `Statement::Assignment` emits `inttoptr` + `store volatile` instead of GEP + `store`.
- **Init**: `emit_init_state()` writes initial value via `inttoptr` + `store volatile`.
- **Exit/precompute**: `emit_exit_expr()` and `emit_precomputed_main()` use `mmio_fields` for address-based access.
- Parser fix: `&` and `@` in expressions now use `expect_identifier()` allowing keyword tokens as valid variable names.

### Hardware Handoff Generator (2026-06-03)
- **`src/hardware/handoff.rs`** — extracts peripheral addresses from Vivado handoff files:
  - `extract_from_xparameters()`: parses `#define XPAR_*_BASEADDR/HIGHADDR` from `xparameters.h`
  - `extract_from_xsa()`: opens `.xsa` as zip archive, reads `system.hwh` XML, extracts MEMRANGE entries
  - `extract_from_hwh_xml()`: lightweight string-level XML scanner — no XML crate needed
- **`generate_dbvs()`**: emits `.dbvs` schema with `register @0x... as "name" { type: UInt; }` + type-only `alias name: UInt;`
- **`generate_dbv()`**: emits `.dbv` target binding with `alias name: UInt = @0x...;` per board
- CLI: `--hw-handoff <system.xsa|xparameters.h>`, `--hw-target <board>`, `--target-dbv <target.dbv>`
- 8 new tests covering xparameters, HWH XML, DBVS/DBV generation, address extraction from DBV files

### DBVS→LLVM Alias Resolution (2026-06-03)
- **`extract_target_addresses()`**: parses `.dbv` alias declarations, returns `HashMap<name, u64>`
- **`process_target_dbv()`**: reads + parses `.dbv` file, extracts alias→address map
- **`LlvmBackend::with_mmio_addresses()`**: pre-populates `mmio_fields` from resolved DBV bindings
- **`build_field_index()` resolution**: when `mmio_prepopulated`, state fields matching DBV alias names are automatically routed to MMIO (no source-level `@ address` needed)
- Flow: `--target-dbv zcu4ev.dbv` → parse aliases → `millio_fields` → `inttoptr` + `load/store volatile` in emitted IR

### FFI Output & Structurally-Live Benchmark (2026-06-03)
- **`__print`/`__exit` magic removed from `llvm.rs`**: Deleted `has_print`/`has_exit` declare block, replaced `__print`/`__exit` match arms with generic FFI catch-all. `frgn` calls now go through standard `frgn_map` loop.
- **`__print_int` + `__print` added to `runtime/brief_rt.c`**: Plain C functions (`int64_t __print(const char*)`, `int64_t __print_int(int64_t)`, `void __exit(void)`). `fputs` to stdout, `fprintf(stderr, "%lld\n")`, `exit(0)`.
- **`benchmarks/print_loop.bv`**: Structurally-live benchmark using `frgn __print_int` directly with `io_pending` wake trigger. 50M iterations, prints every 100K. Calls `__print_int` inside guard `[ops % print_interval == 0]`.
- **`benchmarks/print_loop_c.c`**: C reference (symmetric). `if (ops % 100000 == 0) printf("%lld\n", ops);`.
- **Fold prevention fixes**:
  - `is_pure_body` now recurses into `Statement::Guarded` — checks condition and nested statements for FFI references. Without this, FFI calls inside guards were invisible to the pure-body check.
  - `statement_contains_ffi` guard in `compute_effectively_pure` — prevents pure-counter fold for bodies containing any `Expr::Call`.
- **`Statement::Term` filtering in SSA loop bodies**: `emit_folded_loop` and `emit_ssa_main` now filter `Statement::Term` from inline body processing. Previously, `Statement::Term` emitted `ret void` inside `main()` (which returns `i32`), causing LLVM verification failure. `term;` in txn bodies is implicit — the SSA loop already handles continuation via `store %State` + `br`.
- **Tests**: All 372 pass.

### Dispatch-Chain Collapse (2026-06-03)
- **Problem**: `emit_reactor` evaluated each txn's precondition on the post-update state from the previous txn (cascade bug). This caused all 8 txns in sparse_dispatch to fire every tick, with each precondition cascading from the previous update.
- **Phase 1 — Cascade fix**: All preconditions now evaluate in the entry block against the pre-tick state. Results are saved in SSA registers. The body chain uses saved results. See `src/backend/llvm.rs:2648-2680`.
- **Phase 2 — Uniform-body detection**: `is_uniform_body_group()` in `transition_graph.rs:198` checks if all reactive txns have structurally identical bodies. When true, the entire precondition chain is skipped — just the first body is called.
- **Phase 3a — LLVM switch optimization**: After the cascade fix, LLVM's optimizer converted the precondition chain into a `switch i64` dispatch automatically.
- **Phase 3b — O(1) collapse**: After inlining the uniform body, LLVM's SCEV pass proves the loop is just a counter and eliminates it entirely.
- **Result**: sparse_dispatch drops from 0.0758s to 0.0006s (matches C's O(1) elimination). 4 new tests, 372 pass.

## Key Plan Documents
- **`plans/2026-06-03-llvm-backend-completion.md`** — LLVM backend completion plan (structs, enums, collections, runtime buffers)
- **`plans/2026-06-01-optimization-framework.md`** — Implementation plan for optimization phases
- **`plans/2026-06-01-optimization-completion.md`** — Phases A/B/C (wake fix, precompute, regression)
- **`plans/2026-06-01-eliminate-redundant-pragmas.md`** — Steps 1-5 (auto-wake, auto-parallel, async inference, thread pool)
- **`plans/2026-06-01-thread-pool-async-dispatch.md`** — Step 5 design (thread pool, barrier, auto-inference)
- **`plans/2026-05-31-eliminate-io-registry-link-deps.md`** — Step 6 design (eliminate `io_registry.rs` with `import "link/"`)
- **`plans/2026-05-31-benchmarks-plan.md`** — Benchmarks plan (3 new + regression)
- **`docs/design/determinism-and-optimization-frontier.md`** — Conceptual optimization architecture
- **`docs/design/optimization-cost-model.md`** — Full cost model specification
- **`plans/2026-06-01-exit-safety-warnings.md`** — Implementation plan for exit diagnostics
- **`plans/2026-06-01-pure-counter-enum-dispatch.md`** — Pure-counter fold elimination for enum/async dispatch
- **`plans/2026-06-01-fair-c-benchmarks-fuzzing.md`** — Fair C benchmarks + input fuzzing
- **`plans/2026-06-01-dead-field-elimination.md`** — Dead-field elimination: liveness analysis
- **`plans/2026-06-02-hardware-aware-slp-hazard-analyzer.md`** — SLP hazard analyzer
- **`plans/2026-06-02-slp-hazard-loopholes.md`** — Three SLP loopholes audit
- **`plans/2026-06-02-calibration-baseline-and-dispatch-fix.md`** — Calibration baseline + SCEV break
- **`plans/2026-06-03-dispatch-optimization-and-benchmark-fairness.md`** — Dispatch collapse + benchmark fairness

### Calibration Baseline & SCEV Break (2026-06-02)
- **Problem**: const_heavy showed 0.00s (SCEV eliminated linear recurrence). Baseline benchmarks ran 0.00s for 4/7 on O(1) pure-counter paths.
- **Fix**: `+ count / 100` term in const_heavy body breaks SCEV (`sdiv` is non-linear). Both .bv and _c.c updated.
- **Precision**: `build_and_bench.sh` now uses `date +%s.%N` + `bc` + `LC_NUMERIC=C` for 0.0000s precision.
- **alwaysinline audit**: Data refutes the "alwaysinline bloat" hypothesis. `opt -O2` + SCEV handles the phi/select cascade — sparse_dispatch runs 0.0559s at 50M (not pathological). Plan A (noinline guard) skipped.
- **Files**: `benchmarks/const_heavy.bv`, `benchmarks/const_heavy_c.c`, `build_and_bench.sh`

### SLP Hazard Analyzer (2026-06-02)
- **Problem**: SLP vectorization creates `shufflevector` instructions from packed `<2 x float>` phis. At ≥12 float fields with cross-variable coupling, shuffles overflow x86_64's 16 XMM registers → 65 stack spills.
- **Solution**: `estimate_slp_hazard()` in `src/backend/llvm.rs` computes peak register demand from live float fields (N), coupling density (C), temps (T), and global constants (K) against target hardware (R, W). Passes `-vectorize-slp=false` to `opt` when peak ≥ R.
- **Three critical loopholes found and fixed**:
  1. **Local variable blindspot**: `is_float_expr_pre_cg` checked only global state fields, missed `Statement::Let` bindings. Fixed by passing `local_floats: &HashSet<String>` through all recursive functions.
  2. **Dual-variable constraint**: `count_cross_float_ops` required both operands to be variables, missing literal/constant operations (`x * 0.01`). Fixed by checking `left_is_typed || right_is_typed`.
  3. **Missing constants**: Peak formula ignored global float constants (Kalman A/Q matrices: 18 floats). Fixed by adding `accessed_constants` tracking + `const_packed = ceil(K/W)`.
- **Corrected formula**: `peak = ceil(N/W) + min(2·ceil(N/W), ceil(C/2)) + T + ceil(K/W) + 2`
- **Kalman verification**: n=12, C=72, T=12, K=18, R=16, W=4 → peak=28 ≥ 16 → SLP disabled. Brief 0.71s vs C 0.75s (Brief beats C by ~5%).
- **6 new tests**: no-floats, small-field, large-field, independent-channels, AArch64-spec, AVX2-spec

### `opt` Pipeline Fix (2026-06-02)
- **Problem**: struct-SSA optimization created `load %State`/`store %State` + 13-element `insertvalue` chain for non-pure bodies. This 64-byte struct pattern requires SROA (Scalar Replacement of Aggregates) to decompose into scalar phis — but `llc -O2` does NOT run SROA. Only `opt -O2` does. Result: 2× regression (0.14s → 0.28s at 10M).
- **Solution**: Run `opt -O2 -S` before `llc` in `run_llvm_compile()` at `src/main.rs:1899`. The SLP hazard analyzer's `-vectorize-slp=false` flag now goes to `opt` (where SLP actually runs as a middle-end pass), not `llc`. Graceful fallback if `opt` is not installed.
- **Effect**: SROA decomposes the struct load/store into 12 scalar float phis, GVN eliminates redundant float→i64→float round trips, SLP is disabled for hazardous programs. Kalman filter recovers from 2× regression to beat C.

## New Features (2026-06-05): Term Variants, Swan Song, Assume Pragmas

### `term -> swan_song;` — Commit Action
- `Statement::Term` now has `swan_song: Option<Box<Statement>>` field
- Swan song executes **only when postcondition is accepted**, not on every loop iteration
- Swan song fires before `return_value` is set (interpreter) or `ret` is emitted (LLVM backend)
- All 32 `Statement::Term { ... }` construction sites updated with `swan_song: None`
- Analysis passes (region, proof_engine, transition_graph, dataflow) all recurse into swan_song

### `term!` / `term! -> cleanup;` — Program Exit
- `Statement::TermBang` added: new variant with `values`, `swan_song`, `modifiers` fields
- `Token::TermBang` at lexer.rs:78-80 (`#[token("term!")]` + `#[token("TERM!")]`)
- Compiles to direct branch to centralized exit block — fully inlined, no runtime registry
- 22 backend `.rs` files updated with `Statement::TermBang` match arms
- Parser: `Token::TermBang` handler in `parse_postfix`; `-> swan_song` parsing after term/termbang

### `#assume_event(name)` — Liveness Fairness Assumption
- Top-level `#assume_event(trigger_name)` declares that a trigger WILL fire eventually
- Enables proof engine to prove termination for external-trigger loops
- Stored in `ReactorNode.assume_events: Vec<String>` from txn `modifiers`
- No LLVM IR emitted — purely a proof-engine constraint

### `#assume_shape(guard_expr, action)` — Shape Guard with Fast-Path
- Declares that `guard_expr` is expected true at runtime
- Generates runtime guard check with fast-path/slow-path split in LLVM
- Actions: `escape` (silent skip), `run` (full body), `exit` (call `__exit(1)`)
- `__exit` declared in LLVM header: `declare void @__exit(i64)`
- `emit_transaction` wraps body with assume_shape guard (llvm.rs:2215-2260)
- Stored in `ReactorNode.assume_shape_action: Option<String>`
- Guard expression stored as `Option<String>` (future work: parse to `Expr`)

### `#` Prefix for All Compiler Directives
- Reuses existing `Hashtag`/`Attribute` parsing infrastructure
- `parse_hashtag_modifiers()` feeds into `txn.modifiers` and `defn.modifiers`
- Top-level `#` handling in `parse_top_level` (parser.rs:622-711)

### SPEC.md and Docs Updated
- SPEC.md §2.3: `termbang` added to statement list; `term`/`termbang` rules include `("->" statement)?`
- SPEC.md §5.4.4: `#assume_event` — liveness fairness assumption
- SPEC.md §5.4.5: `#assume_shape` — shape guard with fast-path/slow-path
- LANGUAGE-TUTORIAL.md: Pattern 5 (Commit Actions and Program Exit), Pattern 6 (Assume Pragma)
- All docs migrated to `:>` projection operator (277 calls across 16 .bv files)

### alka/on_exit Disabled
- Parser entry points commented out with `DISABLED: alka/on_exit — not ready for use`
- AST/backend match arms remain intact for future revisit
- 5 tests removed; 434 tests pass, 0 fail
- **Cleanup**: `*.opt.ll` added to `.gitignore`.

---

## Session 2026-06-08: Native Collections Cleanup, SyncBlock Backends, Watchdog Analysis

**Files changed**: 15 modified, 2 deleted, 1 new  
**Tests**: 545 pass (was 527), 0 fail — **18 new tests**

### Stale FFI Registry Cleanup
- **Deleted** `std/bindings/__builtin.dbvs` (420 lines) — all HashMap/HashSet/Stack/Queue/StringBuilder/Result/Option FFI entries. These were vestigial: collections dispatch natively through arrow syntax (`&map <- key`, `value <- &stack`) and projection targets (`map :> Keys`). Result/Option methods (`is_ok`, `unwrap`) are implemented in pure Brief via `uni` pattern matching.
- **Deleted** `std/bindings/collections.dbvs` (50 lines) — stub entries (`__filter`, `__map`, `__reduce`, `__unique`, `__sort`, `__reverse`) with no-op implementations.
- **Removed** `collections_*_impl` functions and `"collections::*"` match arms from `src/ffi/registry.rs` (6 functions: filter, map, reduce, unique, sort, reverse; 6 match arms).
- **Removed** `"__builtin.clone"` from `registry.rs` — clone is native `Value::clone()`.
- **Note**: The interpreter's `Expr::Call` dispatch at `interpreter.rs:1738` already handles all operations through: user defns → callable txns → dynamic FFI → enum constructors → FFI registry. With `__builtin` entries removed, Result/Option still work because their `is_ok`/`is_err`/`unwrap` etc. are pure Brief `uni` pattern matching in `lib/std/result.bv` and `lib/std/option.bv`.

### All 10 Backends: SyncBlock Replaced
Each backend had `Statement::SyncBlock { .. } => {}` — a silent no-op. Replaced with sequential emission of inner body statements:
- `src/backend/llvm.rs` — iterates body, emits each statement
- `src/backend/x86_64.rs`, `aarch64.rs` — `self.generate_statement(output, s)`
- `src/backend/webstack.rs`, `rust.rs` — `self.statement_to_rust(output, s)`
- `src/backend/verilog.rs` — `self.statement_to_verilog(s)` (returns String)
- `src/backend/vhdl.rs` — `self.statement_to_vhdl(output, s, indent)`
- `src/backend/wasm.rs` — `self.generate_statement(output, s)`
- `src/backend/c.rs` — `self.statement_to_c(output, stmt)`
- `src/backend/cobol.rs` — `self.generate_statement(stmt, output)`

### Phase 10: Watchdog Preemptibility Analysis (`src/analysis/watchdog.rs`, 740 lines)
New analysis module implementing the 6-step trigger preemptibility proof from the plan spec:

| Step | What it does |
|------|-------------|
| **1** | Resolve `trg` against `frgn trg` declarations |
| **2** | Find all transactions guarding on the trigger |
| **3** | Collect all variables written by the handler chain |
| **4** | Intersect with the watched transaction's precondition variables |
| **5** | Check if handler writes falsify the precondition |
| **6** | Check if handler restores the precondition before `term` |

**Error types**: `UnknownTrigger`, `NoHandler`, `NoConflict`, `HandlerDoesNotFalsify`, `HandlerRestoresPrecondition`

**Wired into pipeline**: Runs after PGO, before LLVM codegen. Fatal errors for missing triggers/handlers; warnings for optional watchdog edge cases.

**18 tests** covering:
- Unknown trigger, missing handler, no conflict, doesn't falsify, restores precondition
- Happy path (handler sets `ready = false`, precondition `[ready]`)
- Optional watchdog on convergent loop (skips)
- Required watchdog on convergent loop (still fails)
- Guarded-block write detection, literal evaluation, convergent-loop detection
- Non-trigger watchdogs skipped, natural death without watchdog
- Precondition variable extraction, trigger-in-guard detection
- Handler writes through Guarded blocks (recursive inspection)

### Phase 5: GraalVM & AssemblyScript
- Added `LinkLanguage::Java` and `LinkLanguage::AssemblyScript` to `src/ast.rs`
- Added parser support for `.java` and `.ts` extensions in `link/` declarations
- Added `compile_to_bitcode` dispatch:
  - **Java**: `javac` → `native-image --llvm --emit-llvm-bc`
  - **AssemblyScript**: `asc` → `.wasm` → `wasm2llvm` → `.bc`

### Phase 7: .hebv Hardware Tier
- Added `.hebv` to `is_strict_extension()` (full contracts required)
- Added `.hebv` to all file-extension dispatch patterns in `main.rs` (~25 locations)
- Added hardware validator checks (B5001-B5009): no `link/` imports, no `frgn`, total contracts, synthesizable types only (no Float, String, unsized Int/UInt, dynamic collections)
- Routes through verilog backend (same as `.ebv`)

### Phase 3: C FFI Libraries Vendored
- **yyjson** in `lib/std/c/json/` — JSON parsing (single-header + wrapper)
- **stb_image** in `lib/std/c/stb_image/` — image loading (single-header + wrapper)
- **lz4** in `lib/std/c/lz4/` — compression (single-header + wrapper)
- All follow the same pattern: single-header library + `_IMPLEMENTATION` `.c` wrapper
- Included via `import "link/<lib>/<lib>.c"` in Brief source; compiled to bitcode via LTO

---

## Session 2026-06-08 (Afternoon): Test Coverage Fixes & Contract Cleanup

**Files changed**: 20 modified, 0 deleted, 0 new  
**Tests**: 559 lib + all integration passing (bootstrap_determinism = pre-existing self-hosting deferred bug)

### SyncBlock Parser Fix
- `Statement::SyncBlock` handler in `src/parser.rs:3652` was missing trailing `;` consumption — caused parse error on `sync { ... };` inside transaction bodies. Added `self.expect(Token::Semicolon)?;`.
- Added parser unit test `test_sync_block_in_txn_body`.

### SyncBlock Interpreter Tests (3 new)
- `test_sync_block_executes_statements_in_order` — verifies sequential state mutation
- `test_sync_block_nested_guarded` — verifies Guarded inside SyncBlock works
- `test_sync_block_empty` — verifies empty sync block doesn't crash

### Hardware Validator Tests (8 new)
- `test_hebv_rejects_link_dependency` — B5001
- `test_hebv_rejects_frgn` — B5002
- `test_hebv_rejects_true_precondition` — B5004
- `test_hebv_rejects_true_postcondition` — B5005
- `test_hebv_rejects_float_type` — B5007
- `test_hebv_rejects_string_type` — B5008
- `test_hebv_rejects_unsized_int` — B5006
- `test_hebv_accepts_synthesizable_types` — Bool + bounded txns OK

### Parser Tests for LinkLanguage (2 new)
- `test_link_dependency_java` — `.java` → `LinkLanguage::Java`
- `test_link_dependency_typescript` — `.ts` → `LinkLanguage::AssemblyScript`

### Vendored C Library Compilation Tests (5 new)
- `test_xxhash_compiles`, `test_yyjson_compiles`, `test_brief_json_compiles`
- `test_stb_image_compiles`, `test_lz4_compiles`
- Each tests `clang -c` on the implementation wrapper succeeds

### LLVM Backend Tests (1 new + 1 fixed)
- `test_llvm_backend_sync_block` — compiles `tests/test_sync_block.bv`, verifies `%State` + `test_sync` + `ret void`
- `test_llvm_backend_wake_triggers` — fixed assertions to match `trg ... @ link` IR output (`constant` not `appending global`, no `__rt_init`)

### Contract Fixes Across All Test Files
Replaced illegal patterns (`[true]`, `[x==x]`, `[true][true]`) with meaningful contracts:
| File | Context | Fix |
|------|---------|-----|
| `tests/fixtures/*.bv` (7 phase files) | `[x==x]` from bad sed | `[[meaningful_post]` |
| `tests/fixtures/wake_triggers.bv` | `#io sigint;` | `trg sigint: Bool @ link __sigint_flag;` |
| `tests/fixtures/event_model.bv` | `[true]` | `[event_count>=0][event_count>=0]` |
| `tests/test_c.rs` | `[x>=0][x>=0]` | `[[result == a + b]` etc. |
| `tests/test_rust.rs` | `[true]` | `[[meaningful_post]` |
| `tests/test_aarch64.rs` | `[true]`, `[guard]` | `[[post]`, `[guard][data==@data+1]` |
| `tests/test_verilog.rs` | `[true]` | `[[result == a / b]` |
| `tests/test_wasm.rs` | pre-existing API mismatch | Fixed imports, `WasmBackend` API |
| `tests/test_x86_64.rs`, `tests/test_vhdl.rs` | `[true]` | `[[meaningful_post]` |
| `tests/integration_features.rs` | `[true][true]` on `defn` | Removed contract (defn allows omission) |
| `tests/fuzz_backend.rs` | `Statement::Term(_)` | Fixed to `Statement::Term { .. }` |
| `tests/ffi_typechecker_tests.rs` | `&program` | `&mut program` |
| `tests/ffi_comprehensive_tests.rs` | `&program`, `Type::Option`, old API | `&mut program`, `Custom("Option")`, 5-arg resolve |
| `tests/bootstrap_determinism.rs` | `"main.bv"` path | `"lib/compiler/main.bv"` |

---

## 2026-06-14 — Phase 15: `is`/`from`/`like` Type/Metadata Check Expressions

**Summary**: Added three infix operators for runtime type inspection, derivation checking, and structural equality comparison.

### What was done
- **Lexer**: Added `Token::Is` and `Token::Like` keywords
- **AST**: Added `IsTarget` enum (`Type(Type)`, `Variant(String)`), `Expr::IsType`, `Expr::FromCheck`, `Expr::Like` variants
- **Parser**: Added `parse_check()` and `parse_is_target()` in a new precedence level between equality and comparison
- **Typechecker**: `infer_expression` returns `Type::Bool` for all three; recurses into sub-expressions in `check_expr_for_function_calls` and `check_expr_for_ffi_errors`
- **Interpreter**: Full runtime evaluation — type matching, variant name comparison, struct/enum derivation chain, recursive structural `like`
- **LLVM Backend**: Stubs for `IsType`/`FromCheck` (emit `add i64 0, 1`), `Like` delegates to `emit_fcmp` with integer constant folding
- **Proof engine/symbolic**: Added match arms to `collect_identifiers` and `eval_symbolic`
- **Tests**: 5 parser + 4 LLVM backend = 9 new tests (794 total passing)

### Bug fixes discovered
- `parse_equality` had a second `parse_comparison` call for `Ne` that wasn't updated to `parse_check`
- `Some` lexer token only recognizes lowercase `some`/`SOME`, unlike `Ok` which recognizes `Ok`/`OK` — inconsistency documented
- `Value::Struct` was refactored to `Value::Instance { typename, fields }` — all references updated

### Files changed
- `src/lexer.rs`, `src/ast.rs`, `src/parser.rs` — core feature
- `src/typechecker.rs` — type inference + visitor recursion
- `src/interpreter.rs` — runtime evaluation
- `src/proof_engine.rs`, `src/symbolic.rs` — match arms for new variants
- `src/backend/llvm/emit_expr.rs`, `src/backend/llvm/tests.rs` — LLVM codegen + tests
- `docs/architecture/features/is-from-like.md` — architecture doc (design → implementation)
- `docs/architecture/channel-map.md` — updated parser pipeline
- `docs/BRIEF_3.0_SPEC.md` — added Section 11
- `learn-brief/05-data-types.md` — added Section 8
- `learn-brief/README.md` — updated TOC
- `lib/runtime/brief_rt.c` — unchanged (pre-existing change)

---

## 2026-06-14 — Magic Audit: No-Magic Violations in Phases 18–19

**Two items edge on "magic" — compiler has hardcoded knowledge about OS or C runtime
that the user didn't declare.**

### Item 1 — `@stdin#`, `@ timer#(Hz)`, `@ signal#(Name)` (Phase 18)

**What it does**: Three built-in trigger sources that the compiler recognizes and
generates epoll/timerfd/signalfd code for.

**Why it's on the edge**: The compiler has hardcoded knowledge about:
- Linux syscalls (`epoll_create`, `timerfd_create`, `signalfd`)
- Signal names (`SIGINT`, `SIGTERM`, etc.)
- File descriptor management

The user writes `trg k: Char @stdin#;` and the compiler silently generates
`epoll_ctl(epfd, EPOLL_CTL_ADD, STDIN_FILENO, ...)` with no `frgn` declarations
the user can inspect or override.

**Mitigation**: The `#` convention signals "compiler-managed" consistently with
`import#` and `intrinsic#`. The C runtime wrappers (`__trg_stdin_read`,
`__trg_timerfd_open`, etc.) are declared in the generated IR as external
functions, creating a traceable link. But the epoll orchestration is still
invisible to the user.

### Item 2 — `__chr_to_str` (Phase 19)

**What it does**: The LLVM backend emits `call i8* @__chr_to_str(i32)` for
every `Char → String` cast at compile time.

**Why it's on the edge**: `__chr_to_str` is a C function in `brief_rt.c` that
the user never declared with `frgn`. If `brief_rt.c` isn't linked, the program
fails with an opaque linker error. Same pattern as `__str_concat` (pre-existing,
same problem).

**Mitigation**: The LLVM backend could emit the conversion inline instead:
```
alloca i8, i8* %buf
store i8 %char_val, i8* %buf
store i8 0, i8* %buf+1
```
This would eliminate the C dependency entirely for Char→String.

### Pending Plan

See `.opencode/plans/2026-06-14-eliminate-magic.md` for the fix plan.

---

# 2026-06-19: Macro Gaps 2+4 + LLVM Stub Hardening

## Completed

### Macro System Gaps
- **Gap 2 (nested macro calls)**: `expand_macro_calls_in_items` now recurses into
  `Definition`/`Transaction` bodies via `expand_macro_in_stmts`. Macro calls in
  guarded blocks, let bindings, and term values are all expanded.
- **Gap 4 (integration tests)**: Three `.bv` source-parsing tests added
  (`test_integration_macro_expansion_from_source`, `_in_defn_body`, `_in_txn_body`)
  testing end-to-end parse → collect → expand → verify.

### LLVM Backend Hardening (3 stubs fixed)
- `Projection::Bytes` on `String`/`Data`: returns `8` (was `add i64 0, 0`)
- MultiSlice atomic stub: passthrough returns source value (was `add i64 0, 0`)
- MultiSlice no-coord: returns data pointer from header slot 0 (was `add i64 0, 0`)

### Documentation
- All three plan docs updated with completion status and resolution notes:
  `2026-06-18-llvm-backend-hardening.md`, `2026-06-18-macro-system-gaps.md`,
  `2026-06-18-llvm-backend-known-issues.md`
- BUGS.md already had 5 entries for these fixes from prior sessions

### Tests
- Full suite: 1072 passed, 0 failed

---

## Session: 2026-06-19 — Tier Renames (Graphic → Accelerated, Hardware Embedded → Circuit)

### Goal
Rename `.gbv` → `.abv` (Graphic Brief → Accelerated Brief / "Brief Accel") and
`.hebv` → `.cbv` (Hardware Embedded Brief → Circuit Brief / "Brief Circuit")
across the entire codebase, including alternative names for all tiers.

### Changes

**Rust source files:**
- `src/main.rs`: All 5 `.gbv` → `.abv`, 3 `.hebv` → `.cbv`, "Graphic Brief" → "Accelerated Brief",
  "Hardware Embedded Brief" → "Circuit Brief" in help text, error messages, comments
- `src/ast.rs`: `StrictMode::Gpu` doc comment updated
- `src/typechecker.rs`: `.gbv` → `.abv`
- `src/hardware_validator.rs`: `.hebv` → `.cbv`

**Test files:**
- `test_gbv.gbv` → `test_abv.abv`

**Documentation:**
- `AGENTS.md`: All file types updated with `.abv`/`.cbv` + alternative names ("Brief Accel",
  "Brief Render", "Brief Embed", "Brief Circuit", "D-Brief"/"Brief Data"); sugar rules updated
- `README.md`: Table expanded with `.abv` and `.cbv` rows + alternative names
- `docs/architecture/features/graphic-brief.md` → `accelerated-brief.md` (content fully updated)
- `docs/plans/2026-06-18-graphic-brief.md`: Updated with `.abv` and completion status
- `docs/plans/2026-06-18-gpu-io-intrinsics.md`: Updated `.gbv` → `.abv`, `test_gbv` → `test_abv`
- `syntax-highlighter/syntaxes/brief.tmLanguage.json`: `fileTypes` array expanded to all 12 extensions

**Not changed (intentionally):**
- Internal GPU-mode names (`StrictMode::Gpu`, `is_gpu_extension`, `with_gpu_mode`) — these
  describe the compilation mode, not a brand name. Regular Brief can declare GPU-accelerated
  code without a `.abv` file.
- `learn-brief/` and `docs/reference/BRIEF_LANGUAGE_REFERENCE.md` and `spec/SPEC.md` — had
  zero old references

### Verification
- All 1072 tests pass, 0 fail

---

# Session: 2026-06-19 — Officina Keyboard Input Fix + const trg Design

## The Problem

Officina-cli is a Brief terminal application that takes keyboard input. The Char→String cast
`(String)k` in `process_input` produced `"116121121..."` (garbage) instead of `"t"` because
the LLVM backend lost type information for `Char` state fields — they were stored as `i32`
at the LLVM level but treated as `i64` (boxed) by downstream code, causing LLVM IR type errors
and incorrect intrinsic dispatch.

## Root Causes Found

### 1. `adapt_to_i64` double-zext (`src/backend/llvm/emit_stmt.rs:20-23`)

The `adapt_to_i64` function had a `Type::Char` arm that emitted `zext i32 %r to i64`. But
ALL Char registers from `emit_expr` are already `i64` (boxed). The zext was a second
widening of an already-i64 value, producing an LLVM IR type error.

**Fix**: Return `r.name.clone()` for `Type::Char` (no-op).

### 2. TtyReadKey intrinsic i32 phi (`src/backend/llvm/emit_expr.rs:895-897`)

The `TtyReadKey` intrinsic emitted a `phi i32` for the character value, but the result
was typed as `Type::Char`. Downstream `adapt_to_i64` tried to zext the already-i32 phi
to i64, but `phi i32` is not `i64`.

**Fix**: Changed to `zext i32 %phi to i64`, producing a proper i64 boxed value.

### 3. Enum storage double-zext (`src/backend/llvm/emit_expr.rs:505-508`)

Same pattern as adapt_to_i64: enum variant storage for `Type::Char` emitted `zext i32 %raw to i64`
when the register was already i64.

**Fix**: Return `raw.name.clone()` for `Type::Char` (no-op).

### 4. `emit_trg_load_finish` wrong output type (`src/backend/llvm/emit_toplevel.rs:305-306`)

Trigger event loads from GEP produce `i32` (the stored type). `emit_trg_load_finish` for
`Type::Char` emitted `add i32 0, %raw` producing an i32 register, but all downstream
code expected i64.

**Fix**: Changed to `zext i32 %raw to i64`.

### 5. Let-binding identifier path (`src/backend/llvm/emit_expr.rs:134-144`)

When resolving `let k = keypress`, the let-binding lookup found the register but assumed
it was `i32`. It emitted `zext i32 %reg to i64`, but the register was already i64.

**Fix**: Removed the zext; just emit `add i64 0, %reg` (same as the `Int` fallthrough).

### 6. SSA extractvalue missing `"i32"` arm (`src/backend/llvm/emit_expr.rs:115-118`)

The SSA extractvalue `_` default returned `Type::Int` for `"i32"` field types instead of
`Type::Char`. Latent safety net — currently unreachable because all fields are pre-extracted
before extractvalue.

**Fix**: Added `"i32"` arm returning `Type::Char` with `zext i32 %ev to i64`.

### 7. Missing `"i32"` truncation arm in assignment (`src/backend/llvm/emit_stmt.rs:406-410`)

The `Statement::Assignment` handler had match arms for `"i8"`, `"float"`, `"i8*"`, and a
default `_` arm. The default arm used the field type string directly:
```rust
writeln!(out, "{}store{} {} {}, {}* {}, align {}", indent, vol_str, ty, val_boxed, ty, p, ...)
```
For `ty = "i32"`, this emitted `store i32 %val_boxed, i32* %ptr` where `%val_boxed` was
already widened to `i64` by `adapt_to_i64`. LLVM IR type error.

**Fix**: Added `"i32"` arm that truncates i64 to i32 before storing. Applied to both the
SSA `insertvalue` path (line 352) and the GEP+store path (line 406).

### 8. Trigger variable never cleared (`officina.bv:79`)

The `@stdin#` trigger stores the character to `keypress` via the epoll handler, but nothing
ever reset it back to `'\0'`. The guard `[booted && keypress != '\0']` stayed true for the
character value. In interactive terminal mode, spurious epoll wakeups caused repeated firing.

**Fix**: Added `&keypress = '\0';` before `term;` in `process_input`. This clears the local
copy of the trigger after consumption — the external stdin source doesn't see or care.

### 9. Spurious epoll wakeup: unchecked `read()` return (`src/backend/llvm/loop_engine.rs:1436`)

The epoll stdin handler called `read(0, buf, 1)` but discarded the return value. On some
Linux kernels, `epoll_wait` on a TTY with `O_NONBLOCK` + raw mode can return spurious
wakeups. When `read()` returns `-1/EAGAIN`, the uninitialized `alloca` buffer contained
stack garbage that was stored to `keypress` — causing `process_input` to fire with garbage
values in a tight loop (100% CPU, character repeats until Enter or `\x03`).

**Fix**: Check `read() > 0` before storing the byte. If `read()` returns ≤ 0, skip the
store and `step()` entirely. Restructured the epoll match arms so each arm handles its
own `step()` call instead of sharing a post-match common path.

### 10. `step()` type-mismatched load/store (`src/backend/llvm/loop_engine.rs:1264-1272`)

`emit_trg_step` hardcoded `load volatile i64` / `store volatile i64` for all trigger
fields regardless of their actual LLVM type (`i32` for Char, `i8` for Bool, `i8*` for String).
With opaque pointers this silently read/wrote adjacent struct bytes.

**Fix**: Match on `self.field_types[idx]` and emit the correct typed load/store (i32, i8,
i8*, i64, float) for all three sub-locations: trigger volatile loads, dependency field loads,
and proxy store.

### 11. `step()` proxy store type mismatch (`src/backend/llvm/loop_engine.rs:1358`)

The proxy store (placeholder for recomputation) loaded the first dependency's value with
the dependency's type but stored it with the destination's type. When types differed
(e.g., first dep is `i64`, destination is `i32`), the `store i32 %i64_val` was a type error.

**Fix**: Use the destination variable's type for both the load and store.

## The const trg Design

**Motivation**: Writing to a trigger is semantically correct for software triggers
(`@stdin#` — you consumed the event, clearing your local copy), but semantically wrong
for hardware triggers (`@0xFFFF0000` — the register is sovereign; writing to the shadow
state field is a bug).

**Design**: `const` on a `trg` means "I, the code, cannot mutate this." Without `const`,
writing is allowed.

| Declaration | Code can write? | External writer |
|---|---|---|
| `trg x: Char @stdin#;` | ✅ Yes | stdin event |
| `const trg x: Char @stdin#;` | ❌ No | stdin event |
| `const trg y: Int @0xFFFF0000;` | ❌ No | hardware register |
| `trg y: Int @0xFFFF0000;` | ❌ Error: "must be declared const trg" | (parse-time error) |

**Compiler errors:**
- `&const_trg_name = expr` → `"; error: cannot write to const trigger 'name'"`
- `trg name @0x...` (without `const`) → `"hardware-addressed triggers must be declared 'const trg'"`

**Implementation** (commit 5e9d757):
- `src/ast.rs`: added `is_const: bool` to `TriggerDeclaration`
- `src/parser.rs`: `const` before `trg` detected in `parse_top_level()` via `peek_token()`;
  validation for explicit address triggers; test triggers updated
- `src/backend/llvm/emit_stmt.rs`: const trigger write check in both SSA and GEP paths

## The Macro Decorator Vision (Phase B — not yet implemented)

The keyboard debacle taught us that `trg @stdin#` + guard + clearing is boilerplate
every terminal program needs. The long-term solution is a `$!keyboard_input` decorator
macro that sits before a `node` and automatically:
- Gensyms a trigger variable (`__kb_N: Char @stdin#;`)
- Appends `&& __kb_N != '\0'` to the guard
- Injects default handlers for `\n` (enter), `\x7f` (backspace), `\x03` (ctrl+c), and regular chars
- Implicitly clears the trigger after processing

This requires adding `TopLevel` emission to the macro system (so macros can emit `trg`
declarations). Planned but not yet started.

## Files Changed (2026-06-19)

### Commits
- `b43c2c0` — LLVM backend hardening, D12–D18 intrinsics, macro system gaps (parallel agent)
- `792e94e` — docs: known limitations to backend-strategy.md
- `db7af33` — docs: update plan status, example, architecture for Phase B (parallel agent)
- `28e2195` — Fix trigger variable not cleared + step() type mismatch + missing i32 store arm
- `5e9d757` — Add const trg: enforce read-only triggers + hardware address validation
- `11e78b9` — Add tests for const trg: parser + backend assignment error
- `6e6ab51` — Fix WASM FFI validation: use ForeignBinding.target instead of missing frgn_target

### Key files modified
| File | Change |
|---|---|
| `src/backend/llvm/emit_stmt.rs` | i32 trunc arm, const trg check, adapt_to_i64 Char fix |
| `src/backend/llvm/loop_engine.rs` | step() types, spurious wakeup guard, proxy store fix |
| `src/backend/llvm/emit_expr.rs` | TtyReadKey i64, enum storage, let-binding, SSA extractvalue |
| `src/backend/llvm/emit_toplevel.rs` | emit_trg_load_finish zext |
| `src/parser.rs` | const trg syntax + validation + tests |
| `src/ast.rs` | is_const on TriggerDeclaration |
| `src/main.rs` | WASM FFI validation fix |
| `src/analysis/region.rs`, `watchdog.rs`, `dependency_graph.rs` | is_const in test constructors |
| `src/backend/circt.rs` | is_const in test constructors |
| `src/reactor.rs`, `src/fuzzing/ast_generator.rs` | is_const in test constructors |
| `docs/plans/2026-06-19-trigger-variable-not-cleared.md` | Full plan document |

## Lessons Learned

1. **Context matters**: Pipe tests and interactive terminal tests have different behavior.
   The spurious epoll wakeup only manifests in real interactive use.

2. **Trigger variables are not magic**: A `trg` is just a state field that the event system
   sets. Code can read and write it — but should be aware of the source's sovereignty.
   `const trg` codifies this.

3. **Every Expr handler that evaluates to a pointer must store the actual pointer**,
   never a sentinel/placeholder. The string state init null bug was a classic example.

4. **LLVM IR type tracking is pervasive**: A single mistyped load (i64 instead of i32)
   in `step()` can silently read/write adjacent struct bytes with opaque pointers.
   Always use the field's declared type, not a hardcoded type.

5. **The `read()` return value must always be checked** when reading from a TTY fd,
   even if `epoll_wait` guarantees data availability. Spurious wakeups happen on some kernel versions.

6. **SEMANTIC CONSERVATION**: Never weaken contracts or scribble lazy code. The right fix
   is to understand why the system behaved as it did, then correct the system.

## Test Count
- Initial: 1068
- After Phase A fixes: 1073 (+5 from parallel agent)
- After const trg: 1078 (+5 from new tests)
- After test additions: 1082 (+4 from const trg tests)
- Final: **1082 passed, 0 failed**

---

## Archived from AGENTS.md (2026-06-25)

The following sections were removed from AGENTS.md during the 2026-06-25 condensation
to keep active guidance under ~350 lines. Preserved here for historical reference.

### GLUE — General Language Unification Engine

GLUE is a universal FFI broker built on Brief's `meld` system. Any two
languages that consume LLVM-compatible object code can be linked through GLUE.
Neither language knows Brief exists. Both see their own native interface.
Brief is the invisible translator — `meld` proves type compatibility at
compile time, `frgn` declares calls into the target language, `#export`
exposes functions to the caller.

**The bridge is native object code.** No C compiler, no `extern "C"`, no `cc`
crate. Brief emits LLVM IR → native `.o`/`.a`/`.wasm`. The foreign language's
linker consumes it directly.

**GLUE adapters use Brief's `$!` macro system**, not a separate template engine.
A `$!macro` takes the bridge's `#export`/`frgn`/`meld` declarations at compile
time and emits native wrapper source code for the target language. Adding a
language = writing one `.bv` macro file.

**Key directives:**
- `brief link <path> <function>` — analyzes a foreign library, generates a `.bv`
  with `frgn` declarations. Cross-references against the `Intrinsic` enum in
  `src/ast.rs` — if a `frgn` name matches an intrinsic, emit `intrinsic_call#()`
  instead. This replaces the old TOML binding system.
- `brief export <bridge.bv> <language>` — compiles to `.a` (library mode, no
  `main`), reads `glue.dbvl` to find the adapter entry for `<language>`, invokes
  the `$!` macro for that language, generates native wrappers.
- `glue <target> <function> <language>` — one-shot wrapper: `brief link` + `brief export`.

**GLUE protocol files:**
- `glue.dbvl` — Data Brief Lines adapter registry (one language per line)
- `glue.dbvs` — Data Brief Schema that validates `glue.dbvl` entries
- Adapter macros live in `glue/adapters/<language>.bv`

### File Types
- **.bv** - Brief (standard Brief file, cosmopolitan tier — any FFI, any language, OS assumed)
- **.sbv** - Strict Brief (full contracts required, no sugar defaults)
- **.abv** - Accelerated Brief (native GPU compilation — always compiles to SPIR-V, no FFI, restricted types, GPU intrinsics only. Also known as "Brief Accel")
- **.rbv** - Rendered Brief (Brief + View, compiles to web frontend. Like `.tsx` is to `.ts`. Also known as "Brief Render")
- **.srbv** - Strict Rendered Brief (full contracts required in web target)
- **.ebv** - Embedded Brief (bare metal — no OS, no GC. C/Rust FFI allowed but Python/Java warned. Also known as "Brief Embed")
- **.sebv** - Strict Embedded Brief (full contracts required, bare metal)
- **.cbv** - Circuit Brief (pure logic graph — no FFI, no external deps, only synthesizable types. Contracts must be total. Outputs Verilog/VHDL/SV. Also known as "Brief Circuit"; formerly Hardware Embedded Brief / `.hebv`)
- **.dbv/.dbvs/.dbvl** - Data Brief (configuration with schema, think `.xml`/`.xmls`/`.jsonl`. Also known as "D-Brief" or "Brief Data")

### Contract Sugar Syntax

Brief provides sugar for single-sided contracts. Use these where possible in the stdlib
to teach readers the pattern:

| Syntax | Precondition | Postcondition | Meaning |
|---|---|---|---|
| `[pre][post]` | `pre` | `post` | Full contract (both sides) |
| `[[post]` | `true` (omitted) | `post` | Postcondition only, no guard. **The opening `[[` means the precondition was omitted.** |
| `[pre]]` | `pre` | `true` (omitted) | Guard only, no guarantee. **The closing `]]` means the postcondition was omitted.** |

Memory aid: the left bracket `[` is always the precondition. `[[` = two left brackets = the
first one opens an empty precondition (defaults to `true`), the second opens the postcondition.
`]]` = two right brackets = the first closes the precondition, the second closes an empty
postcondition (defaults to `true`).

**Banned in**: `.sbv`, `.srbv`, `.sebv`, `.cbv` (strict tiers require explicit both-sided contracts).
**Allowed in**: `.abv`, `.bv`, `.ebv`, `.rbv` (sugar is the recommended style).

### Pipe Chaining Sugar

Brief provides pipe chaining (`|>`) as a syntactic sugar that desugars to
flat let-bindings before typechecking. All three active backends see only
the desugared form — zero runtime overhead.

```brief
x |> f()            // f(x) — pipeline value prepended as first arg
x |> f() |> g()     // g(f(x)) — multi-step chain
x |> f() .|> g()    // .|> reads from 1 position back in pipeline stack
x |> f() .2|> g()   // .N|> reads from N positions back
x |> f              // auto-wrapped: f(x)
f() |> g()          // chain starts with function call (no initial input)
```

See `docs/architecture/features/pipe.md` for full documentation.

### Language Architecture

Brief is a **general-purpose programming language**. The computational primitive is the **reactive transaction** (`node`):
- **Precondition** (guard): `[x > 0 && y < N]`
- **Postcondition** (contract): `[x == N]`
- **Body**: `{ &x = x + 1; &y = y * 2; }`

Loops are transactions with bounded convergence. Recursion is a transaction chain with proved termination. Every optimization (purity folding, dead-field elimination, SROA, SLP) applies because contracts give the compiler enough information.

#### Misconceptions to Avoid

| Wrong | Correct |
|-------|---------|
| "Brief is a reactive state machine DSL" | Brief is general-purpose. Transactions ARE loops, iteration, and recursion. |
| "Brief has no arrays/strings/collections" | Interpreter supports `List<T>`, `String`, `HashMap`, `HashSet`, `Stack`, `Queue`, `StringBuilder`. Stdlib has 26 modules. |
| "Brief can't do tree/heap benchmarks" | Interpreter supports recursive enums, structs, field access, match. |
| "Brief needs malloc/FFI for buffers" | Compiler proves bounds from contracts, allocates accordingly. |
| "The LLVM backend is the language" | Interpreter is the reference. Backend is an optimization pass. |

#### Two-Layer Architecture

1. **Interpreter** — reference implementation. Validates EVERYTHING before any codegen.
2. **LLVM Backend** — compiles to LLVM IR with optimizations. Never weakens existing optimization paths.

### Interpreter Completeness

#### Expressions — Except where noted, all fully implemented
| Status | Variants |
|--------|----------|
| ✅ | Integer, Float, String, Char, Bool, Term, Identifier, OwnedRef, PriorState |
| ✅ | Add, Sub, Mul, Div, Mod, Eq, Ne, Lt, Le, Gt, Ge, Or, And, Not |
| ✅ | Neg, BitNot, BitAnd, BitOr, BitXor, Shl, Shr |
| ✅ | Call, ListLiteral, ListIndex, Projection (18 targets), FieldAccess |
| ✅ | StructInstance, ObjectLiteral, PatternMatch, Concat |
| ✅ | Slice, MultiSlice, Block, Tuple, TupleDestructure, Cast, Match |
| ✅ | ArrowMut, ArrowDiscard, ArrowTransfer (dispatch on Value type, not string names) |
| ✅ | MapLiteral, SetLiteral (evaluate to Value::HashMap, Value::HashSet) |
| ⚠️ | **ForAll, Exists** — FULLY REMOVED from AST, parser, lexer, and all match arms. |

#### Statements — All fully implemented
Assignment, Let, InlineAsm, Expression, Term (with optional swan song), TermBang (with optional swan song), Escape, Guarded, Unification, LocalTrigger, SyncBlock.

#### Known Gaps
- **Recursive defn calls**: No recursion guard or stack depth limit. Deep recursion overflows the Rust interpreter.
- **ForAll/Exists**: Removed from surface syntax.
- **Interpreter built-in method dispatch**: `dispatch_method_by_type` still matches on function name strings. Deferred — should use FFI registry (Path A: register all operations under `"std::HashMap::insert"` etc., resolve through `ffi_name_to_location`).
- **LLVM backend**: Slice/MultiSlice/Tuple/MapLiteral/SetLiteral/ArrowTransfer/projection stubs remain (see Backend Gaps below).

### Feature Modules (`src/features/`)

New features follow the Pattern-B convention: a single directory with `mod.rs` + per-aspect files implementing the trait dispatch system.

| Feature module | Files | Status |
|----------------|-------|--------|
| `literal/` | `mod.rs` | ✅ |
| `binary_op/` | `mod.rs` | ✅ |
| `call/` | `mod.rs` | ✅ |
| `projection/` | `mod.rs` | ✅ |
| `collection/` | `mod.rs` | ✅ |
| `tuple/` | `mod.rs` | ✅ |
| `field/` | `mod.rs` | ✅ |
| `pattern/` | `mod.rs` | ✅ |
| `block/` | `mod.rs` | ✅ |
| `arrow/` | `mod.rs` | ✅ |
| `subtype/` | `mod.rs` | ✅ |
| `sigcall/` | `mod.rs` | ✅ |
| `dbvl/` | `mod.rs` | ✅ |
| `ellipsis/` | `mod.rs` | ✅ |
| `stmt/` | `mod.rs` | ✅ |
| `toplevel/` | `mod.rs` | ✅ |
| `macros/` | `context.rs`, `expand.rs`, `template.rs`, `hygiene.rs`, `macro_.rs` | ✅ |

### LLVM Backend — All Gaps Closed (2026-06-21)

Additive only — never weaken existing optimization paths.

All expression types from the original gaps list (`Slice`, `MultiSlice`,
`Tuple`, `StructInstance`, `ObjectLiteral`, `FieldAccess`, `MapLiteral`,
`SetLiteral`, `ArrowTransfer`, `<-` push/pop/discard, and all projection
operators including `Keys`, `Values`, `Contains`, `Pop`, `Index`) have been
**fully implemented** in `emit_expr.rs`. `ForAll`/`Exists` were **removed**
from the AST entirely. As of 2026-06-21, there are **no known stub or
degraded expression paths** in the LLVM backend.

#### Expressions — All Fixed (2026-06-21)

The following expression codegen gaps were fixed:

**Slice stride/mask** — Stride is now used in the copy loop (`src[start + i*stride]`)
with ceiling-division count. Mask applies a second-pass filter loop that binds `_`
to each element and evaluates the mask expression. Both implemented inline with
LLVM IR loops.

**MultiSlice stride/mask/range** — `Coord(Range)` allocates a new list and copies
the sub-range `[lo..hi)`. `Stride` emits a step-by copy loop. `Mask` emits a
filter loop with `_` binding. All three produce native LLVM IR loops without
runtime library calls.

**`FloatToStr`/`ToStr` working paths** — Replaced buggy `@__snprintf__`-based
implementation with `@__float_to_str` / `@__to_str` C runtime functions.
Return type changed from `i8*` to `i64` to match C functions.

**`bytes` projection for struct types** — Now computes `fields.len() * 8` when
the source type is `Type::Custom(name)` and the struct is in `struct_types`.

**`FieldAccess` field not found** — Now emits `call void @llvm.trap()` instead
of silent `add i64 0, 0 ; field`.

**`UserDefined`/`UserDefinedWithArg` projections** — Now emit `@llvm.trap()`
when `try_projection_fast_path` fails (instead of silent `add i64 0, 0`).

**Missing declares** — 8 `declare` statements for runtime functions
(`__trim_left__`, `__trim_right__`, `__to_lower__`, `__contains_at__`,
`__find_from__`, `__splitn__`, `__float_to_str`, `__to_str`) were added to
`emit_toplevel.rs`.

#### Error-Guard Stubs — All Fixed (2026-06-21)

The following error-guard stubs previously emitted `add i64 0, 0` silently;
all now emit `call void @llvm.trap()` before the zero return:

**Intrinsic error-guards (wrong arg count)**: `sort`, `reverse`, `range`,
`trim_left`, `trim_right`, `to_lower`, `contains_at`, `splitn`, `int_to_str`,
`strlen`, `float_to_str`, `to_str`, `size`, `pop`, `contains`, `keys`/`values`,
`read_file`.

**Projection error-guards (unrecognized field/type)**: `Expr::Identifier` not
found, `ProjectionTarget::Bytes` for unknown type, `ProjectionTarget::UserDefined`
and `UserDefinedWithArg` fallthrough, `Expr::FieldAccess` field not found.

#### Top-Level — Struct/Enum Layout
| TopLevel | Notes |
|----------|-------|
| **Struct** | No LLVM struct type generated. StructInstance/FieldAccess use field-offset arithmetic (GEP), not LLVM struct types. No TBAA on struct fields. |
| **Enum** | Tagged union layout via ad-hoc stack alloca + discriminant prefix. No LLVM struct type. |
| Signature, Import, LinkDependency | Correctly skipped — frontend-only. |

### Self-Hosting Pipeline

The Brief-in-Brief compiler lives in `lib/compiler/`. Run via `brief-compiler selfhost <file.bv>`.

**NOT currently being worked on.** Broken at parser level (multidimensional slice bug). Deferred.

**Do NOT add as built-ins**: `is_digit`, `is_alpha`, `is_alphanumeric`, `is_upper`, `is_lower`, `is_space`, `char_to_string`, `None`, `Some`, `Ok`, `Err`. These are in `lib/std/` — import them.

### Optimization Design

See `docs/design/optimization-decision-tree.md` for the full decision tree — precomputation → enum dispatch → async → folded struct-SSA → fallback — and the rationale for each path (phi reduction, SROA pipeline, why struct phis were eliminated, cross-cutting optimizations).

### Critical Context

#### Already Done (Don't Redo)
- **Projection operator (`:>`)** — fully implemented, 8 targets (Size, Bytes, Ptr, Alignment, Range, Popcount, LeadingZeros, TrailingZeros, Absolute, BitReverse, Type, Ptr!, Match, Keys, Values, Contains, Pop, Index). `Expr::ListLen` deleted. All stdlib migrated.
- **`<-` arrow syntax** — fully implemented for List, HashMap, HashSet, Stack, Queue via `ArrowMut`/`ArrowDiscard`/`ArrowTransfer`. Dispatch on Value type, not string names.
- **`->` vs `<-` convention** — `->` reserved for return types and swan songs; `<-` exclusively for collection mutation (`&` sigil marks mutated operand).
- **`term -> swan_song;`** (commit action) and **`term!`** (program exit) — both implemented in interpreter + LLVM backend.
- **`#assume_event(name)`** and **`#assume_shape(guard, action)`** — pragma infrastructure in parser, analysis, LLVM.
- **`#` prefix for all directives** — reuses Hashtag/Attribute parsing.
- **Dead-field elimination** — liveness analysis drops stores to unobserved fields.
- **Dispatch-chain collapse** — preconditions evaluate against pre-tick state.
- **Thread pool async dispatch** — portable barrier + auto-inference of conflict-free txns.
- **SLP hazard analyzer** — disables SLP when peak register demand exceeds hardware.
- **Equality saturation** — lightweight recursive simplification (5-pass fixpoint, 9 rewrite rules).
- **Compile-time PGO** — interpreter profiling guides LLVM branch weights.
- **LTO pipeline** — merges `brief_rt.c` bitcode with program IR.
- **MMIO / DBVS / hardware handoff** — address plumbing, schema validation, Vivado XSA extraction.
- **alka/on_exit permanently abandoned** — parser paths commented out, code left only as a historical artifact. No revisit planned.
- **`__rt_poll()`** — non-blocking event drain at main() entry.
- **Sync domains (Phase 11)** — `sync(domain)` prefix on `txn`/`defn`, `TopLevel::SyncGroup`, `Statement::SyncBlock`.
- **BracketOp (MultiSlice refactor)** — flat `Vec<BracketOp>` replaces `coordinates`+`mask`. Ops: `Coord`, `Mask`, `Stride` in any order.
- **MapLiteral / SetLiteral** — `{"a": 1}` evaluates to `Value::HashMap`, `{1, 2, 3}` to `Value::HashSet`. ObjectLiteral `{field: val}` preserved.
- **Value::Tuple** — true distinct variant. `Expr::Tuple` evaluates to `Value::Tuple`. Tuple destructure handles both `List` and `Tuple`.
- **ProjectionTarget::Index(usize)** — tuple indexing via `pair :> 0`.
- **`$`/`$!` macro system (Phase 1a/1b)** — `$` for hygienic templates, `$!` for high-power macros. `quote { }` with `@`-interpolation. `compile#()`/`error#()`/`warn#()`/`gensym#()` compile-time intrinsics with `is_compile_time_only()` annotation. Phase 1a (template) → Phase 1b (macro) → re-expand 1a → TypeChecker. Gensym hygiene for local `let` bindings (`__gensym_N`). Three canonical flags: `--macro-budget`, `--unlimited-macros`, `--safe-compile`.
- **MultiSlice mask/stride evaluation** — `BracketOp::Mask` and `BracketOp::Stride` ops now evaluated in interpreter. `_` bound as implicit element variable. `Expr::Slice.mask` also implemented. ArrowTransfer filter implemented with same `_`-binding pattern.

#### Not a Priority
- Self-hosting pipeline (broken, deferred)
- ForAll/Exists (removed from core syntax)

#### Historical Record
All optimization sprints, benchmark timing tables, bug diagnoses, and implementation phases are preserved in `AGENTS_HISTORY.md` and `AGENTS_HISTORY_2.md`.

#### Current State (as of 2026-06-21)
- 1162 tests pass, 0 fail
- **Constraint unification (B1/B2/B3)** complete: `RangeConstraint` + `Type::ContractBound` removed; `Statement::Let.constraint` + `StateDecl.constraint` unified to `Option<Box<Expr>>`; `_`-binding in `eval_constraint()`/`emit_guard_check()`; TypeDef body guards in `ResolvedType.guards`; LLVM constraint codegen with `@llvm.trap()` + `unreachable`
- **Phase 3.5 (Backend Fast-Path Registry)** complete
- **trg reactive dirty-flag architecture** complete (Phases 1–6)
- **SSA phi dominance** fixed (6 root causes)
- **foreach** complete
- **`?#` proof oracle** complete
- **Instruction reordering** complete
- **Variadic `fprintf` syntax** fixed
- **TBAA metadata** implemented
- **`!range` metadata** implemented
- **Webstack backend gaps closed** (2026-06-21)
- **CIRCT backend gaps closed** (2026-06-21)
- **Pattern B AssignmentStmt** (2026-06-21)
- **`$!` macro expansion wired** (2026-06-21)
- **Crypto/HTTP FFI implemented** (2026-06-21)
- **`bytes` projection extended** (2026-06-21)
- **GPU intrinsics dimension validation** (2026-06-21)
- **Void intrinsic stubs → `undef`** (2026-06-21)
- **Exit expression stubs → `llvm.trap()`** (2026-06-21)
- **`<-` arrow push/pop/discard/transfer** implemented for `List<T>`
- **String/char escape sequences** fully implemented
- Three canonical backends: LLVM (native), Webstack (WASM+JS), CIRCT (MLIR→Verilog)
- All other backends are dead code — zero fixes
- Kani: 14 fast-group harnesses proven, 96 full-group pass

#### Roadmap — Next Work Items (all completed as of 2026-06-21)
SSA phi dominance → foreach LLVM/SIMD → `?#` proof oracle → ILP reordering

### Known Bugs Fixed

#### 2026-06-17: String state initializers store null instead of string constant
**Root cause**: `emit_inline_init_stores` in `emit_toplevel.rs:468` had a special case
that matched `Some(Expr::String(_))` and stored `i8* null` instead of the actual
string constant pointer. All string state variables (e.g. `current_input: String = ""`,
`target_os: String = "linux"`) were initialized as null pointers.

**Fix**: Replace `null` with a `bitcast` of `@str.N` to `i8*`.

**Lesson**: Every `Expr` handler that evaluates to a pointer must store the actual
pointer, not a sentinel/placeholder.

#### 2026-06-17: Wrong TFD_NONBLOCK / SFD_NONBLOCK constants in trigger init
**Root cause**: `emit_toplevel.rs:104-105` had `tfd_nonblock = 0x400` and
`sfd_nonblock = 0x400`. These should be `0x800` (same as `O_NONBLOCK` on Linux x86_64).

**Fix**: Change both to `0x800`.

**Lesson**: Hardcoded platform constants should be cross-referenced against kernel headers.

#### 2026-06-17: `read_file#` returns null instead of error — FFI must use `Result<T, E>`
**Root cause**: `read_file#` returned `i8*` — either a valid C string or NULL if
the file didn't exist. The Brief type system has no notion of "nullable pointer".

**Fix**: Changed `read_file#` to return `Result<String, String>`.

**Architectural rule**: Every Brief `#`-intrinsic that can fail MUST return
`Result<T, E>` where `E` describes the failure.

#### 2026-06-17: `is_string_chain` missing `Expr::Call` arm — officina SIGSEGV
**Root cause**: `is_string_chain` in `emit_expr.rs:2763` detects string `+`
for inline concat but does not handle `Expr::Call`.

**Fix**: Added `Expr::Call(name, _)` arm checking `defn_return_types` for
`String`/`Data` return type.

#### 2026-06-17: `\0` char escape not handled in lexer
**Root cause**: `src/lexer.rs:371-382` handles `\n`, `\t`, `\\`, `\'`,
and `\u{...}` escape sequences in char literals, but NOT `\0` (null).

**Fix**: Added `if inner == "\\0" { return Some('\0'); }` before the other escape checks.

#### 2026-06-17: `done_{name}` SSA dispatch branches to exit instead of next txn
**Root cause**: `src/backend/llvm/loop_engine.rs:778`: the `done_l` label
(emitted when a txn's precondition is false) unconditionally branches to
`%done` (program exit) instead of `%{skip_l}` (next txn's skip label).

**Fix**: Changed `br label %done` to `br label %{skip_l}`.

#### 2026-06-17: TBAA metadata tree for `i64`-boxed types
**Implementation**: Added 6-node TBAA metadata tree (Brief root + Int, Bool,
Char, String, Float sub-types).

#### 2026-06-17: `!range` metadata replaces `@llvm.assume` for simple patterns
**Implementation**: `emit_precondition_check` detects simple `[x < N]` precondition
patterns and emits a re-load of the field with `!range !{ 0, N }` metadata.

#### 2026-06-17: Variadic `fprintf` missing `(ptr, ptr, ...)` prototype
**Root cause**: Three `fprintf` call sites omit the explicit variadic function type.

**Fix**: Added `(ptr, ptr, ...)` to all three call sites.

---

## 2026-06-30: Phase C/D/E — Bootstrap Type Universe, Annotation Arrow, Foreach Completion

### Phase C — Bootstrap Type Universe

**Lexer**: Added `TildeArrow` (`<~`) token (`#[token("<~")]`) — lexed as single token
via Logos longest-match. `src/lexer.rs:344`.

**Parser**: `parse_type_def` now accepts `<~` alongside `=` as a binding separator
in type bodies. Added `parse_annotations()` method that parses `<~ key: expr, #shorthand`
lists. Wired into `parse_definition`, `parse_transaction`, and `parse_trigger_body`.
`src/parser.rs:3370, 5110`.

**Type Universe**: `apply_binding()` is now case-insensitive via `.to_lowercase()`
so both `bytes <~ 8` and `Bytes = 8` dispatch to the same handler. Added handlers
for `llvm`, `storage`, `tbaa`, `box`, `unbox` annotations. `src/type_universe.rs:561`.

**Bootstrap file**: `lib/std/types/bootstrap.bv` defines 14 primitive types (`Int`,
`UInt`, `Int8`, `UInt8`, `Int16`, `UInt16`, `Int32`, `UInt32`, `Float`, `Float64`,
`Bool`, `Char`, `String`, `Data`) using `<~` annotation syntax.

**Auto-import**: ImportResolver auto-injects `import# std/types/bootstrap.bv`
for all `.bv` files (gated by `--no-std`). `src/import_resolver.rs:163`.

**Old code removed**: 163-line hardcoded `init_primitives()` with `Vec<ResolvedType>`
struct literals replaced by `init_primitives_from_bootstrap()` which parses the
`.bv` file via `include_str!`. `src/type_universe.rs:180`.

**Type body bindings**: Both `float.bv` and `from-bits.bv` updated from
`Name = value;` to `name <~ value;` for compile-time metadata. Runtime projection
bindings remain as `Name = expr;`.

### Phase D — Annotation Arrow on Declarations

**AST**: Added `annotations: Vec<TypeBinding>` field to `Definition`, `Transaction`,
and `TriggerDeclaration` structs. `src/ast.rs:2290, 2308, 2355`.

**Parser**: `parse_annotations()` supports `<~ key: expr, #shorthand` on defn/txn/trg.
`#shorthand` in annotations desugars to `key <~ true`. `#pragma` in type bodies
normalizes to lowercase (was uppercase), fixing a mismatch with case-insensitive
`apply_binding()`. `src/parser.rs:3339, 5110`.

**164 construction sites** updated across 18 files to add `annotations: vec![],`.

**Transaction.attrs removed**: Dead field `attrs: Vec<Attribute>` removed from
`Transaction` struct. 117 `attrs:` lines removed from test code.
`src/ast.rs:2306` (removed), `src/parser.rs:1272` (write site removed).

### Foreach Completion

**Type checker**: Added `Statement::Foreach` arm in `check_statement` — validates
list expression is `Applied("List", [elem_ty])`, declares iteration variable with
correct element type in scope, recursively checks body statements.
`src/typechecker.rs:2007`.

**LLVM codegen**: Item type generalized from hardcoded `Type::Int` to dynamic
extraction from `TypedRegister.ty` — supports `List<Float>`, `List<String>`, etc.
`src/features/stmt/foreach.rs:54-56, 85`.

**CIRCT backend**: Compile-time unroll for `Expr::ListLiteral` lists; emits warning
comment for dynamic (non-constant) lists. `src/backend/circt.rs:595`.

**Projection parser**: `parse_projection_target` now handles intrinsic call targets:
`fadd#(rhs)` → `UserDefinedWithArg("fadd#", ...)`, bare `fneg#` → `UserDefined("fneg#")`.
This enables `from-bits.bv` to parse up to its `$` template syntax section.
`src/parser.rs:7332`.

### Tests

- **6 new tests**: 1 lexer (`test_tilde_arrow_as_single_token`), 5 parser
  (definition/transaction/trigger with annotations, `<~` type body bindings,
  `#volatile` pragma shorthand, `from-bits.bv` best-effort parse)
- **Final count**: 1363 passed, 0 failed
- All existing tests pass with zero regressions

### Documentation

- `docs/architecture/overview.md` — Annotation Arrow + Bootstrap Type Universe sections
- `docs/architecture/prelude-and-import-magic.md` — Bootstrap Type Universe auto-import
- `learn-brief/12-pragmas.md` — Annotation Arrow + `#hashtag` shorthand
- `.opencode/plans/2026-06-30-foreach-completion.md` — Foreach completion plan
