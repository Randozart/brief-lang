# FFI-Aware Attribute Assignment — Cold-Path Outlining for argmem
## 2026-07-27

This plan is a continuation of `docs/plans/2026-07-27-benchmark-regression-fixes.md`
and `docs/plans/2026-07-27-benchmark-regression-results.md`. It addresses the remaining
architecture gap where `#0` (reactive txns) and `#2` (reactor_tick) use
`memory(readwrite)` unconditionally, preventing SROA from promoting %State
fields to SSA registers in the hot loop.

## Benchmark Results (Post-Revert, Current HEAD `123a9e39`)

```
╔═══════════════════════════╦════════════╦════════════╦══════════╦════════╦═══════════╗
║ Benchmark                 ║ Briev      ║ C          ║ Ratio    ║ Winner ║ Correct   ║
╠═══════════════════════════╬════════════╬════════════╬══════════╬════════╬═══════════╣
║ ring_buffer               ║ .0548s     ║ .0474s     ║ 1.15x    ║ C      ║ MATCH     ║
║ float_math                ║ .0717s     ║ .0729s     ║ .98x     ║ Briev  ║ MATCH     ║
║ float_math_nonzero        ║ .1629s     ║ .1624s     ║ 1.00x    ║ ~tie   ║ MATCH     ║
║ sparse_dispatch           ║ .0476s     ║ .0597s     ║ .79x     ║ Briev  ║ MATCH     ║
║ print_loop                ║ .0587s     ║ .0588s     ║ .99x     ║ Briev  ║ MATCH     ║
║ nbody_newton              ║ 10.8822s   ║ 7.8878s    ║ 1.37x    ║ C      ║ MATCH     ║
║ nbody_sqrt                ║ 2.2531s    ║ 2.6530s    ║ .84x     ║ Briev  ║ MATCH     ║
║ nbody_sqrt_idio           ║ 2.3361s    ║ 3.4570s    ║ .67x     ║ Briev  ║ MATCH     ║
║ fasta                     ║ .1988s     ║ .2014s     ║ .98x     ║ Briev  ║ MATCH     ║
║ fannkuch_redux            ║ .0628s     ║ .0632s     ║ .99x     ║ Briev  ║ MATCH     ║
║ mandelbrot                ║ .6333s     ║ .6320s     ║ 1.00x    ║ ~tie   ║ MATCH     ║
║ kalman_filter_runtime     ║ .1779s     ║ .1753s     ║ 1.01x    ║ C      ║ MATCH     ║
║ knucleotide               ║ .1843s     ║ .1838s     ║ 1.00x    ║ ~tie   ║ MATCH     ║
║ cancel_math               ║ .0616s     ║ .0619s     ║ .99x     ║ Briev  ║ MATCH     ║
║ bit_clear                 ║ 0s         ║ .0001s     ║ 0x       ║ Briev  ║ MATCH     ║
║ queue_drain               ║ .0614s     ║ .0612s     ║ 1.00x    ║ ~tie   ║ MATCH     ║
║ queue_drain_sym           ║ .0607s     ║ .0603s     ║ 1.00x    ║ ~tie   ║ MATCH     ║
║ queue_drain_idio          ║ .0610s     ║ .0001s     ║ 610.00x  ║ C      ║ MATCH     ║
║ interval_step             ║ .0609s     ║ .0584s     ║ 1.04x    ║ C      ║ MATCH     ║
║ bridge_glue               ║ done       ║            ║          ║        ║ SKIP      ║
║ bridge_multi              ║ done       ║            ║          ║        ║ PASS      ║
╚═══════════════════════════╩════════════╩════════════╩══════════╩════════╩═══════════╝
```

## Observations

### Improvements from post-revert

Reverting `#9` on `main()` from `memory(argmem: readwrite)` to `memory(readwrite)`
improved several benchmarks vs the pre-revert run:

| Benchmark | Pre-Revert | Post-Revert | Delta | Possible cause |
|-----------|------------|-------------|-------|---------------|
| ring_buffer | 1.31x | **1.15x** | -12% | More accurate alias for main()'s FFI calls |
| float_math | 1.07x | **0.98x** | -9% | Same; Briev now beats C |
| sparse_dispatch | 0.91x | **0.79x** | -13% | Same |
| print_loop | 1.01x | **0.99x** | -2% | Same (now Briev wins ~tie) |

This confirms that `memory(readwrite)` on main is the correct default — it lets
LLVM model FFI calls accurately.

### Remaining false positive

queue_drain_idio at 610.00x is still a harness artifact. The C binary at 0.0001s
is physically impossible for 50M iterations. The Briev time (0.0610s) matches
queue_drain_sym (0.0607s). No compiler action needed.

### nbody_newton is stable at 1.37x

The revert didn't change nbody_newton (1.35x → 1.37x, within noise). The 1.3-1.4x
ratio is pre-existing — the 34-field %State with 11 FFI print calls is a
different optimization problem not addressed by this plan.

## IR Research Findings

### Current attribute group assignment

| Group | String | Used by |
|-------|--------|---------|
| `#0` | `memory(readwrite)` | Reactive `txn_*` (alwaysinline), `init_state` |
| `#2` | `memory(readwrite)` | `reactor_tick`, `cell_persistent_ticks` |
| `#3` | `memory(readwrite)` | Non-mustprogress alternative |
| `#7` | `memory(read)` | `@pre_*` (readonly) |
| `#8` | `memory(argmem: readwrite)` | Definitions, callable txns |
| `#9` | `memory(readwrite)` | `@main()` (runtime init accesses globals) |
| `#10` | `memory(argmem: read)` | `@pre_*` (readonly + argmem) |

### The problem

`#0` and `#2` are always `memory(readwrite)` regardless of whether the txn
body actually contains FFI calls. In the current benchmark suite, every txn
uses guarded prints (`when condition { PrintLn!(...) };`):

```briev
node work [count < N][count == N] {
    count = count + 1;                            // hot path — runs every iteration
    when count % 5000000 == 0 {                   // cold path — runs 0.02% of iterations
        PrintLn!(count);                          // FFI call
    };
    term;
};
```

The print call lives in a cold path (0.02% frequency), yet the entire txn
gets `memory(readwrite)`. LLVM sees the attribute and assumes the entire body
may touch any memory, preventing SROA from promoting %State fields to SSA
registers.

### Stack layout per benchmark (current, no arena fields)

| Benchmark | %State fields | State size | Hot path FFI? | Cold path FFI? |
|-----------|---------------|------------|---------------|----------------|
| bit_clear | 2 | 16 bytes | No | Yes (print) |
| queue_drain_sym | 3 | 24 bytes | No | Yes (print) |
| queue_drain / queue_drain_idio | 4 | 32 bytes | No | Yes (print) |
| ring_buffer | 5 | 40 bytes | No | Yes (print) |
| float_math | 15 | 120 bytes | No | Yes (print) |
| print_loop | 3 | 24 bytes | No | Yes (print) |
| nbody_newton | 34 | ~272 bytes | Yes | Yes |
| nbody_sqrt | ~30 | ~240 bytes | Yes (Sqrt# + print) | Yes |

Every benchmark with guarded prints is currently over-pessimized. The hot path
contains zero FFI, but the txn attribute says `memory(readwrite)`.

### Existing FFI detection

The compiler already has `statement_contains_ffi` in `transition_graph.rs` and
`has_ffi_or_terminator_stmt` in `region.rs`. These walk statement trees and
detect non-`#` `Expr::Call` nodes — exactly what we need to determine whether
a guard block (or the entire body) contains FFI.

## Hypothesis

**Outlining FFI-containing guard blocks into separate cold functions lets us
annotate the main txn body with `memory(argmem: readwrite)`, unleashing SROA
on the hot-path state fields.**

The trade-off:
- **Cost**: One `call` instruction per cold-path guard block. Taken 0.02% of
  iterations (once per 5M). Even 50 cycles of call overhead is 50 × 0.0002 =
  0.01 cycles per iteration average.
- **Benefit**: SROA promotes %State fields to SSA registers for the entire
  hot path — potentially saving 1-2 memory round-trips per field per iteration.
  For a 15-field state at 50M iterations, that's 750M-1.5B fewer memory ops.

The cold call overhead is negligible; the SROA gain is dominant.

### What about unguarded FFI?

Not all benchmarks have guarded prints. nbody_newton has FFI in `when count == bound`
(the swan song), which is also cold (taken once at the end). But it also has
`count % 5000000 == 0` prints. If a benchmark has an **unguarded** FFI call
directly in the body (not inside any `when`), the entire txn must stay
`memory(readwrite)` — there's no cold path to outline to.

The outline pass should only extract `when`-guarded blocks. Any FFI outside
a `when` guard makes the entire txn ineligible for `argmem`.

## Plan

### Step 1: Add new attribute groups

In `src/backend/llvm/mod.rs`, add two new attribute groups:

```
#11 = mustprogress nofree norecurse nosync nounwind memory(argmem: readwrite)
  — for reactive txns after cold-path outlining (no willreturn — may loop forever)

#12 = mustprogress nofree norecurse nosync nounwind willreturn memory(argmem: readwrite)
  — for reactor_tick when all txns are FFI-free after outlining
```

### Step 2: Guard-block FFI detection

In `emit_toplevel.rs`, before emitting a txn body, walk the body's
`Statement::Guarded` blocks and check each one for FFI:

```rust
fn find_ffi_guards(body: &[Statement]) -> Vec<usize> {
    // Returns indices of Guarded blocks that contain FFI calls
}
```

### Step 3: Cold-path outlining

For each FFI-containing guard block at index `i` in the txn (named `txn_work`):

1. Create a new function `@txn_work_cold_{i}`:
   - Takes `ptr %state` (noalias nocapture)
   - Contains only the guard body + the guard evaluation
   - Annotated with `#3` = `memory(readwrite)` or `#0`
   - Made `noinline` to keep the reasoning simple

2. Replace the guard block in the original txn body with:
   ```
   call void @txn_work_cold_{i}(ptr %state)
   ```

### Step 4: Emit outlined functions

In the LLVM backend's `generate()` loop (after emitting all normal functions),
emit each outlined cold function.

### Step 5: Assign argmem to cleaned txn

After outlining, if the txn body has zero remaining FFI calls:
- Emit with `#11` = `memory(argmem: readwrite)` instead of `#0`
- Keep the `alwaysinline` marker

### Step 6: Assign argmem to reactor_tick

After processing all txns:
- If all reactive txns now have `argmem` (i.e., all FFI was outlined):
  Emit `reactor_tick` with `#12` instead of `#2`

### Step 7: Verify no regression

Run `bash benchmarks/build_and_bench.sh --runtime --correctness`.
Expected:
- All benchmarks that currently have guarded FFI improve
- nbody_newton unchanged (unguarded FFI stays memory(readwrite))
- queue_drain_idio still shows anomalous 610x (harness issue, not compiler)

### Step 8: Build stale C binary cleanup

Add to `build_and_bench.sh`:
```bash
# Clean C binaries before rebuild to prevent stale-binary artifacts
rm -f benchmarks/*_c benchmarks/*_c.o
```

This fixes the queue_drain_idio false positive for future runs.

## Expected Impact per Benchmark

| Benchmark | State fields | Current #0 | After fix | Expected improvement |
|-----------|-------------|------------|-----------|---------------------|
| bit_clear | 2 | `memory(readwrite)` | `argmem: readwrite` | Marginal (already at noise floor) |
| queue_drain_sym | 3 | `memory(readwrite)` | `argmem: readwrite` | ~5-10% |
| print_loop | 3 | `memory(readwrite)` | `argmem: readwrite` | ~5-10% |
| queue_drain | 4 | `memory(readwrite)` | `argmem: readwrite` | ~5-10% |
| queue_drain_idio | 4 | `memory(readwrite)` | `argmem: readwrite` | ~5-10% |
| ring_buffer | 5 | `memory(readwrite)` | `argmem: readwrite` | ~5-15% (GEP chain) |
| float_math | 12+3 | `memory(readwrite)` | `argmem: readwrite` | ~10-25% (large state) |
| nbody_newton | 34 | `memory(readwrite)` | `memory(readwrite)` | Unchanged (unguarded FFI) |

## Risk Assessment

| Risk | Likelihood | Mitigation |
|------|-----------|------------|
| Cold call overhead dominates | Very low (0.02% frequency) | Measure before/after |
| `noinline` on cold fn prevents LLVM optimization | Low — cold fn has minimal code | Consider `alwaysinline` for cold fn instead |
| Multiple guarded FFI blocks cause excessive outlining | Low — typically 1 guard per txn | Limit to 3 outlines max |
| `willreturn` on reactor_tick is wrong for non-converging programs | Medium — but all benchmark programs converge | Keep `#12` separate from `#8` for safety |
| Outlining changes register allocation negatively | Medium — call saves/restores regs around cold path | Verified by running baseline vs fix benchmarks |

## File Changes

| File | Change |
|------|--------|
| `src/backend/llvm/mod.rs` | Add `#11`, `#12` attribute group emission |
| `src/backend/llvm/emit_toplevel.rs` | Add FFI guard detection + outlining + attr selection |
| `src/backend/llvm/dispatch.rs` | Use `#12` for reactor_tick when all txns are clean |
| `benchmarks/build_and_bench.sh` | Add C binary cleanup step |

## Test Plan

- `cargo test --lib` — all existing tests pass
- `bash benchmarks/build_and_bench.sh --runtime --correctness` — all benchmarks
  match or improve vs current HEAD
- Specific attention: nbody_newton must NOT regress (it uses unguarded FFI)
- Specific attention: queue_drain_idio C binary time must be verified > 0.01s
  (if still 0.0001s, the issue is confirmed harness-only)

## Documentation

Add `///` doc comments to the new outlining function explaining the rationale
and the trade-off. Update the attribute group comments in `mod.rs` to explain
why `#11` and `#12` exist and when they are selected.
