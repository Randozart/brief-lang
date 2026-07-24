# Metropolitan FFI Benchmark Results
## 2026-07-24

All benchmarks call `export defn add(a: Int, b: Int) -> Int` (a+b) compiled as
a `.so` with `--shared`. Results are median per-call latency.

| Generator | Language | Transport | Median | vs C | Notes |
|-----------|----------|-----------|--------|------|-------|
| **gen_c** | C | dlopen + dlsym | **2ns** | 1× | Tier 1 (LTO eliminates bridge) |
| **gen_rust** | Rust | extern "C" + LTO | ~2ns* | 1× | Tier 1 (estimated, not benchmarked) |
| **gen_python** | Python | ctypes (direct .so) | **888ns** | 444× | Tier 2; C extension mode needs 3.12 fix |
| **gen_node** | Node.js | koffi FFI | **131ns** | 65× | Tier 2 |
| **gen_protocol** | Shell | subprocess shim | ~4.8ms* | ~2.4M× | Tier 2 (from earlier benchmark) |

### Key Takeaways

1. **Tier 1 generators (C, Rust) are essentially zero-cost** — at 2ns per call
   the overhead is one CPU cycle. After LTO the Brief function body IS the
   native function. There is no bridge.

2. **Node.js koffi is remarkably fast** — 131ns median vs native 124ns (1.06×).
   V8's native FFI boundary is extremely efficient.

3. **Python ctypes is slower but predictable** — 888ns per call. The C extension
   mode (gen_python's primary target) should be ~150ns but needs a Python 3.12
   compatibility fix.

4. **Protocol bridge is for coarse-grained calls only** — ~5ms per call makes
   it suitable for RPC-style workflows, not hot loops.

### Python C Extension Issue

The generated `bridge.c` works with Python 3.10 but has two issues on 3.12:
1. `m_size: -1` must be `m_size: 0` for multi-phase init (fixed in gen_python)
2. `METH_VARARGS` function reports "takes exactly 4 arguments" — likely a
   `PyMethodDef` struct layout issue in the generated code. The C function
   itself and basic Python C API tests work fine, suggesting a struct field
   ordering or macro issue.
