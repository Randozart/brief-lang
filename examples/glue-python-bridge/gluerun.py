#!/usr/bin/env python3
"""
GLUE Python Bridge — call Brief exports from Python via ctypes.

Workflow:
  1. brief build --no-stdlib --library bridge.bv --out .  → bridge.ll
  2. llc -filetype=obj bridge.ll -o bridge.o              → bridge.o
  3. cc -shared bridge.o -o libbridge.so                  → libbridge.so
  4. python3 gluerun.py                                   → runs this script

This script automates steps 2-4 and demonstrates the calling convention.
"""

import ctypes
import subprocess
import sys
import os
from pathlib import Path

SCRIPT_DIR = Path(__file__).parent


def build_bridge():
    """Compile bridge.ll → bridge.o → libbridge.so."""
    bridge_ll = SCRIPT_DIR / "bridge.ll"
    if not bridge_ll.exists():
        print("ERROR: bridge.ll not found. Run first:", file=sys.stderr)
        print(f"  brief build --no-stdlib --library bridge.bv --out {SCRIPT_DIR}", file=sys.stderr)
        sys.exit(1)

    bridge_o = SCRIPT_DIR / "bridge.o"
    lib_path = SCRIPT_DIR / "libbridge.so"

    # llc: .ll → .o
    subprocess.run(
        ["llc", "-filetype=obj", "-O2", "--relocation-model=pic",
         "-o", str(bridge_o), str(bridge_ll)],
        check=True,
    )
    print(f"  Compiled: {bridge_o.name}")

    # cc: .o → .so (position-independent for shared library)
    subprocess.run(
        ["cc", "-shared", "-fPIC", "-o", str(lib_path), str(bridge_o)],
        check=True,
    )
    print(f"  Shared lib: {lib_path.name}")

    return lib_path


def run_demo(lib_path):
    """Load the shared library and call Brief exports."""
    lib = ctypes.CDLL(str(lib_path))

    # --- Configure function signatures ---

    # __brief_init_state() -> c_void_p
    lib.__brief_init_state.argtypes = []
    lib.__brief_init_state.restype = ctypes.c_void_p

    # add(State, i64, i64) -> i64
    lib.add.argtypes = [ctypes.c_void_p, ctypes.c_int64, ctypes.c_int64]
    lib.add.restype = ctypes.c_int64

    # multiply(State, i64, i64) -> i64
    lib.multiply.argtypes = [ctypes.c_void_p, ctypes.c_int64, ctypes.c_int64]
    lib.multiply.restype = ctypes.c_int64

    # --- Call it ---
    print("═══ GLUE Python Bridge Demo ═══")

    state = lib.__brief_init_state()
    print(f"  Brief runtime initialized (state={state:#x})")

    result = lib.add(state, 40, 2)
    print(f"  add(40, 2) = {result}")
    assert result == 42, f"add failed: {result} != 42"

    result = lib.multiply(state, 6, 7)
    print(f"  multiply(6, 7) = {result}")
    assert result == 42, f"multiply failed: {result} != 42"

    print("═══ All bridge calls passed ═══")


if __name__ == "__main__":
    lib_path = build_bridge()
    run_demo(lib_path)
