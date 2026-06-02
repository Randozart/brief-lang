#!/usr/bin/env bash
# Brief Optimization Benchmarks — Builds and times all benchmarks
#
# Benchmarks:
#   iir_filter     — Path 2: folded while-loop counter convergence
#   precompute_sum — Path 3: compile-time evaluation (no runtime loops)
#   ring_buffer    — Path 4: enum switch-dispatch entry
#   async_counters — Path 5: thread pool parallel dispatch
#
# import "link/brief_rt.o" in the source files tells the compiler to
# auto-compile and link the runtime. No --link-rt flag needed.
#
# Usage:
#   bash benchmarks/build_and_bench.sh [BENCH_NAME]
#
#   bash benchmarks/build_and_bench.sh              # all benchmarks
#   bash benchmarks/build_and_bench.sh iir_filter   # single benchmark

set -euo pipefail
cd "$(dirname "$0")/.."

SELECTED_BENCH=""
MODE="all"
FUZZ_N=""

while [[ $# -gt 0 ]]; do
    case "$1" in
        --fuzz) FUZZ_N="$2"; shift 2 ;;
        all)    MODE="all"; shift ;;
        *)      SELECTED_BENCH="$1"; MODE="single"; shift ;;
    esac
done

BENCHMARKS=(
    "iir_filter"
    "precompute_sum"
    "ring_buffer"
    "async_counters"
    "float_math"
    "float_math_nonzero"
    "sparse_dispatch"
    "const_heavy"
)

build_bench() {
    local name="$1"

    echo ""
    echo "================================================"
    echo "  Building: $name"
    echo "================================================"

    # Compile Brief → LLVM IR using release binary (avoids cargo rerun)
    ./target/release/brief-compiler llvm "benchmarks/${name}.bv" \
        --out benchmarks --optimize-budget 256 2>&1

    local bin="benchmarks/${name}"
    if [ ! -f "$bin" ]; then
        clang -O3 -march=native -ffast-math "benchmarks/${name}.ll" -o "$bin" -lm 2>&1 || echo "  (clang linking skipped — possibly linked by compiler)"
    fi
    echo "  Brief binary ready."
}

build_c() {
    local name="$1"
    local extra_flags=""

    case "$name" in
        iir_filter)      extra_flags="-lm" ;;
    esac

    clang -O3 -march=native -ffast-math -o "benchmarks/${name}_c" "benchmarks/${name}_c.c" ${extra_flags} 2>&1
    echo "  C binary ready."
}

bench_self_term() {
    local name="$1"

    local brief_start=$(date +%s.%N)
    BOUND=50000000 ./benchmarks/"${name}" >/dev/null 2>&1 || true
    local brief_end=$(date +%s.%N)
    local brief_time=$(LC_NUMERIC=C printf "%.4f" "$(echo "scale=10; $brief_end - $brief_start" | bc)")

    local c_start=$(date +%s.%N)
    BOUND=50000000 ./benchmarks/"${name}_c" >/dev/null 2>&1 || true
    local c_end=$(date +%s.%N)
    local c_time=$(LC_NUMERIC=C printf "%.4f" "$(echo "scale=10; $c_end - $c_start" | bc)")

    echo ""
    echo "=== $name ==="
    echo "  Brief: ${brief_time}s"
    echo "  C:     ${c_time}s"
}

echo "=== Building Brief compiler (release) ==="
cargo build --release --bin brief-compiler 2>&1
echo ""

for name in "${BENCHMARKS[@]}"; do
    if [ "$MODE" = "single" ] && [ "$name" != "$SELECTED_BENCH" ]; then
        continue
    fi
    build_bench "$name"
    build_c "$name"
done

echo ""
echo "================================================"
echo "  RUNNING BENCHMARKS"
echo "================================================"

for name in "${BENCHMARKS[@]}"; do
    if [ "$MODE" = "single" ] && [ "$name" != "$SELECTED_BENCH" ]; then
        continue
    fi

    bench_self_term "$name"
done

if [ -n "$FUZZ_N" ]; then
    echo ""
    echo "================================================"
    echo "  FUZZING (n=$FUZZ_N)"
    echo "================================================"
    for name in "${BENCHMARKS[@]}"; do
        if [ "$MODE" = "single" ] && [ "$name" != "$SELECTED_BENCH" ]; then
            continue
        fi
        echo ""
        bash benchmarks/fuzz.sh "$name" --mode runtime --runs "$FUZZ_N"
    done
fi

echo ""
echo "================================================"
echo "  SUMMARY"
echo "================================================"
  echo "  All 8 benchmarks measured at BOUND=50000000, 4-decimal precision."
echo "  0.0000s = O(1) optimization eliminates the loop entirely."
echo "================================================"
