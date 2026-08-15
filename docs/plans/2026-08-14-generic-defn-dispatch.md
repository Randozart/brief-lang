# Plan: Generic `defn f<T>` dispatch — type-param inference at call sites

**2026-08-14.** The Universal Operation Language (UOL, §6b) gave every
operation a uniform surface (`OpName#`, UFCS). This plan delivers the
**generic-function layer**: `defn f<T>(...)` dispatch, so stdlib helpers like
`iter_map<T,U>`, `new_map<K,V>`, `new_stack<T>` actually work. It was listed
as the §8 follow-up; it is now the active work.

## 1. The gap (verified)

Generic `defn f<T>` **parses** (`parse_definition` reads `type_params`,
`definitions.rs:559`) but does not **dispatch**:

- `fn_return_types[name]` stores the RAW return type with `T` free
  (`typechecker/mod.rs:2515-2557`).
- `fn_param_types[name]` stores RAW param types with `T` free
  (`:2571-2576`).
- `infer_call` (`:1306-1353`) validates args against the raw param types
  (`List<T>` vs `List<Int>` → mismatch) and returns the raw return type (`T`
  instead of `Int`).
- The defn BODY is elaborated once at declaration with `T` free
  (`:1454-1458`); a `T`-typed op call (`At#(xs, 0)` on `List<T>`) resolves
  structurally but its result is the free `T`.
- Codegen emits the defn ONCE (`emit_definition`, `emit_toplevel.rs:1993`)
  with `T`-typed params falling through `fallback_llvm_type` → `"i64"`.

Verified failure (`generic4.bv`): `defn first<T>(xs: List<T>) -> T` body
typechecks (returns `T`), but `first(items)` where `items: List<Int>` returns
raw `T` → `type mismatch: expected Int for assignment, found T`.

## 2. Design decision: type-erased generics (Go-style), NOT monomorphization

The body is emitted ONCE; `T`-typed values are **i64 boxes** (the codegen
already falls through to `"i64"` for `TypeVar`). This matches Briev's
boxed-i64 ABI and avoids per-instantiation emission machinery. Consequences:

- `defn first<T>(xs: List<T>) -> T` emits one `@first`, taking/returning i64.
- A `T` that is a collection (`List<Int>`) is a boxed handle — already i64.
- No C++-style codegen duplication. Specialization (width-specific codegen)
  is a FUTURE plan; it would re-emit per instantiation, but the type-erased
  form is the correct baseline and is what the stdlib needs.

The typechecker does the inference + substitution; codegen is unchanged.

## 3. Work items

### 3.1 Typechecker: carry type params per defn

`TypecheckContext` gains `fn_type_params: HashMap<String, Vec<String>>`
(the defn's declared type params, e.g. `["T"]` for `first`), populated
alongside `fn_return_types`/`fn_param_types` from `Definition.type_params`
(`:2515-2576`).

### 3.2 Typechecker: infer type params at the call site (`infer_call`)

In `infer_call`, before arg validation:
1. Get `fn_type_params[name]` and `fn_param_types[name]`.
2. **Unify** each param type against the corresponding arg type, extracting a
   substitution `{T: Int, ...}`. Unification walks the type recursively:
   `List<T>` vs `List<Int>` → `{T: Int}`; `T` vs `Int` → `{T: Int}`;
   `HashMap<K,V>` vs `HashMap<String,Int>` → `{K: String, V: Int}`; concrete
   vs concrete mismatch → error (as today). If a param is `T` (free) and the
   arg is any type → bind it.
3. **Substitute** the bindings into the param types (for arg validation —
   `substitute_type_params`, `:3598`) and into the return type (`:1349`).
4. Fall back to the current behavior (raw types) when the defn has no type
   params.

`substitute_type_params` already handles `Type::Custom("T")` (a type param
parses as `Custom`, `parser/types.rs:217`). Extend it to also substitute
`Type::TypeVar` if any defn uses that form.

### 3.3 Typechecker: body elaboration with bound params

The body is elaborated once with `T` free (`:1454`). This already works for
`At#(xs, 0)` → `T` (verified: the body typechecked). No change needed to the
body elaboration itself; the call-site substitution makes the RETURN type
concrete. (If a body op needs the concrete type for validation — e.g. an op
only declared on `List<Int>` — that's a per-instantiation concern deferred to
a future specialization plan.)

### 3.4 Codegen: no change

`emit_definition` emits once; `T`-typed params/returns fall through to i64
(`fallback_llvm_type`). The body's `T`-typed values are i64 boxes. Verify the
call site passes the boxed arg and consumes the i64 return.

### 3.5 Stdlib verification

The motivating cases become usable:
- `new_map<K,V>()`, `get<K,V>`, `put<K,V>` (hashmap.bv)
- `new_stack<T>()`, `push<T>`, `pop<T>` (stack.bv)
- `iter_map<T,U>`, `iter_filter<T>`, `iter_take<T>` (iterator.bv)

### 3.6 Tests

- `defn first<T>(xs: List<T>) -> T` over `List<Int>` and `List<String>`
  (typecheck + codegen: one `@first`, i64 ABI, correct return).
- Two type params: `defn pair<T,U>(a: T, b: U) -> (T, U)`.
- Generic over a collection op: `defn len_of<T>(xs: List<T>) -> Int` calls
  `Count#(xs)`.
- Mismatch: `first(5)` (Int, not a List) → clean error.
- Interpreter parity: `execute`-level if the interpreter has generic defns
  (verify; the interpreter may treat a generic defn as its raw signature —
  document the boundary if so).

### 3.7 Docs

- SPEC §13 (definitions): generic `defn f<T>` — type-param inference at call
  sites, type-erased codegen.
- `learn-briev`: a generic-functions section.
- Arch: note in `iterable-protocol.md` that `Count#`/`At#` make generic
  functions expressible (the §8 follow-up).

## 4. Out of scope

- **Value specialization** (width-specific codegen, C++-style re-emission) —
  future plan; the erased form is the baseline.
- **The typechecker stdlib-op visibility gap** (op-bearing types need an
  explicit `import "std/collections.bv"` for `type_members`; `foreach` works
  via fallback) — separate investigation, pre-existing.
- Protocol-constrained generics (`defn f<T: #Int>`) — the bound field on
  `TypeParam` exists but is unused; future.

## 5. Commit sequence

1. **Typechecker**: `fn_type_params` registry + call-site inference +
   substitution (`infer_call`) — commit.
2. **Tests + interpreter parity + docs** — commit.
3. **Stdlib verification** — compile `new_map`/`new_stack`/`iter_map` paths;
   benchmark MATCH — commit (if any stdlib change is needed).

## 6. Execution addendum (2026-08-14)

**Commit `c0adc6f1` landed the core** (§3.1-3.2): `fn_type_params` registry,
call-site inference (`infer_defn_type_args` + `unify_defn_type`), substitution
into param validation + return type, and expected-type binding for nullary
generics (`let s: Stack<Int> = new_stack()` seeds `ctx.expected_call_type`).
Verified end-to-end: `defn first<T>(xs: List<T>) -> T` over `List<Int>` and
`List<String>` (one `@first`, i64 ABI); `defn count_of<T>(xs: List<T>) -> Int`
calling `Count#` in the body; two type params infer independently; a
non-matching arg is a clean type error. 1850 tests green.

**Remaining blockers for the §3.5 stdlib goal (documented, not yet fixed):**
- **Body literals with free `T`**: a generic body's `[]`/`{}` literal defaults
  to `List<Int>`/concrete, not `List<T>` (`empty_list<T>` returns `[]` →
  mismatch against `List<T>`). Needs literal inference to consult the declared
  return type (`current_output_type`).
- **Closure-typed generics** (`iter_map<T,U>(list, f: T -> U)`): the closure
  `x -> x` has no typecheck case (infers `Int`), so `T -> U` unification can't
  bind. Pre-existing closure-typing gap.
- **Typechecker stdlib-op visibility**: `List`'s ops are visible via
  `import "std/collections.bv"`; `HashMap`'s Tier-1 ops and the `Count#`-on-a-
  Tier-1-type mismatch are stdlib-design issues (HashMap has no `op Count`).
  The plan §4 note stands — op-bearing non-List types need attention.
- These are follow-ups; the core args-driven generic dispatch is complete.

**Commit `5ce9aa8c` (closure-typed generics + generic txns):** the closure-
typing and generic-txn blockers are resolved:
- **Generic txns parse** — `parse_transaction` reads type params
  (`txn iter_map_loop<T, U>`); the stdlib iterator adapters were generic txns
  that failed to parse.
- **Function-typed params parse** — `parse_parameter_list` handles `f: T -> U`
  and `f: (U, T) -> U` (a param LIST, not a tuple, matching the stdlib).
- **Multi-param lambdas** — `(a, b) -> body` (parse_grouping lambda detection);
  the single-param `x -> body` form was already the canonical SPEC §9.2
  syntax. The stdlib's `|acc, x|` pipe-lambdas migrated to `(acc, x) ->`.
- **`unify_defn_type` + `substitute_type_params` handle `Type::Function`** — a
  closure-typed generic param (`iter_map(items, f)` with `f: T -> U`) infers
  `T`/`U` from the closure shape.
- `iterator.bv`'s `result.append(x)` migrated to `result <- x` (op InsertAt).
- Verified: `iter_map`/`iter_map_loop` inference succeeds end-to-end; 1855
  tests green; benchmarks MATCH.
- **Remaining** (documented in BUGS.md): other iterator.bv functions
  (`iter_fold`/`zip`/`enumerate`/`find`/`max`) have free-`T`-body and
  `Option`/`Some`/`None` constructor issues; hashmap.bv's `new_map<K,V>()`
  nullary construction is unverified. These are a stdlib-cleanup pass
  (separate from the dispatch core).

> **2026-08-14 handoff:** the stdlib-cleanup pass is now its own plan —
> `docs/plans/2026-08-14-stdlib-cleanup.md`. See it for the free-`T`-body
> (`Type::Generic`), `Option` constructor, and `new_map` work items.

**Additional commit `7972e4d2` (`term` canonical result placeholder):** a
follow-up that came up during generic-contract testing. `term` in a defn/txn
POST-condition is now bound to the declared output type during elaboration
(previously the `elaborate_expr` `unwrap_or(Type::int())` fallback made it
typecheck by accident). Post-condition `term` type mismatches are now REAL
errors. SPEC's four `#R`-as-result examples (lines 696/873/1260/1611) migrated
to `term`; the op-binding runes (`#Lh`/`#Rh`) are untouched. This makes
`[term == true]`, `[term.Count#() == n]`, `[term == []]` type-correct, which
generic contracts depend on. 1853 tests green; runtime benchmarks MATCH.

