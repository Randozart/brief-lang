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

**B status (2026-08-01, DONE):** three countdown gaps fixed and the A/B
recorded.
- **Guard-block terminator** (when-ended guard bodies): `FunctionContext.cur_block`
  tracks the emitter's live block; the rem reset is emitted BEFORE the guard
  body (dominates the guard's control flow); the latch phis use the guard's
  FINAL block as the guard predecessor, not `.cdg_`. Verified: a nested-when
  guard fires correctly at -O3 (5M/10M).
- **if-ended inner bodies**: the seed `if i == 0` broke the same way — the
  rem/fire landed in the if's merge block. `cur_block`/`body_final` now feed
  the latch phis' `.cdb_` predecessor.
- **Array-state stores were silently DROPPED in the countdown** — the Assign
  arm only handled Ptr-indexed stores, so `f[i] = v` (Float[16]) vanished
  (seed + loop writes). Routed through the shared `emit_array_state_store`.
- **A/B experiment (interleaved x3, BOUND=50M, countdown-dispatched,
  non-folded)**: sweep_dense (16 scalars) Brief 0.41s vs C 0.26s = 1.57x;
  sweep_arr (Float[16]) Brief 0.40s vs C 0.35s = 1.17x. **Conclusion (rule
  #19): the hypothesis that the array form makes Brief faster is REFUTED** —
  Brief's throughput is unchanged (0.40s both); the gap closure is C's array
  reference being slower. No cyclic-shift pass is shipped (no Brief-side win).
  sweep_arr registered as a runtime benchmark (1.18x, MATCH). Diverges from C
  at real bounds like sweep_dense (f32 recurrence compounding; harness
  correctness at BOUND=5 passes).

### Phase C — Watchdogs
1. Parse `-> handler(val)` into `WatchdogSpec.on_fire`.
2. Liveliness per-iteration check; fuel via the countdown budget; time via
   the new `Now#`.
3. On-fire emission (handler call with the last value); required/optional
   paths.
4. Wire `analysis::watchdog::analyze` into the pipeline.
5. Benchmark: `series_converge` (`?[|next−last| > ε] -> print_best(val)`).

**C status (2026-08-01, DONE):**
- **C1/C2** — `-> handler(val)` parses into `WatchdogOnFire { handler, arg }`
  (`val` names the value passed on the fire path; `()` = no arg). Committed
  `444ed2d3` + `7a2610e2`.
- **C4** — `watchdog::analyze` runs on BOTH the check path (parse_and_check)
  and build path; `check_on_fire_handlers` validates the handler names a
  declared txn/defn/node. Committed `9487918f`.
- **C2/C3** — liveliness emission in the countdown (`.cdw_`/`.wdf_`) AND the
  memory-counter emitter (`.cmwd_`/`.cmwdf_`): continue while `?[cond]`
  holds; on false, call `handler(arg)` with the last computed value + exit;
  required-without-handler calls `__watchdog_fail`. The handler's Float param
  surfaced a PRE-EXISTING defn-param bug (boxed i64 handle passed as float in
  frgn calls/returns) — fixed via `reg_float_cache` unboxing in
  `coerce_to_param_type`, `PrintFloat#`, and the Term return path.
- **C5** — `series_converge`: `?[(x-last)^2 > eps] -> print_best(x)` fires on
  convergence. Brief and C both print 0.500050008 (MATCH, 1.00x). Registered
  in the harness.
- **Deferred**: fuel (`?[N cyc]` — cycles_bound) and time (`?[N ms]` — `Now#`
  clock_gettime) — the parse consumes the units but the bounds aren't wired to
  emission yet (the liveliness predicate covers the benchmark; time/fuel need
  the `Now#` intrinsic + deadline compare).

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
- **Phase A5d + blocker 2** — committed (`ea60b4e2`). State-slot MethodCall
  works at `-O3`: emit_method_call saves/restores `last_val_temps` (the reactor
  emits a body twice; stale temps leaked). The pre-existing clang `-O3`
  segfault on Bool state fields was root-caused to `!range` metadata always
  using `i64` bounds on `i8` (Bool) loads; `range_metadata` now formats the
  range in the field's load width and skips unrepresentable bounds. A plain
  Bool program builds and runs at `-O3` (previously segfaulted).
- **Phase A6** — committed (`a9ca39ff`). `<-` dispatch routes onto obj-member
  bindings (`op InsertAt: push(#L,#R)`): `emit_member_body` extracted (shared
  by MethodCall + `<->`), `emit_strategy_member_call` emits a self-bound
  member call; the typechecker treats `&collection <- value` as a push (RHS
  checked against the member's first param type).
- **Phase A7** — committed (`d0964aad`, `e7f1db90`). `op Init` construction:
  `let x: T = val` with `op Init: init(#L,#R)` builds the instance via the
  Init member (allocate + self-bound call + address store). All op_bindings
  now reach operator_defs (not just Cast*). Struct field reads on state-slot
  receivers fixed (scalar via emit_field_access GEP; array via address GEP) —
  the old extractvalue/alloca-spill paths treated the stored instance address
  as a struct value (invalid IR at -O3).
  **Obj instances now work end-to-end at -O3**: state-slot receivers,
  `<-` dispatch, `op Init`, scalar + array field reads.
- **Phase A8 (monomorphization)** — committed (`6326d2e1`). `Stack<Int, 8>`
  instantiates: `Type::Number(i64)` size args (parse + display + exhaustive
  matches), Named-dimension → Anonymous substitution in `substitute_type`,
  backend `ensure_mono`/`resolve_obj_key` registering the mono slots + members
  under the applied key. Verified: `obj Stack<T, N> { data: T[N] }` with
  `Stack<Int, 8>` builds and runs at -O3 (data at 0, len at 64).
- **Phase A9 (stdlib + queue_drain)** — committed (`a686c40f`). collections.bv
  had a malformed `defn hash` signature that broke the ENTIRE file's parse —
  and since imports silently discard unparseable modules, it dropped
  RingBuffer/Stack from every importer. Fixed. queue_drain (RingBuffer<Int>
  import, `<-` push/pop, `op Init`, monomorphized RingBuffer) now builds and
  runs, but its periodic print is off-by-one. **Two countdown bugs found**
  (not yet fixed — next session):
  1. The countdown's counter increment does not populate
     `last_val_temps["count"]`, so a guard that prints the COUNTER (queue_drain
     `PrintLn!(count)`) falls to the header phi (`%cdc`, pre-increment) and
     prints count-1 at the boundary. Guards that print float state (kalman)
     are correct because their Assigns populate last_val_temps. Fix: route the
     counter read to the post-increment register when emitting the guard.
  2. The countdown inner-body emission DROPS the `queue <- count` push member
     call (`.cdb_` has only the pop address + increment); the member-call
     dispatch in `emit_countable_body` needs the self-slot path. The output
     still matches a pure-count loop because the collection ops are discards,
     but the push must emit. Fix: the `Statement::Assign(AddrOf, _)` and
     Expression paths in `emit_countable_body` must reach the `<-` member-call
     dispatch (emit_strategy_member_call).
  queue_drain stays OUT of the harness until both are fixed.
  **FIXED (2026-08-01, A9b):** all three root causes addressed —
  1. The guard off-by-one was NOT the phi/`last_val_temps` read order: the
     countdown's `let_to_field` alias map misread the `<-` push
     (`Assign(Identifier("queue"), Identifier("count"))`) as a `field = local`
     let-alias and remapped the guard's `count` reads to `queue`. Fixed in
     `counter.rs` by excluding field-to-field assignments (both operands in
     `field_index_map`) from the alias map — only a genuine non-field local
     creates an alias.
  2. The dropped push: `emit_countable_body`'s `Assign` arm treated `<-` as a
     scalar backedge. Added the `find_insert_strategy` + `emit_strategy_member_call`
     dispatch (matching the normal `emit_stmt.rs` path) so the push member call
     emits (`data[write % 256] = val` visible in `.cdb_`).
  3. Two more: collections.bv declared `op Init: init(#L, #R)` without defining
     the `init` member (added to Stack/RingBuffer — a stdlib gap, rule #13),
     and the countdown's `emit_inline_init_stores` didn't run the A7 Init-op
     construction (mirrored `emit_init_state`'s dispatch), so the queue slot
     stored 0 and the push dereferenced null.
   queue_drain is re-enabled in `benchmarks/build_and_bench.sh` — output MATCHES
   C at real bounds, and it WINS (0.56x, 0.0357s vs C 0.0631s).
- **Phase A10 (partial)** — committed (`1ef18bac`). `stack_push_pop`
  (Stack<Int, 256> push/pop cycle via `<-` ops) registered in the harness —
  wins 0.50x vs C (0.0296s vs 0.0590s), MATCH. Four fixes to get the generic
  Stack push/pop correct: (a) `emit_strategy_member_call` resolves the mono
  key so member bodies GEP correct self-slot offsets (the generic base layout
  `data: T[N]` computed degenerate offsets — len at 0 instead of 2048); (b)
  the countdown Expression arm dispatches `<- &collection` (the ExtractFrom
  member call) — the pop must run or `len` never decrements and the next push
  overflows; (c) `find_extract_strategy` peels AddrOf layers (`<- &st` lowers
  to `Expression(AddrOf(AddrOf(Identifier))))` — the lookup never reached the
  identifier); (d) the Identifier arm returns the self pointer for self-slot
  ARRAY names (`data` in `data[i]`) instead of falling through to the global
  lookup, which emitted an undefined `@data` global. `ring_buffer_drain` is
  subsumed by queue_drain (same RingBuffer machinery); `hash_ops`/`enemy_swarm`
  defer to the D phase (HashMap heap + SoA desugar).
- Phase B: …
- Phase C: …
- Phase D: …
- Phase E: …

## 8. Merge reconciliation — plugin-macro-rework (2026-08-01)

The other agent's `feat/plugin-macro-rework` merged into `main` at `b00c9681`
(17 commits, 1359 tests, zero regressions). Our branch (`feat/
collections-watchdogs-memory`, 25 commits past the shared base `d6c6c818`)
merges `main` in. Textual conflicts: `BUGS.md` + `emit_toplevel.rs`. Semantic
conflicts (git-silent):

1. Countdown functions keep our `watchdog` param AND adopt their
   `emit_main_header` (`main(i32,ptr)`); callers in `mod.rs` thread both.
2. `!range` — both branches fixed the same clang crash; consolidate
   `range_metadata` / `emit_range_metadata` to one.
3. Macro renames — their Phase 1 makes `GetEnvInt!`/`PrintLn!` errors; our
   benchmark files move to `get_env_int!`/`println!`.
4. Float-param unboxing composes with their B1 `#String` changes.
5. Concurrency gate (now enforced) — our single-node benchmarks must pass.

**Merge result (2026-08-01, committed `20411019` + `fa4909a0`):**
- Textual conflicts resolved: `BUGS.md` (both bug logs kept), `emit_toplevel.rs`
  (our A7/A8 obj/init functions + main's B4 comment + the `!range` consolidated
  to main's `emit_range_metadata`, our `range_metadata` removed).
- Semantic reconciliation: the countdown functions keep our `watchdog` param
  AND adopt their `emit_main_header` (`main(i32,ptr)`); the `%state` alloca
  follows the header; the watchdog blocks (`.cdw_`/`.wdf_`/`.cmwd_`/`.cmwdf_`)
  + `cur_block`/`body_final`/`guard_pred` phis + array-state stores all survive.
- Macro renames: `GetEnvInt!`→`get_env_int!`, `PrintLn!`→`println!` in
  stack_push_pop/sweep_arr/series_converge/global_lifetime.
- Verified: 1364 tests green; all five of our benchmarks build, llc-verify,
  and MATCH C at -O3 with the merged signature; full harness **zero MISMATCH**
  (queue_drain 0.56x, stack_push_pop 0.47x, global_lifetime 0.47x, sweep_arr
  1.17x, series_converge 2.00x — the last two C-win but MATCH).
- Praetor: only pre-existing baseline diagnostics (protocol_llvm_type
  complexity, allocation.rs O(n²), emit_folded_loop_shape 9 params — a dispatch
  hub already over the limit on both branches).
