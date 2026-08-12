# 2026-06-05 — Benchmark Implementation Sprint

## Summary

Created 4 new halting-pattern benchmark pairs (.bv + _c.c), registered in
`build_and_bench.sh`. Fixed an LLVM FFI attribute bug (`memory(argmem: write)`)
that caused `__print_int` elimination in small loops. Updated AGENTS.md with
"Precomputation is Correct" doctrine. Documented 3 bugs in BUGS.md. Added
3 new trophies.

## New Benchmarks

| File | Pattern | Halting Pass | Bound | Iterations |
|------|---------|-------------|-------|------------|
| `cancel_math.bv` | `x + (R+1) - R` → `x+1` | `simplify_body` + `detect_increments` | `__get_env_int` | 50M |
| `bit_clear.bv` | `reg & (reg - 1)` popcount | `detect_popcount_decay` | Compile-time (63) | 63 |
| `queue_drain.bv` | `<- &queue` collection ops | `detect_collection_drain` + counter | `__get_env_int` | 50M |
| `interval_step.bv` | `(x + R1) - R2` net step ≥ 1 | `detect_increments` interval arm | `__get_env_int` | 50M |

## Results

| Benchmark | Briev | C | Ratio | Winner |
|-----------|-------|---|-------|--------|
| **cancel_math** | 0.0410s | 0.0555s | **0.73×** | Briev |
| **bit_clear** | 0.0008s | 0.0006s | 1.33× | C (63 iters, negligible) |
| **queue_drain** | 0.0423s | 0.0491s | **0.86×** | Briev |
| **interval_step** | 0.0583s | 0.0591s | **0.98×** | Parity |
| print_loop | 0.0364s | 0.0518s | 0.70× | Briev (baseline) |
| nbody_newton | 19.7s | 8.57s | 2.30× | C (pre-existing sqrtf wrapper) |

## Bug Fixes

### 1. Unused `io_pending` import forces reactor (06-05)

**File**: `bit_clear.bv`, `queue_drain.bv`
**Root Cause**: Importing `io_pending` from `std/briev_rt.bv` without referencing
it in any precondition activates the reactive runtime (`__rt_wait()` 100ms per
tick), even for pure-state convergence.
**Fix**: Removed unused imports. Benchmarks now run in SSA mode (tight while-loop).

### 2. Low print modulo on short benchmarks (06-05)

**File**: `bit_clear.bv`
**Root Cause**: Print guard `[reg % 1000000 == 0]` never fires in 63 iterations.
**Fix**: Lowered to `[reg % 100000 == 0]` — ensures at least `reg=0` fires.

### 3. `memory(argmem: write)` FFI attribute eliminates IO calls (06-05)

**File**: `src/backend/llvm.rs:1344`
**Root Cause**: Attribute #1 = `{ ... memory(argmem: write) }` told LLVM all FFI
functions only write through pointer arguments. For `__print_int(i64)` with no
pointer args, LLVM concluded "writes nothing" and eliminated the call (dead
return value + `willreturn`).
**Fix**: Removed `memory(argmem: write)` from attribute #1. The conservative
default (no memory restriction) lets LTO's FunctionAttrs infer correct attributes
from actual function bodies.

## AGENTS.md

Added "Precomputation is Correct, Not a Bug" subsection under Benchmark
Philosophy. Documents that compile-time-known bounds WILL be precomputed, and
the fix is to make bounds runtime-determined via `__get_env_int("BOUND")`.

## Trophies

Three new trophies added:

| Trophy | Ratio | Why |
|--------|-------|-----|
| `trophies/cancel_math/` | 0.73× | Algebraic simplify + LTO FFI inlining |
| `trophies/queue_drain/` | 0.86× | Inline collection ops + unified folded loop |
| `trophies/interval_step/` | 0.98× | Interval bounds detection parity |

## Git History

```
7d354e3 Term variants, swan song, assume pragmas, docs update, AGENTS.md split
372f30f Projection operator :> complete: parser, stdlib, docs, alka/on-exit disabled
c727555 Halting patterns: P1 fix, algebraic simplify, popcount, collection drain, ...
[HEAD] Halting pattern benchmarks + FFI attri fix + docs
```

## File Manifest

### New files
- `benchmarks/cancel_math.bv` + `benchmarks/cancel_math_c.c`
- `benchmarks/bit_clear.bv` + `benchmarks/bit_clear_c.c`
- `benchmarks/queue_drain.bv` + `benchmarks/queue_drain_c.c`
- `benchmarks/interval_step.bv` + `benchmarks/interval_step_c.c`
- `trophies/cancel_math/` (8 files + README.md)
- `trophies/queue_drain/` (8 files + README.md)
- `trophies/interval_step/` (8 files + README.md)
- `.opencode/plans/2026-06-05-benchmark-implementation.md`
- `reports/2026-06-05-benchmark-implementation.md`

### Modified files
- `benchmarks/build_and_bench.sh` — added 4 new benchmarks to BENCHMARKS
- `src/backend/llvm.rs:1344` — removed `memory(argmem: write)` from #1
- `AGENTS.md` — added Precomputation is Correct doctrine
- `BUGS.md` — 3 new bug entries
- `trophies/README.md` — added 3 new trophies