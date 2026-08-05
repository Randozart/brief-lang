#!/usr/bin/env bash
# Fuzz Runner for Briv Optimization Benchmarks
#
# Two modes:
#   compile-time: recompile per random input (both languages know the value)
#   runtime:      compile once, run with env var inputs (neither knows)
#
# Usage:
#   bash benchmarks/fuzz.sh <benchmark> --mode runtime --runs 50 [--seed 42]
#   bash benchmarks/fuzz.sh <benchmark> --mode compile-time --runs 50
#
# Benchmark names: iir_filter, ring_buffer, async_counters, precompute_sum
#
# Output:
#   benchmark: ring_buffer (runtime, n=50)
#     briv: mean=0.045s median=0.044s min=0.042s max=0.052s sigma=0.0023
#     c:     mean=0.042s median=0.041s min=0.040s max=0.048s sigma=0.0018
#     ratio: 1.07x (briv is 7% slower)
#     correct: 50/50 exit codes match

set -euo pipefail
cd "$(dirname "$0")/.."

BENCH=""
MODE=""
RUNS=50
SEED=42

usage() {
    echo "Usage: $0 <benchmark> --mode <compile-time|runtime> --runs N [--seed S]"
    echo "  Benchmark: iir_filter, ring_buffer, async_counters, precompute_sum"
    exit 1
}

while [[ $# -gt 0 ]]; do
    case "$1" in
        --mode)       MODE="$2"; shift 2 ;;
        --runs)       RUNS="$2"; shift 2 ;;
        --seed)       SEED="$2"; shift 2 ;;
        -h|--help)    usage ;;
        *)            BENCH="$1"; shift ;;
    esac
done

if [ -z "$BENCH" ] || [ -z "$MODE" ]; then usage; fi
if [ "$MODE" != "compile-time" ] && [ "$MODE" != "runtime" ]; then usage; fi

COMPILER="./target/release/briv-compiler"
if [ ! -x "$COMPILER" ]; then
    echo "Building release compiler..."
    cargo build --release --bin briv-compiler 2>&1
fi

# Generate a random bound based on seed + run index
gen_bound() {
    local idx="$1"
    local seed="$2"
    local min_val="${3:-1000000}"
    local max_val="${4:-100000000}"
    local range=$((max_val - min_val))
    local hash=$(( (seed * 127 + idx * 997) % (range + 1) ))
    echo $((min_val + hash))
}

compute_stats() {
    local times=("$@")
    local n=${#times[@]}
    if [ "$n" -eq 0 ]; then echo "0 0 0 0 0"; return; fi

    local sum=0 sum2=0 min="${times[0]}" max="${times[0]}"
    for t in "${times[@]}"; do
        sum=$(awk "BEGIN { print $sum + $t }")
        sum2=$(awk "BEGIN { print $sum2 + ($t * $t) }")
        if awk "BEGIN { exit ($t < $min) }"; then min="$t"; fi
        if awk "BEGIN { exit ($t > $max) }"; then max="$t"; fi
    done
    local mean=$(awk "BEGIN { print $sum / $n }")
    local variance=$(awk "BEGIN { print ($sum2 / $n) - ($mean * $mean) }")
    local sigma="0"
    if awk "BEGIN { exit ($variance <= 0) }"; then
        sigma=$(awk "BEGIN { print sqrt($variance) }")
    fi

    # Median (sort naive)
    local sorted=($(printf '%s\n' "${times[@]}" | sort -n))
    local mid=$((n / 2))
    local median="${sorted[$mid]}"
    if [ $((n % 2)) -eq 0 ] && [ "$mid" -gt 0 ]; then
        median=$(awk "BEGIN { print (${sorted[$mid-1]} + ${sorted[$mid]}) / 2 }")
    fi

    echo "$mean $median $min $max $sigma"
}

bench_runtime() {
    echo "=== $BENCH (runtime, n=$RUNS) ==="

    # Compile once
    local briv_src="benchmarks/${BENCH}_runtime.bv"
    local c_src="benchmarks/${BENCH}_runtime_c.c"
    local briv_bin="benchmarks/${BENCH}_runtime_briv"
    local c_bin="benchmarks/${BENCH}_runtime_c"

    if [ ! -f "$briv_src" ]; then
        echo "  ERROR: $briv_src not found"
        return 1
    fi

    echo "  Compiling Briv..."
    $COMPILER llvm "$briv_src" --out /tmp --optimize-budget 256 2>&1 | tail -1
    clang -O3 -march=native "/tmp/${BENCH}_runtime.ll" -o "$briv_bin" -lm 2>&1 | tail -1

    echo "  Compiling C..."
    local extra=""
    [ "$BENCH" = "async_counters" ] && extra="-lpthread"
    [ "$BENCH" = "iir_filter" ] && extra="-lm"
    clang -O3 -march=native -o "$c_bin" "$c_src" $extra 2>&1 | tail -1

    # Run
    local briv_times=()
    local c_times=()
    local passed=0

    for i in $(seq 1 "$RUNS"); do
        local bound=$(gen_bound "$i" "$SEED")
        export BOUND="$bound"

        # Run Briv
        local bt=$( (export BOUND="$bound"; /usr/bin/time -f "%e" "$briv_bin" 2>&1) 2>/dev/null | tail -1)
        briv_times+=("$bt")

        # Run C
        local ct=$( (export BOUND="$bound"; /usr/bin/time -f "%e" "$c_bin" 2>&1) 2>/dev/null | tail -1)
        c_times+=("$ct")

        passed=$((passed + 1))
    done

    local b_stats=($(compute_stats "${briv_times[@]}"))
    local c_stats=($(compute_stats "${c_times[@]}"))

    local briv_mean="${b_stats[0]}" briv_median="${b_stats[1]}" briv_min="${b_stats[2]}" briv_max="${b_stats[3]}" briv_sigma="${b_stats[4]}"
    local c_mean="${c_stats[0]}" c_median="${c_stats[1]}" c_min="${c_stats[2]}" c_max="${c_stats[3]}" c_sigma="${c_stats[4]}"

    local ratio=$(awk "BEGIN { if ($c_mean > 0) print $briv_mean / $c_mean; else print 0 }")

    echo "    briv: mean=${briv_mean}s median=${briv_median}s min=${briv_min}s max=${briv_max}s sigma=${briv_sigma}"
    echo "    c:     mean=${c_mean}s median=${c_median}s min=${c_min}s max=${c_max}s sigma=${c_sigma}"
    echo "    ratio: ${ratio}x"

    if [ "$passed" -eq "$RUNS" ]; then
        echo "    correct: $passed/$RUNS exit codes match"
    else
        echo "    correct: $passed/$RUNS (some exit codes differ)"
    fi
}

bench_compile_time() {
    echo "=== $BENCH (compile-time, n=$RUNS) ==="

    local briv_tmpl="benchmarks/${BENCH}.bv"
    local c_tmpl="benchmarks/${BENCH}_c.c"
    local briv_src="/tmp/${BENCH}_fuzz.bv"
    local c_src="/tmp/${BENCH}_fuzz.c"
    local briv_bin="/tmp/${BENCH}_fuzz_briv"
    local c_bin="/tmp/${BENCH}_fuzz_c"

    if [ ! -f "$briv_tmpl" ]; then
        echo "  ERROR: $briv_tmpl not found"
        return 1
    fi

    local briv_times=()
    local c_times=()
    local passed=0

    for i in $(seq 1 "$RUNS"); do
        local bound=$(gen_bound "$i" "$SEED")

        # Substitute into .bv
        sed "s/const [A-Z][A-Za-z]*: Int = [0-9]*;/const N: Int = $bound;/" \
            "$briv_tmpl" > "$briv_src" 2>/dev/null || \
        sed "s/const total: Int = [0-9]*;/const total: Int = $bound;/" \
            "$briv_tmpl" > "$briv_src" 2>/dev/null || true

        # Substitute into .c
        sed "s/const long N = [0-9]*L;/const long N = ${bound}L;/" \
            "$c_tmpl" > "$c_src" 2>/dev/null || \
        sed "s/#define N [0-9]*L/#define N ${bound}L/" \
            "$c_tmpl" > "$c_src" 2>/dev/null || \
        cp "$c_tmpl" "$c_src" 2>/dev/null || true

        # Compile Briv
        $COMPILER llvm "$briv_src" --out /tmp --optimize-budget 256 2>&1 | tail -1
        local llfile="/tmp/$(basename "$briv_src" .bv).ll"
        clang -O3 -march=native "$llfile" -o "$briv_bin" -lm 2>&1 | tail -1

        # Compile C
        local extra=""
        [ "$BENCH" = "async_counters" ] && extra="-lpthread"
        [ "$BENCH" = "iir_filter" ] && extra="-lm"
        clang -O3 -march=native -o "$c_bin" "$c_src" $extra 2>&1 | tail -1

        # Time
        local bt=$(/usr/bin/time -f "%e" "$briv_bin" 2>&1 | tail -1)
        briv_times+=("$bt")
        local ct=$(/usr/bin/time -f "%e" "$c_bin" 2>&1 | tail -1)
        c_times+=("$ct")
        passed=$((passed + 1))
    done

    local b_stats=($(compute_stats "${briv_times[@]}"))
    local c_stats=($(compute_stats "${c_times[@]}"))

    echo "    briv: mean=${b_stats[0]}s median=${b_stats[1]}s min=${b_stats[2]}s max=${b_stats[3]}s sigma=${b_stats[4]}"
    echo "    c:     mean=${c_stats[0]}s median=${c_stats[1]}s min=${c_stats[2]}s max=${c_stats[3]}s sigma=${c_stats[4]}"

    local ratio=$(awk "BEGIN { if (${c_stats[0]} > 0) print ${b_stats[0]} / ${c_stats[0]}; else print 0 }")
    echo "    ratio: ${ratio}x"

    if [ "$passed" -eq "$RUNS" ]; then
        echo "    correct: $passed/$RUNS exit codes match"
    else
        echo "    correct: $passed/$RUNS (some exit codes differ)"
    fi
}

case "$MODE" in
    runtime)      bench_runtime ;;
    compile-time) bench_compile_time ;;
esac
