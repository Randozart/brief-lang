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

**Current**: Phases 3-4.3 done. 328 tests pass. Cost model, chain composition, and branching implemented.

### Done
- 7 LLVM backend bug fixes (cast no-op, zero-init, float hex, mustprogress UB, memory scoping, #volatile, negative float)
- `transition_graph.rs` — bounded pre detection, increment patterns, pure/impure body
- `emit_folded_main()` / `emit_folded_pure_counter()` — while-loop collapse for bounded-counter rxns
- IIR filter benchmark: Brief 0.15s vs C 0.23s (1.53× faster)
- `TopLevel::Constant` in LLVM backend (`@name = constant` globals, const identifier resolution)
- Convergence verification (`check_convergence`) — pre validation, relational post-ops, overshoot detection
- Architecture: convergence skip moved to `ProofEngine::verify_contracts`
- **RegionAnalyzer** (`src/analysis/region.rs`): VarClass (Pure/Bounded/Opaque), Interval, dep graph, BFS prop, region detection, value-set estimation, 9 unit tests
- **Phase 2**: `region_analyzer` in `AnalysisResults`, `emit_folded_loop` helper refactor, `emit_enum_main` with switch dispatch for enumerable triggers (budget=256 combos)
- **Phase 3**: `--optimize-budget <N>`, `--optimize-report`, `--optimize-size <bytes>` CLI flags. Budget wired through to enum check. Report shows trigger value sets, combinations, budget fit, size estimation.
- **Phase 4.1**: Linear transaction chain detection — txn_reads/txn_writes per txn, A→B→C traversal, maximal chain dedup. Shown in `--optimize-report`.
- **Phase 4.2**: Expression substitution engine (`substitute_var`/`substitute_expr` for all 46 Expr variants). Chain composition with slack-link variable forwarding. Composability constraint validation (single upstream producer, shared convergence contract, no FFI). 12 new unit tests.
- **Phase 4.3**: `emit_fused_composed()` — emits composed chain bodies as LLVM functions. `emit_enum_main()` extended with per-trigger-value composed function dispatch. Trigger branching produces one fused function per concretized trigger value (Bool → `@txn_fused_txn_trg_0` / `_trg_1`). Partial composability tracks `all_internal` flag for store elimination eligibility.
- **Report**: Extended with optimization priority ranking table (RID, txns, class, weight, iter, cost, score, chain/GPU tags), budget allocation plan (allocated/skipped regions), composed chain details (trigger values, all-internal status).
- **Types**: `ComplexityClass` (Trivial/Light/Medium/Heavy/Unbounded), `RegionScore`, `BudgetPlan`, `ComposedChain` with `trigger_values: Option<Vec<(String, i64)>>` and `all_internal: bool`.
- **Analysis pipeline**: 10-phase `analyze()` — register→depgraph→seed→propagate→regions→value_sets→chains→iter_bounds→region_scores→compose; `build_budget_plan()` called separately with budget parameter.
- **Design docs**: `determinism-and-optimization-frontier.md`, `optimization-cost-model.md`
- **Plan doc**: `plans/2026-06-01-optimization-framework.md`

### Next Up
- Phase 5: Compile-time complete evaluation (if state space ≤ budget, precompute all results)
- Partial composability optimization: eliminate stores for all-internal chain variables
- GPU backend exploration (LLVM AMDGPU/NVPTX triple toggle)
- ~325 total tests currently passing

## Key Design Documents

- **`docs/design/determinism-and-optimization-frontier.md`** — Conceptual architecture for Brief's optimization framework: determinism analysis, atomic reactive regions, value-set enumeration, budget-controlled compile-time optimization.
- **`docs/design/optimization-cost-model.md`** — Full specification for the optimization cost model: `ComplexityClass`, `RegionScore`, `BudgetPlan`, `ComposedChain` types; complexity estimation, region scoring with ROI metric, greedy budget allocation, chain composition with trigger branching, fused emission, GPU eligibility analysis, report format. Target: O(n) → O(1) reduction on every provable axis.
- **`plans/2026-06-01-optimization-framework.md`** — Implementation plan for building the framework, phased from tactical convergence-proof fixes through value-set enumeration and report system.