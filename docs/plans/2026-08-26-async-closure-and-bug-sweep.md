# Async Arc Closure + Open-Bug Sweep

**Date:** 2026-08-26
**Track:** A then B — close the async Phase D arc with permanent verification,
then sweep the five open bugs from `docs/plans/planned-features-tracker.md`.

## Baseline

- Suite: 1960 tests green at `ff5e01c7`.
- Phase D slice 2/3 landed (`340c43c9`): compiled port events verified
  ad-hoc in `/tmp/opencode/ae-verify.bv` (stdout `17`). No PERMANENT e2e
  exists yet — plan §Verification item 2 was closed informally.

## Research findings (2026-08-26 session, evidence file:line)

| Tracker bug | Reality found |
|---|---|
| 1. Round-trip proofs silently skipped | OPEN, two sites: `src/analysis/protocol_graph.rs:154-160` (backend) AND `src/protocol_verify.rs:73-86` (interpreter-side eval failure also silently Ok). Missing bodies: `lib/std/protocols.bv:7-17` declares 4 conversion bindings with zero defn bodies anywhere. |
| 2. Silent width/alignment fallbacks | PARTIALLY mitigated (register_types.rs records warnings since Phase 3). Truly silent remainders: `type_universe/mod.rs:246,302-303` (String bytes=8), inline `ResolvedType{alignment:8}` at llvm/mod.rs:2635,2900, enum-handle shape register_types.rs:130-143. |
| 3. `%ac0` i64-vs-ptr | FIXED 2026-08-18 w/ regression test (`tests.rs:3728`); stale OPEN duplicate BUGS.md:4551. |
| 4. Plain txn → empty program | Warning mitigation landed 2026-08-26 (`warn_undispatched_txns`, mod.rs:128-176) but fires ONLY on EmitSequentialSsa path (mod.rs:4116). |
| 5. Duplicate Webstack arms | Closed upstream; cosmetic dup doc-comment compile.rs:1651-1652. |

## Part A — Async arc closure

### A1. Permanent compiled-port e2e

- New `examples/async-events-compiled.bv`: the acc twin of async-events.bv.
  Consume blocks on unready port; produce fires; scheduler wakes it;
  `endprogram println!(acc)` where acc = consume(7) + produce(1)*10 → `17`.
- New `tests/async_compiled_events_test.rs` (Pattern A, mirrors
  termination_diagnostics_test.rs):
  - builds via `brievc build <bv> --llvm --out tmp`
  - links with the harness-exact clang command + lib/runtime/briev_rt.c
  - runs binary, asserts stdout == "17"
  - skips cleanly when clang absent (house pattern)
- Interpreter parity probe on the SAME .bv source asserting v==7,
  produced==1 (extends the existing `async_phase_b_example_runs` probe).

### A2. `.^Ready` gated top-level read e2e

- Compiled binary asserting a gated read observes false→true across a fire.
  If a top-level node/twin cannot express this deterministically today,
  scope-note it and test what is expressible (task-visible gating).

## Part B — Open bugs

### B4. warn_undispatched_txns hoisted ahead of dispatch-mode selection
Hoist the call so enum/reactor/fold branches all get it (mod.rs ~4108).
Regression test: non-reactive plain top-level txn warns under each mode.

### B2. No unrecorded representation fallbacks
Route the silent remainders through the recorded-warning path:
String bytes at type_universe/mod.rs:246+302, inline alignment literals
llvm/mod.rs:2635+2900, enum-handle shape register_types.rs:130.
Explicit-authority types keep behavior; only *implicit* defaults gain
recorded warnings. SPEC §2.1 truth.

### B1. Round-trip proofs never silently skip
1. Implement the four bodies in `lib/std/protocols.bv`:
   ascii_to_utf8, utf8_to_ascii (trivial byte maps),
   utf16_to_utf8 (surrogate-aware UTF-16→UTF-8),
   Posit32_to_IEEE754 (posit decode→f32 bits). Heaviest item — if outgrows
   session, stop and report rather than half-do.
2. Flip both skip arms to hard errors:
   protocol_graph.rs `_ =>` fallthrough and protocol_verify.rs eval-failure
   Ok(()) paths must fail compilation with what/why/fix messages.

### B3+B5. Bookkeeping
Close stale BUGS.md %ac0 duplicate (:4551), remove dup doc-comment
compile.rs:1651-1652, update tracker statuses for bugs 1/2/4.

## Verification

- cargo test --lib green incl. sweep; praetor clean on changed dirs.
- Baseline table not required (no performance-affecting change intended);
  compare_baseline run if any benchmark-relevant path is touched.

## Non-goals

- Cells compiled stays off; json.bv recursive-generic blocker untouched;
  VM parity fixtures stay foreign-track.

## Undo

Each part commits separately; revert that commit only.

## Milestone log — executed 2026-08-26

| Part | Commit | Result |
|---|---|---|
| A1+compiled tests | `2f34bfc7` | async-events-compiled.bv → `17`; async-ready-gate.bv → `111`; CLI+clang integration tests; interpreter parity probes both files. Suite 1972 green. |
| (A1 bonus) SSA guard fix | same commit | emit_expr read top-level fields via phi_field_regs only — a mid-body guard merge in pending_phi_backedge was ignored by every later statement (second when chained off stale pre-guard SSA). Reads prefer the live chain; header phi fallback. |
| B4 | `84a85852` | warn_undispatched_txns hoisted before dispatch-mode selection; library shim silence kept; +2 unit tests. |
| B2 | `e614a18e` | record_structural_layout shared helper; Obj/TypeDef backfill sites migrated; recorded deduped warnings. |
| B1 (part) | this commit | protocol_verify.rs silent Oks → hard errors with what/why/fix. |
| B3+B5 | this commit | BUGS.md %ac0 tombstone→resolved entry; duplicate compile.rs doc-comment removed; tracker statuses synced; BUGS.md #1/#2 statuses updated. |

**Deferred (B1 remainder):** backend skip-arm gate flip REQUIRES the four
codec bodies + SPEC §8.7 explicit axiom marker first (flipping alone bricks
all stdlib imports — Rule 11 prereqs). Tracked as one coherent session in
planned-features-tracker.md bug #1.

Session diagnostics worth noting: the async-ready-gate example surfaced the
folded-shape SSA staleness bug precisely because guards interleave assigns to
the same top-level — ungated twins hid it. Compiled-vs-interpreter parity on
per-example probes is now the standing acceptance pattern for scheduler work.
