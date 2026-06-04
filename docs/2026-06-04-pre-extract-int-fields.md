# Pre-extract Int Fields in SSA Mode — Fix Int Benchmark Overhead

**Date:** 2026-06-04
**Status:** Implementing

## Root Cause

Struct-SSA mode pre-extracts Float fields once before the loop body (via
`pre_extract_float_fields()`), caching them in `ssa_old_float_regs`. Every
subsequent read of a Float field hits the cache — zero extra extractvalue ops.

Int fields have no equivalent. Every `&seed = seed * IA` reads `seed` via a
fresh `extractvalue %State %inXX, 2` from the latest insertvalue chain. With
11 Int fields × 4 unroll iterations × ~5 reads per field = ~200 extractvalue
operations in the unrolled body.

Float_math has 145 extractvalue for the same 11 fields and same 4× unroll.
Mandelbrot has 212. The ~70 extra ops are all Int field re-extractions from
the insertvalue chain.

## Plan

1. Add `ssa_old_int_regs` alongside existing `ssa_old_float_regs`
2. Add `pre_extract_int_fields()` — mirrors `pre_extract_float_fields()`
3. Call after `pre_extract_float_fields()` at all 4 body emission sites
4. Handle cached reads in `Expr::Identifier` alongside the Float cache check
5. Clear alongside Float cache at all reset sites

## Expected

| Before | After |
|--------|-------|
| ~212 extractvalue/insertvalue | ~40 |
| mandelbrot 1.14× C | ~0.9× C |
| All Int-heavy benchmarks get proportional improvement |
