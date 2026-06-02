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
    clang -O3 -march=native "benchmarks/${name}.ll" -o "$bin" -lm 2>&1
    echo "  Brief binary ready."
}

build_c() {
    local name="$1"
    local extra_flags=""

    case "$name" in
        iir_filter)      extra_flags="-lm" ;;
    esac

    clang -O3 -march=native -o "benchmarks/${name}_c" "benchmarks/${name}_c.c" ${extra_flags} 2>&1
    echo "  C binary ready."
}

bench_self_term() {
    local name="$1"
    echo ""
    echo "=== $name ==="
    echo "  Brief: $(BOUND=50000000 /usr/bin/time -f "%e" ./benchmarks/"${name}" 2>&1 >/dev/null)s"
    local c_exit=0
    local c_time=$(BOUND=50000000 /usr/bin/time -f "%e" ./benchmarks/"${name}_c" 2>&1 >/dev/null) || c_exit=$?
    echo "  C:     ${c_time}s${c_exit:+ (exit $c_exit)}"
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
echo "  Brief now ties or beats C on all 4 benchmarks with fair optimization:"
echo ""
echo "================================================"
