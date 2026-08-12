<!-- 2026-06-10 -->

# Benchmark Fix Plan

## Priority Order

### P0 — Fix output asymmetry (verify Briev semantics are the reference)

**Problem**: 2 benchmarks (cancel_math, queue_drain) guard on `count % 5M == 0`.
Briev's `node` checks the guard against pre-tick state. C checks after
increment. This produces different first-print timing.

**Fix**: The C reference should mirror Briev's semantics. Move the guard
BEFORE the increment in C. Briev's node semantics are the reference
implementation — C benchmarks should match them.

Files: `benchmarks/cancel_math_c.c`, `benchmarks/queue_drain_c.c`

### P0 — Rebuild float benchmarks with fixed compiler

**Problem**: nbody_newton, nbody_sqrt produce `-nan`. The `constant float 0`
bug was fixed earlier in this session. These benchmarks may have been linked
from stale `.o` files before the fix.

**Fix**: Force rebuild: `rm -f benchmarks/nbody_*.ll benchmarks/nbody_*; 
./briev-compiler llvm benchmarks/nbody_newton.bv --out benchmarks --optimize-budget 2048`

### P1 — Investigate mandelbrot hang

**Problem**: `mandelbrot.bv` times out at BOUND=5. Likely the same structural
issue as the `test_mod.bv` hang: `term!` inside a guard in `node` emits
`ret` which exits `reactor_tick`, but `main` loops and re-calls it. State
never commits, infinite re-entry.

**Fix**: Restructure mandelbrot to use `term!` only at convergence (count == N),
not inside a periodic guard. Or verify the benchmark uses the correct pattern.

File: `benchmarks/mandelbrot.bv`

### P1 — Fix fannkuch_redux output mismatch

**Problem**: Briev outputs result to stderr; C returns it as exit code.
These are different mechanisms and produce different numbers (Briev "10",
C "10" as exit code coincidentally the same at BOUND=5, but will differ
at scale — the result is 10 at BOUND=5 for both, just one on stderr and
one as exit code).

**Fix**: Align to use the same mechanism. Stderr is preferred (consistent
with __print_int output convention). Update `fannkuch_redux_c.c`.

### P2 — Investigate C reference exit 6

**Problem**: `float_math_c`, `float_math_nonzero_c`, `const_heavy_c` exit
with code 6 at BOUND=5. No output produced.

**Fix**: Compile each C reference with debug symbols (`-g -O0`), run under
GDB with `BOUND=5`, find crash point. Likely missing `-lm` or a null
pointer in the runtime init path.

### P2 — queue_drain symmetric split

**Problem**: Hillel Wayne observed that queue_drain.bv and queue_drain_c.c
use different algorithms. The Briev version does more work (multiple fields,
modulo dispatch) than the C version (simple counter). They produce the same
result through different paths, which invalidates the comparison.

**Fix**: Create two benchmarks:
- `queue_drain_sym.bv` — mirrors C step-for-step using Briev features
- `queue_drain_idio.bv` — Briev-native pattern (contract-proven, reactive)
Both verified to produce identical output for the same input.

## Verification

After each fix:

1. `BOUND=5` run — confirm Briev and C produce identical stderr output
2. `BOUND=50000000` run — confirm outputs match at scale
3. `cargo test --lib` — all tests must pass
4. Record any new diagnostics (A000–A005) observed during compilation

## Non-Goals

- Fixing the `term!` in guard structural issue (`reactor_tick` exit vs
  program exit). This is a compiler bug that affects all programs using
  `term!` inside a guard in `node`. Documented in BUGS.md (2026-06-07).
  Fix deferred — the benchmark fix is to not use `term!` in guards.

- Adding new benchmarks (spectral-norm, binary-trees). Deferred to next
  cycle.

- `const_heavy` — this is an optimizer benchmark with all-const inputs.
  It doesn't produce output at any BOUND. The C reference exit 6 is the
  only issue.
