# Optimization Pipeline

**Date:** 2026-06-11
**Status:** Current

## Decision Tree

After parsing, typechecking, and proof engine, the compiler enters a
decision tree that selects the codegen strategy per program:

```mermaid
flowchart TD
    A[Program] --> B{All const inputs?}
    B -->|Yes| C[Precomputation]
    C --> D{Within --optimize-budget?}
    D -->|Yes| E[A000: Constant-fold all txns]
    D -->|No| F[Fall through]
    B -->|No| F
    
    F --> G{is_counter_bounded?}
    G -->|Yes| H{Body has FFI?}
    H -->|No| I{A001: Pure counter fold?}
    I -->|Const bound| J[A001: O(1) store]
    I -->|Runtime bound| K[A005: Phi pipeline]
    H -->|Yes| L{Body has branching?}
    L -->|No| M[A005a: Folded SSA insertvalue]
    L -->|Yes| N{prove_linear?}
    N -->|Yes| M
    N -->|No| O[A005b: Folded memory (no phi)]
    
    G -->|No| P{Async/MMIO/triggers?}
    P -->|Yes| Q[Reactor tick loop]
    P -->|No| R[A006: Direct SSA loop]
```

## Codegen Strategies

### A000: Precomputation (--optimize-budget)

When all inputs are compile-time constants (no `frgn __get_env_int` calls
in the hot path), the interpreter evaluates each reactive transaction up
to the `--optimize-budget` limit (default 256). If all transactions converge
within the budget, the final state is emitted as a single `store i64 N,
%state.field` — the entire loop is folded away.

Detection: `analysis/region_analyzer.rs` — `is_fully_precomputable(budget)`.
If the program has runtime-determined inputs (`__get_env_int`, `frgn` calls
in the loop body), precomputation is disabled.

--dev vs --prod:
- --dev (default, budget=256): fast compilation, precomputes small loops
- --prod (budget=u64::MAX): fully precomputes every bounded loop
- --optimize-budget <N>: overrides both

### A001: Pure Counter Fold

When a reactive txn has a compile-time-constant bound AND a pure body
(no FFI calls), the compiler emits a single O(1) store of the final
counter value. No runtime loop.

File: `src/backend/llvm/loop_engine.rs` — `emit_folded_pure_counter`

### A005: Folded SSA / Memory (Counted Loop)

For programs with a counter-bounded reactive txn (e.g. `[count < N]`)
and a non-pure body (has FFI calls), the compiler emits a counted-loop
`main()`. Two sub-paths based on body structure:

#### A005a: Folded SSA Insertvalue (Straight-Line or Provably Linear)

When the body contains NO `Guarded`/`Escape` statements, OR when all
guard conditions are pairwise mutually exclusive (`prove_linear()` returns
true), the compiler uses the SSA insertvalue chain:

```
entry → _hdr → _body4 (4× unrolled) → _hdr (backedge) → _done → ret
```

- State read: `extractvalue %State %ssa_reg, %field_idx`
- State write: `insertvalue %State %ssa_reg, %val, %field_idx`
- Guard merge: `phi %State [ %then_val, %then_l ], [ %else_val, %entry_l ]`
- `%slot_` alloca carries state across loop iterations (load/store %State)

Safe only when `prove_linear()` confirms all guards are pairwise mutually
exclusive — otherwise phi incoming values don't dominate the merge block.

File: `src/backend/llvm/loop_engine.rs` — `emit_folded_main(use_phi=false, body=Some(stmts))`

#### A005b: Folded Memory (Non-Linear Body)

When the body HAS branching control flow (Guarded/Escape) AND `prove_linear()`
cannot prove mutual exclusivity, the compiler falls back to per-field
GEP+load/store with no phi nodes:

```
entry → _hdr → _body → _hdr (backedge) → _done → ret
```

- State read: GEP+load from `%state` pointer (via `pre_load_all_fields`)
- State write: GEP+store to `%state` pointer (via `ssa_state_reg = None`)
- No slot alloca, no extractvalue/insertvalue, no phi
- No 4× unrolling (single body emission)

LLVM's GVN/LICM eliminate redundant GEPs across the loop.

File: `src/backend/llvm/loop_engine.rs` — `emit_folded_memory_main` (2026-06-13)

### A005: Folded Phi Pipeline (Pure Body, Runtime-Variable Bound)

When the body is pure (no FFI) but the bound is runtime-determined
(`__get_env_int`), the compiler emits a counter-only phi pipeline.
The txn body is NOT emitted inline — it's called as `@txn(%State* %state)`.

File: `src/backend/llvm/loop_engine.rs` — `emit_folded_main(use_phi=true, body=None)`

### A006: Direct SSA Loop

For programs with NO async triggers and NO MMIO mappings, and where the
counter-bounded optimization (A005) does not apply, the reactor tick
dispatch function (`@reactor_tick`) is eliminated. Instead, a tight `while`
loop is emitted directly in `main()` with per-field GEP+load+store.

File: `src/backend/llvm/loop_engine.rs` — `emit_ssa_main()`

Key features:
- Per-field GEP codegen: `getelementptr %State, %State* %state, i32 0, i32 N`
  instead of `load %State; extractvalue; insertvalue; store %State`
- Multiple reactive txns, each with precondition check and body
- Exit condition determines loop termination
- Self-loop on precondition failure replaced with direct branch to `done`
  label (2026-06-11), enabling LLVM loop unrolling
- `loop_exit_label` for `term!` inside reactive loops emits `br %done`
  instead of `ret`, also enabling unrolling

### Linearity Proof (2026-06-13)

Standalone `prove_linear()` in `proof_engine.rs` determines whether a
transaction body's guard conditions are pairwise mutually exclusive.
Used at codegen time to choose between A005a (SSA insertvalue) and
A005b (memory).

**Algorithm**: For each pair of guard conditions (collected recursively
from `Guarded` statements), check `check_satisfiable(a, b)`. Returns
`false` (unsat → mutually exclusive) using:

1. **Bound contradiction**: `x > 5 && x < 4` — same var, contradictory bounds
2. **Equality contradiction**: `x == 5 && x == 10` — same var, different constants
3. **Boolean contradiction**: `x && !x` or `true && false`

All checks use standalone `extract_bound_from_expr()` and
`extract_eq_pair_from_expr()` free functions — no `SymbolicExecutor`
state needed.

### Reactor Tick Loop

For programs with async triggers or MMIO, the reactor tick function is
emitted as a separate `@reactor_tick` function that the runtime calls.
This is the legacy codegen path (emit_declare_loop).

File: `src/backend/llvm/emit_toplevel.rs`

## SLP Hazard Analysis

File: `src/backend/llvm/hazard.rs`

The compiler estimates register pressure from SLP vectorization candidates.
If the estimated peak live floats exceeds hardware register count, SLP is
disabled to avoid spilling.

### compute_peak_live_floats (2026-06-11)

Replaces `max_float_temps` (which counted ALL float temporaries as
simultaneously live). The new liveness-interval analysis:

1. For each float temp: find its def point (Let binding) and last use
   (last statement referencing it)
2. For field reads: def point is the first reference
3. Sweep program points counting active intervals → true peak register demand

Impact: nbody_sqrt went from 1.17x slower to 1.22x faster than C.

## Expression Simplification (Equality Saturation)

File: `src/analysis/equality_saturation.rs` — `simplify_program()`

A bottom-up rewriting pass with hash-cons cache that runs on the AST before
codegen. Rewritten 2026-06-13 — the original 5-pass fixpoint engine was
removed because it caused O(10^n) blowup on deeply nested `||` chains.

**Algorithm**: Single bottom-up pass (children before parent). Each node is
visited exactly once. A `HashMap<u64, Expr>` cache maps structural hashes
to simplified results, so identical subexpressions are simplified once.

**Complexity**: O(n) — 26-term `||` chain visits 26 nodes (previously ~10^26).

**18 rewrite rules**: `x+0→x`, `x*1→x`, `x-x→0`, `!!x→x`, `--x→x`,
`true&&x→x`, `false||x→x`, `(a+b)-b→a`, `(a-b)+b→a`,
`x&0→0`, `x|0→x`, `x^0→x`, `x<<0→x`, `x>>0→x`, and identity rules for
`Mul(0)`, `Div(1)`, `Or(true)`, `And(false)`, `x&&x`, `x||x`.

**Gating**: Only runs with `--prod`/`--release` flag. Disabled in `--dev` mode.
Controlled independently via `--simplify-budget <N>` and `--no-simplify`.

**Canonical replacement for `has_candidates`**: The old `has_candidates` function
(pre-check that returned true only when a candidate pattern existed) is dead
code — the new bottom-up pass is cheap enough (O(n)) to always run when enabled.

## Dispatch Collapse

File: `src/analysis/transition_graph.rs`

Preconditions evaluate against pre-tick state. Guards referencing unchanged
fields are skipped. The dispatch analysis builds a dependency graph between
state fields and transaction guards. When a tick updates field X, the
compiler walks the graph to find only the transactions whose guards
reference X.

## Copy Elimination

File: `src/backend/llvm/mod.rs` — `emit_expr` for `Expr::FieldAccess`

When loading a field value that was just stored (same field, same tick),
the load is replaced with `add i64 0, <stored_value>` — a register copy
instead of a memory load.

## Benchmark-Specific Optimizations

### nbody_sqrt (SLP fix)

The nbody benchmark has ~30 float fields with ~90 ALU operations per tick.
SLP vectorization was falsely disabled because `max_float_temps` counted
~113 simultaneously-live float temps when the true peak was ~6. The
liveness-interval fix corrected this.

### fannkuch_redux (self-loop + loop_exit_label)

The fannkuch benchmark's hot loop had two structural issues preventing
LLVM optimization:
1. Precondition failure created a live self-loop (`br i1 %ok, %body, %tick`)
   that LLVM could not eliminate. Fixed by branching to `done_{txn}`.
2. `term!` inside the loop body emitted `ret`, making the loop appear
   non-countable to LLVM's unroller. Fixed by emitting `br %done` via
   `loop_exit_label`.

Combined impact: 3.85x → 0.98x (brief ties C).

## Related Files

| File | Role |
|------|------|
| `src/backend/llvm/loop_engine.rs` | Direct SSA loop, folded SSA A005a, folded memory A005b, pure counter A001 |
| `src/backend/llvm/mod.rs` | Field index map, exit condition, codegen dispatch (A005 vs A006 decision) |
| `src/backend/llvm/emit_expr.rs` | Expr codegen (per-field GEP, copy elimination) |
| `src/backend/llvm/emit_stmt.rs` | Statement codegen (Guarded handler, let_bindings save/restore) |
| `src/backend/llvm/emit_toplevel.rs` | Definition/txn/callable-txn emission, ret terminators |
| `src/backend/llvm/hazard.rs` | SLP hazard analysis, compute_peak_live_floats |
| `src/analysis/equality_saturation.rs` | Expression simplification pass (bottom-up + hash-cons) |
| `src/analysis/region_analyzer.rs` | Precomputation budget analysis |
| `src/analysis/transition_graph.rs` | Dispatch collapse, is_counter_bounded |
| `src/analysis/proof_engine.rs` | `prove_linear()`, `check_satisfiable()`, `extract_bound/eq_pair` |
