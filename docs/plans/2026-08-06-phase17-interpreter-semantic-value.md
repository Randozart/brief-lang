# Phase 17 — Interpreter Semantic-Value Migration

**Date:** 2026-08-06
**Status:** Investigation complete — implementation not started
**Plan source:** `docs/plans/2026-08-05-implement-normative-language-spec.md` §23

---

## 0. Executive Summary

SPEC §2.2 requires the reference interpreter to operate on **generic semantic
values** — semantic type identity, optimized primitive atoms, bits, products,
sums, references, closures, void — and forbids hardcoding stdlib concepts
(`List`, `HashMap`, `Option`, `Result`, JSON, DOM) as interpreter value
variants. The current interpreter carries 15 ad-hoc `Value` variants and its
eval path is a mix of placeholder stubs and type-specific dispatch.

This plan migrates the interpreter to the semantic model in vertical slices,
each a commit boundary. The migration is **interpreter-internal**: it must not
change codegen output or benchmark numbers (interpreter is a compile-time
reference tool; the native backend never reads `Value`).

A prior estimate of "87 files" was inflated by `PropertyValue`/`NavValue`
false positives and dead modules. The **true blast radius is ~18 live files**:
the interpreter core (6 files), the reactor, the fuzz checker, the derive
(CEGIS) engine, and `analysis/pgo.rs`. Three compound variants
(`Enum`, `Instance`, `HashMap`) have **zero live users** outside their own
definition and can be dropped outright.

---

## 1. Investigation Findings

### 1.1 Value enum today (`src/interpreter/mod.rs:230`)

| Variant | Live producers | Live consumers | Verdict |
|---|---|---|---|
| `Bits(Vec<u8>)` | eval literals, intrinsics | interpreter, derive, pgo, fuzz | keep → `bits` |
| `Int(i64)` | eval | interpreter, derive | keep → `atom Int` |
| `Float(f64)` | eval | interpreter, derive | keep → `atom Float` |
| `Bool(bool)` | eval | interpreter | keep → `atom Bool` |
| `Char(char)` | eval | interpreter | keep → `atom Char` |
| `Void` | eval | interpreter, derive, fuzz | keep → `void` |
| `Ref(Box<Value>)` | eval AddrOf | interpreter | keep → `reference` |
| `List(Vec<Value>)` | reactor, fuzz | reactor, fuzz, interpreter | **replace** (reactor state collections) |
| `HashMap` | — (dead/orphaned only) | — | **drop** |
| `Enum` | — (dead/orphaned only) | — | **drop** |
| `Instance` | — (dead/orphaned only) | — | **drop** |
| `Constructor(String, Vec<Value>)` | derive/engine.rs CEGIS | derive/engine.rs | replace → `sum` + tag |
| `Defn(String)` | — | — | drop (meta) |

**Key finding:** `Enum`, `Instance`, `HashMap`, `Defn` have no live uses outside
their definition (all earlier grep hits were `PropertyValue::List` — a config
type — or orphaned files under `features/` and `archive/`). `Constructor` is
produced and consumed **only** by the derive (CEGIS) synthesis engine.

### 1.2 Live interpreter consumers (build path)

- `compile.rs:656` — derivation assertion verification; runs the interpreter on
  every compile of a program with a derivation block.
- `reactor.rs` — node pre/post conditions and state collections
  (`Value::List` at reactor.rs:318).
- `protocol_verify.rs` — protocol contract evaluation.
- `analysis/pgo.rs` — PGO contract evaluation (`Value::Bits`).
- `analysis/region.rs` — has its own `eval_expr_simple`, does **not** use
  `interpreter::eval_expr` (separate path, out of scope).
- `fuzz_checker/mod.rs` — fuzz case evaluation.

The native backend, optimizer, and codegen never read `Value` — the migration
cannot perturb benchmarks by construction, but **every slice still runs the
full benchmark suite** (rule 11) because `compile.rs` runs derivation
verification on some programs.

### 1.3 Eval-path feature gaps (the actual user-facing unlocks)

`src/interpreter/eval.rs` arms that are placeholders or wrong:

| Expr | Current behavior | Target |
|---|---|---|
| `List(exprs)` | `zero_bits(8) // placeholder` (eval.rs:63) | `product` value |
| `Index(obj, idx)` | `zero_bits(8) // placeholder` (eval.rs:73) | index into product/bits |
| `Slice { .. }` | evals array, drops slice (eval.rs:199) | range+slice over bits/string |
| `Match(_, arms)` | evals first arm body, no pattern/guard (eval.rs:117) | full pattern matching |
| `Lambda(_, _)` | `Value::Void` (eval.rs:126) | `closure` value |
| `Field` (2nd arm) | `Void` (eval.rs:149; dead dup of line 70) | product field access |
| `MethodCall` | `Void` (eval.rs:180) | dispatch + `Reflect` |
| `IsType` | always `Bool(true)` (eval.rs:111) | real membership check |
| `Reflect` | only String `Len`/`Bytes` (eval.rs:153) | descriptor reflection |
| `Tuple(exprs)` | returns first element (eval.rs:55) | `product` value |
| `DerivationBlock`/`StructLiteral` | `Void` (eval.rs:129) | struct construction |

### 1.4 Dead / out-of-scope code confirmed

- `features/{block,call,collection,dbvl,ellipsis,field,pattern,projection,sigcall,stmt,subtype,toplevel,tuple}.rs`
  are **orphaned** (not referenced from `features/mod.rs`) and do not compile.
  Their `Value::List/Instance/Enum` uses are irrelevant to the migration.
- `archive/` and `ffi/archive/` are non-live.
- `dispatch_ffi` (`interpreter/ffi.rs`) is a stub returning
  `UndefinedForeignFunction`.

### 1.5 Semantic model prerequisites

- No `TypeId`/semantic descriptor exists in `type_universe/` today
  (`type_universe/{mod,operators,resolve,validate}.rs`); `resolve.rs` states
  semantics come from bootstrap.bv source declarations.
- AST already carries the constructs the interpreter lacks: `Expr::Slice`,
  `Expr::Lambda`, `Expr::Reflect`, `Statement::Foreach`, `Pattern::Range`.
  The gap is entirely in eval, not parsing.
- 46 interpreter tests exist across `eval.rs` (17), `intrinsics.rs`, `mod.rs`.

---

## 2. Semantic Value Model

Replace the 15-variant enum with the SPEC §2.2 generic model:

```rust
pub enum Value {
    Atom(Atom),          // Int, Float, Bool, Char, Ptr (raw, bit-identical)
    Bits(Vec<u8>),       // raw bytes: opaque payloads, heap strings
    Product(Vec<Value>), // struct/tuple fields, in declared order
    Sum { tag: usize, payload: Vec<Value> },  // enum variant, tagged
    Ref(Box<Value>),     // reference / address-of
    Closure { params: Arc<Vec<String>>, body: Arc<Expr>, env: Arc<HashMap<String, Value>> },
    Void,
}
```

- **Semantic type identity** lives at the *typechecker* level (a `TypeId` that
  maps `Type` → universe key), not inside `Value`. The interpreter is
  dynamically typed; `Sum`/`Product` carry only what eval needs (tag, field
  order). This keeps eval decoupled from the casting graph and matches how the
  interpreter is invoked today (values are untyped at runtime).
- `Atom` is a small dense enum; `as_i64/as_f64/as_bool` become methods on
  `Value` delegating to `Atom` (signature-compatible with today's callers).
- Bootstrap atoms `Int/Float/Bool/Char` remain **first-class**, matching codegen
  protocol-category dispatch (the 2026-08-01 audit rationale stays valid).

### 2.1 Boundary conversion

The only places that may construct `Product`/`Sum`/`Bits` from named stdlib
types are:
- the FFI marshalling layer (`interpreter/ffi.rs` — currently a stub),
- the derive (CEGIS) engine for synthesis candidates.

No eval path matches `"List"`, `"HashMap"`, `"Option"`, JSON, or DOM names.

---

## 3. Vertical Slices (each a commit boundary)

Order chosen to keep every commit green and each slice independently testable.
Slices are dependencies of the next; per AGENTS "migrate when touched", a slice
fixes all stubs it touches.

### Slice A — Drop dead variants, introduce `Atom`
- Remove `Enum`, `Instance`, `HashMap`, `Defn` from `Value`.
- Introduce `Atom { Int, Float, Bool, Char }`; keep `Value` API
  (`as_i64`, `as_f64`, `as_bool`, `i64_to_bits`, …) source-compatible via
  delegating constructors so no consumer churn.
- Delete the orphaned `features/*` files that referenced dropped variants
  (they don't compile today; archiving is required so the dead-variant
  references leave the tree).
- Tests: all existing 46 interpreter tests pass unchanged; new test that
  `Value::Enum`/`Value::Instance` no longer exist.

### Slice B — Correct the primitive eval arms
- Fix `Expr::IsType` (real membership), `Expr::Cast` stays, `Expr::Within`.
- `Expr::Tuple` → `Product`; `Expr::List` → `Product` (unnamed product; named
  stdlib list behavior remains stdlib, not interpreter).
- `Expr::Index` → index into `Product`/`Bits` with bounds error.
- Tests: tuple/index/istype eval tests.

### Slice C — Match with patterns, guards, exhaustiveness
- Full `Expr::Match`: evaluate scrutinee, walk `Pattern` (Wildcard, Literal,
  Binding, Tuple, EnumVariant, Range), bind names, evaluate guards
  (`when`), pick first matching arm, signal non-exhaustive match as error.
- `Expr::Sum` construction from `Expr::Constructor` (if parser/typechecker
  produce it) — verify parser output first.
- Tests: exhaustive guard tests, binding capture, range patterns, failure
  paths.

### Slice D — Struct/enum construction, field access, method calls
- `Expr::StructLiteral` → `Product`; `Expr::Field` → field access by index
  (order-resolved, not name-matched at eval time).
- `Expr::MethodCall` → resolve method, evaluate args, apply (mirror codegen
  dispatch).
- `Expr::DerivationBlock` → evaluate examples (feeds compile.rs derivation
  verification).
- Tests: struct literal + field access, method call eval.

### Slice E — Closures
- `Expr::Lambda` → `Value::Closure` capturing `env` (snapshot of bindings).
- Call path: `Expr::Call` on a `Closure` value applies it with a scoped env;
  non-# names no longer fail as `UndefinedVariable`.
- Tests: closure capture, application, nested closure, re-entrancy.

### Slice F — Slices, ranges, strings
- `Expr::Slice` → range/stride over `Bits` (strings) and `Product`.
- `Pattern::Range` (needed by Slice C) — implemented in Slice C, reused here.
- Tests: string slicing, stride, bounds, out-of-range errors.

### Slice G — Reflection and collection boundary
- `Expr::Reflect` runtime/compile-time kinds beyond string `Len`/`Bytes`:
  product field count, sum tag, atom category — the value-side half of
  reflection (descriptor side is Phase 7 frontend work, tracked separately).
- `reactor.rs` state collections migrate from `Value::List` to `Product`
  (its only compound use).
- `fuzz_checker/mod.rs` and `analysis/pgo.rs` migrate to the new API.
- Tests: reflection eval tests; reactor state round-trip.

### Slice H — Derive (CEGIS) constructor migration
- `derive/engine.rs` `Constructor` → `Sum { tag, payload }` + tag-name table.
- Assertion/equivalence/mcmc/verify migrate atom usage.
- Tests: existing derive tests pass; new synthesis round-trip.

### Slice I — FFI boundary + closure of Phase 17
- Implement `dispatch_ffi` marshalling to build `Product`/`Sum`/`Bits` at the
  boundary (replaces the `UndefinedForeignFunction` stub for the marshalling
  types it can express).
- Sweep: grep guarantees — no `Value::List`/`HashMap`/`Enum`/`Instance`/
  `Constructor` in live code; no eval arm matches stdlib type names.
- Update status matrix §17 to done; close the phase.

---

## 4. Benchmark Baseline (required by rule 11)

Baseline measured at commit `6ff24d59` (Phase 15 .f frontend, immediately
before Phase 17 begins), `cargo build --release` + `bash
benchmarks/build_and_bench.sh --runtime`. All 36 benchmarks **MATCH** (C vs Briv
times, ratio, winner):

| Benchmark | C (s) | Briv (s) | Ratio | Winner |
|---|---:|---:|---:|---|
| ring_buffer | .0552 | .0462 | 1.19x | C |
| float_math | .0433 | .0691 | .62x | Briv |
| float_math_nonzero | .1594 | .1663 | .95x | Briv |
| sparse_dispatch | .0572 | .0679 | .84x | Briv |
| print_loop | .0328 | .0588 | .55x | Briv |
| nbody_newton | 7.4690 | 8.7524 | .85x | Briv |
| nbody_sqrt | 2.2951 | 2.9866 | .76x | Briv |
| nbody_sqrt_idio | 3.2748 | 4.0351 | .81x | Briv |
| fasta | .2183 | .2362 | .92x | Briv |
| fannkuch_redux | .0604 | .0672 | .89x | Briv |
| mandelbrot | .7245 | .6838 | 1.05x | C |
| kalman_filter_runtime | .1558 | .1805 | .86x | Briv |
| knucleotide | .1930 | .1935 | .99x | Briv |
| cancel_math | .0562 | .0645 | .87x | Briv |
| bit_clear | .0003 | .0002 | 1.50x | C |
| queue_drain | .0362 | .0631 | .57x | Briv |
| queue_drain_sym | .0362 | .0653 | .55x | Briv |
| queue_drain_idio | .0363 | .0654 | .55x | Briv |
| stack_push_pop | .0327 | .0602 | .54x | Briv |
| interval_step | .0633 | .0639 | .99x | Briv |
| telemetry_stream | .1952 | .2047 | .95x | Briv |
| pid_control | .3458 | .3489 | .99x | Briv |
| matrix_pipeline | .4623 | .7168 | .64x | Briv |
| accumulator_flush | .1133 | .1514 | .74x | Briv |
| sweep_sparse | .2198 | .1577 | 1.39x | C |
| sweep_mid | .2640 | .2360 | 1.11x | C |
| sweep_dense | .3948 | .2666 | 1.48x | C |
| sweep_arr | .4021 | .3443 | 1.16x | C |
| series_converge | .0003 | .0003 | 1.00x | ~tie |
| global_lifetime | .0305 | .0732 | .41x | Briv |
| deep_recursion | .0001 | .0004 | .25x | Briv |
| arena_churn | .0891 | .1028 | .86x | Briv |
| linked_list | 1.1528 | 1.6884 | .68x | Briv |
| hash_ops | 1.0211 | 1.1226 | .90x | Briv |
| hash_ops_idio | .0305 | .0543 | .56x | Briv |
| enemy_swarm | .0919 | .1347 | .68x | Briv |

`bridge_glue` SKIP, `bridge_multi` PASS. 36/36 runtime benchmarks MATCH.

**Expectation:** every slice must leave the table unchanged. Because the native
backend never consumes `Value`, a changed number is a signal the migration
leaked into codegen — investigate, don't accept.

---

## 5. Test Strategy

- Every slice: `cargo test --lib` green, Praetor on changed files (max
  complexity 15 / params 6 / lines 100), no new warnings.
- New tests per slice as listed above (behavioral, not literal — a test must
  pass after refactor if behavior is preserved).
- Interpreter rule (#4): the interpreter is the reference. Slices that fix an
  eval arm must be cross-checked against the backend emission semantics (e.g.
  `Cast` value conversion vs codegen protocol-category dispatch).
- Kani harnesses only for safety-critical new code (bounds-checked index/slice
  helpers qualify).
- After Slice C and Slice G, run `.smoke` fixtures + a program using
  `match`/`foreach` through the interpreter (derivation verification path).

---

## 6. Documentation Updates (in the same structural commits)

- `spec/SPEC.md` — no change (SPEC already mandates the model; we converge).
- `docs/architecture/overview.md` — interpreter section: document the generic
  value model, boundary conversion points, and the "no eval path matches
  stdlib type names" guarantee.
- `docs/architecture/intrinsics-vs-stdlib.md` — clarify where list/map behavior
  lives (stdlib, via FFI boundary), not interpreter variants.
- `src/interpreter/mod.rs` header comment — rewrite the 2026-07-14 FFI-only
  variants note to the semantic model; preserve the historical rationale as
  rewrite, not delete.
- `docs/plans/2026-08-05-spec-implementation-status.md` §17 row → done at
  Slice I; intermediate rows record per-slice progress.
- `AGENTS.md` — no change required (rules already cover this).

---

## 7. Risks and Mitigation

| Risk | Impact | Mitigation |
|---|---|---|
| Migration leaks into codegen (backend reads `Value`) | Benchmark regressions | Native backend never reads `Value` (verified: grep `Value::` under `backend/` returns only `PropertyValue`); run full suite per slice |
| Derivation verification on compile path breaks | Builds fail for programs with derivation blocks | compile.rs tests + `.smoke` fixture through Slice D; keep `call_function`/`load_program` API stable |
| Reactor state collections (`Value::List`) | Runtime simulation wrong | Slice G dedicated; reactor tests exist |
| CEGIS engine depends on `Constructor` shape | Synthesis verification breaks | Slice H converts engine + tests atomically |
| `as_i64`/`as_f64`/`string_bytes` signature drift | Wide consumer churn | Keep public `Value` helper API source-compatible across Slice A |
| Scope creep into Phase 7/8 frontend work (reflection descriptors, range parser) | Plan slips | Slices F/G deliver only the *value-side*; descriptor frontend tracked separately |

---

## 8. Out of Scope

- Phase 7 reflection descriptor frontend (`.^`/`.^^` parse→descriptor); only the
  value-side eval arm ships here.
- Phase 8 closures in codegen (LLVM closure env lowering); interpreter closure
  value ships here, backend lowering is separate.
- `analysis/region.rs` `eval_expr_simple` (own path, not `interpreter::`).
- `PropertyValue`/`NavValue` (unrelated config types — do not confuse with
  `Value`).
- Benchmarks that read `Value` — none exist.

---

## 9. Slice Status Tracker

- [x] A — Drop dead variants, introduce `Atom` (commit boundary) — 2026-08-06
- [x] B — Correct primitive eval arms (IsType/Tuple/List/Index) — 2026-08-06
- [x] C — Match with patterns, guards, exhaustiveness — 2026-08-06
- [x] D — Struct/enum construction, field access, method calls — 2026-08-06
- [x] E — Closures — 2026-08-06
- [x] F — Slices, ranges, strings — 2026-08-06
- [x] G — Reflection value-side + reactor/fuzz/pgo migration — 2026-08-06
- [x] H — Derive (CEGIS) constructor migration — 2026-08-06
- [x] I — FFI boundary + Phase 17 close (grep guarantees, status matrix) — 2026-08-06

**Phase 17 complete** (2026-08-06). The interpreter Value model now matches
SPEC §2.2 exactly: atoms, bits, products, sums, references, closures, void.
`dispatch_ffi` marshals at the boundary (marshal/unmarshal conversion +
intrinsic-surface dispatch); compound construction from named types happens
only in `ffi.rs` and the derive engine. Grep guarantees verified: no
`Value::List`/`HashMap`/`Enum`/`Instance`/`Constructor`/`Defn` in live code,
no eval path matches stdlib type names.
