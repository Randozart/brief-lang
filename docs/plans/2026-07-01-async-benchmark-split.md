# 2026-07-01: Async Benchmark Split (sym / idio)

## Problem

`async_counters` benchmark produces misleading timing because its C reference
was changed (ad67b83) from a real 2-thread program to a fold-trivial version
(`long g_a = 25000000L; (void)g_a;`), while the Briev body gained `print_int#()`
guards for observability, making it non-pure. The compiler correctly chooses the
thread pool path (barriers) for impure bodies, resulting in ~500s runtime vs C's
~0.001s — a meaningless comparison.

## Solution

Split into two benchmarks per the `_sym`/`_idio` convention:

### `async_counters_sym` (runtime tag)
- **Briev**: `print_int#(a)` guards in body (observable, non-pure)
- **C reference**: Real 2-thread pthread_barrier program with matching prints
- **What it tests**: "Does Briev's thread pool throughput match C's pthread?"
- **Expected**: Both ~500s at BOUND=50e6 — fair comparison

### `async_counters_idio` (optimizer tag)
- **Briev**: Pure bodies (no prints), multi-txn pure fold → O(1) register pipeline
- **C reference**: Current trivial version (clang folds to `long x = N`)
- **What it tests**: "Does Briev fold pure async loops as aggressively as C?"
- **Expected**: Both O(1) — fair comparison

### Old `async_counters` removed
Replaced by the two new benchmarks in `build_and_bench.sh`.

## Compiler state (no changes needed)

The compiler already handles both cases correctly:

| Case | Body type | Path | Runtime |
|------|-----------|------|---------|
| Sym (with print_int) | Non-pure (FFI in body) | Thread pool + barriers (mod.rs:2387) | O(N) |
| Idio (pure) | Pure body | Multi-txn pure fold (mod.rs:2224) | O(1) |

The distinction is at `mod.rs:2194`: `node.is_pure_body || node.is_effectively_pure`.
With print_int → false → thread pool. Without → true → pure fold.

## Files changed

| File | Action |
|------|--------|
| `benchmarks/async_counters_sym.bv` | New — runtime version with print_int guards |
| `benchmarks/async_counters_sym_c.c` | New — 2-thread C reference with barriers |
| `benchmarks/async_counters_idio.bv` | New — optimizer version (pure bodies) |
| `benchmarks/async_counters_c.c` | Unchanged — C ref for idio (trivial fold) |
| `benchmarks/build_and_bench.sh` | Replace `async_counters` with sym + idio |

## Verification

1. `cargo test --lib` — all pass
2. `bash benchmarks/build_and_bench.sh --correctness` — both new benchmarks MATCH
3. `bash benchmarks/build_and_bench.sh --optimizer` — async_counters_idio shows O(1)
4. `bash benchmarks/build_and_bench.sh --single async_counters_sym` (quick check, BOUND=5)
