#!/usr/bin/env bash
set -euo pipefail

echo "=== cargo check ==="
cargo check

echo "=== cargo test --lib ==="
cargo test --lib

echo "=== cargo kani (fast group) ==="
cargo kani --lib

echo ""
echo "=== Full Kani suite (requires --features kani_full) ==="
echo "Run separately: cargo kani --lib --features kani_full"
echo ""
echo "All verifications passed."
