#!/usr/bin/env bash
# 2026-08-23 (Plan 3.5): hardware validation harness.
# Per fixture: brievc → .mlir → circt-opt parse → Verilog export → verilator
# lint → optional Vivado. Gated on circt-opt availability.
set -uo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
BRIEFC="$ROOT/target/release/brievc"
CIRCT_BIN="$ROOT/tools/circt/bin"

command -v circt-opt >/dev/null 2>&1 && CIRCT_OPT=circt-opt
[ -x "$CIRCT_BIN/circt-opt" ] && CIRCT_OPT="$CIRCT_BIN/circt-opt"
if [ -z "${CIRCT_OPT:-}" ]; then
    echo "hw-harness: SKIP — circt-opt not available"
    exit 0
fi

FAIL=0
for FIX in tmp_fixtures/hw/*.bv; do
    NAME="$(basename "$FIX" .bv)"
    WORK="$(mktemp -d)"

    if ! "$BRIEFC" build "$FIX" --backend circt >/dev/null 2>&1; then
        echo "HW $NAME: FAIL (compile)"; FAIL=1; rm -rf "$WORK"; continue
    fi
    MLIR="${FIX%.bv}.mlir"; [ -f "$MLIR" ] || MLIR="${NAME}.mlir"
    [ -f "$MLIR" ] || { echo "HW $NAME: FAIL (no .mlir)"; FAIL=1; rm -rf "$WORK"; continue; }
    mv "$MLIR" "$WORK/top.mlir"

    # Parse/layout validity
    if ! "$CIRCT_OPT" "$WORK/top.mlir" > /dev/null 2> "$WORK/opt.err"; then
        echo "HW $NAME: FAIL (circt-opt parse)"
        head -5 "$WORK/opt.err" | sed 's/^/    /'
        FAIL=1; rm -rf "$WORK"; continue
    fi

    # Lower seq + export Verilog
    SV="$WORK/top.sv"
    "$CIRCT_OPT" --lower-seq-to-sv --export-verilog "$WORK/top.mlir" > "$SV.raw" 2>/dev/null || true
    if grep -q "^endmodule$" "$SV.raw"; then
        sed '/^endmodule$/q' "$SV.raw" > "$SV"
    else
        cp "$SV.raw" "$SV"
    fi
    if [ ! -s "$SV" ]; then
        echo "HW $NAME: FAIL (empty verilog export)"
        FAIL=1; rm -rf "$WORK"; continue
    fi

    # Verilator lint
    if command -v verilator >/dev/null 2>&1 && [ -s "$SV" ]; then
        if ! verilator --lint-only -Wno-fatal --top-module top "$SV" \
             --Mdir "$WORK/vl" > "$WORK/vl.log" 2>&1; then
            echo "HW $NAME: FAIL (verilator lint)"
            grep -m3 "%Error" "$WORK/vl.log" | sed 's/^/    /'
            FAIL=1; rm -rf "$WORK"; continue
        fi
    fi

    # Vivado compile + optional synthesis
    VIVADO_BIN="${VIVADO_BIN:-/mnt/data/tools/Xilinx/Vivado/2023.1/bin}"
    if [ -x "$VIVADO_BIN/xvlog" ] && [ -s "$SV" ]; then
        if ! bash "$ROOT/tools/vivado_check.sh" "$SV" > "$WORK/viv.log" 2>&1; then
            echo "HW $NAME: FAIL (vivado xvlog)"
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
