# Plan: Calibration Benchmarks for New Optimizations

**Date:** 2026-06-02  
**Status:** Plan + implementation ready

## Motivation

Our 8 new optimizations show zero performance improvement on the existing 4 benchmarks because those benchmarks were already at O(1) 0.00s (pure-counter folded loops). The optimizations target code paths these benchmarks never exercise:

| Optimization | Code Path | Why Existing Benchmarks Miss It |
|---|---|---|
| Float register promotion | SSA-mode float extraction + `i64_to_float_reg` cache | No float-heavy SSA mode benchmark |
| `llvm.assume` on convergent preconditions | Folded loop with convergence proof | All 4 are pure-counter O(1) — no loop exists |
| Key extraction + perfect hashing | Sparse trigger dispatch | All triggers are dense Bool (0/1) |
| Constant inlining | `const X = N` referenced by name | Constants resolved to compile-time totals before codegen |

We need calibration benchmarks that isolate and measure each optimization independently.

## Benchmark 1: `float_math` — Float Register Promotion

**Purpose:** Measure the i64 boxing tax elimination for float-heavy SSA loops.

**Design:** 12 float state fields, ~60 float operations per tick (matrix multiply), 50M iterations via runtime variable `BOUND`. Single `node step [x0 == x0]` to ensure SSA mode (non-pure body prevents pure-counter elimination).

```brief
import "link/brief_rt.o"
import env from "std/env.bv"

let x0: Float = 0.0
let x1: Float = 0.0
let x2: Float = 0.0
let p00: Float = 0.0
let p01: Float = 0.0
let p02: Float = 0.0
let p10: Float = 0.0
let p11: Float = 0.0
let p12: Float = 0.0
let p20: Float = 0.0
let p21: Float = 0.0
let p22: Float = 0.0
let count: Int = 0

const A00: Float = 1.0
const A01: Float = 0.01
const A02: Float = 0.0
const A10: Float = 0.0
const A11: Float = 1.0
const A12: Float = 0.01
const A20: Float = 0.0
const A21: Float = 0.0
const A22: Float = 1.0

const Q00: Float = 0.1
const Q01: Float = 0.0
const Q02: Float = 0.0
const Q11: Float = 0.1
const Q12: Float = 0.0
const Q22: Float = 0.1

#!exit count == total;

let total: Int = __get_env_int("BOUND");

node step [x0 == x0] {
    &x0 = A00 * x0 + A01 * x1 + A02 * x2;
    &x1 = A10 * x0 + A11 * x1 + A12 * x2;
    &x2 = A20 * x0 + A21 * x1 + A22 * x2;
    &p00 = p00 + Q00;
    &p11 = p11 + Q11;
    &p22 = p22 + Q22;
    &count = count + 1;
}
```

**C reference:** Local float variables (no boxing), same algorithm, `clang -O3 -march=native`.

### Expected Impact

| Variant | Est. Runtime (50M) | vs C |
|---|---|---|
| Before (full boxing) | ~1.5-2.0s | 2-3× slower |
| After (float reg promotion + cache) | ~0.75-0.85s | ~1.0-1.2× |
| C (local float vars) | ~0.70-0.80s | baseline |

### Verification
- Before IR: `load float → bitcast to i32 → zext to i64 → trunc → bitcast → fadd → bitcast → zext`
- After IR: `extractvalue float → fadd → bitcast → zext` (boxing on store only)

## Benchmark 2: `sparse_dispatch` — Perfect Hashing

**Purpose:** Measure sparse trigger dispatch before vs after perfect hashing.

**Design:** 8 transaction cases, gated by an Int trigger with sparse values `{101, 204, 404, 808, 1616, 3232, 6464, 128}`. Each case increments its own counter. 50M dispatches reading from `__io_pending` (volatile trigger).

```brief
import "link/brief_rt.o"

let count: Int = 0

node case_ping  [io_pending == 101] { &count = count + 1; }
node case_ack   [io_pending == 204] { &count = count + 1; }
node case_err   [io_pending == 404] { &count = count + 1; }
node case_debug [io_pending == 808] { &count = count + 1; }
node case_data  [io_pending == 1616] { &count = count + 1; }
node case_ctrl  [io_pending == 3232] { &count = count + 1; }
node case_sync  [io_pending == 6464] { &count = count + 1; }
node case_stat  [io_pending == 128] { &count = count + 1; }
```

**C reference:** Same dispatch via computed goto, same algorithm.

### Expected Impact

| Variant | Dispatch Method | Ops/sec |
|---|---|---|
| Before (binary tree) | O(log 8) = 3 branches | ~100M/s |
| After (perfect hash + jump table) | O(1) = mul + shift + jump | ~300M/s |
| C (computed goto) | O(1) = computed goto | ~300M/s |

## Benchmark 3: `const_heavy` — Constant Inlining

**Purpose:** Measure the impact of replacing `load @CONST` with immediate `add i64 0, N`.

**Design:** 20 constants referenced by name in a tight loop. Each iteration performs arithmetic with all 20. Runtime bound.

```brief
import env from "std/env.bv"

const C00: Int = 100;  const C01: Int = 200;
const C02: Int = 300;  const C03: Int = 400;
const C04: Int = 500;  const C05: Int = 600;
const C06: Int = 700;  const C07: Int = 800;
const C08: Int = 900;  const C09: Int = 1000;
const C10: Int = 1100; const C11: Int = 1200;
const C12: Int = 1300; const C13: Int = 1400;
const C14: Int = 1500; const C15: Int = 1600;
const C16: Int = 1700; const C17: Int = 1800;
const C18: Int = 1900; const C19: Int = 2000;

let total: Int = __get_env_int("BOUND");
let count: Int = 0;
let acc: Int = 0;

#!exit count == total;

node step [count < total] {
    &acc = acc + C00 + C01 + C02 + C03 + C04
          + C05 + C06 + C07 + C08 + C09
          + C10 + C11 + C12 + C13 + C14
          + C15 + C16 + C17 + C18 + C19;
    &count = count + 1;
}
```

**C reference:** Same accumulation with `const int` macros, which clang inlines as immediates.

### Expected Impact

| Variant | Each iteration | Est. Runtime (50M) |
|---|---|---|
| Before (loads) | 20 `load i64` from RAM | ~0.12s |
| After (immediates) | 0 loads, all immediate | ~0.04s |
| C (macros) | All immediate | ~0.04s |

## Implementation

### Files

| File | Content |
|------|---------|
| `benchmarks/float_math.bv` | Float calibration benchmark |
| `benchmarks/float_math_c.c` | C reference |
| `benchmarks/sparse_dispatch.bv` | Sparse dispatch benchmark |
| `benchmarks/sparse_dispatch_c.c` | C reference |
| `benchmarks/const_heavy.bv` | Constant inlining benchmark |
| `benchmarks/const_heavy_c.c` | C reference |
| `src/backend/llvm.rs` | Fix `reg_float_cache` for SSA float extraction |

### Acceptance Criteria

1. All three new benchmarks compile and run without crashing
2. Float math: IR shows `extractvalue float → fadd → bitcast → zext` (no redundant boxing)
3. Sparse dispatch: IR shows `mul i64 + lshr i64 + switch` with verification guards
4. Const heavy: IR shows `add i64 0, 100` (no `load i64 @C00`)
5. C refs produce same output as Brief for all three
6. `cargo test --lib` passes (368+ tests)
