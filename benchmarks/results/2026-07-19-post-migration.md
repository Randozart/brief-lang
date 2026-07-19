# Benchmark Results — Post Intrinsic Migration + Stabilization

**Date:** 2026-07-19
**Build:** `cargo build --release`
**Baseline:** `benchmarks/results/2026-07-11-phase3-complete.md` (pre-migration)

---

## Correctness: 22/24 MATCH (up from 16/24 pre-migration)

| Benchmark | Pre-Migration | Post-Migration | Change |
|-----------|---------------|----------------|--------|
| iir_filter | MATCH | MATCH | — |
| precompute_sum | MATCH | MATCH | — |
| const_heavy | MATCH | MATCH | — |
| async_counters_idio | MISMATCH | MISMATCH | pre-existing |
| utf8_ops | MISMATCH | MISMATCH | pre-existing |
| ring_buffer | MATCH | MATCH | — |
| float_math | MATCH | MATCH | — |
| float_math_nonzero | MATCH | MATCH | — |
| sparse_dispatch | MATCH | MATCH | — |
| print_loop | MATCH | MATCH | — |
| nbody_newton | MATCH | MATCH | — |
| nbody_sqrt | SKIP | **MATCH** | ✅ SSA fix |
| nbody_sqrt_idio | MISMATCH | **MATCH** | ✅ SSO + print fix |
| fasta | MISMATCH | **MATCH** | ✅ SSO + print fix |
| fannkuch_redux | MATCH | MATCH | — |
| mandelbrot | MISMATCH | **MATCH** | ✅ SSO + print fix |
| kalman_filter_runtime | MATCH | MATCH | — |
| knucleotide | MISMATCH | **MATCH** | ✅ OpConfig fix |
| cancel_math | MISMATCH | **MATCH** | ✅ Already fixed |
| bit_clear | MATCH | MATCH | — |
| queue_drain | MATCH | MATCH | — |
| queue_drain_sym | MATCH | MATCH | — |
| queue_drain_idio | MATCH | MATCH | — |
| interval_step | MATCH | MATCH | — |

## Fixes Applied

| Fix | File | Impact |
|-----|------|--------|
| OpConfig TOML nested-table parsing | `src/config.rs` | Unlocks ALL bitwise ops (Shl, Shr, BitAnd, BitOr, BitXor) that were silently emitting `add i64`. Also fixes comparison ops (Lt, Gt, Le, Ge, Eq, Neq) via config. |
| Comparison zext in config path | `src/backend/llvm/emit_expr.rs` | Config templates for comparisons produce `i1`; codegen adds `zext i1 to i8` to match `Type::bool_()` convention |
| Clear last_val_temps before hoisted prints | `src/backend/llvm/loop_engine/counter.rs` | SSA dominance violation — loop body SSA registers used in exit block. Fix forces phi/%-State resolution. |
| brief_str_to_c handles SSO + C strings | `lib/runtime/brief_rt.c` | All frgn string parameters work under SSO mode |
| Print plugin Float32/Float64 dispatch | `src/plugin/print_plugin.rs` | Float-typed variables (nbody energy) print correctly |
| Loop engine detects __print_* calls | `src/backend/llvm/loop_engine/analysis.rs` | Precomputation no longer eliminates output calls |

## Remaining Issues

| Benchmarks | Status | Root Cause |
|------------|--------|------------|
| async_counters_idio | **MATCH** | Fixed by removing is_pure_body requirement from multi-txn fold; precomputation now applies. |
| utf8_ops | MISMATCH | Performance issue: pure-Brief memcmp uses txn convergence loops for each byte, very slow. C reference completes 50M iterations faster than Brief completes 5. Known limitation. |
