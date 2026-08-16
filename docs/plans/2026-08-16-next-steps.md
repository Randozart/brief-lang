# Next steps — post 2026-08-16 session backlog

**Date:** 2026-08-16
**Head commit:** `aacb6ae1`
**Context:** this session shipped three perf wins —
`8e89f07b` multi-node internal fold (nbody_newton_accel 7x → 1.18x),
`7b7e3bde` coll_length local-coll tracking (Direction 2),
plus the earlier `a7e3af74` coll grow-on-full + guard elimination
(queue_drain_idio 0.58x). This file is the backlog of the four candidate next
steps, ranked. Each is a self-contained investigation or feature.

## Benchmark state (2026-08-16, runtime suite)

Worst C losses remaining: sweep_dense 1.50x, sweep_sparse 1.43x, sweep_arr
1.17x, ring_buffer 1.16x, sweep_mid 1.11x, mandelbrot 1.02x.
Near parity / Briev wins: the rest (queue_drain* 0.56-0.60x, float_math 0.65x,
global_lifetime 0.42x, accel 1.18x, etc.).

## 1. Sweep-family triage (PRIMARY — biggest remaining losses)

**Observed:** sweep_sparse/mid/dense/arr lose 1.1–1.5x. All are SINGLE-node
counted loops with a periodic `when count % N == 0` guard. sweep_dense
dispatches via the countdown loop (`emit_countable_countdown_main`, N=5M,
17 fields) — the batch-loop structure, NOT the D3 multi-node fold.

**Hypotheses (each needs a measurement, rule 19):**
- The inner compute loop's vectorization / if-conversion vs the C reference.
- The countdown `%fire` guard / cold-block handling.
- Register pressure across 17 PerFieldPhi fields.
- The density→`#0` downgrade (config/targets.dbvl `dense_compute_density`).

**Verification:** `opt -O3`→asm of the sweep loop vs the C reference; compare
vector width, instruction count, and branch structure per element (the D3
playbook). If a structural gap surfaces, land it as a first-class pass.

## 2. Accel 8-wide probe (cheap, uncertain)

**Observed:** nbody_newton_accel at 1.18x (was 7.05x pre-D3). The gap: LLVM
vectorizes the folded countdown to 4-wide (`rcpps %xmm`) while C gets 8-wide
(`vrcpps %ymm`). NOT alignment (C uses unaligned ymm loads) — it's the LLVM
cost model.

**Probe:** force wider vectorization (loop metadata `vectorize.width`, a
restructured loop, or an alignment experiment on `%State`) and measure. If
accel drops below 1.0x, land it. Low effort; payoff uncertain.

## 3. D2 completion — monotone-loop pre-grow (user-facing completeness)

**Deferred from Direction 2** (plan 2026-08-16-proven-subset-extension.md):
- **Pre-grow:** `foreach x in 0..N { q <- x }` with N > cap — emit
  `EnsureCap#(q, N)` before the loop, then strip the per-push guard. Exact
  allocation, no geometric overalloc. Needs the frontend to communicate the
  bound to the backend and insert the EnsureCap (AST-level or codegen hook).
- **Single-firing relaxation:** a `[done == 0][done == 1]` node fires once —
  its state-field coll's monotone pushes are bounded by the body alone. Needs
  a firing-count proof (bounded_pre / terminal-flag analysis).

No benchmark impact (no monotone build-loop benchmark); real user-facing value.

## 4. Coll track (deferred roadmap)

From the pre-session plan (coll-length-semantics / grow-on-full plans):
- **Coll-struct construction:** list-literal→`Int[N]` coercion so
  `coll struct Fixed { data: Int[4] }` constructs from literals. Blocks the
  SPEC §8.10 example end-to-end.
- **Const generics:** `coll struct Fixed<T,N> { data: T[N] }` per SPEC §8.10
  (stays normative; spec-outlined is work-to-do).
- Then: OPEN BUGS.md stdlib files (iterator.bv / hashmap.bv never compile),
  iterable slice-6 deletions, fundamentals-as-types (Data as reflective floor).

## Suggested order

1 → 2 (side-probe) → 3 → 4. All keep the "prove the known subset, fall back
conservatively" discipline — polynomial passes, no combinatorial search.
