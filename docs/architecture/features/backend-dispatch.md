# Backend Dispatch (Optimization Path Selection)

**Date:** 2026-07-04 (updated — A005a/A005b/A005d removed, replaced by A005c with dual-path)
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
    CounterBounded -->|Yes + anything else| A005c[Per-field phi loop A005c]
    CounterBounded -->|No, reactive| A006[Direct SSA Loop A006]
```

There is no longer a field-count threshold. A005c (per-field phi loop)
handles ALL countable-loop field counts — from 1 to 31+. Chunk allocas
(≤15 fields per chunk, `MAX_FIELDS_PER_ALLLOCA=15`) let SROA decompose
even 31-field states into independent scalar phi nodes.

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

### A005c: Per-Field Phi Loop (formerly A005a/A005b/A005d)

**File:** `loop_engine.rs:emit_countable_main`
**Condition:** Counter-bounded, any body (pure or non-pure)
**Gate:** `mod.rs:2181` (default path for all non-precomputed countables)

**Mechanism:**
One phi per state field at the loop header. LLVM sees a canonical
`phi + icmp + add` structure for each field — enabling induction
variable analysis, SROA, and loop vectorization.

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

## What Was Removed

| Variant | Removed | Reason |
|---------|---------|--------|
| **A005a** (struct-SSA insertvalue) | 2026-07-03 | Replaced by A005c per-field phi. SROA dependency was fragile. |
| **A005b** (memory counter) | 2026-07-03 | Replaced by A005c. GVN-dependent reload eliminated by native backedge. |
| **A005d** (memory loop for >8 fields) | 2026-07-04 | Chunk allocas make per-field phi viable for all field counts. A005d paid GEP+load+store per field per iteration — a pessimization vs phi loop. |

## Why One Path for All Field Counts

Chunk allocas (≤15 fields per `<%StateChunkN>`) decompose monolithic
`%State` into SROA-friendly chunks. SROA's 64-element threshold is
never exceeded. With Path A, even a 31-field A005c loop has **zero
memory traffic** — every field is a phi register. The phi loop is
strictly better than any memory-based alternative.

## Linearity Proof

`proof_engine.rs::prove_linear(body)` checks whether a transaction body
can have at most one guard then-path firing per iteration. Used for the
A005a path (removed). Preserved for future use.

## Files

| File | Lines | Role |
|------|-------|------|
| `src/backend/llvm/mod.rs` | 3248 | Dispatcher — decision logic at lines 2160-2190 |
| `src/backend/llvm/loop_engine.rs` | 2869 | A005c per-field phi loop, A006 direct SSA, hoisted prints |
| `src/backend/llvm/emit_stmt.rs` | 1122 | `emit_memory_field_store` — gate on `needs_state_stores_in_body` + `done_needs_fields` |
| `src/backend/llvm/context.rs` | 483 | `FunctionContext` flags: `needs_state_stores_in_body`, `parallel_safe_body`, `done_needs_fields` |
| `src/proof_engine.rs` | 3600+ | `prove_linear`, `check_satisfiable`, `extract_bound/eq_pair` |
| `src/analysis/transition_graph.rs` | — | `is_counter_bounded` analysis |
