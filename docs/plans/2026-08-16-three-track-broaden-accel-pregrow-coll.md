# Plan: three-track broaden — accel width (2), D2 pre-grow (3), full coll track (4)

**Date:** 2026-08-16
**Head commit:** `0f97c1c7` (sweep triage closed; baseline below)

Oppens backlog items 2–4 of `2026-08-16-next-steps.md` as ONE plan. Every
track is probe-first (rule 19): the fix lands only if its probe wins. Full
coll track is Phase 3 and the largest; Phases 1–2 are separate, pre-checked
landings.

---

## Baseline (rule 11 — recorded at `0f97c1c7`, clean `cargo build --release`,
`bash benchmarks/build_and_bench.sh --runtime`, BOUND=50M, 5-iter avg)

| Benchmark | Briev | C | Ratio | Winner |
|-----------|------:|--:|------:|--------|
| nbody_newton_accel | .1562s | .1341s | **1.16x** | C |
| sweep_dense | .3612s | .2687s | **1.34x** | C |
| sweep_arr | .4076s | .3476s | **1.17x** | C |
| ring_buffer | .0532s | .0456s | **1.16x** | C |
| sweep_sparse / sweep_mid | .1522 / .2228 | .1558 / .2371 | 0.97x / 0.93x | Briev |
| fasta / mandelbrot | .2369 / .7402 | .2267 / .7125 | 1.04x / 1.03x | C |
| knucleotide / interval_step | .1984 / .0632 | .1979 / .0623 | 1.00x / 1.01x | ~tie |
| all 27 others | — | — | 0.43–0.98x | Briev |

Correctness: 40/40 MATCH. No precomputed binaries (all runtime).

---

## Phase 1 — Item 2: accel 8-wide via config-driven vector_max_width

### Diagnosis (from `2026-08-16-sweep-family-investigation.md` + this probe)

Same-box evidence: C and Briev compile with the SAME clang `-O3 -march=native
-flto`; clang gives C `8-wide` and Briev `4-wide` for the countdown. Width gap
is **shape-driven**, not hardware-driven: LLVM's cost model under-estimates the
countdown scalar-phi web's per-lane profitability. The emitter currently emits
NO width hint — `emit_loop_metadata_nodes` (src/backend/llvm/mod.rs:874-921)
emits only `vectorize.enable` + `loop.align 32`. Width is NOT runtime-decidable
(it is baked into `.ll` at brievc time); it IS compile-time estimable as
**register width ÷ element protocol width**, both known to the frontend.

### Probe (BEFORE any code — in /tmp)

1. Patch `benchmarks/nbody_newton_accel.ll` backedge metadata (`!llvm.loop`
   node, the `.cm_header`/`.cm_latch` loop) to add
   `!{"llvm.loop.vectorize.width", i32 8}`.
2. Link with the exact harness command:
   `clang -O3 -flto -march=native -ffast-math -fdata-sections
   -ffunction-sections -Wl,--gc-sections nbody_newton_accel.ll
   lib/runtime/briev_rt.c -o <p>`
3. Time vs the C reference (BOUND=50M, BODYCOUNT=4096), best-of-5,
   `LC_ALL=C /usr/bin/time -f "%e"`.
4. Verdict: accel < 1.0x ⇒ WIN; flat ⇒ refuted like sweep P1–P3.

### Land (only if probe wins)

1. **Config**: add `vector_max_width` to `TargetSettings`
   (`src/config_tuning.rs:33`, parser at :121-151, defaults :225-245) —
   x86_64/aarch64/arm64 = 8, wasm32 = 4. Separate knob from `vector_min_width`
   (the vector-phi promotion gate) — do NOT conflate.
2. **Frontend**: emit width metadata in `emit_loop_metadata_nodes` — an additive
   arm gated on the shape being a foldable countdown with float-protocol fields
   AND `vector_max_width > 0`. Never global (sweep P2/P3 showed wrong width
   hurts).
3. **Tests**: `.ll` emission asserts `llvm.loop.vectorize.width` + value; harness
   re-run records accel ratio.

**Docs to update**: `docs/architecture/hash-words.md` (no), add § to
`2026-08-16-sweep-family-investigation.md` §8 for the width probe results.

---

## Phase 2 — Item 3: D2 pre-grow (monotone bounded foreach)

### Feasibility (verified by code audit)

- Bound folding already exists: `range_len` `src/analysis/coll_length.rs:568-588`;
  `walk_foreach` `:483-507` tracks the coll through static-bound foreach.
- `EnsureCap#` exists: `intrinsics.rs:751-776` → `__briev_coll_resize`.
- Default cap 16 hardcoded `COLL_DEFAULT_CAP` `coll_length.rs:20`; scaffold
  init emits cap 16 (`coll_scaffold.rs:126-162, 289-327`).
- Emission site: foreach range arm `emit_stmt.rs:1329-1333` builds
  `IterKind::Counter { init, bound, inclusive }`; bound already available.
- Guard strip: `emit_expr.rs:2529-2546` keyed on whole-(txn, base)
  `coll_safe_txns`; pre-grow needs per-site granularity.

### Plan

1. **Analysis** — new `AnalysisResults` fact alongside `coll_safe_txns`
   (`src/backend/mod.rs:70`): `HashSet<(txn_name, coll_type, bound)>` +
   the foreach site needs a marker so the strip is scoped. Shares
   `coll_length.rs` walker; the proof: `foreach x in 0..N`, N statically
   folded, coll is monocoll struct with cap 16 < N ⇒ no per-push guard needed
   after a single `EnsureCap#` to N.
2. **Emit** — foreach arm: when in-`coll_safe_pre_grow`, emit
   `EnsureCap#(q, N)` once before the loop header; set a context flag so the
   per-push grow-guard strips (extend `emit_expr.rs:2544` condition with
   per-site marker rather than whole-(txn,base) membership).
3. **Fail-safe** — if the proof can't fire (runtime bound, conditional body,
   nested foreach, coll not in proof), emit the guard path exactly as today.
   `_ => {}` at `counter.rs:1898` silently drops Foreach from counted walker —
   the pre-grow emission goes through the standard emit_stmt path (same as
   current guard tests, verified).
4. **Verify** — tutorial 21-push foreach becomes guard-free with one
   pre-grow; drain unchanged; 40/40 MATCH (no monotone build-loop benchmark
   exists — zero perf impact expected).

**Docs to update**: `docs/plans/2026-08-16-proven-subset-extension.md` (item 3
shipped marker), SPEC §8.10 grow-on-full cross-reference if behavior wording
changes.

---

## Phase 3 — Item 4: FULL coll track

Scope per user: **coll-struct literal construction, const generics, BUGS.md
stdlib files (iterator.bv / hashmap.bv), iterable slice-6 deletions, Data
reflective floor.**

Already landed (verified): `coll obj` (growable) end-to-end; coll declarations
validate; `defn f<T>` type-param dispatch; ensure_mono exists.

Gaps (code audit, verified): coll struct literal construction falls back to
heap-seq (`derive_sequence_member` has no `Type::Vector` arm, no List→Vector
coercion, `construct_local_collection` gate `has("Count") && has("At")` fails);
const-generic `N` never reaches coll layout (`coll_fixed_length` handles
`Anonymous` only); iterator.bv has free-T body errors; hashmap.bv `term {}`
parse error; slice-6 `emit_heap_seq` live for expr-position lists; Data floor
decided but partially pending.

### 3a. Coll-struct literal construction (blocker for SPEC §8.10)

1. **List→`Int[N]` coercion** — `typechecker/mod.rs:1224-1244` accepts any list
   for coll targets; add scalar-array subtype check so `let f: Fixed = [1,2,3,4]`
   then storage is the inline `[N x T]`, not heap-seq (`emit_expr.rs:794-808`
   fallback must never fire for coll struct).
2. **Backend members** — `derive_sequence_member` add `Type::Vector` arm
   (`coll_scaffold.rs:378-424`) → synthesized ops for InlineFixed; lift the
   `has("Count") && has("At")` gate for InlineFixed so
   `construct_local_collection` (`emit_toplevel.rs:1197-1201`) works.
3. **Slice-6 deletion 1** — `emit_heap_seq` for coll-struct literals dies by
   construction (the coercion makes it unreachable); the `Expr::Tuple`/expr-
   position arm stays (separate path, not coll).
4. **Verify** — SPEC §8.10 example end-to-end: `coll struct Fixed { data: Int[4] }`,
   literal construction, `.^Length == 4`, iterate, `Data` floor reflection π.

### 3b. Const generics (`coll struct Fixed<T,N> { data: T[N] }`)

1. **Wire `N` into coll layout** — `coll_fixed_length` (`mod.rs:5456-5470`):
   add mono-keyed `struct_types` lookup + `Dimension::Named` resolution so
   `Fixed<Int,4>` → `Int[4]` via `substitute_type` (`typechecker/mod.rs:4149-4154`)
   + `ensure_mono` (`emit_toplevel.rs:1502-1527`).
2. **Verify** — SPEC §8.10 generic example; literal construction for `T[N]`.

### 3c. BUGS.md stdlib files

1. **iterator.bv** — free-`T`-body limitation (`?<-` arrow assign where element
   type is a type param): fix the arrow-reader/setter typechecking to adopt the
   declared helper type rather than requiring a concrete receiver member
   (needs `op Count`/`<-` for `List<T>` — likely a `op Count` for the generic
   list surface). `Option<T>`/`Some`/`None` constructors + `''+''` list concat.
2. **hashmap.bv** — `term {}` parser gap (empty block at :2): decide the empty-
   map literal form; unblock; typecheck.
3. **Verify** — both files `brievc check` clean; run-through a small consumer.

### 3d. Iterable slice-6 deletions (`2026-08-14-iterable-slice6-cleanup.md`)

1. Delete the production-dead arms now accessible by 3a's coercion:
   *hardcoded `List` foreach* (dead since `a53c7f90`), the coll-struct list
   literal heap path (dead via 3a). Keep `ringbuf_inline`.
2. **Verify** — `git grep` of removed names zero; suite green.

### 3e. Data as reflective floor (fundamentals-as-types)

1. Finish the landed rename: `Data`-floor semantics pending bits
   (`2026-08-15-spec-implementation-status.md:67-74`);
   `Bit<N>`↔`Bits` unification; no universal `Data` supertype edge (decided).
2. **Verify** — SPEC §17.1 hierarchy examples typecheck.

**Docs to update**: `docs/plans/2026-08-15-coll-length-semantics.md` (shipped
markers), `2026-08-15-spec-implementation-status.md`, SPEC §8.10/§17.1 (same
commit), `BUGS.md` entries closed.

---

## Verification & commit discipline

- Every phase: `cargo test --lib` green; Praetor on changed dirs (no NEW
  diagnostics in changed files; baseline tolerated); Kani only for new
  safety-critical code (none expected).
- Continuous commits per rule; docs updated in the SAME commit as code.
- Full harness re-run after each phase that touches codegen.
- Rule 4: interpreter first — any new list→array coercion behavior goes through
  the interpreter reference test before codegen.

## Risks / fail-safes

- Phase 1: width metadata is honored but can hurt (sweep P2/P3) — gated to
  accel-like shape + config, probe-gated landing.
- Phase 2: pre-grow strip far narrower than whole-(txn,base); if the marker
  plumbing proves fragile, fall back to AST-level insertion (`PluginManager`
  `run_ast` precedent `tests.rs:5917`).
- Phase 3: largest scope — each sub-item lands separately; if 3c/3e prove
  blocked on deeper machinery (e.g. Option-valued system), record remaining
  blockers in BUGS.md and do not weaken contracts.