#!/usr/bin/env bash
# 2026-08-23 (plan 2026-08-23-circt-toolchain-validation §3.5): hardware
# validation harness.
#
# Pipeline per fixture (.cbv):
#   1. brievc build  → .mlir
#   2. circt-opt     → parse check (layout/dialect validity)
#   3. circt-translate --export-verilog → .sv
#   4. verilator --lint-only   → lint the generated RTL
#   5. (Vivado, if installed) xvlog + synth-check on a small part
#
# Green = emitted hardware description is accepted by real toolchains.
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BRIEFC="$ROOT/target/release/brievc"
CIRCT_BIN="$ROOT/tools/circt/bin"

command -v circt-opt >/dev/null 2>&1 && CIRCT_OPT=circt-opt
[ -x "$CIRCT_BIN/circt-opt" ] && CIRCT_OPT="$CIRCT_BIN/circt-opt"
CIRCT_OPT="${CIRCT_OPT:-}"
if [ -z "${CIRCT_OPT:-}" ] || ! command -v "$CIRCT_OPT" >/dev/null 2>&1 && [ ! -x "$CIRCT_OPT" ]; then
    echo "hw-harness: SKIP — circt-opt not available (tools/install-circt.sh)"
    exit 0
fi
command -v circt-translate >/dev/null 2>&1 && CIRCT_TR=circt-translate
[ -x "$CIRCT_BIN/circt-translate" ] && CIRCT_TR="$CIRCT_BIN/circt-translate"

FAIL=0
for FIX in tmp_fixtures/hw/*.bv; do
    NAME="$(basename "$FIX" .bv)"
    WORK="$(mktemp -d)"

    if ! "$BRIEFC" build "$FIX" --backend circt >/dev/null 2>&1; then
        echo "HW $NAME: FAIL (compile)"; FAIL=1; rm -rf "$WORK"; continue
    fi
    MLIR="${FIX%.bv}.mlir"; [ -f "$MLIR" ] || MLIR="${NAME}.mlir"
    [ -f "$MLIR" ] || { echo "HW $NAME: FAIL (no .mlir produced)"; FAIL=1; rm -rf "$WORK"; continue; }
    mv "$MLIR" "$WORK/top.mlir"

    # 2. parse/layout validity
    if ! "$CIRCT_OPT" "$WORK/top.mlir" > /dev/null 2> "$WORK/opt.err"; then
        echo "HW $NAME: FAIL (circt-opt)"; sed 's/^/    /' "$WORK/opt.err" | head -5
        FAIL=1; rm -rf "$WORK"; continue
    fi

    # 3. Verilog export
    SV="$WORK/top.sv"
    if [ -n "${CIRCT_TR:-}" ]; then
        if ! "$CIRCT_TR" --export-verilog "$WORK/top.mlir" > "$SV" 2> "$WORK/tr.err"; then
            echo "HW $NAME: FAIL (circt-translate)"; sed 's/^/    /' "$WORK/tr.err" | head -5
            FAIL=1; rm -rf "$WORK"; continue
        fi
    fi

    # 4. verilator lint
    if command -v verilator >/dev/null 2>&1 && [ -s "$SV" ]; then
        if ! verilator --lint-only -Wno-fatal --top-module top "$SV" \
             --Mdir "$WORK/vl" > "$WORK/vl.log" 2>&1; then
            echo "HW $NAME: FAIL (verilator lint)"; grep -m3 "Error\|%Error" "$WORK/vl.log" | sed 's/^/    /'
            FAIL=1; rm -rf "$WORK"; continue
        fi
    fi

    # 5-6. Vivado compile + optional synthesis (user install; gated)
    if [ -x "${VIVADO_BIN:-/mnt/data/tools/Xilinx/Vivado/2023.1/bin}/xvlog" ] && [ -s "$SV" ]; then
        if ! bash "$ROOT/tools/vivado_check.sh" "$SV" > "$WORK/viv.log" 2>&1; then
            echo "HW $NAME: FAIL (vivado)"; sed 's/^/    /' "$WORK/viv.log" | head -5
            FAIL=1; rm -rf "$WORK"; continue
        fi
        if [ "${VIVADO_SYNTH:-0}" = "1" ]; then
            VIVADO_SYNTH=1 TOP_MODULE=top bash "$ROOT/tools/vivado_check.sh" "$SV" \
                > /dev/null 2>&1 || { echo "HW $NAME: FAIL (vivado synth)"; FAIL=1; rm -rf "$WORK"; continue; }
        fi
    fi

    echo "HW $NAME: ok"
    rm -rf "$WORK"
done
exit $FAIL
