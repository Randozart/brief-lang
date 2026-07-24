# Remaining Pipeline: Struct Literals, Pure-Brief GLUE, Protocol Bridge, Doc Command

**Date:** 2026-07-24
**Status:** Plan → Implementation

## Overview

Five remaining features to complete the GLUE bridge pipeline and make
it fully Brief-native. Implemented in priority order.

---

## 1. Struct Literals (`Expr::StructLiteral`)

Parser + codegen change to construct static struct values inline.

### AST

```rust
Expr::StructLiteral {
    type_name: String,
    fields: Vec<(String, Expr)>,
}
```

### Parser

Parse `StructType { name: expr; field2: expr; }` as an expression.

Key detail: `&export_fn` in a struct literal context evaluates to the
function's address as `ptr` — the codegen already handles `Expr::AddrOf`.

### Codegen

Emit LLVM struct constant with field values at the offsets defined by
the `struct` declaration. Field types are checked against the struct
definition at parse time.

### Evaluation (macro DSL)

Struct literals evaluated in `$(Stage)` blocks produce a `NavValue::Struct`
variant containing field name → NavValue pairs. Used by the GLUE generator
for constructing method tables inline.

---

## 2. Pure-Brief GLUE Pipeline

The generator emits the entire Python C extension as Brief source using
`struct` declarations + struct literals + `export defn`. No C file, no
`ShellCmd$("clang")`, no `InsertObject$`.

The `$(Normalized)` stage block generates `PyMethodDef[]`, `PyModuleDef`,
and `PyInit_bridge` as Brief source that the compiler compiles natively.

### Blockers

**1. `frgn Ptr` → `i64` mismatch.** The LLVM backend lowers `frgn`
parameters and return types of `Ptr` to `i64` (Brief's internal pointer
representation). For external C functions like `PyModule_Create2`,
pointers must be LLVM `ptr`, not `i64`. Fix needed in `emit_frgn_call`:
when the frgn signature has `Ptr`, emit LLVM `ptr` type, not `i64`.

**2. `&list` in struct literals.** `&methods` where `methods` is a
`let`-bound list produces the address of a heap-allocated sequence, not a
C-compatible array pointer. For `PyMethodDef[]`, the struct literal needs a
pointer to a static constant array, not a heap list. Fix: struct literal
fields of type `Ptr` should accept both `&var` and `&"string"`.

**3. Struct field offsets.** `lookup_field_offset` uses `i * 8` (simplified).
Real struct layouts need C ABI packing rules (platform-specific alignment).
Fix: compute field offsets from the `struct` declaration's field types using
platform ABI rules (or pack = 1).

---

## 3. Protocol Bridge (stdin/stdout)

For languages without C FFI (WASM, sandboxed scripting, browser).

The generator emits:
- A C shim with `main()` that reads text protocol from stdin and
  dispatches to Brief exports via dlopen/dlsym
- The protocol is newline-delimited: `"add 3 4\n"` → `"7\n"`
- Any language that can spawn a child process and read/write pipes
  can use Brief exports — no FFI required at all

---

## 4. `brief doc` Command

CLI subcommand that reads doc comments (`///` and `//!`) from `.bv` files
and renders them as HTML.

- Scans all items in the program
- Groups doc comments by item type (defn, txn, struct, frgn, etc.)
- Renders a simple index page + per-item detail pages
- Markdown subset: paragraphs, code blocks, lists

---

## 5. DWARF-Based Struct Discovery

Read `.debug_info` sections from compiled `.so` files to discover struct
layouts without needing header files or probe programs.

- New intrinsic: `DwarfReadLayout$("libgeo.so", "Point")`
- Minimal DWARF parser (~200 lines, DWARF 4/5 struct DIEs)
- Returns same format as `InjectTypeLayout$` consumes
- Populates the protocol graph with foreign type layouts

---

## Implementation Order

1. Struct literals (parser + codegen) — unblocks pure-Brief GLUE
2. Pure-Brief GLUE generator — eliminates last C dependency
3. Protocol bridge — target languages without C FFI
4. `brief doc` — documentation output
5. DWARF discovery — eliminates probe programs

---

## 6. Multi-Language Bridge Benchmark

Build and benchmark bridges for 5+ languages to prove Brief's GLUE system
can match or beat native interop for any calling convention.

| Language | Bridge type | Expected latency | Status |
|----------|-------------|------------------|--------|
| Python (native C ext) | C wrapper | ~150ns | ✅ Working |
| Python (ctypes) | cdylib | ~800ns | ✅ Working |
| Python (subprocess) | Protocol bridge | ~200µs | 🔧 Need stdin/stdout shim |
| Node.js | Subprocess protocol | ~200µs | 🔧 Need JS wrapper |
| WASM | Shared memory protocol | ~50ns (wasm call) | ❌ Need llc -march=wasm64 |
| Shell/awk | Subprocess protocol | ~200µs | ✅ Working (echo \| bridge) |
| gRPC | HTTP/2 server | ~500µs | ❌ Need C shim + protobuf |

### Benchmark protocol

- Same operation (`add(3,4)`) for all bridges
- 200K iterations, median latency
- Compare against native C .so baseline
- Measure: call latency, throughput, binary size, init time

### Goal

Show the full spectrum: from 150ns (native C extension) to 200µs (protocol
bridge) with clear trade-offs. Brief adapts to the calling convention of
whatever language it needs to talk to — no single approach dominates.
