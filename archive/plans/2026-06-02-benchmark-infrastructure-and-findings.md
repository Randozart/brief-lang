# Benchmarking Infrastructure & ASR Findings (2026-06-02)

## Three Methodology Issues Fixed

### 1. Dead-Field Elimination Eating Benchmarks

`float_math` and `float_math_nonzero` had `#!exit count == total` — only `count` was live. The dead-field elimination pass classified ALL float fields (`x0`, `x1`, `x2`, `p00`…`p22`) as dead, collapsing the entire 50M-iteration body to O(1) `store i64 50000000`. C references actually ran the loop. This created a false Brief-wins impression.

**Fix**: Added `&& x0 >= 0.0` to both exit conditions. The trivially-true guard forces live-field analysis to mark `x0` live, which transitively pulls in the fields that chain-depend on `x0` in the matrix multiply body.

### 2. Nanosecond Timing Harness

`date +%s.%N` + `bc` gave single-shot wall-clock times unreliable at the millisecond scale. Replaced with a C fork+exec harness using `CLOCK_MONOTONIC` giving microsecond precision (0.000001s).

**Harness**: `benchmarks/build_and_bench.sh` now auto-compiles and caches `/tmp/brief_bench_timer` on first run.

### 3. 5-Iteration Averaging

Single-shot timing was dominated by exec() + page fault noise. Each benchmark now runs 5 times with min/max/average reported.

## Wrong Assumptions Debunked

| Assumption | Reality |
|-----------|---------|
| "iir_filter regressed from 0.000s to 0.188s" | Stale binary from pre-dead-field-elim commit. O(1) fold intact. Fixed bench script to always rebuild. |
| "float_math/nonzero are O(1) — Brief beats C" | Dead-field elim ate the body. C actually runs the 50M loop. Brief falls behind when the loop isn't folded. |
| "SLP vectorization benefits are net positive" | Only when ops/field ≥ 1.5. Below that, packing shuffles (Port 5) cost more than parallel throughput saves. ASR gate added. |
| "Loop unrolling will close the gap" | Only 4% improvement. The body instructions are the bottleneck, not loop overhead. |
| "Remaining gap is phi-scheduling µarch" | The gap is struct alloca+insertvalue/extractvalue overhead inside the loop. not the phi nodes themselves. |

## Current Benchmark State (5-iteration avg, CLOCK_MONOTONIC)

| Benchmark | Brief | C | Ratio | Who wins |
|-----------|-------|---|-------|----------|
| iir_filter | 0.0333s | 0.1526s | 0.21x | **Brief** (O(1) fold) |
| precompute_sum | 0.0009s | 0.0005s | 1.80x | startup noise |
| ring_buffer | 0.0006s | 0.0006s | 1.00x | ~tie (O(1) fold) |
| async_counters | 0.0005s | 0.0006s | 0.83x | ~tie (O(1) fold) |
| **float_math** | 0.0161s | 0.0066s | **2.43x** | C |
| **float_math_nonzero** | 0.5737s | 0.2431s | **2.35x** | C |
| sparse_dispatch | 0.0018s | 0.0011s | 1.63x | startup noise |
| const_heavy | 0.0007s | 0.0548s | 0.01x | **Brief** (7x faster) |

Only two real gaps remain — both from struct overhead in the folded-loop body. Zero SLP packing ops survive in .opt.ll for both.

## Root Cause of the 2.4× Gap

The folded-loop body emits `alloca %State` + `load %State` + `extractvalue`/`insertvalue` chains inside the loop. `opt -O3` runs SROA which decomposes these, but the resulting scalar register lifetimes are wider than C's hand-coded local variables. The allocation lives in the loop preheader and loads/stores to it on every iteration's backedge, creating artificial memory traffic that C doesn't have.

## Next Step: Idea D — Register-Resident State

Move `@global_state` from a module-level global to `alloca %State` in `main()`, passed as `noalias nocapture %State*` to all functions. This gives `opt -O3` maximum alias freedom to eliminate the alloca entirely and promote all fields to scalar registers for the entire tick loop duration.
