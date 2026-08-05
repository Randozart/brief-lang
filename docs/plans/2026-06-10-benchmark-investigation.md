# Benchmark Performance Investigation

## Setup
- `opt -O3 -ffast-math -mtriple=x86_64-pc-linux-gnu` ✓ (already set)
- `llc -O3 --mcpu=native` ✓ (already set)
- C reference: `clang -O3 -ffast-math -march=native`

ISA is not the gap. Experiments target structural IR differences.

## Experiment 1: `add i64 0, %src` instruction counting
Count how many redundant copy instructions each benchmark's hot loop emits.
Method: grep the unoptimized IR for `add i64 0, %` in the hot function body.
Expected: ~30% of all instructions are copies.

## Experiment 2: Hot loop instruction count (asm)
Emit assembly from both C and Briv LTO-merged IR for the same benchmark.
Count instructions in the hot loop (between branch-back targets).
Gives exact instruction-count gap, isolated from pipeline effects.

## Experiment 3: SLP hazard for straight-line code
kalman's `propagate` has 56 float ops in straight-line (no loops).
SLP can't vectorize across loop back-edges that don't exist.
Change `estimate_slp_hazard` to skip functions with zero nested loops.
Measure before/after on kalman.

## Experiment 4: Dead-field elimination trace
Trace fannkuch's field liveness chain:
  field[0..12] = rotate(field[0..12])
  count = count + 1
  print(count) — only count is observed
Check if liveness analysis correctly identifies field[0..12] as dead.
If not: fix liveness to trace through pure computation chains to observable output.

## Experiment 5: `add i64 0, %src` elimination
Remove redundant copies in LLVM IR emission.
These come from the uniform register model — every value is `add i64 0, %src`.
Fix: when a value is already in the right register (no transformation needed),
skip the copy and just return the original register name.

## Experiment 6: Switch dispatch for knucleotide
Trace the 64-way guard dispatch structure.
Check if LLVM's simplifycfg converts chained icmp/br to switch.
If not: add a Briv-level optimization to emit switch directly.
