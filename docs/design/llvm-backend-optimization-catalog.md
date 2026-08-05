# LLVM Backend — Optimization Catalog

**Date**: 2026-05-31
**Tests**: 343 passing

This document catalogues every optimization path in the Briv LLVM backend, the conditions that trigger each, and what each path achieves in terms of runtime performance.

---

## Decision Cascade

The `generate()` method applies optimizations in strict priority order:

```
FOLDED  →  PRECOMPUTED  →  ENUM  →  STANDARD
```

Each path replaces `@main` (and optionally `@reactor_tick`) with a specialized emission. Lower-ranked paths fall through.

---

## Path 1: Pure Counter Elimination

**Function**: `emit_folded_pure_counter`

**Triggers** (all must hold):
- Exactly one transaction in the program
- No triggers declared
- Convergence contract with `counter < bound` precondition
- Body has a `counter += delta` increment pattern
- Body is *pure* — no state writes beyond the counter, no FFI, no `term`

**What it emits**: A single `@main` that calls `@init_state()` then stores the final counter value directly into the state field. One store instruction. No loop, no transaction call.

**Performance**: O(1). The entire N-iteration convergence loop is eliminated at compile time.

**Analysis**: `src/analysis/transition_graph.rs` — `BoundedPre`, `IncrementInfo`, `is_pure_body`

---

## Path 2: Folded While-Loop

**Function**: `emit_folded_main` + `emit_folded_loop`

**Triggers**: Same as Path 1 except the body is *impure* (has state writes).

**What it emits**: A canonical `while (counter < bound) { body(); }` loop. The counter is loaded from the state field, the bound from a state field or constant global, and the body is called directly — no precondition re-evaluation, no trigger sampling, no dispatch-chain branching.

**Performance**: O(N) iterations with eliminated per-iteration dispatch overhead. LLVM recognizes the counted loop and applies loop unrolling, GVN, and dead-code elimination across iterations.

**Analysis**: `src/analysis/transition_graph.rs`

---

## Path 3: Compile-Time Complete Evaluation

**Function**: `emit_precomputed_main`

**Triggers**:
- All composed chains exist and are `all_internal` (no trigger-dependent values, no FFI, no external reads/writes)
- No trigger values in any composed chain (no runtime switch needed)
- No `Unbounded` complexity regions
- Total chain count ≤ budget

**What it emits**: A `@main` that calls `@init_state()` then stores final values into every state field via direct `getelementptr` + `store i64`. Evaluates all composed chain bodies at compile time using a symbolic evaluator (`eval_expr_simple`, `eval_stmt`) supporting 22 expression variants (arithmetic, boolean, bitwise, comparisons, negation, shift, cast).

**Performance**: O(number of state variables). Zero runtime iterations. The entire program resolves to a handful of LLVM store instructions.

**Analysis**: `src/analysis/region.rs` — `is_fully_precomputable()`, `collect_final_values()`, `eval_stmt()`, `eval_expr_simple()`

---

## Path 4: Enum Dispatch (Switch-Based)

**Function**: `emit_enum_main`

**Triggers**:
- At least one trigger exists
- No wake triggers (wake reactors need infinite loop with `@__rt_wait()`)
- Every trigger variable has a known compile-time value-set size
- Total product of all trigger value sets ≤ `--optimize-budget` (default 256)

**What it emits**: A `@main` that samples all triggers once via `load volatile`, then `switch i8` dispatches to per-value folded loops. Each case calls a concretized fused function or stores the final counter value directly for all-internal chains. A `_residual` case calls `@reactor_tick()` for out-of-range trigger values.

For composed chains with trigger branching, each trigger value gets its own `@txn_fused_<chain>_trg_<val>` function with the trigger identifier substituted to a concrete integer. All-internal chains skip fused function emission entirely; per-case switch arms store the final counter value directly.

**Performance**: O(1) per trigger value at runtime — trigger sampled once, one path executed. Code size grows as O(product of trigger value-set sizes).

**Analysis**: `src/analysis/region.rs` — `value_set_size_of()`, `estimate_value_sets()`, `composed_chains`, `linear_chains`

---

## Path 5: Standard Reactor (Fallback)

**Function**: `emit_main` + `emit_reactor` / `emit_parallel_reactor`

**Triggers**: Always reached when no optimization above applies. Also used as residual fallback in Path 4.

**What it emits**: Standard polling reactor — `define void @reactor_tick()` polls all triggers, checks preconditions, calls transaction bodies. `@main` is an infinite loop calling `@reactor_tick()`. With wake triggers, adds `@__rt_init()` and `@__rt_wait()`.

**Performance**: O(transactions) per tick. No optimization applied.

---

## Cross-Cutting Optimizations

These apply within any path above.

### Fused Transaction Pairs

**What**: Pairs of transactions (A, B) where A's writes feed B's reads are combined into `@A_B_fused`, eliminating the intermediate precondition check and state re-load.

**Guards**: No overlapping writes, no async txns, B's precondition must not reference triggers.

**Analysis**: `detect_fusable_pairs()` in `src/backend/mod.rs`

### Composed Linear Chains

**What**: Linear chains (A → B → C) are composed by expression substitution — intermediate state reads replaced with upstream write expressions. Produces a single fused body. All-internal chains skip fused function emission; counter values stored directly.

**Analysis**: `src/analysis/region.rs` — `detect_linear_chains()`, `compose_chains()`, `chain_is_composable()`, `substitute_var()`/`substitute_expr()`, `find_counter_var()`

### Constant Globals

**What**: `const` declarations emit as `@name = constant <ty> <value>`. Identifier resolution loads from the global, letting LLVM constant-propagation fold values.

### Alwaysinline for Acyclic Call Graphs

**What**: When the call graph has no cycles, transaction functions are tagged `alwaysinline`. LLVM inlines bodies into the dispatch loop, eliminating call overhead.

**Analysis**: `src/analysis/call_graph.rs`

### Guard-to-Select Optimization

**What**: When a guarded statement has exactly one assignment to a state field, emits `select i1 <cond>, <true_val>, <existing_val>` + store instead of branch→then→merge, eliminating branch overhead for conditional writes.

### Precondition `@llvm.assume` Injection

**What**: After emitting precondition check branches (with `unreachable` in the false path), issues `call void @llvm.assume(i1 <cond>)`. LLVM uses this to eliminate dead paths, infer range constraints, and simplify downstream expressions.

### `!range` Metadata on State Loads

**What**: Preconditions of the form `var < constant` are extracted as (lo, hi) ranges and attached as `!range` metadata on state field loads. LLVM uses this for value-range analysis across load sites.

---

## Analysis Pipeline

The `analyze_program()` function in `src/backend/mod.rs` runs:

| Step | Module | Output |
|------|--------|--------|
| Call graph construction | `src/analysis/call_graph.rs` | `has_cycles` |
| Parameter ranges | `ParameterRanges` | Range metadata |
| Fusable pair detection | `src/backend/mod.rs` | `fusable_pairs` |
| Transition graph | `src/analysis/transition_graph.rs` | `BoundedPre`, `IncrementInfo`, `is_pure_body`, `has_triggers` |
| Region analysis (10-phase) | `src/analysis/region.rs` | Regions, value sets, chains, iter bounds, scores, composed chains |

The region analyzer's 10-phase pipeline: register declarations → dependency graph → frontier seeding → classification propagation → region computation → value-set estimation → linear chain detection → iteration bound resolution → region scoring → (composition + budget called separately).

---

## Path Priority Rationale

1. **Folded first**: A single-txn convergence program with no triggers can always be collapsed — no risk, pure win.
2. **Precomputed second**: If all composed chain bodies are fully evaluable at compile time, there is no runtime cost at all — dominates all other paths.
3. **Enumerated third**: Trigger sampling with switch dispatch covers the majority of interactive programs with bounded trigger value sets. Falls through to standard reactor for out-of-range values.
4. **Standard last**: The fallback works for every program. No optimization opportunity needed.
