# LLVM Auto-Vectorization Enablement — Replace Hand-Rolled SLP with willreturn

**Date:** 2026-07-28
**Status:** Proposed
**Experiment ID:** EXP-2026-07-28-LLVM-AUTOVEC

## Hypothesis

Adding `willreturn` to `#11` (reactive txn function attribute) enables LLVM's native
auto-vectorizer on convergence loops. Combined with removing hand-rolled SLP (which
interferes with LLVM's vectorizer via non-native vector widths like `<3 x float>`),
this lets LLVM choose the optimal vectorization strategy for each benchmark.

## Scientific Method

### Control
Current recovery-branch (SLP enabled, `#11` without `willreturn`, no `alwaysinline`):
- nbody_sqrt_idio: 0.67x — all-time best (SLP was harmful)
- nbody_newton: 1.09x — SLP is currently beneficial
- All others: at parity

### Treatment A
`#11` with `willreturn` added, SLP still enabled:
- Measures whether `willreturn` alone helps at all

### Treatment B
`#11` with `willreturn` added, SLP disabled:
- Measures whether `willreturn` + LLVM auto-vec ≥ hand-rolled SLP for nbody_newton

### Treatment C (fallback)
If Treatment B regresses nbody_newton, try adding a separate `#13 = #11 + willreturn`
and select it only for reactive txns with simple bounds:
- `#13`: reactive txns with `[count < N][count == N]` convergence
- `#11`: reactive txns with complex convergence (unknown to be willreturn-safe)

### Variables
| Independent | Dependent | Controlled |
|------------|-----------|------------|
| `willreturn` on `#11` | All 19 benchmark ratios | Thermal cooldown (60s) |
| SLP enabled/disabled | instruction count (opt -O3) | Compiler version |
| | .text section size | Same hardware, same clang |

### Expected Outcomes

| Scenario | nbody_sqrt_idio | nbody_newton | ring_buffer | All others |
|----------|----------------|--------------|-------------|------------|
| **Best case**: `willreturn` enables LLVM auto-vec, SLP off | 0.67x | **≈1.05x** | 1.06x | parity |
| **Neutral**: No change from Treatment A | 0.67x | **1.09x** | 1.06x | parity |
| **Worst case**: `willreturn` harms LLVM optimization | regresses | regresses | regresses | regresses |

### Mechanistic Prediction

LLVM's auto-vectorizer (at `-O3 -ffast-math`) requires `willreturn` on the function
containing the loop. Without `willreturn`, LLVM must assume the loop might be infinite
and cannot apply loop vectorization, LICM, or DSE past the loop terminator.

With `#11` currently lacking `willreturn`, nbody_newton's convergence loop
(`[count < bound][count == bound]`) is opaque to LLVM. The loop IS vectorized by
our SLP (which emits `<3 x float>` ops from pattern matching), but LLVM cannot
add its own vectorization on top.

Adding `willreturn` lets LLVM:
1. SROA: Promote %State fields within the txn function (already active via `argmem:readwrite`)
2. Loop vectorizer: Create `<2 x float>` or `<4 x float>` ops at native width
3. LICM: Hoist invariant loads out of the convergence loop
4. DSE: Eliminate dead stores within the loop

The expected net effect: LLVM's auto-vectorizer produces AT LEAST as efficient code
as our hand-rolled SLP, because LLVM:
- Uses native vector widths (SSE=4×float, AVX2=8×float)
- Has a sophisticated cost model for profitability
- Can unroll, fuse, and schedule beyond simple pattern matching

## Procedure

```
1. Add `willreturn` to `#11`                             # mod.rs:3309
2. cargo test --lib
3. cargo build --release
4. sleep 60
5. FULL benchmark suite (SLP enabled, #11+willreturn)     # Treatment A
6. Disable SLP (if false && should_vec)                   # counter.rs:750
7. cargo build --release
8. sleep 60
9. FULL benchmark suite (SLP disabled, #11+willreturn)    # Treatment B
10. Compare Treatment A vs Treatment B vs Control
11. If nbody_newton regresses in BOTH → discard hypothesis
12. If nbody_newton improves in B → select Treatment B (SLP off)
13. If Treatment B is selected: commit SLP-off + willreturn, remove all Axes 2+3 code
14. Write conclusions
```

## Preregistered Commitment

We commit to accepting the result. If `willreturn` on `#11` + SLP-off achieves nbody_newton
at ≤1.10x (current baseline), we accept it as the correct optimization strategy and remove
all hand-rolled SLP code (Axes 2+3). If nbody_newton regresses past 1.10x, we revert
and keep SLP.

## Appendix: Why `willreturn` Is Safe

Every reactive txn in Briv has a convergence contract: `[pre][post]`. The pre-condition
defines when the loop executes; the post-condition defines when it terminates. For
`[count < bound][count == bound]`, the loop executes while `count < bound` and terminates
when `count == bound`. This IS `willreturn` — the function will always return.

The only concern is a reactive txn without a convergence post-condition (e.g.,
`node serve [true]` with no terminal path). Such a program loops forever — `willreturn`
would be a miscompilation risk. But such programs violate the Briv language specification
(§3.2 of spec/SPEC.md: "Every txn must have a postcondition that provably terminates").

Adding `willreturn` to `#11` is therefore semantically correct for all valid Briv programs.
