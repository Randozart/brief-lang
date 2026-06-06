# Benchmark Implementation Plan — 2026-06-05

## Execution Log

**2026-06-05 16:30 UTC** — Full Tiers execution begun. AGENTS_HISTORY.md "LLVM Backend Gaps" section found to be severely outdated — StructInstance, FieldAccess, `<-` arrows, all 13 projection targets, Tuple/TupleDestructure, Slice/MultiSlice are all fully implemented in LLVM backend. Tier 2 (spectral-norm, binary-trees) is now unblocked.

**Status:** In progress
**Depends on:** Halting pattern compiler passes (committed in `c727555` — P1 fix, algebraic simplify, popcount decay, collection drain, interval bounds, lexicographic ranking)

## Context

All 5 halting pattern compiler passes are committed. What's missing are the benchmark `.bv` files, `_c.c` references, and harness registration that exercise them and verify termination + symmetric performance.

## Pending Benchmarks

### Tier 1 — Halting pattern benchmarks (4 new pairs)
| # | File | Pattern | Halting Pass | Est. Lines |
|---|------|---------|-------------|-----------|
| 1 | `cancel_math` | `&x = x + (R+1) - R` → `x + 1` | `simplify_body`/`detect_increments` | 40 .bv + 30 .c |
| 2 | `bit_clear` | `® = reg & (reg - 1)` popcount decay | `detect_popcount_decay` | 30 .bv + 20 .c |
| 3 | `queue_drain` | `x <- &queue` with `:> Size` drain | `detect_collection_drain` | 50 .bv + 40 .c |
| 4 | `interval_step` | `&x = (x + R1) - R2` net step ≥ 1 | `detect_increments` interval arm | 40 .bv + 30 .c |

### Tier 2 — CLBG gap benchmarks (2 new pairs)
| # | File | Pattern | LLVM Gap |
|---|------|---------|----------|
| 5 | `binary-trees` | Struct pool + index tree walk | Struct/FieldAccess stubs |
| 6 | `spectral-norm` | Float arrays at N=5500 scale | Collection/runtime alloc |

### Tier 3 — Existing benchmarks needing harness runs (6 existing)
| # | File | Status |
|---|------|--------|
| 7 | `fasta` | .bv exists, never run through harness |
| 8 | `fannkuch_redux` | .bv exists, never run through harness |
| 9 | `mandelbrot` | .bv exists, never run through harness |
| 10 | `knucleotide` | .bv exists, never run through harness |
| 11 | `nbody_sqrt` | P1 fix committed, needs verification |
| 12 | `nbody_newton` | .bv exists, never run through harness |

## Execution Order

1. **Create Tier 1 .bv files + _c.c files** (this sprint)
2. **Register all in `benchmarks/build_and_bench.sh`** (this sprint)
3. **Build compiler** and run `cargo test --lib` for regression check
4. **Run `build_and_bench.sh`** for Tier 1 benchmarks
5. **Fix any compilation/runtime failures** and document in BUGS.md
6. **Document results** (trophy folder update if Brief beats C)
7. **Defer** Tier 2 (CLBG) and Tier 3 (harness runs) to next sprint

## Designs

### cancel_math.bv
Tests that `simplify_body` reduces `x + (R+1) - R` → `x + 1`.
```
#!exit count == N && acc >= 0;
const R: Int = 100;
rct txn step [count < N][count == N] {
    &acc = acc + count;
    &count = count + (R + 1 - R);
    [count % 5000000 == 0] { __print_int(acc); };
    term;
};
```
C reference: counter loop with symmetric print.

### bit_clear.bv
Tests `detect_popcount_decay` on `reg & (reg - 1)` pattern.
Limited to popcount(initial_reg) iterations (63 for i64::MAX).
```
const initial_reg: Int = 0x7FFFFFFFFFFFFFFF;
rct txn clear [reg != 0][reg == 0] {
    &reg = reg & (reg - 1);
    [reg % 1000000 == 0] { __print_int(reg); };
    term;
};
```
C reference: same bit-clear loop, 63 iterations.

### queue_drain.bv
Tests `detect_collection_drain` on `x <- &queue` with `:> Size` precondition.
Two-phase: fill then drain via concurrent transactions.
```
#!exit push_count == N && queue :> Size == 0;
rct txn fill [push_count < N][push_count == N] { &queue <- push_count; &push_count = push_count + 1; term; };
rct txn drain [queue :> Size > 0][queue :> Size == 0] { let x: Int <- &queue; term; };
```
C reference: push/pop loop, N iterations.

### interval_step.bv
Tests that `detect_increments` handles `(x + R1) - R2` where R1 - R2 = 1.
```
#!exit count == N && acc >= 0;
const R1: Int = 200;
const R2: Int = 199;
rct txn step [count < N][count == N] {
    &acc = acc + count;
    &count = (count + R1) - R2;
    [count % 5000000 == 0] { __print_int(acc); };
    term;
};
```
C reference: counter loop with symmetric print.

## Verification
- `cargo test --lib` — 434+ pass (no regressions)
- Each .bv compiles via `brief-compiler llvm benchmarks/<name>.bv --out benchmarks --optimize-budget 256`
- Each binary runs to completion with `env BOUND=50000000`
- Symmetric C reference exits with same return code
