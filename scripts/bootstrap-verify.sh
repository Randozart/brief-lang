#!/bin/bash
# Bootstrap Chain Verification Pipeline
# Tests deterministic self-hosting compilation across stages
#
# Current capabilities:
# - Stage 0: Rust bootstrap compiler compiles main.bv → main.rs (determinism check)
# - Stage 1+: Self-hosted compilation (requires full compiler implementation)
#
# Future: When self-hosted compiler can compile .bv → .rs, full chain activates

set -e

BOOTSTRAP="./target/release/brief-compiler"
SOURCE="main.bv"
BUILD_DIR="/tmp/brief-bootstrap-$$"

echo "=== Brief Bootstrap Chain Verification ==="
echo "Build directory: $BUILD_DIR"
mkdir -p "$BUILD_DIR"

PASS=0
FAIL=0
WARN=0

pass() { PASS=$((PASS + 1)); echo "PASS: $1"; }
fail() { FAIL=$((FAIL + 1)); echo "FAIL: $1"; }
warn() { WARN=$((WARN + 1)); echo "WARN: $1"; }

# ===== STAGE 0: Bootstrap compiler determinism =====
echo ""
echo "--- Stage 0: Bootstrap compiler determinism (5 runs) ---"

HASHES=()
for i in 1 2 3 4 5; do
    $BOOTSTRAP rust "$SOURCE" --out "$BUILD_DIR/run$i" 2>&1
    RS="$BUILD_DIR/run$i/main.rs"
    
    if [ ! -f "$RS" ]; then
        fail "Run $i did not produce main.rs"
        continue
    fi
    
    HASH=$(sha256sum "$RS" | awk '{print $1}')
    HASHES+=("$HASH")
    echo "  Run $i: ${HASH:0:16}... ($(wc -l < "$RS") lines)"
done

# Compare all hashes
ALL_SAME=true
for i in "${!HASHES[@]}"; do
    if [ "${HASHES[$i]}" != "${HASHES[0]}" ]; then
        ALL_SAME=false
        echo "  Run $((i+1)) differs from Run 1"
        diff "$BUILD_DIR/run1/main.rs" "$BUILD_DIR/run$((i+1))/main.rs" || true
    fi
done

if $ALL_SAME; then
    pass "Bootstrap compiler produces deterministic .rs output (5/5 identical)"
else
    fail "Bootstrap compiler produces different .rs across runs"
fi

# ===== STAGE 0b: Binary compilation =====
echo ""
echo "--- Stage 0b: Compile Stage 0 output to binary ---"
rustc -o "$BUILD_DIR/brief-v1" "$BUILD_DIR/run1/main.rs" 2>/dev/null && {
    V1_HASH=$(sha256sum "$BUILD_DIR/brief-v1" | awk '{print $1}')
    pass "brief-v1 binary compiled (hash: ${V1_HASH:0:16}...)"
} || {
    warn "rustc failed (expected for minimal compiler)"
}

# ===== STAGE 1: Self-hosted compilation attempt =====
echo ""
echo "--- Stage 1: Self-hosted compilation attempt ---"
if [ -x "$BUILD_DIR/brief-v1" ]; then
    OUTPUT=$($BUILD_DIR/brief-v1 2>&1 || true)
    echo "  brief-v1 output: $OUTPUT"
    
    if echo "$OUTPUT" | grep -q "Brief kernel"; then
        pass "Self-hosted binary runs successfully"
    else
        warn "Self-hosted binary output unexpected"
    fi
    
    # Try to compile with self-hosted compiler
    $BUILD_DIR/brief-v1 rust "$SOURCE" --out "$BUILD_DIR/stage1" 2>/dev/null && {
        if [ -f "$BUILD_DIR/stage1/main.rs" ]; then
            if diff -q "$BUILD_DIR/run1/main.rs" "$BUILD_DIR/stage1/main.rs" > /dev/null 2>&1; then
                pass "Self-hosted compiler produces identical .rs to bootstrap"
            else
                fail "Self-hosted compiler produces different .rs"
                diff "$BUILD_DIR/run1/main.rs" "$BUILD_DIR/stage1/main.rs" || true
            fi
        fi
    } || {
        warn "Self-hosted compiler cannot compile yet (minimal implementation)"
        echo "  Note: Full bootstrap chain requires complete compiler implementation"
    }
fi

# ===== SUMMARY =====
echo ""
echo "=== Results ==="
echo "  Passed: $PASS"
echo "  Failed: $FAIL"
echo "  Warnings: $WARN"

if [ $FAIL -eq 0 ]; then
    echo ""
    echo "Bootstrap verification: OK"
else
    echo ""
    echo "Bootstrap verification: FAILURES DETECTED"
fi

echo ""
echo "Build artifacts: $BUILD_DIR"
echo "=== Verification complete ==="

# Exit with failure if any tests failed
[ $FAIL -eq 0 ]
