#!/usr/bin/env bash
set -euo pipefail

# Briv Compiler — benchmark demo
# Runs the existing harness, presents a clean summary table.
# Run: bash demo.sh

HERE="$(cd "$(dirname "$0")" && pwd)"
cd "$HERE"

echo "╔══════════════════════════════════════════════════════════════╗"
echo "║              Briv Compiler — benchmark suite               ║"
echo "║  Ahead-of-time compiled, contract-driven safe language      ║"
echo "╚══════════════════════════════════════════════════════════════╝"
echo ""

# ── 1. Correctness (fast) ────────────────────────────────────────
echo "=== Correctness check ==="
echo ""
CORR_OUT=$(bash benchmarks/build_and_bench.sh --correctness 2>&1) || true

# Parse correctness results
RESULTS=$(mktemp)
echo "$CORR_OUT" | grep -E "=== |correctness:" | while read -r line; do
    if echo "$line" | grep -q "^==="; then
        bm=$(echo "$line" | sed 's/=== //;s/ ===//')
    elif echo "$line" | grep -q "correctness:"; then
        result=$(echo "$line" | sed 's/.*correctness: //')
        echo "${bm}|${result}" >> "$RESULTS"
    fi
done

echo "  Done."
echo ""

# ── 2. Runtime timing ────────────────────────────────────────────
echo "=== Runtime benchmarks ==="
echo ""
RUNTIME_OUT=$(bash benchmarks/build_and_bench.sh --runtime 2>&1) || true

# Parse runtime results
TIMING=$(mktemp)
echo "$RUNTIME_OUT" | grep -E "=== |Briv:|C:|Ratio:" | while read -r line; do
    if echo "$line" | grep -q "^==="; then
        bm=$(echo "$line" | sed 's/=== //;s/ ===//')
    elif echo "$line" | grep -q "^  Briv:"; then
        briv=$(echo "$line" | sed 's/.*Briv: //;s/s.*//' | xargs)
    elif echo "$line" | grep -q "^  C:"; then
        c=$(echo "$line" | sed 's/.*C:[[:space:]]*//' | xargs)
    elif echo "$line" | grep -q "Ratio:"; then
        ratio=$(echo "$line" | sed 's/.*Ratio: //' | xargs)
        echo "${bm}|${briv}|${c:-}|${ratio}" >> "$TIMING"
    fi
done

echo "  Done."
echo ""

# ── 3. Summary table ─────────────────────────────────────────────
echo "╔═══════════════════════╦══════════════╦══════════════╦══════════════╦══════════╗"
echo "║ Benchmark             ║ Correctness  ║ Briv        ║ C            ║ Ratio    ║"
echo "╠═══════════════════════╬══════════════╬══════════════╬══════════════╬══════════╣"

print_row() {
    printf "║ %-21s ║ %-12s ║ %-12s ║ %-12s ║ %-8s ║\n" "$1" "$2" "$3" "$4" "$5"
}

# First print nbody (key benchmarks) then others
for priority_group in "nbody_newton|nbody_sqrt|nbody_sqrt_idio|nbody_newton_sym" "all"; do
    while IFS='|' read -r bm result; do
        if [ "$priority_group" != "all" ]; then
            if ! echo "$bm" | grep -qE "^($priority_group)$"; then continue; fi
        else
            if echo "$bm" | grep -qE "^(nbody_newton|nbody_sqrt|nbody_sqrt_idio|nbody_newton_sym)$"; then continue; fi
        fi
        briv=$(grep "^${bm}|" "$TIMING" 2>/dev/null | cut -d'|' -f2 || echo "-")
        c=$(grep "^${bm}|" "$TIMING" 2>/dev/null | cut -d'|' -f3 || echo "-")
        ratio=$(grep "^${bm}|" "$TIMING" 2>/dev/null | cut -d'|' -f4 || echo "-")
        [ -z "$briv" ] && briv="-"
        [ -z "$c" ] && c="-"
        [ -z "$ratio" ] && ratio="-"
        print_row "$bm" "$result" "$briv" "$c" "$ratio"
    done < "$RESULTS"
done

echo "╚═══════════════════════╩══════════════╩══════════════╩══════════════╩══════════╝"
echo ""
echo "System: $(uname -m) $(uname -s) | $(nproc) cores"
echo "C compiler: $(clang --version 2>/dev/null | head -1 || gcc --version 2>/dev/null | head -1 || echo "none")"

rm -f "$RESULTS" "$TIMING"
