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

### PROBE RESULT (2026-08-16) — REFUTED, no code landed

Patching `nbody_newton_accel.ll` metadata to add `llvm.loop.vectorize.width = 8`
changed NOTHING. Disassembly of the linked binary shows the current emission is
ALREADY 8-wide (`vrcpps %ymm` at the hot loop, 41 ymm instrs — identical in the
patched and unpatched binary). The 4-wide `rcpps %xmm` diagnosis was stale: the
post-`f67eeaba` emission already vectorizes the countdown to 8 lanes, so width
is NOT the gate.

Timing (BODYCOUNT=2048, BOUND=50000, 5x): base .154s avg ≈ w8 .156s avg, C
.132s avg → both hold the 1.16x residual. Output equality at BOUND=50
(print-boundary crossing): three binaries IDENTICAL.

The accel 1.16x residual is now the documented AVX1 scheduling boundary
(same class as sweep_dense/arr) — NOT a width decision. `vector_max_width`
lands nothing this pass. Gap remains for the deferred vector-state SSA
(VectorPhiGroup) route.

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

### DESIGN REFINEMENT (2026-08-16, recorded after the feasibility audit)

The feasibility audit refined the plan in three ways; the implemented design
follows THIS section, not the sketch above.

1. **Emit `EnsureCap#(q, peak)` at the LET site, not the foreach arm.**
   `q` may be pushed *before* the loop (`q <- 1; foreach ...`) — a foreach-arm
   emission would come too late. The local coll is constructed by
   `construct_local_collection` inside `Statement::Let` (emit_stmt.rs:371-383);
   emitting right after the binding (emit_stmt.rs:421) guarantees the cap is
   raised before ANY push on the coll.
2. **Strip keyed per coll NAME, not `(txn, base)`.** The grow-guard strip at
   `emit_expr.rs:2541` keys on `(txn, base_type)` — two local `Q` colls in one
   txn, only one pre-grown, would strip both. Pre-grow facts are keyed
   `(txn, coll_name)` and the strip reverse-locates the receiver's binding name
   from `let_bindings` (recv_reg.name == the coll handle reg).
3. **Gate on no declared `op Grow`.** `test_coll_grow_override_binding_wins`
   (GeometricQueue + `op Grow: triple(#Lh)`, LOCAL coll, 17 pushes) must keep
   its guard — pre-grow would bypass the declared growth strategy. Exclude any
   coll base whose `TypeDef.body.op_bindings` declares `Grow`.

Soundness: the guard fires `len == cap` BEFORE the store. After
`EnsureCap#(q, peak)` with `peak = track.max`, cap == peak >= len always; the
push that would reach peak starts at `len == peak-1` ≠ cap, so no grow, and the
store lands in `[0, peak-1]`. The walker's `max` is an upper bound over all
paths, so every store is in-bounds. `track.known` (no runtime-bound foreach,
no opaque call, no capacity write) and `cap >= 0` (no `Resize#/EnsureCap#/
TrimCap#` in the txn) are required.

### SHIPPED (2026-08-16, commit follows)

Implementation landed exactly along the DESIGN REFINEMENT:

- **Analysis** `analyze_pregrow` (`coll_length.rs:103`): reuses `seed_tracks` +
  `walk_body`; emits `HashMap<(txn, coll_name), peak>` for LOCAL colls only
  (state colls grow across firings — intra-firing peak is NOT a bound),
  `tr.known && cap >= 0 && max > COLL_DEFAULT_CAP`, base not in `declared_grow`
  (parsed from `TypeDef.body.op_bindings` `Grow`). Two extra backend gates:
  emission only when the let took the scaffolded op-construction path
  (`construct_local_collection` returned Some — `val.name` is the coll handle,
  not a generic heap-seq value) AND the base storage mode is `HeapGrowable`
  (a fixed `T[N]` coll has no grow guard to strip and EnsureCap# would corrupt
  its buffer).
- **Emit** at the Let site (`emit_stmt.rs:421` right after the binding), via an
  `Expr::Call("EnsureCap#", [q, peak])` — one resize call per body emission,
  BEFORE any push and before the foreach.
- **Strip** (`emit_expr.rs:2539-2557`): existing `proven` path keeps
  whole-(txn, base) membership; new `pregrown` path reverse-locates the receiver
  name from `let_bindings` (value == `recv_reg.name`) and checks `(txn, name)`
  in `coll_pregrow`. Per-NAME keying: two local `Q`s in one txn, one pre-grown,
  cannot share a strip.
- **Tests** `test_coll_pregrow_local_moves_resize_before_loop` (region scan:
  NO `__briev_coll_resize` inside any `foreach.body`…`foreach.end` window) and
  `test_coll_pregrow_strip_keys_on_coll_name_not_base` (per-name, cross-coll
  isolation). Existing `test_coll_grow_on_full_guard_emits` still passes (the
  let-site EnsureCap# keeps a resize call in the IR) — but note it now uses the
  pre-grow path, so it is ALSO the pre-grow correctness guard.
- **Verify** (rule 4/19, runtime): compiled the 21-push local-List program
  against the real toolchain; runs 21 pushes, `Count#() == 21`, exit 0.
  Benchmark suite: 40/40 MATCH, no coll benchmark regresses (all use state
  colls with explicit Init — no monotone build-loop benchmark exists, exactly
  as the plan predicted; the pre-grow path is codegen-neutral on the suite).
- Baseline A/B vs `0f97c1c7` worktree on queue_drain / ring_buffer / linked_list
  / global_lifetime: within tolerance (the coll suite is untouched by this
  path).

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

### SHIPPED 3a (2026-08-16)

Implementation landed with one deviation from the sketch (the "op surface" was
STORAGE-AWARE, not the shared growable surface):

- **`derive_sequence_member` `Type::Vector` arm** (`coll_scaffold.rs:386`): a
  fixed `T[N]` sequence member derives `(data, T)` — previously None, so a
  `coll struct` never registered a sequence, never classified storage, and
  fell to the heap-seq literal. This single arm unblocked coll_storage_mode
  (InlineFixed), member synthesis, and literal construction.
- **`llvm_type`/`declare_struct_types` Vector arm** (`emit_toplevel.rs:530`,
  `:437`): `Int[4]` fields declare `[4 x i64]`, not the scalar `{ i64 }`
  collapse. The old `{ i64 }` made `%Fixed = type { i64 }` and field GEPs read
  misaligned heap-seq. The universe path was already correct (`llvm_type` per
  field); the legacy struct_types path hardcoded all-i64 and now routes through
  `llvm_type`.
- **`coll struct` (StaticStruct) member synthesis** (`mod.rs:2463`): the
  scaffolded op surface (`op Count`/`op At` + construction) now synthesizes
  for `coll struct` too — previously only `coll obj` (TypeDef) got it. The
  typechecker mirror (`synthesize_members_for_check` for StaticStruct,
  `typechecker/mod.rs:3060`) makes `Count#`/`Capacity#`/foreach type-check.
- **Storage-aware members** (`synthesize_members`): a FIXED coll gets
  `op Count` returning the constant N (`synth_op_count_fixed`) — it has NO
  hidden `len` slot — plus `op At`; the malloc-based `init_empty`/`init`/`push`/
  `get`/`pop` are skipped for fixed (they assign a Ptr to the array field).
- **`construct_local_fixed_collection`** (`emit_toplevel.rs:1319`): a local
  `let f: Fixed = [1,2,3,4]` mallocs the struct, GEPs the inline array field,
  and stores elements at `data[0..N-1]` — NO [len] header. Over-length literals
  panic (codegen defense in depth) AND are rejected by the typechecker
  (fixed-N literal bound, `typechecker/mod.rs:1237`).
- **`instance_prefix_for` pooled-instance guard** (`emit_expr.rs:2205`): a
  local boxed coll value (InlineFixed with synthesized members) was wrongly
  classified as a POOLED instance once `obj_members` had it — the member body
  resolved `data` against a nonexistent `f.data` column and emitted an
  undefined `@data` global. Now the pooled path requires evidence of unpacked
  `{base}.` columns (matching emit_member_body's boxed-fallback panic test).
- **`Capacity#` fixed branch** (`intrinsics.rs:710`): a fixed coll struct has
  no `cap` slot — Capacity# returns the compile-time N (SPEC §8.10), instead
  of loading a neighboring field at offset 8.
- **Verify** — runtime: `f.data[3]`=4, `.^Length`=4, `Count#`=4, `Capacity#`=4,
  foreach sum=10 → total 26, exit 0. Interpreter parity test
  (`coll_struct_literal_semantics_parity`, 18 = 4+4+10). Benchmarks: 40/40
  MATCH, no regression. Tests: 5 new (inline-array IR, foreach no-extractelement,
  oversize panic, typechecker lifecycle, interpreter parity) — 1887 lib green,
  zero new Praetor diagnostics.

**Not yet done (3a remainder)**: `Data` floor reflection π on a coll struct
value, and `emit_heap_seq` deletion for coll-struct literals (the oversize
panic + typechecker rejection now make it unreachable for fixed colls; the
growable coll obj path never used it either — the deletion is now provably
dead for ALL coll structs and can be removed in the slice-6 cleanup, 3d).

### 3b. Const generics (`coll struct Fixed<T,N> { data: T[N] }`)

1. **Wire `N` into coll layout** — `coll_fixed_length` (`mod.rs:5456-5470`):
   add mono-keyed `struct_types` lookup + `Dimension::Named` resolution so
   `Fixed<Int,4>` → `Int[4]` via `substitute_type` (`typechecker/mod.rs:4149-4154`)
   + `ensure_mono` (`emit_toplevel.rs:1502-1527`).
2. **Verify** — SPEC §8.10 generic example; literal construction for `T[N]`.

### SHIPPED 3b (2026-08-16)

The plan's assumption ("mono/substitute already handle `Fixed<Int,4>`") was
WRONG for the DIMENSION — `substitute_type` resolves the inner `T` but a
`Dimension::Named("N", 0)` stayed unresolved on the generic base, so
`coll_fixed_length` returned 0 and the scaffolded Count read a nonexistent
`len` slot (undefined `@len` global). Three fixes:

- **`coll_fixed_length` mono-keyed** (`mod.rs:5498`): an `Applied("Fixed",
  [Int, 4])` reads the MONO `struct_types` entry (`Fixed<Int, 4>`, whose slot
  is the substituted `Int[4]`), not the generic base (`T[N]` still Named).
- **`ensure_mono` coll re-synthesis** (`emit_toplevel.rs:1649`): a generic
  `coll struct` re-synthesizes its op surface against the SUBSTITUTED slots —
  the mono Count is the constant N. A `coll struct` (no user members) REPLACES
  the base copy; a `coll obj` keeps the dedup-merge (user members preserved).
- **Typechecker const-generic bound** (`typechecker/mod.rs:1237`): the
  over-length literal gate substitutes `Named` dims from the APPLIED args
  (`Fixed<Int, 2>` bound = 2), so `[1,2,3]` for `Int[2]` is rejected.

Verify: `Fixed<Int, 4>` literal `[1,2,3,4]` → `[4 x i64]` GEPs, `Count#`=4,
`Capacity#`=4, `data[3]`=4, foreach sum=10; over-length generic rejected.
Tests: 2 new (backend IR: no `@len`, `[4 x i64]` GEPs; typechecker lifecycle)
— 1889 lib green, benchmarks no failures, zero new Praetor diagnostics.

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