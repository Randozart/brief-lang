# Plan: Linearity Proof + Memory Fallback + LLVM Backend Fix

**Date:** 2026-06-13
**Status:** Active

## Problem

The LLVM backend's `emit_folded_main` SSA insertvalue path creates `phi %State`
nodes at Guarded block merge points. When guard conditions aren't mutually
exclusive, phi-incoming values don't properly dominate — `llc` reports 17 SSA
verification violations on `officina-cli`.

## Solution

Three-tier dispatch based on control flow analysis:

```
Body has Guarded/Escape?
  → NO  → SSA insertvalue path (trivially safe — single block)
  → YES → prove_linear(): guards pairwise mutually exclusive?
     → YES → SSA insertvalue path (proven safe — at most one then-path fires)
     → NO  → Memory path (GEP+load+store, no phi, no dominance issue)
```

## Implementation

### Phase 1: Standalone `check_satisfiable` (proof_engine.rs)

Extract `extract_bound` and `extract_eq_pair` from `SymbolicExecutor` into
standalone free functions. Add `split_and` decomposition and
`check_satisfiable(a, b) -> bool` that proves `a && b` is unsat using:
- Bound contradiction: `x > 5 && x < 4`
- Equality contradiction: `x == 5 && x == 10`
- Boolean contradiction: `x && !x`

### Phase 2: `prove_linear` (proof_engine.rs)

Collect all guard conditions from a transaction body. For each pair, check
if `condition_i && condition_j` is unsat via `check_satisfiable`. If all
pairs are unsat → at most one then-path fires per iteration → linear.

### Phase 3: Memory-based counted loop (loop_engine.rs)

New `emit_folded_memory_main` — counted-loop structure with direct memory
access (like `emit_ssa_main`) instead of SSA insertvalue chains. No slot
alloca, no extractvalue/insertvalue, no phi. Self.ssa_state_reg = None so
writes go through GEP+store. Uses existing `pre_load_all_fields`.

### Phase 4: Dispatch (mod.rs)

At the A005 folded dispatch point, check `body_has_branching`. If branching,
run `prove_linear`. Linear → SSA path. Not linear → memory path.

### Phase 5: Test with officina-cli

Build officina.ll, verify `opt -passes=verify` passes. Fix any additional
backend bugs encountered.

## Files Changed

| File | What |
|------|------|
| `src/proof_engine.rs` | `extract_bound` + `extract_eq_pair` as standalone fns; `split_and`; `check_satisfiable`; `prove_linear` |
| `src/backend/llvm/loop_engine.rs` | `emit_folded_memory_main` |
| `src/backend/llvm/mod.rs` | Dispatch: branching check + linearity proof → path choice |
| `BUGS.md` | Document SSA dominance + fix |
