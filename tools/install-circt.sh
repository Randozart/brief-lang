#!/usr/bin/env bash
# 2026-08-23 (Plan 0.6, backend-scaffolding-foundation): install CIRCT for
# toolchain-validated hardware-backend tests (plan
# 2026-08-23-circt-toolchain-validation.md).
#
# Installs into tools/circt/ (bin/, lib/) — NOT the system prefix. Tests
# discover the binaries via tools/circt_probe.sh, never a hard-coded path.
#
# CIRCT has no stable release tarballs; builds come from the llvm/circt
# monorepo. This script pins a commit and builds Release with the MLIR
# dialects the backend emits (HW/Comb/Seq/SV). First build takes ~30-60 min.
#
# Pin: circt-monorepo commit (update deliberately; record why here).
# 2026-08-23: pinned to upstream main HEAD at script-creation time
# (231e82511, "[ESI] Use ChannelMergeOneValid for ChannelMMIO responses").
# The original placeholder hash was fabricated — that was the
# "reference is not a tree" failure.
CIRCT_PIN="231e82511220444b0fc1be37e8deabea19925a5a"
JOBS="$(nproc)"

set -euo pipefail
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
DEST="$ROOT/tools/circt"
SRC="$ROOT/tools/circt-src"

if [ -x "$DEST/bin/circt-opt" ]; then
    echo "circt-opt already present at $DEST/bin/circt-opt"
    exit 0
fi

command -v cmake >/dev/null || { echo "cmake required"; exit 1; }
command -v ninja >/dev/null || { echo "ninja required (apt install ninja-build)"; exit 1; }

mkdir -p "$ROOT/tools"
if [ ! -d "$SRC/.git" ]; then
    git clone https://github.com/llvm/circt.git "$SRC"
fi
git -C "$SRC" fetch origin main --tags
git -C "$SRC" checkout "$CIRCT_PIN"

# CIRCT builds against a matching LLVM at the pinned monorepo commit.
git -C "$SRC" submodule update --init llvm

cmake -G Ninja -S "$SRC" -B "$SRC/build" \
    -DCMAKE_BUILD_TYPE=Release \
    -DLLVM_ENABLE_ASSERTIONS=ON \
    -DCIRCT_SV_FRONTEND=ON \
    -DLLVM_TARGETS_TO_BUILD=host \
    -DCMAKE_INSTALL_PREFIX="$DEST"
cmake --build "$SRC/build" --target install -j"$JOBS"

"$DEST/bin/circt-opt" --version
echo "OK: CIRCT installed. Tests pick it up via tools/circt_probe.sh."
