# nbody_sqrt Symmetric/Idiomatic Split + knucleotide Fix

**Date:** 2026-06-16  
**Author:** OpenCode  
**Status:** Plan — implementation in progress

## Root Cause

The `[count == N] { term! -> print_int#(nchksum); }` guard in knucleotide.bv produces
an extra post-convergence print that the C reference (`knucleotide_c.c`) does not have.
The C only prints periodically inside the loop — there is no `fprintf` after the loop.

For nbody_sqrt, the Briv version computes energy every iteration (10 extra `sqrt#` calls
per tick) and prints periodically, while the C reference computes energy only once after
the loop and prints once. This creates two differences:
1. Briv outputs an extra periodic line at count=0 that C doesn't have
2. The final energy values differ slightly due to FP accumulation order

## Changes

### 1. knucleotide.bv — Remove post-convergence print
**File:** `benchmarks/knucleotide.bv`  
**Change:** Delete line 25:
```briv
[count == N] { term! -> print_int#(nchksum); };
```
C reference already matches: prints only every 5M inside the loop, no post-loop print.

### 2. nbody_sqrt.bv — Make symmetric (compute energy once)
**File:** `benchmarks/nbody_sqrt.bv`  
**Change:** Match the C reference step-for-step.

Remove from the main body:
- Lines 171-198: redundant energy computation (`edist01` through `energy`):
  These recompute `sqrt#(dsq)` for all 10 body pairs, duplicating the sqrt work
  already done in lines 64-123.
- Lines 200-202: periodic print `[count % 5000000 == 0] { print_float#(energy); }`

Add INSIDE the `[count == bound]` guard (currently lines 203-205):
```briv
[count == bound] {
    let edist01: Float = sqrt#((bx0 - bx1)*(bx0 - bx1) + (by0 - by1)*(by0 - by1) + (bz0 - bz1)*(bz0 - bz1));
    let edist02: Float = sqrt#((bx0 - bx2)*(bx0 - bx2) + (by0 - by2)*(by0 - by2) + (bz0 - bz2)*(bz0 - bz2));
    let edist03: Float = sqrt#((bx0 - bx3)*(bx0 - bx3) + (by0 - by3)*(by0 - by3) + (bz0 - bz3)*(bz0 - bz3));
    let edist04: Float = sqrt#((bx0 - bx4)*(bx0 - bx4) + (by0 - by4)*(by0 - by4) + (bz0 - bz4)*(bz0 - bz4));
    let edist12: Float = sqrt#((bx1 - bx2)*(bx1 - bx2) + (by1 - by2)*(by1 - by2) + (bz1 - bz2)*(bz1 - bz2));
    let edist13: Float = sqrt#((bx1 - bx3)*(bx1 - bx3) + (by1 - by3)*(by1 - by3) + (bz1 - bz3)*(bz1 - bz3));
    let edist14: Float = sqrt#((bx1 - bx4)*(bx1 - bx4) + (by1 - by4)*(by1 - by4) + (bz1 - bz4)*(bz1 - bz4));
    let edist23: Float = sqrt#((bx2 - bx3)*(bx2 - bx3) + (by2 - by3)*(by2 - by3) + (bz2 - bz3)*(bz2 - bz3));
    let edist24: Float = sqrt#((bx2 - bx4)*(bx2 - bx4) + (by2 - by4)*(by2 - by4) + (bz2 - bz4)*(bz2 - bz4));
    let edist34: Float = sqrt#((bx3 - bx4)*(bx3 - bx4) + (by3 - by4)*(by3 - by4) + (bz3 - bz4)*(bz3 - bz4));
    let e01: Float = m0 * m1 / edist01;
    let e02: Float = m0 * m2 / edist02;
    let e03: Float = m0 * m3 / edist03;
    let e04: Float = m0 * m4 / edist04;
    let e12: Float = m1 * m2 / edist12;
    let e13: Float = m1 * m3 / edist13;
    let e14: Float = m1 * m4 / edist14;
    let e23: Float = m2 * m3 / edist23;
    let e24: Float = m2 * m4 / edist24;
    let e34: Float = m3 * m4 / edist34;
    let ep: Float = -(e01 + e02 + e03 + e04 + e12 + e13 + e14 + e23 + e24 + e34);
    let ek0: Float = 0.5 * m0 * (vx0 * vx0 + vy0 * vy0 + vz0 * vz0);
    let ek1: Float = 0.5 * m1 * (vx1 * vx1 + vy1 * vy1 + vz1 * vz1);
    let ek2: Float = 0.5 * m2 * (vx2 * vx2 + vy2 * vy2 + vz2 * vz2);
    let ek3: Float = 0.5 * m3 * (vx3 * vx3 + vy3 * vy3 + vz3 * vz3);
    let ek4: Float = 0.5 * m4 * (vx4 * vx4 + vy4 * vy4 + vz4 * vz4);
    let energy: Float = ep + ek0 + ek1 + ek2 + ek3 + ek4;
    term! -> print_float#(energy);
};
```

This makes the Briv compute energy exactly once, at convergence — matching C's
post-loop EPAIR section. Output: one line (the periodic print at count=0 is removed).

### 3. nbody_sqrt_idio.bv + _c.c — New idiomatic pair
Creates a variant where both Briv and C compute energy every iteration.

**New file:** `benchmarks/nbody_sqrt_idio.bv`
- Same body as current nbody_sqrt.bv (energy computed in-body every tick)
- Periodic print at `[count % 5000000 == 0] { print_float#(energy); }`
- NO final `[count == bound]` print (periodic print already prevents fold elimination)
- Uses `print_float#(energy)` — already migrated

**New file:** `benchmarks/nbody_sqrt_idio_c.c`
- Same loop body as nbody_sqrt_c.c (PAIR macros, position updates)
- ADD in-loop energy computation + fprintf every 5M matching the Briv
- REMOVE post-loop energy computation (the in-loop print is the final one)

```c
for (count = 0; count < total; count++) {
    // PAIR(0,1)... PAIR(3,4)  (same as symmetric)
    // position updates (same as symmetric)
    // ENERGY COMPUTATION every tick (new — matches Briv idiomatic)
    float energy = 0.0f;
    #define EPAIR(ia, ib) { \
        float dx = bx[ia] - bx[ib]; \
        float dy = by[ia] - by[ib]; \
        float dz = bz[ia] - bz[ib]; \
        float dsq = dx*dx + dy*dy + dz*dz; \
        energy -= m[ia] * m[ib] / sqrtf(dsq); \
    }
    EPAIR(0,1) EPAIR(0,2) EPAIR(0,3) EPAIR(0,4)
    EPAIR(1,2) EPAIR(1,3) EPAIR(1,4)
    EPAIR(2,3) EPAIR(2,4)
    EPAIR(3,4)
    #undef EPAIR
    for (int i = 0; i < 5; i++)
        energy += 0.5f * m[i] * (vx[i]*vx[i] + vy[i]*vy[i] + vz[i]*vz[i]);
    if (count % 5000000 == 0)
        fprintf(stdout, "%.9f\n", energy);
} // end for — NO post-loop energy computation
```

### 4. nbody_newton_sym.bv — Migrate old-style prints
**File:** `benchmarks/nbody_newton_sym.bv`  
**Change:** Replace `__print_float(energy)` → `print_float#(energy)` (two occurrences).

### 5. Harness update
**File:** `benchmarks/build_and_bench.sh`  
Add nbody_sqrt_idio to the benchmark lists:
```bash
TAG[nbody_sqrt_idio]=runtime
nbody_sqrt_idio) budget=2048 ;;
    nbody_sqrt_idio) extra_flags="-lm" ;;
```

## Verification
1. `cargo test --lib` — all tests pass
2. `cargo build --release --bin briv-compiler` — no warnings
3. Build + run each changed benchmark:
   ```bash
   cargo build --release --bin briv-compiler
   ./target/release/briv-compiler llvm benchmarks/knucleotide.bv --out /tmp
   clang -O3 -march=native -ffast-math /tmp/knucleotide.ll -o /tmp/knucleotide -lm
   diff <(env BOUND=50000000 /tmp/knucleotide) <(env BOUND=50000000 ./benchmarks/knucleotide_c)
   # same for nbody_sqrt, nbody_sqrt_idio
   ```
4. Full benchmark run: `bash benchmarks/build_and_bench.sh --correctness`
