# Closing the Remaining Performance Gaps

**Date:** 2026-06-11
**Status:** Phase 0 in progress

## Current Benchmark Results

```
Benchmark              Briv      C          Ratio    Winner
fannkuch_redux         0.127s     0.069s     1.84x    C 1.84x       ← gap
float_math_nonzero     0.183s     0.167s     1.10x    C 1.10x       ← gap
```

## Root Cause

Every tick emits 17 separate GEP+load pairs (read old state) + 17 GEP+store
pairs (write new state). LLVM's SROA promotes `alloca` but not function
arguments (`%State*`). The tick loop operates on a pointer argument, so
17 fields survive as memory operations instead of phi nodes.

## Three-Phase Plan

### Phase 0 — Memcpy Round-Trip (largest impact)

At tick start: `alloca %State` + `@llvm.memcpy` from `%state` → alloca.
Operate on alloca via GEP+load+store.
At tick end: `@llvm.memcpy` from alloca → `%state`.
LLVM inlines the memcpy, SROA sees the alloca, promotes all 17 fields to phi.

**Expected:** fannkuch 1.84x → ~1.1x

### Phase 1 — Prior-State Elision

Skip prior-state load for fields that are write-only in a given tick.

**Expected:** ~0.5% gain

### Phase 2 — Precondition/Exit Dedup

When `exit_cond -> ¬pre_cond`, skip the precondition check.

**Expected:** ~3% gain

## Files Affected

| Phase | File | Change |
|-------|------|--------|
| 0 | `src/backend/llvm/loop_engine.rs` | alloca + memcpy preamble at tick start, memcpy epilogue at tick end. Redirect GEP operations to use alloca instead of %state |
| 1 | `src/analysis/dataflow.rs` | Expose read-field set per transaction |
| 1 | `src/backend/llvm/loop_engine.rs` | Skip prior-state load for write-only fields |
| 2 | `src/backend/llvm/loop_engine.rs` | Implication check: `exit_cond → ¬pre_cond` |
