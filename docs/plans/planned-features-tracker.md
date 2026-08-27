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
- **Status:** DONE (code landed `38943d5c`, entry updated, stale tracker
  note removed 2026-08-26)
- **Remaining:** None

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

### 5. ~~SPIR-V kernel emission§~~ — COMPLETE 2026-08-27
- **Plan:** `docs/plans/2026-08-23-spirv-kernel-emission.md` — ALL sections landed.
- §2.3 Load#/Store# over SSBO address expressions (`1b26d9bd`);
- §2.4 universe-driven scalars via casting-graph SPIR-V table, signedness
  fixed (`7c18fcf6`); §2.5 validation harness + spirv-dis structural sweep,
  Vulkan smoke probe-gated (`2286f09d`); §2.6 backend-strategy.md v2 table.
- Follow-up (new surface work, not this plan): accel eligibility model does
  not yet classify Load#/Store# bodies as kernels — tracked below.

### 6. ~~VM compile-tail parity~~ — COMPLETE 2026-08-27
- **Plan:** `docs/plans/2026-08-23-vm-compile-tail-parity.md` — ALL sections landed.
- §1.3 resolved WITHOUT new opcodes: corpus demanded const REFERENCES, not
  opcodes — top-level Int consts now inline via const_values resolution
  (`20a6fe24`), unblocking the tamer self-package step; the parity harness
  passes end-to-end for the first time. Opcode floor verified = corpus demand.
- §1.5 determinism audit: one real hazard fixed (field_offset_any sorted);
  all other emission paths were Vec/order-safe (`4d4d8433`).

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

### 8. cbv foreign hardware imports + MMIO pins (plan 2026-08-27)
- **Plan:** `docs/plans/2026-08-27-cbv-foreign-hardware-and-mmio.md`
- **Status:** ACTIVE — Slice C DONE (`d59d1ecf`), Slice A DONE (`fe5dadb7`:
  extern blackboxes + companions + capability gates, 1984 green);
  Slice B (MMIO pins) remaining

## New follow-ups surfaced during completion (2026-08-27)

1. **Accel eligibility vs Load#/Store#** — `analysis/accel.rs` purity model
   does not classify bodies containing Load#/Store# intrinsics as eligible
   kernels; §2.3 lowering is locked by direct-shape tests. Track when GPU
   offload corpus demands it.
2. **Vulkan runner smoke fixture** — probe-gated test exists; wire a real
   fixture once a runner (vkm/vkrunner) is installed.

## Open Bugs (BUGS.md)

### Planned fixes (have explicit plans)
1. Protocol round-trip proofs silently skipped — PARTIAL 2026-08-26: interpreter-side
   silent Ok paths became hard errors (`src/protocol_verify.rs`). Remaining:
   backend missing-body skip arm (`protocol_graph.rs`) → hard error REQUIRES
   the four codec bodies (ascii↔utf8, utf16→utf8, Posit32↔IEEE754) plus an
   explicit `axiom` cast-edge marker per SPEC §8.7 ("visibly declared trusted
   foreign/intrinsic axiom"). One coherent session; flipping alone bricks all
   stdlib imports.
2. Silent representation width/alignment fallbacks — RESOLVED 2026-08-26
   (`e614a18e`): backend backfill sites share record_structural_layout with
   deduped recorded warnings. Enum-handle shape stays explicit+documented
   (boxed {tag,payload} image — declared representation, not a fallback).
3. String-param library exports `%ac0` i64-vs-ptr codegen — closed as stale
   duplicate (fixed 2026-08-18, BUGS.md tombstone points at resolved entry).
4. Plain `txn` at top level compiles to empty program — RESOLVED 2026-08-26
   (`84a85852`): warning hoisted ahead of dispatch-mode selection; fires for
   enum/reactor/SSA modes, silent for library/shared shims.
5. Duplicate `BackendKind::Webstack` match arms — CLOSED upstream 2026-08-26;
   cosmetic duplicate doc-comment removed.

### Pre-existing (no immediate fix planned)
1. CIRCT ExportVerilog rejects `hw.module.generated` (FIRRTL_Memory)
2. 134-file conformance sweep residual
3. SSA let-bound tuples segfault edge case

### Resolved
- Qualified enum paths (`Enum::Variant`) — resolved 2026-08-26 (`38943d5c`)
- Guard-fold bug — resolved 2026-08-25
- Non-reactive-txn dispatch gap — warning added 2026-08-26
