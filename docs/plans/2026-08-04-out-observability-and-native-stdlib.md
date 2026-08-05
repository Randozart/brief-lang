# `out` Observability Keyword + Native-Stdlib Path (C Independence)

**Date:** 2026-08-04
**Status:** Phases 1–3 DONE (`03becb29` cast, `cc8fe299`+`47e5964c`+`29da13b0`+`d86f9844` out, `bcb3cebe` casting de-dup); Phase 4 (`.ebv` heap reframe) IN PROGRESS
**Branch:** `feat/out-observability` (worktree `../briv-compiler-out`)
**Related:**
- `docs/plans/2026-08-03-glue-folders-node-bridge.md` (GLUE convergence — Phase 6)
- `docs/plans/2026-08-01-consumptive-operators-lifetime-and-c-surface.md` (intrinsic audit, C-surface reduction)
- `docs/architecture/intrinsics-vs-stdlib.md` (the dividing-line doc, needs update)
- `docs/architecture/frgn-export-glue-architecture.md` (frgn/export/GLUE)
- `docs/architecture/casting-protocol.md` (the casting graph — Int⇄String lanes)
- `AGENTS.md` Rule 2 (never-faster contract) and Rule 13 (stdlib is the extension mechanism)

---

## The Goal

Prove Briv can hold its own as a **systems language without dependencies**: every
C-expressible piece of the runtime becomes Briv-native. C shrinks to the
**irreducible OS boundary** (syscalls, threads, signals) — and even some of that
is eventually raw-syscall lowered. Every native implementation is a win.

This plan has two threads that interlock:

1. **The `out` keyword** — the stdlib-side observability pin. It lets stdlib
   functions claim "the compiler must not eliminate calls to me," which is what
   permits formatting/logic to move out of the C runtime and into Briv without
   being dead-code-eliminated.
2. **The native-stdlib migration** — moving the pure-logic half of
   `lib/runtime/briv_rt.c` (digit→string, string byte-ops, `str_to_int`, file
   read/write) into Briv, and creating the first real `.ebv` freestanding
   target.

---

## Core Design: Keywords are Pins, Never Accelerations

Briv's compiler **always maximizes optimization** — even if that means folding a
program into a single LUT. No annotation, pragma, or keyword makes a program run
*faster*. Keywords like `seq`, `vol`, and `out` are **restrictions**: they tell
the compiler "I need this specific thing done, you cannot optimize it out." They
put pins in the program.

This is the never-faster contract, already codified in `AGENTS.md` Rule 2 and
instantiated in `seq` (`src/backend/llvm/mod.rs:2125-2128`: "a modifier must
never win — if the default parallel path is slower than seq, that is a compiler
bug to fix in the default"). `out` joins that family under the same contract.

**Two enforcement points, one concept:**

| Where | Mechanism | Semantics |
|---|---|---|
| Intrinsics | `observable: true` in `intrinsic_signatures.rs` (enforced at `emit_toplevel.rs:1668`) | Pinned **by design** — an intrinsic IS its behavior; no declaration needed. `Print#`, `Malloc#`, `Copy#` etc. |
| Stdlib (`defn`/`node`/`txn`/`let`) | **`out` keyword** | The author declares "calls to this must survive DCE and must not be folded away." |

Observable intrinsics need **no `out`** — they are already exact by design.

---

## `out` Semantics

### Scope of application

- **`out defn`** — a callable whose call sites are liveness roots: the call
  survives DCE and blocks pure-loop folding. The function's **body is still fully
  optimized** (its internals may be folded, inlined, or reordered so long as
  behavior is identical); only the *call boundary* survives.
- **`out node` / `out txn`** — the reactive firing itself must be kept.
- **`out let`** — the variable's **reads and writes are liveness roots**: they
  are never eliminated. It does **not** necessitate a memory slot — unlike `vol`,
  it does not force `load volatile`/`store volatile` MMIO semantics. `out let` is
  "this variable's computation must happen"; `vol let` is "these accesses must be
  volatile memory ops." `vol` **implies** `out` (`out vol let` is legal but
  redundant).
- **NOT `out export`** — whatever `export` returns IS the output boundary for the
  other language; it is inherently observable at the ABI. If the same function is
  also called from Briv, there are two versions: the exported one (ABI-pinned)
  and a Briv twin that may be optimized into other bodies.
- **NOT `out frgn`** — a `frgn` is always an input/boundary. Its call is
  inherently observable (external side effects unknown to the compiler).

### What `out` does NOT do

- Does not force volatile memory semantics (`vol` does that).
- Does not pin the function's *internals* — only the call boundary survives.
- Does not make the function faster, and never should (a modifier-beaten default
  is a compiler bug).

### Observability propagation

**Direct-only.** A pure function that *calls* an `out` function is not
transitively pinned for DCE — the `out` function's own body is emitted (it's a
liveness root), so anything it calls survives *inside* that body. Call-chain
propagation is not needed and would over-pin.

---

## Phase 1 — `(Type) expr` Cast

**Goal:** both `expr as Type` and `(Type) expr` are valid, lowering to the same
`Expr::Cast`.

**Current state (verified):** only `expr as Type` parses (`src/parser/expressions.rs:232-239`).
`(expr)` is grouping only (`parse_grouping`, `expressions.rs:511`).

**Work:**
- In the primary/grouping path, after `(`, if a **known-type** identifier is
  followed by `)` and then a non-operator expression, parse `(Type) expr` →
  `Expr::Cast(Box::new(expr), ty)`.
- Ambiguity resolution (the C approach): the identifier must resolve to a
  registered type (universe / type table) — if it's not a known type, fall back
  to grouping. Lookahead past `)` distinguishes `(Int) x` from `(x)` and
  `(a, b)`.
- Both forms hit the casting-graph lane resolution identically.

**Tests:** `(String) n` ≡ `n as String`; `(Int) f` ≡ `f as Int`; `(x)` grouping
unchanged; `(a, b)` tuple unchanged; unknown `(NoSuchType) x` → grouping error.

**Files:** `src/parser/expressions.rs`, `src/parser/tests.rs`.

---

## Phase 2 — `out` Keyword

### Lexer

New `Token::Out`. Reserved word `out`. (Verified: no `Token::Out` exists today;
`Token::Output` is a distinct identifier-usable token for `output`.)

### Parser

Mirror the existing prefix-modifier patterns:
- `seq node`/`seq txn` (`definitions.rs:39-53`)
- `async node` (`definitions.rs:59-64`)
- `vol let` (`statements.rs:16-27`)
- `sync<group> node` (`definitions.rs:33`)

Top-level dispatch arms:
- `out defn` → parse `defn`, push `Annotation { name: "out", value: None }` onto
  `Definition.modifiers` (`top.rs:104` already has `modifiers: Vec<Annotation>`).
- `out node` / `out txn` → same on `Transaction.modifiers`.
- `out let` → push onto `Statement::Let.modifiers` (the `vol let` pattern).

### Frontend → AnalysisResults

The frontend computes the **observable-name set** and stores it in
`AnalysisResults` (frontend-driven dispatch pillar — the backend only consumes):

- Collect names of `defn`/`node`/`txn` marked `out`.
- Collect `let` variables marked `out` or `vol` (vol implies out).
- Expose as a queryable set, e.g. `AnalysisResults.observable_names`.

### Backend consumption (consume only)

The passes that today read intrinsic observability gain a companion "is this
named defn/node/txn `out`" check:

- `emit_toplevel.rs:1668` — the outlining/guard logic (`is_ffi_call`): a call to
  an `out`-named function is treated like an observable intrinsic call.
- The purity/folding gate (`loop_shape.rs:195-206`,
  `transition_graph.rs:739-799`): calls to `out` functions block pure folding.
- DCE / liveness: calls to `out` functions are liveness roots.
- `out let`: reads/writes of the variable are liveness roots (no volatile
  requirement).

**Key invariant:** `out` never changes emitted behavior for non-out code. The
default path (functions pure, optimizable, foldable into other bodies) is
unchanged.

### Benchmark anchor migration

**2026-08-04 finding (supersedes the original intent):** on inspection, the
three candidate benchmarks GENUINELY use the memory intrinsics for computation —
they are not pure observability anchors:

- `benchmarks/arena_churn.bv` — `Load#(a)` feeds `sum` (real work).
- `benchmarks/utf8_ops.bv` — `Store#` writes runtime-varying data that
  `memcmp`/`UTF8_validate` consume (real work).
- `benchmarks/linked_list.bv` — uses `node[0]`/`node[1]` indexing (real work).

Forcing `out` onto them would BREAK benchmark semantics, so **no migration**. The
backend is already conservative (any call blocks folding; `Malloc#`/`Store#`/
`Load#` are `observable: true` intrinsics), so the "keep the allocation
observable" goal is already met without `out`. The `out` keyword's benchmark
value materializes in Phases 4–5, when stdlib print/format functions move into
Briv and need `out` to survive DCE.

**Tests:** parse tests for all four forms; a DCE test proving an `out` function
call survives while an identical non-`out` call is folded; a folding test proving
a loop body containing an `out` call is not purely folded; an `out let` test
proving reads/writes are live but not volatile.

**Files:** `src/lexer.rs`, `src/parser/definitions.rs`, `src/parser/statements.rs`,
`src/analysis/` (observable set), `src/backend/llvm/emit_toplevel.rs`,
`src/backend/llvm/loop_engine/`, `src/analysis/loop_shape.rs`,
`src/analysis/transition_graph.rs`, benchmarks.

---

## Phase 3 — Casting De-duplication (native-wins)

**Status: DONE (2026-08-04, commit `bcb3cebe`).**

**Goal:** delete the hardcoded `IntToStr#`/`FloatToStr#`/`ToString#` arms and
route casting through the casting graph's existing lanes.

**Current state (verified):**
- `graph.rs:195-196` already declares `Int ⇄ String = ExtCall("int_to_str")` /
  `ExtCall("str_to_int")` — the *generic* "convert base protocol to base
  protocol" machinery.
- `emit_expr.rs:767-782` has a **redundant hardcoded** arm: when the target is
  `#String`/`#Data` and source is `i64`, it emits `call ptr @__int_to_str__`
  directly, bypassing the graph it duplicates.
- `IntToStr#` was removed from `intrinsic_signatures.rs` in `ffd55677` but
  `std/string.bv:248` still calls it → **latent break** (would be rejected as
  "unknown intrinsic" if `to_string` were exercised).

**Work:**
- Delete the hardcoded arms; route `(n as String)` / `(String) n` through
  `emit_cast_path` → the `Int ⇄ String` lanes.
- Fix `std/string.bv` `to_string` to use the casting path (or a Briv digit
  loop — see Phase 4).
- **Creates the `.ebv` seam:** the lane *implementation* becomes swappable —
  `.bv` uses `ExtCall("int_to_str")` (C), `.ebv` uses a Briv digit loop.

**Tests:** `(n as String)` produces the same IR as today (byte-identical before
the `.ebv` seam); `to_string` works; no `IntToStr#` references remain.

**Files:** `src/backend/llvm/emit_expr.rs`, `lib/std/string.bv`,
`src/backend/llvm/intrinsics.rs`.

---

## Phase 4 — `.ebv` Freestanding Heap + Stdlib: The Independence Proof

**Status: IN PROGRESS (2026-08-04).** Wiring done (`70f596f9`, `c7f25a95`);
static bump arena + heap-rejection downgrade done (`f2b57043`). Remaining: the
C-free `.ebv` stdlib/prelude (see "Remaining" below).

**Goal:** prove Briv can hold its own as a systems language without C — a real
`.ebv` freestanding target with a **heap** (static bump arena, no `@malloc`),
String/Data support, and the string-conversion stdlib implemented in Briv.

### Why `.ebv` rejects heap is WRONG (the provenance)

`check_embedded_restrictions` (commit `28543c3b`, 2026-06-19) rejects heap types
(`#String`/`#Data`/List/HashMap/…) on `.ebv`. This is a **vestige of the
`.ebv`/`.cbv` entanglement**, not a freestanding design:

- The restriction and the backend split (`docs/plans/2026-06-19-backend-routing-
  async-await.md:156`, "Split `.ebv`/`.sebv` from `.cbv`") are the **same date**.
  The checker was written while `.ebv` still shared the circuit/hardware mindset
  with `.cbv` — the routing plan even marked `"embedded" => "llvm.toml"` as
  "wrong for both" (line 153) before the split.
- After the split, `.ebv` → LLVM embedded (heap-capable), `.cbv` → CIRCT circuit
  (genuinely no heap — it synthesizes hardware). The heap rejection should have
  gone with `.cbv`.
- The documented `.ebv` vision REQUIRES a heap and Strings:
  - `docs/plans/2026-08-03-data-briv-config-and-board-hardware-map.md:81` —
    `.ebv` is explicitly `Malloc#`→ bump allocator, `Print#`→ pure Briv
    formatting + `write`, `Now#`→ freestanding clock.
  - `docs/plans/2026-06-23-arena-allocation.md` — the bump-allocator design.
  - `docs/plans/2026-07-26-tamer-zero-c-and-static-memory.md:992` — `SysCall#`
    with inline asm, "no C runtime needed".

**Decision (2026-08-04):** heap is RIGHT for `.ebv`. The rejection belongs to
`.cbv`. Briv should PROVIDE an allocator on bare metal, not forbid Strings.

### Current state (verified)

- `is_embedded_extension()` was **documented but nonexistent** (backend-strategy
  said it existed; grep found nothing). The `is_embedded` flag existed on the
  backend but was never wired.
- **Latent link-error bug found+fixed** (`70f596f9`): the casting graph declares
  10 `ExtCall` string-lane symbols; only `int_to_str` existed in `briv_rt.c`.
  The other nine (str_to_int, uint_to_str, str_to_uint, float_to_str,
  str_to_float, str_to_bool, bool_to_str, str_first_char, char_to_str) were
  undefined — `(s as Int)` etc. LINK-ERRORED for .bv and .ebv alike. Added the
  C definitions (String ABI = ptr to [len: i64][bytes]; Float lanes use the
  32-bit float ABI) + the lane declares.
- **Wiring done** (`c7f25a95`): `with_prefer_ebv` in the import resolver (an
  .ebv target picks the .ebv stdlib variant over .bv); `.ebv` activates
  `with_embedded_mode(true)` at the four LLVM backend construction sites; the
  cast-lane declares are skipped when the program defines the symbol.
- The arena machinery (`emit_arena_alloc`, `mod.rs:1280`) exists but is
  `@malloc`-backed — the `.ebv` branch replaces `@malloc`/`@free` with a static
  `.bss` buffer.

### Work

**1. Static bump arena for `.ebv`** — ✅ DONE (`f2b57043`)
- `emit_arena_init` embedded branch: the bump pointer targets a static
  `@embedded_heap` global (configurable size = `ir-lowering arena_initial_size`,
  default 64KB) — no `@malloc`.
- `emit_arena_alloc` embedded: grow path yields null (no `@realloc` on bare
  metal; fixed-size heap).
- `emit_arena_fini` embedded: skip `@free` (static global lives for the
  program's lifetime).
- `Malloc#`/`Alloc#` in `.ebv` mode dispatch to the static arena; the `.ebv`
  default becomes `AllocStrategy::Arena`.

**2. Downgrade `check_embedded_restrictions`** — ✅ DONE (`f2b57043`)
- Heap types (`#String`/`#Data`/List/…) are now a **TargetWarning** (finite
  static arena), not a TargetError.
- **Threading intrinsics stay rejected** (genuine: bare metal has no threads).
  Recursion check stays (no stack growth).
- Tests: `test_embedded_string_state_uses_static_heap` (IR has `@embedded_heap`,
  no `@malloc` call), `test_embedded_string_state_warns_not_errors`.

**3. `.ebv` string-conversion stdlib** — ⏳ REMAINING
- `lib/std/conversions.ebv` provides the cast-lane symbols (`int_to_str`,
  `str_to_int`, `uint_to_str`, `str_to_uint`, `float_to_str`, `str_to_float`,
  `str_to_bool`, `bool_to_str`, `str_first_char`, `char_to_str`) as **Briv
  defns** over the arena. The declare-guard (`c7f25a95`) already skips the
  backend declares when the program defines the symbol.
- `with_prefer_ebv` means an `.ebv` program importing `std/conversions` picks
  the `.ebv` variant over a `.bv` one.

**3b. C-free `.ebv` prelude** — ⏳ REMAINING (the freestanding-link blocker)
- The `prelude` plugin (`plugins/parsed/prelude.bv`) imports `std/os/*`,
  `std/io.bv`, `std/env.bv` — these transitively import `std/briv_rt.bv`
  which does `import "link/briv_rt.c"`, pulling `briv_rt.o` into EVERY build
  including `.ebv`. Verified: `nm` on an `.ebv` binary shows `int_to_str` (a
  briv_rt.c symbol).
- For a true freestanding `.ebv`, the prelude must be a **C-free variant**
  (import only `std/types/bootstrap.bv` + C-free compute modules), and the
  `.ebv` build must skip `briv_rt.c` in `collect_extra_objects`. This is a
  stdlib restructure (os/* and io/env are C-backed).

**4. Stale docs**
- `docs/architecture/backend-strategy.md` — fix the `is_embedded_extension()` /
  `is_circuit_extension()` routing (describes code that doesn't exist); update
  the restrictions table ("Reject heap types ✅" → "Static bump arena").

### Measure

A `.ebv` firmware program with String state + `(n as String)` / `(s as Int)`
compiles, links, and runs with `briv_rt.c` NOT linked (freestanding).

---

## Phase 5 — `Print#` Split

**Goal:** separate observability (intrinsic) from formatting (stdlib).

**Design:**
- `Print#` stays the observable intrinsic (pinned by design). It writes to a
  **`#StdOut`/`#StdErr`/`#StdIn` stream symbol**, not `STDOUT_FILENO` directly —
  the stream symbol resolves per target (OS fd / WASM fd / `.ebv` transport).
  This is how `Print#` avoids being locked to stdio.
- **Formatting moves to stdlib** — the Phase 4 digit loops (`int_to_str`,
  `float_to_str`). The `.ebv` print path is pure Briv formatting + a raw
  `write` syscall.
- `out` (Phase 2) keeps the stdlib print function from being elided.

**Files:** `src/backend/llvm/intrinsics.rs` (Print# stream resolution),
`lib/std/out.bv` (or `.ebv`), the stream-symbol config.

---

## Phase 6 — `ExtCall#`/frgn Unification (converge with GLUE)

**Status:** deferred — the GLUE tree (`glue-host-callable` branch, worktree
`../briv-compiler-glue-host`) is landing `2026-08-03-glue-folders-node-bridge.md`.
This phase records the convergence contract; **no code now**.

**Contract:**
- `ExtCall#(sym, args…)` = the generic foreign-call *process*: call a foreign
  symbol, converting args/return via protocol paths.
- `frgn` = declaration sugar that lowers to `ExtCall#` (signature + provenance +
  fallback declared once; every call emits the intrinsic).
- GLUE's `ResolvedFrgn::Bridge` protocol-path machinery (`frgn-export-glue-architecture.md`)
  is the shared converter for both `ExtCall#` (import) and `export` (export).
- The GLUE plan's per-language `lib/glue/<lang>/glue.dbvl` + generic renderer +
  toolchain-recipe-in-config makes the converter concrete.

When the GLUE plan lands, re-evaluate: does `frgn` → `ExtCall#` unification land
there or here?

---

## Explicitly Out of Scope

**`alloc`/`load`/`store` keyword-ification** — verified to add no value:
- `Load#`/`Store#` emit `load iN`/`store iN`, which `*p`/`&x` (`Deref#`/`AddrOf#`)
  already express. Their only real use today is observability anchors — replaced
  by `out` (Phase 2).
- `Alloc#` is used only in `lib/tamer/` and `lib/std/memory/arena.bv`; `var`/state
  declarations already allocate implicitly.
- `free`/`keep` remain the memory-model keywords. Everything mechanical stays
  operator/intrinsic.

---

## Success Metrics

1. `(Type) expr` and `expr as Type` are interchangeable; tests green.
2. `out` works for `defn`/`node`/`txn`/`let`; DCE keeps `out` calls; folding
   respects them; benchmarks migrated to `out` (no `Store#`-as-anchor left).
3. `IntToStr#`/`ToString#`/`FloatToStr#` gone; casting routes through the graph;
   `std/string.bv:248` latent break fixed.
4. First real `.ebv` stdlib exists; `is_embedded_extension` wired; `briv_rt.c`
   shrinks; `--no-stdlib` string formatting works.
5. `Print#` writes to a `#StdOut` symbol; formatting is stdlib.
6. GLUE convergence contract recorded; no divergence with `glue-host-callable`.

---

## Docs to Update (in the same commits as the structural changes)

- `docs/architecture/intrinsics-vs-stdlib.md` — add the `out` keyword as the
  stdlib observability pin; update the three-layer table.
- `docs/architecture/agent-reference.md` — keyword section (`seq`/`vol`/`out`),
  the "keywords are pins" framing, `#out` anti-pattern line updated to
  "use `out`" (line 361 currently says use `!> observable: true`).
- `docs/architecture/casting-protocol.md` — Int⇄String lanes as the casting
  generalization; `.ebv` lane impl seam.
- `docs/architecture/backend-strategy.md` — `.ebv` freestanding wiring reality.
- This plan (the authoritative record).

---

## Verification

- `cargo test --lib` green after each phase.
- `cargo build` no new warnings.
- Praetor on changed dirs (no NEW diagnostics vs the pre-change baseline).
- Kani harnesses for any new safety-critical code.
- Benchmark suite: `bash benchmarks/build_and_bench.sh --runtime` — zero MISMATCH,
  no regressions (baseline table recorded before each perf-relevant change).
- Update BUGS.md for the `std/string.bv:248` latent break and any surfaced bugs.

---

## Sequencing

1. Phase 1 (`(Type)` cast) + Phase 2 (`out` keyword) — foundational, bundle.
2. Phase 3 (casting de-dup) — needs Phase 1's uniform cast syntax.
3. Phase 4 (`.ebv` stdlib) — needs Phase 2 (`out` keeps prints alive) and Phase 3
   (digit loops need a lane to live in).
4. Phase 5 (`Print#` split) — needs Phase 4's formatting.
5. Phase 6 (`ExtCall#`/frgn) — waits for GLUE landing.
