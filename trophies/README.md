# Trophy Benchmarks — Where Briv Beats C

## What is this?

A collection of compiler achievements where Briv's LLVM backend produces
code **faster than C** compiled with `clang -O3 -march=native -ffast-math`,
plus the architectural improvements that made it possible.

Each subdirectory contains:
- `README.md` — Trophy writeup (what, why, how, before/after)
- `.bv` — Briv source files demonstrating the achievement
- `_c.c` — C reference (when applicable)
- `.ll` — Generated LLVM IR (before optimization)
- `.opt.ll` — After `opt -O2`
- `.bc` / `_merged.opt.bc` — LTO-merged + optimized bytecode
- `.o` — Object file
- `.s` — `objdump -d` disassembly

## Trophy Index

| Trophy | Benchmark | Ratio | Briv vs C |
|--------|-----------|-------|------------|
| [slp-hazard](slp-hazard/) | nbody_sqrt | **0.82x** | Briv **1.22x faster** |
| [float-boxing](float-boxing/) | float_math | **0.66x** | Briv **1.52x faster** |
| [per-field-gep](per-field-gep/) | nbody_newton | **1.0x** | parity (enables SROA) |
| [intrinsics](intrinsics/) | n/a | n/a | 29 compiler intrinsics |
| [bracket-universal](bracket-universal/) | n/a | n/a | SIMD protocol for all types |
| [top-level-init](top-level-init/) | n/a | n/a | Scripting with boot safety |
| [dispatch-collapse](dispatch-collapse/) | ring_buffer | **0.80x** | Briv **1.25x faster** |
| [equality-saturation](equality-saturation/) | cancel_math | **0.5x** | Briv beats C via fold |
| [direct-ssa-loop](direct-ssa-loop/) | nbody_newton | **0.6x** | Briv **1.6x faster** |
| [benchmarks](benchmarks/) | all | **7 of 10** | Briv beats C |

## How to reproduce

```bash
# Build the Briv compiler
cargo build --release

# Run all benchmarks
bash benchmarks/build_and_bench.sh

# Run a single benchmark
bash benchmarks/build_and_bench.sh nbody_sqrt
```
