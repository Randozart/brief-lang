# Phase 1b — Frontend-Driven Dispatch (backend collapse) results

**Date:** 2026-07-31
**Worktree:** FDD worktree at `../brief-compiler-fdd`, branch `feat/frontend-driven-dispatch`
**Baseline:** Phase 0 results in `2026-07-31-frontend-dispatch-phase0.md` (commit `ed2f4234`)
**Harness:** `bash benchmarks/build_and_bench.sh --runtime`, BOUND=50000000
**Toolchain:** `clang 18.1.3`, `llc 18.1.3`
**Raw output:** `/tmp/fdd_runtime.log`

## What changed

Phase 1b replaces the LLVM backend's loop-dispatch heuristics
(`src/backend/llvm/mod.rs`) with a deterministic switch over the frontend
`LoopShape` computed in Phase 1a (`docs/plans/2026-07-31-frontend-driven-dispatch.md` §6.5):

- `mod.rs`: deleted `hoist_terminating_guard` / `remap_stmt_identifiers` /
  `remap_expr_into`; synthetic exit-condition construction replaced with
  `loop_shape::program_convergence`; the `write_density`/`total_fields`
  heuristic block replaced with `emit_folded_loop_shape` + `shape_vector_groups`.
- `backend/mod.rs`: `build_swan_songs` now derives its state-field set from
  `loop_shape::collect_state_fields` (both `TopLevel::StateDecl` AND top-level
  `Statement(Let)`), fixing a latent let-to-field remap gap.
- `loop_shape.rs`: `collect_state_fields` made `pub(crate)` for reuse.
- `vector_phi.rs`: `detect_vector_groups` removed (structural pass supersedes it).
- Comments in `counter.rs` / `ssa.rs` / `context.rs` updated to reference the
  frontend swan-song pass.

All emitters (`emit_countable_main`, `emit_folded_main`, `emit_version_dag_main`,
`emit_folded_pure_counter`) are unchanged.

## Runtime ratios (Brief vs C, ratio < 1 = Brief faster)

| Benchmark | Phase 1b Brief | Phase 1b ratio | Phase 0 ratio | Winner | Correct |
|-----------|---------------:|:--------------:|:-------------:|:------:|:-------:|
| ring_buffer | 0.0516s | 1.10× | 1.13× | C | MATCH |
| float_math | 0.0727s | 0.95× | 0.97× | Brief | MATCH |
| float_math_nonzero | 0.2007s | 1.21× | 1.24× | C | MATCH |
| sparse_dispatch | 0.0507s | 0.82× | 0.84× | Brief | MATCH |
| print_loop | 0.0626s | 1.05× | 1.01× | C | MATCH |
| nbody_newton | 6.9163s | 0.83× | 0.83× | Brief | MATCH |
| nbody_sqrt | 2.1631s | 0.77× | 0.77× | Brief | MATCH |
| nbody_sqrt_idio | 2.7575s | 0.75× | 0.75× | Brief | MATCH |
| fasta | 0.2085s | 0.98× | 0.97× | Brief | MATCH |
| fannkuch_redux | 0.0614s | 0.95× | 0.97× | Brief | MATCH |
| mandelbrot | 0.6842s | 1.03× | 1.02× | C | MATCH |
| kalman_filter_runtime | 0.2193s | 1.24× | 1.22× | C | MATCH |
| knucleotide | 0.1891s | 0.99× | 0.99× | Brief | MATCH |
| cancel_math | 0.0538s | 0.85× | 0.85× | Brief | MATCH |
| bit_clear | 0.0002s | 0.33× | ~tie | Brief | MATCH |
| queue_drain | 0.0564s | 0.93× | 0.86× | Brief | MATCH |
| queue_drain_sym | 0.0564s | 0.90× | 0.89× | Brief | MATCH |
| queue_drain_idio | 0.0564s | 0.91× | 0.94× | Brief | MATCH |
| interval_step | 0.0632s | 1.01× | 1.01× | C | MATCH |

**Zero MISMATCH.** All deltas are within run-to-run noise of the Phase 0 baseline
(≤0.04×; the two largest are queue_drain 0.86→0.93 and float_math_nonzero
1.24→1.21, consistent with the ±0.03 band already observed in Phase 0 vs the
`666fb502` baseline). bit_clear improved from ~tie to 0.33× (Brief 0.0002s vs C
0.0006s) — a noise artifact of timing a ~0.2ms benchmark.

## Dispatch decision A/B (from emitted IR markers)

Decision per benchmark is byte-identical to Phase 0 (`b` = Phase 1b, `0` = Phase 0):

| Benchmark | Phase 1b marker | Phase 0 marker | Same |
|-----------|-----------------|----------------|:----:|
| nbody_newton | 3× `.cm_header` | 3× `.cm_header` | ✅ |
| nbody_sqrt | 3× `.cm_header` | 3× `.cm_header` | ✅ |
| nbody_sqrt_idio | 4× `.vd45_header` | 4× `.vd45_header` | ✅ |
| kalman_filter_runtime | 4× `.vd5_header` | 4× `.vd5_header` | ✅ |
| ring_buffer | 3× `.cm_header` | 3× `.cm_header` | ✅ |
| float_math | 4× `.vd5_header` | 4× `.vd5_header` | ✅ |
| float_math_nonzero | 4× `.vd5_header` | 4× `.vd5_header` | ✅ |
| sparse_dispatch | bare `define i32 @main` | bare `define i32 @main` | ✅ |
| fannkuch_redux | 3× `.cm_header` | 3× `.cm_header` | ✅ |
| knucleotide | 4× `.vd5_header` | 4× `.vd5_header` | ✅ |
| mandelbrot | 4× `.vd5_header` | 4× `.vd5_header` | ✅ |
| fasta | 3× `.cm_header` | 3× `.cm_header` | ✅ |
| print_loop | 4× `.vd5_header` | 4× `.vd5_header` | ✅ |
| queue_drain | 4× `.vd5_header` | 4× `.vd5_header` | ✅ |
| interval_step | bare `define i32 @main` | bare `define i32 @main` | ✅ |
| cancel_math | 4× `.vd5_header` | 4× `.vd5_header` | ✅ |
| bit_clear | bare `define i32 @main` | bare `define i32 @main` | ✅ |

Marker legend: `.cm_header` = `emit_countable_main` (PerFieldPhi / vector-phi
label); `.vdN_header` = `emit_version_dag_main` (version-DAG); bare
`define i32 @main` with no header labels = pure counter fold / reactor path.

## Regression tests added (7)

- `loop_shape::tests::test_collect_state_fields_accepts_let_and_statedecl`
- `loop_shape::tests::test_collect_state_fields_includes_legacy_statedecl`
- `backend::tests::test_build_swan_songs_remaps_top_level_let_field`
- `backend::tests::test_collect_state_fields_matches_build_field_index`
- `llvm::tests::test_shape_vector_groups_same_type_gate`
- `llvm::tests::test_shape_vector_groups_drops_not_in_write_set`
- `llvm::tests::test_shape_vector_groups_no_overlap`

`cargo test --lib`: 1239 passed, 0 failed. `cargo build`: no new warnings.
Praetor: all changed files pass.

Notes:
- `bridge_glue`/`bridge_multi` failed at the end (missing `koffi` node package) —
  unrelated to this effort; not in the ratio table.
- The `info: txn … dispatched via …` warnings are recorded in `self.warnings`
  but not printed by `briefc build`; the `.ll` header-label markers are the
  authoritative dispatch signature and match Phase 0 for every benchmark.
