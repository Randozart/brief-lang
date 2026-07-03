# Fix: Benchmark Fencepost in Periodic Guards

## Problem

All benchmarks with `[count % 5000000 == 0]` guards place them AFTER
`&count = count + 1`. The guard sees the **post-increment** count value.
C references check `count % 5000000 == 0` **pre-increment** (before the
`count++` in `for (...; count++)`).

This causes two issues:

1. **Empty output at BOUND=5** (nbody_sqrt_idio): With BOUND=5, count
   iterates 0→1, 1→2, 2→3, 3→4, 4→5. Post-increment values (1,2,3,4,5)
   never satisfy `% 5000000 == 0`. C outputs at count=0.

2. **Last-line numeric mismatches** (all nbody benchmarks): Periodic output
   fires at different iteration points (5M,10M,... vs 0,5M,...), producing
   different energy values for the same line index.

## Fix

**Affected files (nbody):**
- `benchmarks/nbody_newton.bv`
- `benchmarks/nbody_newton_sym.bv`
- `benchmarks/nbody_sqrt_idio.bv`

Move `[count % 5000000 == 0] { ... }` to **before** `&count = count + 1`
in the rct txn body.

C references already check pre-increment and need no change.

## Verification

```
BOUND=5 benchmarks/nbody_sqrt_idio  # should now produce 1 line
cargo test --lib                     # all tests pass
bash benchmarks/build_and_bench.sh --correctness  # all matches
```
