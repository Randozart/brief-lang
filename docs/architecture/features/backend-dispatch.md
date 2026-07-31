# Backend Dispatch (Optimization Path Selection)

**Date:** 2026-07-07 (updated — is_decreasing in A005c, flexible base extraction for vector phis)
**Status:** Current

> **2026-07-31 — Frontend-driven dispatch.** The decision tree below is the
> LEGACY heuristic dispatch. Since Phase 1b the dispatch is computed ONCE in
> the frontend (`AnalysisResults` / `LoopShape`) and the backend consumes it —
> no body re-walks, no `write_density` arithmetic, no hardcoded field counts.
> The periodic-post-increment-guard case now emits the **countdown loop** (a
> single tight loop + cold guard block), and single `when`-guard bodies go
> through the recursive version-DAG decomposition (§5.3 of
> `backend-architecture.md`). See `docs/plans/2026-07-31-frontend-driven-
> dispatch.md` §5-§6 and `docs/plans/2026-07-31-fmn-countdown-vs-batch-and-
> new-benchmarks.md`. The composite-node/version-DAG note below is retained for
> the guard-decomposition history.

## Purpose

Select the optimal codegen strategy for each compiled program based on
program structure: loop bounds, FFI presence, swan song patterns.

## Decision Tree

The dispatch runs after typechecking in `mod.rs:emit_emit_main_phase3_llvm()`:

```mermaid
graph TD
    Start[Program] --> AllConst{All inputs const?}
    AllConst -->|Yes + within budget| Precomp[Precomputation A000]
    AllConst -->|No| CounterBounded{Counter-bounded?}

    CounterBounded -->|Yes + pure + const bound| PureCounter[Pure counter fold — O(1)]
    CounterBounded -->|Yes| Periodic{Periodic post-increment guard<br>count % N == 0?}
    Periodic -->|Yes| Countdown[Countdown loop A007 — tight loop + cold guard]
    Periodic -->|No| OneGuard{One runtime when guard?}
    OneGuard -->|Yes| VersionDAG[Recursive version-DAG decomposition]
    OneGuard -->|No| InlineSsa{Write set is exactly {counter}?}
    InlineSsa -->|Yes| A005a[Inline SSA — insertvalue chain A005a]
    InlineSsa -->|No| A005c[Per-field phi loop A005c]
    A005c --> Rotate{Rotation pattern detected?}
    Rotate -->|Yes + 12+ cycle| Step4[Step-4 GEP reload decomposition]
    Rotate -->|No| Standard[Standard latch backedge]

    CounterBounded -->|No, reactive| A006[Direct SSA Loop A006]
```

> **2026-07-31:** the decisions above (periodic guard, one-guard, write-set
> subset) are computed by the frontend analysis passes — see
> `src/analysis/batch_shape.rs`, `loop_shape.rs`, and the dispatch switch in
> `src/backend/llvm/mod.rs`.

## Path Details

### A000: Precomputed
**File:** `loop_engine.rs:emit_precomputed_main`
**Condition:** All inputs compile-time constant, iterations ≤ budget
**Mechanism:** Interpreter simulates all loops, final state stored in single `store`
**No phi, no loop, no SSA — just `main = { store; ret; }`**

### A001: Pure Counter Fold
**File:** `loop_engine.rs:emit_folded_pure_counter`
**Condition:** Counter-bounded, pure body, compile-time constant bound
**Mechanism:** O(1) store of final counter value. No runtime loop.
**Gate:** `mod.rs:2167 — total_val.is_some()`

### A005a: Inline SSA (insertvalue chain, re-added 2026-07-05)
**File:** `loop_engine.rs:emit_folded_main`
**Condition (2026-07-31):** selected structurally when the write set is exactly
`{counter}` (`LoopShape.counter_only_writes`) — the old heuristic gate below is
historical.
**Gate:** `mod.rs:2245` (adaptive dispatch for pure bodies)
**Mechanism:** Single `%State` phi with extractvalue/insertvalue for field
access. LLVM's SROA+GVN sees the entire state as one SSA unit.
**Performance:** ~2× faster than A005c for dense-write small-state benchmarks
(e.g., knucleotide: 0.42x best-known), but blocked for FFI-containing bodies
to prevent LLVM from eliminating fprintf calls through @stdout analysis.

Removed in `a71c586` (2026-07-03), re-added in `4ff9bde` (2026-07-05) with
FFI guard. See `docs/plans/2026-07-05-adaptive-loop-dispatch.md`.

### A005c: Per-Field Phi Loop (Default)

**File:** `loop_engine.rs:emit_countable_main`
**Condition:** Counter-bounded, any body (non-precomputed, non-A005a-eligible)
**Gate:** `mod.rs:2257` (default path for all non-precomputed countables)

**Mechanism:**
One phi per state field at the loop header. LLVM sees a canonical
`phi + icmp + add` structure for each field — enabling induction
variable analysis, SROA, and loop vectorization.

**is_decreasing support** (`82752a0`): A005c previously only supported
increasing counters (`icmp slt` in the header exit check).  For decreasing
counters (e.g., popcount decay: `reg > 0` via `[reg != 0]` precondition),
the header now emits `icmp sgt` when `is_decreasing` is true.  The latch
handles modified fields via GEP-reload from `%State`, so no latch arithmetic
change is needed.  The dispatch gate at `mod.rs:2183` was extended to accept
`bound_literal` programs (no field or constant for the bound).

**Expr::Ne in extract_bounded_pre** (`82752a0`): `[reg != 0]` preconditions
normalize to `Expr::Ne`, which `extract_bounded_pre` did not handle — the
program fell through to A006 (direct SSA loop with `any_fired`/`cycle_count`
overhead).  Now matched as decreasing convergence toward the literal bound,
validated by `extract_valid_bounded_pre` against `IncrementInfo`.  bit_clear:
routes through A005c per-field phi instead of A006.

**Vector phi grouping** (`82752a0`): Replaced the naming-convention regex
`[a-z][a-z][0-9]+` with flexible base extraction — strips trailing digits
from any field name and groups by the base (e.g., `vel_x_0` → `vel_x_`,
`vx0` → `vx`, `x0` → `x`).  A sequential-index guard (first 4 members
must have indices 0, 1, 2, 3) prevents false positives from matrix fields
(p00/p01 vs p10/p11 sharing base `p`).  No expression-shape consistency
check is required because the vector phi is register-storage aggregation,
not SIMD arithmetic.  nbody_sqrt: 0.72x (best known).  kalman_filter:
0.99x (no false positive from matrix fields).

**Rotation decomposition** (`ca9f483`): Detects circular permutation chains
(e.g., p0←p1←...←p11←p0) and uses GEP-reload from %State in the latch
instead of `pending_phi_native_backedge` values. Breaks the 12-cycle for
SCEV analysis. fannkuch_redux: 1.65x → 1.37x.

**Hybrid rotation hot/cold path** (`0dba619`): Extends rotation to emit
a pre-check `count + step <= bound` before the unrolling loop. When the
full trip fits, a straight-line basic block executes step-1 copies
without exit checks — eliminating ~3 branches per trip (step=4). When
only a partial trip fits (final trip), the cold path preserves per-copy
exit checks. `pending_phi_native_backedge` is saved before the hot path
and restored before the cold path to maintain SSA dominance. fannkuch_redux:
1.29x → 0.94x.

**Terminating guard filter in rotation copies** (`2cbcfe3`): The hot and
cold path body copies re-emit the original txn body, which still contains
the `[count == N] { term! -> print_int#(checksum) }` terminating guard.
Although `hoist_terminating_guard` already extracted this into
`pending_post_hoist` for post-loop emission, the rotation body copy loops
at `loop_engine.rs:1701,1770` only filtered `Statement::Term` /
`Statement::TermBang`, leaving the `Statement::Guarded` wrapper intact.
Each rotation copy emitted a dead `icmp eq count, N` + `br i1` — 4 per
4-iteration batch = ~50M dead branches for N=50M.

Fix: filter `Statement::Guarded{statements}` where `terminating_guard()`
returns true, using the existing helper. The swan song print is safely
handled by `emit_hoisted_post_loop_prints` after the loop exits naturally.
fannkuch_redux: 1.14x → 0.99x. See `docs/plans/2026-07-07-fannkuch-straight-line-rotation.md`.

**Dual-path architecture** (controlled by `needs_state_stores_in_body`):

| Path | Flag | Stores in body | Use case |
|------|------|---------------|----------|
| **A** | `false` | Zero | No post-loop hoisted guards. Phi registers + `pending_phi_native_backedge` carry all values. Zero memory traffic — pure register pipeline. |
| **B** | `true` | Per-field subset | Post-loop hoisted guards exist (`term! -> swan_song`). Stores ensure `done:` block's GEP+load sees fresh values. **Per-field liveness** (`done_needs_fields`) limits stores to only the subset of fields that `done:` actually reads. |

**Decision** at `loop_engine.rs:1250`:
```
needs_state_stores_in_body = !pending_post_hoist.is_empty()
```

### A006: Direct SSA Loop

**File:** `loop_engine.rs:emit_ssa_main`
**Condition:** Not counter-bounded, no async/MMIO/triggers
**Mechanism:** Tick loop with per-field GEP+load+store. Multiple txns supported.
**Gate:** `mod.rs:2200 — !folded`

### Reactor Tick Loop

**File:** `dispatch.rs:emit_reactor` / `emit_parallel_reactor`
**Condition:** Has async triggers or MMIO mappings
**Mechanism:** Calls `@reactor_tick` in a loop via the runtime dispatch layer.

## Swan Song Hoisting

`mod.rs:hoist_terminating_guard()` extracts terminating guard bodies into
`pending_post_hoist` for post-loop emission. Three patterns:

| Guard pattern | Hoisted? | Path |
|---------------|----------|------|
| `[cond] { stmts; term! -> swan_song; }` | Yes — full guard body + swan song | B |
| `[cond] { term! -> swan_song; }` | Yes — swan song only (empty body) | B |
| `[cond] { stmts; print_int#(val); }; term;` | No — not a TermBang guard | A |

When the swan song references a state field (e.g., `print_int#(checksum)`),
the field is tracked in `done_needs_fields`. Only that field gets a store
in the hot loop body (Path B) — the other fields are skipped. See
`loop_engine.rs:1255` for the pre-population scan.

## Optimizations Tracked Per-Architecture

| Feature | Added | Enabled | Benchmarks Affected |
|---------|-------|---------|---------------------|
| A005a re-add (adaptive dispatch) | `4ff9bde` | write_density≥50%, fields<8, no body FFI | knucleotide, mandelbrot, ring_buffer |
| A005c latch save/restore | `2b2ef32` | All A005c loops | mandelbrot (dominance fix) |
| Precomputation fix | `981819c` | All programs | knucleotide, fasta, cancel_math |
| Dead-field liveness | `6529f29` | All A005c loops | nbody_newton, float_math |
| Rotation decomposition | `ca9f483` | 12+ cycle detected | fannkuch_redux |
| Hybrid rotation hot/cold | `0dba619` | rotation_step > 1, no body FFI | fannkuch_redux |
| Terminating guard filter | `2cbcfe3` | rotation_step > 1, terminating Guarded present | fannkuch_redux |
| Vector phi emission | `a849b2d` | 4+ float fields per group | nbody_sqrt, nbody_newton, nbody_sqrt_idio |
| Vector phi flexible base | `82752a0` | Any field name with trailing digits | nbody_sqrt, nbody_newton, nbody_sqrt_idio |
| Decreasing counter A005c | `82752a0` | bound_literal + is_decreasing | bit_clear |
| Expr::Ne in bounded_pre | `82752a0` | Precondition `[var != N]` | bit_clear |

## Performance Results (2026-07-05, BOUND=50000000)

| Benchmark | Before A005e | Best A005c | After | Improvement |
|-----------|-------------|-----------|-------|-------------|
| nbody_newton | 1.41x | 0.89x | **0.63x** | -37% |
| nbody_sqrt | 1.29x | 1.25x | **0.72x** | -39% |
| nbody_sqrt_idio | 0.96x | 0.82x | **0.72x** | -14% |
| fannkuch_redux | 2.16x | 1.65x | **0.99x** | -41% |
| knucleotide | 1.00x | 1.00x | **0.99x** | tied |
| float_math | 0.83x | 0.86x | **0.83x** | tied |

## What Was Removed

| Variant | Removed | Re-added? | Reason |
|---------|---------|-----------|--------|
| **A005a** (struct-SSA insertvalue) | 2026-07-03 (a71c586) | **Yes** (4ff9bde, adaptive) | Restored as adaptive path for dense-write, <8 fields. |
| **A005b** (memory counter) | 2026-07-03 | No | Replaced by A005c. GVN-dependent reload eliminated by native backedge. |
| **A005d** (memory loop for >8 fields) | 2026-07-04 | No | Chunk allocas make per-field phi viable for all field counts. |
| **A005e** (hybrid counter-phi + memory) | 2026-07-05 (4ff9bde) | No | Re-introduced memory traffic. interval_step: 100× slower. |

## Why One Path for Most Field Counts

Chunk allocas (≤15 fields per `<%StateChunkN>`) decompose monolithic
`%State` into SROA-friendly chunks. SROA's 64-element threshold is
never exceeded. With Path A, even a 31-field A005c loop has **zero
memory traffic** — every field is a phi register. The phi loop is
strictly better than any memory-based alternative.

Vector phis go further: grouping fields into `<4 x float>` reduces
phi count from 32 to ~14, eliminating register spills. This is
orthogonal to chunk allocas — vector phis operate at the SSA level,
chunk allocas at the alloca level.

## Files

| File | Lines | Role |
|------|-------|------|
| `src/backend/llvm/mod.rs` | 3250+ | Dispatcher — decision logic at lines 2240-2320 |
| `src/backend/llvm/loop_engine.rs` | 4023 | A005a, A005c, vector phis, rotation, A006 |
| `src/backend/llvm/emit_stmt.rs` | 1219 | `emit_memory_field_store` — vector group insertelement, store gate |
| `src/backend/llvm/context.rs` | 562 | `FunctionContext` flags + vector_phi_groups + rotation_fields |
| `src/proof_engine.rs` | 3600+ | `prove_linear`, `check_satisfiable`, `extract_bound/eq_pair` |
| `src/analysis/transition_graph.rs` | — | `is_counter_bounded`, `statement_contains_ffi` |
