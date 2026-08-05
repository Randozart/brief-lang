# Normative SPEC Implementation Status

**Date:** 2026-08-05
**Normative source:** `spec/SPEC.md`
**Plan:** `docs/plans/2026-08-05-implement-normative-language-spec.md`
**Policy:** A normative section is either Implemented (matching reference
interpreter and applicable active backends), Staged-rejected (parser emits
`SyntaxError::StagedFeature`), or in progress. No section may silently behave
differently from the SPEC.

Status legend:

- **Not started** — no implementation work yet.
- **In progress** — partial implementation being migrated.
- **Implemented** — semantic behavior matches SPEC and reference interpreter.
- **Staged-rejected** — normative but not implemented; compiler must reject
  with `SyntaxError::StagedFeature` per SPEC §25.

| SPEC § | Normative section | Status | Notes |
|---:|---|---|---|
| 1 | Scope and conformance | In progress | Conformance runner in Phase 0 |
| 2 | Core model | In progress | No-layout frontend; interpreter Value migration is Phase 17 |
| 3 | Source files and target profiles | In progress | `.c`/`.sbv` removed; profiles per §3.2 |
| 4 | Lexical conventions | In progress | Phase 3 removed dead tokens, `sig`, `++`, pragmas, `prop`, `Ptr!`, `@`/prefix literals, width suffixes |
| 5 | Delimiters and arrows | In progress | Phase 3 removed `|>`; `<:`/`:>` and `++` removed |
| 6 | Grammar overview | In progress | Phase 2 canonical formatter; Phase 3 `render Name` |
| 7 | Modules and imports | Not started | Phase 11 (`:` binding, no globs, no cycles) |
| 8 | Declarations | Not started | Phase 4 (`type`/`trait`/`proto`/`struct`/`enum`/`impl`) |
| 9 | Functions, transactions, nodes, objects, cells | Not started | Phases 4/8/10 |
| 10 | Contracts, invariants, watchdogs | Not started | Phase 6 (`[true][true]` rejection) |
| 11 | Control flow | Not started | Phase 3 (`rollback`, `exit program`, `defer`, `mutex`, `barrier`) |
| 12 | Concurrency and task lifecycle | Not started | Phase 10 (`spawn`/`await` handles) |
| 13 | Triggers and external events | Not started | Phase 10 (`trg`, remove `#assume_event`) |
| 14 | Ownership, lifetimes, effects | Not started | Phase 9 (ownership algebra, effects) |
| 15 | Expressions and operations | Not started | Phase 5 (`op` resolution, no `++`) |
| 16 | Literals, ranges, slicing | Not started | Phase 7 (`#r`/`#b`, Python slices, const dims) |
| 17 | Reflection | Not started | Phase 7 (`.^`/`.^^`) |
| 18 | Compile-time execution and macros | Not started | Phase 8 |
| 19 | Foreign functions, export, GLUE | Not started | Phase 12 (four provenance forms, no meld) |
| 20 | Assembly declarations | Not started | Phase 16 (`asm<target>` + effect profile) |
| 21 | Rendered Briv | Not started | Phase 14 (`render Name`, `b-when`) |
| 22 | Data Briv | Not started | Phase 13 (`.dbv`/`.dbvl` modes) |
| 23 | Diagnostics, tooling, documentation | In progress | Phase 1 vocab + LSP + grammar; docs Phase 20 |
| 24 | Standard-library boundary | Not started | Phase 18 (no compiler collection knowledge) |
| 25 | Implementation staging | In progress | `SyntaxError::StagedFeature` added in Phase 0 |

## Fixture runner

Normative SPEC examples become conformance fixtures once the construct is
implemented. Until then the compiler must reject the construct as staged, not
accept a partial subset. The fixture inventory is produced by
`src/conformance.rs::discover_active_sources()`.
