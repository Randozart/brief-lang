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

### Phase 1 — RegionAnalyzer (in progress)
Building the analysis pipeline from `docs/design/determinism-and-optimization-frontier.md`:

- **1.1 RegionAnalyzer struct** — trace dependency graph, compute connected components
- **1.2 Variable classification** — Pure / Bounded / Opaque axis
- **1.3 Bound propagation** — interval arithmetic through expressions
- **1.4 Value-set estimation** — state space size per frontier variable
- **1.5 Integration into ProofEngine**

## Running Benchmark
```
IIR filter benchmark (50M iterations):
  Brief: 0.15s  C: 0.23s
  Brief is 1.53× faster than C
```
