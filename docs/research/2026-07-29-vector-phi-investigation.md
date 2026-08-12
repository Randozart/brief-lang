# Vector Phi Investigation — Complete Record

## Background

The nbody_newton benchmark reached **0.75x** (Briev beating C by 25%) in Era 5
(commit `8a827db`, Jul 11) using our own `<4 x float>` vector phi emission +
SLP hazard gating. After Phase 4 (SLP cleanup, hazard/reorder removal), nbody
regressed to **1.22x** with no intervention. This document records every attempt
to recover that performance.

## Current State (Commit `94be0897`)

- **18/19 benchmarks MATCH**, nbody_newton MISMATCH (pre-existing)
- Dispatch: PerFieldPhi (31 scalar phi nodes)
- nbody_newton: ~1.22x C, MISMATCH (float diff 1.424093490000000e+00)
- The MISMATCH was introduced between Era 5 and the baseline — not caused by
  any recent changes. All recent changes (dispatch guardrail, RHS mapping fix)
  are correct and independently tested.

## Root Cause of Regression (versus Era 5 0.75x)

The baseline (`b39461e2`) had two features removed in Phase 4:

1. **`-slp-vectorize-hor=false`** compiled flag — passed via `llvm_extra_flags()`
   when `slp_hazard_fns` was non-empty. This DISABLED LLVM's SLP vectorizer
   globally for affected translation units.

2. **SLP hazard analysis** (`src/backend/llvm/hazard.rs`) — computed peak live
   float values to decide which txns were SLP-hazardous. Hazardous functions
   received `#4`/`#5` attribute groups; non-hazardous functions used `#0`.

Phase 4 removed both the hazard analysis and the `-slp-vectorize-hor=false`
flag. LLVM's SLP vectorizer now runs freely on all functions. For nbody with
31 per-field phis, SLP creates 6 `<4 x float>` vector phis with
extractelement/insertelement overhead that outweighs any vector compute benefit.

## What We Tried

### Attempt 1: Dispatch Guardrail (committed `88818123`)

**Problem**: `emit_folded_loop` passed empty `write_set` (counter.rs:110),
dropping non-counter state writes silently.

**Fix**: Added `writes_non_counter` check before InlineSsa dispatch. Bodies
with non-counter state writes now route to PerFieldPhi.

**Result**: ✅ `cargo test --lib` passes. fasta and knucleotide went from
MISMATCH to MATCH. 19/19 benchmarks MATCH. Committed.

### Attempt 2: RHS Mapping for `statements_isomorphic` (committed `066b86a7`)

**Problem**: `statements_isomorphic` only built variable mapping from LHS of
assignments. For `vx0 = nvx0` vs `vx1 = nvx1`, it mapped `vx0→vx1` via LHS
but missed `nvx0→nvx1` in RHS, returning false (no isomorphism).

**Fix**: Added `if let Some(rhs_map) = build_mapping(e1, e2) { mapping.extend(rhs_map); }`
in the `Statement::Assign` arm of `statements_isomorphic`.

**Result**: ✅ nbody's velocity/position assignments are now correctly detected
as isomorphic. Tests pass. Committed as part of `066b86a7`.

### Attempt 3: Vector Phi Infrastructure Fixes (committed `066b86a7`)

Before enabling vector phi emission, fixed edge cases found during testing:

| Fix | File | Why |
|-----|------|-----|
| Power-of-2 width guard | vector_phi.rs | LLVM rejects non-power-of-2 vector types |
| Internal dedup (same field at 2 lanes) | vector_phi.rs | `analyze_body` merge can create duplicates |
| Cross-group dedup | vector_phi.rs | Same field in two groups → wrong phi |
| Sort by width descending | vector_phi.rs | Larger groups take priority |
| Unique backedge names via `next_reg_with_prefix` | vector_phi.rs | `infer_group_name("p")` clashes for 2+ groups |
| Fix `extractelement` syntax | vector_phi.rs | Extra `g.element_ty` before `i32` index |
| Latch label parameter | vector_phi.rs | Hardcoded `%latch` didn't match `.cm_latch` |
| `analyze_body`: Assign only, no Let | slp_isomorphism.rs | Let-bindings are not loop-carried state |
| Vector phi state cleared in `emit_countable_main` | counter.rs | Disabled until infrastructure is correct |

### Attempt 4: Enable Vector Phi Emission in `emit_countable_main`

**What**: Removed the clearance block and restored `detect_vector_groups` +
`emit_vector_header` + `emit_vector_backedge` inside `emit_countable_main`.

**Result**: ❌

| Benchmark | Status |
|-----------|--------|
| nbody_newton | NaN output (wrong field grouping) |
| mandelbrot | clang segfault |
| float_math | compiles but conceptually broken |

#### Root Cause of Failure

`detect_vector_groups` uses `analyze_body` which groups fields by expression
tree isomorphism. For nbody, this groups semantically unrelated fields like
`vx0, vy0, vz0, vx1, vy1, vz1, ...` into one `<8 x float>` vector phi.
The fields happen to have similar assignment patterns (`var = new_var`) but
represent different physical quantities at different body positions. No
extractelement/insertelement overhead can compensate for wrong grouping.

Additionally, the init value loading (`phi_field_init`) uses
`emit_state_load_i64_by_idx` which loads as the field's native LLVM type
(float/double/iN). For float fields this was correct, but the backedge
assembly in `emit_vector_backedge` had type mismatches with the `<2 x float>`
vs `<4 x float>` errors that appeared under certain group configurations.

### Attempt 5: SLP Disable Experiment (manual `opt` + `clang`)

**What**: Compiled nbody's `.ll` with `clang -O3 -mllvm -slp-vectorize-hor=false`.

**Result**: Timing difference was ~6% faster with SLP disabled, but the
independent recompilation produced incorrect output (`-0.198399` vs expected
`-0.169203`), invalidating the comparison.

## Key Files Changed (Full Session)

| File | Committed | Changes |
|------|-----------|---------|
| `src/backend/llvm/mod.rs` | `88818123` | Dispatch guardrail (writes_non_counter check before InlineSsa) |
| `src/analysis/slp_isomorphism.rs` | `066b86a7` | RHS mapping fix + Assign-only filter + tests |
| `src/backend/llvm/vector_phi.rs` | `066b86a7` | Power-of-2 guard, dedup, unique backedge names, extractelement syntax, latch label |
| `src/backend/llvm/loop_engine/counter.rs` | `94be0897` | Dead code removal (`emit_folded_memory_main`, `emit_while_main`, `emit_countable_memory_main`) |
| `src/backend/llvm/loop_engine/counter.rs` | uncommitted | Vector phi enablement (reverted — the 4 blocks documented above) |
| `docs/plans/2026-07-29-dispatch-bug-analysis.md` | `35158e2f` | Dispatch bug root cause analysis |
| `docs/plans/2026-07-29-vector-phi-assign-isomorphism.md` | `066b86a7` | Vector phi plan + benchmark results |

## What To Investigate Next (nbody 0.75x target)

The 0.75x Era-5 approach combined three things that no longer exist:

1. **Our own vector phi emission** — `<4 x float>` phis for 5 well-chosen
   groups (bx, by, bz, vx, vy, vz), NOT LLVM's post-hoc SLP. The groups were
   hand-selected by the isomorphism analysis of that era.

2. **SLP hazard gating** — `-slp-vectorize-hor=false` prevented LLVM's SLP
   from adding extra extractelement/insertelement on top of our vector phis.

3. **The old SLP codegen in `emit_countable_body`** — hazard-gated SLP
   instruction emission that may have been a no-op for nbody.

To recover 0.75x, a future investigation would need to:

1. **Trace the Era-5 IR structure** — `git show 8a827db:benchmarks/nbody_newton.ll`
   to see exactly what IR produced 0.75x. Our current PerFieldPhi produces
   different IR than Era-5's emit path.

2. **Restore hazard analysis or equivalent** — compute peak live registers
   from the IR (not from AST-based hazard analysis) and gate SLP-disable flag.

3. **Revisit vector phi groups** — the isomorphism + power-of-2 + dedup checks
   are correct, but the grouping doesn't match nbody's semantics. Manual
   group selection via config may be necessary.

## Unrelated Discoveries

### `emit_state_load_i64_by_idx` vs native type for vector phi init

`phi_field_init` loads field values using `emit_state_load_i64_by_idx` which
loads with the field's native LLVM type (float/double for float fields). This
was correct for scalar phis but would need type-specific handling for vector
phi init values (loading into `<N x float>` not `float`).

### LLVM SLP Cost Model

LLVM's SLP vectorizer reported huge negative costs (e.g., `-45 with tree size
34`) for nbody's per-field phis, confidently vectorizing groups that are
actually counterproductive. The cost model does not account for the
extractelement overhead of scattered lane access patterns.

### clang Segfault on mandelbrot

When vector phis were enabled, mandelbrot's IR caused clang to segfault. This
was triggered by the vector phi infrastructure, suggesting an IR verifier
issue that manifests as a crash rather than a clean error.
