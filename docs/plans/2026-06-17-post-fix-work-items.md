# Post-Fix Work Items — Plan for 2026-06-17

**Datetime**: 2026-06-17T09:45:00-06:00

## Status

The duplicate-register bug (`emit_expr.rs:107` missing `return`) is fixed.
All 909 tests pass. Mandelbrot (1.045x) and knucleotide (1.005x) now compile
and produce correct output matching C. Fannkuch_redux is at 1.102x (unchanged).

## Remaining Work Items

### 1. Fasta buffer coalescing (highest impact)

**Problem**: Fasta is 99x slower than C (~23.7s vs 0.27s at BOUND=100M).
Each `putchar#(c)` compiles to a single `fputc(c, stdout)` call — ~10M
libc calls at the benchmark bound.

**Solution**: Coalesce consecutive `putchar#` intrinsic calls into
`fwrite(buf, 1, N, stdout)` per line. This is a pure LLVM backend
optimization — no source changes to fasta.bv.

**Approach**:
- In `emit_expr.rs` `Intrinsic::Putchar` handler, peek ahead at the
  next statement in the body. If it's also a `Putchar` call, buffer
  the character and skip the `fputc` emission.
- Emit the buffered line via `fwrite` when a non-`Putchar` statement
  is encountered or the buffer is full.
- Use a fixed-size buffer (e.g., 8192 bytes) on the C stack (alloca).

**Impact**: ~99x gap → ~1-2x. The most impactful single optimization
remaining in the compiler.

### 2. Fannkuch remaining gap (1.09x → ?)

**Problem**: Fannkuch_redux at 1.09x vs C. The remaining gap is from
phi placement differences — 14 per-field phis vs C's 12 rotation phis.

**Investigation**:
1. Run `opt -O3 -pass-remarks-missed=sroa,gvn,licm` on both Briev and
   C `.ll` files to see what LLVM can't optimize in each.
2. Check if `-march=native` closes the gap (already used in benchmark
   harness but may not be passed through `llc` properly).
3. Compare the phi placement in the unrolled body — Briev uses
   `insertvalue`/`extractvalue` chains while C uses direct phi nodes.

**Low effort, potentially closes to 1.0x**.

### 3. Dead-field elimination for rotation-only fields

**Problem**: fannkuch's 12 permutation fields (p0..p11) are rotated
every iteration but never observed externally — only `checksum` and
`seed` produce observable output. The compiler warns they're "never
read" but still emits all 12 as phi nodes at the loop backedge.

**Solution**: Extend liveness analysis to trace def-use chains through
phi nodes. If a field is only assigned and read within the loop body
but never observed by an FFI call or `#!exit`, eliminate it.

**Impact**: Removes ~12 phis from the loop backedge, reducing register
pressure. Modest but measurable improvement on fannkuch.

### 4. Officina-cli compilation test

**Result** (2026-06-17): Officina-cli compiles to valid LLVM IR with
the fixed compiler. No SSA dominance violations, no duplicate register
errors. The `compile --target native` subcommand is used.

**Remaining issues**:
- **Arrow operator stubs**: 7 warnings — `<-` (collect/discard/transfer)
  are stubs returning 0 in the LLVM backend. Officina-cli uses these
  for list/map operations. Stubs cause silent data corruption.
- **Missing runtime symbols**: `briev_spawn_with_output`, `briev_getenv`,
  `json_parse`, `briev_tty_raw_mode`, `briev_tty_size`, `briev_read_file`,
  `__int_to_str`, `__rt_wait`, etc. are declared as `frgn` but the
  runtime library (`briev_rt.c`) was deleted in Phase I. The `compile`
  subcommand doesn't link against any runtime.
- **No exit path**: Warning: "program has wake triggers but no exit
  path" — the reactive loop spins forever after convergence.

**Next step**: These are upstream officina-cli issues, not compiler
regressions. The compiler fix unblocked the frontend (LLVM IR generation).

### 5. Fix `-o` flag behavior

**Problem**: The `llvm` subcommand's `-o <file>` flag doesn't write
to the specified path — it writes to `<basename>.ll` in the current
directory instead. Only `--out <dir>` works correctly (writes
`<dir>/<basename>.ll`). The `-o` flag appears to be parsed but ignored
at `main.rs` or `mod.rs` in the llvm subcommand handler.

**Impact**: Low — the benchmark harness uses `--out` and works. But
manual testing with `-o /tmp/foo.ll` silently writes to `./foo.ll`.

## Priority Order

1. Fast buffer coalescing (biggest single impact)
2. Officina-cli runtime linking
3. Fasta buffer coalescing implementation
4. Fannkuch phi gap investigation
5. Dead-field elimination
6. `-o` flag fix

## Test Plan

- All work items: `cargo test --lib` must pass
- Fasta: `BOUND=100M briev-compiler llvm fasta.bv --out /tmp/` + compile/run
- Fannkuch: `BOUND=50M` timing, compare before/after ratios
- Officina: `briev compile officina.bv --target native`, verify IR is valid
- Every commit: 909+ tests pass, no regression in existing benchmarks
