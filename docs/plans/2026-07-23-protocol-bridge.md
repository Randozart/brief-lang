# Protocol Bridge + Brief-Native Wrappers

**Date:** 2026-07-23
**Status:** Plan → Implementation

## Goal

1. Move Python C extension wrappers from C to Brief (`export defn` + `frgn`)
2. Add protocol bridge `$defn` for languages without C FFI (WASM, sandbox)
3. Config-driven dispatch on `bridge_kind`

## Step 1: Wrappers in Brief, C only for struct literals

**Before:** Generator emits a complete C file with wrappers, methods table,
module def, and init function. Compiled via ShellCmd$(clang).

**After:** Generator emits `export defn` wrappers using `frgn` for Python
API calls. C file shrinks to just `PyMethodDef[]` + `PyModuleDef` +
`PyInit_bridge` — ~10 lines. The wrappers are native Brief functions.

```brief
frgn PyArg_ParseTuple(Ptr, Ptr, ...) -> Int from "c" fallback 0;
frgn PyLong_FromLongLong(Int) -> Ptr from "c" fallback 0;

export defn _pybridge_add(self: Ptr, args: Ptr) -> Ptr {
    let a: Int = 0;
    let b: Int = 0;
    PyArg_ParseTuple(args, "LL", &a, &b);
    let r: Int = add(a, b);
    term PyLong_FromLongLong(r);
};
```

## Step 2: Protocol Bridge

For languages without C FFI, the generator emits native source + a small
C shim. The two sides communicate through a negotiated transport:

| Transport | Brief side | Target side |
|-----------|-----------|-------------|
| Shared memory | `mmap` + busy-wait | `SharedArrayBuffer` + spin |
| Socket | `socket()` + `read()` | `fetch()` / `WebSocket` |
| Pipe | `pipe()` + `fork()` | stdin/stdout JSON |

The generator uses `bridge_kind` to choose the output:

- `c_wrapper` → emit Brief wrappers + minimal C struct file
- `protocol_bridge` → emit `bridge.h` (C shim) + target source (JS/TS/etc.)

## Files Touched

| File | Change |
|------|--------|
| `lib/glue/generator.bv` | Add protocol bridge `$defn`, dispatch on bridge_kind |
| `lib/glue/python.bv` | Add `frgn` declarations for Python C API |
| `lib/glue.toml` | Add `bridge_kind` entries for wasm, js |
