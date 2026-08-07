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
| 2 | Core model | In progress | Token-aware layout frontend (`.f`) wired in Phase 15; interpreter Value migration (Phase 17, slices A–I) complete 2026-08-06 |
| 3 | Source files and target profiles | In progress | `.c`/`.sbv` removed; profiles per §3.2; `.f` layout frontend + profile detection in Phase 15 |
| 4 | Lexical conventions | In progress | Phase 3 removed dead tokens, `sig`, `++`, pragmas, `prop`, `Ptr!`, `@`/prefix literals, width suffixes |
| 5 | Delimiters and arrows | In progress | Phase 3 removed `|>`; `<:`/`:>` and `++` removed |
| 6 | Grammar overview | In progress | Phase 2 canonical formatter; Phase 3 `render Name`, `b-when` |
| 7 | Modules and imports | In progress | Phase 11: selective rename `{ Local: Exported }`, glob rejection, import-collision errors, `export import` re-export (2026-08-06); remaining: `:` module alias, configured-root determinism records |
| 8 | Declarations | In progress | Phase 5: `trait`/`impl` declarations, relationship list `type X: Parent, Trait, #Proto`, impl coherence, structural conformance; `meld` deferred to Phase 12 |
| 9 | Functions, transactions, nodes, objects, cells | In progress | `escape`→`rollback`, `term!`→`exit program` done; `endprogram` real process exit + `beginprogram` entry-loop shipped 2026-08-06 |
| 10 | Contracts, invariants, watchdogs | In progress | Phase 6: mandatory non-trivial contracts on node/txn/asm; explicit `[true][true]` rejected |
| 11 | Control flow | In progress | `rollback`, `exit program` done; `endprogram`/`beginprogram` entry-loop implemented 2026-08-06 (SPEC 11.5 no longer staged); `defer`/`mutex`/`barrier` Phase 11; no `main` — first-firing-node entry |
| 12 | Concurrency and task lifecycle | Not started | Phase 10 (`spawn`/`await` handles) |
| 13 | Triggers and external events | In progress | `#assume_event` dead data removed; port contracts Phase 10 |
| 14 | Ownership, lifetimes, effects | In progress | Phase 9: garbage scheduler wired across fold paths + only soundly-emittable frees planned (2026-08-06); ownership algebra + `.s` enforcement pending |
| 15 | Expressions and operations | In progress | Phase 5: op elaboration — declared variant ops lower to their functions (`resolve_binary_op_binding` + `elaborate_ops`, 2026-08-06); remaining: effects/access-shape carrying, dyn Trait |
| 16 | Literals, ranges, slicing | In progress | width suffixes removed; `#b` Data byte literals + `#r` raw strings + Python slices shipped 2026-08-06; `..=` inclusive range patterns shipped 2026-08-06 (DotDotEq token, `Pattern::RangeInclusive`, interpreter + codegen match lowering); boolean mask indexing shipped 2026-08-07 — `data[mask]` → Data (byte gather), `Int[N][mask]`/`Bool[N][mask]`/`Float[N][mask]` → `List<T>` (typed + f32 gathers), heap `List<Int>`/`List<Float>` → `List<T>` (i64 bit-pattern slots); masks = Bool list literal or `Bool[N]` state field; Float64 vector masks are a hard error; iterable ranges + `foreach` shipped 2026-08-07 (§11.4 — `Expr::Range`, `Value::Range`, counted-loop codegen for ranges AND collections — heap List, Data bytes, vector state fields — via index loops); remaining: named selectors, const generics |
| 17 | Reflection | In progress | Phase 7: `^^Type` descriptor (category code) + value-side eval shipped 2026-08-06; remaining: const generics |
| 18 | Compile-time execution and macros | In progress | Phase 8: escaping closures (interpreter + codegen env blocks) + interpreter user-fn support shipped 2026-08-06; `$name`/`name!` macros + stage timing pending |
| 19 | Foreign functions, export, GLUE | In progress | GLUE FFI merged (per-language glue folders, native extensions for hosts); frgn/export + `--shared` PIC `.so`; four provenance forms Phase 12 |
| 20 | Assembly declarations | In progress | Phase 6: contracts mandatory on asm; full effect profile Phase 16 |
| 21 | Rendered Briv | In progress | Phase 3 `render Name`, `b-when`; full lifecycle Phase 14 |
| 22 | Data Briv | Not started | Phase 13 (`.dbv`/`.dbvl` modes) |
| 23 | Diagnostics, tooling, documentation | In progress | Phase 1 vocab + LSP + grammar; Phase 6 helpful-messages rule; docs Phase 20; 2026-08-06 diagnostics sweep (op-target, closure-as-value, scheduler-leak, GetEnvInt#, lambda arrow, interpreter user-fn) |
| 24 | Standard-library boundary | Not started | Phase 18 (no compiler collection knowledge) |
| 25 | Implementation staging | In progress | `SyntaxError::StagedFeature` added in Phase 0 |
| 26 | Casting and protocols | In progress | Phase 5: one cross-protocol edge per written `as` enforced |

## Fixture runner

Normative SPEC examples become conformance fixtures once the construct is
implemented. Until then the compiler must reject the construct as staged, not
accept a partial subset. The fixture inventory is produced by
`src/conformance.rs::discover_active_sources()`.
