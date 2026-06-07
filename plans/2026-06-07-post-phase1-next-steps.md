# Next Steps — Post-Phase 1 Cleanup

**Date**: 2026-06-07
**Status**: Ready to execute
**Preceding work**: Phase 1 (no-magic FFI dispatch) complete

## Priority Order

### 1. Benchmark regression check + modernization

Run `bash benchmarks/build_and_bench.sh` to verify no regressions from today's changes.

Then fix stale benchmarks:

**Remove redundant `#!exit` pragmas** from 4 benchmarks that already use `term! ->` swan song:
- `benchmarks/fannkuch_redux.bv` — remove `#!exit count == N && checksum >= 0 && max_flips >= 0;`
- `benchmarks/knucleotide.bv` — remove `#!exit count == N && hash >= 0;`
- `benchmarks/mandelbrot.bv` — remove `#!exit count == N && escapes >= 0 && ...;`
- `benchmarks/nbody_sqrt.bv` — remove `#!exit count == bound;`

**Add FFI output to prevent dead-code elimination** in 10 benchmarks:
- Each needs `frgn __print_int(n: Int) -> Bool ;` declaration and periodic `[count % N == 0] { __print_int(count); };` in the body

### 2. Eliminate hardcoded `emit_declares` in LLVM backend (Step B)

- Add `frgn __rt_init() -> Void;`, `frgn __rt_poll() -> Void;`, `frgn __rt_wait() -> Void;` to `lib/std/brief_rt.bv` (or a new `lib/std/rt.bv`)
- Change `emit_declares()` in `llvm.rs:1844` to emit from `self.frgn_map` instead of hardcoded strings
- Delete the hardcoded block (it's ~20 lines with the TODO comment)
- Update test assertions at `llvm.rs:5658-5746` if needed
- All 526 tests must pass

### 3. Stdlib completeness (if needed)

- `lib/std/collections.bv` is minimal (just `append`/`len` for `List<Int>`) — could be extended to generic collections
- No `__builtin_*` references remain — already clean

## Completed
- Phase 1: No-magic FFI dispatch (2026-06-07, commit 5070564)
- MultiSlice mask/stride evaluation (2026-06-07)
- BracketOp refactor (prior session)
- Phases 11-13 (sync, HashMap/HashSet, Stack/Queue/Tuple)
