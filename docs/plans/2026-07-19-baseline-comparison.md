# Baseline Comparison Worktree

**Date:** 2026-07-19
**Purpose:** Permanent regression detection — a pinned baseline commit to compare against.

## Setup

The baseline is a persistent git worktree at the Phase 3 anchor commit:

```bash
git worktree add ../brief-compiler-baseline e88fd55
```

This creates a checkout at `../brief-compiler-baseline` that:
- Shares git history/objects with the main worktree (no duplication)
- Is writable — can build the compiler there
- Persists until replaced (never deleted unless benchmarks are equal or better)
- Does not interfere with the main worktree (different branches, different commits)

## Usage

To compare a benchmark against baseline:

```bash
cd ../brief-compiler-baseline
cargo build --release
BOUND=50000000 ./target/release/brief-compiler build benchmarks/nbody_newton.bv --out benchmarks
BOUND=50000000 bash -c "TIMEFORMAT='%3R'; time ./benchmarks/nbody_newton" 2>&1

# Same steps in main worktree, compare times
```

## Updating the Baseline

When ALL benchmarks at the main worktree equal or exceed the baseline, update:

```bash
rm -rf ../brief-compiler-baseline
git worktree prune
git worktree add ../brief-compiler-baseline <current-tip-commit>
```

The old baseline is discarded. The new baseline is the current tip.

## Current Baseline Commit

```
334a168 Fix: emit hoisted let bindings in post-loop prints
```

This is the last commit before the intrinsic migration started (July 19). Includes Phase 1 fixes (terminator, SSO concat) but none of the migration/stabilization changes.

## Comparison Script

`benchmarks/compare_baseline.sh` — compares a single benchmark between baseline and current:

```bash
bash benchmarks/compare_baseline.sh nbody_newton
```
