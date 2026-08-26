# Planned Features Tracker

**Created:** 2026-08-26
**Purpose:** Master list of all planned feature work with status, remaining items,
and effort estimates. Updated as work completes.

## Status Key

| Symbol | Meaning |
|--------|---------|
| DONE | Fully complete, no remaining work |
| NEAR | Core work done, small cleanup remains |
| ACTIVE | Work in progress |
| BLOCKED | Waiting on prerequisite |
| TODO | Not started |

---

## Small Effort (≤1 session each)

### 1. Close qualified enum paths bug — BUGS.md
- **Status:** DONE (code landed `38943d5c`)
- **Remaining:** Update BUGS.md entry to reflect resolution
- **Plan:** N/A (bugfix, no plan needed)

### 2. Backend scaffolding §0.4 doc truth sweep
- **Plan:** `docs/plans/2026-08-23-backend-scaffolding-foundation.md`
- **Status:** NEAR — §0.1 (analysis hoist), §0.2 (capability matrix), §0.5 (dead weight), §0.6 (CIRCT install) DONE. §0.3 amended/scope-reduced.
- **Remaining:** §0.4 doc truth sweep — verify architecture docs match implementation
- **Files:** `docs/architecture/backend-contracts.md`, `docs/architecture/backend-type-dispatch.md`, `docs/architecture/backend-architecture.md`

### 3. Kalman/float-math parity doc updates
- **Plan:** `docs/plans/2026-07-31-regain-kalman-float-math-parity.md`
- **Status:** NEAR — core perf done (kalman 1.21x → 1.02x via batch-loop). `float_math_nonzero` stays at 1.21x (body too small).
- **Remaining:** Doc updates per §7: backend-architecture.md dispatch table, features/backend-dispatch.md, final benchmark recording in results doc
- **Files:** `docs/architecture/backend-architecture.md`, `benchmarks/results/`

### 4. Housekeeping directives Part 3
- **Plan:** `docs/plans/2026-08-11-housekeeping-directives.md`
- **Status:** NEAR — Parts 1–2 DONE (webstack arms, todo.rbv migration, b-class/b-style/b-attr)
- **Remaining:** Part 3 (2b2 instance-state) — delegated to `docs/plans/2026-08-11-phase2b2-instance-state.md`
- **Note:** Separate plan owns this; tracker just records the delegation

---

## Medium Effort (2–3 sessions each)

### 5. SPIR-V kernel emission §2.3–2.6
- **Plan:** `docs/plans/2026-08-23-spirv-kernel-emission.md`
- **Status:** ACTIVE — §2.1 (core lowering) + §2.2 (frontend-driven selection) LANDED
- **Remaining:**
  - §2.3: Load#/Store# + intrinsic surface for SPIR-V
  - §2.4: Universe-driven type resolution for SPIR-V types
  - §2.5: Validation harness (spirv-val integration)
  - §2.6: Doc truth sweep
- **Estimated:** 2–3 sessions

### 6. VM compile-tail parity §1.3/§1.5
- **Plan:** `docs/plans/2026-08-23-vm-compile-tail-parity.md`
- **Status:** ACTIVE — §1.1 (arg-drop fix), §1.2 (parity harness), §1.4 (diagnostics), §1.6 (debug reachability) LANDED
- **Remaining:**
  - §1.3: Opcode floor — add opcodes on demand per corpus
  - §1.5: Determinism audit — verify HashMap iteration order doesn't affect VM
- **Estimated:** 2–3 sessions

### 7. Enum variant construction stdlib migrations
- **Plan:** `docs/plans/2026-08-23-enum-variant-construction.md`
- **Status:** ACTIVE — bare variant construction LANDED (`4bb965cb`), qualified paths LANDED (`38943d5c`)
- **Remaining:** Migrate ~20 stdlib files to use enum construction (json.bv, process.bv, string.ebv, error-handling.bv, etc.)
- **Estimated:** 2–3 sessions (mechanical but numerous)

---

## Large Effort (full track, deferred)

### 8. Collections/watchdogs/memory Phase E
- **Plan:** `docs/plans/2026-07-31-collections-watchdogs-memory.md`
- **Status:** Phases A–D DONE (collections, sweep arrays, watchdogs, memory-by-proof all landed with benchmarks)
- **Remaining:** Phase E — `seq`/`vol`/`async`/`sync<g>` modifiers + concurrency gate
  - Lex/parse: new keywords and modifier syntax
  - Analysis: modifier semantics in frontend
  - Codegen: modifier-driven emission paths
  - Regression tests: ensure no perf regressions
- **Estimated:** Full track (multiple sessions)
- **Deferred:** User decision — handle smaller items first

---

## Completed Plans (reference)

| Plan | Date | Notes |
|------|------|-------|
| CIRCT seq.firmem | 2026-08-25 | mem/reg keywords, policy engine, firmem emission, Vivado A/B |
| CIRCT toolchain validation | 2026-08-23 | FSM semantics, simulation parity, watchdog, sized scalars |
| Webstack v2 completion | 2026-08-23 | Legacy emitter deleted, flush-buffer complete |
| Frontend-driven dispatch | 2026-08-06 | Implemented |
| Accel GPU offload | 2026-08-06 | Shipped |
| Endprogram/beginprogram | 2026-08-06 | Shipped |
| String unification & boundary | 2026-08-14 | #String is Iterable<Char>, Abs#/bit-intrinsic migration |

---

## Open Bugs (BUGS.md)

### Planned fixes (have explicit plans)
1. Protocol round-trip proofs silently skipped — missing conversion functions in stdlib
2. Silent representation width/alignment fallbacks — SPEC forbids silent defaults
3. String-param library exports `%ac0` i64-vs-ptr codegen
4. Plain `txn` at top level compiles to empty program
5. Duplicate `BackendKind::Webstack` match arms

### Pre-existing (no immediate fix planned)
1. CIRCT ExportVerilog rejects `hw.module.generated` (FIRRTL_Memory)
2. 134-file conformance sweep residual
3. SSA let-bound tuples segfault edge case

### Resolved
- Qualified enum paths (`Enum::Variant`) — resolved 2026-08-26 (`38943d5c`)
- Guard-fold bug — resolved 2026-08-25
- Non-reactive-txn dispatch gap — warning added 2026-08-26
