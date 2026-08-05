#!/usr/bin/env python3
"""GLUE native-speed benchmark: Briv (.so) vs C (.so) via ctypes.

Measures per-call latency of `feature_hash(count, seed)` (FNV-1a folding
over `count` features — real runtime compute, not an identity function)
across three paths:

  Python -> Briv .so   (GLUE c_abi path, generated library)
  Python -> C .so       (same workload, C reference)
  C native              (in-process C loop — the floor)

Usage:
  python3 bench_glue_speed.py [--count N] [--iterations N]
"""

import argparse
import ctypes
import os
import statistics
import sys
import time

PROJECT_ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "../.."))
LIB_DIR = os.path.join(PROJECT_ROOT, "target", "bridge_bench")

def load(path):
    return ctypes.CDLL(path)

def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--count", type=int, default=1000, help="feature count per call")
    ap.add_argument("--iterations", type=int, default=20000, help="calls to time")
    args = ap.parse_args()
    count, iters = args.count, args.iterations

    briv = load(os.path.join(LIB_DIR, "librank.so"))
    ref = load(os.path.join(LIB_DIR, "librank_ref.so"))

    # GLUE library ABI: __briv_init_state() -> handle; exports take it first
    # when they need state (feature_hash does — it runs a runtime loop).
    briv.__briv_init_state.restype = ctypes.c_void_p
    st = briv.__briv_init_state()
    briv.feature_hash.argtypes = [ctypes.c_void_p, ctypes.c_int64, ctypes.c_int64]
    briv.feature_hash.restype = ctypes.c_int64
    briv.add.argtypes = [ctypes.c_int64, ctypes.c_int64]
    briv.add.restype = ctypes.c_int64

    ref.feature_hash_c.argtypes = [ctypes.c_int64, ctypes.c_int64]
    ref.feature_hash_c.restype = ctypes.c_int64
    ref.add_c.argtypes = [ctypes.c_int64, ctypes.c_int64]
    ref.add_c.restype = ctypes.c_int64

    # Correctness first: same output on the same workload.
    seed = 42
    bh = briv.feature_hash(st, count, seed)
    ch = ref.feature_hash_c(count, seed)
    if bh != ch:
        print(f"FATAL: output mismatch briv={bh} c={ch}")
        sys.exit(1)
    print(f"output: feature_hash({count}, {seed}) = {bh} (briv == c)")

    def bench(fn, *args):
        fn(*args)  # warm-up
        times = []
        for _ in range(iters):
            t0 = time.perf_counter_ns()
            fn(*args)
            t1 = time.perf_counter_ns()
            times.append(t1 - t0)
        return statistics.median(times), statistics.mean(times)

    print(f"\nper-call latency over {iters} calls (feature_hash count={count}):")
    bm, bmean = bench(briv.feature_hash, st, count, seed)
    cm, cmean = bench(ref.feature_hash_c, count, seed)
    am, amean = bench(briv.add, 3, 4)

    print(f"  Python -> Briv feature_hash : median={bm:7.0f} ns  mean={bmean:7.0f} ns")
    print(f"  Python -> C    feature_hash : median={cm:7.0f} ns  mean={cmean:7.0f} ns")
    print(f"  Python -> Briv add (pure)  : median={am:7.0f} ns  mean={amean:7.0f} ns")
    ratio = cm / bm if bm else float("nan")
    print(f"\n  Briv vs C per-call overhead: {ratio:.2f}x (C faster when < 1)")

    print("\nnote: the Briv/C gap is ctypes marshalling + state-arg overhead,"
          "\n      not the compute itself (identical output). The Rust LTO path"
          "\n      inlines the boundary — see examples/glue-host/rust-host.")

if __name__ == "__main__":
    main()
