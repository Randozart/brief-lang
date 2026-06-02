# ASR-Based SLP Profitability Analysis (2026-06-02)

## Problem
The current `estimate_slp_hazard()` only checks register pressure (will SLP overflow XMM registers?). This misses cases like `float_math_nonzero` where register pressure is low (~5 floats) but SLP still hurts: the vectorizer inserts 5 `vinsertps`/`vblendps`/`vshufps` packing instructions that saturate Port 5, creating a net regression. This is the primary cause of the 2.25× gap.

## Root Cause
SLP vectorization must build vectors from loop-carried scalar accumulators. On x86, shuffle/blend/insert instructions are restricted to Port 5 (1.0 reciprocal throughput), while arithmetic ops can use Ports 0+1+5 (0.5 reciprocal throughput). When too few independent arithmetic ops exist per shuffle, Port 5 saturates and the OoO window stalls.

## Solution: Arithmetic-to-Shuffle Ratio (ASR)

### Formula

```
N    = independent float ops in body (result NOT a loop-carried field)
P_φ  = loop-carried float accumulators (fields both read AND written in body)
W    = vector width (4 for AVX128, 8 for AVX256)
t    = target-specific port penalty ratio (2.0 for x86, 1.5 for AArch64)

ASR = N / (2 × P_φ × (1 − 1/W))
```

If ASR < t, SLP is mathematically guaranteed to lose — disable it.

### Why 2 × P_φ × (1 − 1/W)?

Each loop-carried variable needs ~(1 − 1/W) shuffle instructions to pack into a vector at the backedge, plus ~(1 − 1/W) to unpack at the preheader. That's 2 × P_φ × (1 − 1/W) total shuffles. N independent ops compete with these for execution ports.

### Why the formula targets independent ops (N) not total ops

Total arithmetic ops include ops feeding loop-carried accumulators — these are sequential dependencies SLP can't vectorize anyway. Only independent-parallel ops benefit from packing. Unrolling increases total N but NOT independent N (same accumulators chain through all copies).

### Why unrolling doesn't artificially inflate ASR

The analysis runs on the *base* body (AST), before unrolling. Unrolling is a codegen artifact; SLP sees the unrolled body as one block. Our analysis correctly models the base body.

## Integration

### Existing model (register pressure)
```
peak = ceil(N/P) + min(2×ceil(N/P), ceil(C/2)) + T + ceil(K/W) + 2
if peak ≥ R: disable SLP
```

### New model (adds ASR gate after register check)
```
if peak ≥ R: disable SLP  (existing)
else compute ASR:
  if ASR < target.t: disable SLP  (new)
```

Both gates must pass for SLP to be enabled. Either can independently disable it.

## P_φ Computation

A float field counts as loop-carried (P_φ) if:
1. It's in the live set (from `compute_live_fields`)
2. It appears as an `Assignment` target (LHS) in the transaction body
3. It's a float type

This distinguishes accumulators from read-only constants and temporaries.

## N Computation

Count float operations (fadd, fmul, fsub, fdiv) where:
1. The result is NOT assigned to a loop-carried field
2. At least one operand is float

Operations feeding accumulators are excluded because SLP can't vectorize sequential dependencies.

## Target Parameters

| Target | W | t | Note |
|--------|---|---|------|
| x86 (AVX128) | 4 | 2.0 | 3 execution pipes, 1 port for shuffles |
| x86 (AVX256) | 8 | 2.0 | Same port constraint, wider vectors |
| AArch64 (NEON) | 4 | 1.5 | 3 NEON pipes, shuffles on 2 of them |
| WASM | 4 | 2.0 | Conservative default |
| RISC-V (V) | 4 | 2.0 | Conservative default |

## Edge Cases Considered

1. **Constants vs. accumulators**: xmm7/xmm8 loaded once, read-only → NOT P_φ. Only mutated fields count.
2. **Temporaries**: SSA values created+ killed in body → NOT P_φ. No packing needed.
3. **Zero accumulators (P_φ=0)**: No packing overhead, SLP is always safe. Skip ASR.
4. **All-independent body**: High N, low P_φ → high ASR → SLP enabled. Correct.
5. **Interaction with unrolling**: ASR computed on base body. Unrolling is a separate codegen decision unaffected by this analysis.
6. **ASR on unrolled body**: Would falsely inflate ASR (4× total ops, same P_φ). Must compute on base.

## Files Changed

- `src/backend/llvm.rs`: `estimate_slp_hazard()` — add ASR computation after register pressure check
- `src/analysis/transition_graph.rs`: Export compute_live_fields result or track mutated float fields for P_φ

## Implementation Steps

1. In `estimate_slp_hazard()`, after existing register pressure check:
   a. For each txn with body (not pure-counter folded), compute P_φ and N
   b. Compute ASR with W from target spec (default 4)
   c. If ASR < t (default 2.0): add to slp_hazard_fns
2. Run test suite
3. Benchmark float_math_nonzero to validate gap reduction
