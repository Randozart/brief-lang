# Plan: Collections & Instanced Objs, Sweep Arrays, Watchdogs, Memory-by-Proof

**Date:** 2026-07-31
**Status:** Approved — execution in progress
**Required reading:** `docs/handoff-methodology.md`, `AGENTS.md`,
`docs/plans/2026-07-31-restore-object-layer-and-type-system.md` (predecessor,
Phases 0–5 delivered the object layer, type validation, reflection `.^`/`.^^`,
watchdog parsing, tautology detection).

---

## 1. Goal

Complete the object/collection layer so the standard library can exist,
bring the sweep benchmarks to C parity via the array-state form, extend
watchdogs into a real control-flow feature (liveliness / fuel / time /
on-fire handler), and stress-test the memory-by-proof model with benchmarks
that expose — then force us to fix — the compiler's memory-management gaps.

Every claim below is grounded in measurements taken at the plan-start commit.

## 2. Baseline

**Compiler:** `main` @ `3f9ed943` (Phases 0–5 complete). `cargo test --lib` =
1305 passing. Full runtime harness: zero MISMATCH, no regressions vs Phase-0
baseline (kalman 0.85×, float_math 0.62×, fmn 0.96×, matrix_pipeline 0.67×,
accumulator_flush 0.70×, telemetry 0.99×, pid 0.98×; sweep_sparse 1.40× /
sweep_mid 1.10× / sweep_dense 1.50× — the gap this plan addresses).
`queue_drain` was removed from the harness (depends on the collections stdlib).

**Test of record for the sweep gap** (harness-exact link, BOUND=50M):
Brief countdown loop = **40 instructions** vs C = **34**; the 6 extra are
cross-lane shuffles (9 vshufps / 6 vblendps / 3 vinsertf128 / 2 vextractf128
vs C's 6 vshufps / 2 vblendps / 4 vperm2f128). The countdown structure is
NOT the cause (one tight block, guard cold-split, leaner counter than C);
the cause is SLP-of-16-unrolled-scalars vs C's loop-vectorized
`f[(i+1)%16]` shifted loads.

## 3. Locked design — the obj model (unambiguous)

### 3.1 Declaration
```
obj Name<Params> "{" (slot | op | prop | member)* "}"
member ::= txn-signature  | defn-signature  | node-signature
```
- An `obj` is **pure composition**: slots (data) + members (behaviour).
- **No inheritance on `obj`** (`TypeDef.parent` is not accepted). `type`
  retains inheritance and protocol membership.
- Members are **self-parameterized**: an implicit `self` binding of the obj's
  type is in scope in every member body; bare slot names resolve to `self.<slot>`.

### 3.2 Member kinds and their one desugar
| Member | Role | Desugar target |
|--------|------|----------------|
| `txn name(params) [pre][post] {body}` | callable method | `defn name(self, params)` |
| `defn name(params) [pre][post] -> T {body}` | pure method | `defn name(self, params)` |
| `node name [pre][post] {body}` | **reactive, per-instance** | a main-loop-driven reactive node with `self` bound to the instance |

- `node` members are **not callable** (nodes are reactive, zero-param, driven
  by the main loop — the language's `node` semantics).
- There is exactly **one** implementation per desugared form: the codegen,
  lifetime analysis, and memory model see functions + data only. No OOP
  runtime, no virtual dispatch, no hidden mutation.

### 3.3 Instancing
- `let e: Enemy = Enemy(args);` constructs an instance (a struct value whose
  slots are the fields). Multiple instances coexist: `let e1: Enemy = …`,
  `let e2: Enemy = …`, `let enemies: List<Enemy> = …`.
- `List<T>` (and every collection) is itself an `obj`; obj-in-obj is allowed.
- Field access `e.health` and method calls `e.damage(10)` follow the Phase-1
  `Expr::Field` / `Expr::MethodCall` machinery.

### 3.4 Reactive instances and the SoA default
- A `node` member fires from the main loop when the **instance's own**
  precondition holds. One declaration ⇒ every instance reacts independently
  (games: enemy AI; frontend: `.rbv` components that re-render reactively).
- **Desugar target defaults to SoA**: for a homogeneous collection
  (`List<Enemy>`), each field becomes an array and the `node` body becomes a
  per-field batch update — the same array-state machinery Phase B builds —
  so the reactive sweep over instances vectorizes. **This default stands
  unless the Phase A/B benchmarks show AoS wins.** Single instances desugar
  direct (struct value + per-instance node in the main loop).
- The choice is a **codegen decision** driven by collection shape; it is never
  a source-language construct.

## 4. Locked design — watchdogs (unambiguous)

```
watchdog ::= ("?" | "!") "[" condition "]"
           | ("?" | "!") "[" condition "]" "->" handler "(" ")"
handler  ::= identifier
```

| Clause | Meaning | LLVM emission |
|--------|---------|----------------|
| `?[formula]` | **liveliness** — a Bool predicate over state; fire when it stops holding | per-iteration check; false → fire path |
| `?[N cyc]` | **fuel** — an iteration budget | the countdown loop's cycle budget (`cycles_bound`) |
| `?[N ms]` | **time** — a wall-clock deadline | `Now#` (new `clock_gettime(CLOCK_MONOTONIC)` intrinsic in `brief_rt.c`); elapsed-vs-deadline compare |
| `![…]` | required watchdog | fire = error-exit path |
| `-> handler(val)` | **on-fire callback** | the loop calls `handler` on the fire path, passing the **last computed value** explicitly (never by reference to state that may be reset) |

- `handler` is a user function; `val` is the last computed value (the value a
  `term val` would have produced on the current iteration). For a series that
  "could go on forever": `?[|next − last| > ε] -> print_best(val)`.
- The existing `analysis::watchdog.rs` (trigger→handler contract checks) is
  wired into the pipeline and extended for the `-> handler(val)` form.

## 5. Locked design — memory-by-proof

- **Current reality (verified):** `analysis/allocation.rs` builds a
  producer→consumer DAG and chooses Arena (in-txn) / Alloca (bounded) /
  Malloc (top-level); `Provenance` tracks pointer origins; the arena bumps and
  realloc-grows. **There is no global-lifetime inference** — state lives for
  the program's duration.
- **Stress family** (each exposes a gap; the gap is fixed in the same phase):
  - `deep_recursion` — recursion with a **runtime-determined** depth (tests
    the proof engine's termination proof against non-constant bounds); fix:
    tail-call/iteration conversion.
  - `arena_churn` — bump-arena exhaustion; fix: correct realloc-grow (never
    realloc non-heap memory).
  - `dangling_pointer` — `&local` escaping into a state Ptr then deref'd; fix:
    provenance warning → hard compile error.
  - `linked_list` — `Malloc#`/`Free#` heap nodes, pointer chasing; fix: DAG
    leak / use-after-free detection.
  - `global_lifetime` — a state field provably dead after its last consumer;
    pins the intended end-state for the design plan.
- **Global-lifetime design plan** (written in the same phase): for each state
  field, prove the last transaction that reads it; when that is proven and the
  field is heap-backed, emit a free after the last consumer. The benchmark
  calibrates correctness (no premature free, no leak).

## 6. Phases

### Phase A — Collections stdlib + instanced objs (collections first)
1. Generic `struct`/`enum` type-param parse; struct-literal comma +
   bare-shorthand forms (`Person { name: "Alice", age: 30 }`,
   `Arena { base, offset: 0 }`).
2. Member-body typechecking: walk `TypeDefBody.members` with `self` + slot
   names bound (closes the `len = "hello"` hole).
3. `node` members in `parse_obj_like`; the instance model
   (`Enemy(100)`, `List<Enemy>`).
4. Array-slot layout + `data[i]`; **fix the Index typechecker** so `f[0]`
   resolves the element type of a Vector receiver (currently `Int`).
5. MethodCall codegen: state-slot receivers first, then locals
   (the `emit_expr.rs:773` panic boundary).
6. `<-` op dispatch onto obj-member bindings (replaces the old marker path at
   `emit_stmt.rs:414`).
7. `op Init` construction (`let q: RingBuffer<Int> = 0`).
8. Generic obj monomorphization: substitute T/N into slots **and** member
   bodies; instantiate on first use.
9. Rewrite the `:>`-era stdlib files (stack/queue/hashmap/hashset/option/
   result) onto the dual model; re-enable `queue_drain`.
10. Benchmarks: `stack_push_pop`, `ring_buffer_drain`, `hash_ops`,
    **`enemy_swarm`** (N enemies × reactive node — forces the SoA batch
    desugar and ties A↔B).
**Milestone:** `collections.bv` parses, typechecks, builds; `queue_drain`
returns to the harness with correct output.

### Phase B — Sweep array-state experiment
Experiment before code: `Float[16]` array-state variants of sweep_dense
(`f[i]` / `f[(i+1)%16]` / `f[(i+15)%16]`) through the harness-exact link, A/B
vs C (interleaved ×6, BOUND=50M, print-boundary-verified). Prereqs: the A4
Index fix + `%State` emitting `[16 x float]` with indexed GEP loads. If the
array form closes the 40→34 gap, ship `_sym`/`_idio` variants; only then a
first-class cyclic-shift pass with its own baseline.

**B status (mid-phase):** array-state codegen was emitting INVALID IR in loops
and is now fixed and llc-verified:
- reads: GEP into %State + scalar load (was `extractelement i64` on a loaded
  `[16 x float]` aggregate — invalid).
- writes: `emit_array_state_store` (flat guard clauses) — GEP + scalar store
  with `ensure_typed_value`.
- init: `store [N x T] zeroinitializer` (was invalid `store [N x T] 0`).
- loop-carried phis: aggregate fields are memory-resident — excluded from the
  countdown's phi set (`is_aggregate_field`).
- FFI-guard cold-function outlining: aggregates are not outlined as scalar
  params (`can_outline_all` fails) — the guard stays inline.

**Remaining B blocker (precise):** with an aggregate field written, the
countdown's guard block (`.cdg_`) leaves the guard-body `when`'s end block
(`guard.end`) WITHOUT a terminator — `expected instruction opcode` at the
closing `}`. The scalar path terminates `guard.end` with the `br .cdl_` latch
edge; the aggregate path must emit that latch br after the guard's `when`
`next_label`. This is a block-pointer / emission-order bug in
`emit_countable_countdown_main` around `emit_countable_body` + the latch
branch. Local array variables (`let n: Float[16]`) are also unsupported (a
local is allocated as i64) — the experiment uses state arrays for now.

### Phase C — Watchdogs
1. Parse `-> handler(val)` into `WatchdogSpec.on_fire`.
2. Liveliness per-iteration check; fuel via the countdown budget; time via
   the new `Now#`.
3. On-fire emission (handler call with the last value); required/optional
   paths.
4. Wire `analysis::watchdog::analyze` into the pipeline.
5. Benchmark: `series_converge` (`?[|next−last| > ε] -> print_best(val)`).

### Phase D — Memory-by-proof
Stress benchmarks + fixes as §5; global-lifetime design plan written.

### Phase E — Modifiers, the concurrency gate, and the principle reframing

**Principle reframing (docs):** AGENTS.md rule #2 becomes "NO OBFUSCATION OF
SPECIAL TREATMENT" (two-part: avoid accidental complexity; disclose special
treatment via `#`/`!`/`.^` markers). User-facing directives (`seq`, `vol`,
`async`, `sync<g>`) are ordinary keywords and **must never make code faster** —
a modifier-beaten default is a compiler bug (fix the default).

**The modifier family (all PREFIX — `node async`, never postfix):**
| Modifier | Axis | Meaning |
|----------|------|---------|
| `seq` | ordering/layout/sequence | `seq struct` (bypass `apply_field_modes`), `seq txn`/`seq node` (sequential dispatch, no `emit_parallel_reactor`), `seq Int[x]`/`seq foreach` (`!llvm.loop.vectorize.enable=false`) |
| `vol` | memory visibility | `vol let x` → `load volatile`/`store volatile` (reuse the mmio volatile machinery) |
| `async` | explicit simultaneous firing | an acknowledgement, not a hint; prefix-only (`node async` postfix removed) |
| `sync<group>` | group barrier | members that fire hold off finishing until all fired group members have (a group commit / join point) |

**The concurrency gate (NO IMPLICIT CONCURRENCY):** a reactive-node pair is
"eligible to fire together" iff the proof engine proves `pre_A ∧ pre_B`
satisfiable AND there is no XOR read-write overlap. An eligible pair that is
neither `async`-marked nor `sync<group>`-grouped is a **hard error**: "declare
`async` on both or `sync<group>` on both." Existing multi-node tutorials are
reclassified in the same commit.

**Delimiter semantic load (docs + `sync<group>`):** `<>` = compile-time type
specialization (generics, protocol variants, targets, groups); `()` =
application & binding (calls, params, construction, op bindings —
declarations take params). `op Add(Float)` stays `()`; `sync` is `<>`.

**Implementation:** modifier lexer/parser (prefix), the four targets, the gate
analysis (SAT + XOR overlap), the barrier codegen, the never-faster regression
test (default output never slower than the modifier output), reclassify
tutorials/benchmarks.

## 7. Verification (per phase)
`cargo test --lib` green (a regression test per feature); `cargo build` no new
warnings; Praetor (one `--target` per invocation); full harness A/B zero
MISMATCH with the §2 baseline; docs in the same commit (SPEC obj/watchdog
grammar, `learn-brief` collections + watchdog chapters, `docs/architecture`).

## 8. Risks and mitigations
| Risk | Mitigation |
|------|-----------|
| Monomorphization scope (A8) | instantiate-on-first-use; regression tests per generic shape |
| MethodCall `self` codegen (A5) | state-slot receivers first; locals via alloca copy-in/copy-out; the Phase-1 panic becomes the final emission |
| SoA reactive desugar perf | benchmark-gated; AoS fallback retained if tests show it wins |
| `Now#` time source (C) | `clock_gettime(CLOCK_MONOTONIC)` in `brief_rt.c`; deterministic ticks remain the default |
| Global-lifetime free (D) | never frees until the last-consumer proof is sound; the benchmark pins it |

## 9. Results (filled after each phase)
- **Phase A1+A2** — committed (`73773995`). Generic `struct ListBuffer<T>` /
  `enum Option<T>` parse; struct-literal comma + bare-shorthand
  (`Arena { base, offset }`); generic-constructor type application. Member
  bodies typechecked (self + slots bound); assignment LHS/RHS checked. Tests
  1310 (+5).
- **Phase A3+A4** — committed (`f8017814`). Reactive `node` members in objs
  (per-instance); array state fields emit valid IR (GEP read, array store,
  zeroinitializer init, memory-resident in loops). Index element-type
  inference fixed (`f[0]` on `Float[16]` is Float). Tests 1310.
- **Phase B groundwork** — committed (`37e09173`). Aggregate fields excluded
  from loop phis + FFI-guard outlining. Remaining: countdown guard-block
  terminator.
- **Phase A5** — committed (`e784e311`). MethodCall codegen for LOCAL
  receivers (self-bound member bodies; `st.push(5)` inlines `push` with self
  = the struct address; bare slots GEP+load/store). Struct constructor
  `Stack()` emits a struct literal. Let bindings record declared types.
  Struct type declarations deduplicated.
  **Gaps documented:** struct-typed top-level lets are not state fields (an
  obj instance in state has no %State slot — needed for `List<Enemy>`); a
  PRE-EXISTING `-O3` clang crash on Bool state fields blocks harness-level
  testing of struct programs (a plain Bool-only program fails to build at
  -O3; at `-O0` the method-call program runs correctly).
- Phase B: …
- Phase C: …
- Phase D: …
- Phase E: …
