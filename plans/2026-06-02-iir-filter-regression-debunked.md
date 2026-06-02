# iir_filter Regression Debunked (2026-06-02)

## What appeared to be a regression
The benchmark run showed iir_filter at 0.1876s — this was 0.000s (O(1)) in the AGENTS.md baseline.

## Root cause: stale binary
The `build_and_bench.sh` script had a guard:
```bash
if [ ! -f "$bin" ]; then
    clang -O3 -march=native -ffast-math "${name}.ll" -o "$bin" -lm
fi
```
This meant if a binary already existed, it was never rebuilt. The old `benchmarks/iir_filter` binary was compiled from an earlier commit (before dead-field elimination), so it contained the full 50M-iteration folded loop body.

## Verification
- Fresh compile with current source: `store i64 50000000, i64* %gp` at line 257 of the .ll — O(1) fold intact
- `/usr/bin/time -v`: User time (seconds): 0.00
- 5 consecutive runs: all 0.00s user time

## Fix applied
`build_and_bench.sh` now does `rm -f "$bin"` before compiling, ensuring every benchmark run gets a fresh binary from current source.

## Impact on optimization plan
Step A (iir_filter regression fix) is a no-op. The fold was never broken. We can proceed directly to loop unrolling (Step B).
