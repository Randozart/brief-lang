# Float Optimization Tracks — Match Clang's Constant Propagation + SSA Fix

## Discovery

In the calibration baseline (2026-06-02), `float_math` showed a **9.79× gap**
(Briv 0.4231s vs C 0.0432s). Investigation of the C IR revealed that **clang -O3
completely eliminated the matrix multiply** by proving x0=x1=x2=0 invariants
through the recurrence:

```llvm
; C hot loop (after clang -O3):
  phi: p00, p11, p22, count
  fadd  p00, Q00 ; only 3 fadds + count remain
  fadd  p11, Q11
  fadd  p22, Q22
  count++
```

The entire 9-element matrix multiply (12 fmul + fadd) was eliminated. Briv's
`opt -O2` could NOT do the same — it emitted all 17 float ops per iteration.

## Root Cause: Three Barriers

### Barrier 1: `@global_state` has external linkage

```llvm
@global_state = global %State zeroinitializer  ; external
```

LLVM's SCCP treats all loads from external globals as `overdefined` (any
external function might have written to it). Even though `@global_state` is
initialized to `zeroinitializer`, the compiler cannot prove no other module
writes to it.

### Barrier 2: `emit_init_state` uses `store volatile`

```llvm
store volatile float 0.0, float* @global_state
```

The `volatile` qualifier prevents GVN from forwarding known values through the
initialization stores. Even after inlining `init_state()` into `main()`, LLVM
sees side-effect barriers that block value propagation.

### Barrier 3: SSA mode loads `%State` every iteration

```llvm
case_body:                              ; EVERY iteration
  %ssa = load %State, %State* @global_state
  ... extractvalue / insertvalue ...
  store %State %ssa, %State* @global_state
```

By loading + storing the entire 64-byte `%State` struct on every iteration,
SROA cannot decompose into scalar phi nodes. This prevents mem2reg → SCCP →
GVN from folding the computation, and creates unnecessary memory traffic.

## Track 1 — Make `opt -O2` match clang on zero-propagation

**Fix Barrier 1**: Change `@global_state` to `internal` linkage.

```diff
-@global_state = global %State zeroinitializer
+@global_state = internal global %State zeroinitializer
```

With `internal`, LLVM proves no external function (including `__rt_init`,
`__rt_poll`) can access `@global_state`. The initializer `zeroinitializer`
(all zeros) becomes known.

**Fix Barrier 2**: Remove `volatile` from all `store` instructions in
`emit_init_state`. With internal linkage, volatile is unnecessary and blocks
value forwarding.

```diff
-store volatile float %val, float* %gep
+store float %val, float* %gep
```

After inlining `init_state()` into `main()`, GVN sees the stores write known
values to known fields and propagates them through subsequent loads.

**Fix Barrier 3**: Change SSA mode from load-every-iter to load-once + phi
preheader.

```diff
 ; BEFORE (load+store every iteration)
 case_body:
   %ssa = load %State, %State* @global_state
   ... extract/insert ...
   store %State %ssa, %State* @global_state
   br label %hdr

 ; AFTER (load once, phi in header, store once)
 preheader:
   %ssa_init = load %State, %State* @global_state
   br label %hdr
 hdr:
   %ssa = phi %State [ %ssa_next, %body ], [ %ssa_init, %preheader ]
   ... extract/insert on %ssa → %ssa_next ...
   br i1 %cond, label %body, label %done
 body:
   br label %hdr
 done:
   store %State %ssa, %State* @global_state
```

This lets SROA decompose the struct into scalar phis. With the phi preheader
carrying known initial values (from zeroinitializer), SCCP propagates the
zero constants through the entire matrix computation, matching clang's
optimization.

**Expected**: float_math (original, x0=x1=x2=0) should drop from 0.4231s to
~0.04s, tying C.

## Track 2 — Non-trivial benchmark for fair comparison

After Track 1, float_math will be trivially optimized (zero-matrix eliminated).
We need a second benchmark where BOTH compilers must do real work.

**`float_math_nonzero.bv` / `float_math_nonzero_c.c`**:
```diff
- x0 = 0.0, x1 = 0.0, x2 = 0.0
+ x0 = 1.0, x1 = 0.5, x2 = 0.2
- A10 = 0.0, A20 = 0.0
+ A10 = 0.01, A20 = 0.001, A02 = 0.001
```

Non-zero initial values + cross-coupling prevent clang from proving zero-
stability. Both compilers must compute the full 9-element matrix multiply.

Also keep `p00/p11/p22` only (not all 9 covariance fields) to eliminate the
dead-field noise. The original float_math has p01-p21 all zero-initialized
with zero Q-coefficients — those 6 fields are dead work that Briv shouldn't
pay for. The nonzero variant should match the C variant field-for-field.

**Expected gap**: TBD — this measures the real structural overhead of Briv's
reactive model (preconditions, global state, dispatch) vs C's plain while-loop.

## Track 3 — Close the residual gap

**Problem**: After Barrier 3 fix, SROA decomposes the struct. But the
`insertvalue` chain still creates LLVM `load %State`/`store %State` around
the loop. With scalar phi nodes, each field is a separate SSA register.

If Track 1+2 don't match C, the SSA-mode fix (Barrier 3) should be the
primary fix for 32-byte or 64-byte struct register pressure.

## Benchmark Matrix

| Benchmark | Initial | Matrix | Expected Briv vs C |
|-----------|---------|--------|-------------------|
| `float_math` (original) | x=0,y=0,z=0 | Identity+coupling | ~tie (both eliminate) |
| `float_math_nonzero` | x=1.0,y=0.5,z=0.2 | Full coupling | TBD — real overhead |

## Files Changed

- `src/backend/llvm.rs` — 3 changes (internal, volatile removal, SSA phi)
- `benchmarks/float_math_nonzero.bv` — new
- `benchmarks/float_math_nonzero_c.c` — new
- `benchmarks/build_and_bench.sh` — add `float_math_nonzero`
- `plans/2026-06-02-float-optimization-tracks.md` — this file
- `AGENTS.md` — update benchmark tables
