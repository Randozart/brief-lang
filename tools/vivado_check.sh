#!/usr/bin/env bash
# 2026-08-23 (plan 2026-08-23-circt-toolchain-validation): Vivado check —
# compile (xvlog) and optionally SYNTHESIZE generated RTL through real
# Xilinx tooling. Proves the emitted hardware description is acceptable to
# industry tools, not just our own parser.
#
# Usage:
#   tools/vivado_check.sh <top.sv|dir-of-sv> [part]
# Env:
#   VIVADO_BIN  — path to Vivado bin dir
#                 (default /mnt/data/tools/Xilinx/Vivado/2023.1/bin)
#   VIVADO_PART — FPGA part for synthesis
#                 (default xck26-sfvc784-2LV-c — the imp_kv260 board)
set -uo pipefail
VIVADO_BIN="${VIVADO_BIN:-/mnt/data/tools/Xilinx/Vivado/2023.1/bin}"
PART="${2:-${VIVADO_PART:-xck26-sfvc784-2LV-c}}"

if [ ! -x "$VIVADO_BIN/xvlog" ]; then
    echo "vivado-check: SKIP — xvlog not found at $VIVADO_BIN"
    exit 0
fi

TARGET="${1:?usage: vivado_check.sh <file.sv|dir> [part]}"
FAIL=0

mapfile -t FILES < <(if [ -d "$TARGET" ]; then find "$TARGET" -maxdepth 1 -name '*.sv' | sort; else echo "$TARGET"; fi)
[ ${#FILES[@]} -gt 0 ] || { echo "vivado-check: no .sv files in $TARGET"; exit 1; }

WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT

echo "[vivado] xvlog (${#FILES[@]} file(s))"
if ! (cd "$WORK" && "$VIVADO_BIN/xvlog" -sv "${FILES[@]}" > "$WORK/xvlog.log" 2>&1); then
    echo "vivado-check: FAIL (xvlog)"
    grep -m5 "^ERROR" "$WORK/xvlog.log" | sed 's/^/    /'
    exit 1
fi

if [ "${VIVADO_SYNTH:-0}" = "1" ]; then
    TOP_MOD="${TOP_MODULE:-top}"
    cat > "$WORK/synth.tcl" << TCL
read_verilog -sv ${FILES[*]}
set_part $PART
synth_design -top $TOP_MOD
report_utilization -file $WORK/util.rpt
exit
TCL
    echo "[vivado] synth_design -top $TOP_MOD -part $PART (minutes…)"
    if ! (cd "$WORK" && "$VIVADO_BIN/vivado" -mode batch -nojournal -log "$WORK/vivado.log" \
          -source "$WORK/synth.tcl" > "$WORK/viv.out" 2>&1); then
        echo "vivado-check: FAIL (synth_design)"
        grep -m5 "^ERROR" "$WORK/viv.out" | sed 's/^/    /'
        exit 1
    fi
    echo "[vivado] synthesis OK — utilization in $WORK/util.rpt"
    # 2026-08-25: optional persistent copy (the EXIT trap wipes $WORK).
    if [ -n "${VIVADO_REPORT_DIR:-}" ]; then
        mkdir -p "$VIVADO_REPORT_DIR"
        cp "$WORK/util.rpt" "$VIVADO_REPORT_DIR/util.rpt"
        cp "$WORK/vivado.log" "$VIVADO_REPORT_DIR/vivado.log"
        echo "[vivado] reports preserved in $VIVADO_REPORT_DIR"
    fi
fi

echo "vivado-check: PASS"
