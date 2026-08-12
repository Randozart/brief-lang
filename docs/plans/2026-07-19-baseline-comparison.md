# Baseline Comparison Worktree

**Date:** 2026-07-19
**Purpose:** Permanent regression detection — a pinned baseline commit to compare against.

## Setup

The baseline is a persistent git worktree at the Phase 3 anchor commit:

```bash
git worktree add ../briev-compiler-baseline e88fd55
```

This creates a checkout at `../briev-compiler-baseline` that:
- Shares git history/objects with the main worktree (no duplication)
- Is writable — can build the compiler there
- Persists until replaced (never deleted unless benchmarks are equal or better)
- Does not interfere with the main worktree (different branches, different commits)

## Usage

To compare a benchmark against baseline:

```bash
cd ../briev-compiler-baseline
cargo build --release
BOUND=50000000 ./target/release/briev-compiler build benchmarks/nbody_newton.bv --out benchmarks
BOUND=50000000 bash -c "TIMEFORMAT='%3R'; time ./benchmarks/nbody_newton" 2>&1

# Same steps in main worktree, compare times
```

## Updating the Baseline

When ALL benchmarks at the main worktree equal or exceed the baseline, update:

```bash
rm -rf ../briev-compiler-baseline
git worktree prune
git worktree add ../briev-compiler-baseline <current-tip-commit>
```

The old baseline is discarded. The new baseline is the current tip.

## Current Baseline Commit

```
8a827db 2026-07-11: Phase 3.4 — fix remaining = in type bodies across std lib
```

This is the Phase 3 anchor commit. The Phase 3 benchmark results (benchmarks/results/2026-07-11-phase3-complete.md) were recorded at this point.

Note: The performance difference between this baseline and the current compiler comes from the loop engine dispatch changed between Phase 3 and the July 18 feature set. Phase 3 uses a simple `loop_hdr`/`latch` direct loop. The current compiler uses a convergence-check loop (`.cm_`/`.cm_`) with per-field phi nodes, which is inherently slower per iteration. This is a known trade-off — the convergence loop enables better compile-time analysis and supports async dispatch, but at a runtime cost of ~2.2x for tight loops like nbody_newton.

## Comparison Script

`benchmarks/compare_baseline.sh` — compares a single benchmark between baseline and current:

```bash
bash benchmarks/compare_baseline.sh nbody_newton
```
