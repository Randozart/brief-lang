#!/bin/bash
# Display Briev-vs-C benchmark results from build_and_bench.sh output.
# Pipe benchmark output through this script, or pass a log file as argument.
#
# Usage:
#   bash benchmarks/build_and_bench.sh --runtime 2>&1 | bash benchmarks/table.sh
#   bash benchmarks/table.sh benchmark_results.log

INPUT="${1:-/dev/stdin}"

echo ""
echo "  Briev vs C Runtime Benchmarks (BOUND=50000000, 5 iterations)"
echo "  ─────────────────────────────────────────────────────────────"
printf "  %-28s %-10s %-10s %-8s %s\n" "Benchmark" "Briev" "C" "Ratio" "Winner"
echo "  ─────────────────────────────────────────────────────────────"

grep -E '^(=== |  (Briev|C|Ratio))' "$INPUT" | while IFS= read -r line; do
  case "$line" in
    ===*)
      name=$(echo "$line" | sed 's/^=== //;s/ ===$//')
      briev=""; c=""; ratio=""; winner=""
      ;;
    Briev:*)
      briev=$(echo "$line" | sed 's/^  Briev: //;s/s  .*//')
      ;;
    C:*)
      c=$(echo "$line" | sed 's/^  C:     //;s/s$//')
      ;;
    Ratio:*)
      ratio=$(echo "$line" | sed 's/^  Ratio: //;s/x.*//')
      winner=$(echo "$line" | sed 's/.*→  //;s/ wins//')
      if [ "$winner" = "Briev" ]; then w="✓ Briev"; elif [ "$winner" = "C" ]; then w="C"; else w="—"; fi
      printf "  %-28s %-10s %-10s %-8s %s\n" "$name" "${briev}s" "${c}s" "${ratio}x" "$winner"
      ;;
  esac
done

echo "  ─────────────────────────────────────────────────────────────"
