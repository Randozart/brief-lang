## Benchmark Verification: All Benchmarks Match C Output

**What**: Every runtime benchmark in the suite has been verified to produce
identical output to its C reference for the same input.

**Why it matters**: Performance comparisons are meaningless without
correctness. A benchmark that computes a different result might be faster
for the wrong reasons (precomputation, fold elimination, or silently
incorrect code). Every benchmark in the suite has a C reference that
produces the same output for BOUND=5 across all 20+ runtime benchmarks.

**How**: The benchmark harness runs each Briev binary and C binary with
identical environment variables (`BOUND=5`), captures stdout, and compares
byte-for-byte. The `build_and_bench.sh` script reports "MATCH" or
"MISMATCH" for each benchmark. The suite includes symmetric and idiomatic
variants where algorithm differences exist (queue_drain, nbody_newton,
fannkuch_redux).

**Current status**: 20 runtime benchmarks, all verified. The full suite
runs in ~2 minutes for correctness checking via
`bash benchmarks/build_and_bench.sh --correctness`.
