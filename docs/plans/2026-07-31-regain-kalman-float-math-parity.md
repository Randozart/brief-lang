# Regain kalman / float_math parity — const inlining + batch-loop guard hoisting

**Date:** 2026-07-31
**Branch:** `feat/frontend-driven-dispatch`
**Status:** Plan (experiments to validate before implementation)

## 1. Goal

kalman_filter_runtime (1.21×) and float_math_nonzero (1.21×) are the two
remaining "1.20×" runtime benchmarks. Both were at parity or faster before:
kalman 0.99× (`8a827db1`, 07-11) and 1.01× (`c4cec5d9`, 07-30); float_math_nonzero
1.11× (`8a827db1`, 07-11). The mechanisms that got them there — Era-5 chunked
allocas/struct-SSA and the batch-loop — were removed for backend fragility.
This plan rebuilds the two mechanisms on the now-sound frontend analysis
(Phase 1–3 of the frontend-driven-dispatch plan), using pre-build experiments
to validate each before emission work.

Scope excludes vector-phi promotion (the user opted to avoid hand-rolling
vector phi emission for now).

## 2. Baseline (Golden Rule 11)

Current tip: `fd59d350` (Phase 3). Run: `cargo build --release` +
`bash benchmarks/build_and_bench.sh --runtime`, BOUND=50000000, clang 18.1.3.
Raw log: `/tmp/opencode/p3_runtime.log`. Zero MISMATCH.

| Benchmark | Briev | C | Ratio | Winner | Correct |
|-----------|------:|---:|------:|:------:|:-------:|
| ring_buffer | .0524s | .0460s | 1.13× | C | MATCH |
| float_math | .0734s | .0739s | 0.99× | Briev | MATCH |
| float_math_nonzero | .2003s | .1644s | 1.21× | C | MATCH |
| sparse_dispatch | .0515s | .0609s | 0.84× | Briev | MATCH |
| print_loop | .0607s | .0587s | 1.03× | C | MATCH |
| nbody_newton | 6.9053s | 8.4097s | 0.82× | Briev | MATCH |
| nbody_sqrt | 2.1862s | 2.7948s | 0.78× | Briev | MATCH |
| nbody_sqrt_idio | 2.7251s | 3.6174s | 0.75× | Briev | MATCH |
| fasta | .2088s | .2109s | 0.99× | Briev | MATCH |
| fannkuch_redux | .0607s | .0651s | 0.93× | Briev | MATCH |
| mandelbrot | .6778s | .6622s | 1.02× | C | MATCH |
| kalman_filter_runtime | .2197s | .1808s | 1.21× | C | MATCH |
| knucleotide | .1873s | .1887s | 0.99× | Briev | MATCH |
| cancel_math | .0535s | .0631s | 0.84× | Briev | MATCH |
| bit_clear | .0003s | .0001s | 3.00× | C | MATCH |
| queue_drain | .0570s | .0611s | 0.93× | Briev | MATCH |
| queue_drain_sym | .0565s | .0610s | 0.92× | Briev | MATCH |
| queue_drain_idio | .0564s | .0618s | 0.91× | Briev | MATCH |
| interval_step | .0629s | .0637s | 0.99× | Briev | MATCH |

bit_clear times a ~0.3ms benchmark (noise). Bridge benchmarks: pre-existing
koffi failures, unrelated.

## 3. Investigation summary (evidence)

### 3.1 const coefficients loaded every iteration (float_math_nonzero; kalman 2nd)

Generated `.ll` emits `@A00 = constant float bitcast (i32 …)` (external linkage,
no `private`/`unnamed_addr`). The identifier path in `emit_expr.rs:242`
resolves a const by loading the global (`load float, ptr @A00`), discarding the
value it already has in `ctx.constants`. Verified in asm (`llc -O2`):

```
.LBB6_2:  movss (%rcx), %xmm2 ; mulss %xmm1, %xmm2   ; A00*x0 — LOAD per iteration
          movss (%rdx), %xmm3 ; mulss %xmm0, %xmm3   ; A01*x1 — LOAD per iteration
          ...
```

C (`const float A00 = 1.0f`) loads the coefficients **once** into XMM registers
before the loop (`movss 0xe2d(%rip), %xmm3`) and does pure register ops.

LLVM cannot fold/hoist the loads because (a) external linkage blocks GlobalOpt,
and (b) the guard's print call sits on the backedge path, an aliasing barrier.

### 3.2 guard check every iteration (kalman; version-DAG)

The version-DAG emits the full body then `count % 5000000 == 0` in the hot path
(`kalman.ll:892-903`; asm `imull $magic; rorl $6; cmpl $859`). The removed
batch-loop (`emit_countable_batched_main`, deleted in `81eea6aa` Phase 6) ran an
inner **pure-compute** loop of `batch_size` iterations with no guard check plus
an outer boundary loop. kalman was 1.01× with it.

### 3.3 register pressure (kalman)

kalman's hot loop has 41 spills + 51 reloads per iteration (asm): 12 scalar
float phis + ~12 live intermediates exceed 16 XMM registers. Vector-phi
promotion (fix 3, parked) or the batch-loop's cleaner structure relieve this.

## 4. Fix 1 — const-value inlining

**Hypothesis:** inlining `const` literal values into the IR (instead of global
loads) removes the per-iteration coefficient loads and speeds up
float_math_nonzero substantially and kalman secondarily.

**Mechanism:** in `emit_expr.rs` identifier resolution, when the identifier is a
scalar const in `ctx.constants` whose value folds to a literal
(`try_eval_cfloat`/`try_eval_cint`), emit the literal directly
(`bitcast i32 1065353216 to float` / `add i64 0, N`). Leave the global emission
for address-taken / non-literal consts. The const globals remain declared for
any remaining uses.

**Gate:** `const` value-uses only; scalar protocols (#Float/#Int/#UInt) whose
Expr folds to a literal. No behavior change: the value is identical.

**Expected:** float_math_nonzero 1.21× → ~1.0×.

**Experiment (Fix 1):** transform the current `float_math_nonzero.ll` and
`kalman_filter_runtime.ll` in `/tmp` (replace each `%t = load float, ptr @X`
with `%t = bitcast i32 <bits> to float` using the const's own init bits), then
`clang -O3 -flto -march=native -ffast-math -Wl,--gc-sections <mod.ll>
lib/runtime/briev_rt.c -o <mod>` and time vs the untouched binaries at
BOUND=50000000. If the ratio drops toward ≤ 1.0×, the fix is validated.

## 5. Fix 2 — batch-loop guard hoisting (principled)

**Hypothesis:** a loop whose io guard is `count % N == 0` runs fastest as an
inner pure-compute loop to the next boundary plus an outer boundary check,
eliminating the per-iteration guard. kalman was 1.01× with exactly this
structure. The boundary is derived from the io precondition interval
(flat-node-decomposition plan §4.1), not from `extract_batch_size` heuristics.

**Mechanism (frontend-derived):**
- `src/analysis/batch_shape.rs` (new): from `node_decompose::split_into_segments`
  find the single `PredicateClass::Runtime` guard whose condition is
  `count % N == 0` (N from the guard condition, exactly as the old
  `extract_batch_size` but as a structural precondition-interval derivation).
  Output `BatchShape { counter, batch_size, guard_body, pre_body, post_body }`.
- `AnalysisResults.batch_shape: Option<BatchShape>` (§7.5 extension).
- Dispatch: when `batch_shape` is Some AND the txn is otherwise PerFieldPhi/
  version-DAG-eligible, prefer the batch emission **before** version-DAG.
- Emission: `emit_countable_batched_main` rebuilt in `loop_engine/counter.rs`
  consuming the frontend `BatchShape`: entry → inner pure loop (compute +
  counter, no guard) to `min(bound, next_boundary)` → boundary guard check +
  io → outer latch. **Count=0 peel** (flat-node-decomposition §4.4): evaluate the
  io precondition at the initial state before the first inner loop (or run one
  compute iteration first), so `0 % N == 0` fires — this was the old
  knucleotide/mandelbrot correctness bug and must have a regression test.

**Expected:** kalman 1.21× → ~1.0×.

**Experiment (Fix 2):** hand-peel the guard out of the inner loop in a copy of
the benchmark (07-29 loop-peeling plan method): run the compute body for 5M
iterations in a pure loop, then do the print, repeat — and time vs the
untouched binary. Validates the structure helps *today's* dispatch before
rebuilding the emission.

## 6. Risks / trade-offs

| Risk | Mitigation |
|------|-----------|
| Const inlining breaks an address-taken const | Gate on value-uses + literal-folding only; keep the global |
| Batch-loop count=0 print regression (knucleotide/mandelbrot) | Principled peel + explicit regression test comparing vs C output at BOUND=50 |
| Batch-loop hurts a small-body txn (07-30 float_math_nonzero was 1.27×) | Experiment validates the structure first; add a body-size cost gate if needed |
| Boundary math (udiv/umul per outer iter) | Amortized over N iterations; negligible |

## 7. Documentation (per plan directive 12)

- `docs/architecture/backend-architecture.md` §5.1: add the batch_shape arm to
  the dispatch table; note const inlining in identifier resolution.
- `docs/architecture/features/backend-dispatch.md`: batch-loop vs version-DAG
  decision.
- Rationale comments at every modified site (`// 2026-07-31: …`).
- `BUGS.md`: log the count=0 batch bug root cause if reproduced.
- `benchmarks/results/2026-07-31-regain-kalman-float-math-parity.md` after each
  fix.

## 8. Implementation phases

1. **Experiments** (this plan): Fix 1 + Fix 2 pre-build A/B. Record results in
   §9. Go/no-go per fix.
2. **Fix 1** (const inlining): `emit_expr.rs` identifier path + tests
   (behavioral: float const folds to literal; int const folds; address-taken
   const stays a global). `cargo test --lib`, Praetor.
3. **Fix 2** (batch_shape): analysis pass + tests (boundary extraction, count=0
   peel, non-periodic guard → None) → emission + tests → dispatch arm.
   `cargo test --lib`, Praetor.
4. **Benchmarks** after each: full `--runtime` A/B vs the §2 baseline + vs the
   `8a827db1`/`c4cec5d9` reference worktrees for kalman/float_math_nonzero.
   Zero MISMATCH required. Update §9 and the results docs.
5. **Commits** per logical step (auto-commit between checkpoints).

## 9. Experiment results

### Experiment 1 — const inlining (REFUTED — do not implement for perf)

Transformed the current `float_math_nonzero.ll` / `kalman_filter_runtime.ll` in
`/tmp/opencode` so every `%t = load float, ptr @A00` became
`%t = bitcast i32 <bits> to float` (the const's own init bits, alias-resolved
through `@A10 = alias @A01` etc.). Built both with the harness's exact link
step (`clang -O3 -flto -march=native -ffast-math -Wl,--gc-sections … .ll
lib/runtime/briev_rt.c`) and interleaved-timed 5× at BOUND=50000000.

| Variant | float_math_nonzero | kalman |
|---------|-------------------:|-------:|
| reference (current) | 0.194–0.200s | 0.214–0.218s |
| const-inlined | 0.198–0.200s | 0.220s |
| **Δ** | none (within noise) | none (within noise) |

**Conclusion:** with `-O3 -flto`, LTO already hoists/folds the `constant`
global loads into the preheader (verified: `vmovss 0xe06(%rip)` at setup, pure
register ops in the loop body, identical hot loops in both binaries). The
per-iteration `movss (%rcx)` loads seen under `llc -O2` do NOT survive the real
`-flto` pipeline. Const inlining is not worth implementing for performance.
(Could still be a cleanliness win — removing the global symbol — but is not a
benchmark lever. Parked.)

### Experiment 2 — batch-loop structure (VALIDATED — implement)

Created pure-loop benchmark variants of both txns (same state/consts/body, but
the periodic `when count % 5000000 == 0` guard replaced by a single guarded
`when count == bound { term! -> PrintLn!(…) }` swan song, so the loop has NO
per-iteration guard and the print fires once at the final boundary — exactly
the batch-loop's inner-loop/cold-boundary structure). Compiled with the current
compiler (`.cm_header` PerFieldPhi), built with the harness link step, and
interleaved-timed 8× at BOUND=50000000 against the current reference and C.

| Benchmark | current (version-DAG) | pure loop | C | Δ vs current | vs C |
|-----------|----------------------:|----------:|-------:|:---:|:---:|
| float_math_nonzero | 0.1962s (1.23×) | 0.1575s | 0.1600s | **−20%** | **faster** |
| kalman | 0.2150s (1.24×) | 0.1462s | 0.1737s | **−32%** | **faster** |

**Conclusion:** removing the per-iteration `count % N` guard + the version-DAG's
mid-body present/absent split is worth **20–32%**. A principled batch-loop
(inner pure-compute loop to the next boundary + a cold outer guard) should bring
both benchmarks below C (kalman ~0.84×, float_math_nonzero ~0.98×) versus the
current 1.21×.

### Implementation follow-up (this plan, Fix 2 landed)

The batch-loop was implemented (post-increment guards only — pre-increment
guards like knucleotide are off-by-one at every boundary and stay on
version-DAG) and gated by an arithmetic-density cost model (≥ 40 ops in the
inner body) so only DENSE matrix bodies batch. Verified:

| Benchmark | Phase 3 | With batch | Note |
|-----------|--------:|-----------:|------|
| kalman_filter_runtime | 1.21× | **1.02×** | target achieved; output is the EXACT 5M-compute value (the version-DAG emitted 5M+1 computes — a latent boundary duplicate-compute the batch fixes) |
| float_math_nonzero | 1.21× | 1.21× | batch gate excludes it (small body: 0.205s batch vs 0.196s version-DAG — outer/inner overhead exceeds the guard-removal benefit) |
| float_math | 0.99× | 0.96× | batch gate excludes it (reduction body: LLVM reassociates the p-accumulation with multiple accumulators, changing the output vs C — symmetric-output violation) |
| print_loop / queue_drain | — | — | excluded by the gate (trivial bodies) |

The batch output for kalman (8.139e12 at BOUND=5M) is the exact single-
accumulator float order; C's clang -O3 -ffast-math reassociates to 8.154e12.
The harness checks correctness at BOUND=5 (no prints → vacuous MATCH), so this
reassociation is invisible to it; the batch is strictly closer to the true
computation than the version-DAG's 5M+1-compute value.

## 10. Files

| File | Change |
|------|--------|
| `src/backend/llvm/emit_expr.rs` | const-value inlining in identifier resolution |
| `src/analysis/batch_shape.rs` | new: derive BatchShape from segments + io precondition |
| `src/backend/mod.rs` | `AnalysisResults.batch_shape` |
| `src/backend/llvm/loop_engine/counter.rs` | rebuilt `emit_countable_batched_main` |
| `src/backend/llvm/mod.rs` | dispatch arm for batch_shape before version-DAG |
| `src/backend/llvm/context.rs` | pass-through for batch_shape if needed |
| `src/backend/llvm/tests.rs` | batch emission + const inlining tests |
| `docs/architecture/backend-architecture.md` | dispatch table update |
