# Optimization Decision Tree

**Last updated**: 2026-06-07
**Tests**: 450 passing

This document synthesizes the Briv compiler's optimization design — the decision tree the backend traverses and the rationale behind each path. See `llvm-backend-optimization-catalog.md` for the earlier 5-path formulation; this document reflects the full evolved pipeline.

---

## Core Principle: Contracts Enable Optimizations

Every optimization is sound **because** the programmer declares contracts. Without preconditions (guards), postconditions, and state declarations, the compiler would have to guess — Briv never guesses, it proves.

| Contract feature | Enables |
|---|---|
| `[count < N]` convergence bound | Pure-counter fold elimination, SCEV loop deletion |
| State field declarations | Dead-field elimination (liveness from FFI consumption) |
| Disjoint field access (proof engine) | Conflict-free async dispatch, no atomics needed |
| Bounded state space (value-set sizes) | Compile-time precomputation within budget |
| Trigger-gated preconditions `[trg == K]` | Enum switch dispatch, perfect hashing |

---

## Decision Tree

The `generate()` method in `src/backend/llvm.rs` applies optimizations in priority order. Each path replaces `@main` (and optionally `@reactor_tick`) with a specialized emission. Lower-ranked paths fall through.

```
Is entire state space ≤ --optimize-budget (default 256)?
  ├── YES → Path 1: Precomputation
  │           Zero runtime — all work done at compile time.
  │
  └── NO → Has triggers?
            ├── YES → Triggers enumerable (finite value-set)?
            │         ├── YES → No wake triggers?
            │         │         ├── YES → Path 2: Enum Switch Dispatch
            │         │         │           switch i64 per trigger value.
            │         │         │           Each case:
            │         │         │             ├── Pure body? → SCEV eliminates. O(1).
            │         │         │             └── Non-pure? → Struct-SSA body inlined.
            │         │         │
            │         │         └── NO  → Wake reactor: infinite loop with @__rt_wait().
            │         │                   Falls through to Path 5/6.
            │         │
            │         └── NO  → Triggers not enumerable (e.g. Int with large range).
            │                   Falls through to Path 5/6.
            │
            └── NO → Transactions conflict-free (pairwise)?
                      ├── YES → Path 3: Thread Pool Async
                      │           Per-txn worker functions on portable barrier.
                      │           Each worker:
                      │             ├── Pure body? → SCEV eliminates. O(1).
                      │             └── Non-pure? → Struct-SSA body inlined.
                      │
                      └── NO → Exactly one transaction, convergent, pure body?
                                ├── YES → Path 4: Pure-Counter Phi Loop
                                │           use_phi=true. Single i64 phi for counter,
                                │           no body inlined. SCEV eliminates. O(1) store.
                                │           **The only remaining phi usage** — scalar only.
                                │
                                └── NO → Non-pure body or multi-txn?
                                          ├── YES → Path 5: Folded Struct-SSA Loop
                                          │           use_phi=false. Load %State once,
                                          │           inline unrolled body (4× default) with
                                          │           extractvalue/insertvalue chains,
                                          │           store once per tick. No struct phi.
                                          │           Relies on `opt -O2` → SROA to decompose.
                                          │
                                          └── NO  → Path 6: SSA Main (generic fallback)
                                                      tick → load → body → store loop.
                                                      Still struct-SSA (no struct phi).
```

---

## Path Details

### Path 1: Precomputation (most aggressive)

**Trigger**: Total state space (product of all field value-sets) ≤ `--optimize-budget`.

**What it emits**: A single `@main` that calls `@init_state()` then stores final values into every state field via direct `getelementptr` + `store i64`. All transactions evaluated at compile time by the symbolic evaluator.

**Performance**: O(number of state variables). Zero runtime iterations.

**Key insight**: If your program folded, **the compiler was right**. Your program produced no runtime-dependent output. The fix is to make the bound runtime-determined (e.g., `__get_env_int("BOUND")`), not to add liveness hacks.

**Analysis**: `src/analysis/region.rs` — `is_fully_precomputable()`, `collect_final_values()`

---

### Path 2: Enum Switch Dispatch

**Trigger**: Enumerable triggers with known value-set sizes, no wake triggers.

**What it emits**: Sample triggers once, `switch i64` to per-value case blocks. Each case either stores the final counter directly (all-internal chain) or runs a concretized fused body.

**Sub-path — Pure-counter arm**: Counter in an i64 phi node, body skipped entirely. LLVM's SCEV pass eliminates the loop. O(1) at runtime.

**Sub-path — Non-pure arm**: Body inlined per switch case with struct-SSA (see Path 5).

**Perfect hashing**: `find_perfect_hash()` in `llvm.rs` finds multiplicative hash `(k*M)>>S` for sparse key sets. Falls back to standard switch when no hash found.

**Analysis**: `src/analysis/region.rs` — `value_set_size_of()`, `estimate_value_sets()`

---

### Path 3: Thread Pool Async

**Trigger**: Multiple reactive transactions, pairwise conflict-free (proof engine proves disjoint field access).

**What it emits**: Per-txn worker functions (`pre→fire` pattern) dispatched on a portable barrier (mutex+cond+counter). Main thread waits on barrier.

**Safety**: No atomics on state fields — the proof engine guarantees disjoint field access per txn group, so plain loads/stores are data-race-free (C11 5.1.2.4p25).

**Sub-path — Pure-counter worker**: SCEV eliminates. O(1).

**Sub-path — Non-pure worker**: Struct-SSA body inlined in worker.

**Analysis**: `src/analysis/transition_graph.rs` — conflict detection

---

### Path 4: Pure-Counter Phi Loop

**Trigger**: Exactly one transaction, convergent `[count < bound]`, pure body (counter increment only), no FFI, no field writes beyond counter.

**What it emits**: `use_phi=true`. A single `i64` phi for the counter value. No body inlining — just `phi → icmp → sub → branch` in a counted-down loop. LLVM's SCEV pass recognizes this as a linear recurrence and eliminates the entire loop.

**Performance**: O(1) — one store of the final counter value.

**Why no struct phi**: See "Why Phi Was Reduced" below. This is the only remaining phi usage because it carries a scalar `i64`, not the full `%State` struct.

---

### Path 5: Folded Struct-SSA Loop (workhorse path)

**Trigger**: Non-pure body (FFI, field updates, guarded blocks) or multi-txn but not async/enumerable/precomputable.

**What it emits**: `use_phi=false`. The compiler:

1. Loads `%State` once into an SSA register (line 4341)
2. Pre-extracts all float and int fields into individual SSA registers (lines 4126-4127)
3. Inlines the transaction body with `extractvalue` for reads and `insertvalue` chains for writes — no struct phi
4. Unrolls 4× by default (line 4317)
5. Stores `%State` once per tick (line 4153)

**Why no struct phi here**: The body modifies state fields. If the state were threaded through a phi node, every tick would create a 64-byte struct-typed phi. SROA must decompose these into scalar phis — but `llc -O2` doesn't run SROA, only `opt -O2` does. Without SROA, struct phis cause a 2× regression. The fix: **avoid struct phis entirely**, keep field values as individual SSA registers, and let `opt -O2` handle the struct load/store decomposition via SROA.

**Unrolling**: Default factor of 4. Each unrolled iteration starts with `pre_extract_float_fields` + `pre_extract_int_fields` so all field reads in the body use old values. This enables independent float operations that fill all CPU execution ports.

---

### Path 6: SSA Main (generic fallback)

**Trigger**: Multi-txn program that doesn't match any above path (not foldable, precomputable, enum-dispatchable, or async).

**What it emits**: `tick:` label → load `%State` → for each txn: check precondition → if true, emit body inline (with ssa_state_reg tracking insertvalue chains) → if false, phi-merge ssa_state_reg at guard skip → store `%State` → branch back to `tick` or `done` based on exit condition.

**Still uses struct-SSA**: No struct phi. Guard merge points use phi (line 4366) but only at per-guard merges, not as a general loop-carried struct phi.

---

## Why Phi Was Reduced

The original struct-SSA design threaded `%State` through phi nodes. This backed the compiler into a corner:

1. Struct-typed phis produce 64-byte values flowing through the loop header.
2. These must be decomposed into scalar phis by LLVM's SROA pass.
3. **`llc -O2` does NOT run SROA** — only `opt -O2` does.
4. Without SROA, the struct phis stay opaque → **2× regression** (0.14s → 0.28s at 10M).

The solution was two-fold:
- **Avoid struct phis**: Use `load %State` once → `extractvalue` for reads → `insertvalue` chains for writes → `store %State` once. No phi on the struct. Field values stay in individual SSA registers via pre-extraction.
- **Run `opt -O2` before `llc`**: SROA decomposes the single remaining struct load/store into scalar phis. GVN eliminates redundant float↔i64 round trips.

The only remaining phi is a single `i64` for the pure-counter case — a scalar, not a struct. SROA is irrelevant for scalars.

---

## Cross-Cutting Optimizations

These apply within any emission path above.

### Dead-Field Elimination
**What**: Liveness analysis tracks which state fields are ever consumed by an FFI call. Stores to unobserved fields are dropped.
**Why sound**: The postcondition only guarantees outcomes for observed outputs. Unobserved fields are definitionally dead.
**Analysis**: `src/analysis/liveness.rs`

### Dispatch-Chain Collapse
**What**: In multi-txn programs, preconditions previously evaluated against the post-update state of the previous txn (cascade bug). Fix: all preconditions evaluate in the entry block against pre-tick state, saved in SSA registers. The body chain uses saved results.
**Further collapse**: `is_uniform_body_group()` in `transition_graph.rs` detects structurally identical bodies. When all bodies match, the entire precondition chain is skipped — just the first body is called. LLVM then converts the chain to a `switch`, and after inlining, SCEV eliminates the loop entirely.
**Fixed at**: `src/backend/llvm.rs:2648-2680`

### Float Register Promotion
**What**: SSA mode emits native `float` registers alongside boxed `i64` forms. `i64_to_float_reg()` helper with `reg_float_cache` skips redundant `trunc`/`bitcast` chains.
**Impact**: Kalman filter boxing instructions reduced by ~85%.
**Added**: 2026-06-02 optimization sprint

### `llvm.assume` Injection
**What**: After emitting precondition branches, emits `call void @llvm.assume(i1 %cond)`. LLVM uses this to eliminate dead paths and simplify downstream expressions.
**Where**: Folded loop header emission

### SLP Hazard Analyzer
**What**: `estimate_slp_hazard()` computes peak register demand from live float fields (N), coupling density (C), temps (T), and global constants (K) against target hardware (R, W). Passes `-vectorize-slp=false` to `opt` when peak ≥ R.
**Formula**: `peak = ceil(N/W) + min(2·ceil(N/W), ceil(C/2)) + T + ceil(K/W) + 2`
**Why needed**: At ≥12 float fields with cross-variable coupling, `shufflevector` instructions from packed `<2 x float>` phis overflow x86_64's 16 XMM registers → 65 stack spills.
**Verified**: Kalman n=12, C=72, T=12, K=18, R=16, W=4 → peak=28 ≥ 16 → SLP disabled. Briv 0.71s vs C 0.75s.

### Equality Saturation
**What**: Lightweight recursive simplification — 5-pass fixpoint with 9 rewrite rules applied to expression trees at compile time.
**Examples**: `x + 0 → x`, `x * 1 → x`, `x & x → x`, `x | x → x`, `!!x → x`, `x - x → 0`, `x + (-y) → x - y`, `(a + b) * c → a*c + b*c`, `x && true → x`

### Compile-Time PGO
**What**: The interpreter profiles the program before codegen. Branch weights derived from interpreter profiling guide LLVM's `!prof` metadata on emitted branches.

### Constant Inlining & Deduplication
**What**: Integer/bool constants referenced by name emit as instruction immediates instead of `load` from global RAM. Identical constants emit as `@alias` — single global declaration, zero extra cache lines.

### Peephole Constant Folding
**What**: `emit_binop` and `emit_fcmp` fold integer+integer at compile time. Covers add/sub/mul/sdiv/and/or/xor/shl/lshr + all comparisons.

### Guard-to-Select Optimization
**What**: When a guarded statement has exactly one assignment to a state field, emits `select i1 <cond>, <true_val>, <existing_val>` + store instead of branch→then→merge, eliminating branch overhead.

### Alwaysinline for Acyclic Call Graphs
**What**: When the call graph has no cycles, transaction functions are tagged `alwaysinline`. LLVM inlines bodies into the dispatch loop, eliminating call overhead. No bloat observed — `opt -O2` + SCEV handles the phi/select cascade.

### LTO Pipeline
**What**: `compile_to_bitcode()` compiles C sources to bitcode via clang. `link_and_optimize()` merges program bitcode with `briv_rt.c` and any `import "link/..."` dependencies via `llvm-link`, then runs `opt -O2` for cross-module optimization.

### `!range` Metadata on State Loads
**What**: Preconditions of the form `var < constant` are extracted as (lo, hi) ranges and attached as `!range` metadata on state field loads. LLVM uses this for value-range analysis.

---

## Analysis Pipeline

Before any emission, `analyze_program()` in `src/backend/mod.rs` runs:

| Step | Module | Output |
|---|---|---|
| Call graph construction | `src/analysis/call_graph.rs` | `has_cycles` |
| Parameter ranges | `ParameterRanges` | Range metadata |
| Liveness analysis | `src/analysis/liveness.rs` | Live fields per FFI call chain |
| Transition graph | `src/analysis/transition_graph.rs` | `BoundedPre`, `IncrementInfo`, `is_pure_body`, `has_triggers`, uniform body groups |
| Conflict detection | `src/analysis/transition_graph.rs` | Pairwise conflict-free txns |
| Region analysis (10-phase) | `src/analysis/region.rs` | Regions, value sets, chains, iteration bounds, scores, composed chains |
| SLP hazard estimate | `src/backend/llvm.rs` | Peak register demand, vectorize-skip decision |

---

## Historical Context

All optimization sprints, benchmark timing tables, and implementation phases are preserved in `AGENTS_HISTORY.md`. Key milestones:

| Date | Milestone |
|---|---|
| 2026-05-31 | Pure-counter fold + precomputation + enum dispatch |
| 2026-06-01 | Dead-field elimination, dispatch-chain collapse, SROA pipeline fix |
| 2026-06-02 | Float register promotion, SLP hazard analyzer, `llvm.assume`, peephole folding |
| 2026-06-03 | Thread pool async, uniform-body collapse, calibration baseline |
| 2026-06-05 | `term! -> swan_song`, `#assume_event`, `#assume_shape` |
| 2026-06-07 | Phase 0–1: Universal FFI registry, No-Magic architecture |

---

## Relationship to Other Documents

| Document | Covers |
|---|---|
| `llvm-backend-optimization-catalog.md` | Original 5-path formulation (May 2026) |
| `optimization-cost-model.md` | Cost model specification, budget formulation |
| `determinism-and-optimization-frontier.md` | Determinism guarantees, optimization soundness |
| `AGENTS_HISTORY.md` | Full implementation history, benchmark tables, bug diagnoses |
