# Adaptive Loop Dispatch: A005a + A005c + Dead-Field Analysis

Date: 2026-07-05
Status: Plan
After: HEAD~3 (50c5527..5b2c9e1) — A005e regression
Target: Combine best of all dispatch paths into an adaptive decision tree.

## 1. Executive Summary

The compiler lost ~50% perf on knucleotide, mandelbrot, float_math, and
fannkuch_redux during the 2026-07-03/04 optimization series.  The root
cause: three dispatch paths (A005a single-chain, A005b memory, A005c
per-field phi) were collapsed into one (A005c per-field phi) on July 3
at `a71c586`, then replaced with A005e (memory) on July 5 at `5b2c9e1`.

No single dispatch path wins for all programs.  The fix is an adaptive
decision tree that selects the optimal path per txn.

## 2. The Three Dispatch Paths

### 2.1 A005a — Single %State Phi (insertvalue chain)

What: A single `%State` SSA phi at the loop header.  Reads use
`extractvalue`, writes build an `insertvalue` chain ending with a
single `store` to a `%slot` alloca at loop end.  The next iteration
loads the updated state from the alloca.

When it wins: ALL fields written every iteration (dense writes).
LLVM sees one SSA value flowing through the loop and optimizes the
entire insertvalue chain as a unit (GVN, SCCP, DSE all fire on the
whole state at once).

Benchmarks where A005a was best (from `scripts/benchmark-results.sh`,
June 11 era):
- knucleotide:  0.42x (beats C by 2.4×) ← current: ~1.00x
- mandelbrot:   0.64x (beats C by 1.56×) ← current: ~1.06x
- float_math:   0.67x (beats C by 1.5×)  ← best recent: 0.78x

When it loses: Branching guards create extra phi predecessors, and
the `insertvalue` chain becomes long for large field counts (>15).

Removed in: `a71c586` (2026-07-03, "simplify dispatch").

### 2.2 A005b — Memory Loop (GEP load/store)

What: Per-field GEP loads at body entry, GEP stores at body exit.
No phi nodes.  Counter kept in memory.

When it wins: Bodies with non-linear branching guards (phi dominance
failures).

When it loses: Always — GEP+load+store per field per iteration is
~3 uops vs 0 for phi-based.

Removed in: `a71c586` (2026-07-03, "simplify dispatch").  Obsoleted
by A005c which handles guarded bodies gracefully (each guard branch
stores to the same GEP address; the latch reloads and GVN eliminates
the redundant load-via-store).

### 2.3 A005c — Per-Field Phi Loop

What: Each state field gets its own phi node at the loop header.
Reads use the phi register directly.  In the latch, modified fields
reload from %State (GVN eliminates the redundant load-via-store
roundtrip).  Unmodified fields use identity backedge (zero insns).

When it wins: Sparse writes (not all fields modified), large field
counts, dead fields present, parallel-safe bodies.

Benchmarks where A005c was best (from 876c6f0, 2026-07-05):
- nbody_newton:      0.89x (beats C)       ← A005e: 1.41x
- nbody_sqrt_idio:   0.75x (beats C)       ← A005e: 0.96x
- float_math_nonzero: 1.00x (tied)         ← A005e: 2.46x
- interval_step:      0.01x (100× faster)  ← A005e: 1.00x

Sub-optimizations added after its introduction:
- Path A (`ad89ee5`): Zero stores in hot loop when done: doesn't
  read %State.  The phi registers + pending_phi_native_backedge
  carry all values forward.  ~20% improvement across all A005c
  benchmarks.
- Phi commit block (`eb842d8`): When done: needs %State, create
  last-value allocas that store the phi's final value ONCE at loop
  exit instead of every iteration.  Extends Path A benefit to
  benchmarks with swan_song guards.
- Parallel-safe mode (`1d8d385`): All reads use old phi values,
  making every computation independent for LLVM vectorization.
  Counter and guard condition fields are exempt.
- LLVM attribute optimizations (`8b6b6b3..78849d2`): !invariant.load
  for read-only fields, !range for narrow types, dereferenceable(N)
  for Ptr params, argmemonly, align_of.
- SROA chunk allocas (`641eb41`): Split %State into ≤15-field chunks
  so SROA can decompose even 31-field states.
- Dead-field liveness analysis (`876c6f0`): Helpers to identify
  fields never consumed by observable output.  NOT YET WIRED in
  A005c — only wired in A005e.

Introduced in: `8c08890` (2026-07-03), made universal in `a71c586`,
Path A in `ad89ee5`, commit block in `eb842d8`.

### 2.4 A005e — Hybrid Counter Phi + Memory Fields (REGRESSION)

What: Only the counter has a phi node.  All fields loaded from %State
at body entry via `pre_load_all_fields`.  Stores always go to %State
every iteration (`needs_state_stores_in_body = true`).  No commit
block.  done: reads from %State directly.

Why it regressed: Reintroduces the exact memory traffic that Path A
eliminated.  `pre_load_all_fields` loads ALL fields (including dead
ones).  Every store goes to %State (not even dead-field filtering
helps fully since loads still happen).  For interval_step (no data
fields, just a counter), this went from 0.01x to 1.00x — literally
100× slower.

Benchmark impact (A005e at ff7599d vs A005c at 876c6f0):
- nbody_newton:     0.89x → 1.41x  (-37%)
- nbody_sqrt_idio:  0.75x → 0.96x  (-22%)
- interval_step:    0.01x → 1.00x  (-99%)
- fannkuch_redux:   1.61x → 2.16x  (-25%)

Introduced in: `5b2c9e1` (2026-07-05), wired in `d18d236`.

PLAN ACTION: Revert A005e.  The A005c + Path A + commit block
approach is strictly better.

## 3. Regression Timeline

Commit graph (oldest first):

```
ff7599d  Jul 3 16:02  hoist terminating guard body to post-loop block
                      ↑ Pure loop body enables Path A.  Good.
8c08890  Jul 3 12:31  Phase 3: add countable loop with per-field phi nodes
                      ↑ A005c introduced.  Good.
a71c586  Jul 3 15:05  simplify dispatch: per-field phi loop as default
                      ↑ Collapsed A005a/A005b into A005c.  Net positive
                        for most benchmarks, but regressed knucleotide
                        (0.42x → ~1.00x) and mandelbrot (0.64x → ~1.06x)
                        by removing the A005a insertvalue-chain path.
ad89ee5  Jul 4 13:03  eliminate dead stores in A005c hot loop bodies
                      ↑ Path A: zero stores.  ~20% across the board.
eb842d8  Jul 4 22:54  phi commit block: eliminate per-iteration body stores
                      ↑ Extends Path A to swan_song benchmarks.  Good.
1d8d385  Jul 4 21:20  enable parallel-safe mode for all A005c bodies
                      ↑ Enables LLVM vectorization.  Good.
a546464  Jul 4 21:30  refine parallel-safe exemption analysis
                      ↑ More precise.  Good.
8b6b6b3..78849d2 Jul 4  LLVM attribute optimizations
                      ↑ !invariant.load, !range, dereferenceable, etc.
                        All incremental improvements.
876c6f0  Jul 5 11:16  Step 1: Add dead-field liveness analysis helpers
                      ↑ Analysis code added (dead code — not yet wired).
                        This is the pinnacle: A005c + all improvements
                        + dead-field scaffolding ready to wire.
5b2c9e1  Jul 5 11:29  Steps 2-5: A005e hybrid loop
                      ↓ REGRESSION: Replaced A005c with A005e.
                        Memory traffic reintroduced.  All gains lost.
d18d236  Jul 5 11:31  Step 6: Wire dead-field analysis
                      ↓ Only marginally helps A005e (still loads all
                        fields from memory at body entry).
50c5527  Jul 5 12:13  Fix nbody_sqrt dead-field liveness
                      ↓ Bug fix for A005e.  Doesn't solve the root cause.
```

Benchmark data across key commits (BOUND=50000000, runtime tag):

```
Benchmark            Jun11 cache  a71c586  ad89ee5  876c6f0  ff7599d  HEAD
                    (A005a era)  (A005c)  (Path A)  (peak)   (A005e)  (now)
──────────────────  ───────────  ───────  ────────  ───────  ───────  ──────
nbody_newton          no data     1.42x    1.34x?    0.89x    1.41x   1.54x
nbody_sqrt           0.82x        1.22x    ?         1.28x    1.23x   1.26x
nbody_sqrt_idio      no data      0.98x    ?         0.75x    0.96x   0.96x
float_math           0.67x        0.85x    0.78x     0.82x    0.80x   0.86x
float_math_nonzero   1.10x        ?        ?         1.00x    2.46x   2.44x
knucleotide          0.42x        1.00x    0.96x     1.01x    1.00x   1.00x
mandelbrot           0.64x        ?        ?         broken   1.06x   broken
fannkuch_redux       0.99x        ?        ?         1.61x    2.16x   1.83x
interval_step        ?            ?        ?         0.01x    1.00x   0.01x
ring_buffer          0.80x        0.97x    0.88x     0.98x    1.02x   1.05x
fasta                0.99x        ?        ?         1.00x    1.02x   1.01x
kalman_filter        ?            1.01x    0.99x     0.91x    1.01x   0.90x
bit_clear            ?            1.14x    0.68x     1.00x    1.00x   1.16x
```
(? = not measured in that run; values from adjacent commits used as proxy)

## 4. The Decision Tree

### 4.1 Inputs

Collected per txn during analysis:

- `write_density`: fraction of state fields written by the body
  (0.0 = read-only, 1.0 = every field written every iteration)
- `field_count`: total number of state fields
- `has_guards`: whether the body contains branching guards
- `parallel_safe`: whether all body computations are independent
- `dead_field_ratio`: fraction of fields never consumed by output
- `has_swan_song`: whether term! has a post-loop swan_song body

### 4.2 Decision Rules

Evaluated in order:

```
1. if body is pure AND bound is constant:
     → A000c (pure counter fold, O(1) — single store, no loop)

2. if write_density >= 0.5 AND field_count < 8 AND !has_guards:
     → A005a (single %State phi, insertvalue chain)
     Rationale: dense writes mean every field is updated; the
     single-chain approach lets LLVM optimize the entire state
     as one SSA unit.  A005c would create N phi nodes + N
     backedge reloads for the same work.

3. if field_count >= 30 AND write_density < 0.3:
     → A005c (per-field phi) with dead-field elimination
     Rationale: most fields are loop-invariant (read-only) or
     dead.  A005c lets us skip phi nodes for dead fields and
     use identity backedge for read-only fields.  The ~30+
     phi threshold is where SROA chunk decomposition matters.

4. else → A005c (per-field phi) with Path A (zero stores)
     Rationale: the universal path.  Per-field phis handle
     sparse writes efficiently.  Path A eliminates all body
     stores.  The phi commit block handles swan_song.

5. if has_swan_song AND path is A005c:
     → A005c with phi commit block (last_val_temps allocas)
     Rationale: the commit block stores phi final values once
     at loop exit instead of every iteration.
```

### 4.3 Cost Model Simplifications

- `write_density` is computed from the write_set (collected during
  pre-analysis).
- `dead_field_ratio` from `trace_live_fields` (876c6f0).
- `parallel_safe` from `is_body_parallel_safe`.
- `field_count` from `ctx.field_index_map.len()`.
- `has_guards` from scanning body for `Statement::Guarded`.
- `has_swan_song` from scanning body for `Statement::TermBang`.

### 4.4 Threshold Tuning

The thresholds in rule 2 (density ≥ 0.5, field_count < 8) and
rule 3 (field_count ≥ 30, density < 0.3) are initial guesses based
on benchmark analysis.  They must be validated and tuned via:

```bash
bash benchmarks/build_and_bench.sh --runtime
# Before/after comparison against results/2026-07-05-baseline.txt
```

If a benchmark regresses, the first diagnostic step is checking
which dispatch path was selected (printed as `info: txn '...'
dispatched via ...` in the compiler output).

## 5. Implementation Phases

### 5.1 Phase 1: Revert A005e → A005c

Files: `src/backend/llvm/loop_engine.rs`, `context.rs`

Changes:
1. Restore `emit_countable_setup_phis_and_header` to per-field phi
   version (from 876c6f0).  Every field gets its own phi node and
   backedge register.  The counter phi is one of them (not special).
2. Restore `emit_countable_body` to call `phi_regs_to_ssa_old()`
   instead of `pre_load_all_fields()`.
3. Restore `emit_countable_latch` to reload modified fields from
   %State (or use `pending_phi_native_backedge`) and use identity
   backedge for unmodified fields.
4. Restore `emit_countable_main` with the commit block
   (`last_val_temps` allocas) and `exit_label` logic (commit vs
   done).
5. Remove `pre_load_all_fields` call from `emit_countable_body`.
6. Update `emit_hoisted_post_loop_prints` to use `last_val_temps`
   when they exist, falling back to `pre_load_all_fields` otherwise.

Preserve everything from `eb842d8..876c6f0`: Path A, commit block,
parallel-safe mode, LLVM attributes, SROA chunks.

### 5.2 Phase 2: Wire Dead-Field Liveness into A005c

Files: `src/backend/llvm/loop_engine.rs` (helpers already exist)

Changes:
1. In `emit_countable_main`, call `trace_live_fields(body, ...)`
   BEFORE phi setup to determine the set of live fields.
2. In `emit_countable_setup_phis_and_header`, skip phi node
   creation for fields NOT in the live set.  Dead fields get
   no init load, no phi register, no backedge register.
3. Apply `filter_dead_assignments(body, live_fields)` before body
   emission AND before parallel-safe scanning.  The filtered body
   is used for both.

Effect on fannkuch_redux: seed and max_flips are dead (written
but never printed or output).  Removing their phi nodes shrinks
the body from ~80 to ~40 LLVM instructions, enabling the loop
unroller to fire (4× unrolling).

### 5.3 Phase 3: Restore A005a Dispatch Option

Files: `src/backend/llvm/loop_engine.rs`, `mod.rs`

Changes:
1. Restore `emit_folded_main` with `use_phi=false, body=Some`
   path (insertvalue chain).  This uses a single `%State` phi,
   `extractvalue` for reads, `insertvalue` chain for writes,
   one `store` to a `%slot` alloca per iteration.
2. In `emit_countable_main` dispatch (`mod.rs`), add the adaptive
   decision tree from Section 4.2.
3. Ensure A005a also uses Path A logic (no stores when done:
   doesn't need %State).  The single `store %State %val, ptr %slot`
   is not per-field — it's one store for the whole state.  That's
   already optimal.

### 5.4 Phase 4: Regression Guards

1. Record benchmark baseline to `results/2026-07-05-baseline.txt`.
   Format:
   ```
   benchmark_name briev_sec c_sec ratio winner source_commit
   ```
   The `source_commit` column records the git hash where this ratio
   was measured, so regressions can be root-caused.

2. Add a `--check-regression` flag to `build_and_bench.sh` that
   compares current results against the baseline and exits non-zero
   if any benchmark regresses by >10%.

3. For each dispatch path, add a unit test that verifies the path
   is selected for known programs:
   ```
   #[test]
   fn test_dispatch_selects_a005a_for_knucleotide() { ... }
   #[test]
   fn test_dispatch_selects_a005c_for_nbody_newton() { ... }
   ```

4. Add a compiler warning when a path other than the optimal one
   is selected for a known benchmark pattern.

### 5.5 Phase 5: Code Commenting Mandate

Every dispatch decision point gets a comment block documenting:
```
// 2026-07-05: Why [chosen path] for [criteria]:
//   A005a (single-chain): dense writes, small fields, no guards.
//     → knucleotide: 0.42x, mandelbrot: 0.64x
//   A005c (per-field phi): sparse writes, large fields, dead fields.
//     → nbody_newton: 0.89x, interval_step: 0.01x
//   If this heuristic is wrong, benchmark X will regress to ~Yx.
//   Measure with: bash benchmarks/build_and_bench.sh --runtime
```

Every backend emission variant gets a comment documenting:
```
// 2026-07-05: A005a / A005c / A005e — what changed, what pattern
// it targets, what it costs.  See docs/plans/2026-07-05-adaptive-
// loop-dispatch.md for the full decision tree.
```

## 6. Verification

### 6.1 Correctness

```bash
cargo test --lib           # All 1398+ tests pass
bash benchmarks/build_and_bench.sh --correctness  # All benchmarks MATCH
```

Known correctness issues to fix:
- fannkuch_redux: output "6" vs C "10" in several commits.
  Root cause: dead-field elimination for seed/max_flips must not
  remove them if they affect the loop's convergence proof (they
  are only read by their own backedge — self-referential dead
  cycle).
- bit_clear: output "" vs C "0" at 876c6f0.  Root cause: check
  whether the final print is correctly emitted.
- sparse_dispatch, queue_drain: "use of undefined value" in IR.
  Root cause: phi register not defined in all predecessor blocks.
  This is an A005c/A005e emission bug unrelated to dispatch.

### 6.2 Performance Targets

| Benchmark | Target Ratio | Best Known | Source |
|-----------|:-----------:|:----------:|--------|
| knucleotide | ≤ 0.50x | 0.42x | Jun 11 baseline (A005a) |
| mandelbrot  | ≤ 0.70x | 0.64x | Jun 11 baseline (A005a) |
| float_math  | ≤ 0.75x | 0.67x | Jun 11 baseline (A005a) |
| nbody_newton | ≤ 0.95x | 0.89x | 876c6f0 (A005c peak) |
| nbody_sqrt_idio | ≤ 0.80x | 0.75x | 876c6f0 (A005c peak) |
| interval_step | ≤ 0.02x | 0.01x | 876c6f0 (A005c peak) |
| float_math_nonzero | ≤ 1.05x | 1.00x | 876c6f0 (A005c peak) |
| fannkuch_redux | ≤ 1.20x | 0.99x | Jun 11 baseline (A005a) |
| fasta | ≤ 1.00x | 0.99x | Jun 11 baseline (A005a) |
| ring_buffer | ≤ 0.85x | 0.80x | Jun 11 baseline (A005a) |
| bit_clear | ≤ 1.00x | 0.68x | ad89ee5 (Path A peak) |

"Target Ratio" = initial goal.  "Best Known" = all-time best.  If
the best-known came from a different dispatch path than the one the
decision tree picks, investigate whether the decision tree's criteria
can be refined to select the optimal path.

### 6.3 Optimized IR Quality Checks

For the hot loop in each benchmark, verify:
1. No GEP+load+store roundtrip for fields in Path A mode.
   (grep the optimized `.ll` for `getelementptr` inside the loop.)
2. SROA decomposes State chunks: `opt -O3 -pass-remarks=sroa`
   shows `promoted` for all non-dead fields.
3. Vectorization fires where expected: `opt -O3 -pass-remarks=loop-vectorize`.
4. Loop is rotated and countable: `opt -O3 -pass-remarks=licm`.

### 6.4 Unit Tests for Dispatch Selection

```rust
// After implementing the decision tree, add tests like:
#[test]
fn dispatch_knucleotide_selects_a005a() {
    // knucleotide: 4 fields, write_density=1.0, no guards
    assert_eq!(select_dispatch_path(4, 1.0, false), DispatchPath::A005a);
}

#[test]
fn dispatch_nbody_selects_a005c() {
    // nbody_newton: 31 fields, write_density~0.35, guards present
    assert_eq!(select_dispatch_path(31, 0.35, true), DispatchPath::A005c);
}

#[test]
fn dispatch_interval_step_selects_a005c() {
    // interval_step: 2 fields, write_density=0.5, no guards
    // But one field is the counter (dense for that field)
    assert_eq!(select_dispatch_path(2, 0.5, false), DispatchPath::A005c);
}
```

## 7. Risks and Mitigations

### 7.1 Risk: SROA fails on large phi webs

A005c with ~31 fields creates 31 phi nodes.  Without chunk allocas
(641eb41), SROA bails out.  Mitigation: already in place — chunk
alloca at MAX_FIELDS_PER_ALLLOCA=15.

### 7.2 Risk: A005a insertvalue chain too long

A005a creates one `insertvalue` per field write.  For 31 fields,
that's a 31-deep insertvalue chain.  LLVM may explode compilation
time.  Mitigation: A005a is only selected for field_count < 8.

### 7.3 Risk: Dead-field analysis removes convergence-critical fields

A field may be "dead" w.r.t. output but still necessary for loop
convergence (e.g., the counter is only compared against the bound
and never printed).  `trace_live_fields` traces through guard
conditions, so it WILL include fields used in pre/postconditions.
Verified by the correctness: if dead-field elimination removes a
field required for convergence, the loop diverges and the test
hangs or produces wrong output.

### 7.4 Risk: Decision tree thresholds wrong for future programs

Benchmarks are a small sample.  A program with 7 fields and
write_density=0.6 might be better with A005c.  Mitigation: the
decision tree logs its choice per txn at `info` level.  Future
regression reports can reference the logged path.  Thresholds are
documented in this plan and can be tuned.

## 8. Appendix: Key Commits Reference

```
Commit        Date       Description
────────────  ────────   ───────────────────────────────────
8c08890       Jul 3      A005c per-field phi loop (initial)
a71c586       Jul 3      Collapse A005a/A005b → A005c
ad89ee5       Jul 4      Path A: zero stores in hot loop
eb842d8       Jul 4      Phi commit block (last_val_temps)
1d8d385       Jul 4      Parallel-safe mode for A005c
8b6b6b3       Jul 4      LLVM attribute optimizations
876c6f0       Jul 5      Dead-field liveness analysis (PEAK)
5b2c9e1       Jul 5      A005e hybrid loop (REGRESSION)
```
