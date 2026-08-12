#!/usr/bin/env python3
"""Benchmark: GLUE Bridge vs C FFI for cross-language string calls.

Compares three paths:
  C FFI:  Python → ctypes → C .so → Python
  Briev:  Python → ctypes → Briev .so → Python
  Native: Pure Python implementation (reference)

Usage:
  python3 bench_glue_cross.py          # runs all variants
  python3 bench_glue_cross.py c        # C only
  python3 bench_glue_cross.py briev    # Briev only

Requires:
  libstr_prepend_c.so  — compiled from str_prepend.c
  libpp_types.so       — compiled from pp-types.bv (via briev build)
"""

import ctypes
import os
import sys
import time
import statistics

PROJECT_ROOT = os.path.abspath(os.path.join(os.path.dirname(__file__), "../.."))
BRIDGE_DIR = os.path.join(PROJECT_ROOT, "target", "bridge_bench")

# ── Load .so files ─────────────────────────────────────────────────────

def load_c_lib():
    """Load the C reference .so."""
    path = os.path.join(BRIDGE_DIR, "libstr_prepend_c.so")
    lib = ctypes.CDLL(path)
    lib.c_str_echo.argtypes = [ctypes.c_int64]
    lib.c_str_echo.restype = ctypes.c_int64
    return lib

def load_briev_lib():
    """Load the Briev bridge .so."""
    path = os.path.join(BRIDGE_DIR, "libpp_types.so")
    lib = ctypes.CDLL(path)
    # 2026-08-09 (Bug 5): briev_test_cstr_roundtrip needs NO state — its export
    # ABI is fn(i64) -> i64. A 2-arg declaration here silently passed state(0)
    # as input_ptr and returned '<null>'.
    lib.briev_test_cstr_roundtrip.argtypes = [ctypes.c_int64]
    lib.briev_test_cstr_roundtrip.restype = ctypes.c_int64
    return lib

# ── String helpers ─────────────────────────────────────────────────────

# Keep references to ALL allocated C strings to prevent GC from freeing them
_allocated_c_strings: list = []

def c_str(s: str) -> int:
    """Convert a Python string to a C string pointer as int64."""
    b = s.encode()
    _allocated_c_strings.append(b)  # prevent GC
    return ctypes.cast(ctypes.c_char_p(b), ctypes.c_void_p).value

def from_c_str(ptr: int) -> str:
    """Convert a C string pointer back to a Python string WITHOUT freeing."""
    if ptr == 0:
        return "<null>"
    # Use memmove to copy the string out — ctypes.c_char_p auto-frees
    buf = ctypes.create_string_buffer(256)
    ctypes.memmove(buf, ctypes.c_void_p(ptr), 256)
    return buf.value.decode()

# ── Benchmark implementations ──────────────────────────────────────────

def bench_c(lib, _state, s: str) -> str:
    """Call via C FFI — c_str_echo(s)."""
    s_ptr = c_str(s)
    result_ptr = lib.c_str_echo(s_ptr)
    return from_c_str(result_ptr)

def bench_briev(lib, state, s: str) -> str:
    """Call via Briev GLUE bridge — briev_test_cstr_roundtrip(input_ptr)."""
    s_ptr = c_str(s)
    # 2026-08-09 (Bug 5): briev_test_cstr_roundtrip does NOT need the runtime
    # state (its body only calls the cstr_to_briev/str_to_c FFI), so the export
    # ABI is `fn(i64) -> i64` — a 2-arg call would pass state(0) as input_ptr
    # and return `<null>`. Match the Rust pp_roundtrip test: single arg.
    result_ptr = lib.briev_test_cstr_roundtrip(s_ptr)
    return from_c_str(result_ptr)

def bench_native(s: str) -> str:
    """Pure Python — reference."""
    return s

# ── Main benchmark loop ────────────────────────────────────────────────

def run_bench(name: str, fn, *args, iterations=10000):
    """Run a benchmark and print results."""
    # Warm-up
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

    print(f"  {name:20s}  median={median_ns:8.0f}ns  mean={mean_ns:8.0f}ns  "
          f"min={min_ns:6.0f}ns  max={max_ns:8.0f}ns  result={result!r}")

def main():
    print("=" * 60)
    print("GLUE Bridge vs C FFI — String Round-Trip Benchmark")
    print("=" * 60)

    test_string = "42"  # simple, matches pp_type_bits("42")

    # Native reference
    print("\n[Pure Python]")
    run_bench("native", bench_native, test_string, iterations=50000)

    # C FFI
    print("\n[C FFI]")
    lib_c = load_c_lib()
    c_state = ctypes.c_int64(0)
    run_bench("c_str_echo", bench_c, lib_c, c_state, test_string)

    # Briev GLUE
    print("\n[Briev GLUE]")
    lib_briev = load_briev_lib()
    # Allocate fresh state buffer per call
    run_bench("briev_cstr_rt", bench_briev, lib_briev, ctypes.c_int64(0), test_string)

    # Correctness check
    print("\n[Correctness]")
    c_result = bench_c(lib_c, c_state, test_string)
    b_result = bench_briev(lib_briev, ctypes.c_int64(0), test_string)
    n_result = bench_native(test_string)
    print(f"  C:      {c_result!r}")
    print(f"  Briev:  {b_result!r}")
    print(f"  Native: {n_result!r}")
    if c_result == n_result == b_result:
        print("  ✅ All match")
        return True
    print("  ❌ MISMATCH")
    # 2026-08-09 (Bug 5): a wrong bridge result must exit non-zero so the
    # harness records FAIL (previously the shell re-ran check_correctness
    # which SKIPped on a missing compiled binary, masking the mismatch).
    return False

if __name__ == "__main__":
    # 2026-08-09 (Bug 5): a wrong bridge result exits 1 so the shell harness
    # records FAIL instead of masking the mismatch behind a SKIP.
    ok = main()
    import sys
    sys.exit(0 if ok else 1)
