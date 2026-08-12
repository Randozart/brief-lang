#!/usr/bin/env python3
"""Multi-language bridge benchmark: Briev export defn called from Python.

Compares:
  1. Direct ctypes FFI (fastest FFI path)
  2. Protocol bridge subprocess (stdin/stdout text protocol)
  3. Pure Python (reference)

Usage:
  python3 benchmarks/multi_lang/bench_multi_lang.py
"""

import ctypes
import os
import subprocess
import sys
import time
import statistics

PROJECT_ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "../.."))
BUILD_DIR = os.path.join(PROJECT_ROOT, "target", "multi_lang")
SO_PATH = os.path.join(BUILD_DIR, "export_add.so")
SHIM_PATH = os.path.join(BUILD_DIR, "proto_shim")

# ── 1. Direct ctypes FFI ────────────────────────────────────────────────

def load_cdll():
    lib = ctypes.CDLL(SO_PATH)
    lib.add.argtypes = [ctypes.c_int64, ctypes.c_int64]
    lib.add.restype = ctypes.c_int64
    lib.mul.argtypes = [ctypes.c_int64, ctypes.c_int64]
    lib.mul.restype = ctypes.c_int64
    return lib

def bench_ctypes(lib, a, b):
    return lib.add(a, b)

# ── 2. Protocol bridge (subprocess) ──────────────────────────────────────

def bench_protocol(a, b):
    p = subprocess.Popen(
        [SHIM_PATH],
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    req = f"add {a} {b}\n"
    out, _ = p.communicate(req.encode())
    return int(out.decode().strip())

# ── 3. Pure Python reference ────────────────────────────────────────────

def bench_native(a, b):
    return a + b

# ── Benchmark runner ────────────────────────────────────────────────────

def run_bench(name, fn, *args, iterations=10000):
    result = fn(*args)

    times = []
    for _ in range(iterations):
        t0 = time.perf_counter_ns()
        fn(*args)
        t1 = time.perf_counter_ns()
        times.append(t1 - t0)

    median_ns = statistics.median(times)
    mean_ns = statistics.mean(times)
    min_ns = min(times)
    max_ns = max(times)

    print(f"  {name:30s}  median={median_ns:8.0f}ns  mean={mean_ns:8.0f}ns  "
          f"min={min_ns:6.0f}ns  max={max_ns:8.0f}ns  result={result}")

def main():
    print("=" * 65)
    print("Multi-Language Bridge Benchmark — Briev export defn from Python")
    print("=" * 65)

    a, b = 3, 4

    # Pure Python reference
    print("\n[Pure Python]")
    run_bench("native add", bench_native, a, b, iterations=50000)

    # Direct ctypes FFI
    print("\n[Python ctypes]")
    lib = load_cdll()
    run_bench("ctypes add", bench_ctypes, lib, a, b)

    # Protocol bridge subprocess
    if os.path.exists(SHIM_PATH):
        print("\n[Protocol Bridge (subprocess)]")
        run_bench("proto_shim add", bench_protocol, a, b, iterations=100)
    else:
        print(f"\n  (no proto shim at {SHIM_PATH}, skipping)")

    # Correctness
    c_result = bench_ctypes(lib, a, b)
    n_result = bench_native(a, b)
    print(f"\n[Correctness]")
    print(f"  ctypes:  {c_result}")
    print(f"  native:  {n_result}")
    if os.path.exists(SHIM_PATH):
        p_result = bench_protocol(a, b)
        print(f"  proto:   {p_result}")
        if c_result == n_result == p_result:
            print("  ✅ All match")
        else:
            print("  ❌ MISMATCH")
    else:
        if c_result == n_result:
            print("  ✅ ctypes == native")

if __name__ == "__main__":
    main()
