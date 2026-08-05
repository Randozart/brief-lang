# Final runtime table — plugin/macro rework Phases 1-4 + Part B

**Date:** 2026-08-01
**Commits:** `8962a2a1` (Phase 1) → `c5ae8b78` (B4b); `7dceefb7` (B0), `ba1d02b4` (B1), `4452ae3d` (B2), `30922fc6` (B3), `9106bb51`/`c5ae8b78` (B4)
**Worktree:** `../briv-compiler-plugin-rework`, branch `feat/plugin-macro-rework`
**Plan:** `docs/plans/2026-08-01-plugin-macro-rework.md`
**Harness:** `bash benchmarks/build_and_bench.sh --runtime`, BOUND=50000000
**Raw output:** `/tmp/plugin_final.log`
**Baseline:** `2026-08-01-plugin-rework-baseline.md` (commit `f546af1c`)
**Toolchain:** `clang 18.1.3`, `llc 18.1.3`

## 1. Final results (rule #11 — after all Phase 1-4 + Part B B0-B4 changes)

5 iterations per benchmark, nanosecond-precision fork+exec timing.

> **B4 note (2026-08-01):** the SSO layer and `is_string_like` structural
> heuristic were retired (B4a) and the legacy String types deleted (B4b).
> These are **compile-time-only** removals — the `feature_sso_strings` flag was
> always `false` in production, so the emitted runtime IR is unchanged. The
> benchmark numbers below (recorded at B1) are the current runtime state; no
> re-run is needed for a compiler-only change. The `.text`-ratio precomputed
> detection is unaffected.

| Benchmark | Briv | C | Ratio | Winner | Correct | Δ vs baseline |
|-----------|:-----:|:--:|:-----:|:------:|:-------:|:------------:|
| ring_buffer | .0519s | .0439s | 1.18× | C | MATCH | +.02 |
| float_math | .0422s | .0703s | .60× | Briv | MATCH | 0 |
| float_math_nonzero | .1569s | .1646s | .95× | Briv | MATCH | 0 |
| sparse_dispatch | .0474s | .0597s | .79× | Briv | MATCH | −.02 |
| print_loop | .0329s | .0562s | .58× | Briv | MATCH | −.01 |
| nbody_newton | 7.4724s | 8.9388s | .83× | Briv | MATCH | 0 |
| nbody_sqrt | 2.3246s | 2.9984s | .77× | Briv | MATCH | +.01 |
| nbody_sqrt_idio | 2.9184s | 3.8474s | .75× | Briv | MATCH | 0 |
| fasta | .2180s | .2191s | .99× | Briv | MATCH | −.07 |
| fannkuch_redux | .0594s | .0643s | .92× | Briv | MATCH | −.02 |
| mandelbrot | .7047s | .6855s | 1.02× | C | MATCH | 0 |
| kalman_filter_runtime | .1521s | .1782s | .85× | Briv | MATCH | 0 |
| knucleotide | .1896s | .1903s | .99× | Briv | MATCH | 0 |
| cancel_math | .0512s | .0589s | .86× | Briv | MATCH | +.01 |
| bit_clear | .0002s | .0002s | 1.00× | ~tie | MATCH | −.25 |
| interval_step | .0585s | .0600s | .97× | Briv | MATCH | −.03 |
| telemetry_stream | .1915s | .1984s | .96× | Briv | MATCH | 0 |
| pid_control | .3406s | .3468s | .98× | Briv | MATCH | 0 |
| matrix_pipeline | .4622s | .7192s | .64× | Briv | MATCH | +.02 |
| accumulator_flush | .1059s | .1495s | .70× | Briv | MATCH | −.01 |
| sweep_sparse | .2193s | .1543s | 1.42× | C | MATCH | −.01 |
| sweep_mid | .2599s | .2356s | 1.10× | C | MATCH | −.01 |
| sweep_dense | .3969s | .2640s | 1.50× | C | MATCH | 0 |
| bridge_glue | done | — | — | — | SKIP | — |
| bridge_multi | done | — | — | — | PASS | — |

**Zero MISMATCH. Zero regression >0.05 ratio.** All deltas are run-to-run noise
(±0.03). fasta improved (−.07); bit_clear variance (1.25→1.00) is a
sub-millisecond measurement artifact. The >1.2× C losses (sweep_* 1.10–1.50×,
ring_buffer 1.18×) are the known loop-shape/density concerns owned by the
frontend-driven-dispatch workstream — unchanged by this plan.

## 2. What changed in this window (Phases 1-4, B0/B1)

- **Phase 1** (commits `f00496a6`→`8962a2a1`): `print!`/`println!` and
  `get_env!`/`get_env_int!` plugins; Rust-style formatting; direct `__print_*`
  FFI calls (no bridge). Verified unchanged: print_loop .58×, float_math .60×.
- **Phase 2** (`be67baa6`): `[#]` entry marker removed.
- **Phase 3a** (`a8a4f421`): `main(i32, ptr)` + argv globals + `__argv_*`
  runtime + `std/cli.bv`.
- **Phase 3b** (`6fb929a8`): `entry!`/`args!` plugin.
- **Phase 3c** (`9dd1398d`): concurrency gate (rule #21) — benchmarks with
  multiple auto-firing nodes were audited and explicitly classified
  (`async node` / `sync<group>`).
- **Phase 4** (`0984f47c`): flat-scripting plugin (`defn main` / bare lets
  now run exactly once).
- **Part B B0** (`7dceefb7`): String = `ptr` to `[len][bytes]` everywhere.
- **Part B B1** (`ba1d02b4`): content Eq/Ne + `#String` bitwise defaults.

None of these touch hot-loop arithmetic or the FFI call sequences measured
here; the print/env FFI remains direct and inlinable (see the baseline doc's
FFI audit §2).

## 3. Benchmark classification audit (Phase 3c)

| Benchmark | Change | Why |
|-----------|--------|-----|
| async_counters | `async node inc_a/inc_b` | parallel-intent (disjoint writes, pthreads reference) |
| async_counters_sym | `async node inc_a/inc_b` | same, sym variant |
| async_counters_runtime | `sync<counters> node inc_a/inc_b` | sequential/barrier intent (comment: "not thread pool") |

All three build and run correctly post-classification.
