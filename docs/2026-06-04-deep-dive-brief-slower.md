# Deep Dive: Where Brief Loses to C

**Date:** 2026-06-04

## Summary

| Benchmark | Brief | C | Ratio | Root cause |
|-----------|-------|---|-------|------------|
| nbody_sqrt | 6.96s | 3.23s | 2.15× | `__sqrtf` is PLT call; C uses `vrsqrtps` hardware instr |
| mandelbrot | 0.74s | 0.65s | 1.14× | 212 extractvalue/insertvalue + Int `/ SCALE` overhead |
| kalman | 0.161s | 0.153s | 1.05× | Marginal — within noise |

---

## 1. nbody_sqrt — Float boxing kills sqrt performance (2.15×)

### Root cause: frgn sqrt calls vs hardware instruction

Brief's `frgn __sqrtf(d: Float) -> Float` goes through libc `sqrtf()`:

```llvm
  %t56 = call float @__sqrtf(float %nffl59)   ; PLT call to libc sqrtf
  %fbi60 = bitcast float %t56 to i32           ; float → i32 boxing
  %fze61 = zext i32 %fbi60 to i64             ; i32 → i64 extension
```

C with `-ffast-math` converts `sqrt()` to hardware reciprocal sqrt:
```asm
  vrsqrtps %xmm3,%xmm7     ; vector reciprocal sqrt — single instruction
```

**21 sqrt calls per tick, 50M ticks = 1.05 billion boxing instructions.**

### Why it happens

Brief represents all values as `i64` (boxed). Float operations go through:
1. `bitcast float → i32` (unbox)
2. `zext i32 → i64` (fit into Brief's uniform register width)
3. `call __sqrtf` (PLT call to libc)
4. `trunc i64 → i32` (unbox result)
5. `bitcast i32 → float` (re-box as float register)

Each sqrt costs ~5 instructions + PLT call. C does one instruction.

### Fix options

**A) `llvm.sqrt.f32` intrinsic** — Replace `call @__sqrtf` with `call float @llvm.sqrt.f32(float)` in `emit_expr` for `Expr::Call("__sqrtf", ...)`. Combined with `fast-math`, LLVM converts this to `fsqrt`/`vsqrtps`. Zero changes to `brief_rt.c`.

**B) Inline `__sqrtf` with `alwaysinline`** — `__attribute__((alwaysinline)) static inline float __sqrtf(float x) { return sqrtf(x); }` in `brief_rt.c`. LTO should then inline it and `fast-math` will convert to `fsqrt`.

**C) Mark `__sqrtf` as a builtin in LLVM** — Add `@llvm.compiler.used` or an attribute that tells LLVM "this is just sqrt, use the intrinsic."

**Recommendation**: Option A. It's a 1-line change in `emit_expr` and works independently of LTO. The FFI layer maps `__sqrtf` to `llvm.sqrt.f32`, which `opt -O2 -ffast-math` converts to hardware instructions.

### Expected improvement

21 sqrt calls per tick, 50M ticks, ~5 instructions saved per call = ~250M instructions saved. Should close the 2.15× gap entirely.

---

## 2. mandelbrot — Integer division and SSA shuffle overhead (1.14×)

### Root cause: Int `/ SCALE` generates sdiv, extractvalue chains dominate

Brief's mandelbrot uses integer fixed-point arithmetic (`zr * cr / SCALE`). Each `/ SCALE` generates:
- `sdiv i64` (signed division — 20-80 cycles on x86, much slower than float div)
- The guarded `zr >= -200` check creates SSA control flow with `extractvalue`/`insertvalue`

**212 extractvalue/insertvalue in the LLVM IR** — these create the struct-SSA shuffle pattern that `opt -O2` must decompose via SROA. For Int fields, SROA is less effective than for Float fields (Int types don't benefit from `fast-math` flags).

### Why it happens

Brief's `Int` type is 64-bit. Integer division is fundamentally slower than float division on modern CPUs (~20-80 cycles vs ~10-14 cycles). The fixed-point scaling (multiply by 100, divide by 100) generates `sdiv` instructions that are expensive.

C's mandelbrot uses `double` Float arithmetic — faster division, no integer scaling needed.

### Fix options

**A) Float mandelbrot** — Convert to Float arithmetic directly. Brief's Float pipeline is proven faster than C (float_math wins). The mandelbrot can use Float with `fast-math` and get the same SROA + SIMD benefits.

**B) Strength reduction** — `zr * cr / SCALE` where SCALE=100 could be 100.0 (Float) or replaced with a shift-based approximation.

**C) Eliminate guarded blocks** — The `zr >= -200` check in `#!exit` creates SSA control flow. Remove it; use `#!exit count == N && escapes >= 0` instead.

**Recommendation**: Option A. It's already proven that Brief beats C on Float benchmarks. Convert mandelbrot to use `Float` everywhere and drop the integer scaling.

---

## 3. kalman_filter_runtime — Marginal overhead (1.05×)

### Root cause: Universal loop dispatch overhead

At 1.05×, the gap is within noise for most benchmarks (~0.008s difference at 50M).

The universal folded loop emits:
- One `load %State` per tick
- `extractvalue` for each field
- `insertvalue` chain for updates
- `store %State` at end

C's while-loop has:
- Direct field access (scalar registers)
- No struct load/store per iteration

The SSA-SROA path handles this for Float fields, but the struct load/store adds ~2-3 instructions per tick that C doesn't have.

### Fix options

**A) Accept it** — 1.05× is very close. Not worth optimizing for this margin.

**B) Float-only body** — If the body has no Int state fields, skip the struct-SSA entirely and use native float registers. (Already done for float_math via SSA mode.)

**Recommendation**: Accept. Focus on nbody_sqrt (2.15×) and mandelbrot (1.14×) first.

---

## Implementation priority

1. **nbody_sqrt** — `llvm.sqrt.f32` intrinsic (1 line, closes 2.15× gap)
2. **mandelbrot** — Convert to Float arithmetic (10-20 lines in .bv, may win)
3. **kalman** — Accept or investigate if option 2 unblocks further gains
