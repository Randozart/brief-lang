# Spec Conformance: Gap Analysis → Full Sequenced Remediation

**Date:** 2026-08-22
**Status:** Active plan
**Scope:** Close every gap between `spec/SPEC.md` (Draft 2026-08-05) and the implementation, per owner decisions below.
**Method:** Audit verified each normative SPEC claim against parser / typechecker / backend / interpreter with file:line evidence (§ "Gap inventory"). Line anchors were true at plan time; re-grep before editing (code moves).

## Decisions (owner, 2026-08-22)

| Question | Decision |
|---|---|
| Scope | **All of it, sequenced** (reject → deviations → features, dependency order) |
| Code-vs-spec conflicts | Case-by-case, resolved here: |
| — `if`/`else` accepted | **Remove** (spec-pure). Migrate ~11 active sites. |
| — glob imports work | **Remove** (spec-pure). Zero users. |
| — `.s` strict thin | **Build enforcement.** Dotted-flag form ONLY: `.s.bv`, `.s.rbv` — never `.sbv`/`.srbv` (compound). `conformance::classify` already rejects compound forms; preserve + keep tested. |
| Slicing masks/ellipsis | **Build both** |
| Diagnostics quality | **Include** |
| obj/cell port model | **Build fully** (both obj ports and sealed cells) |
| Build list | match checks, structural sums, `dyn` dispatch, task lifecycle, ports, masks+ellipsis, `.s` |
| Dead surface (`$!` token, `StateDecl`/`Signature` AST variants, `input`/`output` lexer tokens, orphan fixtures `tests/test_strict.sbv` + `tests/test_sig*.bv`, `Ok/Err/Some/None` vocab rows contradicting §4.4, `is_reserved()`/`is_removed_keyword()` zero-caller status beyond what Phase 2 adds) | **Defer** → record in BUGS.md, fix opportunistically when touching those files |

## Ground rules (from AGENTS.md, binding throughout)

1. Contract-first: never weaken a contract to make code pass.
2. Additive codegen paths only (`_ => return None;` fallthroughs unchanged).
3. Interpreter is reference: feature works in interpreter BEFORE backend.
4. No type-name matching in Rust (Rule 19): sums, sums-in-match, dyn tables, and event delivery key on universe/casting-graph/op surface, never on `"Int"`/`"Event"` strings. Compiler constructs (Ptr/Vector/Bits precedent) may add AST `Type` variants — `Type::Sum`, `Type::Task` qualify as compiler constructs.
5. Every phase: `cargo test --lib` green, Praetor on changed dirs (`praetor validate --warn --target <dir>`, directory not file), docs updated same commit, continuous commits per logical step, no `todo!()`/`unreachable!()` in committed code.
6. Flat control flow (≤2 nesting), HashMap iteration sorted where it emits IR.
7. Kani harnesses for safety-critical additions (task cancellation proofs).
8. Behavioral tests: test the contract, not the implementation.

---

## Gap inventory (audit result, evidence)

### Tier 1 — normative features missing
| # | Gap | SPEC | Evidence (at audit) |
|---|---|---|---|
| G1 | obj/cell port model: `(inputs) -> outs` headers unparseable; no `Event<T>` anywhere; cell body skipped token-by-token into empty skeleton | §9.5/§9.6 | `src/parser/definitions.rs:2170` (`parse_obj_like` body-only), `:851–889` (`parse_cell` skip loop) |
| G2 | Match semantics: exhaustiveness / unreachable-arm / arm-type-compat all absent; `infer_match` returns first arm's type | §11.3 | `src/typechecker/mod.rs:2106–2117` |
| G3 | Structural sums `Int \| String` + typed-binding arms `n: Int => …` | §8.4 | no pipe path in `parse_type`; `Pattern` has no typed-binding variant (`src/ast/expr.rs:212`) |
| G4 | `dyn Trait` runtime dispatch | §8.6 | zero occurrences outside Rust's own `&dyn Fn` |
| G5 | Task lifecycle: drop-live-handle error, `free task` cancellation gate, `keep` transfer | §12.2 | `FreeHint`/`KeepHint` are storage hints only (`src/ast/top.rs:302–308`); spawn/await exist (`src/parser/expressions.rs:443–510`, `SpawnStorage` `src/ast/expr.rs:14–32`) |
| G6 | Python slicing: boolean mask indexing, `...` ellipsis | §16.5 | slice start:stop:stride + named dims exist (`expressions.rs:339–386`); `Token::Ellipsis` lexed never consumed (`lexer.rs:457`) |
| G7 | `.s` strict: representation-fallback rejection + unresolved-lifetime escalation missing; sole `.s` gate is SRBV view refs | §3.2 | `src/compile.rs:506–518`, `conformance.rs:84–91` |
| G8 | Diagnostics: no did-you-mean for misspelled keywords; casing table unconsumed; reserved `sed`/`pvt`/`reg` usable as identifiers; staged features fail generically instead of `StagedFeature` | §4.1, §1 | `errors.rs` has hint plumbing (`:83,:129,:230`) nothing produces hints; `vocab.casing` (`vocab.rs:92,:287`); `error_staged` exists (`parser/helpers.rs:328`) underused |

### Tier 2 — deviations (code ≠ spec letter)
| # | Deviation | SPEC | Decision |
|---|---|---|---|
| D1 | `if`/`else` fully parseable (`statements.rs:209–231`, `Statement::If` consumed by 43 files) | §11.1 | Remove |
| D2 | Quoted-path glob imports expand (`import_resolver.rs:615–624`, `resolve_glob` `:940–989`, positive tests `~1376+`) | §7.2 | Remove |

### Already conformant (no action)
File variants/profile detection, dotted-profile parsing (`.f` thorough), delimiters/arrows, all 22 grammar item kinds (cell body aside), imports (aliases/selective/re-export/conflict/cycle), init bounded value sets, `$` bindings/stages/`:=`/Error# usage-gate, seq/pack struct/spec metadata/atomic fields/union/trap, coll scaffold (+`backend/llvm/coll_scaffold.rs`), op-as-member iteration tiers, transfer arrows + Phase D consume flag, reflection stored-length rules, accel GPU offload (`backend/llvm/kernel.rs`: kernel collect, SPIR-V compile+embed), watchdog parse + `Now#` deadline codegen, beginprogram goal-reachable + mutual-exclusivity proofs (`typechecker/mod.rs:3472,3551–3588`), global XOR concurrency gate (`analysis/concurrency_gate.rs:103–113`), mutex/barrier/defer, box/spill spawn storage classes, frgn provenance/optional/variadic, asm decls, rbv directives + reset txns + mount forms, dbv/dbvl parsers + schema validation + serialization, fmt round-trip, memcheck subcommand.

---

## Phases

Execution order = dependency order. Phases 1–2 independent; 3→4 chained; 5,6,7 independent of 3/4; 8 after 5's ABI decisions are irrelevant but after spawn infra review; 9 last (consumes memcheck/lifetime maturity); 10 closes.

---

### Phase 0 — Baseline & bookkeeping

- [ ] `cargo build && cargo test --lib` green at plan commit; note count.
- [ ] Record benchmark baseline ONLY if a phase touches codegen hot paths (Phases 3–8 touch codegen; Rule 12 requires baseline before perf-relevant change): run `bash benchmarks/build_and_bench.sh --runtime` once before Phase 3, save output to `benchmarks/results/` via the harness's normal flow. Do NOT hand-time.
- [ ] BUGS.md: append "Deferred dead surface" entries (decision table row) listing: `$!` token no consumer (`lexer.rs` DollarBang), `StateDecl`/`Signature` legacy AST variants, `input`/`output` cell-file tokens, orphan fixtures `tests/test_strict.sbv`, `tests/test_sig*.bv`, `Ok/Err/Some/None` vocab rows mislabeled Removed (`vocab.rs` — behavior already matches spec; label wrong).
- [ ] Commit plan doc + BUGS.md.

**Done when:** tests green recorded, BUGS.md carries deferral list.

---

### Phase 1 — Spec-pure removals

#### 1a. Remove glob imports (D2)

Files: `src/import_resolver.rs`.

Steps:
1. Delete call site: the `let is_glob = …; if is_glob { return self.resolve_glob(…) }` block (~`:620–624`).
2. Delete `fn resolve_glob` (~`:940–989`).
3. Delete positive glob tests (search test module for `glob`); ADD rejection tests: `import "./dir/*.bv";` and `import "./dir/**";` produce a syntax/diagnostic error naming the rule ("glob imports are invalid — import files explicitly", house style: what/why/fix).
   - Enforcement point: reject at PARSE time if feasible (path is just a string literal there — do it in resolver where the path string is inspected, matching where `resolve_glob` lived). Error kind: reuse an existing diagnostic shape that renders with span; message states the fix.
4. Update any docs mentioning glob support (`grep -rn "glob" docs/ learn-briev/ spec/` — spec already forbids; fix strays elsewhere).

Acceptance: `cargo test --lib` green; new negative tests prove `*`/`**` paths rejected; no active-source behavior change (zero users).

#### 1b. Remove `if`/`else` (D1)

This deletes `Statement::If` across its 43 consumer files. Mechanical but wide; do in two commits:

Commit 1 — parser rejects:
1. Delete `parse_if_statement` (`statements.rs:209–231`) and the `Token::If` dispatch arm (`:106–107` region).
2. Replace acceptance with a dedicated diagnostic: "`if`/`else` do not exist — use `when cond { … };` for one-sided branching or exhaustive `match`" (house style: what/wrong/proof/fix). Implementation: small helper in `helpers.rs` next to `error_staged` (NOT StagedFeature — this is removed surface, wording must say removed, suggest replacement).
3. Keep `Statement::If` AST variant temporarily so the tree still compiles; existing AST-level tests referencing If get updated/deleted in commit 2.

Commit 2 — excise the variant:
4. Remove `Statement::If` from `src/ast/top.rs` enum; delete every `Statement::If` match arm in all consuming files (43 files incl. analysis/*, typechecker, reactor, proof_engine, interpreter/eval.rs, backend llvm+vm emit_stmt, beast serialize/deserialize, beastpack, annotator, macros/eval+selection, derive/verify_smt, normalizer, display, canonical, helpers). Each deletion is "remove the arm" — no replacement logic anywhere (nothing legal produces it anymore). Beast format: bump or handle absence of If payloads (verify serializer round-trip tests still pass; if wire format versioned, increment).
5. Migration of active sources (exact inventory, grep-verified at plan time):
   - `benchmarks/enemy_swarm.bv:17` `if i == 0 {` → `when i == 0 { … };`
   - `benchmarks/enemy_swarm.bv:37–41` `if hp[i % 64] > 0 { … } else { … }` → two complementary guards or `match hp[i % 64] > 0 { true => …, false => … };` (pick whichever preserves semantics exactly; verify output equality by running the benchmark binary once pre/post migration).
   - `benchmarks/sweep_arr.bv:13` → `when i == 0 { … };`
   - `examples/fizzbuzz.bv:10–16` else-if chain → `match (is_fizz, is_buzz) { (true, true) => …, (true, false) => …, (false, true) => …, (false, false) => … };` (tuple patterns exist: `Pattern::Tuple`). Verify printed sequence identical pre/post.
   - `examples/pointer-trickery.bv:62` → `when … { … };`
   - `examples/ptr-arithmetic.bv:69,78,90` (one else chain) → when/match split preserving exact UART register-write order.
6. Sweep ALL other `.bv/.ebv/.rbv/.f.bv` under `lib/ examples/ benchmarks/ .smoke/ learn-briev/ docs/ spec/` for `if `/`} else`/`else {` occurrences; migrate every hit (grep is source of truth; the list above was current at plan time).
7. Check `learn-briev/` tutorial chapters and syntax highlighter (`grep -rn '"if"'` highlighter sources, `vim-syntax`/`tree-sitter` dirs if present) — remove `if` as keyword/color; update SPEC cross-references none needed (spec already forbids).
8. Layout frontend: confirm `.f` never allowed braces anyway; no special case needed.

Acceptance: `rg -n "\bif\b|\belse\b" lib examples benchmarks .smoke learn-briev -g '*.bv' -g '*.ebv'` returns only false positives (identifiers/comments containing the substring inside words) — eyeball each; `cargo test --lib` green; fuzz corpus (`src/fuzzing`) has no if-based cases left (delete/update if found); enemy_swarm/fizzbuzz outputs byte-identical pre/post migration.

---

### Phase 2 — Diagnostics quality (G8)

1. **Did-you-mean helper.** New centralized fn (home: `src/errors.rs` or `src/vocab.rs` — put in vocab.rs next to keyword tables; DRY single home):
   ```rust
   pub fn closest_keyword(input: &str, candidates: impl Iterator<Item = &'static str>) -> Option<&'static str>
   ```
   Levenshtein ≤ 2 (transpositions optional; keep simple, tested). Wire producers:
   - Parser top-level dispatch fallthrough (`helpers.rs:194–199` "unexpected top-level item"): if the offending identifier is within distance 2 of ANY canonical keyword (`vocab.canonical_keywords()`), emit `UnexpectedKeyword { found, suggestion }` rendering "= hint: did you mean `node`?" via existing `with_hint` plumbing.
   - Statement-position unknown identifier dispatch: same treatment.
   - Undefined-variable diagnostics: attach available-names suggestion using slot lookup already present (`errors.rs:350` — Display currently drops `available:`; render it as a hint line, capped at ~5 candidates).
2. **Reserved words.** In `expect_identifier`/identifier-producing helpers (`parser/helpers.rs:383–396`), reject `sed`/`pvt`/`reg` via `vocab::is_reserved()` (this gives it its first caller): "reserved for future language contracts". Tests for all three.
3. **Casing advisory.** New warning pass (site: end of typechecker top-level walk, or standalone lint invoked from compile pipeline after name resolution): for user-declared names, consult `vocab.casing` categories; violations emit warning diagnostics (non-fatal), e.g. function named PascalCase → "functions are snake_case by convention (advisory)". Gate behind nothing (always on, informational severity). Tests: one violation per category triggers, correct names silent.
4. **Staged-feature sweep completion.** Grep remaining known-unimplemented surfaces reached by parser and route them through `error_staged`: at minimum square-bracket generic ALIAS use where distinguishable from containment is NOT distinguishable — skip; focus on: `...` ellipsis until Phase 6 lands (add then remove in Phase 6), any `frgn?`-adjacent forms not covered. Verify each of §4.4 removed forms produces EITHER a removal-named diagnostic (meld does) or a clear generic error; upgrade `term!`/`trg!`/`sync!`/`frgn!` postfix-bang-on-keyword cases to explicit "removed form" messages IF they currently pass silently as identifiers followed by `!(` expansion (test each spelling).

Acceptance: misspelling `nod foo…` yields hint naming `node`; `sed x` rejected; casing warnings appear in a demo file; `cargo test --lib` green; Praetor clean on touched dirs.

---

### Phase 3 — Structural sums (G3)

SPEC §8.4: `Int | String` anonymous sum; matched by typed bindings `number: Int => use_int(number)`.

Design decisions (binding):
- AST: `Type::Sum(Vec<Type>)` — flat list (A|B|C nests trivially). New `Pattern::TypedBinding(String, Type)` for `name: Type =>`.
- Grammar: in `parse_type` postfix loop, on `Token::Pipe` continue collecting operand types into `Type::Sum`. Precedence: `|` binds loosest within type grammar (function arrows inside operands OK; avoid conflict with `||` — distinct tokens).
- Universe: `TypeUniverse` registers sum types like any constructed type (normalizer registers; NO name keys — identity is structural member list).
- Typing: scrutinee of sum type; arm binding types must EACH be members (identity by casting-graph equivalence, not string equality); expression-arm result types unify (full check lands in Phase 4; Phase 3 keeps first-arm behavior for non-sum matches unchanged — additive).
- Exhaustiveness for sums also Phase 4; Phase 3 delivers: parse, infer, construct, match-with-typed-bindings runs correctly in INTERPRETER.
- Interpreter value model: inspect `src/interpreter/eval.rs` value enum; add `SumValue { member_index: usize, payload: Box<Value> }`. Construction sites: values of sum-typed bindings arise from scrutinee expressions returning different member types — interpreter tags dynamically by matching member type against sum members.
- Codegen (LLVM): tagged layout `{ tag: i64, payload: largest-aligned-member }` derived via casting graph `resolve_llvm_type` on members (Rule 19 compliant); construction inserts tag; typed-binding arm extracts after tag compare. Additive match-lowering arm keyed on `Type::Sum` scrutinee.
- Contracts: sums are ordinary types in contract positions (no special case).

Tests: parse round-trip; interpreter program constructing both members through branches and reading back; LLVM compile+run parity vs interpreter on the same fixture (add to existing interp/backend parity harness location used by other type tests); negative: arm binds a non-member type → error naming members.

Order: interpreter first (Golden Rule 5), then LLVM.

Acceptance: fixture `sums.bv` with `Int | String` flows through contracts, match, foreach-free consumption identically in interp + compiled binary.

---

### Phase 4a — Complete pattern grammar (amended in 2026-08-22 session; owner: enrich stmt-match too)

Discovered during Phase 1b migration: tuple patterns are declared (`ast::expr::Pattern::Tuple`), displayed, and matched by the interpreter (`interpreter/eval.rs:839`) but UNPARSEABLE — `parse_pattern` (expressions.rs:783) lacks a `(` arm. Two divergent pattern grammars exist:

1. **Unify on one grammar (DRY):** statement-match's ad-hoc `StmtMatchPattern` {Wildcard, Int-literal, String, Multi} folds into `ast::expr::Pattern`. Add `Pattern::Multi(Vec<Pattern>)` for `|`-separated or-arms; both match forms support them. Statement arms keep block bodies + trailing `;`; expression arms stay exprs. Delete `StmtMatchPattern`/`StmtMatchArm` once migrated.
2. **Tuple patterns:** `(p1, p2, …)` arm in `parse_pattern`, recursing into full pattern grammar. Typing: scrutinee must be a tuple of equal arity; each sub-pattern compatible member-wise. Interpreter already works; add typecheck + backend parity test.
3. **Bool literal patterns:** parse path exists (expressions.rs:811); typing/coverage verified end-to-end as part of this phase.
4. **Range patterns** (`1..5`, `1..=5` — parse today): coverage semantics = contributes its bound interval to exhaustiveness v1 only via `_` fallback elsewhere (no adjacent-interval solver); document.
5. **Enum-variant patterns:** unchanged (working).
6. **fizzbuzz restored** to the elegant tuple form:
   ```briev
   let result: String = match (is_fizz, is_buzz) {
     (true, true) => "FizzBuzz",
     (true, false) => "Fizz",
     (false, true) => "Buzz",
     _ => String(current),
   };
   ```
   and becomes the feature's regression fixture (tuple-scrutinee + Bool literals + wildcard fallback). enemy_swarm keeps integer statement-match (`0 => / _ =>`) — that form's natural shape.

Acceptance: fizzbuzz tuple fixture passes interp + compiled parity; `|`-or-arm parses in BOTH match forms with correct first-match semantics; stale `StmtMatchPattern` fully gone; negative: arity mismatch, non-tuple scrutinee with tuple pattern.

---

### Phase 4 — Match semantics (G2)

Replace `infer_match` stub (`typechecker/mod.rs:2106–2117`).

Algorithm (single pass over arms):
1. Compute scrutinee closedness: enum → closed over its `__variant_*` slots (names from TypeDef, NOT string-matched semantics — they ARE declared slots); `Type::Sum` → closed over members; anything else → open (requires `_` arm).
2. Coverage set: fold arms; each contributes: `EnumVariant(name,…)` covers that variant (recurse payload patterns only for nested-exhaustiveness v2 — OUT of scope, note in doc); `TypedBinding(_, T)` covers sum member T; `Wildcard`/`Binding` covers everything (open or closes any gap); `Literal(e)` covers constant domain only when scrutinee is Bool (true/false literal coverage) — integer literals stay OPEN unless `_` follows (no range-domain reasoning v1).
3. Errors (all with spans + house-style proof text):
   - Non-exhaustive closed scrutinee without `_`: list uncovered variants/members.
   - Unreachable arm: arm whose pattern is fully subsumed by prior coverage (duplicate enum variant, duplicate literal, TypedBinding of already-covered member, any arm after a covering wildcard). SPEC: unreachable arms are ERRORS.
   - Arm-type compatibility: unify all arm body types; mismatch = error showing both types + spans (trap bodies exempt — never-type).
4. Guards: an arm with `when` guard does NOT close coverage (condition unknown) — it contributes coverage only if no later arm needs it for exhaustiveness AND we report "guarded arm cannot satisfy exhaustiveness" when it's the sole coverer of a variant.
5. Result type = unified arm type (replaces first-arm inference).

Interpreter: no change (it evaluates matched arm). Backend: no lowering change.

Tests: exhaustive enum ok; missing variant → error lists it; wildcard-after-specific → specific fine, duplicate-specific → unreachable error; guarded-only coverage error; mixed-type arms error; bool literal match exhaustive without `_`; int literal match requires `_`.

---

### Phase 5 — `dyn` trait dispatch (G4)

SPEC §8.6: `let value: dyn Printable = source;` explicit runtime dispatch.

Design decisions (binding):
- Grammar: `dyn` recognized in TYPE prefix position (check_identifier("dyn") before a trait name) → `Type::Dyn(Box<Type>)` wrapping the TRAIT type. Parse-side only; no Token needed (contextual, mirrors `proto`/`init` handling).
- Assignment rule: coercion concrete→dyn allowed ONLY into an explicitly `dyn`-annotated binding/param (spec: explicit). Reverse coercion: not in v1 (document).
- Representation: fat pointer `{ data: ptr, table: ptr }`. Table generated per (trait, concrete type) pair at monomorphization: one thunk per required trait fn signature + per required op binding, calling the concrete impl. Consistent with boxed-generic ABI precedent (generics erase to boxed i64).
- Call resolution: method/op call on `Type::Dyn` value loads thunk from table by fixed slot index (slot assignment = declaration order in trait; deterministic, sorted where iterated).
- Effects/lifetime of captures: closures-as-dyn OUT of scope v1 (note); dyn applies to named-type/trait dispatch.
- Interpreter: dynamic dispatch naturally — resolve through trait's registered conformance for the VALUE'S runtime type; add `DynValue { trait_name, inner }` wrapper carrying the trait identity for resolution (trait identity by registered name in universe — acceptable: traits are compiler-visible declarations, this is dispatch not semantic typing).
- Restriction documented: trait must be locally visible; requirements limited to defn-shaped signatures + op bindings (field requirements unsupported v1 → staged error if attempted).

Tests: trait w/ default + override dispatched correctly; two concrete types through one dyn binding; passing dyn to defn param; negative: implicit coercion without `dyn` annotation errors; field-requirement trait → staged error.

---

### Phase 6 — Slicing completion (G6)

1. **Boolean masks.** Index position holding an expression that infers as a vector/array of Bool (or Bool-sum-of-vector context) lowers to mask select. Resolution ORDER matters and is pinned: try element-index read (`op At`/vector index) FIRST with the expression as scalar index; if the INDEX EXPRESSION's inferred type is Bool-array/vector while receiver is same-shape numeric vector → mask select (compacted output length = count of true; runtime popcount-based compaction loop in LLVM, filter in interpreter). Ambiguous cases (index type not provably Bool-sequence) → plain indexing. Document precedence in code comment + SPEC already implies masks are ordinary mask indexing.
2. **Ellipsis.** `...` inside `[start:stop:step, ...]` multi-dim slice coordinates: consume `Token::Ellipsis` in the bracket-parse loop (where named dims + ranges already parse, `expressions.rs:339–386`) producing `SliceDim::Ellipsis` filling remaining dimensions with full-range. V1 scope: valid in multidim slices over vectors-of-vectors up to depth 2 (deeper → staged error). Single-dim `array[...]` = whole array copy.
3. Interp first, LLVM second (mask select → shufflevector-free compaction loop; ellipsis → dimension desugar).
4. Remove Phase 2's temporary staged-error for `...` once live.

Tests: mask filters Int vector (interp+LLVM parity); mask on String chars errors cleanly (chars aren't mask-selectable v1 — document); `t[1:3, ...]` equals `t[1:3, :]`; empty-mask result is empty vector; composed named-dim + mask.

---

### Phase 7 — obj/cell port model (G1) — LARGEST; sub-plan required before coding

Write `docs/plans/2026-08-22-obj-cell-ports.md` expanding this section into step detail AFTER Phases 3–5 land (their machinery — sums for multi-event products? no — ports don't need sums; dependency is really Phase 4's matcher maturity for node guards over port state). Sequence within master plan stands.

Design direction (binding):
- **AST:** extend the shared obj/type header parser: `obj Name<T>(in1: Type, in2: Type) -> out1: Type, out2: Type { … }`. Both sides optional. Store on `TypeDef`: `ports_in: Vec<(String, Type)>`, `ports_out: Vec<(String, Type)>`. Same header grammar for `cell`.
- **Cell body:** replace skip-loop (`definitions.rs:851–889`) with real recursive item parsing reusing the obj-like body walker (slots, metadata clauses, members txn/node/defn, internal trg). Cell gains `parameters` (already in skeleton struct), outputs, fields populated.
- **Event<T>:** ordinary generic type declared in stdlib (`lib/std/event.bv` or collections-adjacent module) with minimal op surface (post/read-style ops). COMPILER knows PORTS (AST-level, structural), never matches `"Event"` strings (Rule 19). Port types unrestricted — Event<T> is convention for reactive payloads, enforced nowhere by name.
- **Semantics:** input port = per-instance reactive source (like `trg` bound to instance); firing a member txn that writes an output port publishes to subscribers (objects wired by constructor argument passing, mirroring spec example `obj Enemy(damage: Event<Damage>) -> died: Event<EnemyId>`). Delivery order deterministic (scheduler order, no implicit concurrency — XOR gate still governs).
- **Cell sealing:** external references to cell internals (fields/members) = compile error; interaction only via ports. Enforced in typechecker name-resolution: cell members namespaced private except ports.
- **Runtime:** interpreter first (port queues as scheduler-visible channels), then LLVM (instance layout grows port slots; publish = enqueue + mark ready).
- **rbv tie-in (§21.3):** `render Name` resolves against obj WITH ports; component_instances analysis consumes port declarations for trigger wiring.

Sub-phases: 7a parse+typecheck obj ports; 7b cell real parse + sealing checks; 7c interpreter event delivery; 7d LLVM; 7e rbv integration. Each independently committable.

Acceptance: spec example shape `obj Enemy(damage: Event<Damage>) -> died: Event<EnemyId>` parses/typechecks/runs in interp; cell Timer ticks via port only; sealed-access negative test; rbv counter component still passes existing suite.

---

### Phase 8 — Task lifecycle (G5)

- `Type::Task(Box<Type>)` compiler construct (Ptr-precedent) for handles. `spawn defn(…)` produces it (obj spawns keep current Custom-instance typing — task rules apply to TASK handles only, per spec §12.2 wording).
- **Linearity:** await/free/keep each CONSUME the handle (move). Second use = use-after-move error (extend existing move-checking used for consume ownership — locate during execution; if none exists for locals, add minimal linear tracking scoped to Task-typed bindings only — additive).
- **Drop-live-handle:** `lifetime.rs` inject_drop pass: Task-typed binding reaching scope end without consumption = COMPILE ERROR "live task handle dropped — await, free, or keep it" (not auto-drop; spec forbids silent discard).
- **`free task`:** permitted only when effect analysis proves cooperative cancellation points reachable in callee (callee contains await/port-wait/cancellable-FFI markers) — reuse/extend effect inference; if effects too weak v1, free REQUIRES the callee be annotated `cancellable` (new contextual modifier on defn; disclosed strategy keyword per §8.1 philosophy). Runs defer cleanup (defer stack already LIFO on rollback/term/endprogram — hook free path).
- **`keep task`:** suppresses drop error; transfers ownership to enclosing boundary (export/FFI boundary or parent scope). Semantics v1: annotation of escape, no runtime effect beyond suppressing the error + marking handle escaped in lifetime analysis.
- Kani harness: free-before-await twice cannot typecheck; handle escaping via return requires keep.

Tests: drop-handle error; double-await move error; free on non-cancellable → error naming fix; happy path spawn→await value round-trip (exists? verify) + spawn→free cleanup ordering observed via defer side effect.

---

### Phase 9 — `.s` strict enforcement (G7)

Constraint (owner): dotted-flag forms only. `conformance::classify` already rejects `.sbv`/`.srbv` (tested) — PRESERVE; add one more regression assert that `.s.ebv` etc. classify fine (profile composes with every base ext).

1. **Representation-fallback rejection:** memcheck (`macros/memcheck.rs`) already inventories memory-decision points including fallback-tier outcomes. Under `is_strict(path)`, compile pipeline runs the memcheck decision scan and any fallback-classified decision = hard error citing the decision point + why ambiguous. Wire-through: compile.rs gains strict flag from `conformance::is_strict`; pass into the scan.
2. **Lifetime escalation:** `lifetime.rs` today silently services unresolved lifetimes. Emit WARNING normally (new, informational — "unresolved lifetime serviced by boundary collector") and HARD ERROR under strict. Requires classifying which drops are 'resolved' (proven last-use) vs 'serviced' — the inject_drop pass already distinguishes manual-free/auto scopes; extend classification there.
3. **Trust-boundary report:** under strict, collect trusted FFI/axiom edges encountered (frgn declarations with trusted status) into a verification report emitted to `<out>.report.txt` (path scheme: alongside artifact) listing each trust boundary — satisfies §3.2 bullet list visibly.
4. Existing global gates (tautologies, concurrency) unchanged; strict adds, never relaxes.

Tests: `.s.bv` file with fallback decision fails citing point; same file sans `.s` warns/passes; unresolved lifetime errors under `.s`; report file contains the trusted frgn listed.

---

### Phase 10 — Conformance sweep + closure (§23.4)

1. Wire `discover_active_sources()` (`conformance.rs:167`, currently zero external callers): new integration test (or bin mode `brievc check --sweep`): iterate discovered sources; for each Briev-family file parse+typecheck under classified profile; dbv/dbvl validated via their checker. Failure = test failure listing file+diagnostic. Runs in `cargo test --lib` CI path.
2. Fix whatever the sweep finds (expected: stragglers from earlier phases).
3. Docs sync final pass: learn-briev chapters affected (if/else removal, sums, match rules, dyn, ports, tasks), `docs/architecture/` updates for new AST types (`Type::Sum`, `Type::Task`, `Type::Dyn`, ports on TypeDef) in backend-type-dispatch.md + casting-protocol.md as applicable.
4. Final full harness run; record results; close plan with outcomes appended (results section below).

---

## Verification protocol (every phase)

1. `cargo test --lib` before each commit; no new warnings.
2. `praetor validate --warn --target <changed-dir>` per changed directory.
3. Interpreter/backend parity fixture for each language feature (Phases 3–8).
4. Negative tests carry house-style messages (what's wrong + proof/why + concrete fix).
5. Benchmarks: before/after table for any phase touching emission (3,4,5,6,7d,8) — Rule 12; regressions investigated via `compare_baseline.sh`, never excused.

## Results (appended at completion)

TBD — phases append outcome lines: date, phase, commits, test delta, benchmark deltas.

### 2026-08-22 session 1
- **Phase 0** ✅ c069b680 — baseline 1903 tests; deferrals in BUGS.md.
- **Phase 1a** ✅ ff7702c6 — glob imports removed, rejection tested.
- **Phase 1b** ✅ f7d91644 + 7f053f72 + c13cbdd9 — if/else rejected at parse,
  Statement::If excised crate-wide (62 sites), active sources migrated;
  two latent match-codegen bugs fixed en route (phi typing + edge labels).
- **Phase 2** ✅ 5dbac11e — did-you-mean hints, sed/pvt/reg rejection,
  casing advisory pass wired into build+check; op `reg:` discriminator kept
  contextual.
- **Phase 3** ✅ 2199f5a7 — sums parse/typecheck/interpret; backend staged
  explicitly pending ABI.
- **Phase 4a** ✅ dc59a39e — unified pattern grammar; tuple patterns dispatch
  in LLVM (memberwise heap-image compare); fizzbuzz restored to tuple form;
  dead match_normalize removed.
- **Phase 4** ✅ 65a76d0e — match semantics engine: closed-domain
  exhaustiveness (sums/enums/Bool), open-scrutinee `_` requirement,
  unreachable arms, non-member typed bindings, arm-result unification with
  contextual union escape.
- **Phase 3b** ✅ 574330c5 — tagged-union ABI: i64 handles to {tag,payload}
  images; boxing at call args, heterogeneous arms (probe-pass shape
  prediction), term-of-union, lets, state fields; TypedBinding tag-test +
  payload unbox. examples/structural_sums.bv verifies all seams compiled.
- **Phase 6 partial** ⏳ aee58b05 — masks already correct in interpreter;
  LLVM state-field mask routing segfaults → repro + fix direction recorded
  in BUGS.md. Ellipsis still needs SliceDim AST.

Suite: 1903 → 1916 passing, 0 failures. Remaining: Phases 5 (dyn), 6-finish
(mask codegen + ellipsis), 7 (obj/cell ports), 8 (task lifecycle),
9 (.s strict), 10 (conformance sweep).

### 2026-08-22 session 2
- **Phase 6a** ✅ 940239aa — mask segfault root-caused to TWO stacked faults:
  raw `[len,e…]` gather buffers consumed as tier Lists, and Bool[N] columns
  (`[N x i8]`) read as i64 masks. Tier boxing shared via
  box_gather_as_tier_list; i8-mask runtime variants routed on the stored
  column LLVM type. examples/mask_select.bv pins 13580.
- **Phase 6b** ✅ 9e6d4880 — `a[...]` full-range ellipsis live; multidim
  selectors staged explicitly with named forms; state-column vector slices
  lower to real range gathers (briev_slice_range64/_f32); Vector slices
  infer List<T> (checker/backend aligned). PRE-EXISTING guard-merge
  dominance bug found and fixed (merge phis for conditionally-written
  fields) — it had been silently breaking every multi-guard program shape.
  examples/slice_state.bv pins 56857.
- Deviation from plan: multidim ellipsis (`t[1:3, ...]`) is staged-rejected,
  not built — needs a SliceDim dims-list AST before it can exist honestly.
  Float64 slice/mask gathers still panic (explicit, same as before).
- New pre-existing bug class surfaced by the dominance fix: NONE remaining
  open — both repros closed.

Remaining after this session: Phases 5 (dyn), 7 (obj/cell ports),
8 (task lifecycle), 9 (.s strict), 10 (conformance sweep).

### 2026-08-22 session 2 (cont.) — Phase 9
- **Phase 9** ✅ — `.s` strict enforcement:
  - GlobalLifetime now records WHY each heap field fell back
    (`lifetime_fallbacks`: no reader / last consumer not foldable) —
    single source of truth shared by memcheck output and strict.
  - `analysis::strict::enforce` escalates fallbacks to hard errors under
    dotted-profile sources (`.s.bv`, `.s.rbv`; compound forms already
    rejected and still tested), wired into BOTH build and check paths.
  - Trust-boundary report `<src>.report.txt` written on strict builds:
    every frgn/asm axiom + per-field memory decision (one wording with
    memcheck via field_decision_line).
  - examples/strict_demo.s.bv: passing strict program (freed field);
    orphaned-malloc fixture rejects with the fix in the message.
  - Suite 1916 → 1921.

Remaining: Phases 5 (dyn), 7 (obj/cell ports), 8 (task lifecycle),
10 (conformance sweep).

### 2026-08-22 session 2 (cont.) — Phase 5 slice A
- **Phase 5 (parse/typecheck)** ✅ — `dyn Trait` objects: contextual parse,
  explicit-only coercion gated on asserted traits (checker reclassifies
  syntactic parents naming traits into assertions per §8.5's one-parent
  rule), static requirement-signature checking for dyn member calls.
  Execution staged explicitly (backend panic / interp unimplemented)
  pending the Phase 5b thunk-table ABI — BUGS.md tracks it. Suite 1916 →
  1924.

Remaining: Phase 5b (thunk tables), 7 (obj/cell ports), 8 (task
lifecycle), 10 (conformance sweep).

### 2026-08-22 session 3
- **Phase 5b** ✅ 1d89f510 — interpreter dyn dispatch: Value::Dyn wraps at
  dyn-annotated lets; impl bodies registered under Concrete::fn; receiver
  threads by arity shape. LLVM thunk tables remain open as **Phase 5c**
  (BUGS.md).
- **Phase 8** ✅ 6f955614 — Task<R> typing, yield; checkpoints, full
  linearity discipline (drop-live / use-after-move / checkpoint-gated
  free), wired into both paths. SPEC §12.2 + §11.3 updated same-commit.
  One codegen interaction open (two-guard spawn + cross-guard arithmetic,
  repro in BUGS.md) — static layer fully verified.
- Owner decisions recorded: i64-handle dyn ABI, `yield;`/`term;`
  checkpoint set replacing the yields-annotation draft, full gate now.

Remaining: Phase 5c (LLVM dyn tables), 7 (obj/cell ports),
Phase-8 codegen gap, 10 (conformance sweep + closure).

### 2026-08-22 session 3 (cont.)
- **Two-guard codegen gap CLOSED** ✅ bd4eeb88 — freshly-labeled condition
  blocks (.cmgcN) replace stale cur_block inheritance; .cm_body resets
  terminated/cur_block; free/keep/yield no longer silently dropped in
  countable bodies. Repro runs (4221); all five pinned fixtures green.
  The readiness assessment's "backend composure" concern just lost its
  only open item — multi-guard compositions now verified end-to-end.

Remaining: Phase 5c (LLVM dyn tables), 7 (obj/cell ports),
10 (conformance sweep + closure).

### 2026-08-23 session (post-merge) — Phase 7 COMPLETE (interp slice)
- **7a finish** ✅ 6ed16a4b — ports bind in member bodies; `.Ready`=Bool +
  payload fallthrough on Event<T>; cell sealing (external internals fail
  naming the ports-only rule); node `Name()` parens tolerated.
- **7b** ✅ 0e1f4460 — Value::EventQ + Value::Instance; spawn constructs
  real instances with SHARED port slots (the wire); instance method
  dispatch with slot write-back; ArrowAssign to a port FIRES; payload
  typecheck for event targets. examples/object_ports.bv drives the spec
  Enemy shape end-to-end (arm(100); hits deliver 90/80; died Ready=80;
  damage wiring intact).
- **7c staged** ✅ bd4eeb88-followup — capability matrix gains
  obj_ports/cells flags; LLVM CAPABILITIES = full() minus those two and
  now JOINS the pipeline gate; rejections name what/why/fix.
- Suite 1932 green throughout.

Remaining: Phase 5c (LLVM dyn tables), 10 (conformance sweep + closure),
cell INTERPRETER scheduling (internal nodes per instance — beyond v1,
noted in sub-plan).

### 2026-08-23 session (cont.) — Phase 7 + Phase 10 infrastructure
- **Phase 7 COMPLETE** (interp slice): 7a finish `6ed16a4b` (port
  bindings, Event fields, sealing), 7b `0e1f4460` (EventQ/Instance,
  shared-slot wiring, firing, dispatch), 7c staged `7a42ec60`
  (capability-matrix flags; LLVM joins the gate). SPEC Enemy shape runs:
  examples/object_ports.bv.
- **Phase 10 infrastructure** ✅ e47662e9 — active_roots manifest-
  resolved; discover+per-kind sweep wired as a test; first light triaged
  (152 files: harness gaps / backlog / foreign WIP) into BUGS.md;
  enforcement flips on once check_source moves into the lib. NOT closed.

Remaining: Phase 5c, Phase 10 enforcement (lib refactor DONE ✅ 3739a6f8;
backlog migration blocked on enum variant CONSTRUCTION — new language arc),
cell interpreter scheduling.

## Session-3 decisions (owner, 2026-08-22)

| Question | Decision |
|---|---|
| Dyn ABI | Owner deferred to my efficiency judgment. Verdict: **i64 handle to a {data, table} image** — true two-word fat pointers are theoretically leaner but this backend's registers are single-valued, so a two-word ABI degenerates to pass-by-alloca anyway; the handle is the efficient choice WITHIN the architecture (union-handle precedent, uniform %State columns). |
| `free task` gate | **REPLACED 2026-08-22 session 3 (owner challenged `yields`):** annotation retired. The proof is now STRUCTURAL over cancellation points — `{ yield;, term; }`. `free <task>` valid ⟺ the spawned callable's body contains ≥1 checkpoint (walked through guards/blocks); else error citing the fix. FFI clause self-satisfying (foreign calls are never interruption points). `yield;` = new no-op statement today, grows into the async suspend point; legal in any defn body, advisory warning when never spawned. Owner chose FULL gate now over staging. SPEC §12.2 rewritten accordingly. |
| Phase 7 strategy | **Interp-first slice** (sums/dyn precedent): parse/typecheck/interp event delivery complete; LLVM + rbv staged behind BUGS entry. |
| Cell depth | **Interp-complete cells** in the same arc: cells parse fully, seal-check, run in the interpreter; LLVM cell instances wait for the ports backend slice. |

### Execution order: 5b → 8 → 7 → 10 (unchanged)

## Phase 8 revised work order (2026-08-22 session 3)

1. **`yield;` surface** — Statement::Yield; contextual parse (mirror trap;
   precedent); Display/canonical/beast arms; interp no-op; backend no-op;
   termination analysis aware (yield does not terminate).
2. **Task typing** — Type::Task(Box<Type>) compiler construct; spawn of a
   registered fn infers Task<Ret>; await Task<T> -> T; await on non-task
   errors; llvm_type(Task) = "i64" (eager result-handle model).
3. **Linearity pass** (`src/analysis/task_linear.rs`) — per txn/defn body:
   Let(init=Spawn-of-fn)/Assign-move inserts live handles; Await/FreeHint/
   KeepHint consume (move); scope end with live handle = dropped-handle
   error; consume-of-dead = use-after-move error. FreeHint on a task
   consults the checkpoint proof of its spawn target; FreeHint on
   non-tasks keeps the existing storage-hint meaning.
4. **Wiring** — pass runs from build AND check paths next to termination.

SPEC updates land with these commits (§12.2 cancellation points + yield +
Task<R>; §11.3 tuple patterns — done up front).

## Deferred (BUGS.md, out of scope by decision)

`$!` DollarBang token; StateDecl/Signature AST residue; `input`/`output` cell-file tokens; orphan fixtures; Ok/Err/Some/None vocab labels; cycle-detection keyed on specifier string rather than canonical path; bare-default CLI route accepting only .bv/.rbv/.abv.

## Work order addendum — 2026-08-22 session 2 (owner-confirmed: 6→9→5→8→7→10)

### Phase 6a — mask-indexing segfault (crash first)

Repro: `bugs/repro_mask_index_segfault.bv` (`Int[8]`/`Bool[8]` state fields,
`data[mask]`, index into compacted result). Machinery exists
(`emit_masked_index`, `briev_mask_select{,64,_f32}`); fault site to bisect:
1. Mask-source read: `MaskSource::StateField` GEPs with prefix "m" — verify a
   `Bool[N]` column's storage width vs the `i64*` the gather expects.
2. Result handling: direct state-field path returns the RAW `[len,e0,…]`
   helper buffer while coll paths wrap as tier blocks — check what
   `picked[i]` expects and make both agree.
3. Reactor/endprogram timing: low suspicion (interp identical shape works).

Method: gdb backtrace + IR read BEFORE patching (LTO lesson rule). Fix at
the found site; Float64 masks get an f64 gather or explicit staged error —
the panic goes either way. Parity fixture `examples/mask_select.bv`
(interp == compiled bytes) joins the suite.

### Phase 6b — ellipsis slices

1. AST: `Slice { array, dims: Vec<SliceDim> }`;
   `SliceDim::{Range{start,end,stride}, Named(name), Full}`.
2. Parser: bracket loop consumes `Token::Ellipsis` → Full; legacy forms →
   Range; `name =>` → Named.
3. Interp first: Full desugars per dimension; compose with named dims;
   depth >2 staged error; single-dim `a[...]` = whole copy.
4. LLVM second: per-dim lowering on existing stride machinery.
5. Tests: `t[1:3, ...]` ≡ `t[1:3, :]`; composed named+ellipsis; depth gate.

### Then Phase 9 unchanged (master plan §9)

Acceptance per commit: cargo test --lib green, Praetor touched dirs, docs
same commit; BUGS.md mask entry closed when the segfault dies.
