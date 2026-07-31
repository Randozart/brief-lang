# Session Report — Frontend-Driven Dispatch (2026-07-31)

**Scope:** the `feat/frontend-driven-dispatch` branch (Phases 0–2 + two follow-up
optimization plans), merged into main at `35cee92a`.
**Purpose:** record every finding, decision, benchmark result, and bug from the
session so a future session can reconstruct the reasoning without re-deriving it.

---

## 1. Goal

The LLVM backend re-derived codegen decisions (loop dispatch, type categories,
tuning constants) from body re-walks, hardcoded type-name matches, and
heuristics — a "backend makes decisions" architecture that kept breaking. The
plan `docs/plans/2026-07-31-frontend-driven-dispatch.md` rebuilt it as
**frontend-driven dispatch**: the backend CONSUMES decisions computed once in
the frontend (`AnalysisResults`), derives type knowledge from the casting graph,
and reads tunables from config.

## 2. Phases delivered

| Phase | Commit | What |
|-------|--------|------|
| Plan | `5fed0573` | the architecture plan |
| Phase 0 | `ed2f4234` | baseline + migration groundwork |
| Phase 1a | `0682d764` | swan-song hoist moved to the frontend |
| Phase 1b | `c953c3c4` | the 5-way dispatch collapses onto `LoopShape` |
| Phase 2 | `322d68f3` | measurement passes: density, modulo partition, inline cost, unguarded-FFI |
| Phase 3 | `1742f6f4`…`f2c0daaa` | config migration + Rule 18 cleanup (zero `Type::Custom.*==` in `src/backend/llvm/`) |
| Batch-loop | `caaab9d9` | inner pure-compute loop + outer boundary (kalman parity) |
| Countdown | `9d7a2404` | single tight loop + cold guard block — the universal periodic-guard emission |
| Phase 2 follow-up | `163ae99c` | real-program benchmarks + implicit-coercion type safety |

## 3. Key findings

### 3.1 The kalman / float_math gap was the guard, not the compute
kalman was 1.21×, fmn 1.21× (best-ever 0.99×/1.01× in older eras). Asm-level
root cause: the version-DAG's guard-in-loop costs a modulo + a body-split (~5
extra instructions vs C's loop), and its guard-present block re-ran the matrix
multiply (a latent 5M+1-compute defect — the batch fixed this). A pure loop
(no guard) ran at C parity, proving the entire gap was the guard structure.

### 3.2 The batch loop mis-vectorizes cross-indexed bodies
The batch's PURE inner loop has no conditional, so LLVM's vectorizer runs free.
For cross-indexed matrix bodies (fmn's 3×3) it emits shuffle-heavy AVX
(`vmovsldup`/`vmovshdup`/`vinsertps`) that is SLOWER than the 29-instruction
scalar loop. The guard's conditional branch was accidentally acting as a
vectorizer-blocker. This is why the batch regressed fmn (0.2050s vs 0.1962s).

### 3.3 The countdown loop is the principled optimum
A single tight loop with a loop-carried `%rem` counter (decrement each iteration;
on 0 branch to a COLD guard block that prints and resets) removes the modulo
(`sub;cmp` instead of `imul;shr;imul`), keeps the body in one block (no
body-split), and preserves the periodic print. It won on ALL five existing
periodic-guard benchmarks:

| Benchmark | version-DAG | batch | countdown |
|-----------|------------:|------:|----------:|
| kalman | 0.2150s | 0.1783s | **0.1500s** |
| fmn | 0.1967s | 0.2067s | **0.1533s** |
| float_math | 0.0667s | 0.0000s* | **0.0417s** |

\* the batch's 0.0000s is a reassociation artifact (wrong output).

Full harness (BOUND=50M, zero MISMATCH): kalman **0.86×**, fmn **0.95×**,
float_math **0.66×**, print_loop **0.62×**, queue_drain **0.50×** — several now
faster than C.

### 3.4 The countdown is NOT universal (the sweep finding)
New real-program benchmarks (telemetry_stream, pid_control, matrix_pipeline,
accumulator_flush, sweep_sparse/mid/dense) map where it wins:

| Benchmark | Ratio | Body |
|-----------|------:|------|
| matrix_pipeline | 0.66× | dense 4×4 matmul |
| accumulator_flush | 0.71× | clean reduction + reset |
| telemetry_stream | 0.99× | rolling EMA |
| pid_control | 0.97× | PID loop |
| sweep_dense | **1.49×** | cross-indexed cyclic chain |
| sweep_mid / sweep_sparse | 1.10× / 1.40× | cross-indexed cyclic chains |

The countdown's `%fire` conditional is NOT a reliable vectorizer-blocker: for
cross-indexed neighbor chains (`fi = a·fi + b·f(i+1) + c·f(i−1)`) LLVM
vectorizes it into shuffle-heavy AVX anyway. **The dispatch rule is not "always
countdown"** — per-field-independent bodies (matmul rows, EMA) win with the
countdown; cross-indexed chains need a different structure (open follow-up).

### 3.5 The LTO lesson (methodological)
`llc -O2` / raw `.ll` inspection does NOT reflect the benchmark harness's
`clang -O3 -flto` pipeline. A hypothesis (const-inlining) that looked strong
under `llc -O2` was REFUTED by a pre-build experiment under the real pipeline —
LTO already hoists/folds the const loads. Every codegen claim must be verified
against the actual linked binary before acting on it.

### 3.6 Latent bugs found and fixed
| Bug | Fix |
|-----|-----|
| Implicit `Int * Float` silently bitcast the int (garbage) | **type error** unless a cross-type/cross-protocol `op` overload is declared (`op Mul(#Float)` / `op Mul(Float)`) |
| Casting graph `IntToFloat` emitted `sitofp to double` (broke `as Float`) | emits the target width |
| Outlined-guard float params allocated as i64 | the binding's type |
| Countdown guard-writes lacked a latch phi | guard-written fields get a `[body, guard]` phi |
| `br i1 true` + unreachable rollback in assume_shape | removed |
| SVO packed header `(len<<32)|(cap<<32)` overlapped | disjoint `pack_svo_header` + round-trip test |
| frgn `declare` order non-deterministic (HashMap) | sorted by key |

## 4. Benchmark suite after this session

Added 7 real-program benchmarks with C references (telemetry, PID, matrix
pipeline, accumulator flush, 3-density sweep). The full `--runtime` suite now
covers 26 programs. `cargo test --lib`: **1279 passed**, Praetor clean.

## 5. Architecture state

- `AnalysisResults` carries loop shapes, swan songs, density, modulo partition,
  unguarded-FFI set, inline decisions, and batch shapes.
- The backend dispatch: pure-fold → countdown (periodic post-increment guard) →
  version-DAG → InlineSsa → PerFieldPhi.
- Config: `config/targets.toml` (per-target) + `config/ir-lowering.toml`.
- Zero `Type::Custom.*==` matches in `src/backend/llvm/`.

## 6. Open items

1. **Cross-indexed chains** (the sweep finding): detect structurally and prefer
   scalar/version-DAG for them (Phase 3 of the countdown plan).
2. **The implicit-coercion OPEN bug**: the typechecker's op resolution for
   custom types still uses only the builtin table — custom-type arithmetic
   always errors even for same-type custom ops (pre-existing; the cross-type
   overload path needs `get_operator_intrinsic` to consult `regular_ops`).
3. float_math_nonzero pre-increment guards (knucleotide/mandelbrot) stay on
   version-DAG — the batch/countdown are post-increment-only by design.

## 7. Methodology (the rigorous practice this session used)

See `docs/handoff-methodology.md` — the required-reading companion to
`AGENTS.md` that documents the investigation → plan → experiment → implement →
verify → document loop, with this session as the worked example.
