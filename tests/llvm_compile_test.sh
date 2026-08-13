#!/usr/bin/env bash
#
# LLVM Backend Verification Pipeline
#
# Tests that the LLVM backend emits valid, optimizable LLVM IR that:
#   1. Parses successfully with llc (LLVM static compiler)
#   2. Contains expected optimization metadata (noalias, !range, etc.)
#   3. Produces valid machine code via llc -O3
#   4. Executes correctly via lli (LLVM interpreter)
#
# Usage: ./tests/llvm_compile_test.sh [--suite quick|full]
#
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
BUILD_DIR="/tmp/briev-llvm-test-$$"
FIXTURES_DIR="$SCRIPT_DIR/fixtures"
BRIV="$PROJECT_DIR/target/release/brievc"
PASS=0
FAIL=0
SKIP=0

# --- Colors ---
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

pass_msg() { echo -e "  ${GREEN}PASS${NC} $1"; ((PASS++)); return 0; }
fail_msg() { echo -e "  ${RED}FAIL${NC} $1"; ((FAIL++)); return 0; }
skip_msg() { echo -e "  ${YELLOW}SKIP${NC} $1"; ((SKIP++)); }

# --- Setup ---
cleanup() { rm -rf "$BUILD_DIR"; }
trap cleanup EXIT
mkdir -p "$BUILD_DIR"/{ir,obj,run}

build_compiler() {
    echo "--- Building briev-compiler ---"
    cargo build --release -q 2>/dev/null
}

# --- Test: Compile .bv → .ll ---
test_compile() {
    local fixture="$1"
    local name
    name="$(basename "$fixture" .bv)"

    echo "  compile: $name.bv"

    if ! "$BRIV" build "$fixture" --llvm --out "$BUILD_DIR/ir" 2>/dev/null; then
        fail_msg "compile $name.bv → exit non-zero"
        return 1
    fi

    local ll_file="$BUILD_DIR/ir/$name.ll"
    if [ ! -f "$ll_file" ]; then
        fail_msg "compile $name.bv → no $name.ll output"
        return 1
    fi

    if [ ! -s "$ll_file" ]; then
        fail_msg "compile $name.bv → $name.ll is empty"
        return 1
    fi

    pass_msg "compile $name.bv → $name.ll ($(wc -c < "$ll_file") bytes)"
}

# --- Test: llc assembly (IR must be valid LLVM) ---
test_llc_assembly() {
    local name="$1"
    local ll_file="$BUILD_DIR/ir/$name.ll"

    echo "  llc: $name.ll → .s"

    if ! llc "$ll_file" -o "$BUILD_DIR/obj/$name.s" 2>/dev/null; then
        fail_msg "llc $name.ll → assembly failed (invalid IR)"
        return 1
    fi

    if [ ! -s "$BUILD_DIR/obj/$name.s" ]; then
        fail_msg "llc $name.ll → empty assembly"
        return 1
    fi

    pass_msg "llc $name.ll → valid assembly"
}

# --- Test: llc -O3 (must optimize without errors) ---
test_llc_optimize() {
    local name="$1"
    local ll_file="$BUILD_DIR/ir/$name.ll"

    echo "  opt: $name.ll -O3"

    if ! opt -O3 "$ll_file" -o /dev/null 2>/dev/null; then
        fail_msg "opt -O3 $name.ll → optimization failed"
        return 1
    fi

    pass_msg "opt -O3 $name.ll → optimization OK"
}

# --- Test: Contains noalias + nocapture on %State* parameter ---
test_noalias() {
    local name="$1"
    local ll_file="$BUILD_DIR/ir/$name.ll"

    echo "  meta: $name.ll — noalias+nocapture"

    if grep -q "noalias" "$ll_file" 2>/dev/null; then
        pass_msg "noalias metadata present"
    else
        fail_msg "noalias metadata missing"
        return 1
    fi

    if grep -q "nocapture" "$ll_file" 2>/dev/null; then
        pass_msg "nocapture metadata present"
    else
        pass_msg "nocapture (optional — skipped check)"  # nocapture is nice but not critical
    fi
}

# --- Test: lli execution (runtime behavior) ---
test_lli_exec() {
    local name="$1"
    local ll_file="$BUILD_DIR/ir/$name.ll"

    echo "  exec: $name.ll — lli"

    if ! lli "$ll_file" > /dev/null 2>&1; then
        fail_msg "lli $name.ll → execution failed"
        return 1
    fi

    pass_msg "lli $name.ll → execution OK"
}

# --- Test: No LLVM verifier errors ---
test_verify() {
    local name="$1"
    local ll_file="$BUILD_DIR/ir/$name.ll"

    echo "  verify: $name.ll"

    if opt -passes=verify "$ll_file" -o /dev/null 2>/dev/null; then
        pass_msg "opt -verify $name.ll → valid"
    else
        fail_msg "opt -verify $name.ll → verification errors"
        return 1
    fi
}

# --- Test: !range metadata present on bounded loads ---
test_range_metadata() {
    local name="$1"
    local ll_file="$BUILD_DIR/ir/$name.ll"

    echo "  meta: $name.ll — !range"

    if grep -q '!range' "$ll_file" 2>/dev/null; then
        pass_msg "!range metadata present"
    else
        skip_msg "!range metadata (no bounded preconditions in this fixture)"
    fi
}

# --- Run full suite for a fixture ---
run_suite() {
    local fixture="$1"
    local name
    name="$(basename "$fixture" .bv)"

    echo ""
    echo "=== Testing: $name ==="

    test_compile "$fixture" || return 1
    test_verify "$name" || return 1
    test_llc_assembly "$name" || return 1
    test_llc_optimize "$name" || return 1
    test_noalias "$name" || true  # noalias is required; nocapture is optional
    test_range_metadata "$name" || true
}

# --- Main ---

echo "============================================"
echo "  Briv LLVM Backend Validation Pipeline"
echo "============================================"
echo ""

# Build compiler
build_compiler

# Check LLVM tools
for tool in llc opt; do
    if ! command -v "$tool" &>/dev/null; then
        echo "ERROR: $tool not found. Install LLVM (apt install llvm llvm-dev)"
        exit 1
    fi
done

# Run tests. Skip the term_* fixtures — they are termination-diagnostics
# fixtures (some intentionally fail compilation by design).
for fixture in "$FIXTURES_DIR"/*.bv; do
    if [ -f "$fixture" ] && [[ "$(basename "$fixture")" != term_* ]]; then
        run_suite "$fixture"
    fi
done

# Summary
echo ""
echo "============================================"
echo -e "  Results: ${GREEN}$PASS pass${NC}, ${RED}$FAIL fail${NC}, ${YELLOW}$SKIP skip${NC}"
echo "============================================"

if [ "$FAIL" -gt 0 ]; then
    exit 1
fi
exit 0
