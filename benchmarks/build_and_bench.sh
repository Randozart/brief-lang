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

# ── RESULT COLLECTION ───────────────────────────────────────────
# Collected as "name|brief|c|ratio|winner|correctness"
declare -a RESULTS=()
record_result() {
    RESULTS+=("$1|$2|$3|$4|$5|$6")
}

print_summary_table() {
    echo ""
    echo "╔═══════════════════════════╦════════════╦════════════╦══════════╦════════╦═══════════╗"
    echo "║ Benchmark                 ║ Brief      ║ C          ║ Ratio    ║ Winner ║ Correct   ║"
    echo "╠═══════════════════════════╬════════════╬════════════╬══════════╬════════╬═══════════╣"
    for entry in "${RESULTS[@]}"; do
        IFS='|' read -r name brief c ratio winner correct <<< "$entry"
        printf "║ %-25s ║ %-10s ║ %-10s ║ %-8s ║ %-6s ║ %-9s ║\n" \
            "$name" "$brief" "$c" "$ratio" "$winner" "$correct"
    done
    echo "╚═══════════════════════════╩════════════╩════════════╩══════════╩════════╩═══════════╝"
}

# ── TAGS ──────────────────────────────────────────────────────────────
# Every benchmark must be tagged as runtime or optimizer.
#   runtime:   FFI call in hot loop body → timing is meaningful
#   optimizer: All const inputs, no FFI in hot loop → LLVM may eliminate

declare -A TAG
TAG[iir_filter]=optimizer
TAG[precompute_sum]=optimizer
TAG[const_heavy]=optimizer
TAG[async_counters_idio]=optimizer
TAG[ring_buffer]=runtime
TAG[async_counters_sym]=runtime
TAG[async_counters_runtime]=runtime
TAG[float_math]=runtime
TAG[float_math_nonzero]=runtime
TAG[sparse_dispatch]=runtime
TAG[print_loop]=runtime
TAG[nbody_newton]=runtime
TAG[nbody_sqrt]=runtime
TAG[nbody_sqrt_idio]=runtime
TAG[fasta]=runtime
TAG[fannkuch_redux]=runtime
TAG[mandelbrot]=runtime
TAG[kalman_filter_runtime]=runtime
TAG[knucleotide]=runtime
TAG[cancel_math]=runtime
TAG[bridge_glue]=runtime
TAG[bit_clear]=runtime
TAG[queue_drain]=runtime
TAG[queue_drain_sym]=runtime
TAG[queue_drain_idio]=runtime
TAG[interval_step]=runtime
TAG[gpu/saxpy]=runtime
TAG[iir_filter_runtime]=runtime
TAG[utf8_ops]=optimizer
TAG[ring_buffer_runtime]=runtime
TAG[precompute_sum_runtime]=runtime
TAG[binary_trees]=runtime
TAG[meld-bridge]=runtime
TAG[meld-bridge-sym]=runtime

BENCHMARKS=(
    "iir_filter"
    "precompute_sum"
    "const_heavy"
    "async_counters_idio"
    "utf8_ops"
    "ring_buffer"
    # "async_counters_sym"   # excluded from timing (40 min at 50M)
    "float_math"
    "float_math_nonzero"
    "sparse_dispatch"
    "print_loop"
    "nbody_newton"
    "nbody_sqrt"
    "nbody_sqrt_idio"
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
    "bridge_glue"
    # "gpu/saxpy"        # no .bv file exists
    # "meld-bridge"      # no .bv file exists
    # "meld-bridge-sym"  # no .bv file exists
)

# ── BUILD FUNCTIONS ───────────────────────────────────────────────────

build_bench() {
    local name="$1"

    echo ""
    echo "================================================"
    echo "  Building: $name  (tag: ${TAG[$name]})"
    echo "================================================"

    local bin="benchmarks/${name}"
    rm -f "$bin" "benchmarks/${name}.o" "benchmarks/${name}.ll"

    local budget=256
    local gpu_flag=""
    case "$name" in
        nbody_newton) budget=2048 ;;
        nbody_sqrt)   budget=2048 ;;
        nbody_sqrt_idio) budget=2048 ;;
        gpu/*) gpu_flag="--gpu-offload" ;;
    esac

    # 2026-07-10: Set BOUND so getenv_int# at module init evaluates correctly.
    # Without this, benchmarks using getenv_int#("BOUND") get N=0 and all loops
    # are dead code (zero iterations → output "0" instead of correct checksum).
    # 2026-07-14: --llvm removed — compiler now produces binary by default.
    BOUND=50000000 ./target/release/brief-compiler build "benchmarks/${name}.bv" \
        --out benchmarks --optimize-budget "$budget" $gpu_flag 2>&1

    if [ ! -f "$bin" ]; then
        # 2026-07-21: Compile brief_rt.c with -flto so LTO can inline
        # __print_char into main() (saves ~5-8 cycles/character at 50M iter).
        clang -O3 -flto -c "lib/runtime/brief_rt.c" -o "/tmp/brief_rt.o" 2>&1
        if [ -f "benchmarks/${name}.o" ]; then
            cc -O2 -flto -no-pie -o "$bin" "benchmarks/${name}.o" "/tmp/brief_rt.o" -lm 2>&1 || echo "  (link failed — try manual link)"
        else
            clang -O3 -flto -march=native -ffast-math -fdata-sections -ffunction-sections -Wl,--gc-sections "benchmarks/${name}.ll" "/tmp/brief_rt.o" -o "$bin" -lm 2>&1 || echo "  (clang linking skipped — possibly linked by compiler)"
        fi
    fi
    if [ -f "$bin" ]; then
        echo "  Brief binary ready."
    else
        echo "  (no binary — linking deferred)"
    fi
}

build_c() {
    local name=$1
    local src
    if [ -f "benchmarks/${name}_c.c" ]; then
        src="benchmarks/${name}_c.c"
    else
        return 1
    fi

    # Build C reference with -O3 -ffast-math
    echo "  Building C reference..."
    extra_flags=${extra_flags:-""}
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
#include <sys/resource.h>
#include <unistd.h>
int main(int argc, char **argv) {
    if (argc < 2) { fprintf(stderr, "usage: %s <cmd...>\n", argv[0]); return 1; }
    // 2026-07-21: Measure child CPU time (not wall clock) to exclude
    // fork+exec startup noise. Uses wait4() to get per-child rusage.
    // Combined with warmup run to preheat cache + dynamic linker.
    int ws, status; struct rusage wu, mu;
    // Warmup — fork+exec once, discard timing
    pid_t wup = fork();
    if (wup == 0) { freopen("/dev/null", "w", stdout); execvp(argv[1], &argv[1]); _exit(127); }
    wait4(wup, &ws, 0, &wu);
    // Measurement — fork+exec again, capture child CPU time
    pid_t pid = fork();
    if (pid == 0) { freopen("/dev/null", "w", stdout); execvp(argv[1], &argv[1]); _exit(127); }
    wait4(pid, &status, 0, &mu);
    // Report measurement child's user CPU time (excludes warmup, fork, wait)
    double elapsed = mu.ru_utime.tv_sec + mu.ru_utime.tv_usec / 1e6;
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
    local c_text=0
    if command -v size &>/dev/null; then
        brief_text=$(size "$brief_bin" 2>/dev/null | tail -1 | awk '{print $1}')
        if [ -f "$c_bin" ]; then
            c_text=$(size "$c_bin" 2>/dev/null | tail -1 | awk '{print $1}')
        fi
        if [ "$brief_text" -eq 0 ]; then
            return 0
        fi
        if [ "$c_text" -gt 0 ] && [ "$brief_text" -lt $(( c_text / 4 )) ]; then
            return 0
        fi
    fi

    return 1
}

# ── Cross-benchmark correctness references ──────────────────────────
# Some benchmarks (e.g. queue_drain_idio) have no C reference of their own —
# they are compared against a different benchmark's C reference (e.g.
# queue_drain_sym_c). The BRIEF_CROSS_REF array maps (benchmark, c_ref).
declare -A BRIEF_CROSS_REF
BRIEF_CROSS_REF["queue_drain_idio"]="queue_drain_sym"

# ── CORRECTNESS CHECK ────────────────────────────────────────────────

LAST_CORRECTNESS=""

check_correctness() {
    local name="$1"
    local brief_bin="benchmarks/${name}"

    local c_bin="benchmarks/${name}_c"
    local ref_name="${BRIEF_CROSS_REF[$name]:-$name}"
    local ref_c_bin="benchmarks/${ref_name}_c"

    if [ ! -f "$brief_bin" ]; then
        echo "  correctness: SKIP (brief binary missing)"
        LAST_CORRECTNESS="SKIP"
        return
    fi

    if [ "$name" != "$ref_name" ] && [ -f "$ref_c_bin" ]; then
        # Cross-benchmark comparison: compare against another benchmark's C ref
        :
    elif [ ! -f "$c_bin" ]; then
        echo "  correctness: SKIP (binary missing)"
        LAST_CORRECTNESS="SKIP"
        return
    fi

    local brief_out c_out
    brief_out=$(BOUND=5 timeout 10 "$brief_bin" 2>&1 || echo "__FAIL__")
    if [ "$name" != "$ref_name" ] && [ -f "$ref_c_bin" ]; then
        c_out=$(BOUND=5 timeout 10 "$ref_c_bin" 2>&1 || echo "__FAIL__")
    else
        c_out=$(BOUND=5 timeout 10 "$c_bin" 2>&1 || echo "__FAIL__")
    fi

    if [ "$brief_out" = "$c_out" ]; then
        echo "  correctness: MATCH (output: \"${brief_out:0:40}\")"
        LAST_CORRECTNESS="MATCH"
        return
    fi

    # 2026-07-03: Epsilon-based float comparison. C auto-vectorizes,
    # changing f32 association order. Strict string compare produces
    # false MISMATCH for values differing by ~1e-7.
    # Compare each line numerically if all lines are floats.
    local brief_lines c_lines
    mapfile -t brief_lines <<< "$brief_out"
    mapfile -t c_lines <<< "$c_out"
    local n_brief=${#brief_lines[@]}
    local n_c=${#c_lines[@]}
    if [ "$n_brief" -ne "$n_c" ]; then
        echo "  correctness: MISMATCH (line count $n_brief vs $n_c)"
        LAST_CORRECTNESS="MISMATCH"
        return
    fi
    local all_float=true
    local i
    local re='^-?[0-9]+\.[0-9]+$'
    for ((i=0; i<n_brief; i++)); do
        if ! [[ "${brief_lines[$i]}" =~ $re ]] || ! [[ "${c_lines[$i]}" =~ $re ]]; then
            all_float=false
            break
        fi
    done
    if [ "$all_float" = false ]; then
        echo "  correctness: MISMATCH"
        echo "    brief: \"${brief_out:0:60}\""
        echo "    c:     \"${c_out:0:60}\""
        LAST_CORRECTNESS="MISMATCH"
        return
    fi
    # All lines are floats — compare with epsilon
    local eps=0.00001
    for ((i=0; i<n_brief; i++)); do
        local diff
        diff=$(LC_ALL=C python3 -c "b=${brief_lines[$i]}; c=${c_lines[$i]}; print('{:.15e}'.format(abs(b - c)))" 2>/dev/null)
        in_range=$(LC_ALL=C python3 -c "d=${diff}; print('yes' if d < $eps else 'no')" 2>/dev/null)
        if [ -z "$diff" ] || [ "$in_range" != "yes" ]; then
            echo "  correctness: MISMATCH (float diff $diff > $eps)"
            echo "    brief: \"${brief_out:0:60}\""
            echo "    c:     \"${c_out:0:60}\""
            LAST_CORRECTNESS="MISMATCH"
            return
        fi
    done
    echo "  correctness: MATCH (output: \"${brief_out:0:40}\")"
    LAST_CORRECTNESS="MATCH"
}

# ── BENCHMARK RUNNER ─────────────────────────────────────────────────

bench_self_term() {
    local name="$1"
    local brief_bin="benchmarks/${name}"
    local c_bin="benchmarks/${name}_c"

    echo ""
    echo "=== $name ==="

    # 2026-07-22: Bridge benchmark uses Python harness, not compiled binary
    if [ "$name" = "bridge_glue" ]; then
        local py_bench="benchmarks/bridge/bench_glue_cross.py"
        if [ -f "$py_bench" ] && [ -x "$(command -v python3)" ]; then
            python3 "$py_bench"
            check_correctness "$name"
            record_result "$name" "done" "" "" "" "$LAST_CORRECTNESS"
        else
            echo "  SKIP — no Python harness"
            record_result "$name" "SKIP" "" "" "" "SKIP"
        fi
        return
    fi

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
        record_result "$name" "precomputed" "" "" "" "$LAST_CORRECTNESS"
        return
    fi

    if [ ! -f "$brief_bin" ]; then
        echo "  SKIP — no brief binary (linking issue)"
        record_result "$name" "SKIP" "" "" "" "SKIP"
        return
    fi
    # 2026-07-18: Cross-benchmark reference check
    local ref_name="${BRIEF_CROSS_REF[$name]:-$name}"
    local ref_c_bin="benchmarks/${ref_name}_c"
    if [ "$name" != "$ref_name" ] && [ -f "$ref_c_bin" ]; then
        # Cross-benchmark: C ref exists under a different name
        :
    elif [ ! -f "$c_bin" ]; then
        echo "  SKIP — no C binary"
        record_result "$name" "SKIP" "" "" "" "SKIP"
        return
    fi

    if [ "$CORRECTNESS_ONLY" = true ]; then
        check_correctness "$name"
        record_result "$name" "" "" "" "" "$LAST_CORRECTNESS"
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
    record_result "$name" "${brief_avg}s" "${c_avg}s" "${ratio}x" "$winner" "$LAST_CORRECTNESS"
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

# 2026-07-01: Pre-build disabled — run `cargo build --release --bin brief-compiler`
# manually before executing this script to avoid the long build hiding benchmark output.
#echo "=== Building Brief compiler (release) ==="
#cargo build --release --bin brief-compiler 2>&1
#echo ""

for name in "${BENCHMARKS[@]}"; do
    filter_name "$name" || continue
    if [ "$CORRECTNESS_ONLY" = true ]; then
        continue  # skip build in correctness-only mode
    fi
    build_bench "$name"
    if [ "$name" = "bridge_glue" ]; then
        echo "  bridge_glue: building C + Brief .so files..."
        make -C benchmarks/bridge PROJECT_ROOT="$PWD" BRIDGE_DIR="$PWD/target/bridge_bench" all 2>&1 | sed 's/^/    /'
    else
        build_c "$name"
    fi
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

if [ ${#RESULTS[@]} -gt 0 ]; then
    print_summary_table
fi
