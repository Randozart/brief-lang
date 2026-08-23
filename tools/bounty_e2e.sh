#!/usr/bin/env bash
# 2026-08-23 (plan 2026-08-23-vm-compile-tail-parity §1.2): bounty
# round-trip e2e — the install-time compilation pipeline, verified.
#
#   1. brievc builds the self-hosted tamer natively (lib/tamer/main.bv)
#   2. brievc bounty packages a user program (.lair + .beastpack)
#   3. install_sim drives the native tamer's exported tame() on the archive
#
# Green = one .bounty artifact runs its compile-tail on this machine —
# no per-target compiler needed. Requires: clang.
set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

PROG="${1:-$ROOT/examples/hello/main.bv}"
cd "$ROOT"

echo "[e2e] 1. native tamer"
./target/release/brievc build lib/tamer/main.bv --out "$WORK" >/dev/null 2>&1
clang -O2 -c "$WORK/main.ll" -o "$WORK/tamer_main.o" 2>/dev/null

echo "[e2e] 2. package $PROG"
# brievc writes <source-stem>.bounty NEXT TO THE SOURCE (no --out yet).
./target/release/brievc bounty "$PROG" >/dev/null 2>&1
STEM="$(basename "${PROG%.*}")"
DIR="$(cd "$(dirname "$PROG")" && pwd)"
BOUNTY="$DIR/$STEM.bounty"
[ -f "$BOUNTY" ] || { echo "[e2e] FAIL: no .bounty produced"; exit 1; }

echo "[e2e] 3. install simulation"
clang -O2 -c tamer/install_sim.c -o "$WORK/sim.o" 2>/dev/null
clang -O2 "$WORK/sim.o" "$WORK/tamer_main.o" lib/runtime/briev_rt.c \
    -o "$WORK/install_sim" -lm -Wl,--allow-multiple-definition 2>/dev/null

"$WORK/install_sim" "$BOUNTY"
RC=$?
rm -f "$BOUNTY"
exit $RC
