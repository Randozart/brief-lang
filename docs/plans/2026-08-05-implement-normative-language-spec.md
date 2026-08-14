# Implement the 2026-08-05 Normative Briev Language Specification

> **2026-08-14 addendum (docs-reconciliation).** `exit program` → `endprogram`
> at :145/:295/:586 (the keyword shipped in the endprogram plan).

**Date:** 2026-08-05  
**Status:** Planned — implementation not started  
**Normative source:** `spec/SPEC.md`  
**Decision record:** `docs/plans/2026-08-05-language-audit-decisions-draft.md`  
**Code baseline:** `46f4f741` (`docs: record language audit decisions`)  
**Baseline worktree:** REQUIRED at `../briev-compiler-baseline`; absent at plan creation

## 1. Goal

Bring every active Briev implementation path and shipped source file into conformance with the normative 2026-08-05 specification.

Conformance means:

1. One canonical grammar with no compatibility parser.
2. Parser, AST, resolver, typechecker, analyses, proof engine, interpreter, active backends, stdlib, GLUE, tooling, and documentation agree.
3. Every normative feature is implemented interpreter-first and then by every applicable active backend, or rejected by a frontend target-capability diagnostic.
4. No compiler path selects user semantics from a concrete source type name.
5. No unresolved representation falls back silently to `i64`, zero, `Void`, the first match arm, or the first output.
6. All active shipped Briev/Data Briev files parse and typecheck in CI.
7. Runtime correctness and performance do not regress against the baseline table in §4.

## 2. Non-goals

- No compatibility parser.
- No `briev migrate` command.
- No restoration of retired backends.
- No implicit standard-library concepts in Rust compiler match arms.
- No performance optimization justified without the measurement protocol in §5.
- No retroactive rewriting of timestamped plans or archived specifications.

## 3. Governing architecture

### 3.1 Vertical slices only

Every language feature is implemented through the complete path before its phase closes:

```text
manifest → lexer → parser → AST → resolution → type/effect/proof analysis
         → reference interpreter → target-capability validation
         → active backend lowering → formatter/LSP/highlighter
         → stdlib/examples/tests/docs
```

No phase may leave a parsed construct that evaluates to a stub or backend placeholder.

### 3.2 Frontend owns meaning

The frontend resolves:

- operation identity and concrete behavior binding;
- trait conformance;
- protocol membership and cast route;
- effects;
- ownership and provenance;
- concurrency classification;
- reflection descriptor fields;
- target capability legality;
- layout constraints.

Backends consume these decisions and choose equivalent physical realizations.

### 3.3 Interpreter is reference

The interpreter establishes semantic behavior before LLVM, SPIR-V, CIRCT, or Webstack lowering is accepted.

### 3.4 No hidden compatibility

When a syntax family migrates, the parser and every active repository source using that family migrate in the same phase. Removed aliases are rejected immediately. Historical examples move to `archive/` rather than remaining parseable through compatibility branches.

## 4. Pre-implementation baseline

### 4.1 Commands and environment

Run on 2026-08-05 from the repository root:

```bash
cargo build --release
cargo test --lib
bash benchmarks/build_and_bench.sh --runtime
```

Results:

- Release build: PASS.
- Library tests: **1,496 passed; 0 failed**.
- Runtime benchmark harness: completed.
- Standard benchmark correctness: all rows reported `MATCH` by the harness.
- Known harness defects: §4.3.

### 4.2 Runtime baseline table

The table is the mandatory comparison baseline for all implementation phases touching analysis, runtime, codegen, ABI, or shipped benchmark sources.

| Benchmark | Briev | C | Ratio | Harness correctness |
|---|---:|---:|---:|---|
| ring_buffer | .0591s | .0500s | 1.18x | MATCH |
| float_math | .0469s | .0751s | .62x | MATCH |
| float_math_nonzero | .1614s | .1696s | .95x | MATCH |
| sparse_dispatch | .0478s | .0589s | .81x | MATCH |
| print_loop | .0333s | .0619s | .53x | MATCH |
| nbody_newton | 7.4546s | 8.9307s | .83x | MATCH |
| nbody_sqrt | 2.4702s | 3.3258s | .74x | MATCH |
| nbody_sqrt_idio | 3.2124s | 4.0900s | .78x | MATCH |
| fasta | .2367s | .2552s | .92x | MATCH |
| fannkuch_redux | .0695s | .0752s | .92x | MATCH |
| mandelbrot | .7423s | .7257s | 1.02x | MATCH |
| kalman_filter_runtime | .1557s | .1843s | .84x | MATCH |
| knucleotide | .1989s | .2005s | .99x | MATCH |
| cancel_math | .0547s | .0654s | .83x | MATCH |
| bit_clear | .0001s | .0002s | .50x | MATCH |
| queue_drain | .0363s | .0632s | .57x | MATCH |
| queue_drain_sym | .0385s | .0617s | .62x | MATCH |
| queue_drain_idio | .0380s | .0678s | .56x | MATCH |
| stack_push_pop | .0462s | .0744s | .62x | MATCH |
| interval_step | .0782s | .0776s | 1.00x | MATCH |
| telemetry_stream | .1945s | .2154s | .90x | MATCH |
| pid_control | .3421s | .3485s | .98x | MATCH |
| matrix_pipeline | .4636s | .9737s | .47x | MATCH |
| accumulator_flush | .1296s | .1851s | .70x | MATCH |
| sweep_sparse | .2211s | .1558s | 1.41x | MATCH |
| sweep_mid | .2646s | .2415s | 1.09x | MATCH |
| sweep_dense | .4063s | .2714s | 1.49x | MATCH |
| sweep_arr | .4084s | .3513s | 1.16x | MATCH |
| series_converge | .0001s | .0001s | 1.00x | MATCH |
| global_lifetime | .0390s | .0784s | .49x | MATCH |
| deep_recursion | .0002s | 0s | undefined | MATCH* |
| arena_churn | .0885s | .0982s | .90x | MATCH |
| linked_list | 1.3571s | 1.9493s | .69x | MATCH |
| hash_ops | 1.0316s | 1.1885s | .86x | MATCH |
| hash_ops_idio | .0308s | .0562s | .54x | MATCH |
| enemy_swarm | .1040s | .1340s | .77x | MATCH |
| bridge_glue | custom | — | — | SKIP* |
| bridge_multi | custom | — | — | PASS |

### 4.3 Baseline defects that block trustworthy final comparison

These are pre-existing correctness/reporting defects. Log their root causes in `BUGS.md` before implementation phases depend on the harness.

1. **`deep_recursion`** prints a divide-by-zero runtime error, but the harness reports `MATCH` with output `15` and a zero C duration. The comparison is not trustworthy.
2. **`bridge_glue`** produces Briev result `<null>` versus expected `42`, reports `❌ MISMATCH`, then records `SKIP` because the standard Briev binary is absent. This must not be counted as a passing baseline.
3. Protocol round-trip proofs for ASCII, UTF16, and Posit32 are skipped because implementation bodies are unavailable. The normative SPEC requires proof or explicit trusted axioms.
4. The normalizer emits unresolved-width/alignment fallbacks for `Slice`, `List`, `Stack`, `HashMap`, `RingBuffer`, and other types. The normative SPEC forbids silent representation fallback.
5. Numerous benchmarks still use `#!exit` and trigger warnings that no tick loop will check the condition. Migration to `exit program`, ports, or explicit entry macros must preserve observability and output.

### 4.4 Persistent baseline worktree

Before Phase 1 code changes:

```bash
git worktree add ../briev-compiler-baseline 46f4f741
```

If the path already exists in a future session, verify it points to `46f4f741`; do not overwrite uncommitted work.

Every performance-sensitive phase uses:

```bash
bash benchmarks/compare_baseline.sh <benchmark>
```

The final gate runs the full runtime suite in both worktrees on the same machine.

## 5. Measurement and regression protocol

For any phase that changes frontend analysis, representation selection, runtime, FFI, or backend emission:

1. State the exact performance hypothesis.
2. Inspect the generated IR/binary path affected.
3. Run a pre-build A/B experiment when the hypothesis concerns IR shape.
4. Link with the harness's exact `-O3 -flto -march=native` command.
5. Verify output equality at a bound crossing an observable print boundary.
6. Interleave baseline/new/C runs.
7. Record full results in this plan or a benchmark result document.
8. Reject a refuted optimization hypothesis.
9. Never blame noise or hash iteration without controlled A/B evidence.

Performance is not allowed to regress merely because syntax or architecture became cleaner. Rebuild any correct optimization on the new frontend analyses.

## 6. Phase 0 — Freeze evidence and conformance infrastructure

### 6.1 Deliverables

- Create/verify the persistent baseline worktree.
- Log the baseline harness defects in `BUGS.md`.
- Add a conformance test runner that discovers every active source/data extension.
- Define a staged-feature diagnostic category.
- Add an implementation-status matrix keyed by normative SPEC section.
- Add SPEC examples as executable fixture sources where syntax is already implemented.

### 6.2 Files

- `BUGS.md`
- test/CI configuration
- `src/errors.rs`
- `src/compile.rs`
- `spec/SPEC.md` fixtures or a dedicated conformance fixture directory

### 6.3 Gates

- `cargo test --lib`
- `cargo build`
- Existing active files are inventoried, even when they do not yet conform.
- No normative staged feature silently succeeds through a placeholder.

## 7. Phase 1 — Shared language manifest and diagnostics

### 7.1 Goal

Create one machine-readable source for:

- keywords and reserved words;
- operators and sigils;
- hashwords and intrinsics;
- file extensions and dotted profiles;
- declaration/statement contexts;
- casing requirements;
- highlighter/LSP categories;
- staged/removed status.

### 7.2 Required behavior

- Lexer, LSP, TextMate grammar, formatter, and diagnostics consume generated vocabulary.
- Known compiler vocabulary has exact casing.
- User-defined casing is warning/info only.
- `sed`, `pvt`, and `reg` remain reserved but unavailable as constructs.

### 7.3 Files

- New shared manifest under `config/` or an existing canonical config location
- `src/lexer.rs`
- `src/parser/helpers.rs`
- `src/lsp.rs`
- `syntax-highlighter/syntaxes/briev.tmLanguage.json`
- formatter/display generation code

### 7.4 Tests

- Manifest/lexer parity.
- Manifest/LSP parity.
- Manifest/highlighter parity.
- Exact-casing diagnostics.
- Every token round-trips through display when canonical.

## 8. Phase 2 — Canonical formatter and AST preservation

### 8.1 Goal

Make canonical formatting available before repository-wide syntax migration.

### 8.2 Work

- Separate debug display from canonical source formatting.
- Add parse → format → parse AST-equivalence property tests.
- Preserve source spans and rationale metadata through formatting.
- Ensure macro-generated AST formats canonically.
- Add formatter coverage for Briev, `.f`, RBV embedded expressions, `.dbv`, and `.dbvl` where applicable.

### 8.3 Files

- `src/ast/display.rs`
- `src/ast/top.rs`
- `src/ast/expr.rs`
- `src/annotator.rs`
- new/existing formatter module

### 8.4 Gate

No syntax-family migration begins until the formatter can emit that family's canonical form.

## 9. Phase 3 — Atomic removal of dead and legacy surface forms

Each subsection is an independent vertical slice and commit boundary when explicitly authorized. Parser removal and all active repository source migration occur together.

### 9.1 Remove dead tokens and aliases

Remove:

- `sig`, `state`, `rstruct`, `uni`, `like`, `is`, `prop`, `meld`, `syscall`;
- `term!`, `trg!`, `cell!`, `sync!`, `frgn!`, `frgn?`, `frgn?!`, `syscall!`;
- `Ptr!`, `<:`, `:>`, `|>`, `++`;
- `Ok`, `Err`, `Some`, `None` reserved tokens;
- legacy pragma/attribute tokens;
- `@` raw literal;
- adjacent prefix literals such as `sql"..."`;
- free-form dotted type extensions;
- width-specific numeric lexer token families.

Keep `sed`, `pvt`, and `reg` reserved.

### 9.2 Canonical replacements

- `escape` → `rollback`.
- `term!` → `exit program` plus `defer` where cleanup/observability requires it.
- `sync {}` → `mutex {}`.
- `render struct/obj/cell` → `render Name`.
- `b-if` → `b-when`.
- import/foreign `as` aliases → local-to-source `:` binding.
- `#on_exit` → `defer`.
- `#assume_event` → event-port contract.
- `#assume_shape` → proven contract or ordinary guard.
- `Ptr!` → `Ptr`/`Ptr<T>`.
- host type suffixes → GLUE/protocol configuration.
- concatenation `++` → operation-bound function/operator.
- `sql"..."` → `sql!(...)`.
- raw literals → `#r`/`#b`.

### 9.3 Repository migration scope

- `lib/std/**/*.bv`
- `lib/compiler/**/*.bv`
- `examples/**/*.{bv,ebv,abv,cbv,rbv}`
- `benchmarks/**/*.bv`
- `.smoke/**/*.bv`
- active fixtures/tests
- active architecture/tutorial docs

Historical plans and archived specs are not rewritten.

### 9.4 Tests

For every removed form:

- one rejection test with migration guidance;
- one canonical replacement parse/typecheck/evaluation test;
- formatter output test;
- active-source conformance sweep.

## 10. Phase 4 — Canonical declarations and semantic domains

### 10.1 AST model

Make declaration roles explicit:

- `type`: semantic identity, logical fields, invariants, metadata, functions, `op` bindings;
- `trait`: requirements/defaults, no storage semantics;
- `proto`: compiler-visible semantic/cast coherence category;
- `struct`: data relationships only;
- `seq struct`: order/containment constraint;
- `enum`: nominal closed sum;
- `impl`: narrow behavior attachment for data-only/imported shapes;
- `obj`: identity/lifecycle/parent-reactor behavior;
- `cell`: sealed independent convergence membrane.

### 10.2 Files

- `src/ast/top.rs`
- `src/ast/expr.rs`
- `src/parser/definitions.rs`
- `src/typechecker/mod.rs`
- `src/import_resolver.rs`
- type universe/normalization modules

### 10.3 Structural constraints

- No methods/transactions inside `struct`.
- `impl` ownership/orphan rules.
- `type Child: Parent` refinement-only, one parent maximum.
- No `obj` or `struct` inheritance.
- Constructor initialization validates every logical field or protocol default.
- No generic zero fill.

### 10.4 Tests

- All declaration kinds parse/format/typecheck.
- Invalid behavior placement is rejected.
- Refinement pre/post/invariant rules.
- Inherent/trait impl coherence and orphan errors.
- Generic struct/enum/type/trait instantiation.

## 11. Phase 5 — Traits, operations, protocols, and casting

### 11.1 Traits

Implement:

- structural satisfaction;
- optional explicit assertion;
- logical field requirements;
- required functions/ops/effects;
- default functions and `op` bindings;
- defaults excluded from self-conformance proof;
- behavioral refinement proof;
- conflict diagnostics and explicit resolution;
- `dyn Trait` existential representation with explicit runtime dispatch;
- explicit import of trait node templates.

### 11.2 Operations

- Elaborate every syntactic operator into a resolved semantic operation call.
- Carry contracts, effects, and access shape to lowering.
- Remove collection/type-specific dispatch from interpreter/backends.
- Implement type-directed associative literals through construction/insertion ops.
- Preserve transfer/destructive-transfer semantics.

### 11.3 Protocol graph

- Bare variants remain unresolved until target selection.
- Intra-protocol paths require functional equivalence.
- One written `as` crosses at most one declared cross-protocol edge.
- Multi-category conversion requires multiple written casts.
- Missing proof bodies are errors unless explicitly trusted.

### 11.4 Files

- `src/type_universe/`
- `src/analysis/protocol_graph.rs`
- casting graph modules
- `src/typechecker/mod.rs`
- `src/intrinsic_signatures.rs`
- `src/encoding_registry.rs`
- `config/targets.toml`

### 11.5 Kani

Add harnesses for:

- cast-path edge-count invariants;
- coherent route selection;
- trait-default conflict resolution;
- no invalid implicit cross-protocol path.

## 12. Phase 6 — Contracts, invariants, effects, and proof

### 12.1 Contract representation

Represent omitted pre/post clauses distinctly from explicit expressions. Reject explicit `[true][true]` language-wide.

### 12.2 Invariants

- Parse and retain type invariants.
- Prove constructors establish invariants.
- Prove mutating transformations preserve invariants.
- Prove refinement children strengthen parent invariants.

### 12.3 Unified effects

Replace scattered effect facts with one frontend result covering:

- read/write sets;
- allocation/free;
- spawn/await/cancel;
- FFI/I/O/blocking;
- purity;
- cancellation safety.

Expose effects to traits, contracts, reflection, concurrency gate, lifetime scheduler, and backend attributes.

### 12.4 Files

- contract AST and parser
- `src/proof_engine/`
- `src/symbolic.rs`
- `src/analysis/dataflow.rs`
- `src/analysis/dependency_graph.rs`
- `src/analysis/termination.rs`
- `src/analysis/watchdog.rs`

### 12.5 Gates

- Every proof skip is explicit trusted evidence or error.
- `.s` promotes unresolved lifetime/proof warnings to errors.
- No contract weakening to satisfy existing implementation.

## 13. Phase 7 — Literals, ranges, slicing, reflection, and const generics

### 13.1 Literals

- Standard numeric bases `0x`, `0b`, `0o`.
- No physical-width suffix tokens.
- Canonical duration units.
- `#r` raw strings and `#b` bytes.
- No custom Parse surface yet.
- Type-directed ordered/associative literals.
- No universal null.

### 13.2 Slicing

- Python-style `start:stop:step`.
- `...` only as multidimensional ellipsis.
- named selectors `name => selector`.
- Boolean mask indexing.
- remove semicolon-mask slice form.
- standalone `..` and `..=` ranges.

### 13.3 Reflection

- `.^` runtime fields.
- `.^^` frozen descriptors.
- remove universal projection registry where field/intrinsic behavior applies.
- materialization-sensitive alignment/endian facts.
- no address acquisition through reflection.

### 13.4 Const generics

- Const parameter AST distinct from type arguments.
- Bounds participate in specialization/proof.
- Static dimensions feed slicing, vectorization, CIRCT shape, and ABI checks.

### 13.5 Tests

- Parser precedence and ambiguity.
- Slice shape/property tests.
- Compile-time freeze cycle rejection.
- Const-generic substitution and bounds proof.

## 14. Phase 8 — First-class callables and compile-time execution

### 14.1 Callable semantics

- Type syntax `(params) -> T`.
- Named `defn`/`txn` values.
- Closure capture representation and ownership.
- Interpreter closure application.
- LLVM closure environment lowering.
- Capability rejection on targets unable to represent dynamic callables.

### 14.2 Compile-time syntax

- `$name` compile-time-only bindings.
- `name!(...)` expansion.
- `$(Stage)` timing.
- privileged macro capabilities declared at definition.
- remove `$!` and prefix-discriminator literal paths.

### 14.3 Derivation

Retain `:=` derivation/reference semantics and prove generated implementations against contracts.

### 14.4 Tests

- lexical capture and move/borrow behavior;
- closure lifetime errors;
- interpreter/backend equivalence;
- macro argument grammar ownership;
- stage ordering and descriptor-freeze constraints.

## 15. Phase 9 — Ownership, provenance, and lifetime scheduling

### 15.1 Universal ownership algebra

Implement `borrow`, `consume`, `owned`, `borrowed<source>`, and `shared` as normalized ownership-flow facts.

### 15.2 Provenance

- Dangling pointer: hard error in every profile.
- Mutable access: proven exclusive provenance.
- Shared mutation: explicit atomic/synchronization contract or cell boundary.
- FFI pointer/aggregate ownership: mandatory.

### 15.3 Scheduler

- Proven last use schedules release.
- `free` is verified.
- `keep` transfers owner/boundary lifetime.
- unresolved normal-profile lifetime uses warning + boundary collector;
- `.s` rejects unresolved lifetime.

### 15.4 Files

- `src/analysis/provenance.rs`
- `src/analysis/allocation.rs`
- `src/analysis/global_lifetime.rs`
- `src/lifetime.rs`
- interpreter heap
- LLVM destruction paths

### 15.5 Kani

Prove:

- no use after verified free;
- no double free across scheduler/manual paths;
- borrowed result cannot outlive source;
- linear handle cannot be silently discarded;
- shared release policy balances retain/release.

## 16. Phase 10 — Reactive execution, objects, cells, and tasks

### 16.1 Control forms

Implement vertically:

- `rollback` replacing `escape`;
- `exit program` replacing `term!`;
- `defer` replacing `#on_exit`;
- `mutex` replacing `sync {}`;
- `barrier<group>`;
- `spawn`/`await` handles;
- task cancellation proof;
- removal of statement-level `async` forms.

### 16.2 Objects and cells

- Shared port grammar.
- Complete named product outputs.
- Object parent-reactor identity/lifecycle.
- Cell sealed state and independent convergence.
- Explicit external port dependencies.

### 16.3 Scheduler

- Deterministic interpreter scheduler.
- Interleaving exploration verification mode.
- Existing no-implicit-concurrency gate extended to spawned/reactive instances and RBV handlers.

### 16.4 Watchdogs

- canonical units;
- optional/required contextual sigils;
- handler typing;
- target-independent timing semantics;
- progress proof and effect interaction.

### 16.5 Kani

- no unclassified eligible pair reaches execution;
- barrier membership/liveness invariants;
- cancellation cleanup runs exactly once;
- object/cell handle release closes owned state/ports.

## 17. Phase 11 — Modules and repository graph

### 17.1 Imports

- exact quoted paths;
- configured-root angle paths;
- local-to-source `:` aliases;
- selective imports;
- explicit `export import`;
- hard errors on collisions;
- no globs;
- no cycles;
- diamond dependency support;
- target-selected equivalent module variants.

### 17.2 Implementation coherence

Module ownership is the basis for inherent and trait impl coherence. Resolver output must preserve source ownership after import expansion.

### 17.3 Tests

- diamond graph;
- true cycle;
- conflicting names;
- re-export visibility;
- target module equivalence;
- configured root determinism;
- implementation orphan rules across modules.

## 18. Phase 12 — FFI, export, and GLUE

### 18.1 Source syntax

- canonical `frgn` only;
- local `:` external symbol binding;
- four provenance forms;
- `optional frgn` + `.^^Available`;
- named variadic parameter;
- explicit real return type;
- no fallback clause;
- no `syscall` keyword;
- no MMIO address form;
- `export` only.

### 18.2 Remove meld architecture

Trace and replace all uses in:

- AST/parser/display;
- type universe;
- boundary marshalling;
- bridge generation;
- LLVM codegen;
- tests and docs.

Foreign adaptation comes from:

- GLUE/Data Briev layout descriptors;
- explicit protocol cast edges;
- ownership/effect contracts;
- configured error mapping.

### 18.3 GLUE config migration

Rename and structurally rewrite:

```text
lib/glue/*/glue.dbvl → lib/glue/*/glue.dbv
```

Update loaders, tests, docs, and generated-path diagnostics. Keep generated `bridge-exports.dbvl`.

### 18.4 FFI proof gates

- Ownership required for pointer/aggregate boundaries.
- Actual ABI signature and Briev-visible signature mapping validated.
- Optional symbol availability is compile-time descriptor reflection.
- Raw kernel transition only through explicit `SysCall#`.
- No platform type-name matching in generator code.

### 18.5 Baseline defects

Repair `bridge_glue` mismatch before declaring Phase 12 complete. Add a hard harness correctness failure instead of `SKIP` on produced wrong output.

## 19. Phase 13 — Data Briev

### 19.1 Parser modes

- `.dbv`: structured/category grammar.
- `.dbvl`: one physical line per record, `>` instructions.
- shared raw token/value/schema core.

### 19.2 Schema validation

When asserted, enforce:

- required/unknown fields;
- conversion;
- constraints;
- optional fields;
- named schemas;
- key presence/uniqueness.

### 19.3 Streaming and mutation

- lazy line offsets/indexing;
- append-only canonical writer;
- stable key derivation;
- deterministic serialization.

### 19.4 CLI

`briev check` dispatches by extension and validates schemas.

### 19.5 Migration

- remove `.dbvs` active files;
- rewrite schema types to canonical forms;
- migrate human-authored GLUE configs;
- preserve generated DBVL protocol.

## 20. Phase 14 — Rendered Briev

### 20.1 Document parser

- remove script-wrapper compatibility;
- canonical source outside `<view>`/`<style>`;
- `render Name` only;
- canonical Briev expression parser for directives.

### 20.2 Lifecycle

- custom tags create first-class reactive instances;
- parent owns mounted handles;
- `b-when` mounts/unmounts;
- `b-show` preserves state;
- `b-each` dynamic children require stable keys;
- `b-bind` restricted to proven assignable fields.

### 20.3 Effects and concurrency

- render expressions pure/read-only;
- event handlers carry inferred effects;
- component nodes and handlers pass the no-implicit-concurrency gate.

### 20.4 Web boundary

Replace hardcoded primitive/DOM type matches with GLUE-configured representations and target-capability validation.

## 21. Phase 15 — `.f` strict indentation frontend

Replace the current text preprocessor with a token-aware layout frontend.

Requirements:

- no generated invalid `header; {` sequences;
- no statement-block braces or semicolons in `.f`;
- literal delimiters retained;
- source maps preserved;
- identical AST and semantics to canonical source;
- formatter supports canonical `.f` output;
- all `.f` SPEC fixtures parse/typecheck.

Do not keep the current naive source-line brace insertion as a fallback.

## 22. Phase 16 — Target profiles and active backend parity

### 22.1 Active targets

- LLVM/native and embedded;
- direct SPIR-V for `.abv`;
- CIRCT for `.cbv`;
- Webstack for `.rbv`.

### 22.2 Capability validation

One frontend validator rejects unsupported constructs before lowering. Backends never emit placeholders.

### 22.3 LLVM cleanup

Remove:

- `List`/collection-specific indexing dispatch;
- source type-name alignment rules;
- unresolved `i64` fallback;
- default width/alignment guesses;
- first-arm/first-output fallbacks;
- backend re-analysis of frontend decisions.

Preserve additive optimization fallthroughs and deterministic sorted emission.

### 22.4 SPIR-V/CIRCT/Webstack

For each normative construct:

- implement equivalent semantics; or
- declare target capability absence and hard-error in frontend.

### 22.5 Assembly

`asm<target>` declarations require target syntax validation, complete contracts, effects, and clobbers.

## 23. Phase 17 — Interpreter semantic-value migration

This phase may begin earlier as a dependency, but closes only after all language families use the generic model.

Replace type-specific value semantics with:

- `TypeId`/semantic descriptor;
- optimized bootstrap atoms;
- bits;
- products;
- sums;
- references;
- closures;
- void.

FFI host collections convert at the boundary. No interpreter evaluation path matches `List`, `HashMap`, JSON, DOM, or other stdlib/platform type names.

Add reference behavior for:

- struct/enum construction and access;
- method calls;
- match patterns/exhaustiveness;
- closures;
- reflection;
- spawn/await;
- cells/objects;
- ownership/cancellation;
- FFI error mapping.

## 24. Phase 18 — Standard library and compiler-in-Briev migration

### 24.1 Stdlib

Migrate all active modules to canonical declarations, imports, ops, protocols, qualified sum variants, FFI ownership, and no removed projections.

Collection behavior remains in stdlib. No Rust match arm may be added for `List`, `HashMap`, stack, queue, string utility functions, or host handles.

### 24.2 Compiler-in-Briev

Migrate `lib/compiler/` syntax and ensure every pass parses/typechecks in CI. Remove stale comments/examples that present old grammar as active.

### 24.3 Intrinsics audit

For every stdlib/compiler function currently using `frgn`, check `get_intrinsic_signature()` first. Keep only true bootstrap intrinsics compiler-known.

## 25. Phase 19 — Repository-wide active-source conformance

The conformance runner must check:

- every active stdlib/compiler source;
- every example;
- every benchmark source;
- every smoke fixture;
- every GLUE config;
- every active target fixture;
- every normative SPEC example.

Files that intentionally retain historical syntax move under `archive/` and leave active discovery roots.

No source is excluded merely because current tests did not import it.

## 26. Phase 20 — Documentation convergence

### 26.1 Architecture documents

Update in the same structural commits:

- `docs/architecture/overview.md`
- `docs/architecture/agent-reference.md`
- `docs/architecture/backend-architecture.md`
- `docs/architecture/backend-type-dispatch.md`
- `docs/architecture/casting-protocol.md`
- `docs/architecture/hash-words.md`
- concurrency/modifier docs
- reflection/projection docs
- ownership/lifetime docs
- GLUE/FFI docs
- Data Briev docs
- Rendered Briev docs
- target/backend strategy docs

### 26.2 Tutorials

Rewrite `learn-briev/` around the normative grammar. Do not preserve obsolete syntax as alternatives. Migration notes may name removed forms but must not teach them as accepted.

### 26.3 CLAUDE.md and contributor instructions

`CLAUDE.md` is currently materially stale (`.br`, Rust transpilation defaults, `.dbvs`, old CLI/backend descriptions). Rewrite it to point to the normative SPEC and current architecture instead of duplicating obsolete language contracts.

### 26.4 Commentary preservation

When refactoring implementation files:

- preserve rationale comments;
- rewrite them for the new mechanism rather than deleting them;
- retain date, reason, targeted pattern, and removal path;
- mark temporary bridges with `// TEMP: YYYY-MM-DD:` and a permanence path.

## 27. Phase 21 — Retired backend and archive boundary

Move unsupported backend implementations out of active-looking module paths after confirming no active router/import references remain.

Retain history under `archive/`; do not make retired implementations compile against the new AST merely to preserve dead code.

Active backend documentation lists only LLVM, SPIR-V, CIRCT, and Webstack paths supported by target profiles.

## 28. Verification gates

### 28.1 Per vertical slice

```bash
cargo test --lib
cargo build
praetor validate --warn --target <changed-directory>
```

Additional gates:

- parser/formatter round-trip tests;
- changed active sources conformance sweep;
- applicable Kani harnesses;
- backend-specific IR inspection;
- no new compiler warnings;
- no removed syntax in active paths.

### 28.2 Search guards

The final repository must have zero active implementation matches for:

- user type-name dispatch in `src/backend/llvm/` and `src/glue/`;
- parser acceptance of removed tokens;
- `TopLevel::Meld`, `TopLevel::RStruct`, standalone `Signature`, old `Escape`/`TermBang` AST paths;
- `Value::List`/`Value::HashMap` semantic dispatch;
- `.dbvs` active paths;
- human-authored `lib/glue/*/glue.dbvl`;
- `render struct`, `render obj`, `b-if`, script-wrapper compatibility;
- glob imports or import `as` aliases;
- unresolved LLVM type/alignment fallback.

Historical plans/archive paths are excluded from these guards.

### 28.3 Final target matrix

Compile and test representative fixtures for:

| Profile | Required checks |
|---|---|
| `.bv` | interpreter + LLVM native |
| `.s.bv` | strict proof/ownership/effect gates |
| `.f.bv` | indentation AST equivalence |
| `.ebv` | embedded target capability validation |
| `.abv` | direct SPIR-V validation |
| `.cbv` | CIRCT output validation |
| `.rbv` | Webstack + component lifecycle |
| `.dbv` | structured parse/schema/canonical write |
| `.dbvl` | line parse/schema/lazy append/canonical write |

### 28.4 Final benchmark gate

1. Fix baseline harness defects in §4.3.
2. Run full runtime suite in current and baseline worktrees.
3. Require output equality for every benchmark; no `SKIP` on wrong output.
4. Compare all ratios to §4.2.
5. Investigate every regression with controlled A/B.
6. Record post-migration table below.

## 29. Post-implementation results

Fill after all phases:

| Benchmark | Baseline ratio | New ratio | Correct | A/B conclusion |
|---|---:|---:|---|---|
| ring_buffer | 1.18x | pending | pending | pending |
| float_math | .62x | pending | pending | pending |
| float_math_nonzero | .95x | pending | pending | pending |
| sparse_dispatch | .81x | pending | pending | pending |
| print_loop | .53x | pending | pending | pending |
| nbody_newton | .83x | pending | pending | pending |
| nbody_sqrt | .74x | pending | pending | pending |
| nbody_sqrt_idio | .78x | pending | pending | pending |
| fasta | .92x | pending | pending | pending |
| fannkuch_redux | .92x | pending | pending | pending |
| mandelbrot | 1.02x | pending | pending | pending |
| kalman_filter_runtime | .84x | pending | pending | pending |
| knucleotide | .99x | pending | pending | pending |
| cancel_math | .83x | pending | pending | pending |
| bit_clear | .50x | pending | pending | pending |
| queue_drain | .57x | pending | pending | pending |
| queue_drain_sym | .62x | pending | pending | pending |
| queue_drain_idio | .56x | pending | pending | pending |
| stack_push_pop | .62x | pending | pending | pending |
| interval_step | 1.00x | pending | pending | pending |
| telemetry_stream | .90x | pending | pending | pending |
| pid_control | .98x | pending | pending | pending |
| matrix_pipeline | .47x | pending | pending | pending |
| accumulator_flush | .70x | pending | pending | pending |
| sweep_sparse | 1.41x | pending | pending | pending |
| sweep_mid | 1.09x | pending | pending | pending |
| sweep_dense | 1.49x | pending | pending | pending |
| sweep_arr | 1.16x | pending | pending | pending |
| series_converge | 1.00x | pending | pending | pending |
| global_lifetime | .49x | pending | pending | pending |
| deep_recursion | invalid baseline | pending | pending | pending |
| arena_churn | .90x | pending | pending | pending |
| linked_list | .69x | pending | pending | pending |
| hash_ops | .86x | pending | pending | pending |
| hash_ops_idio | .54x | pending | pending | pending |
| enemy_swarm | .77x | pending | pending | pending |
| bridge_glue | invalid baseline | pending | pending | pending |
| bridge_multi | PASS | pending | pending | pending |

## 30. Commit and review boundaries

Each subsection that changes a syntax/semantic family is an atomic review boundary. At every boundary:

1. Parser/AST/analysis/interpreter/backend behavior is complete.
2. Tests and active sources are migrated.
3. Architecture docs are current.
4. `cargo test --lib`, `cargo build`, Praetor, and applicable Kani pass.
5. Benchmarks are run when runtime/IR may change.
6. Only intended files are staged.
7. A commit is created only when explicitly authorized by the user/session policy.

Do not amend failed commits, skip hooks, force push, stash unrelated work, or discard uncommitted work.

## 31. Completion definition

The migration is complete only when:

- every normative SPEC section has an implementation-status PASS;
- every active shipped source/data file conforms;
- every active backend implements or frontend-rejects every normative construct;
- interpreter/backend semantics match;
- no removed syntax remains active;
- no user type-name matching remains in active backend/GLUE code;
- all proof, ownership, and concurrency gates are enforced;
- all docs/tutorials/tooling agree with the SPEC;
- library tests, builds, Praetor, Kani, target matrix, and corrected benchmarks pass;
- post-implementation results are recorded.
