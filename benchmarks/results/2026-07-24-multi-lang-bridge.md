# Multi-Language Bridge Benchmark Results
## 2026-07-24 — Post-SLP baseline (`be6583bc`)

All benchmarks call `export defn add(a: Int, b: Int) -> Int` returning `a + b`.
The Briev `.so` is compiled with `--shared` — `add` is `i64 @add(i64, i64)` with no state parameter.

| Language | Transport | Median | vs Native | vs Direct FFI |
|----------|-----------|--------|-----------|---------------|
| Python   | native    | 243ns  | 1×        | —             |
| Python   | ctypes    | 2,106ns | 8.7×      | 1×            |
| Python   | protocol (subprocess) | 1.23ms | 5,067× | 585×      |
| Node.js  | native    | 116ns  | 1×        | —             |
| Node.js  | koffi FFI | 281ns  | 2.4×      | 1×            |
| Node.js  | protocol (subprocess) | 3.74ms | 32,241× | 13,300× |
| Shell    | protocol (subprocess) | 4.76ms | —        | —             |

### Key findings

1. **Direct FFI is fast** — Python ctypes adds ~2µs, Node.js koffi adds only ~280ns.
   Node.js V8's FFI is 7.5× faster than Python's ctypes for simple scalar calls.

2. **Process spawning dominates subprocess protocol** — 1-5ms per call regardless
   of language. The protocol bridge C shim is fast (microseconds for dlopen+dlsym+call),
   but fork+exec dominates at ~1-5ms.

3. **WASM/SharedArrayBuffer would eliminate the spawn cost** — If the protocol bridge
   used shared memory (busy-wait on a flag) instead of a subprocess, the per-call
   latency would drop from milliseconds to microseconds, making protocol bridge
   viable for latency-sensitive workloads.

4. **The protocol bridge pattern works** — All three languages get correct results
   via the same C shim (dlopen + stdin/stdout text protocol). Adding a new language
   requires only a subprocess-capable runtime.

### System

- CPU: (x86_64)
- Kernel: Linux 6.17.0
- Python: 3.x
- Node.js: v24.16.0
- Briev compiler: commit `c3d8c980` (post-struct-array baseline)
