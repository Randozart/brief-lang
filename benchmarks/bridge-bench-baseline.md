# Benchmark: GLUE Bridge vs C FFI for Language Boundary Crossings

**Date:** 2026-07-22
**Status:** Plan

---

## Hypothesis

The GLUE bridge beats C FFI for cross-language calls because:
1. **Protocol BFS** finds the cheapest representation transform — identity costs zero
2. **LTO eliminates the boundary** when representations converge
3. **No intermediary allocations** — C FFI allocates at every crossing

## Benchmark Structure

Two implementations of the same operation, called from Python via ctypes:

### Operation

```
take a string, prepend a prefix, return the concatenated result
```

Implemented in three ways:

| Variant | Implementation | Calling path |
|---------|---------------|--------------|
| **C FFI** (baseline) | Hand-written `str_prepend(const char* s, const char* prefix) -> char*` | Python → ctypes → C → `malloc`+`sprintf` → Python |
| **Briev GLUE** | `briev_test_cstr_roundtrip(input_ptr: Int) -> Int` from `pp-types.bv` | Python → ctypes → `.so` → Briev → Py |
| **Rust FFI** (reference) | Same logic implemented as Rust `unsafe extern "C" fn` | Python → ctypes → Rust FFI → Py |

### Metrics

- **Latency per call** (μs) — median of 10,000 iterations
- **Allocations per call** — `malloc`/`free` count via `LD_PRELOAD` intercept
- **Binary size** of `.so` — KB

## Files

| File | Purpose |
|------|---------|
| `benchmarks/bridge/bench_glue_cross.py` | Main benchmark harness (Python) |
| `benchmarks/bridge/str_prepend.c` | C reference implementation |
| `benchmarks/bridge/str_prepend.rs` | Rust FFI reference (optional) |
| `benchmarks/bridge/bridge_tests.bv` | Briev bridge test file |
| `benchmarks/bridge/Makefile` | Build: C `.so`, Briev `.so`, optionally Rust `.so` |

## Integration with `build_and_bench.sh`

Add a `--bridge` flag that:
1. Builds all three `.so` variants:
   ```bash
   gcc -shared -fPIC -o libstr_prepend_c.so str_prepend.c -lm
   briev export bridge_tests.bv rust --out /tmp/br/
   # (produced .so already linked)
   ```
2. Runs `bench_glue_cross.py` which imports each `.so` via ctypes
3. Outputs latency + allocation comparison

## Success Criteria

- GLUE path has **fewer allocations per call** than C FFI
- GLUE path latency is **within 2× of C FFI** on first iteration (cold cache)
- GLUE path latency is **within 1.2× of C FFI** on subsequent iterations (warm)
- GLUE path `.so` is **smaller** than C + Rust combined
