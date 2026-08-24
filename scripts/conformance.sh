#!/usr/bin/env bash
# Conformance sweep gate — run before every push.
# SPEC §23.4: every active source must parse and typecheck under its profile.
set -euo pipefail
cd "$(dirname "$0")/.."
echo "=== conformance sweep ==="
cargo test --lib conformance_sweep -- --exact
echo "=== full suite ==="
cargo test --lib
cargo test --bin brievc
echo "=== all green ==="
