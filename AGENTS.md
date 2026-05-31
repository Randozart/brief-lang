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

**Current**: All optimization phases complete. 347 tests pass. Thread pool async dispatch done. Benchmarks next.

### Done — Optimization Framework (complete)
- 7 LLVM backend bug fixes (cast no-op, zero-init, float hex, mustprogress UB, memory scoping, #volatile, negative float)
- `transition_graph.rs` — bounded pre detection, increment patterns, pure/impure body
- `emit_folded_main()` / `emit_folded_pure_counter()` — while-loop collapse for bounded-counter rxns (Path 2)
- IIR filter benchmark: Brief 0.15s vs C 0.23s (1.53× faster)
- `TopLevel::Constant` in LLVM backend (`@name = constant` globals, const identifier resolution)
- Convergence verification (`check_convergence`) — pre validation, relational post-ops, overshoot detection
- **RegionAnalyzer** (`src/analysis/region.rs`): VarClass (Pure/Bounded/Opaque), Interval, dep graph, BFS prop, region detection, value-set estimation, 9 unit tests
- **Phase 2**: Value-set enumeration, `emit_enum_main` with switch dispatch for enumerable triggers (Path 4)
- **Phase 3**: `--optimize-budget <N>`, `--optimize-report`, `--optimize-size <bytes>` CLI flags
- **Phase 4.1-4.3**: Linear transaction chain detection, expression substitution, `emit_fused_composed()`, pure counter elimination
- **Phase A**: Wake-trigger/enum dispatch soundness fix (was `has_wake → enumerable=None` gate; now enum+wake hybrid)
- **Phase B**: Compile-time complete evaluation — `is_fully_precomputable`, `collect_final_values`, `emit_precomputed_main` (Path 3)
- **Phase C**: IIR filter benchmark regression test (guard against optimization regressions)

### Done — Eliminate Redundant Pragmas (Steps 1-5, complete)
- **Step 1**: Auto-select `Parallel` dispatch when all reactive txns are conflict-free (no `#pragma dispatch(parallel)` needed)
- **Step 2**: `@ link` triggers default to wake (no `#wake` needed)
- **Step 3**: Wake+enum mutual exclusion lifted — enum dispatch enters hybrid wake mode with `@__rt_wait()` loop
- **Step 4**: `suggest_async_promotion()` lint — A001 warning for conflict-free `rct` txns that could be async
- **Step 5**: Thread pool auto-inference + concurrent async dispatch (Path 5)

### Step 5 Details — Thread Pool + Auto Async/Enum Inference
- **Phase 5a**: Thread pool primitives in `runtime/brief_rt.c` — portable barrier (mutex+cond+counter, works on macOS), `brief_thread_pool_init/release/wait/shutdown`, gated behind `#if defined(BRIEF_THREAD_POOL)`
- **Phase 5b**: Builtin declares (`brief_thread_pool_init`, `brief_barrier_release`, `brief_barrier_wait`)
- **Phase 5c**: Auto-categorize txns in `generate()` — enum candidates (trigger-gated), async candidates (conflict-free pairwise), enum beats async
- **Phase 5d**: `emit_async_body` — per-txn worker functions (`pre→fire` pattern)
- **Phase 5e**: Async phase injection in `emit_main` and `emit_enum_main` — thread pool init at entry, `barrier_release → reactor_tick → barrier_wait`
- **Phase 5f**: `main.rs` link step — detects `@llvm.thread_pool`, adds `-DBRIEF_THREAD_POOL -lpthread`
- **4 new tests** (async body emission, thread pool metadata, barrier calls in main, no thread pool without async txns)
- **No atomics on state fields** — the proof engine guarantees disjoint field access per txn group, so plain loads/stores are data-race-free (C11 5.1.2.4p25)

### Optimization Path Summary
| Path | What | Status |
|------|------|--------|
| Path 2 | Folded while-loop (counter convergence) | Done — IIR filter |
| Path 3 | Compile-time precompute (state space ≤ budget) | Done — `emit_precomputed_main` |
| Path 4 | Enum switch-dispatch (bounded trigger values) | Done — wake+enum hybrid |
| Path 5 | Thread pool async dispatch (conflict-free txns) | Done — auto-inference |

### Next Up
- 3 new benchmarks (ring buffer / async counters / precompute sum) + IIR filter regression
- Benchmark infrastructure: monotonic clock FFI, extended `build_and_bench.sh` with `--link-rt` support

## Key Plan Documents
- **`plans/2026-06-01-optimization-framework.md`** — Implementation plan for optimization phases
- **`plans/2026-06-01-optimization-completion.md`** — Phases A/B/C (wake fix, precompute, regression)
- **`plans/2026-06-01-eliminate-redundant-pragmas.md`** — Steps 1-5 (auto-wake, auto-parallel, async inference, thread pool)
- **`plans/2026-06-01-thread-pool-async-dispatch.md`** — Step 5 design (thread pool, barrier, auto-inference)
- **`docs/design/determinism-and-optimization-frontier.md`** — Conceptual optimization architecture
- **`docs/design/optimization-cost-model.md`** — Full cost model specification