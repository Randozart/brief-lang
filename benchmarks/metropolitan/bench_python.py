#!/usr/bin/env python3
"""Python bridge benchmark — Tier 2: Python C extension (gen_python output).
Compares: ctypes (direct .so) vs C extension (if compiled).

Usage:
  python3 bench_python.py
"""

import ctypes
import time
import statistics
import os
import importlib.util

OUT_DIR = os.path.dirname(os.path.abspath(__file__)) + "/out"
SO_PATH = OUT_DIR + "/bench_add.so"
C_EXT_PATH = OUT_DIR + "/briev_bridge.so"
N_ITER = 50000

def load_ctypes():
    lib = ctypes.CDLL(SO_PATH)
    lib.add.argtypes = [ctypes.c_int64, ctypes.c_int64]
    lib.add.restype = ctypes.c_int64
    return lib

def load_cext():
    if not os.path.exists(C_EXT_PATH):
        return None
    spec = importlib.util.spec_from_file_location("briev_bridge", C_EXT_PATH)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod

def run_bench(name, fn, *args, iterations=N_ITER):
    result = fn(*args)
    times = []
    for _ in range(iterations):
        t0 = time.perf_counter_ns()
        fn(*args)
        t1 = time.perf_counter_ns()
        times.append(t1 - t0)
    median = statistics.median(times)
    print(f"  {name:30s}  median={median:8.0f}ns  result={result}")

def main():
    print("=" * 60)
    print("Metropolitan FFI Benchmark — Python")
    print("=" * 60)

    a, b = 3, 4

    print("\n[Pure Python]")
    run_bench("native add", lambda x, y: x + y, a, b, iterations=N_ITER * 2)

    print("\n[ctypes (direct .so)]")
    lib = load_ctypes()
    run_bench("ctypes add", lib.add, a, b)

    print("\n[C extension (gen_python)]")
    cext = load_cext()
    if cext:
        run_bench("C extension add", cext.add, a, b)
    else:
        print("  (bridge.so not found)")

    print("\n[Correctness]")
    n = (lambda x, y: x + y)(a, b)
    c = lib.add(a, b)
    print(f"  native:  {n}")
    print(f"  ctypes:  {c}")
    if cext:
        p = cext.add(a, b)
        print(f"  c_ext:   {p}")
        if n == c == p:
            print("  ✅ All match")
        else:
            print("  ❌ MISMATCH")
    elif n == c:
        print("  ✅ ctypes == native")

if __name__ == "__main__":
    main()
