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

- Add `frgn __rt_init() -> Void;`, `frgn __rt_poll() -> Void;`, `frgn __rt_wait() -> Void;` to `lib/std/briev_rt.bv` (or a new `lib/std/rt.bv`)
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

---

## 2026-06-07 Addendum: `trg` owns its runtime — auto-link briev_rt.c

**Status**: Ready to execute (build mode)

### Motivation
`trg` must be a complete abstraction. Currently, using a trigger requires:
1. Writing `trg sigint: Bool @ link __sigint_flag;`
2. Also writing `import "link/briev_rt.c";`
3. Relying on hardcoded `emit_declares` in the LLVM backend (`__rt_init`, `__rt_poll`, `__rt_wait`)
4. The `__rt_init` call in generated `main()` is redundant (constructor handles it)

After this change, `trg` is fully self-contained:
- User writes `trg sigint: Bool @ link __sigint_flag;` — that's it
- Compiler auto-links `briev_rt.c` when any `@ link` trigger is present
- Compiler emits `declare void @__rt_wait()` and `call void @__rt_wait()` when `has_wake_triggers`
- Constructor in `briev_rt.c` handles `__rt_init` automatically
- No `frgn` declarations, no hardcoded strings, no manual C file imports

### Steps

1. **Compiler driver auto-links `briev_rt.c`** (main.rs)
   - During compile pipeline scan, check for any `TopLevel::Trigger` with `LinkRef::Linked(_)`
   - If found, add `briev_rt.c` to link dependencies automatically
   - No changes needed for non-trigger programs (they already don't link briev_rt.c)

2. **Remove `__rt_init` and `__rt_poll` from LLVM backend** (llvm.rs)
   - Remove `call void @__rt_init()` (line 4068) and `call void @__rt_poll()` (line 4069)
   - Constructor in briev_rt.c already calls `__rt_init` at load time
   - `__rt_poll` is a non-essential optimization

3. **Gate `__rt_wait` declare + call on `has_wake_triggers`** (llvm.rs)
   - Only emit `declare void @__rt_wait()` when `has_wake_triggers` is true
   - Already gated for `call void @__rt_wait()` — just needs the declare to match
   - Remove the hardcoded `__rt_*` lines from `emit_declares()`
   - Delete the dead `emit_foreign_declares()` stub

4. **Remove `import "link/briev_rt.c"` from all benchmarks** (benchmarks/*.bv)

5. **Update tests** (llvm.rs tests)
   - `test_rt_declares_present` — expects `__rt_wait` declare when `has_wake_triggers`
   - Remove tests asserting `__rt_init`/`__rt_poll` in generated output
