# Benchmark Gap Fixes — 2026-07-03

## Scope

Fix remaining gaps identified in the benchmark-gap-analysis investigation:
1. **precompute_sum correctness**: `term! -> print_int#` silently dropped by pure counter fold path
2. **nbody_sqrt_idio correctness**: Same swan song bug (empty output)
3. **nbody precision mismatch**: All 4 nbody variants use `Float64` vs C's `float` (f32)
4. **`srem` vs `urem`**: Module ops use signed rem when operands are provably non-negative

## Files Touched

### Fix A: Swan song dropped by pure counter fold

- `src/analysis/transition_graph.rs` (~line 77-89): `body_no_term` strips terminating guards containing `term!` with swan songs, making FFI calls invisible to purity/liveness analysis.
  - Fix: pass original `txn.body` to `compute_effectively_pure` and `compute_live_fields` so swan song references are visible.

- `src/backend/llvm/mod.rs` (~line 2112): Add `statement_has_swan_song` check on the original txn body. If a swan song exists, skip pure counter fold path (even when `is_pure_body || is_effectively_pure`).

### Fix B: nbody Float64 → Float32

- `benchmarks/nbody_newton.bv`: Change all `Float64` → `Float32`, `f64` → `f32` suffix
- `benchmarks/nbody_sqrt.bv`: Same
- `benchmarks/nbody_newton_sym.bv`: Same
- `benchmarks/nbody_sqrt_idio.bv`: Same

### Fix C: `urem`/`udiv` for provably non-negative operands

- `src/backend/llvm/helpers.rs` — `emit_binop` function: When modulo or division operands are both compile-time known non-negative constants, emit `urem`/`udiv` instead of `srem`/`sdiv`.

## Architecture Impact

None — all changes are additive or local rewrites.

## Verification

1. `cargo test --lib` — all existing tests pass
2. `bash benchmarks/build_and_bench.sh --correctness` — precompute_sum output matches C
3. `bash benchmarks/build_and_bench.sh` — benchmark ratios should improve for fannkuch_redux
