# Metropolitan FFI Benchmark Results
## 2026-07-24

All benchmarks call `export defn add(a: Int, b: Int) -> Int` (a+b). Results are median per-call latency.

| Generator | Language | Transport | Median | vs C | Tier |
|-----------|----------|-----------|--------|------|------|
| `gen_c` | C | dlopen + dlsym | **1ns** | 1× | Tier 1 |
| `gen_rust` | Rust | extern "C" + LTO | ~1ns* | 1× | Tier 1 |
| `gen_wasm` | JS | WebAssembly | **120ns** | 120× | Tier 2 |
| `gen_python` | Python | C extension | **126ns** | 126× | Tier 2 |
| `gen_node` | JS | koffi FFI | **141ns** | 141× | Tier 2 |
| `gen_protocol` | Shell | Subprocess | **2.7ms** | 2.7M× | Tier 2 |

### Observations

**WASM is the fastest Tier 2** — 120ns, beating koffi (141ns) and Python C extension (126ns). WASM avoids native FFI boundary crossing; it's already in the JS process.

**All Tier 2 approaches are within 2× of each other** — 120-141ns range. The protocol bridge (2.7ms) is the outlier because process spawning dominates.

**gen_c (Tier 1) is ~1ns** — after LTO the function IS the native function. No bridge code at all.

### System

- CPU: x86_64
- Python 3.12.3, Node.js v24.16.0
- Brief compiler commit `732ecc12`
