#!/usr/bin/env bash
# Protocol bridge benchmark — Tier 2: Subprocess text protocol (gen_protocol output)
# 2026-07-24: Measures per-call latency of spawning a subprocess per call.
#
# Build:
#   gcc -O2 -o out/proto_shim out/proto_shim.c -ldl
# Run:
#   bash bench_protocol.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OUT_DIR="$SCRIPT_DIR/out"
SHIM="$OUT_DIR/proto_shim"
SO="$OUT_DIR/bench_add.so"

if [ ! -f "$SHIM" ]; then
    echo "  Compiling protocol shim..."
    gcc -O2 -o "$SHIM" "$OUT_DIR/proto_shim.c" -ldl
fi

echo "============================================================"
echo "Metropolitan FFI Benchmark — Protocol Bridge (subprocess)"
echo "============================================================"

echo ""
echo "[Protocol Bridge (gen_protocol)]"

N=50
total=0
min=999999999
max=0
result=""

for i in $(seq 1 $N); do
    t0=$(date +%s%N)
    r=$(echo "add 3 4" | "$SHIM" 2>/dev/null)
    t1=$(date +%s%N)
    elapsed=$((t1 - t0))
    total=$((total + elapsed))
    result="$r"

    if [ "$i" -eq 1 ] && [ "$r" != "7" ]; then
        echo "    ERROR: expected 7, got $r"
        exit 1
    fi

    if [ "$elapsed" -lt "$min" ]; then min=$elapsed; fi
    if [ "$elapsed" -gt "$max" ]; then max=$elapsed; fi
done

avg=$((total / N))
echo "    proto_shim add          median=${avg}ns  min=${min}ns  max=${max}ns  result=${result}"
