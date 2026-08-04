# Ship common language environments — Brief as the central language

**Date:** 2026-08-04
**Status:** Active plan
**Branch:** `glue-host-callable` (worktree `../brief-compiler-glue-host`)
**Related:** `docs/plans/2026-08-03-glue-folders-node-bridge.md`,
`docs/guides/ffi-and-export.md`

---

## Goal

Ship `brief bindings` / `brief extension` targets for a roster of common
language environments — **C++, Go, Java (JNI), C#, Lua** — and time each against
native performance so the wins/overheads are measured, not assumed. This proves
the config-driven claim (a language = a `lib/glue/<lang>/` folder, zero compiler
changes) and makes Brief the true central language.

## Roster + flavor

| Language | Flavor | Mechanism | Verify |
|----------|--------|-----------|--------|
| C++ | bindings | the C header with `extern "C"` guards | g++ (present) |
| Go | bindings | cgo: `import "C"` externs + typed wrappers; String → `C.GoString` (zero-copy read) | portable Go (~/brief-tools) |
| Java | native ext | JNI shim (`JNIEXPORT` wrappers; String → `NewStringUTF`) + Java class with `native` methods | portable Temurin JDK (~/brief-tools) |
| C# | bindings | P/Invoke `[DllImport]` + `Marshal.PtrToStringUTF8` for the composite | guarded (no .NET) |
| Lua | native ext | C module `luaopen_bridge`; String → `lua_pushstring` | Lua built from source (~/brief-tools) |

Every target is config + templates through the existing generic renderer —
**no Rust changes** (any exception is a finding about the generic system).

## Verification strategy

- **Render tests (run everywhere):** assert `brief bindings|extension` produces
  the expected shape per language (catches template/renderer bugs without the
  toolchain).
- **Round-trips (toolchain-guarded):** C++ verified now; Go + Java via portable
  downloads; Lua built from source; C# guarded.

## Timing (the performance gate)

For each verified language, `feature_hash(count=1000)` is timed at N iterations
vs the native C reference (`rank_ref.c`) and the native Brief `.a` call, on the
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

## Sequencing

1. Plan + downloads (Go/JDK/Lua) in parallel.
2. C++ (fastest, verified).
3. Go (cgo), Java (JNI), Lua (native ext).
4. C# (guarded).
5. Timing all vs native C.
6. Docs (`ffi-and-export.md` per-language sections), BUGS.md, full suite.

## Baseline (rule 11)

At the current commit: Rust→Brief 1127ns, C→Brief 1092ns, Python native ext
~1300ns / pure call ~200ns, ctypes 1058ns pure. Native C `feature_hash`
~1080ns (the FNV-1a work itself).
