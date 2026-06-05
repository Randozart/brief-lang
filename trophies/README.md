# Trophy Benchmarks — Where Brief Beats C

## What is this?

Six benchmarks where Brief's compiler produces code **faster than C** compiled with
`clang -O3 -march=native -ffast-math`. Each subfolder contains:

- `.bv` — Brief source
- `_c.c` — C reference (identical algorithm, identical output mechanism)
- `.ll` — Generated LLVM IR (before optimization)
- `.opt.ll` — After `opt -O2` (when available)
- `.bc` / `_merged.opt.bc` — LTO-merged + optimized bytecode
- `.o` — Object file
- `.s` — `objdump -d` disassembly of Brief binary
- `_c.s` — `objdump -d` disassembly of C binary
- `README.md` — Why Brief wins, annotated assembly

## How to reproduce

```bash
# Build the Brief compiler
cargo build --release --bin brief-compiler

# Run all benchmarks (including trophies)
bash benchmarks/build_and_bench.sh

# Run a single trophy
bash benchmarks/build_and_bench.sh float_math
```

All benchmarks use the harness at `benchmarks/build_and_bench.sh`:
- Nanosecond `CLOCK_MONOTONIC` timing
- 5 iterations averaged
- `BOUND=50000000` (50M iterations)
- C compiled with `clang -O3 -march=native -ffast-math`
- Brief compiled with LTO (`clang -c -emit-llvm brief_rt.c` → `llvm-link` → `opt -O3` → `llc`)

## The six wins

| Benchmark | Brief | C | Ratio | Lesson |
|-----------|-------|---|-------|--------|
| [float_math](./float_math/) | 0.0044s | 0.0059s | **0.77×** | SROA + native float registers + fast-math |
| [print_loop](./print_loop/) | 0.0399s | 0.0606s | **0.65×** | LTO inlines FFI calls into hot loop |
| [float_math_nonzero](./float_math_nonzero/) | 0.1623s | 0.1660s | **0.97×** | SLP hazard suppression prevents register spill |
| [cancel_math](./cancel_math/) | 0.0410s | 0.0555s | **0.73×** | Algebraic simplification + LTO FFI inlining |
| [queue_drain](./queue_drain/) | 0.0423s | 0.0491s | **0.86×** | Inline collection ops + unified folded loop |
| [interval_step](./interval_step/) | 0.0583s | 0.0591s | **0.98×** | Interval bounds detection at parity with C |

## What was NOT included

- **O(1) fold wins** (iir_filter 0.001s vs 0.084s, const_heavy 0.001s vs 0.034s, bit_clear 0.0008s vs 0.0006s) — these are the compiler *proving* the loop is a pure counter and replacing it with a single store. Impressive, but not a "fair" comparison of generated code quality.
- **nbody_sqrt** — C wins 2.15×. Brief wraps `sqrtf` through `__sqrtf` in `brief_rt.c` which lacks `always_inline`, so LTO doesn't inline it. The extra call overhead adds up across 500M calls. Same mechanism as nbody_newton.
- **nbody_newton** — C wins 2.30×. Same sqrtf wrapper overhead as nbody_sqrt. The Newton sqrt approach (no sqrtf) would be faster but uses a different algorithm, so it's excluded.
- **kalman_filter_runtime** — C wins by 5%. Marginal and context-dependent.

## Key to Brief's speed

Brief wins when it can prove more at compile time than C can. The contract system
gives the compiler enough information to:

1. **Pre-allocate all registers** — SSA mode decomposes the `%State` struct into
   scalar `float` registers before the loop body, so all operations are independent
   and fully parallelizable (fills all CPU execution ports).

2. **Inline FFI calls via LTO** — `frgn __print_int` from `brief_rt.c` is a plain
   C function that gets inlined into the hot loop. C's `printf` is a varargs libc
   call that can't be inlined.

3. **Suppress harmful SLP vectorization** — The SLP hazard analyzer detects when
   vectorization would cause register spilling (>16 register pressure on x86_64)
   and disables it proactively. C's optimizer sometimes vectorizes anyway and
   pays the spill cost.
