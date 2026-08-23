# Systems-Language Readiness Assessment — Briev

**Date:** 2026-08-22
**Basis:** full SPEC↔codebase conformance audit (2026-08-22) plus hands-on
verification of compiled behavior across sessions 1–3. Historical document —
do not retroactively edit; supersede with a new dated assessment.

## Verdict

For deterministic, verifiable, close-to-metal programs — MMIO drivers,
packed-register work, verified terminating kernels, embedded logic — Briev
is **usable now**, and its contract-proof model is ahead of anything
mainstream. Roughly **two-thirds of the SPEC's systems promise executes
today**. The remaining third concentrates in true async scheduling, the
port/component model, LLVM dyn dispatch, and ecosystem hardening — runtime
and breadth gaps, not language-design gaps. The spec itself held up under a
full audit needing only local repairs.

## What's production-shaped

| Pillar | State | Evidence |
|---|---|---|
| Correctness apparatus | Strongest pillar, rare in the space | Contracts mandatory + proven (termination, goal-reachability, tautology rejection); XOR concurrency gate global; `.s` strict profile escalates every fallback |
| Low-level determinism | Strong | `Bit<N>` exact widths, pack/seq structs byte-exact, atomic fields, `Load#`/`Store#` volatile MMIO, `asm<target>` with mandatory contracts, `Ptr<T>` typed addresses |
| Compile pipeline | Mature | parse → typecheck → LLVM → `-O3` native, verified end-to-end repeatedly (fizzbuzz, enemy_swarm, structural_sums, mask_select, slice_state all byte-correct) |
| Unusual strengths | Real differentiators | GPU offload (SPIR-V collect+embed), embedded/board targets (`.ebv`, STM32 configs), garbage scheduler (proof-directed frees, not GC), Data Briev config dialect |

## What's young or missing

| Gap | Why it matters |
|---|---|
| Concurrency design-complete, runtime-eager | Tasks execute inline; `free`/`yield;` enforce ownership but cancel nothing yet. The reactive story is the spec's centerpiece — biggest executable gap |
| obj/cell ports absent (Phase 7) | No `Event<T>` component wiring = sealed-state-machine narrative doesn't run |
| dyn compiles to nothing (5c) | Trait objects interpreter-only |
| Backend composure | Latent bugs surface when shapes compose (two-guard repro, match phis earlier) — single-idiom solid, combinations still finding edges |
| Ecosystem | stdlib minimal; no debugger story, package manifest unproven; conformance sweep not CI-wired |
| Perf parity | Harness + baseline discipline exist; hot-loop parity regained historically but not continuously guarded |

## Highest-leverage moves (in order)

1. **Phase 7 ports** — unlocks the reactive identity of the language.
2. **Async scheduler arc** — gives `yield;`/`free task` their teeth; turns
   the checkpoint design from discipline into mechanism.
3. **CI conformance sweep** (`discover_active_sources` wiring) — so shape-
   composition regressions like the two-guard find themselves.
4. Continuous bench guarding via `compare_baseline.sh` on every emission-
   touching phase.
