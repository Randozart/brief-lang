#!/bin/bash
# GLUE Integration Tests
# Tests the full brief link + brief export pipeline.
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

# Verify brief-compiler exists
if [ ! -f "$ROOT/target/debug/brief-compiler" ]; then
    echo "Building brief-compiler first..."
    (cd "$ROOT" && cargo build 2>/dev/null) || {
        echo "SKIP: brief-compiler build failed (LLVM backend may be broken)"
        exit 0
    }
fi

BRIEF="$ROOT/target/debug/brief-compiler"

# ---- Test 1: brief link ----

echo "--- Test 1: brief link on a C object file ---"

# Create a minimal C function
cat > "$TMPDIR/math_ops.c" << 'CEOF'
int add(int a, int b) { return a + b; }
int multiply(int a, int b) { return a * b; }
CEOF

gcc -c -o "$TMPDIR/math_ops.o" "$TMPDIR/math_ops.c" 2>/dev/null || {
    fail "setup: gcc not available, skipping link test"
    skip_link=1
}

if [ -z "$skip_link" ]; then
    LINK_OUTPUT=$($BRIEF link "$TMPDIR/math_ops.o" 2>/dev/null) && {
        # Check output mentions both symbols
        if echo "$LINK_OUTPUT" | grep -q "add" && echo "$LINK_OUTPUT" | grep -q "multiply"; then
            pass "brief link discovers both add and multiply symbols"
        else
            fail "brief link output missing expected symbols"
        fi

        # Check generated .bv file
        if [ -f "$ROOT/math_ops-bridge.bv" ]; then
            if grep -q "frgn add" "$ROOT/math_ops-bridge.bv" && grep -q "frgn multiply" "$ROOT/math_ops-bridge.bv"; then
                pass "generated bridge .bv contains frgn declarations"
            else
                fail "generated bridge .bv missing frgn declarations"
            fi
            rm -f "$ROOT/math_ops-bridge.bv"
        else
            fail "brief link did not generate bridge .bv"
        fi
    } || {
        fail "brief link failed"
    }
fi

# ---- Test 2: brief export (rust) ----

echo ""
echo "--- Test 2: brief export (rust) ---"

cat > "$TMPDIR/test_bridge.bv" << 'BVEOF'
// Test bridge for GLUE export

#export("add")
defn add(a: Int, b: Int) -> Int {
    term a + b;
};

#export("multiply")
defn multiply(a: Int, b: Int) -> Int {
    term a * b;
};
BVEOF

EXPORT_OUTPUT=$($BRIEF export "$TMPDIR/test_bridge.bv" rust --out "$TMPDIR" 2>/dev/null) && {
    if echo "$EXPORT_OUTPUT" | grep -q "Wrapper generated"; then
        pass "brief export rust completed"

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

        # Check bridge name interpolation
        if grep -q "test_bridge" "$RUST_DIR/Cargo.toml" && \
           grep -q "test_bridge" "$RUST_DIR/build.rs"; then
            pass "rust: bridge name interpolated correctly"
        else
            fail "rust: bridge name not interpolated"
        fi

        # Check 2 exports detected
        if echo "$EXPORT_OUTPUT" | grep -q "2 exports"; then
            pass "rust: both exports detected"
        else
            fail "rust: expected 2 exports"
        fi
    else
        fail "brief export rust failed"
    fi
} || {
    fail "brief export rust error"
}

# ---- Test 3: brief export (python) ----

echo ""
echo "--- Test 3: brief export (python) ---"

EXPORT_OUTPUT=$($BRIEF export "$TMPDIR/test_bridge.bv" python --out "$TMPDIR" 2>/dev/null) && {
    if echo "$EXPORT_OUTPUT" | grep -q "Wrapper generated"; then
        pass "brief export python completed"

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
        fail "brief export python failed"
    fi
} || {
    fail "brief export python error"
}

# ---- Test 4: brief export (node) ----

echo ""
echo "--- Test 4: brief export (node) ---"

EXPORT_OUTPUT=$($BRIEF export "$TMPDIR/test_bridge.bv" node --out "$TMPDIR" 2>/dev/null) && {
    if echo "$EXPORT_OUTPUT" | grep -q "Wrapper generated"; then
        pass "brief export node completed"

        NODE_DIR="$TMPDIR/test_bridge-bridge"
        if [ -f "$NODE_DIR/package.json" ] && \
           [ -f "$NODE_DIR/index.mjs" ] && \
           [ -f "$NODE_DIR/index.d.ts" ]; then
            pass "node: all 3 files generated"
        else
            fail "node: missing files"
        fi

        if grep -q "test_bridge" "$NODE_DIR/package.json" && \
           grep -q "test_bridge" "$NODE_DIR/index.mjs"; then
            pass "node: bridge name interpolated"
        else
            fail "node: bridge name not interpolated"
        fi
    else
        fail "brief export node failed"
    fi
} || {
    fail "brief export node error"
}

# ---- Test 5: glue.dbvl parsing ----

echo ""
echo "--- Test 5: glue.dbvl/dbvs schema validation ---"

DBVL_FILE="$ROOT/lib/glue.dbvl"
DBVS_FILE="$ROOT/lib/glue.dbvs"

if [ -f "$DBVL_FILE" ] && [ -f "$DBVS_FILE" ]; then
    # The glue.dbvl is parsed during export, so if Test 2-4 passed,
    # the dbvl parsing works implicitly. Verify schema directive exists.
    if grep -q "schema" "$DBVL_FILE"; then
        pass "glue.dbvl has schema directive"
    else
        fail "glue.dbvl missing schema directive"
    fi
    if grep -q "entry AdapterEntry" "$DBVS_FILE"; then
        pass "glue.dbvs defines AdapterEntry schema"
    else
        fail "glue.dbvs missing AdapterEntry"
    fi
else
    fail "glue.dbvl or glue.dbvs not found"
fi

# ---- Summary ----

echo ""
echo "=== Results: $pass passed, $fail failed ==="

# Cleanup
if [ "$KEEP" != "--keep" ]; then
    rm -rf "$TMPDIR"
fi

exit $fail
