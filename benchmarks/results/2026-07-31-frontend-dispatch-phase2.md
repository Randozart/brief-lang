# Phase 2 — Frontend Measurement Passes results

**Date:** 2026-07-31
**Worktree:** FDD worktree at `../briev-compiler-fdd`, branch `feat/frontend-driven-dispatch`
**Baseline:** Phase 1b results in `2026-07-31-frontend-dispatch-phase1b.md` (commit `c953c3c4`)
**Harness:** `bash benchmarks/build_and_bench.sh --runtime`, BOUND=50000000
**Toolchain:** `clang 18.1.3`, `llc 18.1.3`
**Raw output:** `/tmp/opencode/p2_runtime_final.log`

## What changed

Phase 2 (plan §7) moves four backend measurement decisions into the frontend,
computed ONCE in `analyze_program` and consumed by the LLVM backend:

| Pass | Module | Replaces |
|------|--------|----------|
| Float density | `src/analysis/density.rs` (`ComputeDensity`) | the `#11 → #0` downgrade re-count in `emit_toplevel.rs:1820-1849` (its `count_cross_float_ops_in_expr` ignored the `_all_idents` set — int-only arithmetic inflated the count; the frontend version gates on the txn's float set) |
| Modulo partition | `src/analysis/modulo_partition.rs` (`ModuloPartition`) | `extract_mod_info`/`extract_mod_guard` in `ssa.rs`; dispatch choice is now structural (bounded counter → rotated loop, the only form that handles a bound; one-shot switch only when no txn increments a counter) instead of `cases.len() <= 8` |
| Inline cost | `src/analysis/inline_cost.rs` (`callable_inline_decision`) | `params < 8 && body < 20` in `emit_toplevel.rs` — weighted body cost (call=10, binop=1), threshold 40 (Phase 3 → config) |
| Reactor attr | `transition_graph.has_unguarded_ffi` | the two body re-walks in `dispatch.rs:68-73/357-362` (`#2` vs `#12`) |

`AnalysisResults` extended per §7.5 (`density`, `modulo_partition`,
`has_unguarded_ffi`, `inline_decisions`).

Also fixed: the frgn `declare` block (`mod.rs:2069`) iterated `frgn_map` — a
HashMap with a per-process SipHash seed — unsorted, producing run-to-run
nondeterministic declaration ORDER in the IR (Coding Standard 7). Now sorted by
key. Verified: same compiler, two runs of ring_buffer → byte-identical IR.

## Behavioral equivalence

The reference compiler (Phase 1b tip `ff41a318`, built in a temp worktree) and the
Phase 2 compiler were both used to compile all 38 `benchmarks/*.bv` programs:

- **Per-txn memory attribute** (`#N alwaysinline`): identical for all 38 (diff
  of the sorted attr lists is empty). This includes the density-driven
  downgrades kalman_filter_runtime → `#0` and iir_filter → `#0` (both dense
  float), and all `#11` non-downgrades (nbody, float_math, ring_buffer, …).
- **Main dispatch marker** (`.cm_header` / `.vdN_header` / bare `define i32
  @main`): identical for all 38.
- **Emitted code** for the sensitive set (ring_buffer, kalman, nbody_newton,
  nbody_sqrt, sparse_dispatch, float_math): byte-identical excluding the
  `declare` block ordering (which varies run-to-run even between two reference
  builds — pre-existing non-determinism, now fixed).

## Runtime ratios (Briev vs C, ratio < 1 = Briev faster)

| Benchmark | Phase 2 Briev | Phase 2 ratio | Phase 1b ratio | Δ | Winner | Correct |
|-----------|--------------:|:-------------:|:--------------:|:---:|:------:|:-------:|
| ring_buffer | 0.0567s | 1.18× | 1.10× | +0.08 | C | MATCH |
| float_math | 0.0715s | 0.97× | 0.95× | +0.02 | Briev | MATCH |
| float_math_nonzero | 0.2013s | 1.20× | 1.21× | −0.01 | C | MATCH |
| sparse_dispatch | 0.0537s | 0.86× | 0.82× | +0.04 | Briev | MATCH |
| print_loop | 0.0619s | 1.06× | 1.05× | +0.01 | C | MATCH |
| nbody_newton | 6.8883s | 0.83× | 0.83× | 0.00 | Briev | MATCH |
| nbody_sqrt | 2.1868s | 0.78× | 0.77× | +0.01 | Briev | MATCH |
| nbody_sqrt_idio | 2.7818s | 0.76× | 0.75× | +0.01 | Briev | MATCH |
| fasta | 0.2120s | 1.01× | 0.98× | +0.03 | C | MATCH |
| fannkuch_redux | 0.0627s | 0.97× | 0.95× | +0.02 | Briev | MATCH |
| mandelbrot | 0.6814s | 1.03× | 1.03× | 0.00 | C | MATCH |
| kalman_filter_runtime | 0.2197s | 1.23× | 1.24× | −0.01 | C | MATCH |
| knucleotide | 0.1886s | 0.98× | 0.99× | −0.01 | Briev | MATCH |
| cancel_math | 0.0521s | 0.80× | 0.85× | −0.05 | Briev | MATCH |
| bit_clear | 0.0s | 0× | 0.33× | (noise) | Briev | MATCH |
| queue_drain | 0.0561s | 0.85× | 0.93× | −0.08 | Briev | MATCH |
| queue_drain_sym | 0.0565s | 0.92× | 0.90× | +0.02 | Briev | MATCH |
| queue_drain_idio | 0.0579s | 0.93× | 0.91× | +0.02 | Briev | MATCH |
| interval_step | 0.0629s | 1.01× | 1.01× | 0.00 | C | MATCH |

**Zero MISMATCH.** The two largest deltas are ring_buffer (+0.08×) and
queue_drain (−0.08×, i.e. *faster*). Both benchmark emitted code that is
byte-identical to the reference compiler (verified by diffing the generated
`.ll`), so these deltas are run-to-run noise, not codegen changes — the harness
shows ±0.05× variation on these ~0.05s benchmarks between consecutive runs of
the SAME compiler. All other deltas are ≤0.05×.

## Phase 2 regression tests added (20 total)

- `analysis::density` — dense-kalman downgrades (> 4.0 ops/field), sparse-nbody
  keeps #11 (≤ 4.0), int-only txn zero density, guarded lets excluded, int ops
  don't inflate.
- `analysis::modulo_partition` — 8-way partition, single-txn not a partition,
  mixed residues partition, divergent precondition breaks, different divisor
  breaks.
- `analysis::inline_cost` — small pure body inlines, heavy body not inlined,
  FFI body never inlined, `term` statement blocks inline (matches old
  `has_ffi_or_terminator_stmt` gate), guards weigh.
- `analysis::transition_graph` — `has_unguarded_ffi` set for top-level FFI vs
  guard-outlined FFI.
- `backend::llvm` — modulo partition drives the rotated `.mr_loop`; density
  consumer downgrades a dense outlined txn to `#0`.

`cargo test --lib`: 1259 passed, 0 failed (was 1239 at Phase 1b).
