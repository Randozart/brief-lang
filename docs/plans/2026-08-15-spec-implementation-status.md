# Normative SPEC Implementation Status

**Date:** 2026-08-15
**Supersedes:** `docs/plans/2026-08-05-spec-implementation-status.md`
(historical — kept as the record of the 2026-08-05 snapshot).
**Normative source:** `spec/SPEC.md` (25 sections, draft 2026-08-05)
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

## Section status (2026-08-15)

| SPEC § | Normative section | Status | Notes |
|---:|---|---|---|
| 1 | Scope and conformance | In progress | Conformance runner `src/conformance.rs::discover_active_sources()` active (Phase 0); fixture inventory from active sources |
| 2 | Core model | In progress | Interpreter semantic-value migration complete (2026-08-06). Active track: fundamentals-as-types — `Data` universal **reflective floor** (decided 2026-08-15: not a supertype, no universal inheritance edge), `Bit<N>` unifies `Bits` (Data→Blob rename landed 2026-08-15) |
| 3 | Source files and target profiles | In progress | `.c`/`.sbv` removed; target-selected `.ebv`/`.bv` variants (SPEC §3.3) shipped 2026-08-09. `.f` layout frontend pending (Phase 15) |
| 4 | Lexical conventions | In progress | Phase 3 dead tokens removed; layout keywords `pack`/`seq`/`union`/`atomic`/`spec` (2026-08-13) and `coll` (2026-08-15) added; `Bits`→`Bit<N>` rename pending (fundamentals-as-types) |
| 5 | Delimiters and arrows | In progress | Phase 3 removals done; delimiter-semantic-load rule (§5.1) enforced |
| 6 | Grammar overview | In progress | Phase 2/3 done |
| 7 | Modules and imports | In progress | Phase 11 (2026-08-06) + slice 2 (2026-08-09): selective renames, glob rejection, import-collision errors, `export import` re-export, `:` module alias, deterministic resolved-path records, cross-module impl coherence, target-selected variants |
| 8 | Declarations | In progress | trait/impl + relationship list + coherence + structural conformance (Phase 5); `meld` removed (staged-reject); union untagged overlay + atomic field modifier; init-kind box/spill classification phases 1–4 (2026-08-14); `coll` obj/struct/seq + capacity intrinsics + Grow/Shrink override (2026-08-14/15). **Grow-on-full is now normative (decided 2026-08-15)** — default `op Grow` doubles capacity when `len == cap`; implementation pending (phi-merge work, coll plan §3.6). Remaining: `Ptr<T>`-backed `coll struct` (documented follow-up) |
| 9 | Functions, txns, nodes, objects, cells | In progress | `rollback`, `exit program`, `endprogram`/`beginprogram` entry-loop (2026-08-06); `term` canonical result placeholder bound to the return type (2026-08-14); closures-as-values + generic txns + generic `defn<T>` dispatch (2026-08-14); `accel` node/txn offload implemented (2026-08-06 — §9.7 Staged marker cleared 2026-08-15). **Default non-offload remark now normative (decided 2026-08-15)** — a keyword-marked body on the CPU path always emits a one-line remark; implementation pending. Remaining: objects/cells lifecycle; prior-state txn expression syntax (§9.3) |
| 10 | Contracts, invariants, watchdogs | In progress | Phase 6 mandatory non-trivial contracts on node/txn/asm; explicit `[true][true]` rejected |
| 11 | Control flow | In progress | `rollback`, `exit program`; `endprogram`/`beginprogram` (SPEC 11.5 no longer staged); `defer`/`mutex`/`barrier<group>` (2026-08-09, SPEC 11.6 no longer staged). No `main` |
| 12 | Concurrency and task lifecycle | In progress | `spawn Obj(...)` pools (2026-08-07); `spawn defn(...)` task handles, `await`, `free`/`keep` (2026-08-09); no-implicit-concurrency gate + sync-group classification; task cancellation proof + Kani gate proofs (2026-08-09). Remaining: objects/cells lifecycle, deterministic scheduler interleaving mode |
| 13 | Triggers and external events | In progress | `#assume_event` dead data removed; port contracts Phase 10 |
| 14 | Ownership, lifetimes, effects | In progress | UOL codified — three-surface rule + UFCS (2026-08-13/14); collection-op `Count#` intrinsics under UOL (2026-08-13). Remaining: ownership algebra + `.s` enforcement (Phase 9) |
| 15 | Expressions and operations | In progress | declared variant ops lower to their functions (2026-08-06); UOL generative `OpName#` dispatch + UFCS (2026-08-13). Remaining: effects/access-shape carrying, dyn Trait |
| 16 | Literals, ranges, slicing | In progress | byte literals, raw strings, Python slices, `..=` inclusive ranges, boolean-mask indexing, iterable ranges + foreach, named selectors, multi-dim arrays, const-generic member dim substitution (2026-08-06/07); unconstrained list literal requires type annotation (SPEC §16.3, 2026-08-15). Remaining: list-literal→`Int[N]` coercion blocking `coll struct` construction (next in the coll track) |
| 17 | Reflection | In progress | runtime reflection; `.^^Element` frozen element-type descriptor (2026-08-14); `Abs#` unification + four bit intrinsics (SPEC §17.3, 2026-08-13); runtime `Size`→`Count#` split. Remaining: const generics — **required for the generic `coll struct Fixed<T,N>` (SPEC §8.10 stays normative; 2026-08-15 decision: spec-outlined is work-to-do, not a spec edit)** |
| 18 | Compile-time execution and macros | In progress | escaping closures + interpreter user-fn support (2026-08-06). Remaining: `$name`/`name!` macros + stage timing |
| 19 | Foreign functions, export, GLUE | In progress | GLUE FFI folders; frgn/export + `--shared` PIC; four provenance forms (2026-08-09); frgn grammar conformance — local-name + `:` external-symbol binding, `variadic` named param, MMIO `@` rejection (SPEC 19.1/19.4/19.7); `meld` architecture removed (SPEC 19.6); `fallback` removed → optional frgn + `feature.^^Available` (SPEC 19.3); GLUE config `glue.dbvl → glue.dbv`. Remaining: ownership-for-pointer/aggregate-boundaries gate (rides Phase 9) |
| 20 | Assembly declarations | In progress | contracts mandatory on asm (Phase 6). Remaining: full effect profile (Phase 16) |
| 21 | Rendered Briev | In progress | `render Name`, `b-when` (Phase 3). Remaining: full lifecycle (Phase 14) |
| 22 | Data Briev | In progress | `.dbv`/`.dbvl` v2 parser; schema validation (required/unknown fields, type conversion, constraints, optional fields, key presence + doc-wide uniqueness); canonical serialization (deterministic order/quoting/numeric spelling/map-key sort, round-trip idempotent); `briev check` extension dispatch + schema-import resolution; 20 stale `.dbvs` deleted (all 2026-08-09). GLUE `.dbv` stays (SPEC 22.7) |
| 23 | Diagnostics, tooling, documentation | In progress | Phase 1 vocab + LSP + grammar; Phase 6 helpful-messages rule; 2026-08-06 diagnostics sweep; tutorial + syntax highlighter updated with `coll`/layout keywords |
| 24 | Standard-library boundary | In progress | **SPEC §24 rewritten (2026-08-15)** to codify the coll exception: the compiler never matches collection *type names*; `coll` is the one sanctioned compiler-owned scaffold (hidden length/capacity, op surface, grow-on-full), while collection *policy* (hashing, load factor, rehash) stays in stdlib. "No compiler collection knowledge" enforced via rule-14 grep (zero `Type::Custom ==` matches in `src/backend/llvm/` + `src/glue/`). Formal conformance pass pending (Phase 18) |
| 25 | Implementation staging | In progress | `SyntaxError::StagedFeature` live since Phase 0 (meld, removed lexical forms). 2026-08-15: the last two SPEC `Staged` markers (module-top-level `!>` §8.9, `accel` §9.7) cleared — implementation matches SPEC; no active Staged markers remain |

## Decisions (2026-08-15)

Locked in review with the requester; all documentation updated in the same
family:

1. **coll grow-on-full is normative.** The scaffolded `InsertAt`'s default
   `op Grow` doubles capacity when `len == cap` before the store — an insert
   past capacity is never an out-of-bounds write. SPEC §8.10/§15.2 updated;
   implementation pending (see `docs/plans/2026-08-15-coll-grow-on-full.md`).
2. **SPEC §24 rewritten** to codify the coll exception: no type-name matches;
   `coll` is the sanctioned compiler-owned scaffold; policy stays in stdlib.
3. **Generic `coll struct Fixed<T,N>` stays normative.** The spec is the
   contract — spec-outlined is work-to-do (const generics), not a spec edit.
4. **`accel` non-offload remark is mandatory, not opt-in.** A keyword-marked
   body on the CPU path always emits a one-line compile-time remark; verbose
   `accel_report` adds detail. SPEC §9.7 updated; implementation pending.
5. **`Data` is a reflective floor, not a supertype.** Every value is
   observable as raw storage (treat-as-bits view); no universal inheritance
   edge enters the casting graph. SPEC + bits-thesis + agent-reference +
   hash-words + fundamentals plan updated.
6. **Roadmap: finish the coll track first.** Grow-on-full → coll-struct
   construction (list-literal→`Int[N]`) → const generics for `Fixed<T,N>`;
   then the OPEN BUGS.md stdlib entries, iterable slice-6, and finally
   fundamentals-as-types (with `Data` as reflective floor pinned).

## Dropped row

The prior tracker's §26 "Casting and protocols" row is **dropped**: SPEC.md has
25 sections and no standalone casting section. Casting is normative through
§2 core model, §15.1 operation dispatch, and
`docs/architecture/casting-protocol.md` — the ghost row referenced a section
that no longer exists.

## Fixture runner

Normative SPEC examples become conformance fixtures once the construct is
implemented. Until then the compiler must reject the construct as staged, not
accept a partial subset. The fixture inventory is produced by
`src/conformance.rs::discover_active_sources()`.
