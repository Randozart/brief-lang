# Plan: Stdlib-Cleanup — free-`T` generic bodies, `Option` constructors, and collection-generic construction

**Date:** 2026-08-14 (handoff). **Head commit:** `3595488c`.
**Supersedes/continues:** `docs/plans/2026-08-14-generic-defn-dispatch.md`
(the generic `defn f<T>` dispatch core, DONE). This plan fixes the three
documented blockers that keep the stdlib's generic functions from compiling
(BUGS.md entry "stdlib iterator.bv / hashmap.bv written aspirationally").

A new agent can execute this with this document alone plus the referenced
files. Read `docs/plans/2026-08-14-generic-defn-dispatch.md` first for the
generic-dispatch architecture.

---

## 1. Background: what already works (the generic-dispatch core)

Generic `defn f<T>` dispatch is implemented and verified:

- **Call-site type-param inference**: `infer_defn_type_args`
  (`src/typechecker/mod.rs:3705`) unifies each parameter type against the
  argument type, binding the defn's type params (`List<T>` vs `List<Int>` →
  `T = Int`). `unify_defn_type` (`:3768`) handles `Custom` params, `Applied`,
  `Ptr`, `Vector`, `Tuple`, `Union`, and `Type::Function` (closure-typed
  params). `substitute_type_params` (`:3819`) substitutes into param
  validation and the return type.
- **Generic txns parse** (`txn iter_map_loop<T, U>` — `parse_transaction` reads
  type params, `src/parser/definitions.rs`).
- **Function-typed params parse** (`f: T -> U`, `f: (U, T) -> U` — a param
  LIST, not a tuple).
- **Multi-param lambdas** parse (`(a, b) -> body`).
- **Nullary generics** (`new_map<K,V>()`) bind their params from the enclosing
  `let` annotation via `ctx.expected_call_type` (set at `mod.rs:1893`).
- **`term` is the canonical result placeholder** in post-conditions, bound to
  the declared output type during elaboration (`mod.rs:1514-1535`).
- **Empty list literals in generic bodies** adopt the declared return's element
  type param (`Expr::List`, `mod.rs:738`).

## 2. The three blockers (verified failures)

### 2.1 Free-`T` bodies — generic bodies that manipulate or return `T`

A generic defn body is elaborated ONCE with its type params free
(`check_top_level`, `mod.rs:3204`; params bound via `ctx.bindings` at
`:3226`). `Type::Custom("T")` is not substituted, so:

- `term f(x)` where `f: T -> U` returns the free `U`, and the `term`-vs-return
  check (`mod.rs:2130` region) errors `expected U, found Int` (the Lambda
  typecheck at `mod.rs:769` binds each lambda param to `Type::int()`, so
  `f(x)` is `Int`).
- `res <- f(x)` / `foreach` over a `List<U>` target with `U` free fails
  (`expected List<U> for arrow assignment, found Int`).
- Affected stdlib functions: `iter_fold`, `iter_zip`, `iter_enumerate`,
  `iter_find`, `iter_max` in `lib/std/iterator.bv` (all verified failing
  end-to-end).

`iter_map`/`iter_filter` compile (their bodies return the accumulator, not a
free-`U` closure call).

### 2.2 `Option` / `Some` / `None` constructors

`lib/std/option.bv` declares `enum Option<T> { Some(T), None }`. `iterator.bv`
uses `term Some(list[i])` and `term None` (e.g. `iter_find` at
`lib/std/iterator.bv:96,99`). These fail: `expected Option<T> for term value,
found Int`. The `Some(...)`/`None` constructor expressions are not typechecked
to `Option<T>`.

### 2.3 Nullary collection-generic construction

`lib/std/hashmap.bv:1-3`:
```briev
defn new_map<K,V>() -> HashMap<K,V> [true][term.Count#() == 0] {
    term {};
};
```
The `term {}` (empty struct literal) HashMap construction path is unverified —
`{}` doesn't parse as a struct literal in expression position (the empty
`Option`/struct construction). And `new_map`'s contract calls `Count#` on a
`HashMap<K,V>` (a Tier-1 type with no `op Count`) — a stdlib-design issue.

## 3. Design decision: typecheck generic bodies with `T` treated as a bound generic

**The core question**: how does a generic body typecheck `term f(x)` (returning
`U`) or `res <- f(x)` (a `List<U>` target) when `U` is free?

**Decision: typecheck the body with its type params treated as a "bound generic
type" — an `Applied`-style `Generic("T")` that:**
1. Is **equal to itself** for the `term`-vs-return check (the declared return
   `U` IS `Generic("U")`, and `f(x)` typed against the param `f: T -> U`
   yields `Generic("U")` — so `term f(x)` matches the declared `Generic("U")`).
2. Resolves **structurally** for ops (`Count#(xs)` on `List<Generic("T")>`
   dispatches to `op Count`, which returns `Int`).
3. Is **substituted away at the call site** — the body is only *checked* with
   the generic; codegen already emits it once with `T`-values as i64 boxes
   (the erased model, §1 of the generic-defn plan).

**Concretely**, introduce a `Type::Generic(String)` variant (distinct from
`Type::Custom`) used ONLY inside generic defn bodies for the declared type
params. Then:
- `term`-vs-return: `Generic("U") == Generic("U")` → passes.
- `Expr::Lambda` in a generic body: if the expected function type (from a
  generic param binding) has `Generic` params, bind the lambda params to those
  (`f: T -> U` → `x` binds `Generic("T")`, body `x + 1`... wait, `x + 1` on
  `Generic("T")` is a problem — `+` needs Int/Float).

**Sub-problem**: a generic body CANNOT know `T` supports `+`. That is exactly
the protocol-constrained-generics feature (`defn f<T: #Int>`), OUT OF SCOPE
here. So the free-`T` body must be constrained to bodies that use `T`
*structurally* (pass it to generic functions, return it, put it in
collections) but NOT apply type-specific operators. `iter_map` already does
this (`f(x)`, `result <- x` — structural). `iter_fold`'s `(acc, x) -> acc + x`
is the problem: it adds two `Generic("U")`/`Generic("T")`.

**Therefore the pragmatic scope for THIS plan:**
1. Make `Generic("T")` a first-class body type so `term f(x)` (structural
   return) and `result <- x` (structural collection insert) typecheck.
2. **Fix iterator.bv's operator-in-generic-body usages**: `iter_fold`'s
   `acc + x` is genuinely unrepresentable without `<T: #Int>` — MIGRATE
   `iter_fold` to a non-generic form or a `<T: #Int>`-style constraint later.
   For THIS plan, make `iter_fold` take an explicit operator parameter or move
   it out of the generic adapters until protocol-constrained generics land.
3. Fix `Option`/`Some`/`None` constructors (§4.2).
4. Fix/verify `new_map` construction (§4.3).

## 4. Work items (implementation order)

### 4.1 `Type::Generic` + free-T body typechecking

1. Add `Type::Generic(String)` to `src/ast/types.rs` (a NEW variant, distinct
   from `Custom`). Derive `PartialEq`/`Eq`/`Debug`/`Clone`/`Display`
   (`Display`: render as the bare name, e.g. `T`).
2. In `parse_type`, a type-param reference inside a generic defn body must
   produce `Generic("T")`, not `Custom("T")`. The cleanest: after parsing a
   defn/txn with type params, REPLACE `Custom(p)` for each declared param with
   `Generic(p)` in the params/return/contracts. Add a `substitute_to_generic`
   pass in `build_check_env` (or a helper `genericize(defn)`), running over the
   `Definition`/`Transaction` signature + body expressions.
3. Update `unify_defn_type` (`mod.rs:3768`): a `Generic(p)` param unifies with
   any concrete arg (bind `p`), like the existing `Custom`-param branch — add
   `Generic` to the bare-param check.
4. Update `substitute_type_params` (`mod.rs:3819`): `Generic(p)` in a signature
   substitutes like `Custom(p)`; the body's `Generic` refs are NOT substituted
   at declaration (they're the erased model), only at call-site return
   inference.
5. Update `term`-vs-return: the check at `mod.rs:2130` uses `vty != *out` —
   with both `Generic("U")`, equality holds. Verify no OTHER place compares
   `Type::Custom("U")` vs `Generic("U")` and breaks (grep `Custom` in
   typechecker; audit `resolve_element_type`, `operator_member`, field access).
6. `Expr::Lambda` (`mod.rs:769`): bind lambda params from the EXPECTED
   function type when the lambda is a generic-param argument. The expected
   type is the generic param's `Generic`-typed `Type::Function`. For
   `iter_map`'s `f: Generic("T") -> Generic("U")`, bind `x` to
   `Generic("T")`; `f(x)` then types as `Generic("U")`.
7. `foreach`/`<-` over a `List<Generic("U")>` target: the op-surface dispatch
   (`op At`/`op InsertAt`) already resolves structurally on `List<T>` (verified
   in §1); ensure `Generic` passes the member/field lookups (`operator_member`,
   `obj_type_params` substitution) — audit `Type::Custom` matches for the
   `Generic` case.

### 4.2 `Option` / `Some` / `None` constructors

1. `enum Option<T>` is `TopLevel::TypeDef` with `Some(T)`/`None` variants. The
   `Some(value)` and `None` EXPRESSIONS need a typecheck case:
   - `Expr::Some(value)` → `Option<T>` where `T` is `value`'s type.
   - `Expr::None` → `Option<T>` (T free; binds from context).
   Check whether the parser produces distinct `Expr` variants for these (it may
   desugar `Some(x)` to a `Call` — verify; add the typecheck case where the
   call resolves).
2. `term Some(list[i])` in `iter_find_loop` must type `Option<Generic("T")>`
   matching the declared `Option<T>` return.
3. Verify `option.bv`'s `uni opt(Some(v)) = ...` match arms still typecheck.

### 4.3 Nullary collection-generic construction

1. `term {}` (empty struct literal) — investigate how struct literals parse.
   `HashMap` has slots (`keys`, `vals`, `occupied`, `count`, `cap`); an empty
   `{}` should construct via `op InitEmpty`-style or default field init.
   Decide: either fix `{}` empty-struct construction, or change `new_map` to
   `term [] :> AsHashMap`-style (verify the `:>` cast path).
2. `new_map`'s contract `term.Count#() == 0` calls `Count#` on a Tier-1
   `HashMap<K,V>` (no `op Count`) — the correct count is the `count` slot or a
   `size()` defn. Fix the contract.
3. Verify `let m: HashMap<String, Int> = new_map()` binds `K`/`V` from the
   annotation (the expected-type path, commit `3595488c`).

### 4.4 iterator.bv / array.bv fixes

After §4.1-4.3:
1. `iter_fold` — the `(acc, x) -> acc + x` lambda needs `Generic("T")` to
   support `+`. **Decision: migrate `iter_fold`/`iter_sum`/`iter_product` to a
   form that doesn't apply operators to a free `T`** — either remove them until
   `<T: #Int>` lands, or make them Int-specific (non-generic) wrappers. The
   other adapters (`iter_map`, `iter_filter`, `iter_take`, `iter_skip`,
   `iter_enumerate`, `iter_zip`) should compile once §4.1-4.2 land.
2. `iter_zip`'s `result <- (a[i], b[i])` tuple insert — verify tuple-typed
   list elements work.
3. `iter_enumerate`'s `List<(Int, T)>` — verify.

## 5. Tests

- `defn apply<T,U>(x: T, f: T -> U) -> U [true][term == f(x)] { term f(x); }`
  typechecks (the free-U return — the §2.1 blocker).
- `iter_map`/`iter_filter`/`iter_take` compile end-to-end and run correctly
  (runtime MATCH vs a C reference or an expected result).
- `Some(list[i])`/`None` typecheck in an `Option<T>`-returning defn.
- `let m: HashMap<String, Int> = new_map()` binds K/V; `Count#`-style access
  or a `size()` call works.
- All existing tests stay green (`cargo test --lib`).
- Benchmarks: `queue_drain`/`stack_push_pop`/`hash_ops` MATCH (rule 11).

## 6. Acceptance criteria

1. `lib/std/iterator.bv` imports and its non-operator adapters compile
   (`iter_map`, `iter_filter`, `iter_take`, `iter_skip`, `iter_enumerate`,
   `iter_zip`).
2. `lib/std/option.bv`'s `Some`/`None` constructors typecheck in generic
   bodies.
3. `new_map<K,V>()` construction works; its contract is corrected.
4. A generic body can return a free-`U` closure call and insert into a
   `List<U>` (the §2.1 blockers).
5. Full suite green; benchmarks MATCH.

## 7. Out of scope (future plans)

- **Protocol-constrained generics** (`defn f<T: #Int>`) — the `TypeParam.bound`
  field (`src/ast/top.rs`, `TypeParam` struct) is unused; operator-in-generic
  bodies (`acc + x`) need it. This plan excludes it; `iter_fold` is migrated
  around it.
- **Value specialization** (width-specific codegen) — the erased model stands.
- **Typechecker stdlib-op visibility** for Tier-1 types beyond `List` — verify
  `HashMap`'s cursor ops (`op Iter`/`op Step`/`op IsEnd`/`op Current`,
  `lib/std/collections.bv:167-186`) dispatch for `foreach kv in map`.

## 8. Known file map

- `src/typechecker/mod.rs` — inference (`infer_defn_type_args` :3705,
  `unify_defn_type` :3768, `substitute_type_params` :3819), Lambda :769,
  List :738, term binding :1514, expected-type :1893, `check_top_level` :3204.
- `src/ast/types.rs` — the `Type` enum; add `Generic`.
- `src/parser/types.rs` + `src/parser/definitions.rs` — type-param parsing;
  the `genericize` pass.
- `lib/std/iterator.bv`, `lib/std/option.bv`, `lib/std/hashmap.bv` — the
  stdlib targets.
- BUGS.md — the OPEN entry this plan resolves.
