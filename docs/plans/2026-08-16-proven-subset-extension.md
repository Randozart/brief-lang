# Plan: proven-subset extension (Directions 2/3/1)

**Date:** 2026-08-16
**Head commit:** `34cb50ad` (coll guard elimination shipped; queue_drain_idio 0.59x)

Extends the "prove known subsets cheaply, fall back conservatively" philosophy:
recognize a shape via a syntactic pass, prove ONE property, emit specialized
codegen, fail conservatively — no combinatorial search.

## Direction 3 — nbody_newton_accel 7.05x (first)

**Observed:** 0.90s vs C 0.128s (fair: `nbody_newton_accel_c.c` does the same
2048 bodies × 50000 steps). The accel CPU path is the plain txn body —
`@txn_step_bodies` = one body + `ret`, no countdown loop. The program is 6
async phase-sequenced nodes; the reactor dispatches an if-chain over all of
them per cycle with precondition checks (`phase == 1 && i < nb`). The
single-node `nbody_newton` is 0.83x — LLVM collapses its reactor loop; the
multi-node phase dispatch cannot.

**Root cause (diagnosed 2026-08-16):** the multi-node reactor dispatch
(`emit_ssa_main` / `emit_async_body`) INLINES each node's per-firing body into
the main loop — the `@txn_<name>` functions are never called for reactive
nodes. The `graph.nodes.len() == 1` gate (mod.rs:3368) means the countdown /
PerFieldPhi fold only fires for single-node programs. After LTO inlining, the
shared counter `i` is memory-resident (reloaded ~6× per body from the 98KB
state struct) and the loop never vectorizes — the scalar `divss` division
dominates. C gets a register-counter 8-wide `vrcpps` loop.

**Fix (shipped):** the multi-node internal fold. A counted-loop node whose
whole bounded pass provably runs without starving any other node (the gate
`internal_fold_info`: no other node's pre can fire mid-pass) is emitted as a
noinline countdown `@txn_<name>`; the reactor dispatch (`emit_ssa_main` sync +
`emit_async_body` thread-pool) CALLS it once per pass instead of inlining the
per-firing body. The countdown keeps the counter in a phi register and
vectorizes. Result: **7.05x → 1.18x** (0.15s vs C 0.129s), output matches C
exactly, all 37 benchmarks MATCH, 1877 tests green.

## Direction 2 — monotone push loops via pre-grow (pending)

`coll_length` proves only state-field colls with delta ≤ 0. Extend:
1. Track local colls (fresh per firing; intra-body peak only).
2. Relax the delta rule for single-firing nodes (`[done == 0][done == 1]`).
3. Pre-grow a statically-bounded foreach (`EnsureCap#(q, N)` before it) so the
   per-push guard strips — exact allocation, no geometric overalloc.

Verify: tutorial 21-push example becomes guard-free with a pre-grow; drain
unchanged; 37/37 MATCH.

## Direction 1 — loop-carried invariant generalization (pilot)

`coll_length` is one instance of "prove a loop-carried property, strip runtime
work." Pilot the second instance: strip `emit_precondition_check` when the loop
shape provably implies the contract (counted `[i < N][i == N]` implies `i < N`
at the gate). New invariant class, polynomial walk, fail-conservative; output
in `AnalysisResults`, consumed by the backend.

## Ordering / discipline

3 → 2 → 1. All keep compile time linear-in-the-AST; no search added.
