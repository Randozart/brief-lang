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
gaps in the language AND the library.

### Correct Approach
- Keep contract `[product > 0]` 
- Fix code: make buttons call product-specific transactions like `add_laptop`, `add_keyboard`
- If interpreter raises `UndefinedForeignFunction("is_digit")`, add `import char from "std/char.bv"` to the calling .bv file
- If import resolver can't find a standard library file, fix the search path, not the interpreter

## Language Architecture

Brief is a **general-purpose programming language**. The interpreter proves this — it already supports the full expression language including lists, strings, structs, enums, pattern matching, hash maps, and FFI. The standard library (`lib/std/`) has 26 modules covering strings, collections, math, I/O, JSON, HTTP, encoding, shared memory, and more.

### How Brief Works (Correct Model)

Brief's computational primitive is the **reactive transaction** (`rct txn`). A transaction has:
- A **precondition** (guard): `[x > 0 && y < N]`
- A **postcondition** (contract): `[x == N]`
- A **body**: `{ &x = x + 1; &y = y * 2; }`

The compiler's job is to analyze the transaction graph and emit code for the most efficient execution path. This is NOT a niche reactive DSL — it IS how Brief expresses computation. Loops are transactions with bounded convergence (`[count < N][count == N]`). Recursion is a transaction chain with proved termination. Every optimization (purity folding, dead-field elimination, SROA, SLP vectorization) applies because the compiler has enough information from contracts to prove correctness.

### Misconceptions to Avoid

| Wrong | Correct |
|-------|---------|
| "Brief is a reactive state machine DSL" | Brief is a general-purpose language. Transactions are the computational primitive — they ARE loops, iteration, and recursion. |
| "Brief has no arrays/strings/collections" | The interpreter supports `List<T>`, `String`, `HashMap<K,V>`, `HashSet<T>`, `Stack<T>`, `Queue<T>`, `StringBuilder`. The stdlib has 26 modules including `collections.bv`, `string.bv` (876 lines), `char.bv`, `json.bv`, etc. |
| "Brief can't do tree/heap benchmarks" | The interpreter supports recursive enum types (e.g., `enum Tree { Node(Tree, Tree), Leaf }`), struct instances, field access, and match expressions. |
| "Brief needs malloc/FFI for buffers" | No. The compiler proves bounds from contracts at compile time and allocates accordingly. The programmer writes proofs, the compiler handles memory. |
| "The interpreter has known gaps" | The "known gaps" in older AGENTS.md were stale. All listed Expr/Statement variants are implemented in the interpreter. See "Interpreter Completeness" below. |
| "The LLVM backend is the language" | The interpreter IS the reference implementation. The LLVM backend is an optimization pass over it. If the interpreter runs it, the backend should eventually compile it. |

### Two-Layer Architecture

1. **Interpreter** (`src/interpreter.rs`) — the reference implementation. 2327 lines. Validates EVERYTHING before any codegen work. If something isn't in the interpreter, it doesn't belong in codegen.
2. **LLVM Backend** (`src/backend/llvm.rs`) — 6024 lines. Compiles state/transactions/expressions to LLVM IR. Applies optimizations (purity folding, SROA, SLP, dead-field elimination, etc.). Must never weaken existing optimization paths for new features.

## Interpreter Completeness (Reference Implementation)

The interpreter at `src/interpreter.rs` is the **full reference implementation**. Here is the exact status of every Expr/Statement variant:

### Expressions — Fully Implemented
| Expr | Line | Status |
|------|------|--------|
| Integer, Float, String, Char, Bool, Term, Identifier, OwnedRef, PriorState | 727-751 | ✅ |
| Add, Sub, Mul, Div, Mod | 752-816 | ✅ |
| Eq, Ne, Lt, Le, Gt, Ge | 817-882 | ✅ |
| Or, And, Not | 883-898 | ✅ |
| Neg, BitNot, BitAnd, BitOr, BitXor, Shl, Shr | 899-938 | ✅ |
| Call, ListLiteral, ListIndex, ListLen | 939-1647 | ✅ |
| FieldAccess | 1648 | ✅ — `Value::Instance` field lookup |
| StructInstance | 1662 | ✅ — creates `Value::Instance` |
| ObjectLiteral | 1672 | ✅ — creates `Value::Instance` with typename "ObjectLiteral" |
| PatternMatch | 1682 | ✅ — enum variant matching with field binding |
| Concat | 1716 | ✅ — string/list concatenation |
| Slice | 1727 | ✅ — full start/end/stride/mask support for List and String |
| Block | 1806 | ✅ — saves/restores state, executes stmts, returns last expr |
| Tuple | 1815 | ✅ — evaluates elements into `Value::List` |
| TupleDestructure | 1822 | ✅ — destructures `Value::List` into state bindings |
| MultiSlice | 1848 | ✅ — delegates to Slice or Index per coordinate |
| Cast | 1896 | ✅ — passes through (type erasure) |
| Match | 1897 | ✅ — full pattern matching with Wildcard and Variant arms |
| ForAll | 1838 | ⚠️ stub — always returns `Bool(true)`. Quantification not yet implemented. |
| Exists | 1839 | ⚠️ partial — checks if list is non-empty. Full quantification not yet implemented. |

### Statements — Fully Implemented
| Statement | Line | Status |
|-----------|------|--------|
| Assignment (list index writes included) | 498-531 | ✅ |
| Let | 536 | ✅ |
| InlineAsm | 542 | ✅ |
| Expression | 543 | ✅ |
| Term | 546 | ✅ |
| Escape | 554 | ✅ |
| Guarded | 558 | ✅ |
| **Unification** | 579 | ✅ — matches enum variant, binds fields, executes body |
| LocalTrigger | 620 | ✅ |
| Alka, OnExit | 629 | ✅ |

### Top-Level — Fully Handled
| TopLevel | Status |
|----------|--------|
| Transactions (including reactive), StateDecl, Trigger, Constant, Import, LinkDependency | ✅ |
| ForeignBinding (dynamic .so/.dylib loading) | ✅ |
| Struct (instance creation, field access) | ✅ |
| Enum (constructor calls, variant dispatch, unification) | ✅ |
| Definition (defn) — including calls | ✅ |

### Bug: Recursive defn calls
`defn` functions CAN call themselves (the definition is in scope during execution), but there is **no recursion guard or stack depth limit**. A deeply-recursive `defn` will stack-overflow the Rust interpreter. The self-hosted compiler (`lib/compiler/call_graph.bv:138`) has `collect_call_names` calling itself, which would overflow on non-trivial inputs. This is a correctness gap — recursive defn should be bounded or rewritten.

**Conclusion**: The interpreter supports Brief as a general-purpose language. ForAll/Exists quantification and recursive-defn safety are the only known gaps in the reference implementation.

## LLVM Backend Gaps (Codegen Completeness)

The LLVM backend at `src/backend/llvm.rs` (6024 lines) lags behind the interpreter. Here is the exact status:

### Expressions — Fully Emitted
| Expr | Line | Status |
|------|------|--------|
| Arithmetic, comparisons, logic, bitwise | 2500-2558 | ✅ |
| Cast | 2678 | ✅ — type conversion via emit_cast_convert |
| Block | 2685 | ✅ — inline stmts, return last |
| Match | 2691 | ✅ — switch dispatch with phi merge at merge block |
| PatternMatch | 2736 | ✅ — discriminant extraction + icmp eq |
| Call | 2561 | ✅ — FFI marshaling (i64/float/i8*/i32), defn calls, enum constructors (stack alloca + GEP + store + ptrtoint) |
| ListLiteral | 2638 | ✅ — alloca + GEP + store per element + ptrtoint |
| ListIndex | 2650 | ✅ — inttoptr + GEP |
| Exists | 2747 | ✅ — icmp ne 0 |

### Expressions — Stub (Returns 0 or Degraded)
| Expr | Line | What's Missing |
|------|------|----------------|
| **ListLen** | 2657 | Returns `0` always. Needs: track list length (store it alongside the pointer, e.g., as a 2-slot pair: [ptr, len]). |
| **Slice** | 2658 | Only handles `start` offset into the same buffer. Missing: `end` bound, `stride`, `mask`. Needs: allocate new buffer, compute length, copy elements. |
| **MultiSlice** | 2667 | Returns base pointer unchanged. Missing: coordinate-based indexing, new buffer construction. |
| **Tuple** | 2672 | Returns `0`. Missing: allocate struct or flatten to registers. Blocked by: no LLVM struct type generation for user types. |
| **TupleDestructure** | 2673 | Passes inner value through. Missing: extract elements from tuple representation. |
| **StructInstance** | 2674 | Returns `0`. Missing: allocate struct in %State or on stack, GEP + store fields. |
| **ObjectLiteral** | 2675 | Returns `0`. Same gap as StructInstance. |
| **FieldAccess** | 2676 | Returns object pointer as-is. Missing: GEP into struct at known field offset. |
| **ForAll** | 2746 | Returns `1` always. Missing: bounded loop over value range. Matches interpreter stub. |

### Top-Level — Silently Skipped
| TopLevel | Line | Impact |
|----------|------|--------|
| **Struct** | 427 (`_ => {}`) | No LLVM struct type generated. `StructInstance` and `FieldAccess` stubs above are the symptoms. |
| **Enum** | 427 (`_ => {}`) | No tagged union layout emitted. However, enum `Call` constructors work (line 2616) via ad-hoc stack alloca + discriminant prefix. Move this into proper `TopLevel::Enum` codegen. |
| Signature, Import, LinkDependency | 427 | Correctly skipped — these are frontend-only or consumed by the compiler driver. |
| ResourceDecl, RStruct, RenderBlock, Stylesheet, SvgComponent | 427 | Correctly skipped — .rbv frontend concepts. |

### Collection Method Calls
Collection method dispatch (`list_append`, `hashmap_insert`, `string_builder_append`, etc.) in the interpreter uses method-name string matching in `Expr::Call`. The LLVM backend handles `Expr::Call` via FFI marshal/decode/defn-call/constructor paths (line 2561-2636). Some stdlib collection methods may hit the `UndefinedForeignFunction` path if the function name isn't in `frgn_map` or `defn_params`. Verify per benchmark.

## Key Philosophy for Backend Work

### Never Weaken Optimizations for New Features
Existing optimization paths (purity folding, dead-field elimination, SROA, SLP, switch dispatch, thread pool) MUST NOT regress when adding new codegen. Struct/enum/collection codegen is additive — new match arms that don't touch the existing fold/precompute/dispatch paths.

### The Interpreter is the Source of Truth
If the interpreter produces the correct result for a program, the LLVM backend is expected to eventually compile that program correctly. If there's a conflict between "what's easy to codegen" and "what the interpreter does," fix the codegen. Never change the interpreter to match a weak codegen path.

### Contracts Enable Optimizations — Don't Skip Them
The more contract information the LLVM backend has, the more aggressively it can optimize. Struct and collection codegen should preserve contract information (field types, bounds, pre/post conditions) so the optimizer can reason about them. A `List<T>` with a known fixed bound from a precondition can be stack-allocated at compile time.

## For OpenCode

This project uses OpenCode. When making changes:
1. Read CLAUDE.md for full context
2. Follow Contract-First Philosophy
3. Never weaken contracts - fix code instead
4. Test with `cargo test --lib` before committing
5. Document bugs and root causes in BUGS.md
6. Never add Rust built-ins for things the standard library should provide
7. **No prototyping — build clean**: Every optimization is a first-class pass in its proper module (`src/analysis/` for analysis, `src/backend/` for codegen). Never inline new analysis into codegen as a shortcut.
8. **Never weaken C benchmarks**: Every asymmetry between Brief and C is a signal of a missing Brief optimization. Never hobble C with `volatile`, unused `break;` cases, or artificial liveness hacks to make Brief look better. Fix Brief to match or beat C's optimization.
9. **The interpreter IS the reference**: If the interpreter runs it correctly, the backend should eventually compile it. If the interpreter doesn't support something, add it to the interpreter first, then add codegen.
10. **Benchmarks on our own terms**: Brief benchmarks compare end-to-end results (Input X → Output Y). The compiler chooses the optimal execution path. Adding features for benchmarks is fine IF they add value to the language. Never add features solely to run benchmarks.

## Self-Hosting Pipeline

The Brief-in-Brief compiler lives in `lib/compiler/`. The Rust interpreter runs it via:
```
brief-compiler selfhost <file.bv>
```

**Do NOT add as built-ins**: `is_digit`, `is_alpha`, `is_alphanumeric`, `is_upper`, `is_lower`, `is_space`, `char_to_string`, `None`, `Some`, `Ok`, `Err`. These are in `lib/std/` and should be imported.

## Anchored Summary

**Current**: 400 tests pass. Phases 1-4 complete plus Phase 5 (DBVS import pipeline), N3 (PGO), and N2 (equality saturation). Brief wins or ties on 8 of 9 benchmarks. print_loop at 1.63× of C.

### Done — Eliminate Redundant Pragmas (Steps 1-6, complete)
- **Step 1**: Auto-select `Parallel` dispatch when all reactive txns are conflict-free (no `#pragma dispatch(parallel)` needed)
- **Step 2**: `@ link` triggers default to wake (no `#wake` needed)
- **Step 3**: Wake+enum mutual exclusion lifted — enum dispatch enters hybrid wake mode with `@__rt_wait()` loop
- **Step 4**: `suggest_async_promotion()` lint — A001 warning for conflict-free `rct` txns that could be async
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
- **Cleanup**: `*.opt.ll` added to `.gitignore`.
