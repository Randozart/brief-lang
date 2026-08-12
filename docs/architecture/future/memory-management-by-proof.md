# Memory Management by Proof

**Date:** 2026-07-25
**Status:** Hypothesised feature — no implementation yet

## Summary

Briev's provenance analysis pass currently tracks pointer lifetimes but does
not automatically select allocation strategies (stack arena vs heap). This
feature would extend provenance analysis to automatically promote allocations
between stack-tied arenas and long-lived heaps based on compile-time proof of
pointer escape.

## Key Design Questions

- How does the proof engine decide that a pointer never escapes the current
  stack frame? (Provenance analysis exists but doesn't drive allocation strategy.)
- When a pointer is proven to escape, what heap strategy is used? Thread-local
  arena? Global allocator? User-configurable?
- How does this interact with the existing `term!` / `swan_song` liveness
  pattern? (Observability as liveness.)
- What diagnostic tools help developers understand WHY the compiler chose a
  particular allocation strategy?

## Dependencies

- Provenance analysis pass (`src/analysis/provenance.rs`) — exists, partial.
- Proof engine (`src/proof_engine/`) — SMT solver integration exists, heuristic
  proving exists.
- Backend codegen (`src/backend/llvm/`) — per-allocation strategy emission.

## See Also

- `docs/architecture/narrowing-by-proof.md` — existing proof-driven optimization
  (Int width narrowing) that this feature's architecture parallels.
- `docs/architecture/bits-thesis.md` — memory model foundations.
