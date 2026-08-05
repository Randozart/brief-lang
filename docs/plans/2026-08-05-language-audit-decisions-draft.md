# Briv Language Audit — Draft Decisions

**Date:** 2026-08-05  
**Status:** DRAFT — audit in progress; no implementation authorized by this record  
**Scope:** Language model, syntax, compiler architecture, interpreter/backend contracts, Data Briv, tooling, documentation, and rename verification  
**Performance baseline:** Not applicable. This document records design decisions only; any later performance implementation requires its own baseline and A/B plan.

This document preserves every decision reached during the ongoing post-rename language audit. It is a living audit record until the review is complete. A decision recorded here is not evidence that the compiler currently implements it.

## 1. Language model

- Types have no canonical physical layout.
- Runtime layouts are selected from operations, target constraints, metadata, and observed access patterns.
- Frontend analysis resolves semantic operations; backends choose equivalent physical realizations.
- Materialization ultimately produces bits, but the interpreter uses generic semantic values with optimized primitive atoms.

### 1.1 Declarations

- `type`: semantic behavior, logical fields, invariants, metadata/layout hints, functions, and `op` bindings.
- `trait`: structurally inferred behavioral requirements, logical field requirements, defaults, and optional explicit assertions.
- `proto`: compiler-known semantic/casting categories and semantic variants.
- `struct`: data relationships only; adaptive layout by default.
- `seq struct`: preserves field order and containment, not universal byte offsets.
- `enum`: closed nominal sum type with layout-derived representation.
- `obj`: identity, lifecycle, owned state, ports, and reactive behavior in the parent reactor.
- `cell`: sealed state machine with an independent convergence membrane and port-only communication.
- `impl`: narrowly attaches behavior to data-only declarations such as `struct`, `enum`, and imported foreign shapes.
- `obj` remains; it is not replaced by traits.
- `struct` and `obj` do not support parent inheritance.
- `type Child: Parent` is refinement-only single inheritance: no layout/state inheritance and no diamonds.
- `impl` follows strict ownership/orphan rules.

### 1.2 Traits

- Satisfaction is inferred structurally but can be explicitly asserted.
- Satisfaction requires proof of behavioral refinement, not merely matching names and signatures.
- Only concrete operations establish conformance; defaults cannot make a trait satisfy itself.
- Traits may provide default functions and default `op` bindings.
- Conflicting defaults require explicit resolution.
- Traits may require logical fields without requiring physical storage.
- Both `type` and `obj` can satisfy traits.
- Trait node templates require explicit import; they never activate automatically.
- Static/monomorphized dispatch is the default.
- Runtime dispatch requires explicit `dyn Trait`.

## 2. Objects, cells, and concurrency

- `obj` and `cell` share interface syntax:
  - Inputs in `(...)`.
  - Named outputs after `->`.
- `cell` remains a distinct isolation/concurrency boundary.
- Multiple outputs always form a complete named product; backends may not return only the first.
- External object conditions enter through explicit ports/bindings.
- `cell!` is removed.
- `spawn Component(...)` creates a persistent instance.
- `spawn` returns a linear owned handle; releasing it stops the instance and releases owned state.
- `sync!` is removed.
- Explicit runtime synchronization uses `barrier<group>`.
- Declarative node classification remains `async` or `sync<group>`.
- `async` is not a statement-level fire-and-forget/fork keyword; legacy `async call` and `async await` forms are removed.
- Task and component concurrency use linear `spawn` handles; `await handle` performs explicit join/result acquisition.
- `await task` consumes the task handle and returns the callable's declared result.
- `free task` requests cancellation/stop and runs `defer` cleanup; `keep task` transfers it to the enclosing owner/boundary for detached lifetime.
- Silently dropping or discarding a live task/component handle is an error.
- The reference interpreter uses a deterministic semantic scheduler for normal execution and a verification mode that explores all legal interleavings; it does not rely on nondeterministic host threads.
- `free task` requires effect/proof evidence for cooperative cancellation points and cancellation-safe active FFI; otherwise the handle must be awaited or kept.

## 3. Operations and casting

- `op` binds syntax such as `+`, indexing, and arrows to semantic behavior.
- Compiler knowledge stops at syntactic operation identities; it does not know concrete collection names.
- Semantic `op` resolution happens in the frontend.
- LLVM, CIRCT, and SPIR-V may choose different equivalent layouts and lowerings.
- Declaration-time bindings consistently use `:`, not `=`.
- Generic bounds enforce both:
  - Structural trait conformance.
  - Explicit protocol/cast compatibility.
- Within one protocol, all cast routes must be proven functionally equivalent.
- Each written `as` may cross at most one explicitly declared cross-protocol edge.
- Multiple semantic protocol transitions require explicitly chained casts.
- Protocol variants represent semantic distinctions only, not widths, alignment, or target layouts.
- Bare protocol variants resolve after target selection, not in the parser.

### 3.1 FFI provenance

- Canonical `from` provenance has four forms:
  - `from "path"`: exact project-relative or explicitly addressed path.
  - `from <path>`: path resolved through ordered roots declared by compiler/target configuration.
  - `from #Link<name>`: linker dependency.
  - `from #System`: symbol supplied by the selected system/runtime profile.
- Platform families such as POSIX are selected through target configuration, not source hashwords such as `#POSIX`.
- Configured-root resolution is deterministic and records the resolved path for reproducible builds.
- The standalone `syscall` keyword is removed.
- Named system APIs use `frgn ... from #System`; raw target-specific kernel transitions use an explicit intrinsic such as `SysCall#(...)`.
- A foreign declaration names the local Briv binding first; `:` binds it to a differently named external symbol, for example `frgn local_name(...): external_name from #System;`.
- `as` is reserved for semantic conversion and is not used for foreign-symbol renaming.
- Declaration-level foreign `fallback` clauses are removed; optional-symbol fallback behavior uses ordinary `when`/`match` control flow with `.^^Available`.
- Legacy `frgn name @ address` declarations are removed; MMIO uses configured device/cell ports or explicit pointer/address intrinsics.
- `meld` is removed entirely.
- Foreign shapes are imported/configured as explicit layout descriptors and adapted through declared protocol cast edges plus ownership/effect contracts.
- GLUE synthesizes boundary bridges from those descriptors and cast paths; ordinary semantic conversion and FFI adaptation use the same protocol-casting architecture.
- Exact foreign field order, widths, alignment, calling convention, and release policy live in GLUE/Data Briv configuration, not logical Briv source.
- Variadic foreign signatures use an explicit final named parameter such as `variadic args: ForeignArgs`; `...` is reserved for multidimensional slice ellipsis.
- GLUE configuration supplies the foreign variadic ABI behavior.
- A `frgn` signature declares its actual Briv-visible return type; foreign calls are never implicitly wrapped in `Result`.
- GLUE configuration explicitly maps errno, status codes, exceptions, or delivery failures into `Result` when required.

### 3.2 Module binding

- `as` is not used for module or imported-symbol aliases.
- Import aliases use local-to-source `:` binding, for example `import collections: "std/collections";` and `import { local_name: exported_name } from "module";`.
- Module paths use the same model as FFI provenance: quoted paths are exact/project-relative, while angle-bracket paths resolve through ordered configured roots.
- A separate compiler-registry import concept is removed; package registries are configured roots.
- Conflicting unqualified imported names are hard errors and require an explicit module alias or selective `local: exported` rename; import order never determines meaning.
- Ordinary imports remain private; curated module APIs use explicit `export import ...` re-exports.
- Glob imports are removed; dependencies use module imports or explicit selective imports.
- Extensionless imports select target-specific module variants through the configured target profile, never file-existence order.
- Target-specific sibling modules must satisfy the same exported interface and trait contracts.
- Diamond dependencies are valid and must not be misdiagnosed as cycles.
- Genuine module import cycles are compile errors; shared declarations move into an acyclic interface module.

## 4. Reflection and fields

- `prop` is removed.
- Runtime properties become logical/materialized fields or ordinary functions.
- `value.^Field`: runtime field reflection.
- `value.^^Field`: compile-time reflection over the frozen semantic/layout descriptor.
- Layout freezes before reflection-driven specialization.
- Unknown reflection fields are compile errors.
- `Type` and `Ops` remain semantic reflection.
- `Name`, `Params`, `Returns`, `Arity`, `Loc`, `FnSpan`, `Doc`, `Hash`, `Contracts`, `Module`, and `IsPure` are compile-time-only descriptor fields.
- `Alignment` and `Endian` are materialization-sensitive.
- `Codec` is removed.
- Actual address acquisition uses `&value`.
- Compile-time storage facts use named descriptor fields such as `Addressable`, `AddressSpace`, or `StorageClass`.
- `PtrBang`/`Ptr!` is removed; dereference remains `*ptr`.
- `Values` and `Elements` are fields when declared/materialized, not universal compiler projections.
- `AsStack` and `AsQueue` become type-defined conversions.

### 4.1 Intrinsic-only former projections

- `Absolute`.
- `BitReverse`.
- `Popcount`.
- `LeadingZeros`.
- `TrailingZeros`.

## 5. Control flow and matching

- `if`/`else` is removed intentionally.
- Branching uses `match`.
- Guarded blocks use `when condition { ... }`.
- `[condition] statement;` remains an inline single-statement guard.
- `[condition];` remains a convergence gate.
- `[condition] { ... }` remains invalid to avoid duplicate block-guard syntax.
- Match guards use `when`, not contextual `if`.
- Match arms use `=>`, not `->`.
- `match` must be fully exhaustive for closed sums.
- Open/unknown alternatives require a wildcard.
- `uni` is removed entirely.
- `A | B` is the canonical anonymous structural union syntax.
- Structural-union patterns use typed bindings such as `value: Int => ...`.
- `enum` remains the nominal closed-sum declaration.
- Transaction-wide abort/reversion uses `rollback;` or `rollback reason;`; legacy `escape` statement syntax is removed.
- `rollback` is valid only inside transactions/reactive firings with rollback semantics.
- The standalone `sig` declaration keyword is removed; external signatures use `frgn`, and body-less internal signatures use ordinary `defn` with declared effects/contracts.
- `sed`, `pvt`, and `reg` remain reserved for future language contracts; they stay out of the identifier namespace despite having no current parser/AST use.
- `const` remains the immutable top-level declaration, distinct from mutable top-level/`let` bindings and from compile-time-only `$const`.
- `asm<target> name(...) -> T { ... }` remains an ordinary top-level declaration keyword; `<target>` is the specialization slot and the raw instruction body is validated as target-specific by the configured capability profile.
- `asm<target>` declarations require a declared effect profile (read/write sets, clobbers, FFI/block effects) so the frontend can perform sound optimization, effect, and concurrency analysis.
- Top-level reactive state uses `let`; the separate `state` keyword is removed from the grammar.
- `foreach` is the sole iteration keyword; `for`, `while`, and `loop` remain absent by design.
- The reactive input keyword remains the compact `trg`, not the full word `trigger`; documentation uses `trg` consistently.
- Critical-section blocks are renamed `mutex { body }`; the `sync` keyword is reserved for the `sync<group>` node classification and `barrier<group>` runtime synchronization.
- The dead `|>` pipeline token is removed; composition uses chained calls, the `<-` transfer arrows, and `op` bindings.
- `++` is removed as dedicated concatenation syntax; concatenation resolves through the ordinary `op` binding mechanism.
- `Ok`, `Err`, `Some`, and `None` are removed as reserved lexer tokens; `Option`/`Result` variants resolve as ordinary stdlib-qualified identifiers such as `Option::None` and `Result::Ok`.
- `!>` remains the canonical metadata-binding operator inside declaration bodies; only the duplicated `!> observable: true` spelling is removed in favor of `out`.
- First-class callables use the signature-shaped type `(params) -> T` in type position, mirroring `defn name(...) -> T`; no separate `fn` keyword.
- `defn`/`txn` may be used as values; closures capture lexical bindings and evaluate in the interpreter as real values rather than `Void`.
- Const generics and dependent bounds are the prioritized next capability: `Int[N]` sizes may be compile-time parameters, enabling static bounds proofs, verified slicing, and layout-free dimension contracts.
- Const-generic bounds are enforced during specialization alongside trait and protocol bounds.
- The type grammar accepts only `Ptr`/`Ptr<T>`; the `Ptr!` type alias is removed.
- Free-form dot-extension type suffixes such as `String.c` are removed; host/target qualifiers live in configured GLUE bindings and protocol variants.

## 6. Compile-time syntax and sigils

- `$name`/`$defn`: compile-time-only declaration erased before runtime.
- `name!(...)`: explicit compile-time expansion; arguments may use macro-specific/noncanonical syntax.
- `$(Stage)`: controls when compile-time work executes.
- This three-part distinction must be explicitly documented.
- `$!name` is removed; privileged macros declare capabilities but are invoked as `name!(...)`.
- Prefix `!value` remains Boolean negation.
- Postfix `!` otherwise means compile-time expansion.

### 6.1 Runtime bang replacements

- `term!` becomes the unambiguous process-boundary statement `exit program`.
- `trg!` is removed.
- `frgn!` and `syscall!` are removed.
- Asynchronous foreign execution uses `spawn`.
- Synchronous discarded results use the ordinary discard/transfer form.
- `#on_exit` becomes `defer`.
- `#assume_event` becomes a contract on an explicit event port.
- `#assume_shape` is removed.

### 6.2 Question mark

- Postfix `?` has one expression meaning: typed `Result`/`Option` propagation.
- It evaluates once, runs `defer`, and requires a compatible enclosing return type.
- No implicit error conversion occurs without an explicit cast binding.
- `frgn?`, `frgn?!`, and `fn?` are removed.
- Optional foreign symbols use `optional frgn` and `.^^Available`.
- Watchdogs retain their contextual contract syntax:
  - `?[condition]`: optional watchdog.
  - `![condition]`: required watchdog.
- These are unambiguous because they occur only after contract declarations.

## 7. Contracts and proof

- Explicit `[true][true]` is rejected language-wide.
- Omitted contract clauses retain implicit provenance rather than becoming indistinguishable explicit tautologies.
- Omitted/implicit true postconditions on nodes/transactions indicate immediate completion and cannot request convergence.
- Type-level invariants are retained and must be proven across construction and mutation.
- Protocol proof gaps are errors unless explicitly marked trusted foreign/intrinsic axioms.
- `.s` may allow audited trusted FFI/protocol boundaries when their contracts are complete and proven.
- Trait implementations must refine trait contracts and effects.

## 8. Memory and ownership

- Dangling-pointer findings are hard errors in every profile.
- Mutable pointer access requires proven exclusive provenance.
- Intentional shared mutation requires explicit atomic/synchronization behavior or a cell boundary.
- FFI pointer/aggregate parameters and returns require explicit ownership contracts.
- FFI ownership uses a small universal algebra:
  - `borrow`: caller retains ownership; the callee cannot retain the value beyond the call.
  - `consume`: ownership transfers to the callee.
  - `owned`: the caller receives ownership.
  - `borrowed<source>`: the returned lifetime is bounded by a named input.
  - `shared`: ownership is shared through a declared retain/release policy.
- Read/write permissions belong to the inferred effect system, not ownership syntax.
- Allocation and destruction policies remain configurable; GLUE maps foreign conventions onto the universal ownership algebra and must agree with source signatures.
- `free` and `keep` remain the explicit lifecycle decisions.
- Proven last use is automatically scheduled for release.
- Unresolved lifetimes:
  - Warning in normal profiles, with boundary collector handling.
  - Error in `.s`.
- `keep` deliberately transfers to boundary/owner lifetime.
- `free` requires proof of no later or aliased use.
- Spawned state follows the owned handle’s lifecycle.

## 9. Interpreter and backend correctness

- Interpreter values become a generic envelope:
  - Semantic `TypeId`.
  - Optimized primitive atoms.
  - Generic bits, products, sums, references, closures, and void.
- There are no Rust `Value::List`/`Value::HashMap` semantic special cases.
- First-class closures are required.
- Active backends must implement normative behavior or hard-error; there are no placeholders, zero substitutions, first-arm-only matches, or silent `Void` results.
- Unknown LLVM representation is a hard error; there is no `i64` fallback.
- Backend and FFI generators may not match user-visible type names.
- Platform mappings such as DOM handles live entirely in GLUE/Data Briv configuration.
- One inferred effect system unifies read/write, allocation, free, spawn, FFI, I/O, blocking, and purity facts.
- Effects are usable in traits/contracts and visible through `.^^Effects`.

## 10. Syntax cleanup

- `<:` and `:>` are permanently removed.
- `rstruct` is removed.
- `like`, `is`, and `uni` are removed.
- Legacy pragma/attribute forms are removed.
- Generic specialization uses `<...>` only.
- `[]` remains containment, bounds, indexing, and contracts.
- Keywords are lowercase only.
- `proto` and `trait` become reserved keywords.
- Hash placement is restricted to recognized prefixes or terminal intrinsic suffixes; embedded `foo#bar` is invalid.
- Duration units use canonical abbreviations.
- Compiler-known keywords, intrinsics, hashwords, and operation identities require exact spelling/casing.
- User-declared casing violations produce information/warnings, not errors.
- `out` is the sole ordinary observability/liveness-root modifier; duplicate `!> observable: true` metadata is removed.

### 10.1 Identifier conventions

- `PascalCase`: types, traits, structs, enums, objs, cells, protocol variants, and operation identities.
- `snake_case`: functions, fields, nodes, variables, and macros.
- `PascalCase#`: intrinsics.
- Lowercase: keywords.

## 11. Literals and collections

- Numeric suffixes are internally parsed uniformly rather than having dedicated `i32`/`f64` token families.
- Width is expressed through type annotation or cast.
- Custom `Parse` bindings are not exposed yet.
- Current staged numeric surface:
  - Decimal/float syntax.
  - `0x`, `0b`, and `0o`.
  - Canonical duration suffixes.
  - Unknown suffixes/prefixes are errors.
- Raw strings use `#r"..."`.
- Byte literals use `#b"..."`.
- `@`-quoted literals are removed.
- Formatting/interpolation uses explicit `format!(...)`.
- Ordered literals remain `[a, b]`.
- Type-directed associative literals use `[key => value]`; no `HashMap` is compiler-known.
- There is no universal `null`/`nil`; absence uses typed sum variants such as `Option<T>::None`.
- Regex uses a macro/plugin implementation such as `regex!(#r"...")`, not `/.../` lexer syntax.
- Adjacent prefix-discriminator literals such as `sql"..."` are removed; domain literals use explicit macro calls such as `sql!(...)`.

### 11.1 Slicing

- Bracket slicing follows Python style: `start:stop:step`.
- `...` remains a multidimensional ellipsis coordinate only.
- Standalone ranges use `start..end` and `start..=end`.
- Masking uses ordinary mask indexing, not `range; condition`.
- Named dimensions use `name => selector`, for example `tensor[time => 5, width => 0:10]`.

## 12. Files, targets, and backends

- Active backend documentation lists active backends only.
- Active targets:
  - LLVM/native and embedded.
  - Direct SPIR-V for `.abv`.
  - CIRCT for `.cbv`.
  - Webstack for `.rbv`.
- `.abv` canonically guarantees direct SPIR-V output through `rspirv`.
- LLVM GPU offload remains a separate optional `.bv` mechanism.
- Retired backends are removed from active module paths and retained only under `archive/`.
- Target restrictions move into configured capability profiles validated once by the frontend.
- `.c` filename modifier is removed.
- `.f` remains and must become a real strict indentation dialect:
  - No statement-block braces.
  - No semicolon terminators.
  - Token-aware parsing.
  - Original source spans preserved.
- `.s` remains a stronger verification profile, not a separate syntax grammar.
- Deprecated compact extensions such as `.sbv` are removed; dotted forms such as `.s.bv` are canonical.

### 12.1 Rendered Briv

- Conditional view inclusion uses `b-when="condition"`; legacy `b-if` is removed.
- View-bound values are not restricted by compiler-known primitive names.
- Web GLUE configuration supplies protocol casts/layout descriptors for supported boundary representations; unsupported values are rejected by the target capability validator.
- Custom component tags instantiate first-class reactive component instances rather than macro-expanded HTML.
- The rendered parent owns each mounted component handle; mounting creates it and unmounting releases its state and subscriptions.
- View attachment uses one form, `render Name { ... }`; `render struct`, `render obj`, `render cell`, and `rstruct` are not separate syntax.
- The compiler resolves the declaration kind and enforces the corresponding static-data, reactive-identity, or sealed-port visibility/lifecycle rules.
- Legacy `<script>` and `<script type="briv">` wrappers are removed; Briv source outside `<view>`/`<style>` is the sole `.rbv` source form.
- `b-when` structurally mounts/unmounts its subtree and therefore creates/releases owned component handles.
- `b-show` changes presentation visibility only and preserves DOM/component identity and state.
- Every `b-*` attribute uses the canonical Briv expression parser and type/effect analysis; JS-like ternaries and brace object-literal mini-syntax are removed.
- View expressions use ordinary `match`, associative `[key => value]` literals, calls, and bindings.
- Dynamic `b-each:item="items"` repetitions require an explicit stable `b-key="..."`; positional identity is not used for insertable, removable, or reorderable children.
- `b-bind:value="field"` remains only for assignable logical fields with proven write contracts; computed expressions require separate value and trigger handlers.
- Render expressions are pure/read-only; mutation, FFI, allocation, and spawning are allowed only in explicit event handlers or compiler-managed component lifecycle.

## 13. Data Briv

- `.dbvs` is removed and unified into `.dbv`.
- `.dbv` and `.dbvl` become distinct parser modes sharing a value/schema core.
- `.dbv`:
  - Structured/category data.
  - `>` introduces entries beneath a category.
  - Schema import: `schema Name from "file.dbv";`.
- `.dbvl`:
  - Exactly one record per physical non-instruction line.
  - `>` introduces non-data instructions.
  - Schema selection/import: `>schema Name from "file.dbv";`.
- Values remain raw until interpreted by an asserted schema.
- Without a schema, arbitrary scraped/raw data is allowed.
- If a schema is asserted, validation is complete:
  - Fields.
  - Types.
  - Constraints.
  - Named schemas.
  - Required/optional values.
  - Key uniqueness.
- Schema key fields automatically derive and enforce lookup keys.
- Quoted values are always supported.
- Canonical collection schema grammar:
  - `T[N]`.
  - `List<T>`.
  - `Map<K, V>`.
  - `Option<T>`.
  - `field?: T`.
- Lazy/streaming `.dbvl` is required.
- `.dbvl` supports append-only canonical writes.
- `briv check` directly validates `.dbv` and `.dbvl`.
- Data Briv gets deterministic canonical serialization.
- Remaining `.dbvs` and legacy data files must be migrated or archived.
- Human-authored per-language GLUE configuration migrates from `lib/glue/<lang>/glue.dbvl` to structured, multiline `lib/glue/<lang>/glue.dbv`.
- Generated `bridge-exports.dbvl` remains line-oriented machine metadata suitable for streaming and downstream tooling.

## 14. Tooling and documentation

- Documentation authority:
  1. `spec/SPEC.md`.
  2. `docs/architecture/`.
  3. `learn-briv/`.
- The SPEC is normative with explicitly staged/unimplemented sections.
- Normative SPEC examples become executable conformance fixtures.
- Lexer/parser/tooling vocabularies come from a shared machine-readable manifest.
- The LSP uses the actual compiler parsers and analyses.
- Formatter output must round-trip to an equivalent AST.
- CI parses and typechecks every active shipped source/data file.
- Any excluded source must live under an explicit archive boundary.
- Legacy active-looking docs/examples move under `archive/`; timestamped plans remain immutable.
- `export` is the sole export syntax; `#export` is removed.
- No compatibility parser or `briv migrate` tool is added while the language remains pre-adoption; active source is rewritten directly to canonical syntax.

## 15. Rename status

- Full active rename to Briv was selected.
- Static inspection currently finds no active `Brief`, `brief`, or `dbrief` module/build residue.
- Runtime, crate, binary, syntax-highlighter, and Data Briv paths appear consistently renamed.
- A fresh build/test verification is still pending.
- No compiler or language implementation changes were made as part of the audit before this draft was created.

## 16. Pending audit work

The audit is substantially complete. Remaining work includes:

1. Sweep `lib/std/`, `lib/compiler/`, examples, fixtures, and benchmarks for the removed/renamed forms recorded in this draft (for example `import#`, `uni`, `:>`, `prop`, `sig`, `state`, `rstruct`, `frgn!`, `syscall!`, `term!`, `escape`, `.dbvs`) and rewrite them to canonical syntax; any deliberately non-canonical files move to `archive/`.
2. Rewrite `spec/SPEC.md` grammar/keyword/extension tables to match these decisions; it currently teaches `sig`, `state`, `constant`, `rstruct`, `render struct/obj`, `[#]`, and `trigger`.
3. Enforce the shared language manifest for lexer, LSP, highlighter, and formatter vocabularies, removing `sed`/`pvt`/`reg` (kept reserved), `|>`, `++`, `Ok`/`Err`/`Some`/`None`, `Ptr!`, `FrgnBang`, `TermBang`, and legacy pragma tokens.
4. Migrate human-authored `lib/glue/<lang>/glue.dbvl` configs to `.dbv`; keep generated `bridge-exports.dbvl` line-oriented.
5. Complete interpreter/backend parity work for the accepted semantics (first-class callables, generic values, implement-or-hard-error backends).
6. Produce a prioritized implementation sequence with migration order, compatibility policy, documentation updates, verification commands, and risk analysis.
7. Run build, test, lint/typecheck, and Praetor verification only after implementation work is authorized.

## 17. Required documentation maintenance for later implementation

Any implementation plan derived from this draft must update, as applicable:

- `spec/SPEC.md` as the normative language contract.
- `docs/architecture/` for declaration semantics, reflection, casting, effects, ownership, concurrency, targets, Data Briv, and backend contracts.
- `learn-briv/` for canonical examples only.
- The shared language manifest and generated highlighter/LSP vocabularies.
- Formatter/round-trip fixtures.
- Parser, interpreter, active-backend, and target-profile conformance tests.
- Migration diagnostics for every removed syntax form.
- `BUGS.md` for concrete root causes discovered during implementation.

Timestamped historical plans are not retroactively rewritten. Obsolete active documentation is archived rather than left alongside normative material.
