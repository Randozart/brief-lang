#!/usr/bin/env bash
# 2026-08-23 (Plan 0.6): locate CIRCT binaries for toolchain-validated tests.
# Prints the directory holding circt-opt and exits 0, or exits 1 when absent.
# Search order: tools/circt/ (install-circt.sh target), then PATH.
#
# Rust test-side probe mirrors this logic (see circt/mod.rs tests):
#   tools/circt/bin/circt-opt exists, else `circt-opt --version` succeeds.
set -u
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
if [ -x "$ROOT/tools/circt/bin/circt-opt" ]; then
    echo "$ROOT/tools/circt/bin"
    exit 0
fi
if command -v circt-opt >/dev/null 2>&1; then
    dirname "$(command -v circt-opt)"
    exit 0
fi
exit 1
