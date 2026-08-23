#!/usr/bin/env bash
# 2026-08-23 (plan 2026-08-23-vm-compile-tail-parity §1.2): compile-tail
# parity harness — the VM's conformance metric.
#
# For each fixture in tmp_fixtures/parity/*.bv:
#   1. HOST side:    the // EXPECT: line is cross-checked against an
#                    independent evaluator (cargo test parity_expected_values)
#   2. TAMER side:   brievc bounty packages it; native tamer executes;
#                    stdout must equal the EXPECT list exactly.
#
# A mismatch in either direction fails. Add one fixture per new opcode.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
cd "$ROOT"

# Build the native tamer + driver once (see tools/bounty_e2e.sh).
./target/release/brievc build lib/tamer/main.bv --out "$WORK" >/dev/null 2>&1
clang -O2 -c "$WORK/main.ll" -o "$WORK/tamer_main.o" 2>/dev/null
clang -O2 -D_GNU_SOURCE -c tamer/install_sim.c -o "$WORK/sim.o" 2>/dev/null
clang -O2 "$WORK/sim.o" "$WORK/tamer_main.o" lib/runtime/briev_rt.c \
    -o "$WORK/install_sim" -lm -Wl,--allow-multiple-definition 2>/dev/null

FAIL=0
for FIX in tmp_fixtures/parity/*.bv; do
    NAME="$(basename "$FIX" .bv)"
    EXPECTED="$(rg -o '// EXPECT: .*' "$FIX" | head -1 | sed 's|// EXPECT: ||')"

    ./target/release/brievc bounty "$FIX" >/dev/null 2>&1
    BOUNTY="${FIX%.bv}.bounty"
    [ -f "$BOUNTY" ] || { echo "PARITY $NAME: FAIL (bounty not produced)"; FAIL=1; continue; }

    ACTUAL="$(timeout 20 "$WORK/install_sim" "$BOUNTY" 2>/dev/null)"
    rm -f "$BOUNTY"
    if [ "$ACTUAL" = "$(echo "$EXPECTED" | tr ',' '\n')" ]; then
        echo "PARITY $NAME: ok ($(echo "$EXPECTED" | tr ',' ' '))"
    else
        echo "PARITY $NAME: FAIL"
        echo "  expected: $EXPECTED"
        echo "  actual:   $ACTUAL" | tr '\n' ' '; echo
        FAIL=1
    fi
done
exit $FAIL
