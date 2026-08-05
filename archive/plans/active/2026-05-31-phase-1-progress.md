# Phase 1 Progress — Atomic Region Analysis

**Date**: 2026-05-31
**Commit base**: `08994aa` (Phase 0 — tactical convergence gaps)

## Completed

### Phase 0 — Tactical Gaps (3 soundness fixes + architecture restructure)
- **Pre-condition validation**: `check_pre_matches` verifies `post → ¬pre` structurally. Rejects `[true]`, `[var <= bound]` as pre-conditions for convergence.
- **Relational post-ops**: Handles `Ge`, `Gt`, `Le`, `Lt`, `Ne` (previously `Eq`-only).
- **Overshoot detection**: `step > 1` requires `(bound - init) % step == 0`. Conservative rejection when values aren't compile-time known.
- **Architecture restructure**: Convergence check moved from `SymbolicExecutor::verify_transaction` to `ProofEngine::verify_contracts`, giving program-level access for `initial_values`.
- **8 new tests**, 307 total passing.

### Phase 1 — RegionAnalyzer (complete)
Building the analysis pipeline from `docs/design/determinism-and-optimization-frontier.md`:

- **1.1 RegionAnalyzer struct** — trace dependency graph from trg roots, compute connected components via DFS. 9 unit tests.
- **1.2 Variable classification** — Pure / Bounded / Opaque axis. Bool triggers → Bounded, Int triggers → Opaque. BFS propagates from frontier through rev_deps.
- **1.3 Bound propagation** — Interval (lo, hi) extraction from literals and type bounds (U8 → [0,255], Bool → [0,1]). Requires contract-bound integration for broader coverage.
- **1.4 Value-set estimation** — size = hi - lo + 1 when interval known; Opaque vars → None.
- **1.5 Integration into ProofEngine** — `region_analyzer: Option<RegionAnalyzer>` field on `ProofEngine`, populated in `verify_program()`. Public query API: `region_of()`, `classification_of()`, `is_frontier_dependent()`.**

## Running Benchmark
```
IIR filter benchmark (50M iterations):
  Briv: 0.15s  C: 0.23s
  Briv is 1.53× faster than C
```

## Next: Phase 2 — Value-Set Enumeration in LLVM Backend

Goals:
- 2.1 Region cloning: for small value sets (e.g., Bool → 2), clone folding pass with concretized frontier var
- 2.2 Switch dispatch: `switch(trg) { case true: ...; case false: ... }` 
- 2.3 Residual fallback: uncovered values fall through to reactive segment-folded execution
- 2.4 Integration: `--optimize-budget <N>` CLI flag controls enumeration depth
