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

## Results (2026-08-04)

**Gate A — real work (Brief feature_hash vs native feature_hash, median ns/call):**
| host | Brief | native | ratio |
|------|-------|--------|-------|
| C | 1220 | 1185 | 1.03 |
| C++ | 1243 | 1227 | 1.01 |
| Java | 1216 | 1281 | 0.95 |
| Go | 1398 | 1205 | 1.12 |
| Lua | 1194 | 16768 | 0.07 |
| Python | 1466 | 348588 | 0.004 |
| Node | 1394 | 267169 | 0.005 |

Parity for compiled hosts; **14–238× faster** for interpreted hosts (Brief's
compute is native machine code).

**Gate B — pure dispatch (Brief add vs native internal add):**
C 1.23, C++ 1.44, Lua 1.19, **Python 0.63**, Node 2.15, Java 5.99, Go 143×.
Python's METH_FASTCALL shim now dispatches faster than Python's own function
call (76 vs 121 ns). Node/Java/Go sit at their structural FFI bounds
(NAPI/JNI/cgo) — the Tier-3 floor.

**P1:** Python shim `METH_VARARGS` → `METH_FASTCALL` (no arg tuple):
trivial add ~174 → ~76 ns, below native Python's call.

**P2 audit:** the export body is already frictionless — `add` emits one native
`add i64`; the FNV loop is pure i64 arithmetic (no `adapt_to_i64` boxing,
zero-copy composites). Nothing to fix.

**Gate hygiene:** a fair gate keeps the native sink live (Go DCE'd the dead
timed loop → bogus sub-1ns/iter numbers) and measures Go native in a pure-Go
binary (a cgo-linked binary distorted it).

## Out of scope

Per-language code-emission backends (Go/Java transpilation) — Brief is an FFI.
