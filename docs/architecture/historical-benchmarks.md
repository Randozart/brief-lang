# Historical Benchmark Timeline

Compiled 2026-07-28 from AGENTS_HISTORY.md, AGENTS_HISTORY_2.md, docs/plans/*.md,
benchmarks/results/*.md, and commit messages.

## Era Index

| Era | Dates | Key Event | Key Commit |
|-----|-------|-----------|------------|
| 1 | May 31 – Jun 02 | Pre-fair C (volatile) | earliest |
| 2 | Jun 03 – Jun 04 | Fair C benchmarks | `445733ac` |
| 3 | Jun 03 – Jun 05 | Pure-counter fold, dead-field elimination | — |
| 4 | Jul 06 | "All 22 MATCH", IR determinism | `f598584` |
| 5 | Jul 11 | Phase 3 complete | `8a827db` |
| 6 | Jul 14 | Benchmark audit, `nsw` fixes | — |
| 7 | Jul 19 | Post-migration, 23/24 MATCH | `11c0749e` |
| 8 | Jul 19 | Stabilization plans, HashMap determinism | `139c345` |
| 9 | Jul 21–27 | SLP vectorization experiments | `be6583bc` |
| 10 | Jul 27 | Post-fixes (arena-by-proof, no SLP) | `33d42397` |
| 11 | Jul 27 | Cold-path outlining Runs 1–6 | stride gate tuning |
| 12 | Jul 27 | **Baseline** — SLP stride gate, all 19 parity | `b39461e2` |
| 13 | Jul 28 | Post-baseline regressions (Phase A–I) | `edf671de` |
| 14 | Jul 28 | Recovery Steps 1–6 | `recovery-branch` |

## Era 1 — Pre-Fair C Benchmarks (May 31 – Jun 02)

C references used `volatile`, hobbling clang. BOUND=50000000. 5-iteration average.
```
╔═══════════════════════╦══════════╦══════════╦══════════╦════════╗
║ Benchmark             ║ Brief    ║ C        ║ Ratio    ║ Winner ║
╠═══════════════════════╬══════════╬══════════╬══════════╬════════╣
║ print_loop            ║ .030s    ║ .049s    ║ 1.63x    ║ Brief  ║
║ nbody_newton          ║ 3.62s    ║ 9.73s    ║ 2.70x    ║ Brief  ║
║ nbody_sqrt            ║ 6.96s    ║ 3.23s    ║ 2.15x    ║ C      ║
║ mandelbrot            ║ .74s     ║ .65s     ║ 1.14x    ║ C      ║
║ knucleotide           ║ .188s    ║ .194s    ║ 0.97x    ║ Brief  ║
║ kalman_filter_runtime ║ .161s    ║ .153s    ║ 1.05x    ║ C      ║
╚═══════════════════════╩══════════╩══════════╩══════════╩════════╝
```

## Era 2 — Fair C Benchmarks (Jun 03 – Jun 04)

C references de-volatilized. Same `-O3 -ffast-math` as Brief.
```
╔═══════════════════════╦══════════╦══════════╦══════════╦════════╗
║ Benchmark             ║ Brief    ║ C        ║ Ratio    ║ Winner ║
╠═══════════════════════╬══════════╬══════════╬══════════╬════════╣
║ iir_filter            ║ .1524s   ║ .1028s   ║ 1.48x    ║ C      ║
║ precompute_sum        ║ .0020s   ║ .0018s   ║ ~tie     ║ ~tie   ║
║ ring_buffer           ║ .0019s   ║ .0017s   ║ ~tie     ║ ~tie   ║
║ async_counters        ║ .0018s   ║ .0018s   ║ ~tie     ║ ~tie   ║
║ kalman_filter         ║ .71s     ║ .75s     ║ 0.95x    ║ Brief  ║
╚═══════════════════════╩══════════╩══════════╩══════════╩════════╝
```
Methodology: `build_and_bench.sh` now uses `date +%s.%N` + `bc` + `LC_NUMERIC=C`.

## Era 3 — Pure-Counter Fold, Dead-Field Elimination (Jun 03 – Jun 05)

Pure-counter fold eliminates 50M-iteration loops for pure bodies:
- ring_buffer: 0.11s → 0.00s (110× speedup, loop is dead without FFI)
- sparse_dispatch: .0758s → .0006s (dispatch-chain collapse)
- iir_filter: eliminated via dead-field elimination
- `term -> swan_song;` and `term!` added

Benchmarks entering folded/precomputed regime: ring_buffer, async_counters,
precompute_sum, iir_filter, sparse_dispatch.

## Era 4 — "All 22 MATCH" — commit `f598584` (Jul 06)

```
╔═══════════════════════╦══════════╦══════════╦══════════╦════════╗
║ Benchmark             ║ Brief    ║ C        ║ Ratio    ║ Winner ║
╠═══════════════════════╬══════════╬══════════╬══════════╬════════╣
║ interval_step         ║ .0006s   ║ .0622s   ║ 0.01x    ║ Brief  ║
║ bit_clear             ║ .0006s   ║ .0006s   ║ 1.00x    ║ ~tie   ║
║ mandelbrot            ║ .7236s   ║ .7232s   ║ 1.00x    ║ ~tie   ║
║ knucleotide           ║ .2006s   ║ .2004s   ║ 1.00x    ║ ~tie   ║
║ kalman_filter_runtime ║ .1844s   ║ .1836s   ║ 1.00x    ║ ~tie   ║
║ queue_drain           ║ .0627s   ║ .0621s   ║ 1.00x    ║ ~tie   ║
║ queue_drain_sym       ║ .0621s   ║ .0614s   ║ 1.01x    ║ C      ║
║ ring_buffer           ║ .0664s   ║ .0666s   ║ 0.99x    ║ Brief  ║ ★ best
║ float_math            ║ .0626s   ║ .0739s   ║ 0.84x    ║ Brief  ║
║ nbody_sqrt            ║ 2.9267s  ║ 3.2106s  ║ 0.91x    ║ Brief  ║
║ nbody_newton          ║ 7.2391s  ║ 9.4519s  ║ 0.76x    ║ Brief  ║
║ nbody_sqrt_idio       ║ 3.0738s  ║ 4.0939s  ║ 0.75x    ║ Brief  ║
║ float_math_nonzero    ║ .1877s   ║ .1809s   ║ 1.03x    ║ C      ║
║ cancel_math           ║ .0667s   ║ .0630s   ║ 1.05x    ║ C      ║
║ print_loop            ║ .0710s   ║ .0653s   ║ 1.08x    ║ C      ║
║ fasta                 ║ .2538s   ║ .2497s   ║ 1.01x    ║ C      ║
║ sparse_dispatch       ║ .1006s   ║ .0659s   ║ 1.52x    ║ C      ║
║ fannkuch_redux        ║ .1036s   ║ .0717s   ║ 1.44x    ║ C      ║
╚═══════════════════════╩══════════╩══════════╩══════════╩════════╝
```
Source: `docs/plans/2026-07-06-next-optimizations.md`.

## Era 5 — Phase 3 Complete — commit `8a827db` (Jul 11)

ALL-TIME BEST for: nbody_newton (0.75x), sparse_dispatch (0.09x), queue_drain (0.01x),
fannkuch_redux (0.96x), mandelbrot (0.99x), float_math (0.81x).

```
╔═══════════════════════╦══════════╦══════════╦══════════╦════════╗
║ Benchmark             ║ Brief    ║ C        ║ Ratio    ║ Winner ║
╠═══════════════════════╬══════════╬══════════╬══════════╬════════╣
║ ring_buffer           ║ .0686s   ║ .0676s   ║ 1.01x    ║ C      ║
║ float_math            ║ .0631s   ║ .0771s   ║ 0.81x    ║ Brief  ║ ★ best
║ float_math_nonzero    ║ .1920s   ║ .1727s   ║ 1.11x    ║ C      ║
║ sparse_dispatch       ║ .0060s   ║ .0657s   ║ 0.09x    ║ Brief  ║ ★ best
║ print_loop            ║ .0639s   ║ .0670s   ║ 0.95x    ║ Brief  ║
║ nbody_newton          ║ 7.4132s  ║ 9.8522s  ║ 0.75x    ║ Brief  ║ ★ best
║ nbody_sqrt            ║ 3.0046s  ║ 3.5218s  ║ 0.85x    ║ Brief  ║ ★ best
║ nbody_sqrt_idio       ║ 2.9578s  ║ 4.3184s  ║ 0.68x    ║ Brief  ║
║ fasta                 ║ .2695s   ║ .2636s   ║ 1.02x    ║ C      ║
║ fannkuch_redux        ║ .0763s   ║ .0789s   ║ 0.96x    ║ Brief  ║ ★ best
║ mandelbrot            ║ .7514s   ║ .7538s   ║ 0.99x    ║ Brief  ║ ★ best
║ kalman_filter_runtime ║ .1876s   ║ .1887s   ║ 0.99x    ║ Brief  ║
║ knucleotide           ║ .2093s   ║ .2060s   ║ 1.01x    ║ C      ║
║ cancel_math           ║ .0682s   ║ .0672s   ║ 1.01x    ║ C      ║
║ bit_clear             ║ .0010s   ║ .0009s   ║ 1.11x    ║ C      ║
║ queue_drain           ║ .0007s   ║ .0632s   ║ 0.01x    ║ Brief  ║ ★ best
║ queue_drain_sym       ║ .0639s   ║ .0672s   ║ 0.95x    ║ Brief  ║
║ interval_step         ║ .0009s   ║ .0669s   ║ 0.01x    ║ Brief  ║
╚═══════════════════════╩══════════╩══════════╩══════════╩════════╝
```
Source: `benchmarks/results/2026-07-11-phase3-complete.md`.

## Era 6 — Benchmark Audit (Jul 14)

Fixes: removed `#!exit` pragmas from 3 benchmarks, fixed nbody_sqrt_idio copy-paste bug,
added `nsw` to `emit_binary_op`, added `fast` to float op config templates.
No new timing table documented.

Source: `docs/plans/2026-07-14-benchmark-audit-and-fix.md`.

## Era 7 — Post-Migration Results — commit `11c0749e` (Jul 19)

23/24 MATCH (up from 16/24 pre-migration). Only UTF8_ops remains MISMATCH.

```
╔═══════════════════════╦══════════════╦══════════════╦══════════╗
║ Benchmark             ║ Phase 3 Best ║ Current      ║ Delta    ║
╠═══════════════════════╬══════════════╬══════════════╬══════════╣
║ nbody_newton          ║ 6.1s (0.75x) ║ 11.5s (1.89x)║ +152%    ║
║ mandelbrot            ║ 0.70s (C)    ║ 0.71s (1.01x)║ stable   ║
║ fannkuch_redux        ║ 0.079s (C)   ║ 0.068s (0.86x)║ improved ║
║ print_loop            ║ 0.067s (C)   ║ 0.062s (0.93x)║ improved ║
╚═══════════════════════╩══════════════╩══════════════╩══════════╝
```
nbody_newton regression: `needs_state_stores_in_body = true` stores ALL 33 state fields
each iteration (per-field phi).

Source: `benchmarks/results/2026-07-19-post-migration.md`.

## Era 8 — Benchmark Stabilization (Jul 19)

Status: Terminator bug blocking precompute_sum and async_counters_idio.
nbody_newton at 2.2x regression from Phase 3. HashMap non-determinism ~9% perf variation.
Baseline worktree strategy introduced at `be6583bc`.

Source: `docs/plans/2026-07-19-benchmark-stabilization.md`,
`docs/plans/2026-07-19-post-migration-performance.md`.

## Era 9 — SLP Vectorization Experiments (Jul 21–27)

SLP re-enabled with hazard gating and profitability checks. Baseline worktree pinned at
`be6583bc` (post-SLP anchor).

## Era 10 — Post-Fixes — commit `33d42397` (Jul 27)

ALL-TIME BEST for: nbody_sqrt_idio (0.67x), nbody_sqrt (0.85x), float_math_nonzero (0.99x),
bit_clear (0.50x), queue_drain_sym (0.97x).

Fixes applied: arena-by-proof, ABI coercion, print plugin float inference,
SLP vector emission removed, `memory(argmem: readwrite)` on main.

```
╔═══════════════════════╦══════════╦══════════╦══════════╦════════╗
║ Benchmark             ║ Brief    ║ C        ║ Ratio    ║ Winner ║
╠═══════════════════════╬══════════╬══════════╬══════════╬════════╣
║ ring_buffer           ║ .0603s   ║ .0458s   ║ 1.31x    ║ C      ║
║ float_math            ║ .0748s   ║ .0697s   ║ 1.07x    ║ C      ║
║ float_math_nonzero    ║ .1611s   ║ .1620s   ║ 0.99x    ║ Brief  ║ ★ best
║ sparse_dispatch       ║ .0551s   ║ .0604s   ║ 0.91x    ║ Brief  ║
║ print_loop            ║ .0568s   ║ .0559s   ║ 1.01x    ║ C      ║
║ nbody_newton          ║ 10.6217s ║ 7.8560s  ║ 1.35x    ║ C      ║
║ nbody_sqrt            ║ 2.2434s  ║ 2.6339s  ║ 0.85x    ║ Brief  ║ ★ best
║ nbody_sqrt_idio       ║ 2.3270s  ║ 3.4561s  ║ 0.67x    ║ Brief  ║ ★ best
║ fasta                 ║ .1987s   ║ .1980s   ║ 1.00x    ║ ~tie   ║
║ fannkuch_redux        ║ .0599s   ║ .0612s   ║ 0.97x    ║ Brief  ║
║ mandelbrot            ║ .6317s   ║ .6277s   ║ 1.00x    ║ ~tie   ║
║ kalman_filter_runtime ║ .1741s   ║ .1725s   ║ 1.00x    ║ ~tie   ║
║ knucleotide           ║ .1843s   ║ .1823s   ║ 1.01x    ║ C      ║
║ cancel_math           ║ .0599s   ║ .0582s   ║ 1.02x    ║ C      ║
║ bit_clear             ║ .0002s   ║ .0004s   ║ 0.50x    ║ Brief  ║ ★ best
║ queue_drain           ║ .0601s   ║ .0612s   ║ 0.98x    ║ Brief  ║
║ queue_drain_sym       ║ .0575s   ║ .0588s   ║ 0.97x    ║ Brief  ║
║ queue_drain_idio      ║ .0603s   ║ .0002s*  ║ 301.50x* ║ Brief  ║
║ interval_step         ║ .0599s   ║ .0592s   ║ 1.01x    ║ C      ║
╚═══════════════════════╩══════════╩══════════╩══════════╩════════╝
```
*queue_drain_idio 301.50x is a harness artifact (stale C binary).

Source: `docs/plans/2026-07-27-benchmark-regression-results.md`.

## Era 11 — Cold-Path Outlining Runs 1–6 (Jul 27)

SLP re-enabled with stride gate (max_field_stride <= 1). Run 6 = all 19 at parity.

Source: `docs/plans/2026-07-27-cold-path-refinement.md` (6 runs documented).

## Era 12 — Baseline — commit `b39461e2` (Jul 27)

SLP stride gate active. Three-category cold-path outlining active.
Print plugin emits `__print_int` FFI (NOT `PrintInt#`). No `!range`/`!prof`.
No `noundef`/`dereferenceable`. Baseline worktree at `../brief-compiler-baseline`.

```
╔═══════════════════════╦══════════╦══════════╦══════════╦════════╗
║ Benchmark             ║ Brief    ║ C        ║ Ratio    ║ Winner ║
╠═══════════════════════╬══════════╬══════════╬══════════╬════════╣
║ ring_buffer           ║ .0550s   ║ .0480s   ║ 1.14x    ║ C      ║
║ float_math            ║ .0744s   ║ .0743s   ║ 1.00x    ║ ~tie   ║
║ float_math_nonzero    ║ .1656s   ║ .1675s   ║ 0.98x    ║ Brief  ║
║ sparse_dispatch       ║ .0500s   ║ .0610s   ║ 0.81x    ║ Brief  ║
║ print_loop            ║ .0604s   ║ .0587s   ║ 1.02x    ║ C      ║
║ nbody_newton          ║ 9.0467s  ║ 8.2689s  ║ 1.09x    ║ C      ║
║ nbody_sqrt            ║ 2.7347s  ║ 2.7684s  ║ 0.98x    ║ Brief  ║
║ nbody_sqrt_idio       ║ 3.3417s  ║ 3.6030s  ║ 0.92x    ║ Brief  ║
║ fasta                 ║ .2099s   ║ .2109s   ║ 0.99x    ║ Brief  ║
║ fannkuch_redux        ║ .0653s   ║ .0657s   ║ 0.99x    ║ Brief  ║
║ mandelbrot            ║ .6569s   ║ .6528s   ║ 1.00x    ║ ~tie   ║
║ kalman_filter_runtime ║ .1813s   ║ .1790s   ║ 1.01x    ║ C      ║
║ knucleotide           ║ .1883s   ║ .1909s   ║ 0.98x    ║ Brief  ║
║ cancel_math           ║ .0626s   ║ .0614s   ║ 1.01x    ║ C      ║
║ bit_clear             ║ .0001s   ║ .0002s   ║ 0.50x    ║ Brief  ║
║ queue_drain           ║ .0623s   ║ .0612s   ║ 1.01x    ║ C      ║
║ queue_drain_sym       ║ .0618s   ║ .0611s   ║ 1.01x    ║ C      ║
║ queue_drain_idio      ║ .0624s   ║ .0618s   ║ 1.00x    ║ ~tie   ║
║ interval_step         ║ .0617s   ║ .0588s   ║ 1.04x    ║ C      ║
╚═══════════════════════╩══════════╩══════════╩══════════╩════════╝
```

## Era 13 — Post-Baseline Regressions (Jul 28) — HEAD `70ead990`

~17 commits on top of `b39461e2`. Two catastrophic regressions:
- kalman_filter_runtime: 1.01x → 3.80x (PrintInt# → #11 attr → LLVM auto-vectorizer)
- ring_buffer: 1.14x → 1.28x (stride gate + two-pass analysis)

Source: `.opencode/plans/2026-07-28-baseline-recovery.md`.

## Era 14 — Recovery Steps 1–6 (Jul 28) — `recovery-branch`

Current work-in-progress. Step 1: DataLayout int_bits + fix trunc bug. Step 2: noundef +
dereferenceable on params. Step 3: Bits→Bit rename. Step 4: !range metadata. Step 5: !prof
branch weights. Step 6: !> metadata syntax.

## All-Time Best Ratios Summary

| Benchmark | Best | Commit | Era | Notes |
|-----------|------|--------|-----|-------|
| ring_buffer | **0.99x** | `f598584` | 4 | Brief beat C once |
| float_math | **0.81x** | `8a827db` | 5 | Phase 3 era |
| float_math_nonzero | **0.98x** | `33d42397` | 10 | Post-fixes, no SLP |
| sparse_dispatch | **0.09x** | `8a827db` | 5 | Folded |
| print_loop | **0.93x** | `11c0749e` | 7 | Post-migration |
| nbody_newton | **0.75x** | `8a827db` | 5 | Phase 3 complete |
| nbody_sqrt | **0.85x** | `33d42397` | 10 | No SLP vectorizer |
| nbody_sqrt_idio | **0.67x** | `33d42397` | 10 | No stride gate |
| fasta | **0.95x** | recovery Step 5 | 14 | Latest recovery |
| fannkuch_redux | **0.96x** | `8a827db` | 5 | Phase 3 era |
| mandelbrot | **0.99x** | `8a827db` | 5 | Phase 3 era |
| kalman_filter_runtime | **0.95x** | Era 1 | 1 | Pre-SLP era |
| knucleotide | **0.97x** | Era 1 | 1 | Pre-SLP era |
| cancel_math | **0.96x** | recovery Step 1 | 14 | Latest recovery |
| bit_clear | **0.50x** | `33d42397` | 10 | Arena removal |
| queue_drain | **0.01x** | `8a827db` | 5 | Folded |
| queue_drain_sym | **0.95x** | `8a827db` | 5 | Phase 3 era |
| queue_drain_idio | **0.93x** | recovery Step 1 | 14 | Latest recovery |
| interval_step | **0.01x** | `f598584` | 4 | Folded |

Source: commit `f598584` = 2026-07-06 "All 22 MATCH".
commit `8a827db` = 2026-07-11 "Phase 3 complete".
commit `33d42397` = 2026-07-27 "Post-fixes, no SLP".
commit `b39461e2` = 2026-07-27 "Baseline — stride gate".
