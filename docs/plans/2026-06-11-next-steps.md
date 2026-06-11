# Next Steps

**Date:** 2026-06-11
**Author:** Performance Investigation

## Overview

Four remaining work items:

1. **Architecture docs** — write what exists before changing anything further
2. **Phase 1: Symmetric benchmarks** — create `_sym` variants of precomputed benchmarks
3. **Intrinsic design (`name#()`)** — replace `as intrinsic` with native syntax
4. **Phase 2: SLP hazard fix** — liveness tracking for accurate register pressure

## Step 1: Architecture Docs

Write current state docs for:
- A006 dispatch path (direct SSA loop vs reactor_tick)
- Struct-vs-phi root cause
- R2/R3: float boxing elimination, per-field GEP loops, copy elimination
- Benchmark results table

## Step 2: Symmetric Benchmarks

Create `_sym` copies of nbody_newton and fannkuch_redux with periodic FFI calls
matching the C reference patterns. Keep originals for pure-form reference.

## Step 3: Intrinsic Design — `name#()` syntax

Replace `as intrinsic "llvm.*"` with compiler-known `name#(args)` syntax.
Intrinsics dispatch to the best available implementation per target
(LLVM intrinsic, Rust native, or circuit for FPGA).

## Step 4: SLP Hazard Fix

Replace `max_float_temps` with liveness interval tracking in hazard.rs.
Currently overestimates register pressure by counting all float temps as
simultaneously live.
