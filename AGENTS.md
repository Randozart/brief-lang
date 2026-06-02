# Brief Compiler - Agent Guidelines

See CLAUDE.md for complete documentation. This file ensures OpenCode picks up the same guidelines.

## Quick Reference

### Commands
- **Build**: `cargo build`
- **Test**: `cargo test --lib`
- **Test backend registry**: `cargo test --lib -- backend::tests`
- **Compile RBV**: `./target/release/brief-compiler rbv <file.rbv>`
- **Selfhost**: `cargo run --bin brief-compiler -- selfhost <file.bv>`

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

### Correct Approach
- Keep contract `[product > 0]` 
- Fix code: make buttons call product-specific transactions like `add_laptop`, `add_keyboard`
- If interpreter raises `UndefinedForeignFunction("is_digit")`, add `import char from "std/char.bv"` to the calling .bv file
- If import resolver can't find a standard library file, fix the search path, not the interpreter

## For OpenCode

This project uses OpenCode. When making changes:
1. Read CLAUDE.md for full context
2. Follow Contract-First Philosophy
3. Never weaken contracts - fix code instead
4. Test with `cargo test --lib` before committing
5. Document bugs and root causes in BUGS.md
6. Never add Rust built-ins for things the standard library should provide

## Self-Hosting Pipeline

The Brief-in-Brief compiler lives in `lib/compiler/`. The Rust interpreter runs it via:
```
brief-compiler selfhost <file.bv>
```

**Known gaps in interpreter** (add these legitimately, not as magic):
- `Expr::Block`, `Expr::Tuple`, `Expr::TupleDestructure` — properly implemented in eval_expr
- `Expr::ForAll`, `Expr::Exists`, `Expr::MultiSlice` — properly implemented in eval_expr
- `Statement::Unification` — properly implemented (looks up state, matches variant, executes block)

**Do NOT add as built-ins**: `is_digit`, `is_alpha`, `is_alphanumeric`, `is_upper`, `is_lower`, `is_space`, `char_to_string`, `None`, `Some`, `Ok`, `Err`. These are in `lib/std/` and should be imported.

## Anchored Summary

**Current**: Calibration baseline complete. 7 benchmarks with 4-decimal precision timing. const_heavy now shows non-zero (0.0455s Brief vs 0.0360s C, 1.26× gap after SCEV-breaking `count/100` term). float_math shows 9.79× gap (0.4231s vs 0.0432s) — biggest optimization target. sparse_dispatch shows 32.9× gap but largely O(1) at machine level (SCEV-optimized). 368 tests pass.

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

### Benchmarks (2026-06-02 sprint final — all phases, C with `-ffast-math`, 50M iterations)
| Benchmark | Path | Brief | C | Ratio |
|-----------|------|-------|---|-------|
| iir_filter | **Dead-field elim + pure counter** | **0.000s** | 0.082s | **Brief wins** |
| precompute_sum | Compile-time precomputation | 0.003s | 0.002s | ~tie |
| ring_buffer | Enum O(1) pure-counter | 0.003s | 0.003s | ~tie |
| async_counters | Thread pool O(1) pure-counter | 0.004s | 0.007s | **Brief wins** |
| float_math | **alloca+SROA + fast-math + -O3** | **0.011s** | 0.006s | ~tie (startup) |
| float_math_nonzero | alloca+SROA + fast-math + -O3 + AVX | **0.371s** | 0.165s | **2.25×** |
| sparse_dispatch | Call-chain dispatch (SCEV-opt) | 0.090s | 0.007s | startup |
| const_heavy | Integer arithmetic (sdiv) | 0.002s | 0.061s | **Brief 27× faster** |

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
- **Phase B (typed SSA)**: Refactor emit_expr to return typed registers. Still worth doing for correctness (eliminates `is_float_expr` guess) and for programs where SROA doesn't fully decompose. But don't expect to close float_math_nonzero gap — SROA+opt already eliminates boxing.
- **Phase D (pointer provenance)**: Preserve `i8*` types for strings (no ptrtoint/inttoptr).
- **Phase E (fixes)**: Commutativity pattern fix in extract_trigger_keys, fastcc, per-function SLP guard.
- **Key finding**: Remaining float_math_nonzero 2.32× gap is from instruction scheduling & pipeline effects of phi structure vs C's local-variable register allocation. Both emit ~17 native float ops per iteration with `fast` flags. No boxing, no shuffles. The gap may be intrinsic.
- See `plans/2026-06-02-llvm-backend-optimization-phases.md` for full detail.

## Key Plan Documents
- **`plans/2026-06-01-optimization-framework.md`** — Implementation plan for optimization phases
- **`plans/2026-06-01-optimization-completion.md`** — Phases A/B/C (wake fix, precompute, regression)
- **`plans/2026-06-01-eliminate-redundant-pragmas.md`** — Steps 1-5 (auto-wake, auto-parallel, async inference, thread pool)
- **`plans/2026-06-01-thread-pool-async-dispatch.md`** — Step 5 design (thread pool, barrier, auto-inference)
- **`plans/2026-05-31-eliminate-io-registry-link-deps.md`** — Step 6 design (eliminate `io_registry.rs` with `import "link/"`)
- **`plans/2026-05-31-benchmarks-plan.md`** — Benchmarks plan (3 new + regression)
- **`docs/design/determinism-and-optimization-frontier.md`** — Conceptual optimization architecture
- **`docs/design/optimization-cost-model.md`** — Full cost model specification
- **`plans/2026-06-01-exit-safety-warnings.md`** — Implementation plan for exit diagnostics (unknown identifier error, one-shot warning, no-exit-path warning)
- **`plans/2026-06-01-pure-counter-enum-dispatch.md`** — Pure-counter fold elimination for enum/async dispatch (store total instead of while-loop)
- **`plans/2026-06-01-fair-c-benchmarks-fuzzing.md`** — Phase 1: fair C benchmarks + Phase 2: input fuzzing (compile-time and runtime modes)
- **`plans/2026-06-01-dead-field-elimination.md`** — Dead-field elimination: liveness analysis for effectively-pure body detection
- **`plans/2026-06-02-hardware-aware-slp-hazard-analyzer.md`** — SLP hazard analyzer: deterministic peak register demand formula
- **`plans/2026-06-02-slp-hazard-loopholes.md`** — Three loopholes audit: local var blindspot, dual-var constraint, missed constants
- **`plans/2026-06-02-calibration-baseline-and-dispatch-fix.md`** — Calibration baseline, 4-decimal precision, SCEV break, alwaysinline audit

### Calibration Baseline & SCEV Break (2026-06-02)
- **Problem**: const_heavy showed 0.00s (SCEV eliminated linear recurrence). Baseline benchmarks ran 0.00s for 4/7 on O(1) pure-counter paths.
- **Fix**: `+ count / 100` term in const_heavy body breaks SCEV (`sdiv` is non-linear). Both .bv and _c.c updated.
- **Precision**: `build_and_bench.sh` now uses `date +%s.%N` + `bc` + `LC_NUMERIC=C` for 0.0000s precision.
- **alwaysinline audit**: Data refutes the "alwaysinline bloat" hypothesis. `opt -O2` + SCEV handles the phi/select cascade — sparse_dispatch runs 0.0559s at 50M (not pathological). Plan A (noinline guard) skipped.
- **Files**: `benchmarks/const_heavy.bv`, `benchmarks/const_heavy_c.c`, `build_and_bench.sh`
- **Plan**: `plans/2026-06-02-calibration-baseline-and-dispatch-fix.md`

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