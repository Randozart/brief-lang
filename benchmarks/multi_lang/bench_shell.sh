#!/usr/bin/env bash
# Shell protocol bridge benchmark
# Measures the cost of spawning a subprocess per call
# Usage: bash benchmarks/multi_lang/bench_shell.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BUILD_DIR="$(cd "$SCRIPT_DIR/../../target/multi_lang" && pwd)"
SHIM="$BUILD_DIR/proto_shim"

if [ ! -f "$SHIM" ]; then
    echo "  (no proto shim, run 'make -C benchmarks/multi_lang' first)"
    exit 0
fi

echo "  proto_shim per-call latency (fork+exec+wait)"

N=50
total=0
min=999999999
max=0

for i in $(seq 1 $N); do
    t0=$(date +%s%N)
    result=$(echo "add 3 4" | "$SHIM" 2>/dev/null)
    t1=$(date +%s%N)
    elapsed=$((t1 - t0))
    total=$((total + elapsed))

    # Verify correctness on first iteration
    if [ "$i" -eq 1 ] && [ "$result" != "7" ]; then
        echo "    ERROR: expected 7, got $result"
        exit 1
    fi

    if [ "$elapsed" -lt "$min" ]; then min=$elapsed; fi
    if [ "$elapsed" -gt "$max" ]; then max=$elapsed; fi
done

avg=$((total / N))
printf "    avg=%dns  min=%dns  max=%dns  result=7\n" "$avg" "$min" "$max"
