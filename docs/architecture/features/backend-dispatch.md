# Backend Dispatch (Optimization Path Selection)

**Date:** 2026-07-05 (updated — A005a re-added, vector phis, rotation decomposition)
**Status:** Current

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
    CounterBounded -->|Yes| DensityTest{Dense writes + <8 fields + no FFI?}
    DensityTest -->|Yes| A005a[Inline SSA — insertvalue chain A005a]
    DensityTest -->|No| A005c{Per-field phi loop A005c}
    A005c --> VectorPhi{Vector phi detected?}
    VectorPhi -->|Yes + 4+ float fields| V4[<4 x float> vector phis]
    VectorPhi -->|No| Scalar[Scalar phis]
    A005c --> Rotate{Rotation pattern detected?}
    Rotate -->|Yes + 12+ cycle| Step4[Step-4 GEP reload decomposition]
    Rotate -->|No| Standard[Standard latch backedge]

    CounterBounded -->|No, reactive| A006[Direct SSA Loop A006]
```

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
**Condition:** write_density >= 50%, field_count < 8, no body FFI calls
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

**Vector phi grouping** (`a849b2d`): Fields matching pattern `[a-z][a-z][0-9]+`
with 4+ same-prefix members (e.g., vx0..vx3) emit `<4 x float>` phis instead
of scalar float phis. Reduces register pressure: 32 scalar phis → ~14 phis,
fitting in 16 XMM registers without spills. nbody_sqrt: 1.25x → 0.79x.

**Rotation decomposition** (`ca9f483`): When the body's field assignments form
a circular permutation chain (e.g., p0←p1←...←p11←p0), the latch emits
GEP reloads from %State instead of pending_phi_native_backedge values.
Breaks the 12-cycle for SCEV analysis. fannkuch_redux: 1.65x → 1.37x.

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
| Vector phi emission | `a849b2d` | 4+ float fields per group | nbody_sqrt, nbody_newton, nbody_sqrt_idio |

## Performance Results (2026-07-05, BOUND=50000000)

| Benchmark | Before A005e | Best A005c | After | Improvement |
|-----------|-------------|-----------|-------|-------------|
| nbody_newton | 1.41x | 0.89x | **0.63x** | -37% |
| nbody_sqrt | 1.29x | 1.25x | **0.79x** | -37% |
| nbody_sqrt_idio | 0.96x | 0.82x | **0.67x** | -18% |
| fannkuch_redux | 2.16x | 1.65x | **1.37x** | -18% |
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
