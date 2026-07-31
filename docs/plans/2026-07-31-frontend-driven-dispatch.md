# Frontend-Driven Dispatch — Replacing LLVM Backend Heuristics

**Date:** 2026-07-31
**Status:** Active implementation plan
**Branch:** `feat/frontend-driven-dispatch`
**Baseline commit:** `666fb502` (== `c2fe4402` compiler code; `c2fe4402` is doc-only)
**Baseline worktree:** `../brief-compiler-baseline` (detached HEAD at `666fb502`)
**Comparable with:** `bash benchmarks/compare_baseline.sh`

---

## 0. Executive Summary

The LLVM backend still contains several heuristic decision points — empirical
thresholds (`write_density >= 0.5 && total_fields < 8`, `total_fields > 14`,
`cross_per_field > 4.0`, `cases.len() <= 8`, `params < 8 && body < 20`) and
brittle name/pattern matching (`field_index_map.get("total")`,
`bp.var == inc.var`, hardcoded `box_op` type-name fallbacks) that approximate
structural facts the frontend analysis pipeline already proves or could prove.

This plan migrates every one of those decision points into principled
frontend analysis passes in `src/analysis/`, derives decisions from structural
facts to the greatest extent possible, and pushes the residual target-hardware
knowledge into `config/`. The backend becomes a deterministic switch over
frontend-computed shapes; the only tunables left are documented configuration
values with an audit trail.

The plan is grounded in the project's own history: a two-week sequence of
backend heuristic "gates" each fixed ~3 of 4 benchmark regressions and broke
the 4th. The fixes that survived are all structural (per-field phi,
version-DAG decomposition, minimal-state classification, static predicate
classification). This plan extends that pattern to the remaining backend
decision points.

---

## 1. Motivation: Why the Backend Still Hurts

The recurring failure mode is documented in `docs/plans/2026-07-29-full-recovery-plan.md` axiom 1:

> "LLVM optimizes better when given canonical IR, not pre-optimized IR. The
> frontend should emit clean structural patterns and get out of LLVM's way.
> Every attempt to 'out-smart' LLVM by reordering statements, gating SLP, or
> emitting shufflevector chains was proven counterproductive."

Three concrete pain points remain, each an LLVM surprise caused by backend
approximation rather than frontend proof:

1. **Register spilling from LLVM's auto-vectorizer** (the "kalman 3.5×
   regression"). The backend guesses "this txn is dense-matrix-like" by
   counting cross-field binary ops at codegen time
   (`emit_toplevel.rs:1825-1849`) to decide whether the txn function may keep
   the more-restrictive `#11 = memory(argmem: readwrite)` attribute. The
   threshold `cross_per_field > 4.0` is tuned on exactly two benchmarks
   (kalman 9.3 ops/field, nbody 1.7 ops/field). There is no unit test, the
   metric is computed once and thrown away, and it ignores the second
   parameter (`_all_idents` is unused — the metric may be incomplete).

2. **SROA/vectorizer obstacles from state-wide density math.** The loop
   dispatch picks between VectorPhiGroup / InlineSsa / PerFieldPhi using
   `write_density = write_count / total_fields` and `total_fields` counts
   (`mod.rs:2799-2861`). Post-Phase-7, the number of phi registers a loop
   needs is determined by the **loop-carried** field set
   (`analysis/loop_carried.rs`), not by the ratio of written fields to total
   fields. `total_fields > 14` is a proxy for "many loop-carried fields" —
   the actual fact is computable.

3. **Name-based reconstruction of analysis results.** The backend rebuilds the
   program's convergence predicate by matching `bp.var == inc.var` and
   constructing `Expr::Ge(counter, bound)` AST nodes (`mod.rs:2600-2639`), and
   looks up the "total" bound by field *name* with a `counter_idx + 1`
   fallback (`ssa.rs:183`). Both re-derive data the transition graph already
   computed — and both are fragile to field ordering and naming.

These are not "cleanup" items. Each has caused or is positioned to cause
benchmark regressions that get blamed on "noise" until someone runs a
controlled A/B. This plan removes the class of bug entirely.

---

## 2. Historical Research: The Heuristic Gate Zoo

This section documents the full evolutionary history of LLVM codegen methods
for the benchmarks. It is the empirical basis for the structural-derivation
principle in §4. All commit hashes verified against `git log`.

### 2.1 Hand-rolled SLP vector emission and its gate zoo (all removed)

The canonical example of backend heuristic whack-a-mole:

| Commit | Date | What | Outcome |
|--------|------|------|---------|
| `6fb88032` | 07-21 | SLP isomorphism analysis pass (`slp_isomorphism.rs`) | analysis only, 143 groups/473 lanes in nbody |
| `be62cb88` | 07-21 | Cross-pair merge + dependency tracing + emission | nbody 1.35× → 1.05× |
| `33d42397` | 07-27 | **Remove SLP vector emission**, relax `memory(readwrite)` | reversal begins |
| `e8f81eee` | 07-27 | Hazard-gated SLP + guard-condition fix | fixed 3 of 4 regressions |
| `ca467e20` | 07-27 | SLP profitability check + width cap | fixed 3 of 4 regressions again |
| `b39461e2` | 07-27 | **SLP stride gate** — all 19 at parity | became the reference baseline |
| `a53ddf14` | 07-28 | Restore stride gate (kalman protection) | rollback point |
| `ecf299c9` | 07-28 | **Remove stride gate** | stride gate "measured the wrong thing" |
| `edf671de` | 07-28 | Two-pass SLP consumer analysis (chain-cost) | "principled" kalman-vs-nbody gate |
| `e64d75ac` | 07-29 | Delete `hazard.rs`, `reorder.rs`, `vector_codegen.rs`, `optimizer.rs` → `strategy.rs` | hand-rolled SLP fully removed |

**Failure modes (each iteration fixed ~3 of 4 regressions and broke the 4th):**

- **Stride gate:** helped nbody (stride-1 groups) but hurt kalman (stride-3
  matrix mul `p00,p10,p20`). `ecf299c9` revealed the gate was "measuring the
  wrong thing" — insert-chain cost is identical whether stride is 1 or 100.
- **Total-gap check** (`total_gap < 10` = matrix → block): kalman 3.6× and
  nbody 1.35× — both failed.
- **Depth×width ≥ 10:** blocked both (kalman 9<10, nbody 3<10) — "neither
  benefits."
- **Dense-matrix LLVM auto-vectorizer interference:** kalman's 3.5× regression
  came from **LLVM's own** `<12 x float>` vectors (from `#11` after
  `alwaysinline` inlined `@txn_propagate`), not from our SLP. The fix was
  forcing `#0 = memory(readwrite)` when cross-per-field density > 8
  (`docs/plans/2026-07-28-slp-gate-refinement.md`).
- **Final verdict** (`e64d75ac` + recovery-plan axiom 1): let LLVM
  auto-vectorize with correct alias info; hand-rolled shufflevector chains are
  counterproductive.

### 2.2 Batch-loop optimization (added 07-29, removed 07-31)

- **Added:** `12e5435f` (2026-07-29).
- **Removed:** `81eea6aa` (2026-07-31, Phase 6, −681 lines incl. loop_peeling).
- **Purpose:** split a composite convergence node into an inner pure-compute
  loop + outer boundary-guard loop so LLVM can if-convert the inner loop.
- **Failure modes:**
  - **Correctness:** knucleotide + mandelbrot produced wrong output — the
    `count == 0` periodic print was never emitted. Fixes accumulated over four
    commits (`aa174b14` "only hoist self-contained guards", `c4cec5d9`
    "process let_to_field + Block expr", `f9d994ff` dominance fix,
    `7e9de00b` "emit hoisted outer guards + pending_post_hoist").
  - **Fragility:** `split_hoistable` stripped `let distXX = Sqrt#(dsqXX)`
    bindings even when **no** batch loop was created, leaving undefined
    globals (`@dist01 undefined in IR`) — fixed at `7e9de00b`.
- **Root cause:** the boundary guard is semantically part of the composite
  node; splitting it required re-deriving "batch size" via heuristics
  (`extract_batch_size_from_guards`, `is_safe_to_hoist`, `let_to_field`
  remapping, `count=0` peel). Every heuristic was a special case.

### 2.3 Automatic loop peeling (added 07-29, deleted 07-31)

- **Manual peel** (diagnostic, `benchmarks/nbody_newton_peeled.bv`) proved the
  concept: removing the `when count % 5000000 == 0 { PrintLn!(energy) }` guard
  improved nbody_newton 1.22× → 0.83×.
- **Automatic version failed** because of an LLVM if-conversion blocker:
  "control flow cannot be substituted for a select" when the guarded block
  contains an opaque `PrintInt#` call.
- **Superseded by version-DAG** (`emit_version_dag_main`), which derives the
  structure from the guard predicate itself, not from "peel the body N times"
  heuristics.

### 2.4 Dispatch heuristics removed in Phase 4 (`34c33b4f`, 2026-07-29, −178 lines)

- `write_density >= 0.8 && total_fields >= 8 → emit_folded_memory_main` — removed.
- `phi_cap` / `capped_set` — hardcoded cap 6 (`b153ecfc` 07-21, adaptive
  6–10), then removed: "always use full write_set."
- `has_body_ffi → while-loop dispatch` — `statement_contains_ffi` was buggy and
  never fired; concept removed. `emit_while_main` deleted as dead code.
- `peak_live_floats` dispatch — reverted at `d2778153` ("while-loop worse than
  per-field phi").
- Memory-loop variants (`emit_folded_memory_main`, `emit_countable_memory_main`,
  `emit_while_main`) — dead-code removed.
- `alwaysinline` on reactive txns — removed at `2f3d5752` (ring_buffer
  1.16× → 1.06×): alwaysinline consumed the `#11` attribute, blocking SROA.

### 2.5 Vector-phi early experiments (all rejected, E-series)

Documented in `docs/plans/2026-07-29-frontend-ir-quality-improvements.md`
Appendix E:

- **Pure phi-capping** (12 of 31 phis): 1.43× — GEP+load memory round-trips
  are more expensive than register spills (spills happen at loop edges; GEP+load
  is in the critical path).
- **Capping + `<2 x float>` phis**: 1.52× — extractelement overhead dwarfs the
  register benefit.
- **Extractelement caching**: 1.51× — only fixes half (backedge insertelement
  remains).
- **`<8 x float>` phis**: 1.48× — AVX lane-crossing latency (2-3 cycles vs SSE 1).
- **`!invariant.load` on capped fields**: **MISMATCH** — semantic contract
  violation (fields are written every iteration).
- **Naming-based grouping**: superseded by the principled AoS→SoA independence
  analysis (`soa_reorder.rs`).

### 2.6 The techniques that survived (all structural)

| Technique | Location | Why it survived |
|-----------|----------|-----------------|
| Per-field phi loop | `counter.rs::emit_countable_main` | Canonical `phi + icmp slt + add`; SROA/IV/vectorizer recognize it |
| Chunk allocas | `emit_stmt.rs` `MAX_FIELDS_PER_ALLLOCA=15` | Decomposes %State for SROA |
| version-DAG | `counter.rs::emit_version_dag_main` | Boundary derived from guard predicate at the split point |
| Minimal-state | `analysis/loop_carried.rs` | Loop-carried set = exact phi set; zero %State traffic in hot loop |
| Static predicate | `analysis/node_decompose.rs` | AlwaysTrue inline / AlwaysFalse drop / Runtime two versions |
| Pure counter fold | `counter.rs::emit_folded_pure_counter` | Pure body + constant bound → O(1) store |
| Transition-graph dispatch | `analysis/transition_graph.rs` | bounded_pre + increments; "decision driven by the transition graph, not runtime profiling" (mod.rs:2660-2661) |
| Unconditional conflict detection | `74ec03a2` Phase 1 | writing is a XOR condition; deny on race |
| Brief-level LICM | `analysis/licm.rs` | hoists loop-invariant let-bindings before codegen |
| SoA reorder | `analysis/soa_reorder.rs` | proves field independence, then assigns consecutive indices |
| Float constant emission | `3371f985` | direct `bitcast i32 <hex> to float` |

### 2.7 Regression-trap patterns (recurring root causes)

1. **Every SLP gate fixed ~3 of 4 regressions and broke the 4th.** Heuristic
   gates that approximate an effect they don't model directly produce wrong
   gates.
2. **Attributes silently change LLVM behavior downstream.** alwaysinline
   consumed `#11`, unblocking LLVM's own kalman auto-vectorizer.
3. **Naming-based grouping is fragile.** `bx0..bx4 → group "bx"` only works
   for `prefixNNN` naming. Replaced by independence *proof* + isomorphism.
4. **Phi-capping / memory round-trips cost more than spills.**
5. **Metadata contracts must match actual mutability** (`!invariant.load` on
   written-every-iteration fields = MISMATCH).
6. **Empty write_set bug** (counter.rs:110-111): a seemingly benign default
   silently dropped all non-counter writes — caught only when fasta/knucleotide
   went MISMATCH. Root-caused at `88818123`; the `writes_non_counter` guardrail
   is now a correctness requirement.
7. **HashMap iteration order silently changes GEP offsets** → 29 extra hot-loop
   instructions (nbody_sqrt root cause, `docs/research/nbody-regression-root-cause.md`).

---

## 3. Current-State Assessment (as of `666fb502`)

### 3.1 The loop dispatch today (`src/backend/llvm/mod.rs:2641-2861`)

```
EmitPureCounterFold     — pure body + constant bound → O(1) store
  else if version-DAG   — emit_version_dag_main handles a single runtime guard
  else 3-way structural:
    1. VectorPhiGroup    — isomorphic ≥4-member groups AND total_fields > 14
    2. InlineSsa         — write_density >= 0.5 AND total_fields < 8
                          AND !writes_non_counter (correctness guardrail)
    3. PerFieldPhi       — default, full write_set, no phi_cap
```

The inputs (`write_set`, `bounded_pre`, `increments`, `is_pure_body`,
`is_effectively_pure`) come from the transition graph — principled. The
**thresholds** (`0.5`, `8`, `14`, `>= 4`) are empirical. The comment at
`mod.rs:2752-2756` explicitly documents that a prior generation of these
exact numbers was removed as counterproductive.

### 3.2 The analysis pipeline feeding the backends

`src/backend/mod.rs::analyze_program` produces `AnalysisResults`:

```rust
pub struct AnalysisResults {
    pub call_graph: CallGraph,
    pub param_ranges: ParameterRanges,
    pub fusable_pairs: Vec<(String, String)>,
    pub dataflow_errors: Vec<DataflowError>,
    pub optimize_mode: bool,
    pub transition_graph: ReactorTransitionGraph,
    pub region_analyzer: RegionAnalyzer,
    pub dependency_graph: DependencyGraph,
}
```

Existing frontend analysis that this plan builds on:

| Analysis | Provides | Consumed for |
|----------|----------|--------------|
| `transition_graph.rs` | `bounded_pre {var, bound_var, bound_literal, direction}`, `increments`, `write_set`, `is_pure_body`, `is_effectively_pure` | foldable classification, dispatch |
| `loop_carried.rs` | `FieldClass::{LoopInvariant, LoopCarried, Dead}` per field | minimal-state phi emission |
| `node_decompose.rs` | `Segment::{Compute, Guard}`, `PredicateClass::{AlwaysTrue, AlwaysFalse, Runtime}` | version-DAG |
| `match_normalize.rs` | statement-level `match` → `when` normalization | uniform guards |
| `slp_isomorphism.rs` | structurally isomorphic field-group candidates | vector-phi detection |
| `licm.rs` | hoisted loop-invariant let-bindings | preheader hoist |
| `region.rs` | `iteration_bound_of`, FFI/trigger detection | `iter_bounds`, `!prof` |
| `soa_reorder.rs` | field independence proof | field index ordering |
| `frgn_dispatch.rs` | pre-resolved frgn strategies | call lowering |

---

## 4. The Structural-Derivation Principle

**A compiler is a deterministic tool. Every dispatch decision is derived from
structural facts the frontend already proves. Where a decision depends on
target hardware or representation layout — facts the compiler cannot derive —
it goes into `config/`. No magic constants in codegen.**

The three questions from AGENTS.md ("Intrinsics vs Stdlib") adapted to
analysis:

1. **Is this decision a fact about the program, or a tuned constant?**
   "This txn writes only its counter" is a fact. "InlineSsa is good when
   write_density ≥ 0.5" is a tuned guess. Facts are computed; guesses are
   either eliminated or made explicit configuration.
2. **Does the decision generalize, or is it special-casing one pattern?**
   `writes_non_counter` was added because `emit_folded_loop` silently drops
   writes — a latent bug special-cased into a guardrail. The correct fix is to
   make "counter-only writes" the *definition* of the InlineSsa path.
3. **If this were the only rule left, would the architecture still hold?**
   A `LoopShape` computed from the transition graph and consumed by a backend
   switch works for any program and any backend. A `write_density` threshold
   only works for the two benchmarks it was tuned on.

---

## 5. Candidate Inventory (full audit of backend heuristics)

Legend: **F/A** = replaceable by frontend analysis; **CFG** = belongs in
config; **LLVM** = LLVM-semantics constraint; **N/A** = correctness.

### 5.1 Dispatch / structural codegen (Tier A)

| # | Site | Rule | Type |
|---|------|------|------|
| A1 | `mod.rs:2799-2861` | `write_density >= 0.5 && total_fields < 8` → InlineSsa; `total_fields > 14` + vector-phi groups → VectorPhiGroup; else PerFieldPhi | F/A |
| A2 | `emit_toplevel.rs:1820-1849` | `#11 → #0` when `n > 4 && cross_per_field > 4.0` | F/A (measurement) + CFG (threshold) |
| A3 | `ssa.rs:27-117` | modulo-partition detection; `cases.len() <= 8` rotated-vs-switch | F/A |
| A4 | `mod.rs:2600-2639` | synthetic exit-condition AST construction (`bp.var == inc.var`) | F/A |
| A5 | `mod.rs:136-204` | `hoist_terminating_guard` AST transform in backend | F/A |
| A6 | `mod.rs:2700-2709` | `has_swan_song` re-walk of body | F/A |

### 5.2 Measurement passes (Tier B)

| # | Site | Rule | Type |
|---|------|------|------|
| B1 | `emit_toplevel.rs:2194-2204` | callable-txn auto-inline `params < 8 && body < 20 && !ffi` | F/A + CFG |
| B2 | `emit_toplevel.rs:1886-2008` | `!prof` weight cap `max_w = 1000`; modulo-arithmetic fallback | LLVM (i32 range) — keep principled path, move cap to config |
| B3 | `dispatch.rs:68-73, 357-362` | reactor `#2`/`#12` by re-walking bodies for unguarded FFI | F/A |

### 5.3 Config knowledge (Tier C)

| # | Site | Rule | Type |
|---|------|------|------|
| C1 | `context.rs:201-215` | `float_register_count` triple-prefix match, "else → 16" | CFG |
| C2 | `dispatch.rs:297-313` | write-mask `idx < 64` silently drops fields | F/A (derive width) |
| C3 | `mod.rs:1389-1398`, `context.rs:292-293` | arena budget `< 128`, `arena_initial_size 65536`, `stack_threshold 4096` | CFG |
| C4 | `emit_stmt.rs:11`, `emit_expr.rs:1017, 386` | `MAX_FIELDS_PER_ALLLOCA=15`, SSO `<= 6` bytes, SVO `<= 3` | CFG (+SSO derivable from alignment/tag bits) |
| C5 | `emit_toplevel.rs:1347-1356` | `type_driven_range` byte→range mapping | CFG |

### 5.4 Rule 18 violations — hardcoded Brief type-name matching (Tier D)

AGENTS.md Rule 18: never match Brief type names in Rust. Verified sites:

| # | Site | Names matched |
|---|------|---------------|
| D1 | `emit_toplevel.rs:2243-2250, 1238-1245` | `Bool`, `Char`, `String`, `Data`, `Float` — comment 1244: "box_op removed from ResolvedType — use hardcoded fallback" (documented violation) |
| D2 | `builder.rs:546-613` | `Bool`, `String`, `Data`, `Float`, `Float64`, `Int32`, `UInt32` |
| D3 | `mod.rs:115-132` `primitive_from_name` | `Int`, `UInt`, `Float`, `Bool`, `Char`, `String`, `Data`, ... |
| D4 | `mod.rs:74` `try_eval_cfloat` | `Float`, `Float64` |
| D5 | `mod.rs:2338` store alignment | `Bool`, `Int`, `UInt`, `Char`, `String`, `Data` |
| D6 | `mod.rs:550, 577` TBAA | `g == "Int"` sort tiebreak |
| D7 | `emit_toplevel.rs:308, 322, 572-593` | `String`, `Data`, `Bool`, `Float`, ... |
| D8 | `emit_expr.rs:1525` | `String`, `Data` |
| D9 | `abi.rs:83-84` | `Bool` |
| D10 | `helpers.rs:598, 1626` | `Float` |
| D11 | `ssa.rs:183` | `field_index_map.get("total").unwrap_or(counter_idx + 1)` — hardcoded field NAME |

Permitted-by-exception string matches (LLVM IR type strings, Rule 18(c)):
`tbaa_node`, `mod.rs:2329` (`store_ty == "i64"`), `loop_engine/mod.rs:308/323`,
`emit_stmt.rs:187-258`, `emit_expr.rs:567-601, 1770-1774`. These are kept.

### 5.5 Dead code / latent bugs found during the audit

| # | Site | Issue |
|---|------|-------|
| E1 | `loop_engine/mod.rs:304-380` | `pre_extract_float_fields`, `pre_extract_int_fields`, `pre_load_all_fields` — defined, zero callers |
| E2 | `emit_expr.rs:860` | SVO packed header: `(len << 32) | (cap << 32) | 1` — `len` and `cap` overlap; likely latent bug |
| E3 | `ssa.rs:512-513` | `emit_post_print` ignores its `_ty: &str` parameter |
| E4 | `emit_toplevel.rs:919-926` | ringbuf-init detection stubbed always-false after `insert_at` removal |
| E5 | `emit_toplevel.rs:2587-2614` | `br i1 true` with unreachable `rollback:` block always emitted |
| E6 | `normalizer.rs:159-215` | `.unwrap_or(64)`, `.unwrap_or(8)`, `bytes.min(8)` silent size defaults |

---

## 6. Phase 1 — `LoopShape` Analysis (the flagship)

### 6.1 New frontend pass: `src/analysis/loop_shape.rs`

Backend-agnostic. Computes, per foldable bounded-counter txn, a structured
shape the backend switches on. All inputs are existing frontend results.

```rust
/// The structural shape of a foldable bounded-counter transaction.
pub struct LoopShape {
    pub txn_name: String,
    pub counter: String,          // from bounded_pre.var
    pub bound: Bound,             // field idx | const name | literal
    pub direction: ConvergeDirection,
    pub counter_only_writes: bool, // write_set == {counter}
    pub carried_fields: Vec<String>, // loop_carried classification order
    pub vector_groups: Vec<VectorGroup>, // isomorphism + type + power-of-2
    pub has_swan_song: bool,      // hoist_swan_song result
    pub is_pure: bool,            // is_pure_body || is_effectively_pure
    pub convergence: Convergence, // structured, NOT backend-synthesized Expr
}

pub enum Convergence {
    /// counter >= bound (field/const/literal)
    CounterGeBound { counter: String, bound: Bound },
    /// Explicit exit condition from the program (#!exit)
    Explicit(Expr),
    /// Not provable — conservative reactor loop
    Unprovable,
}
```

| Shape field | Source | Replaces |
|---|---|---|
| `counter`, `bound`, `direction` | `transition_graph::bounded_pre` + `increments` | `mod.rs:2684-2697` re-derivation |
| `counter_only_writes` | `write_set == {counter}` | `write_density >= 0.5 && total_fields < 8` |
| `carried_fields` | `loop_carried::classify_fields` | `total_fields` proxy |
| `vector_groups` | `slp_isomorphism` + same-type + power-of-2 + carried | backend `detect_vector_groups` re-run |
| `has_swan_song` | swan-song hoist | body re-walk `mod.rs:2700-2709` |
| `convergence` | `bounded_pre.bound_var` | synthetic `Expr::Ge` construction |

### 6.2 Structural derivation of the old thresholds

- **InlineSsa** ⟺ `counter_only_writes && !has_swan_song`. This is the
  *definition* of the path — `emit_folded_loop` only emits counter writes
  correctly. The `writes_non_counter` guardrail (`mod.rs:2841-2850`) was a
  correctness patch that already encoded this; it becomes the rule.
- **VectorPhiGroup** ⟺ isomorphic groups exist **and**
  `carried_fields.len() > ctx.float_register_count()` (target fact from config,
  §8.1). Eliminates `total_fields > 14`. The `width >= 4` gate is derived as a
  cost-model break-even: `saved_registers = width - 1` vs
  `overhead ≈ 2·width` (extractelement per read + insertelement per write),
  break-even at `width >= 4`. Keep the derived constant, documented, not a
  magic number.
- **PerFieldPhi** = structural default (any shape that isn't a pure fold,
  version-DAG, InlineSsa, or vector-phi).
- **Pure counter fold** stays as-is: `is_pure && const bound`.

### 6.3 Move `hoist_terminating_guard` into the frontend

`src/backend/llvm/mod.rs:136-204` → `src/analysis/swan_song.rs`:

```rust
pub fn hoist_swan_song(
    body: &[Statement],
    field_index_map: &HashMap<String, usize>,
) -> (Vec<Statement>, Vec<Vec<Statement>>)
```

Pure AST transform (terminating-guard detection + `let_to_field` remap +
`remap_stmt_identifiers`/`remap_expr_into`). Unit-testable in isolation,
reusable by webstack/circt. The backend consumes the result via
`AnalysisResults` and drops the backend-local copy (and its helpers).

### 6.4 Replace synthetic exit-condition construction

`mod.rs:2600-2639` → `loop_shape::convergence(graph, txns) -> Option<Convergence>`.
The backend no longer builds `Expr::Ge(counter, bound)` nodes or matches
`bp.var == inc.var`; it consumes `Convergence`. The `has_persistent_txn`
re-derivation also moves into the analysis.

### 6.5 Backend dispatch collapse

`mod.rs:2641-2861` becomes a deterministic switch:

```
match loop_shape {
  Pure && const bound      => EmitPureCounterFold
  single runtime guard     => emit_version_dag_main
  counter_only && !swan    => emit_folded_main (InlineSsa)
  vector groups && carried > regs => emit_countable_main (VectorPhiGroup)
  _                        => emit_countable_main (PerFieldPhi)
}
```

`emit_countable_main` / `emit_folded_main` / `emit_version_dag_main` are
unchanged — they already consume LoopShape-equivalent inputs.

### 6.6 `AnalysisResults` extension

```rust
pub struct AnalysisResults {
    // ... existing ...
    pub loop_shapes: HashMap<String, LoopShape>,
    pub swan_songs: HashMap<String, (Vec<Statement>, Vec<Vec<Statement>>)>,
}
```

### 6.7 Tests (behavioral, Plan Directive 5)

- `loop_shape`: counter/bound/direction extraction from a sample transition
  graph; `counter_only_writes` true for `count = count + 1`-only bodies and
  false when a second field is written; carried-field ordering deterministic.
- `swan_song`: hoist `when done { term! -> print(acc) }` with `let_to_field`
  remap (mandelbrot `nesc` pattern); hoist even when body is empty
  (`term! -> print_int#(result)` only); non-terminating trailing guard is not
  hoisted.
- `convergence`: `[count < N][count == N]` with `count = count + 1` →
  `CounterGeBound{count, N}`; explicit `#!exit` preserved; unprovable → `Unprovable`.

---

## 7. Phase 2 — Measurement Passes (backend-agnostic)

### 7.1 Density (`src/analysis/density.rs`)

Per-txn measurement, computed once:

```rust
pub struct ComputeDensity {
    /// Distinct float let-bindings referenced by body arithmetic.
    pub float_idents: usize,
    /// BinaryOps whose operands both reference identifiers.
    pub cross_ops: u32,
    /// cross_ops / float_idents (NaN-safe: 0 when float_idents == 0).
    pub per_field: f64,
}
```

Fixes the current metric's gap: `count_cross_float_ops_in_expr`
(`emit_toplevel.rs:1557`) ignores its `_all_idents` parameter; the analysis
version counts cross-field ops over the actual loop-carried float set. The
`#11 → #0` downgrade at `emit_toplevel.rs:1825-1849` reads the measurement;
the threshold `> 4.0` moves to config (§8.1 `dense_compute_density`).

### 7.2 Modulo partition (`src/analysis/modulo_partition.rs`)

Detects "every reactive txn precondition is `count % K == N` for a common K"
structurally:

```rust
pub struct ModuloPartition {
    pub counter: String,
    pub divisor: i64,
    pub cases: Vec<(i64, String)>, // (residue, txn name)
}
```

Replaces `extract_mod_info` / `extract_mod_guard` (`ssa.rs:68-117`).

**Choice derived structurally, not `cases.len() <= 8`:**
- Use the **rotated loop** whenever the txn set has `bounded_pre` — it is the
  only form that handles a bounded counter (this is the semantic that
  `emit_modulo_switch_main` was missing, per the comment at `ssa.rs:55-57`).
- Use the one-shot switch only when analysis proves each case is
  self-terminating (region analysis), regardless of K.
- Fix `ssa.rs:183` `field_index_map.get("total")` → use the shape's
  `bound` (bounded_pre.bound_var), removing the `counter_idx + 1` assumption.

### 7.3 Inline cost (`src/analysis/inline_cost.rs` or `call_graph.rs`)

Replaces `params < 8 && body < 20` (`emit_toplevel.rs:2196-2204`):

```rust
pub fn callable_inline_decision(txn: &Transaction) -> InlineDecision {
    // weighted instruction count over the body (call=10, binop=1, load/store=2)
    // + !has_ffi_or_trigger_stmt_in_chain
    // threshold from config/ir-lowering.toml `callable_inline_weight_threshold`
}
```

### 7.4 Reactor attribute (`dispatch.rs:68-73, 357-362`)

`transition_graph` gains `has_unguarded_ffi: HashSet<String>` (per-txn,
computed once via the existing `statement_contains_ffi`, treating
`Statement::Guarded` as guarded). `dispatch.rs` consumes the set instead of
re-walking bodies twice.

### 7.5 `AnalysisResults` extension (Phase 2)

```rust
pub density: HashMap<String, ComputeDensity>,
pub modulo_partition: Option<ModuloPartition>,
pub has_unguarded_ffi: HashSet<String>,
pub inline_decisions: HashMap<String, InlineDecision>,
```

---

## 8. Phase 3 — Config Migration + Rule 18 Cleanup

### 8.1 `config/targets.toml` — per-target section

New `[target.<triple-prefix>]` sub-tables (absent → current fallback + a
compiler warning so silent x86 assumptions never apply to unknown targets):

| Key | Replaces | Default (x86_64) |
|-----|----------|-------------------|
| `float_registers` | `context.rs:201-215` triple-prefix match | 16 |
| `dense_compute_density` | `emit_toplevel.rs:1845` `cross_per_field > 4.0` | 4.0 |
| `callable_inline_weight` | `emit_toplevel.rs:2196-2204` body-size heuristic | e.g. 40 |
| `vector_min_width` | `vector_phi.rs` `width >= 4` | 4 |

`CompilerContext::float_register_count()` becomes
`load_target_config().float_registers`, with the prefix table in config.

### 8.2 `config/ir-lowering.toml` (new)

| Key | Replaces | Default |
|-----|----------|---------|
| `arena_min_budget` | `mod.rs:1389-1398` `budget < 128` | 128 |
| `arena_initial_size` | `context.rs:292` | 65536 |
| `stack_threshold` | `context.rs:293` | 4096 |
| `max_fields_per_alloca` | `emit_stmt.rs:11` | 15 |
| `sso_max_bytes` | `emit_expr.rs:1017` | 6 |
| `svo_max_elements` | `emit_expr.rs:386` | 3 |

**SSO derivation (replace literal, keep in config as an override):** the 6-byte
limit derives from the String handle representation — `align 8`, 2 tag bits →
`8 - 2 = 6` payload bytes. Document the derivation at the config key.

### 8.3 Derived (not config)

- **Write-mask width** (`dispatch.rs:297-313`): `u128` when
  `field_index_map.len() > 64` else `u64`. No silent drop of 65th+ field.
- **`!prof` `max_w`** (`emit_toplevel.rs:1901`): it is i32-range
  normalization, not a tunable. Keep the scaling but derive the cap from a
  ratio-preserving power of two near `i32::MAX / 2` and document why. The
  primary transition-graph `!prof` path is already principled and is kept.
- **`type_driven_range`**: keep byte→range as a representation fact, but read
  `bytes` from the universe/`int_bits`, not a hardcoded table.

### 8.4 Rule 18 → casting graph

Replace each D1–D11 site with casting-graph/universe resolution:
- D1/D2 (`box_op` fallbacks) → `llvm_type(ty)` + casting-graph boxed-path
  resolution. These are the highest-priority because the comments
  (`emit_toplevel.rs:1244`) explicitly document the regression to name-matching.
- D3 `primitive_from_name` → universe primordial registry lookup.
- D4 `try_eval_cfloat` → `is_protocol_member(ty, "#Float")` via casting graph.
- D5 store alignment → derive from `llvm_type`/alignment of the resolved type.
- D6 TBAA tiebreak → deterministic sort by protocol membership, not `"Int"`.
- D7/D8/D9/D10 → `is_protocol_member` / `resolve_llvm_type`.
- D11 `ssa.rs:183` `"total"` → `LoopShape.bound` (Phase 1 dependency).

### 8.5 Dead code and latent-bug fixes

- E1: delete `pre_extract_float_fields`, `pre_extract_int_fields`,
  `pre_load_all_fields` (`loop_engine/mod.rs:304-380`).
- E2: fix SVO packed header in `emit_expr.rs:860` — `len` and `cap` must not
  overlap. Determine the correct field packing from the SVO layout and add a
  test asserting the round-trip.
- E3: `emit_post_print` — honor `_ty` or remove the parameter.
- E4: either re-enable ringbuf-init detection via `insert_at`-equivalent
  metadata or delete the dead branch.
- E5: remove the unconditional `br i1 true` + unreachable `rollback:` block in
  `emit_shape_guarded_body`.
- E6: `normalizer.rs` silent size defaults — return `Result`/diagnostic instead
  of silent `unwrap_or(64)`/`unwrap_or(8)`.

---

## 9. Verification & Benchmark Methodology

### 9.1 Per-phase gates

1. `cargo test --lib` — all tests pass (Golden Rule 8).
2. `cargo build` — no warnings.
3. Praetor on new/changed files (complexity ≤ 15, lines ≤ 100, params ≤ 6).
4. For dispatch-changing phases: benchmark A/B vs `../brief-compiler-baseline`
   with `bash benchmarks/compare_baseline.sh`, and record results in this
   document's results table (§11).
5. No IR determinism regressions (HashMap iteration sorted before emission —
   AGENTS.md Rule 7; existing determinism tests).

### 9.2 Benchmark set (dispatch-sensitive)

`nbody_newton`, `nbody_sqrt`, `nbody_sqrt_idio`, `kalman_filter_runtime`,
`ring_buffer`, `float_math`, `float_math_nonzero`, `sparse_dispatch`,
`fannkuch_redux`, `knucleotide`, `mandelbrot`, `fasta`, `print_loop`,
`bit_clear`, `queue_drain`, `cancel_math`, `interval_step`.

- `--runtime` for throughput (FFI in hot loop).
- `--correctness` always — a phase is NOT complete on zero MISMATCH.
- `--optimizer` for the const-input folding set.

### 9.3 Baseline (Golden Rule 11)

**Baseline taken from `benchmarks/results/2026-07-31-version-dag-666fb502.md`
(commit `666fb502`, == compiler code at `c2fe4402`):**

`bash benchmarks/build_and_bench.sh --runtime`, 5 iterations, BOUND=50000000:

| Benchmark | Ratio | Winner | Correct |
|-----------|:-----:|:------:|:-------:|
| ring_buffer | 1.15× | C | MATCH |
| float_math | 0.97× | Brief | MATCH |
| float_math_nonzero | 1.21× | C | MATCH |
| sparse_dispatch | 0.83× | Brief | MATCH |
| print_loop | 1.03× | C | MATCH |
| nbody_newton | 0.83× | Brief | MATCH |
| nbody_sqrt | 0.77× | Brief | MATCH |
| nbody_sqrt_idio | 0.75× | Brief | MATCH |
| fasta | 1.00× | ~tie | MATCH |
| fannkuch_redux | 0.98× | Brief | MATCH |
| mandelbrot | 1.03× | ~tie | MATCH |
| kalman_filter_runtime | 1.23× | C | MATCH |
| knucleotide | 0.98× | Brief | MATCH |
| cancel_math | 0.86× | Brief | MATCH |
| bit_clear | 0.66× | Brief | MATCH |
| queue_drain | 0.96× | Brief | MATCH |
| queue_drain_sym | 0.89× | Brief | MATCH |
| queue_drain_idio | 0.90× | Brief | MATCH |
| interval_step | 1.00× | ~tie | MATCH |

**Zero MISMATCH.** (Ratio < 1 = Brief faster.)

### 9.4 Anti-regression rule

Any benchmark whose ratio regresses > 0.05× vs baseline **must** be root-caused
to a specific commit before the phase is committed (AGENTS.md: never blame
"noise"). Use `bash benchmarks/compare_baseline.sh` for the controlled A/B.
If a Phase 1 structural dispatch decision regresses a benchmark, the fix is a
better structural derivation, not a resurrected threshold.

---

## 10. Documentation Requirements (Golden Rule 12, Plan Directive 4)

Updated in the same commit as the code:

| Doc | Update |
|-----|--------|
| `docs/architecture/backend-architecture.md` | dispatch chain now `LoopShape`-driven; document the 5-way switch |
| `docs/architecture/minimal-state-and-purity.md` | Phase 7 wiring completed; LoopShape consumes carried set |
| `docs/architecture/backend-type-dispatch.md` | Rule 18 items migrated to casting graph |
| `docs/architecture/backend-guide.md` (or `8f3bf891` guide) | fragile-strategies section extended: "no backend thresholds" |
| `BUGS.md` | SVO packed-header bug (E2), write-mask >64 fields (C2) |
| `config/` comments | provenance on every new key (`// 2026-07-31: <why>`) |
| `spec/` | only if dispatch changes observable language behavior (it does not) |

**Rationale comments** (Plan Directive 2): every modified site carries
`// 2026-07-31: <why>` with undo path. Never delete existing rationale
comments — rewrite them for the new structure.

---

## 11. Results Log (filled in per phase)

| Phase | Commit | cargo test | nbody_newton | kalman | ring_buffer | float_math | sparse_dispatch | MISMATCH? |
|-------|--------|-----------|--------------|--------|-------------|------------|-----------------|-----------|
| Baseline 666fb502 | — | pass | 0.83× | 1.23× | 1.15× | 0.97× | 0.83× | 0 |
| Phase 0 (baseline capture) | ed2f4234 | pass | 0.83× | 1.22× | 1.13× | 0.97× | 0.84× | 0 |
| Phase 1a (analysis) | 0682d764 | 1232 pass | — | — | — | — | — | 0 |
| Phase 1b (dispatch) | c953c3c4 | 1239 pass | 0.83× | 1.24× | 1.10× | 0.95× | 0.82× | 0 |
| Phase 2 | (next commit) | 1259 pass | 0.83× | 1.23× | 1.18× | 0.97× | 0.86× | 0 |
| Phase 3 | | | | | | | | |
| Final | | | | | | | | |

Full `--runtime` table appended to this section after every phase.

Phase 1b full table (see `benchmarks/results/2026-07-31-frontend-dispatch-phase1b.md`):
all 19 runtime benchmarks within noise of Phase 0 (max delta 0.07×, queue_drain
0.86→0.93×), zero MISMATCH, and the per-benchmark dispatch decision (`.cm_header`
/ `.vdN_header` / bare-`main` markers) is byte-identical to Phase 0 for all 17
programs. All six Phase 1b regression tests added per §6.7. `cargo test --lib`:
1239 passed, 0 failed.

Phase 2 full table (see `benchmarks/results/2026-07-31-frontend-dispatch-phase2.md`):
all 19 runtime benchmarks within noise of Phase 1b (max delta 0.08×, ring_buffer
1.10→1.18×, queue_drain 0.93→0.85×), zero MISMATCH. The per-txn memory attribute
(`#11`/`#0`) and the main dispatch marker are byte-identical to the Phase 1b
reference compiler for all 38 benchmark programs, and the emitted CODE (excluding
the `declare` block) is byte-identical for the sensitive set — so the ring_buffer
and queue_drain deltas are pure run-to-run noise, not codegen changes. `cargo test
--lib`: 1259 passed, 0 failed. Phase 2 also fixed a latent IR determinism bug:
the frgn `declare` block was iterating `frgn_map` (a HashMap) unsorted, producing
run-to-run nondeterministic declaration order (Coding Standard 7); the loop now
sorts by key.

---

## 12. Commit Sequence

1. Plan doc + baseline table (this document) — commit on `main`.
2. Create `feat/frontend-driven-dispatch` worktree from `main`.
3. **Phase 1a:** `analysis/swan_song.rs` + `analysis/loop_shape.rs` +
   `AnalysisResults` fields + tests. Commit.
4. **Phase 1b:** backend dispatch collapse to `LoopShape` switch; delete
   backend `hoist_terminating_guard` + synthetic exit-condition. Full test +
   benchmark A/B. Commit with results.
5. **Phase 2:** density / modulo / inline / reactor-attr measurement passes +
   consumers. Full test + benchmark A/B. Commit with results.
6. **Phase 3a:** `config/targets.toml` + `config/ir-lowering.toml` + C1–C5
   migration + write-mask/SSO derivations. Commit.
7. **Phase 3b:** Rule 18 D1–D11 casting-graph migration + E1–E6 dead code /
   bug fixes. Commit.
8. Final: full suite + full benchmark run + update results table + arch docs.

Each phase ends with `cargo test --lib` green, `cargo build` warning-free,
Praetor-clean on changed files, and benchmark numbers recorded in §11.

---

## 13. Traceability

- Every code change references this plan document and the phase number.
- Every benchmark run is recorded with: commit hash, date, harness flags,
  BOUND, iteration count, and the full ratio table.
- The baseline worktree `../brief-compiler-baseline` is never updated during
  this effort (per AGENTS.md 11b); it is advanced only after ALL benchmarks
  equal or exceed baseline.
- Regression root-cause investigations, if any, are appended to §2.7 with the
  controlling A/B experiment.
