# Plan: String unification, `.^^Element`, and the op/reflection/intrinsic boundary

> **2026-08-14 follow-up addendum (all three follow-ups done).**
> - **(a) `tier2_op_collection` misses `List` — FIXED.** Root cause:
>   `lib/std/collections.bv` (where `obj List<T>` declares `op Count`/`op At`)
>   was never prelude-imported, so `obj_members` had no `List` in default
>   builds and `foreach` silently fell back to the hardcoded List layout.
>   Fix (`a53c7f90`): the prelude plugin now imports `std/collections.bv`
>   (both branches). Verified: `foreach x in list` emits the Count/At member
>   bodies inlined. Full runtime suite MATCH, no regression. The hardcoded
>   `List` foreach arm is now production-dead (reached only by stdlib-free
>   unit tests); the `emit_heap_seq`/tuple blocker remains (separate refactor).
> - **(b) `ringbuf_inline` removed — no regression.** Performance Recovery
>   Protocol experiment (rule 19): gated the `has_insert_strategy` registration
>   OFF and benchmarked the three InsertAt benchmarks. Results (Briev seconds):
>
>   | benchmark | with ringbuf_inline | without | delta |
>   |---|---|---|---|
>   | queue_drain_idio | .0356s | .0348s | −0.2% |
>   | stack_push_pop | .0345s | .0341s | −0.1% |
>   | queue_drain | .0331s | .0343s | +0.4% |
>
>   All MATCH, no regression — the SROA claim (comment at the removed site)
>   does not hold on the current op-surface architecture. Deleted the
>   mechanism (registration, field-modes loop, init pass, handle-derivation
>   special case, `RingbufInlineFields` struct + field). Full runtime suite
>   after deletion: all MATCH. This resolves plan §10.1 (`ringbuf_inline`).
> - **(c) `.^Absolute` removed** (`a44ee863`): after the one-release deprecated
>   alias, it is now an unknown-target error directing to `Abs#` (SPEC §17.3).
>   Typechecker/codegen/interpreter arms removed; stale `.smoke`/`string.ebv`
>   fixtures migrated to `Abs#`/`.^Length`.

> **2026-08-14 UOL addendum.** §6b (the Universal Operation Language — every op
> has a symbol, an intrinsic `OpName#`, and a UFCS method form `a.OpName#(b)`)
> supersedes the earlier §6a direction: the runtime `.^Size` `len` heuristic is
> DELETED (tri-partite rule: element count is an operation, so its home is the
> `Count#` intrinsic, not reflection), not routed through `op Count`. The
> generative `OpName#` dispatch + UFCS fallback is the mechanism that makes
> `Count#`/`At#` uniform. This is plan-driven work yet to execute (§7 commits
> 6-7).

> **2026-08-14 completion addendum.** All commits in §7 landed (green):
> `74d13f19` (docs), `b88f6ee6` (String unification), `54136420` (`.^^Element`),
> `ca9120f2` (Abs# + bit intrinsics), `0ea07424` (slice-5 diagnostic),
> `41a06055` (slice-6 blockers recorded). Deviations from the plan, all
> verified:
> - `Abs#` was **already** dispatched via `template_for_op` (the plan's "no
>   dispatch" claim was stale) — the migration was registration + deprecation.
> - The `Abs#` "arity-broken" claim was wrong: `parameters: vec![]` is the
>   inferred-arity convention shared by Add#/Neg#/etc. — unchanged.
> - Slice-6 deletions 1-3 are BLOCKED on live paths (see `BUGS.md`
>   "Iterable-protocol slice-6 deletions blocked"): `emit_heap_seq` serves
>   `Expr::Tuple` + expression-position literals; the hardcoded `List` foreach
>   arm is the live path because `tier2_op_collection` doesn't fire for `List`
>   in real typechecks. §10.4/6/7 were already done. `ringbuf_inline` kept.
> - **Follow-ups (separate):** (a) isolate why `tier2_op_collection` misses
>   `List` — unblocks the slice-6 deletions; (b) ringbuf A/B rebuild under the
>   Performance Recovery Protocol with a baseline table; (c) `.^Absolute` →
>   unknown-target error after the deprecation release.

**2026-08-14.** Consolidates the remaining work from two working drafts
(`.opencode/plans/2026-08-14-docs-reconciliation.md`,
`.opencode/plans/2026-08-14-op-vs-reflection-boundary.md`) with the String
unification decision. Executes in commits so each is green.

## 1. Boxed Cat Typing — disambiguation (docs only)

**2026-08-14.** "Boxed Cat Typing" is a **Schrödinger's cat** pun, not literal
i64 boxing. A type's representation is indeterminate until a backend
*collapses* it (materialization) or `pack`/`seq`/`spec` *pins* it. The value
registers in `backend-strategy.md` being i64 is coincidental; the joke is
"the cat is in the box, neither alive nor dead until observed" — the type is
neither `Bits(N)` nor a laid-out struct until a codegen site opens the box.

Disambiguation lands in three places:
- `docs/plans/2026-08-13-layout-keywords.md:17-18` — append the Schrödinger's
  cat note to the brand-term definition.
- `docs/architecture/agent-reference.md` Physical layout section — one line.
- `learn-briev/05-data-types.md` — one line in the Physical Layout Modifiers
  section.

SPEC §8.1 callouts stay formal (the joke lives in docs/learn only).

## 2. String unification — `#String` is `Iterable<Char>`

**Design decision (2026-08-14):** pragmatic-efficiency, not purity. The hard
lines are: **no magic matching** (no name-based String special-casing) and
**genuinely fast** (competitive with C). The four Tier-1 cursor ops on
`type String` are the "pure" path — deferred (IR churn per char), not this
plan.

- **Typing is structural (Boxed Cat holds).** `foreach_item_type`
  (`typechecker/mod.rs:3333`) and `.^^Element` derive `#String` → `Char` as a
  **protocol fact** via `declared_protocol_of == "#String"` (rule 14/18: never
  `n == "String"`). Char is the observed element type of `#String` (SPEC
  §17.2). The frozen descriptor and the foreach binding agree by construction.
- **Codegen is a protocol-keyed fast lane.** New `IterKind::String` in
  `emit_stmt.rs`, keyed on `is_string_operand` (disclosed, protocol check):
  bound = `.^Length` (the `[len]` byte header, O(1) load), per-char one runtime
  lane call `briev_str_next_char(ptr, &off) → codepoint` advancing the byte
  offset, item bound as `Char`. Falls in `try_emit_tier_iteration` AFTER tier1
  (an op-bearing String subtype still uses ops) and BEFORE the Data byte
  fallback. `#Data` keeps its byte loop (element Int).
- **Runtime learns.** `briev_str_next_char` added to `lib/runtime/briev_rt.c`
  (UTF8 decode + advance — where `str_first_char`/`briev_char_len` already
  live). LTO-inlineable with the harness (`clang -O3 -flto`).
- **Interpreter parity** (rule 4): String-typed `Value::Bits` foreach decodes
  codepoints → `Atom::Char`; Data stays bytes. The interpreter holds both as
  `Value::Bits` (`mod.rs:353-355`); a `#String`-typed marker distinguishes
  them — resolved during execution.
- **Tests**: ASCII + multibyte UTF8 `foreach c in str` (correct bounds, Char
  item); `.^^Element` on String → Char; interpreter parity; IR contains one
  `briev_str_next_char` per iteration and the `.^Length` bound. SPEC §17.1 /
  arch §8 updated (String→Char real, not aspirational).

## 3. `.^^Element` — proof form, single source

- `resolve_reflect` (`typechecker/mod.rs:3147`) + `Expr::Reflect` caller
  (`:1039`): add the `"Element"` compile-time arm. Derivation order:
  1. `#String` protocol → `Char` (frozen protocol fact).
  2. op-bearing type → read-op return substituted (same evidence as
     `foreach_item_type` — `op At` Tier 2, `op Current` Tier 1).
  3. `Type::Vector(inner, _)` → inner.
  4. else → non-iterable error. Wrong-kind (`^.Element`) → existing error.
- Codegen fold (`emit_reflection`, `emit_expr.rs`): mirror `.^^Type`
  (`:2817-2822`), emitting the element's category code constant. Needs a
  `Type`-based element lookup in the backend — refactor the shared element
  resolution out of `tier2_op_collection`/`tier1_cursor_collection`
  (`emit_toplevel.rs:165/213`) (rule 17).
- SPEC §17.2: rewrite "cross-checked" language to the single-source proof form
  (the element type *is* the read-op return; generic-args and op-return cannot
  drift). `grep -rn "^^Element" src/` returns code, not zero.

## 4. Boundary — `Abs#` wins + the four bit intrinsics

**Corrected stale claims** (verified 2026-08-14): `Abs#` **already dispatches**
via `template_for_op` (`intrinsics.rs:197` int → `llvm.abs.i64`, `:183` float →
`llvm.fabs`), reachable through `emit_intrinsic_call`. The migration is
registration + deprecation, not new machinery. The four bit intrinsics have
LLVM declares (`emit_toplevel.rs:422-426`) but **no** dispatch/signature/
interpreter arms.

- `Abs#` signature arity fix (`intrinsic_signatures.rs:50` has
  `parameters: vec![]` → `[("x", ...)]`).
- `BitReverse#`/`Popcount#`/`LeadingZeros#`/`TrailingZeros#`: signatures +
  interpreter + LLVM dispatch (`llvm.ctpop`/`ctlz`/`cttz`/`bitreverse`); extend
  the parity-test list (`intrinsic_signatures.rs:328-346`).
- `.^Absolute` → deprecated alias: typechecker warns "use `Abs#`", same
  emission (`emit_expr.rs:2806`), for one release; then a follow-up makes it
  an unknown-target error.
- SPEC §17.3 rewritten (the five are `X#` intrinsics; `.^Absolute` is not a
  reflection target); arch `PopCount#`→`Popcount#` (`iterable-protocol.md:238`);
  CIRCT `Bitreverse#` spelling note.
- Tests: IR contains `llvm.abs`/`llvm.ctpop`/`llvm.ctlz`/`llvm.cttz`/
  `llvm.bitreverse`; interpreter parity; `.^Absolute` warn test; `abs(-5)`
  round-trip.

## 5. Docs reconciliation (from `.opencode/plans/2026-08-14-docs-reconciliation.md`)

| # | File | Change |
|---|---|---|
| 1 | `spec/SPEC.md` §21.4 | backport the 4-op Tier-1 contract |
| 2 | `docs/architecture/iterable-protocol.md` §9 | same 4-op backport |
| 3 | `AGENTS.md:76` | `../briev-compiler-baseline` → `../briv-compiler-baseline` |
| 4 | `docs/plans/2026-08-13-layout-keywords.md` | addenda: `spec Align`→`spec Alignment`, §19.7→§17.2 |
| 5 | `docs/plans/2026-08-12-iterable-protocol.md` | addenda: §11.1→§11.4.1, Tier-1 row superseded |
| 6 | `docs/plans/2026-08-06-endprogram-beginprogram.md` | addendum: `defer` cleanup under-specified |
| 7 | `docs/plans/2026-08-05-implement-normative-language-spec.md` | addendum: `exit program`→`endprogram` |
| 8 | `docs/plans/2026-08-03-glue-folders-node-bridge.md` | addendum: `as`/`fallback` removed, `glue.dbvl`→`glue.dbv` |
| 9 | `docs/plans/2026-07-31-frontend-driven-dispatch.md` | addendum: `.dbvl` on disk |

## 6. Slice-5 remainder + scoped slice-6 cleanup

- Unconstrained literal diagnostic (`let x = [1,2,3]` → "type annotation
  required"); typed literals already route via `construct_local_collection`
  (`emit_stmt.rs:353`).
- Delete `emit_heap_seq`/`emit_svo_list`/`emit_svo_index` after verifying
  `Expr::Tuple` (`emit_expr.rs:746`) and typed-literal coverage.
- **Keep `ringbuf_inline`** — a perf mechanism, not legacy layout; deferred to
  the follow-up benchmark plan.
- `foreach_collection_kind` `"List"` arm (`emit_stmt.rs:238`) → dead once tier
  iteration covers all op-bearing collections — verify, then remove.

## 6a. `.^Size` deleted; element count is the `Count#` intrinsic

**2026-08-14 (Boxed Cat principle, iterable-protocol §10.4 row 4, tri-partite
rule).** The runtime `.^Size` codegen arm (`emit_reflection`,
`emit_expr.rs:2705-2736`) reads the count by matching a `len` SLOT NAME — the
pre-iterable heuristic the plan's deletion table names. This guesses structure
the collection never declared (Boxed Cat violation): `RingBuffer` has
`read`/`write`, no `len`, and no `op Count`. Per the tri-partite rule (fields =
user-declared; reflection = compiler-known non-operational metadata; intrinsics
= operations), the element count of a collection is an OPERATION — its home is
the `Count#` intrinsic (which dispatches to the type's declared `op Count`),
not a reflection target. **`.^Size` (runtime) on a collection is deleted** (per
§10.4); `.^^Size` (compile-time) keeps the vector shape. Uses migrate to
`Count#(...)`. This supersedes the earlier "route `.^Size` through `op Count`"
sketch (the previous version of this section) — `Count#` is the uniform form
(§6b).

The deletion-table row `runtime `.^Size` `name == "len"` slot heuristic`
(`docs/architecture/iterable-protocol.md:239`) is resolved by this section.

## 6b. The Universal Operation Language — every op has a symbol, an intrinsic, and a UFCS method form

**2026-08-14 (user decision: unify the op/intrinsic system).** An operation is
declared ONCE as an op member. It has three invocation surfaces, all resolving
the same dispatch:

1. **Symbol** (optional): `a + b`, `c[i]`, `c <- x`, `foreach x in c`.
2. **Intrinsic** (always): `OpName#(a, b)` — the uniform, non-symbolic spelling.
3. **UFCS method form** (always): `a.OpName#(b)` — Uniform Function Call Syntax,
   documented at `learn-briev/00a-base-design.md:149` but only partially
   implemented today.

> **The rule:** `OpName#` is recognized for ANY disclosed operation identity
> (the `operation_identities` vocab, `vocab.rs:263-267`). `a.OpName#(b)`
> desugars to `OpName#(a, b)` when no member literally named `OpName#` exists
> (member wins, then UFCS). Symbols are sugar over the same dispatch. Metadata
> that is compiler-known but non-operational is reflection.

**Precedent already in the tree:** `Add#`/`Sub#`/`Eq#`/`BitAnd#` are the
intrinsic forms of the symbolic ops (`intrinsic_signatures.rs:44-66`); the
interpreter already routes `a.Add#(b)` → `execute_intrinsic` (`eval.rs:531`).
The gaps: the collection ops have op members but no `Op#` forms; the
typechecker (`resolve_method_call`, `mod.rs:3319`) and codegen
(`emit_method_call`, `emit_expr.rs:2367`) do not strip `#` in member lookup and
have no UFCS fallback to plain functions.

### Work items

1. **Typechecker `resolve_method_call` (mod.rs:3295): full UFCS priority.** Strip
   `#` in member lookup; if no member, fall back to the op identity / plain
   function (`fn_return_types`) with the receiver prepended as arg[0]. Member
   wins, then UFCS.
2. **Typechecker `infer_call` (mod.rs:1250): generative `OpName#(a, b)`.** Unknown
   `PascalCase#`: strip `#`, check `operation_identities`; if a disclosed op,
   validate arg[0]'s type declares it, infer return from the op member
   (reuse `resolve_element_type` substitution). Clean error otherwise.
3. **Codegen `emit_method_call` (emit_expr.rs:2367): UFCS fallback.** `#`-strip in
   member lookup; if no member, emit the top-level function (or op dispatch)
   with the receiver as arg[0].
4. **Codegen `emit_intrinsic_call` (intrinsics.rs:134): generative op dispatch.**
   After `template_for_op` misses: bare name in `operation_identities` →
   `emit_method_call(arg0, op_name, rest)`. Makes `At#`, `Count#`, `InsertAt#`,
   `Iter#`… work automatically.
5. **Vocab (vocab.rs:263-267): add the missing iterable ops** `Count`, `Iter`,
   `Step`, `IsEnd`, `Current` (absent today — a vocab gap).
6. **Signatures (intrinsic_signatures.rs):** explicit for the load-bearing forms
   (`At#`, `Count#`, `Slice#`, `InsertAt#`, `ExtractFrom#`) for precise arity
   errors; the rest ride the generative path.
7. **Interpreter parity:** the method form works (`eval.rs:531`); add the prefix
   `OpName#` dispatch for the new ops.
8. **Stdlib migration:** `.Count()`/`.At()` → `Count#(...)`/`At#(...)` so shipped
   code models the rule (`hashmap.bv`, `hashset.bv`, `iterator.bv`,
   `skiplist.bv`). `c[i]`/`foreach` sugar stays.
9. **`.^Size` cleanup** (§6a): `Count#` is the count intrinsic; delete the
   runtime `len` heuristic.
10. **Docs:** SPEC §11.4/§15.2 three-surface rule; `learn-briev` UFCS claim made
    real; arch `iterable-protocol.md` surface table gains intrinsic + method
    columns; plan §2/§16 addendum amending "no collection intrinsics" (the
    ruling targeted engine/layout intrinsics like `RingPush#`; protocol-
    dispatched `OpName#` calling the type's own declared ops is the intended
    uniform surface — same governance as `Add#`/`CharCount#`).

**Design decisions:** `c[i]` stays the idiomatic indexed-read sugar (`At#` is
the explicit form). Arithmetic `Op#` signatures stay (they work). UFCS member
wins over top-level function. This is a superset of §6a's `Count#` need — §6a
deletes `.^Size`, §6b provides `Count#` as its replacement.

## 7. Commit sequence

1. **docs**: Boxed Cat disambiguation + docs-reconciliation (all §5) — commit.
2. **String unification** (§2) — commit.
3. **`.^^Element`** (§3) — commit.
4. **Boundary: `Abs#` + bit intrinsics** (§4) — commit.
5. **Slice 5 remainder + scoped slice 6** (§6) — commit.
6. **Universal Operation Language** (§6b): typechecker UFCS + generative
   `OpName#` (items 1-2), codegen UFCS + generative dispatch (items 3-4), vocab
   (item 5), signatures (item 6), interpreter parity (item 7) — commit.
7. **Stdlib migration + `.^Size` deletion** (§6b item 8, §6a): stdlib
   `.Count()`/`.At()` → `Count#`/`At#`; runtime `.^Size` `len` heuristic
   deleted — commit.

Each: `cargo test --lib` green, `cargo build` no new warnings, Praetor on
changed dirs, docs in the same commit (rules 8, 3, 12).

## 8. Follow-up (separate)

- Ringbuf A/B rebuild under Performance Recovery Protocol with a full baseline
  table (rule 11); `queue_drain_idio`/`float_math` gates.
- `.^Absolute` unknown-target error after the deprecation release.
- Generic `defn f<T>` dispatch (the generic-function layer) — now that
  `Count#`/`At#`/`OpName#` are the uniform surface, generic functions over any
  collection become expressible; separate plan.
