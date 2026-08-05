# Ship common language environments — Briv as the central language

**Date:** 2026-08-04
**Status:** Active plan
**Branch:** `glue-host-callable` (worktree `../briv-compiler-glue-host`)
**Related:** `docs/plans/2026-08-03-glue-folders-node-bridge.md`,
`docs/guides/ffi-and-export.md`

---

## Goal

Ship `briv bindings` / `briv extension` targets for a roster of common
language environments — **C++, Go, Java (JNI), C#, Lua** — and time each against
native performance so the wins/overheads are measured, not assumed. This proves
the config-driven claim (a language = a `lib/glue/<lang>/` folder, zero compiler
changes) and makes Briv the true central language.

## Roster + flavor

| Language | Flavor | Mechanism | Verify |
|----------|--------|-----------|--------|
| C++ | bindings | the C header with `extern "C"` guards | g++ (present) |
| Go | bindings | cgo: `import "C"` externs + typed wrappers; String → `C.GoString` (zero-copy read) | portable Go (~/briv-tools) |
| Java | native ext | JNI shim (`JNIEXPORT` wrappers; String → `NewStringUTF`) + Java class with `native` methods | portable Temurin JDK (~/briv-tools) |
| C# | bindings | P/Invoke `[DllImport]` + `Marshal.PtrToStringUTF8` for the composite | guarded (no .NET) |
| Lua | native ext | C module `luaopen_bridge`; String → `lua_pushstring` | Lua built from source (~/briv-tools) |

Every target is config + templates through the existing generic renderer —
**no Rust changes** (any exception is a finding about the generic system).

## Verification strategy

- **Render tests (run everywhere):** assert `briv bindings|extension` produces
  the expected shape per language (catches template/renderer bugs without the
  toolchain).
- **Round-trips (toolchain-guarded):** C++ verified now; Go + Java via portable
  downloads; Lua built from source; C# guarded.

## Timing (the performance gate)

For each verified language, `feature_hash(count=1000)` is timed at N iterations
vs the native C reference (`rank_ref.c`) and the native Briv `.a` call, on the
same machine, interleaved. Table recorded here + in the plan status. Any
language >2× over native C is flagged for optimization (the boundary is a
single C-ABI call, so the gap should be the host-language call overhead).

## Deliverables per language

1. `lib/glue/<lang>/glue.dbvl` — target entry, protocols/ABI, templates
   (bindings and/or native.*), toolchain recipe.
2. `lib/glue/<lang>/types.bv` — boundary type declarations (thin where the C
   ABI is the substrate).
3. `tests/c_driver_<lang>.rs` — render assertion + guarded round-trip.
4. Timing table.

## Timing results (feature_hash count=1000, ns/call, interleaved median)

| Host | ns/call | vs native C |
|------|---------|-------------|
| C (reference) | 1223 | — |
| C++ | 1229 | +0.4% |
| Java (JNI, JIT) | 1160 | −5% |
| Lua (C module) | 1200 | −2% |
| Node (.node addon) | 1260 | +3% |
| Go (cgo) | 1302 | +6.4% |
| Python (native ext) | 1430 | +16.9% |

All within ~17% of native C; the boundary call itself is ~2–6ns (C), so the
delta is the host-language call overhead per invocation. The compute
(~1180–1200ns) dominates. Java's JIT even lands below the C reference this
round (machine variance). No host language needs optimization — the shims are
already at native speed.

## Sequencing

1. Plan + downloads (Go/JDK/Lua) in parallel.
2. C++ (fastest, verified).
3. Go (cgo), Java (JNI), Lua (native ext).
4. C# (guarded).
5. Timing all vs native C.
6. Docs (`ffi-and-export.md` per-language sections), BUGS.md, full suite.

## Baseline (rule 11)

At the current commit: Rust→Briv 1127ns, C→Briv 1092ns, Python native ext
~1300ns / pure call ~200ns, ctypes 1058ns pure. Native C `feature_hash`
~1080ns (the FNV-1a work itself).
