# Optimization Phases — Regain Performance vs C

Date: 2026-06-20
Status: Plan / Active
Author: randozart

---

## Baseline (from `results-2026-06-16.txt`, commit `8914ac5`)

| Benchmark | Briev (s) | C (s) | Ratio | Winner | Status |
|-----------|-----------|-------|-------|--------|--------|
| knucleotide | 0.2272 | 0.1924 | 1.181x | C | **Regressed 1.5x** vs 0.149s baseline |
| nbody_newton | 7.6564 | 8.7000 | 0.880x | Briev | **Regressed 2.2x** vs 3.48s baseline |
| mandelbrot | 0.8973 | 0.6704 | 1.338x | C | **Regressed 1.78x** vs 0.505s baseline |
| fannkuch_redux | 0.0882 | 0.0674 | 1.310x | C | Improved from 2.25x gap to 1.7x |
| fasta | 26.8059 | 0.2720 | 98.539x | C | **Bug**: putchar# flushes every char |

Regression root cause: commit `392674f` (SSA dominance fix) introduced dual-path dispatch.
mandelbrot/nbody routed to slow `emit_folded_memory_main` (no unrolling, bare GEP+load+store).
Knucleotide hit secondary causes: inline init store bloat, trg IR overhead, SSA phi CFG changes.
Guard hoisting (`e7fac09`) partially recovers mandelbrot/nbody but slow path remains slow.

## Key Missing LLVM Optimizations

| Gap | Severity | Why missing |
|-----|----------|------------|
| `!llvm.loop.*` metadata on main loops | HIGH | Accidental — foreach has it, loop_engine doesn't |
| Dynamic unroll factor | MEDIUM | Hardcoded 4x in `loop_engine.rs:550` |
| `!alias.scope`/`!noalias` on per-field GEPs | MEDIUM | Never implemented |
| `emit_folded_memory_main` unrolling | HIGH | Intentionally created as phi-free fallback; never optimized |
| `cold` on error paths | LOW | Never added |
| `fasta` putchar# fflush per char | HIGH | Runtime FFI issue, not backend |

## Phase 0a — `!llvm.loop.*` metadata (0.5d, NO RISK)

**Condition**: Always safe. `metadata_counter` exists on `LlvmBackend`. `directive.rs` has full `resolve_directives`. `foreach` proves the pattern works. Omission was accidental.

**What**: Add `!llvm.loop` metadata to all backedge branches in `loop_engine.rs`:
- `emit_folded_loop` (phi mode, SSA body4, SSA body1, memory mode)
- `emit_folded_memory_main`
- `emit_main` (tick loop)
- Default: emit `!llvm.loop.vectorize.enable = i1 true`
- If `#unroll`/`#?unroll` directive present: also emit `!llvm.loop.unroll.full`/`enable`

## Phase 0b — Dynamic unroll factor (1d, LOW RISK)

**Condition**: Hardcoded `uf=4` causes register spilling for bodies with high live-float counts.
Use existing `compute_peak_live_floats()` from SLP hazard analyzer.

**What**:
- peak_live_floats > 16 (SSE): `uf = 1`
- Body ≤ 3 fields, ≤ 10 insts: `uf = 8`
- Default: `uf = 4`

## Phase 0c — `!alias.scope` on per-field GEPs (1d, LOW RISK)

**Condition**: Safe for per-field `%State` GEPs (different struct offsets). NOT safe for heap
buffers (list elements, hash map slots) — those can alias across state fields.

**What**: Add per-field alias scope domains on loads/stores in `emit_folded_memory_main`
and `emit_ssa_main`. NOT on list element accesses.

## Phase 1 — Fix `emit_folded_memory_main` (1d, MEDIUM RISK)

**Condition**: Created as phi-free fallback for non-linear bodies. Must keep dominance fix.
Current: `uf=1`, no phi indvar, bare GEP+load+store per field.

**What**: Add compile-time body unrolling (mirror SSA path's body4/body1), counter phi,
`!tbaa` already present, add `!alias.scope` per field. Use SLP hazard to cap unroll factor.

## Phase 2 — Fix fasta putchar# fflush (0.25d, NO RISK)

**Condition**: Runtime library issue. `putchar#` calls `fputc` + `fflush` per char.

**What**: Remove `fflush` from `putchar#`. Add `fflush#` intrinsic if explicit flushing needed.

## Phase 3 — Knucleotide regression (0.5d, DIAGNOSTIC)

**Condition**: Single guard, fast SSA path. Regression from inline init stores (entry block bloat
overwhelming SROA budget) or trg IR overhead.

**What**: Profile IR at each suspect commit. If SROA budget, add `#!large-state` to switch
between inline init stores and `call @init_state`.

## Phase 4 — Fix `ret void` in i32 `main` (0.5d, NO RISK)

**Condition**: Workaround filters `Term`/`TermBang` from top level but misses nested ones
inside `Guarded` blocks with complex swan song expressions.

**What**: Implement `main_body: bool` flag from BUGS.md. Set in all 7 `main()` emitters.
Fallback emits `ret i32 0` instead of `ret void`.

## Phase 5 — Extra metadata (0.5d, NO RISK)

- `cold` on error paths (precondition failure exits, `@llvm.trap`)
- `noreturn` on `@llvm.trap` declaration
- `align 8` on `%State*` function parameters
- `readnone` on pure functions

---

## Execution Order

1. Phase 0a — Loop metadata (infrastructure, unblocks everything)
2. Phase 1 — Fix memory main (largest practical win, unblocks non-linear benchmarks)
3. Phase 0b — Dynamic unroll (prevents regressions from Phase 1 unrolling)
4. Phase 0c — alias.scope (amplifies Phase 1)
5. Phase 4 — Fix ret void (unblocks 9 broken benchmarks)
6. Phase 2 — Fix fasta fflush (biggest single win: 98.5x → ~1x)
7. Phase 3 — Knucleotide investigation (diagnostic)
8. Phase 5 — Extra metadata (polish)
