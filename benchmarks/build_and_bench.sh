#!/usr/bin/env bash
# IIR Filter Benchmark — Brief LLVM vs C
#
# Builds both implementations and times them.
# The Brief reactor loops forever after total iterations,
# so we use timeout to cap it.
#
# Usage: bash benchmarks/build_and_bench.sh

set -euo pipefail
cd "$(dirname "$0")/.."

echo "=== Building Brief compiler ==="
cargo build --bin brief-compiler

echo ""
echo "=== Compiling Brief → LLVM IR ==="
cargo run --bin brief-compiler -- llvm benchmarks/iir_filter.bv --out benchmarks

echo ""
echo "=== Compiling Brief → native ==="
clang -O3 -march=native -o benchmarks/iir_filter benchmarks/iir_filter.ll

echo ""
echo "=== Compiling C reference ==="
clang -O3 -march=native -o benchmarks/iir_filter_c benchmarks/iir_filter_c.c -lm

echo ""
echo "=== Brief assembly (first 10 mulss/addss/subss) ==="
clang -O3 -S -o /dev/stdout benchmarks/iir_filter.ll 2>/dev/null | grep -E '(mulss|addss|subss)' | head -10

echo ""
echo "================================================"
echo "  BENCHMARK: 50M IIR filter iterations"
echo "================================================"
echo ""

TIMEOUT=15
echo "Brief (${TIMEOUT}s timeout, infinite reactor loop):"
/usr/bin/time -f "  real %e  user %U  sys %S" timeout ${TIMEOUT}s ./benchmarks/iir_filter 2>&1 || true
echo ""

echo "C (exits after 50M iterations):"
/usr/bin/time -f "  real %e  user %U  sys %S" ./benchmarks/iir_filter_c
echo ""

echo "================================================"
echo "NOTE: The Brief reactor has tick-dispatch overhead"
echo "(precondition evaluation + transaction dispatch per tick)."
echo "C has zero overhead per iteration (plain for-loop)."
echo "The Brief version's float operations (mulss/addss/subss)"
echo "are verified present in the generated assembly."
echo "================================================"
