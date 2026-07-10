# GLUE Python Bridge Example

Call Brief-exported functions from Python via the GLUE protocol.

## Quick Start

```bash
cd examples/glue-python-bridge
brief build --no-stdlib --library bridge.bv --out .
python3 gluerun.py
```

Expected output:
```
═══ GLUE Python Bridge Demo ═══
  Brief runtime initialized (state=0x...)
  add(40, 2) = 42
  multiply(6, 7) = 42
═══ All bridge calls passed ═══
```

## How It Works

1. `brief build --no-stdlib --library bridge.bv --out .` compiles to `bridge.ll`
   — a reusable LLVM IR module with `__brief_init_state()` + exports, no `main()`
2. `gluerun.py` automates:
   - `llc bridge.ll -filetype=obj -O2 -o bridge.o`
   - `cc -shared bridge.o -o libbridge.so`
   - `ctypes.CDLL("./libbridge.so")`
3. Python calls `__brief_init_state()` once, then calls exports with `c_int64` args

## C ABI Convention

```python
lib.__brief_init_state.argtypes = []
lib.__brief_init_state.restype = ctypes.c_void_p

lib.add.argtypes = [ctypes.c_void_p, ctypes.c_int64, ctypes.c_int64]
lib.add.restype = ctypes.c_int64
```

## Prerequisites

- LLVM toolchain (`llc`) + C compiler — `apt install llvm gcc`
- Python 3 — `ctypes` is built-in
- Brief compiler from this repo
