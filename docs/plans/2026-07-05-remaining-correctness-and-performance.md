# Remaining Correctness and Performance Gaps

Date: 2026-07-05
Status: Execution
After: 6529f29 (Phases A-C: IR fixes + precomputation + nbody_newton)
Updated: seed_observable_idents fix resolved float_math MISMATCH too.

## 1. Correctness Status

| Benchmark | Status |
|-----------|--------|
| ring_buffer | MATCH |
| float_math | **MATCH** (was MISMATCH, fixed by seed_observable_idents) |
| float_math_nonzero | **MATCH** (was MISMATCH, fixed by seed_observable_idents) |
| sparse_dispatch | MISMATCH (pre-existing, modulo-switch dispatch) |
| print_loop | MATCH |
| nbody_newton | **MATCH** (was MISMATCH, fixed by Phase C) |
| nbody_sqrt | **MATCH** (was MISMATCH, fixed by seed_observable_idents) |
| nbody_sqrt_idio | **MATCH** (was MISMATCH, fixed by seed_observable_idents) |
| fasta | MISMATCH (pre-existing, output format) |
| fannkuch_redux | MATCH |
| mandelbrot | MATCH |
| kalman_filter_runtime | **MATCH** (was MISMATCH, fixed by seed_observable_idents) |
| knucleotide | MATCH |
| cancel_math | MATCH |
| bit_clear | MISMATCH (pre-existing) |
| queue_drain | MATCH |
| queue_drain_sym | MATCH |
| interval_step | MATCH |

## 2. Performance Gaps

### 2.1 fannkuch_redux (1.65x → target ~1.00x)

**Root cause:** 12-element circular phi chain (p0←p1←...←p11←p0) prevents
LLVM's SCEV from analyzing the loop. All 12 phis become SCEVUnknown, blocking
loop unrolling and dependence analysis.

**Fix:** Decompose the 12-cycle into 4 independent 3-cycles:
1. Detect rotation patterns in emit_countable_latch
2. Change backedge step from 1 to 4
3. Increment counter by 4 per loop trip
4. Unroll body 4x per trip
5. Add remainder loop for N%4 iterations

C's clang already does exactly this — unrolling by 4 creates the same
3-cycle structure.

### 2.2 Remaining gaps (secondary priority)

| Benchmark | Current | Note |
|-----------|---------|------|
| nbody_sqrt | 1.29x | 6 scalar phi nodes unvectorized |
| mandelbrot | 1.12x | close to C |
| nbody_newton | 0.92x | **beats C by 8%** |
| float_math | 0.84x | best-known 0.67x was folded-loop artifact |
