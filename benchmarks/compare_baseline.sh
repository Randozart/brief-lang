#!/usr/bin/env bash
# Compare a benchmark between baseline worktree and current worktree.
# Usage: bash benchmarks/compare_baseline.sh <benchmark_name>
#
# Example:
#   bash benchmarks/compare_baseline.sh nbody_newton
#
# Runs each binary 5 times and prints average times.
# Returns non-zero if current is >10% slower than baseline.

set -euo pipefail
cd "$(dirname "$0")/.."

NAME="${1:-nbody_newton}"
BASELINE_DIR="../brief-compiler-baseline"
CURRENT_DIR="."
RUNS=5

if [ ! -d "$BASELINE_DIR" ]; then
    echo "ERROR: Baseline worktree not found at $BASELINE_DIR"
    echo "Create it with: git worktree add $BASELINE_DIR 334a168"
    exit 1
fi

echo "=== Comparing $NAME ==="
echo "Baseline: $(cd $BASELINE_DIR && git rev-parse --short HEAD)"
echo "Current:  $(git rev-parse --short HEAD)"
echo ""

# Ensure both binaries exist
if [ ! -f "$BASELINE_DIR/benchmarks/$NAME" ]; then
    echo "Building baseline binary..."
    cd "$BASELINE_DIR"
    BOUND=50000000 ./target/release/brief-compiler build "benchmarks/${NAME}.bv" --out benchmarks 2>&1 | tail -1
    cd "$CURRENT_DIR"
fi
if [ ! -f "benchmarks/$NAME" ]; then
    echo "Building current binary..."
    BOUND=50000000 ./target/release/brief-compiler build "benchmarks/${NAME}.bv" --out benchmarks 2>&1 | tail -1
fi

time_binary() {
    local dir="$1"
    local name="$2"
    local total=0
    for i in $(seq 1 $RUNS); do
        local t=$(BOUND=50000000 timeout 30 bash -c "cd '$dir' && TIMEFORMAT='%3R' time ./benchmarks/$name" 2>&1 | tail -1)
        # Replace comma with dot for locale-independent parsing
        t="${t/,/.}"
        total=$(echo "$total + $t" | bc 2>/dev/null || echo "0")
        echo "  Run $i: ${t}s"
    done
    echo "$total / $RUNS" | bc -l 2>/dev/null || echo "0"
}

echo ""
echo "--- Baseline ---"
baseline_avg=$(time_binary "$BASELINE_DIR" "$NAME" | tail -1)

echo ""
echo "--- Current ---"
current_avg=$(time_binary "$CURRENT_DIR" "$NAME" | tail -1)

echo ""
echo "--- Result ---"
echo "Baseline avg: ${baseline_avg}s"
echo "Current avg:  ${current_avg}s"
ratio=$(echo "scale=4; $current_avg / $baseline_avg" | bc 2>/dev/null || echo "1.0")
echo "Ratio: $ratio (current/baseline)"

if [ "$(echo "$ratio > 1.10" | bc -l 2>/dev/null || echo "0")" = "1" ]; then
    echo "WARNING: Current is >10% slower than baseline!"
    exit 1
else
    echo "OK: Within tolerance."
    exit 0
fi
