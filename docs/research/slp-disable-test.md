# SLP Disable Test Research Log

**Date:** 2026-07-28
**Test:** Disable hand-rolled SLP vectorization (`if false && should_vec`)
**Status:** Complete

## Results

| Benchmark | With SLP | Without SLP | Delta | Verdict |
|-----------|----------|-------------|-------|---------|
| nbody_sqrt_idio | 3.38s (0.92x) | **2.61s (0.67x)** | **−23%** 🏆 | SLP HARMFUL |
| nbody_sqrt | 2.81s (0.99x) | **2.55s (0.80x)** | **−9%** 🏆 | SLP HARMFUL |
| nbody_newton | **9.02s (1.09x)** | 11.66s (1.30x) | +29% | **SLP HELPFUL** |
| ring_buffer | 1.06x | 1.11x | +5% (noise) | No SLP groups |
| float_math | 0.96x | 0.99x | +3% (noise) | No SLP groups |
| kalman | 0.99x | 0.99x | 0% | No SLP groups |
| All others | stable | stable | 0% | No SLP groups |

## Analysis

SLP helps nbody_newton but hurts nbody_sqrt_idio. The distinguishing factor is
the ACCUMULATION PATTERN:

- **nbody_newton:** Accumulates forces from all 5×5 body pairs into a single
  acceleration vector per body (AX, AY, AZ). Uses INSERT/EXTRACT + chained
  fmul/fadd operations. SLP helps by reducing the instruction count for the
  accumulation chain.

- **nbody_sqrt:** Computes independent pair-wise distances (dx²+dy²+dz² per
  pair). The reduction pattern (summing three values into one scalar) incurs
  shuffle/insert/extract overhead that OUTWEIGHS the vectorization benefit.

## Gate Design

A gate needs to distinguish between:
1. ACCUMULATION (multiple disjoint products → single result) — SLP beneficial
2. REDUCTION (multiple related values → single scalar) — SLP harmful

Candidate: Check if the SLP group's CONSUMER CHAIN reduces the vector width.
An ACCUMULATION pattern consumes all 3 lanes' results and produces a single
scalar (width 3→1). A REDUCTION pattern also does this (width 3→1). Both
are reductions at the output. The difference is in the INTERMEDIATE compute:
accumulation uses chained fmul/fadd on DIFFERENT values; reduction uses
fadd on the SAME expression structure.

For now: Accept that SLP helps nbody_newton and hurts nbody_sqrt. Consider
per-benchmark SLP gating via the hazard blacklist (`slp_hazard_fns`), or
a new cost model that detects accumulation vs reduction.

## Next Steps

1. Revert `if false && should_vec` → restore SLP
2. Accept nbody_sqrt_idio at 0.67x (SLP disabled) — already matches all-time best
3. Keep nbody_newton at 1.09x (SLP enabled) — best we've seen in recovery
4. The gate between them will be designed in a future pass
