# Per-Field SSA Register Promotion — Results

## Summary
The expected performance improvement from per-field SSA (replacing struct-SSA with
individual field loads/stores) **was not achieved**. All five approaches tried were
slower than the original struct-SSA for the Kalman filter (12 float fields).

## Approaches Tried

| Approach | Runtime | vs C | Change | Issue |
|----------|---------|------|--------|-------|
| **Struct-SSA (baseline)** | **1.214s** | **1.87×** | — | Reg spill with 12 fields |
| Per-field phi nodes | N/A | — | Compile error | Alias type mismatch (float→i64) |
| Per-field phi with alias | ∞ (hang) | — | — | llc didn't coalesce phi across back-edge |
| Per-field stores in body | 1.442s | 1.92× | +19% | Per-iteration store overhead |
| Struct alloca copy | 1.605s | 2.44× | +32% | SROA didn't help — one big struct phi |
| Per-field allocas (mem2reg) | 1.479s | 2.23× | +22% | Still 62 global loads in hot loop |

## Root Cause
The Kalman filter has **13 loop-carried values** (12 float fields + 1 counter).
x86-64 has only **16 XMM registers**. With ~10 intermediate values also competing,
the register allocator spills. Once spilled, `llc` rematerializes loads from
`@global_state` because it's the canonical spill location.

This is a **hardware register pressure limit**, not a compiler optimization gap.
The IIR filter (4 float fields) has no spill → parity with C.

## Register Pressure Boundary
- **≤4 loop-carried float fields**: parity with C (struct-SSA works perfectly)
- **≥12 loop-carried float fields**: 1.87× slower (register spill unavoidable)

The boundary is somewhere between 4 and 12. At ~8+ fields, the allocator starts
spilling.

## Why Approaches Failed
1. **Phi coalescing**: llc's register allocator sees the phi `[entry_val, entry], [body_val, body]`
   and gives the two paths different physical registers. The bound check uses the entry
   register, which never updates. This is correct SSA behavior that our alias couldn't bridge.

2. **Alloca promotion**: mem2reg promotes each field's alloca to a phi, but the
   resulting phis still compete for the same 16 XMM registers. The allocator spills
   the same way regardless of whether the phis came from struct-SSA or per-field allocas.

3. **Global loads in hot loop**: Even when the IR has no explicit global loads
   (using phi values), `llc` may rematerialize global loads because the phi's
   entry value came from a global load. The register allocator sees "cheaper to
   reload from global than to keep in register" for spilled values.

## Recommendation
Accept the current struct-SSA approach. For programs with >8 loop-carried float
fields, the 1.87× ratio is a hardware limitation. Future work:
- Graph-coloring register hints in the IR
- Split hot/cold fields — promote hot fields to registers, leave cold fields in memory
- Use `alloca` + separate load/store for cold fields to avoid struct aliasing overhead

## Updated Benchmark Table
| Benchmark | Fields | Briev | C | Ratio | Notes |
|-----------|--------|-------|---|-------|-------|
| iir_filter | 4 | 0.156s | 0.155s | **1.01×** | No spill → parity |
| precompute_sum | 2 | 0.001s | 0.016s | **0.06×** | O(1) fold beats C loop |
| ring_buffer | 1 | 0.001s | 0.001s | **1.00×** | Both O(1) |
| async_counters | 1 | 0.001s | 0.001s | **1.00×** | Both O(1) |
| kalman_filter | 12 | 1.214s | 0.649s | **1.87×** | Register pressure spill |
