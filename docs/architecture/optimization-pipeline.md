# Optimization Pipeline

**Date:** 2026-07-05 (updated — A005a re-added, vector phi emission)
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
    G -->|Yes| H{Const bound + pure + no swan song?}
    H -->|Yes| I[A001: O(1) pure counter store]
    H -->|No| C{Periodic post-increment guard<br>count % N == 0?}
    C -->|Yes| D[A007: Countdown loop — single tight loop<br>+ cold guard block]
    C -->|No| V{One runtime when guard?}
    V -->|Yes| E[A008: version-DAG guard-absent/present]
    V -->|No| K[A005a: Inline SSA — counter-only writes]
    K -->|No| L[A005c: Per-field phi loop]
    L --> M{Dual-path memory + swan-song hoist}
    M -->|No post-loop hoist| N[Path A: zero stores in body]
    M -->|Post-loop hoisted guards| O[Path B: filtered stores]
    
    G -->|No| P{Async/MMIO/triggers?}
    P -->|Yes| Q[Reactor tick loop]
    P -->|No| R[A006: Direct SSA loop]
```

> **2026-07-31 (frontend-driven dispatch):** the dispatch tree above is the
> CURRENT structure — computed once in the frontend (`AnalysisResults` /
> `LoopShape`), not re-derived in the backend. The old `write_density >= 50%
> AND fields < 8` heuristic dispatch (A005a-vs-A005c by body re-walk) was
> removed in Phase 1b (`c953c3c4`). The **countdown loop** (A007) is the
> universal emission for periodic post-increment guards (`when count % N == 0`
> after `count++`); see `docs/plans/2026-07-31-fmn-countdown-vs-batch-and-
> new-benchmarks.md`. See the plan for the current 5-way switch order.

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

### A005a: Inline SSA (insertvalue chain)

> **2026-07-31:** The heuristic gate below (`write_density >= 50%, fields < 8,
> no FFI`) was removed in Phase 1b. The InlineSsa path is now selected
> STRUCTURALLY by `LoopShape.counter_only_writes` (the write set is exactly
> `{counter}`) — see the dispatch plan §6.5.

For bodies with write_density >= 50%, field_count < 8, and no FFI calls,
the compiler emits a single `%State` phi with extractvalue/insertvalue
access. This allows LLVM's SROA+GVN to optimize the entire state as one
SSA unit.

Re-introduced in `4ff9bde` (2026-07-05) after being removed in `a71c586`.
The adaptive dispatch decision tree selects A005a vs A005c per transaction.

### A005c: Per-Field Phi Loop (Default for Most Bodies)

For counter-bounded programs that don't qualify for A005a, the compiler
emits a per-field phi loop (A005c). Reverted from A005e hybrid mode
(memory-based fields with counter-only phi) in `4ff9bde` because A005e
re-introduced memory traffic:

- interval_step: 0.01x (A005c) vs 1.00x (A005e) — **100× faster**
- nbody_newton: 0.89x (A005c) vs 1.41x (A005e) — **37% faster**

**Structure:**
```
entry → pre_phi → loop_hdr → body → latch → loop_hdr (backedge) → done → ret
```

- One phi per state field at `loop_hdr` (scalar or `<4 x float>` vector phi)
- Backedge registers for each field computed at `latch`
- Zero or more guards inside `body` for periodic prints
- Swan song guards (`term! -> print_int#`) hoisted to post-loop `done:` block

**Dual-path memory:**

| Path | Stores in body | When selected | Loads in done: |
|------|---------------|---------------|----------------|
| **A** | Zero | No post-loop hoisted guards (Path A active) | Nothing — `done:` skips `pre_load_all_fields` |

(Showing lines 1-80 of 253. Use offset=81 to continue.)
| **B** | Per-field subset via `done_needs_fields` | Post-loop hoisted guards exist (swan song) | Filtered by `done_needs_fields` — only fields the print references |

**Path A** (no stores): The hot loop body is a pure register pipeline:
phi → compute → latch backedge. Zero memory traffic. LLVM's optimizer
sees a clean phi loop with no barriers — enabling full vectorization,
ILP scheduling, and SROA.

**Path B** (stores preserved): The hot loop body emits stores for fields
that `done:` reads. `done_needs_fields` (populated by scanning hoisted
guard bodies) limits stores to the subset of fields the hoisted print
actually references. Unreferenced fields get zero stores.

**Chunk allocas:** `%State` is split into ≤15-field chunks
(`%StateChunk0`, `%StateChunk1`, ...) to ensure LLVM's SROA pass can
decompose each chunk into scalar phi nodes independently.

**Key files:**
- `src/backend/llvm/loop_engine.rs` — `emit_countable_main`, `emit_countable_body`, `emit_countable_latch`
- `src/backend/llvm/emit_stmt.rs` — `emit_memory_field_store` with store gating
- `src/backend/llvm/context.rs` — `needs_state_stores_in_body`, `done_needs_fields`, `parallel_safe_body`

### A006: Direct SSA Loop

For programs with NO async triggers and NO MMIO mappings, and where the
counter-bounded optimization (A005c) does not apply, the reactor tick
dispatch function (`@reactor_tick`) is eliminated. Instead, a tight `while`
loop is emitted directly in `main()` with per-field GEP+load+store.

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

Combined impact: 3.85x → 0.98x (briev ties C).

## Related Files

| File | Role |
|------|------|
| `src/backend/llvm/loop_engine/mod.rs` | `emit_main`: natural convergence exit, `emit_ssa_main`: convergence check with `any_active` tracking |
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

---

## Natural Convergence Exit

When all reactive transactions have converged (their postconditions are met
and preconditions can never become true again), the program should exit
naturally — not spin forever in the main loop.

**Detection — one-shot vs restartable:**

A transaction is *restartable* if external input can make its precondition
true again after convergence. This happens with wake triggers, async workers,
or timer-based wake conditions. Transactions without these are *one-shot*:
once the postcondition is met and precondition is false, they'll never run again.

**Main loop convergence check (emit_main):**

```llvm
; After reactor_tick:
; For one-shot programs (no wake, no async), check if any txn ran this tick.
; If none ran, all are converged — exit.
%any_active = load i64, ptr %active_slot
%done = icmp eq i64 %any_active, 0
br i1 %done, label %.end, label %.loop
```

**SSA loop convergence check (emit_ssa_main):**

```llvm
; Track whether any txn body executed this iteration.
; Body sets active flag; convergence check reads it.
store i64 0, ptr %any_active   ; reset
; ...txn precondition check + body execution...
store i64 1, ptr %any_active   ; body ran
; ...after all txns checked:
%check = load i64, ptr %any_active
%done = icmp eq i64 %check, 0
br i1 %done, label %.end, label %.loop
```

**Natural death logic** (`mod.rs:2239`): Builds a synthetic exit condition
from bounded-counter txns (those with `bounded_pre` + `increments` analysis).
When all bounded counters reach their bounds, the exit condition is met.

**Halting guarantee:** The halting proof is established at compile time.
Every `node` is checked for `bounded_pre` + `increments` via the
transition graph analysis. If any txn lacks a provably bounded
precondition, the exit condition is not set, and the program must rely
on the one-shot convergence check (which triggers when no txn runs for
a full iteration).

**See also:** Block 14 in `docs/plans/2026-07-18-master-overview.md`
