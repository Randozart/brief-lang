# The zero-friction FFI gate — Brief at every host's native call

**Date:** 2026-08-04
**Status:** Active plan
**Branch:** `glue-host-callable` (worktree `../brief-compiler-glue-host`)
**Related:** `docs/plans/2026-08-04-ship-common-language-environments.md`,
`docs/guides/ffi-and-export.md`

---

## Goal

Brief sits natively next to each host language: a host calls a Brief export as
if calling a super-efficient version of itself. The boundary is **frictionless**
— dispatch at the host's FFI floor, compute at native width with zero boxing
and zero-copy composites. **FFI, not a backend**: no per-language code-emission
compilers; the cgo/JNI floors are the accepted FFI bound for Go/Java, and the
gate proves we sit *on* the floor, never above it.

## Tiers

- **Tier 1 (FFI is the native call):** C, C++, Rust, Lua — verify at floor.
- **Tier 2 (FFI is the native-module call):** Python, Node — thin dispatch to
  the floor (Python `METH_FASTCALL` is the concrete work).
- **Tier 3 (FFI crossing above the internal call):** Go (cgo), Java (JNI) —
  the FFI bound; gate proves on-floor.

## The gate (committed, toolchain-guarded)

- **Gate A (real work):** `Brief.feature_hash` from each host vs that host's
  `native_feature_hash` (same FNV-1a, 64-bit wrap — Python masks to 64 bits).
  Target ratio **≤ 1.0** always; **< 0.1** for Python/Lua (Brief compute is
  native machine code).
- **Gate B (pure dispatch):** `Brief.add` vs the host's FFI floor (minimal
  C-extension / NAPI / cgo / JNI / Lua-C / direct C++) vs the host's
  pure-internal `add`. Reports which tier each host is on and any shim sitting
  above its floor.

## Phases

### P0 — baselines
`benchmarks/bridge/`: per-host `native_feature_hash` + `native_add` +
FFI-floor baselines; N scaled for interpreted hosts; interleaved medians.

### P1 — thin the dispatch
Python `METH_VARARGS`+`PyArg_ParseTuple` → `METH_FASTCALL` (direct
`PyObject* const*` args, no tuple) — a config/template change to the python
target's native.* templates. Verify Node/Go/Java/Lua/C++ minimal paths. Re-run
Gate B; tighten any shim above its floor.

### P2 — frictionless compute
Audit the export codegen: params/returns carry the C ABI types; the body must
have zero `adapt_to_i64` boxing for native-width types and zero-copy composites
for String/Data; shim→export→body is identity for Int/Float and
pointer-passing for composites. Fix any residual friction. Re-run Gate A.

### P4 — commit the gate
Wire Gate A + B into the test suite (toolchain-guarded) as the permanent
zero-overhead regression gate. Record both tables in this plan + the roster doc.

## Out of scope

Per-language code-emission backends (Go/Java transpilation) — Brief is an FFI.
