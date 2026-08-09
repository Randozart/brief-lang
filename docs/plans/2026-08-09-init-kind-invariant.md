# `init` kind — runtime-seeded invariant + memory strategy management

**Date:** 2026-08-09 · **Status:** Planned · **Ties to:** SPEC §8.1, §16.6;
`docs/plans/2026-08-08-pool-lifecycle-free-keep-await.md`;
`docs/plans/2026-08-06-endprogram-beginprogram.md`;
`src/macros/memcheck.rs`; AGENTS rule 21 (classify-don't-guess precedent)

## The gap

Briv has `let` (reactive state), `const` (compile-time folded), `$const`
(erased), `beginprogram` (entry-loop marker, implemented). No construct says
**"set once at runtime start, then invariant for the whole run."** Today
`let N = get_env_int!(...)` stays `Bound::Unknown` (loop_shape.rs:33) → cannot
fold; pool capacity stays monotonic-lifetime total; the scheduler can't assume
a field is write-free.

This plan adds a fourth top-level kind, `init`, plus the bounded-set syntax and
memory-strategy classification surface that make it proof-bearing.

## Principles

Briv's operating philosophy is **compile through proof, leave decisions to the
programmer where no single option is best**:

1. **Prove.** The compiler proves what it can — bounds, capacity, scheduling,
   layout, lifetime. Proof is the default path.
2. **Guardrail, with request for disambiguation.** When the compiler cannot
   pick a single best-fitting, fastest, most-efficient strategy, it does NOT
   guess silently. It emits a **warning** and *asks the programmer to explicitly
   name a strategy* (`init`, bounded sets, `box`/`spill`-style markers).
3. **Error if provably reckless and/or underdetermined or overdetermined.**
   When the choice is ambiguous beyond the compiler's model, or a chosen
   strategy provably *conflicts* (use-after-free, capacity < proven need,
   underdetermined bounds), the compiler **errors** — it never ships a program
   that is provably wrong.

This mirrors the existing scheduler contract (AGENTS rule 21: two reactive
nodes that may both fire and overlap are a *compile error* demanding
`async`/`sync<group>` classification) and extends it from concurrency to
memory/lifetime/layout.

## Three-tier alert ladder for memory decisions

| Tier | Condition | Compiler behavior |
|---|---|---|
| Silent | a provably-most-efficient strategy exists | no warning |
| Warn | genuinely ambiguous; must fall back to a default garbage-scheduling path | **WARN** naming *which* strategy fell back + why |
| Error | no provable model, dangerously underdetermined, OR chosen strategy provably conflicts (e.g. capacity < proven need) | **ERROR** |

The `memcheck` subcommand (`src/macros/memcheck.rs`) is **kept and expanded**
as the introspection surface: for every memory decision point it reports the
strategy chosen, where, and the proof obligation that justified it. It does not
change what the compiler does; it makes silent decisions auditable.
(`memcheck` currently reports per-field lifetime decisions — freed after which
txn vs. lives-for-program — plus redundant `keep` hints. That is the seed.)

## The `init` kind

A fourth top-level declaration alongside `let` and `const`:

```briv
// expr form — value seeded once at runtime start.
init BufSize: Int = get_env_int!("BUFSIZE");

// kind-attached bounded set (see below).
init Parallel: [2 | 4 | 8] Int = get_env_int!("PAR");

// body form — seeding runs once before beginprogram / any node fires.
init Layout: [16 | 32 | 64] Int { ...; }
```

Semantics (locked decisions):

- **Set once and only once**, before `beginprogram` or any other node fires.
  The seeding is a dedicated pre-reactor phase at top of `main` /
  `@init_state`.
- **Invariant for the whole run.** Reassignment of an `init` name anywhere
  after seeding is a **compile error** (write-set ∩ init-names = ∅, enforced
  in the typechecker + transition graph). It is a `const`-like invariant but
  **not** compile-time-folded: the value is loaded from the environment/target
  at runtime, then proven immutable for proof purposes.
- `init` is a **kind**, not a suffix on `const`/`let`. This keeps it distinct
  from reactive state (`let`) and from macro/plugin-tied `get_env!` use.
- Distinct from `const` (compile-time folded), `$const` (erased
  compile-time-only), and reactive `let` (mutable per tick).

## Grammar

`init` joins the `item` production alongside `const_decl`/`let_decl`:

```ebnf
init_decl ::= "init" identifier ":" bound_set? type ("=" expr | block)
```

Implementation sites:

- Lexer/vocab: `init` keyword (vocab.rs, `KeywordContext::Declaration`).
- Parser arm at definitions.rs:196 beside `Some(Token::Let)`/`Some(Token::Const)`.
- AST: `TopLevel::Init { name, ty, bound: Option<BoundSpec>, body: Vec<Statement>, value: Option<Expr> }`.
- Walkers: `ast/display.rs`, `beast` (serialize/deserialize), `canonical.rs`,
  `annotator.rs`, `symbolic.rs`, `compile.rs` comptime resolution, plugin
  env/entry/script walkers, `ast/mod.rs` statement collectors.

## Bounded sets — kind-attached bound declarations

A bound is declared **between `:` and the type** (kind-attached), so it is not
misread as an array dimension (`Int[16]` is fixed containment; `init N: [set]
Int` is a *set of possible values*):

```briv
init BufferSize: [64 | lo..hi] Int = ...;   // either exactly 64, or in [lo,hi]
init BitLayout:  [16 | 32 | 64] Int = ...;  // one of three; target picks
```

Extends the existing `BitRange` family (types.rs:119
`Single | Range | Any`) and the loop-shape `Bound` family with a
discrete-choice union:

```rust
enum BoundSpec {
    N(i64),
    Range(i64, i64),
    Choice(Vec<BoundSpec>), // [64 | 32]  discrete union
}
```

Semantics:

- **The value is one of the listed options** — bounds of all expected values
  are declared up front, so proof carries a domain (max of the set), not an
  unbounded integer.
- Pool capacity consumes the **max of the set** as the proven upper bound
  (provably inexhaustible), instead of the dependent-heap total-spawn path.
- `[16 | 32 | 64]` is the byte-layout class the user reaches for when the
  precise bit-layout isn't known for the target but *is* known to be one of a
  few: the target config resolves one; fallback picks `min` of the set.
- At a generic site, `<>`: a bounded set is a finite proof domain (e.g.
  `Stack<T, [2|4|8]>` adapts size per-target without ∞ instantiations).

## Proof axis

- Add `Bound::Init(String)` alongside `Field`/`Const`/`Literal`/`Unknown`
  (loop_shape.rs:26). The fold gate (`mod.rs:4013`) accepts it:
  `emit_countable_load_bound` loads `add i64 0, N` once, at the seeded value.
- Because an init is provably invariant, folding is sound for seeded bounds —
  this kills the "literal-bound loop ran once" class rooted in `Bound::Unknown`,
  and removes the `Bound::Unknown` → `add i64 0, 1` fallback path.
- With a `BoundSet`, load the max only; no Unknown fallback exists.
- Pool capacity (spawn_pool.rs) reads the init bound max; dependent-buffer path
  is unchanged for unbounded inits.
- Layout `[16|32|64]`: pattern the existing `Type::Constrained` +
  `estimate_type_size`/`memory_spec` handling; target resolves, fallback picks
  minimum.

## The `init` proof hierarchy

| init proof quality | compiler will |
|---|---|
| bounded + provable | silent; fold to `Bound::Init`, size pools to set max |
| weak / bounded not part of a proof, but proof-required | **WARN** "will require a runtime check instead — less efficient" |
| dangerously underdetermined / provably conflicting | **ERROR** |
| unbounded but the rest of the contract resolves it | allowed; runtime-check path |

"Unbounded but the surrounding contract proves satisfactory resolution" is
valid: it's *difficult* to base contracts on, not impossible. Only a provably
reckless claimant (underdefined, or overdefined against the actual use, like
declaring too-small a bound on an `init` a pool actually exceeds) is an error.

## Memory strategy management keywords

Extending the existing vocabulary (`seq`, `vol`, `async`, `sync<g>` are
ordinary keywords that **must never make code faster** — the default is always
efficient; a modifier-beaten default is a compiler bug), the memory-strategy
surface is:

- **`init`** — bounded-set-capable runtime invariant (session foundation).
- **`box`** — legalize the per-instance-heap strategy as an *explicit class*
  ("this value is heap-per-instance, not pooled") when the pool decoder is
  ambiguous — the retired boxed path normalized from a hidden special case
  into an explicit, visible choice.
- **`spill`** — mark a value allowed to grow into a growable buffer when a
  static pool column can't hold the proven worst case; a **capacity-tier**
  declaration for pay-as-you-row, not a speed knob.
- **`budget`-style capacity tiers** (delay: specify exact grammar in phase)
  — bind a runtime `init` value/target to value-set/`BoundSet` analysis so a
  boundated trigger drives enumeration.

These keywords share one rule: they exist **only where the compiler cannot
decide a single best strategy**, and they *reveal* a choice the backend would
otherwise hide. They never beat a working default; a keyword-beaten default is
a bug.

## Strategy keywords — the universal compiler surface

**All compiler strategy is expressed in keywords.** Collectively these are
**strategy keywords** — the Briv analog of pragmas in other languages, but
**transparent**: they are ordinary words in the source program, not hidden
directives, and they carry less "knowledge tax" than pragmas because you never
need one to write a correct program. The compiler **reminds you when you need
one** (AGENTS §21: two reactive nodes that may both fire → the compiler demands
`async` / `sync<group>` classification; a genuinely ambiguous capacity decision
→ a warning naming the fallback and asking for a strategy keyword).

Rules:

1. **Keyword-shaped, not directive-shaped.** If a strategy is worth naming, it
   is named by a keyword in the grammar — never by an invisible flag.
2. **Transparent.** `init`, `storage`, `borrow`, `consume`, `spill`,
   `async`, `sync<g>`, `seq`, `vol`, `free`, `keep` — ordinary words. No `#`,
   no compiler-only escape. The `#`/`!` marks are reserved for hashwords and
   compile-time expansion (*different* category: a known, disclosed one).
3. **Zero knowledge tax.** Derived strategy is proven, not annotated. A
   strategy keyword names a choice the compiler genuinely cannot decide alone.
   Omitting it is the common case.
4. **Never make code faster.** The default is the efficient path; a
   keyword-beaten default is a compiler bug.
5. **One syntax shape: `category<mechanism>`.** The *category* keyword is
   program-independent (`borrow`, `storage`, `delivery`); the *mechanism* is a
   value that rides inside `<>` (`borrowed<source>`, `sync<group>`,
   `storage<allocate>`). Precedents already in the grammar: `borrowed<source>`,
   `sync<group>`, `#Link<name>`, `#String<UTF8>`, `asm<chip>`.
6. **Mechanisms resolve by config, categories are keyword.** The value inside
   `<>` either names a compiler-known intrinsic class or a row in a config
   registry (`config/alloc-strategies.dbvl` + siblings per category). This is
   the "stdlib is the extension mechanism" rule applied to *config*: the
   compiler teaches; config and `.bv` files learn.

### The strategy axis table

| Axis | Category keywords | Mechanism rides in `<>` | Registry |
|---|---|---|---|
| Storage/layout | `storage`, `box`, `spill` | `allocate`-class (heap, pool, arena, alloca) | `config/alloc-strategies.dbvl` |
| Ownership (§14.1) | `borrow`, `consume`, `owned`, `shared` | `borrowed<source>` bound-to-input | ownership policy table |
| Concurrency | `async`, `sync` | `sync<group>` | scheduler config |
| Lifetime | `free`, `keep` | — | schedule policy |
| FFI delivery (deferred) | delivery markers (`frgn!`, `frgn?!`) | `fire_forget`, `ack` | delivery registry |
| FFI source (deferred) | `from`/`#Link<…>` | `literal`, `registry`, `system`, `link` | source registry |
| FFI fallback (deferred) | `fallback` | `static`, `call`, `implicit`, `none` | fallback registry |

### Ownership generalization (§14.1)

`borrow`/`consume`/`owned`/`shared` are **category** words: their core meaning
is intrinsic, so they must work even with `--no-stdlib` and no config — the
same bootstrap rule as primitive types. They stay ordinary keywords; rewriting
them as `ownership<borrow>` would garble boundary declarations and buy nothing.
The **generalization is at the policy level**: what each category permits at a boundary (retain-after-call,
free-side, exclusivity obligation) is config-resolved, the same way `pool_serial`
is a config row. `borrowed<lhs>` is already the mechanism form (`<>` carries the
lifetime source). The line from SPEC §14.1 — *"Allocation and layout, not
hardcoded into the ownership keyword"* — becomes literal: allocation policy for
owned results and consumed inputs rides the mechanism registry.

### FFI strategy generalization (deferred grammar)

The `frgn` surface packs four axes that do not match the strategy-keyword shape
today: `frgn?` (optional), `frgn!`/`frgn?!` (`is_fire_forget`/`is_delivery`),
`from <…>` (`FromSpec`), and `Fallback` (=`Static` / `FnCall` / `Implicit` /
`None`). These should eventually walk the same grammar + registry:

- delivery: `delivery<block | fire_forget | ack>` (absorbs `frgn!`/`frgn?!`);
- source: `from ` — literal path / registry / `#System` / `#Link<name>` (the
  `<>` mechanism carrier already exists);
- fallback: `fallback<static | call | implicit | none>`, used for the
  "ambiguous → safe default" tier naming from the three-tier alert ladder.

The *concept and grammar* are settled here and in SPEC; the parser/lexer work is
deferred to a dedicated phase directly after `init`/`storage`/`box`/`spill`
land, so the mechanism registry exists first.

## memcheck expansion (keeps name)

Current `brivc memcheck <file.bv>` (src/macros/memcheck.rs) reports, per
heap-backed state field: "freed after txn X / keep p. redundant" or "lives for
the program (unprovable)." Expanding:

- Report every memory decision point: lifetime, capacity, pool/storage class,
  dependent-vs-static columns, `init` bound resolution.
- For each decision: the **strategy chosen**, **where** (file:line or
  construct), and the **proof obligation/why**.
- Surface the three-tier alert membership (silent / warn / ask) so auditing a
  compiler that chose silently is possible.

## Out of scope / not in this plan

- An `init` generic-parameter form past stage 1 (initialized numbered variant
  at `< >[]` sites comes with the bounded-set consumption; ∞-instantiation
  guards live with Type::Number handling, deferred in the implementation pass).
- Per-`<target>` `init` variants as separate AST (the `[16|32|64]` set is
  the current model: one, target, one pick).
- `budget`/capacity-tier keyword grammar is noted but deferred until after
  `box`/`spill` shape.

## Verification

- Interpreter first (reference implementation), then LLVM codegen. Every
  feature wired parser → AST → analysis → codegen → tests.
- New tests: set-once, once-only-before-beginprogram ordering, reassignment
  error, `Bound::Init` fold, pool capacity from `BoundSet` max,
  `init`+beginprogram, unbounded-but-contract-ok warning, weak/need-runtime
  warn, underdetermined error, memcheck new tier lines.
- `cargo test --lib` green + `cargo build`, Praetor changed files, no `todo!`.
- Spec + tutorial + syntax-highlighter updated in the SAME commit as grammar
  changes.

## Phasing

1. Grammar/AST + lexer/parser — the `init` construct parses and displays.
   **DONE 2026-08-09** (commit `12b9212a`): contextual keyword, AST
   `TopLevel::Init(InitDecl)` + `BoundSpec`/`BoundTerm`, walkers, BEAST
   round-trip, LSP/import/typechecker registrations.
2. Semantics: set-once + before-begin ordering (interpreter reference) +
   reassign error; memcheck sealing.
   **DONE 2026-08-09**: typechecker rejects reassign/arrow/shadow/consume of an
   init, duplicate decls, and init-after-beginprogram; interpreter seeds inits
   (value + body `term <expr>` form) as the reference; backend emits init as a
   mutable global, seeds it in `emit_init_state` / `emit_inline_init_stores` /
   `__briv_init_state`, and reads load it; memcheck reports init-bound pools as
   sealed. Tests: 1686 pass.
3. Proof-link: `Bound::Init` folding.
4. Capacity: pool read via set-max.
5. `box`/`spill` classification around the pool.
6. Mechanism registry generalization: `resolve_mechanism(category, name)`
   shared by storage + ownership policy; FFI delivery/source/fallback axes
   flattened onto the same grammar+registry (deferred grammar, settled here).

## Reference map

| Change | File |
|---|---|
| keyword + doc | `src/lexer.rs`, `src/vocab.rs` |
| parser arm | `src/parser/definitions.rs` |
| AST + BoundSet | `src/ast/top.rs`, `src/ast/types.rs`, `src/ast/expr.rs` |
| walkers/display | `src/ast/display.rs`, `src/ast/canonical.rs`, `src/annotator.rs`, `src/symbolic.rs` |
| typechecker set-once + ordering | `src/typechecker/mod.rs` |
| interpreter reference | `src/interpreter/mod.rs`, `src/interpreter/eval.rs` |
| `Bound::Bound` + fold | `src/analysis/loop_shape.rs`, `src/backend/llvm/mod.rs` (`try_to_fold`), `loop_engine/counter.rs` |
| capacity from set | `src/analysis/spawn_pool.rs`, `src/backend/llvm/emit_toplevel.rs` |
| memcheck expansion | `src/macros/memcheck.rs` |
| mechanism registry | `config/alloc-strategies.dbvl` + per-axis siblings (inherited by phase 6) |
| docs / SPEC / highlighter | `spec/SPEC.md`, `learn-briv/`, syntax highlighter |

## Open decision (deferred)

- `init` body sealing — assumed seeded once per program run, never re-entered
  per scope. Revisit only if a later use case (tests, hot reload) demands
  re-entry semantics.
- Ownership policy table location (own file vs merged into alloc registry)
  deferred until phase 6; the mechanism-registry shape is settled here.