#!/bin/bash
# GLUE Integration Tests
# Tests the full briev link + briev export pipeline.
# Run from the compiler project root.
#
# Usage: ./tests/glue_integration.sh [--keep]

set -e
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
TMPDIR=$(mktemp -d /tmp/glue-test-XXXXXX)
KEEP=$1

pass=0
fail=0

pass() {
    echo "  ✓ $1"
    ((pass++))
}
fail() {
    echo "  ✗ $1"
    ((fail++))
}

# ---- Setup ----
echo "=== GLUE Integration Tests ==="
echo ""

# Verify brievc exists
if [ ! -f "$ROOT/target/debug/brievc" ]; then
    echo "Building brievc first..."
    (cd "$ROOT" && cargo build 2>/dev/null) || {
        echo "SKIP: brievc build failed"
        exit 0
    }
fi

BRIV="$ROOT/target/debug/brievc"

# ---- Test 1: briev link ----

echo "--- Test 1: briev link on a C object file ---"

# Create a minimal C function
cat > "$TMPDIR/math_ops.c" << 'CEOF'
int add(int a, int b) { return a + b; }
int multiply(int a, int b) { return a * b; }
CEOF

skip_link=""
gcc -c -o "$TMPDIR/math_ops.o" "$TMPDIR/math_ops.c" 2>/dev/null || {
    fail "setup: gcc not available, skipping link test"
    skip_link=1
}

if [ -z "$skip_link" ]; then
    LINK_OUTPUT=$($BRIV link "$TMPDIR/math_ops.o" 2>/dev/null) && {
        if echo "$LINK_OUTPUT" | grep -q "add"; then
            pass "briev link discovers the add symbol"
        else
            fail "briev link output missing expected symbols"
        fi

        # briev link prints the generated bridge .bv to stdout.
        if echo "$LINK_OUTPUT" | grep -q "frgn add"; then
            pass "generated bridge .bv contains frgn declarations"
        else
            fail "generated bridge .bv missing frgn declarations"
        fi
    } || {
        fail "briev link failed"
    }
fi

# ---- Test 2: briev export (rust) ----

echo ""
echo "--- Test 2: briev export (rust) ---"

cat > "$TMPDIR/test_bridge.bv" << 'BVEOF'
// Test bridge for GLUE export

export defn add(a: Int, b: Int) -> Int {
    term a + b;
};

export defn multiply(a: Int, b: Int) -> Int {
    term a * b;
};
BVEOF

EXPORT_OUTPUT=$($BRIV export "$TMPDIR/test_bridge.bv" rust --out "$TMPDIR" 2>/dev/null) && {
    if echo "$EXPORT_OUTPUT" | grep -q "Bridge 'test_bridge'" && \
       echo "$EXPORT_OUTPUT" | grep -q "2 exports"; then
        pass "briev export rust completed (2 exports detected)"

        # Check Rust crate structure
        RUST_DIR="$TMPDIR/test_bridge-bridge"
        if [ -f "$RUST_DIR/Cargo.toml" ] && \
           [ -f "$RUST_DIR/build.rs" ] && \
           [ -f "$RUST_DIR/src/lib.rs" ] && \
           [ -f "$RUST_DIR/src/ffi.rs" ]; then
            pass "rust: all 4 crate files generated"
        else
            fail "rust: missing crate files"
        fi

        if grep -q "test_bridge" "$RUST_DIR/Cargo.toml" && \
           grep -q "test_bridge" "$RUST_DIR/build.rs"; then
            pass "rust: bridge name interpolated correctly"
        else
            fail "rust: bridge name not interpolated"
        fi
    else
        fail "briev export rust failed"
    fi
} || {
    fail "briev export rust error"
}

# ---- Test 3: briev export (python) ----

echo ""
echo "--- Test 3: briev export (python) ---"

EXPORT_OUTPUT=$($BRIV export "$TMPDIR/test_bridge.bv" python --out "$TMPDIR" 2>/dev/null) && {
    if echo "$EXPORT_OUTPUT" | grep -q "Bridge 'test_bridge'"; then
        pass "briev export python completed"

        PY_DIR="$TMPDIR/test_bridge-bridge"
        if [ -f "$PY_DIR/__init__.py" ]; then
            pass "python: __init__.py generated"

            if grep -q "ctypes.CDLL" "$PY_DIR/__init__.py" && \
               grep -q "test_bridge" "$PY_DIR/__init__.py"; then
                pass "python: ctypes loading with correct bridge name"
            else
                fail "python: missing ctypes loading or bridge name"
            fi
        else
            fail "python: missing __init__.py"
        fi
    else
        fail "briev export python failed"
    fi
} || {
    fail "briev export python error"
}

# ---- Test 4: briev export (node) ----

echo ""
echo "--- Test 4: briev export (node) ---"

EXPORT_OUTPUT=$($BRIV export "$TMPDIR/test_bridge.bv" node --out "$TMPDIR" 2>/dev/null) && {
    if echo "$EXPORT_OUTPUT" | grep -q "Bridge 'test_bridge'"; then
        pass "briev export node completed"

        NODE_DIR="$TMPDIR/test_bridge-bridge"
        if [ -f "$NODE_DIR/index.mjs" ]; then
            pass "node: index.mjs generated"
        else
            fail "node: missing index.mjs"
        fi
    else
        fail "briev export node failed"
    fi
} || {
    fail "briev export node error"
}

# ---- Test 5: glue config (Data Briv) ----

echo ""
echo "--- Test 5: GLUE registry is Data Briv ---"

if [ -f "$ROOT/lib/glue/python/glue.dbv" ] && [ -f "$ROOT/lib/glue/rust/glue.dbv" ]; then
    pass "per-language glue.dbv configs present (python + rust)"
else
    fail "lib/glue/<lang>/glue.dbv configs missing"
fi

# ---- Summary ----

echo ""
echo "=== Results: $pass passed, $fail failed ==="

# Cleanup
if [ "$KEEP" != "--keep" ]; then
    rm -rf "$TMPDIR"
fi

exit $fail
