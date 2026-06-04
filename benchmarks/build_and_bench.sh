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
    "print_loop"
    "nbody_newton"
    "nbody_sqrt"
    "fasta"
    "fannkuch_redux"
    "mandelbrot"
    "kalman_filter_runtime"
)

build_bench() {
    local name="$1"

    echo ""
    echo "================================================"
    echo "  Building: $name"
    echo "================================================"

    # Force rebuild: remove stale binary so we always get current source
    local bin="benchmarks/${name}"
    rm -f "$bin"

    local budget=256
    case "$name" in
        nbody_newton) budget=2048 ;;
        nbody_sqrt)   budget=2048 ;;
    esac

    ./target/release/brief-compiler llvm "benchmarks/${name}.bv" \
        --out benchmarks --optimize-budget "$budget" 2>&1

    if [ ! -f "$bin" ]; then
        if [ -f "benchmarks/${name}.o" ]; then
            cc -O2 -no-pie -o "$bin" "benchmarks/${name}.o" -lm 2>&1 || echo "  (link failed — try manual link)"
        else
            clang -O3 -march=native -ffast-math "benchmarks/${name}.ll" -o "$bin" -lm 2>&1 || echo "  (clang linking skipped — possibly linked by compiler)"
        fi
    fi
    echo "  Brief binary ready."
}

build_c() {
    local name="$1"
    local extra_flags=""

    case "$name" in
        iir_filter)      extra_flags="-lm" ;;
        nbody_sqrt)      extra_flags="-lm" ;;
        fasta)           extra_flags="-lm" ;;
        fannkuch_redux)  extra_flags="-lm" ;;
        mandelbrot)      extra_flags="-lm" ;;
        kalman_filter_runtime) extra_flags="-lm" ;;
    esac

    clang -O3 -march=native -ffast-math -o "benchmarks/${name}_c" "benchmarks/${name}_c.c" ${extra_flags} 2>&1
    echo "  C binary ready."
}

# Build nanosecond timing harness (compiled C fork+exec timer)
TIMER_BIN="/tmp/brief_bench_timer"
TIMER_SRC="/tmp/brief_bench_timer.c"
if [ ! -f "$TIMER_BIN" ]; then
    cat > "$TIMER_SRC" << 'CEOF'
#define _GNU_SOURCE
#include <stdio.h>
#include <stdlib.h>
#include <time.h>
#include <sys/wait.h>
#include <unistd.h>
int main(int argc, char **argv) {
    if (argc < 2) { fprintf(stderr, "usage: %s <cmd...>\n", argv[0]); return 1; }
    struct timespec start, end;
    clock_gettime(CLOCK_MONOTONIC, &start);
    pid_t pid = fork();
    if (pid == 0) { execvp(argv[1], &argv[1]); _exit(127); }
    int status; waitpid(pid, &status, 0);
    clock_gettime(CLOCK_MONOTONIC, &end);
    double elapsed = (end.tv_sec - start.tv_sec) + (end.tv_nsec - start.tv_nsec) / 1e9;
    printf("%.6f\n", elapsed); fflush(stdout);
    return WIFEXITED(status) ? WEXITSTATUS(status) : 1;
}
CEOF
    gcc -O2 -o "$TIMER_BIN" "$TIMER_SRC" 2>/dev/null
fi

NANOSECONDS=15 # 15-digit nanoseconds for bc precision
bench_self_term() {
    local name="$1"

    local brief_sum=0; local brief_min=999999; local brief_max=0
    local c_sum=0

    for i in 1 2 3 4 5; do
        local bt=$(env BOUND=50000000 "$TIMER_BIN" ./benchmarks/"$name")
        local ct=$(env BOUND=50000000 "$TIMER_BIN" ./benchmarks/"${name}_c")
        brief_sum=$(echo "$brief_sum + $bt" | bc)
        c_sum=$(echo "$c_sum + $ct" | bc)
        if (( $(echo "$bt < $brief_min" | bc -l) )); then brief_min=$bt; fi
        if (( $(echo "$bt > $brief_max" | bc -l) )); then brief_max=$bt; fi
    done

    local brief_avg=$(echo "scale=4; $brief_sum / 5" | bc)
    local c_avg=$(echo "scale=4; $c_sum / 5" | bc)

    local winner="—"
    local ratio="N/A"
    if [ "$c_avg" != "0.0000" ] && [ "$brief_avg" != "0.0000" ]; then
        ratio=$(echo "scale=2; $brief_avg / $c_avg" | bc)
        if (( $(echo "$ratio < 1.0" | bc -l) )); then
            winner="Brief"
        elif (( $(echo "$ratio > 1.0" | bc -l) )); then
            winner="C"
        else
            winner="~tie"
        fi
    elif [ "$brief_avg" = "0.0000" ] && [ "$c_avg" != "0.0000" ]; then
        ratio="Brief wins (O(1) fold)"
        winner="Brief"
    fi

    echo ""
    echo "=== $name ==="
    echo "  Brief: ${brief_avg}s  (min ${brief_min}s, max ${brief_max}s)"
    echo "  C:     ${c_avg}s"
    echo "  Ratio: ${ratio}x  →  ${winner} wins"
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
echo "  5 iterations per benchmark, avg wall clock via CLOCK_MONOTONIC."
echo "  BOUND=50000000. Nanosecond-precision fork+exec timing harness."
echo "================================================"
