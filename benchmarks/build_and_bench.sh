#!/usr/bin/env bash
# Brief Optimization Benchmarks — Builds, times, and tags all benchmarks
#
# Every benchmark is tagged as either --runtime (FFI in hot loop, timing valid)
# or --optimizer (pure computation, LLVM may eliminate loop, timing meaningless).
# The harness detects precomputed binaries by size comparison and reports
# correctness for all.
#
# Usage:
#   bash benchmarks/build_and_bench.sh                          # all benchmarks
#   bash benchmarks/build_and_bench.sh --runtime                # runtime only
#   bash benchmarks/build_and_bench.sh --optimizer              # optimizer only
#   bash benchmarks/build_and_bench.sh --correctness            # output verification only
#   bash benchmarks/build_and_bench.sh iir_filter               # single benchmark
#   bash benchmarks/build_and_bench.sh --fuzz <N>               # with fuzzing

set -euo pipefail
cd "$(dirname "$0")/.."

MODE="all"
SELECTED_BENCH=""
FUZZ_N=""
CORRECTNESS_ONLY=false

while [[ $# -gt 0 ]]; do
    case "$1" in
        --fuzz) FUZZ_N="$2"; shift 2 ;;
        --runtime)   MODE="runtime"; shift ;;
        --optimizer) MODE="optimizer"; shift ;;
        --correctness) CORRECTNESS_ONLY=true; shift ;;
        all)   MODE="all"; shift ;;
        *)     SELECTED_BENCH="$1"; MODE="single"; shift ;;
    esac
done

# ── TAGS ──────────────────────────────────────────────────────────────
# Every benchmark must be tagged as runtime or optimizer.
#   runtime:   FFI call in hot loop body → timing is meaningful
#   optimizer: All const inputs, no FFI in hot loop → LLVM may eliminate

declare -A TAG
TAG[iir_filter]=optimizer
TAG[precompute_sum]=optimizer
TAG[const_heavy]=optimizer
TAG[ring_buffer]=runtime
TAG[async_counters]=runtime
TAG[async_counters_runtime]=runtime
TAG[float_math]=runtime
TAG[float_math_nonzero]=runtime
TAG[sparse_dispatch]=runtime
TAG[print_loop]=runtime
TAG[nbody_newton]=runtime
TAG[nbody_sqrt]=runtime
TAG[fasta]=runtime
TAG[fannkuch_redux]=runtime
TAG[mandelbrot]=runtime
TAG[kalman_filter_runtime]=runtime
TAG[knucleotide]=runtime
TAG[cancel_math]=runtime
TAG[bit_clear]=runtime
TAG[queue_drain]=runtime
TAG[queue_drain_sym]=runtime
TAG[queue_drain_idio]=runtime
TAG[interval_step]=runtime
TAG[iir_filter_runtime]=runtime
TAG[ring_buffer_runtime]=runtime
TAG[precompute_sum_runtime]=runtime
TAG[binary_trees]=runtime

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
    "knucleotide"
    "cancel_math"
    "bit_clear"
    "queue_drain"
    "queue_drain_sym"
    "queue_drain_idio"
    "interval_step"
)

# ── BUILD FUNCTIONS ───────────────────────────────────────────────────

build_bench() {
    local name="$1"

    echo ""
    echo "================================================"
    echo "  Building: $name  (tag: ${TAG[$name]})"
    echo "================================================"

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
    if [ -f "$bin" ]; then
        echo "  Brief binary ready."
    else
        echo "  (no binary — linking deferred)"
    fi
}

build_c() {
    local name="$1"
    local src="benchmarks/${name}_c.c"

    if [ ! -f "$src" ]; then
        echo "  (no C reference — skipping)"
        return
    fi

    local extra_flags=""
    case "$name" in
        iir_filter)      extra_flags="-lm" ;;
        nbody_sqrt)      extra_flags="-lm" ;;
        fasta)           extra_flags="-lm" ;;
        fannkuch_redux)  extra_flags="-lm" ;;
        mandelbrot)      extra_flags="-lm" ;;
        kalman_filter_runtime) extra_flags="-lm" ;;
        knucleotide)      extra_flags="-lm" ;;
    esac

    clang -O3 -march=native -ffast-math -o "benchmarks/${name}_c" "$src" ${extra_flags} 2>&1
    echo "  C binary ready."
}

# ── TIMING HARNESS ────────────────────────────────────────────────────

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

# ── SIZE-GATED PRECOMPUTE DETECTION ──────────────────────────────────

# Returns true (0) if the benchmark is precompute_ok.
# Checks: brief binary .text size < 25% of C binary .text size,
#         or brief binary is missing (linking failed).
is_precompute_ok() {
    local name="$1"
    local brief_bin="benchmarks/${name}"
    local c_bin="benchmarks/${name}_c"

    # Optimizer-tagged benchmarks are always precompute_ok
    if [ "${TAG[$name]}" = "optimizer" ]; then
        return 0
    fi

    # Missing brief binary → no timing possible
    if [ ! -f "$brief_bin" ]; then
        return 0
    fi

    # Size comparison
    local brief_text=0
    local c_text=1
    if command -v size &>/dev/null; then
        brief_text=$(size "$brief_bin" 2>/dev/null | tail -1 | awk '{print $1}')
        c_text=$(size "$c_bin" 2>/dev/null | tail -1 | awk '{print $1}')
        if [ "$c_text" -gt 0 ] && [ "$brief_text" -lt $(( c_text / 4 )) ]; then
            return 0
        fi
    fi

    return 1
}

# ── CORRECTNESS CHECK ────────────────────────────────────────────────

check_correctness() {
    local name="$1"
    local brief_bin="benchmarks/${name}"
    local c_bin="benchmarks/${name}_c"

    if [ ! -f "$brief_bin" ] || [ ! -f "$c_bin" ]; then
        echo "  correctness: SKIP (binary missing)"
        return
    fi

    local brief_out c_out
    brief_out=$(BOUND=5 timeout 10 "$brief_bin" 2>&1 || echo "__FAIL__")
    c_out=$(BOUND=5 timeout 10 "$c_bin" 2>&1 || echo "__FAIL__")

    if [ "$brief_out" = "$c_out" ]; then
        echo "  correctness: MATCH (output: \"${brief_out:0:40}\")"
    else
        echo "  correctness: MISMATCH"
        echo "    brief: \"${brief_out:0:60}\""
        echo "    c:     \"${c_out:0:60}\""
    fi
}

# ── BENCHMARK RUNNER ─────────────────────────────────────────────────

bench_self_term() {
    local name="$1"
    local brief_bin="benchmarks/${name}"
    local c_bin="benchmarks/${name}_c"

    echo ""
    echo "=== $name ==="

    # Check for precomputed
    if is_precompute_ok "$name"; then
        local brief_text=0
        local c_text=1
    if command -v size &>/dev/null; then
        if [ -f "$brief_bin" ]; then
            brief_text=$(size "$brief_bin" 2>/dev/null | tail -1 | awk '{print $1}')
        fi
        if [ -f "$c_bin" ]; then
            c_text=$(size "$c_bin" 2>/dev/null | tail -1 | awk '{print $1}')
        fi
    fi
    if [ -f "$brief_bin" ]; then
        echo "  brief binary: ${brief_text:-0}B  (precompute_ok — skip runtime)"
    else
        echo "  brief binary: (no binary — linking issue)"
    fi
    echo "  c binary:     ${c_text:-0}B"
        check_correctness "$name"
        return
    fi

    if [ ! -f "$brief_bin" ]; then
        echo "  SKIP — no brief binary (linking issue)"
        return
    fi
    if [ ! -f "$c_bin" ]; then
        echo "  SKIP — no C binary"
        return
    fi

    local brief_sum=0; local brief_min=999999; local brief_max=0
    local c_sum=0

    for i in 1 2 3 4 5; do
        local bt=$(env BOUND=50000000 "$TIMER_BIN" "$brief_bin")
        local ct=$(env BOUND=50000000 "$TIMER_BIN" "$c_bin")
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

    echo "  Brief: ${brief_avg}s  (min ${brief_min}s, max ${brief_max}s)"
    echo "  C:     ${c_avg}s"
    echo "  Ratio: ${ratio}x  →  ${winner} wins"

    check_correctness "$name"
}

# ── FILTER ────────────────────────────────────────────────────────────

filter_name() {
    local name="$1"
    if [ "$MODE" = "single" ] && [ "$name" != "$SELECTED_BENCH" ]; then
        return 1
    fi
    if [ "$MODE" = "runtime" ] && [ "${TAG[$name]}" != "runtime" ]; then
        return 1
    fi
    if [ "$MODE" = "optimizer" ] && [ "${TAG[$name]}" != "optimizer" ]; then
        return 1
    fi
    return 0
}

# ── MAIN ──────────────────────────────────────────────────────────────

echo "=== Building Brief compiler (release) ==="
cargo build --release --bin brief-compiler 2>&1
echo ""

for name in "${BENCHMARKS[@]}"; do
    filter_name "$name" || continue
    if [ "$CORRECTNESS_ONLY" = true ]; then
        continue  # skip build in correctness-only mode
    fi
    build_bench "$name"
    build_c "$name"
done

echo ""
echo "================================================"
echo "  RUNNING BENCHMARKS"
echo "================================================"

for name in "${BENCHMARKS[@]}"; do
    filter_name "$name" || continue
    bench_self_term "$name"
done

if [ -n "$FUZZ_N" ]; then
    echo ""
    echo "================================================"
    echo "  FUZZING (n=$FUZZ_N)"
    echo "================================================"
    for name in "${BENCHMARKS[@]}"; do
        filter_name "$name" || continue
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
echo "  Tags: runtime=FFI in hot loop, optimizer=precompute_ok."
echo "================================================"
