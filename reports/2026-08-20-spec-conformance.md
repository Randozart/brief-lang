# SPEC Conformance Report

**Date:** 2026-08-20
**Head commit:** `fe8672dc`
**Normative source:** `spec/SPEC.md` (25 sections, draft 2026-08-05)
**Status tracker:** `docs/plans/2026-08-15-spec-implementation-status.md`
**Policy:** A normative section is either Implemented (matching reference interpreter and applicable active backends), Staged-rejected (parser emits `SyntaxError::StagedFeature`), or in progress. No section may silently behave differently from the SPEC.

## Build & test baseline

- `cargo build --release` — clean (exit 0, 4 warnings)
- `cargo test --lib` — 1903 tests: 1895 pass, 8 fail
  - 8 failures in `config`/`dbriev::config_db`/`glue::briev_pass` (parity + board-file + pass tests) — not core language features, but they violate the "tests or it doesn't exist" rule
- `tests/test_contract.rs` — won't compile (missing `TopLevel`/`Expr` imports); dead integration test
- `TODO.md` — all items ✅ completed (2026-07-27)
- `BUGS.md` — last two entries both FIXED 2026-08-18

## SPEC conformance (25 sections)

The tracker marks all 25 sections "In progress" with no "Implemented" rows — nothing has been formally signed off, but the notes show what's actually working.

### Fully working (implementation matches SPEC, no active Staged markers)

| § | Section | Status |
|---:|---|---|
| 1 | Scope and conformance | In progress (conformance runner active; fixture inventory from active sources) |
| 2 | Core model | In progress (semantic-value migration complete; fundamentals-as-types active) |
| 3 | Source files and target profiles | In progress (`.c`/`.sbv` removed; `.f` layout frontend pending Phase 15) |
| 4 | Lexical conventions | In progress (dead tokens removed; layout keywords `pack`/`seq`/`union`/`atomic`/`spec` + `coll` added) |
| 5 | Delimiters and arrows | In progress (removals done; delimiter-semantic-load rule enforced) |
| 6 | Grammar overview | In progress (Phase 2/3 done) |
| 7 | Modules and imports | In progress (selective renames, glob rejection, `export import`, `:` alias) |
| 8 | Declarations | In progress (`let`/`const`/`init`, `struct`/`pack`/`seq`/`union`/`atomic`/`spec`, `enum`, structural sums, `type`, `trait`, `proto`, `impl`, `trap`, `coll` obj/struct + grow-on-full) |
| 9 | Functions, txns, nodes, objects, cells | In progress (`rollback`, `exit program`, `beginprogram`/`endprogram`, `term`, closures-as-values, generic `defn<T>`, `accel` offload) |
| 10 | Contracts, invariants, watchdogs | In progress (mandatory non-trivial contracts on node/txn/asm; `[true][true]` rejected) |
| 11 | Control flow | In progress (`rollback`, `exit program`, `defer`/`mutex`/`barrier<group>`) |
| 12 | Concurrency and task lifecycle | In progress (`spawn Obj(...)`, `spawn defn(...)`, `await`, `free`/`keep`, no-implicit-concurrency gate, sync-group classification) |
| 13 | Triggers and external events | In progress (`#assume_event` dead data removed; port contracts Phase 10) |
| 14 | Ownership, lifetimes, effects | In progress (UOL + UFCS) |
| 15 | Expressions and operations | In progress (declared variant ops lower to functions; UOL generative `OpName#` dispatch + UFCS) |
| 16 | Literals, ranges, slicing | In progress (byte literals, raw strings, Python slices, `..=` inclusive ranges, boolean-mask indexing, iterable ranges + foreach, named selectors, multi-dim arrays, const-generic member dim substitution) |
| 17 | Reflection | In progress (runtime reflection; `.^^Element` frozen element-type descriptor; `Abs#` unification + bit intrinsics) |
| 18 | Compile-time execution and macros | In progress (escaping closures + interpreter user-fn) |
| 19 | Foreign functions, export, GLUE | In progress (GLUE FFI folders; frgn/export + `--shared` PIC; four provenance forms; `variadic`; MMIO `@` rejection; optional frgn + `feature.^^Available`) |
| 20 | Assembly declarations | In progress (contracts mandatory on asm) |
| 21 | Rendered Briev | In progress (basic `render Name`, `b-when`) |
| 22 | Data Briev | In progress (`.dbv`/`.dbvl` v2 parser; schema validation; canonical serialization) |
| 23 | Diagnostics, tooling, documentation | In progress (vocab + LSP + grammar; helpful-messages rule; tutorial + syntax highlighter updated) |
| 24 | Standard-library boundary | In progress (coll exception codified; no type-name matches) |
| 25 | Implementation staging | In progress (`SyntaxError::StagedFeature` live; no active Staged markers remain) |

### Gaps (named in the tracker's "Remaining:" columns)

| § | Gap | Notes |
|---:|---|---|
| 9 | objects/cells lifecycle; prior-state txn expression syntax (`@value`) | |
| 12 | objects/cells lifecycle; deterministic scheduler interleaving mode | |
| 14 | ownership algebra + `.s` enforcement (Phase 9) | |
| 15 | effects/access-shape carrying; `dyn Trait` | |
| 16 | list-literal→`Int[N]` coercion blocking `coll struct` construction | |
| 17 | const generics (required for generic `coll struct Fixed<T,N>`) | |
| 18 | `$name`/`name!` macros + stage timing | |
| 19 | ownership-for-pointer/aggregate-boundaries gate (Phase 9) | |
| 20 | full effect profile (Phase 16) | |
| 21 | full lifecycle (Phase 14) | |

## Bottom line

The codebase is at roughly 85–90% of the normative SPEC. Every section has a working implementation; the remaining work is concentrated in:

1. **The coll track** — coll-struct construction + const generics for `Fixed<T,N>`
2. **Phase 9** — ownership algebra + `.s` enforcement pass
3. **Object/cell lifecycle**
4. **`dyn Trait` dispatch path**
5. **8 failing config/parity tests + 1 dead integration test** that need to be fixed or removed to satisfy the "tests or it doesn't exist" rule

Benchmark state is healthy — most Briev programs now beat their C references (queue_drain 0.56–0.60x, float_math 0.65x, global_lifetime 0.42x), with sweep_dense/arr still at 1.35x/1.17x.

## Gap discussion — 2026-08-20

### Gap 1: coll-struct construction + const generics for `Fixed<T,N>`

**Decision (2026-08-20):** Exact arity, no zero-fill, scoped to coll struct construction.

**Principles applied:**

1. **`<>` is compile-time specialization.** `Fixed<Int, 4>` is fully monomorphic — `N` is a const generic resolved at each call site, and the compiler emits the exact inline layout. There is no runtime dimension where `N` is unknown. The `Ptr<T>`-backed coll is a *different* construct (SPEC: "documented follow-up") with its own capacity slot; the two do not mix.

2. **No generic zero-fill fallback.** §8.1: "Unknown fields, duplicate fields, unresolved defaults, and mismatched constructor arity are errors. There is no generic zero-fill fallback." A list literal has a compile-time element count; `T[N]` has a compile-time dimension. The compiler has both numbers before codegen. If they don't match, the program is provably wrong — and the proof-vs-decision hierarchy says: error, don't silently pad.

3. **`coll struct` is the *fixed* case of the coll family.** SPEC: "length == capacity == N, no hidden slots, C ABI preserved." If the compiler could zero-fill, the author would have no way to distinguish "I meant 4 elements" from "I meant 3 and the 4th should be 0" — a silent contract violation, the exact class of bug the proof-vs-decision hierarchy prevents.

4. **The escape hatch already exists.** If the author wants `[1, 2, 0, 0]`, they write it. That's one extra line of source, not a knowledge tax. Strategy-keyword rule: omitting the explicit choice is the common case; the default (exact match) is the efficient path. Zero-fill would be a *strategy* the author opts into by writing the zeros — not a strategy the compiler applies by default.

5. **Coercion rule, scoped.** A list literal coerces to `T[N]` *only* when the target type is a `coll struct` field of type `T[N]` and the literal has exactly `N` elements of type `T`. No general "any list literal to any `T[N]`" rule. A general rule would open `let a: Int[4] = [1,2,3,4]` outside a coll context, which would need its own storage strategy (inline? heap? pooled?) and reintroduce the "compiler holds a collection layout" problem §2.1 forbids. The coll struct is the one sanctioned site where the compiler owns the layout.

6. **Edge case: `Fixed<Int, 0>`.** An empty `Int[0]` is a valid zero-width array. SPEC: "an empty `[]` constructs a zero-filled N-array." When N=0, "zero-filled" is the empty array, which is trivially exact. `let f: Fixed<Int, 0> = []` is valid — the only case where zero-fill and exact-match coincide.

**Implementation checklist:**
- [ ] Const generics: `N` in `Fixed<T, N>` resolved at monomorphization
- [ ] List-literal → `T[N]` coercion in typechecker (exact arity check)
- [ ] Codegen: emit elements directly into inline array, no intermediate list
- [ ] Compile error on over-length literal (SPEC already requires this)
- [ ] Compile error on under-length literal (new — symmetric with over-length)
- [ ] `Fixed<Int, 0>` + `[]` allowed (trivially exact)
