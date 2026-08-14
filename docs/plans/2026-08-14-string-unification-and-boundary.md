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

## 7. Commit sequence

1. **docs**: Boxed Cat disambiguation + docs-reconciliation (all §5) — commit.
2. **String unification** (§2) — commit.
3. **`.^^Element`** (§3) — commit.
4. **Boundary: `Abs#` + bit intrinsics** (§4) — commit.
5. **Slice 5 remainder + scoped slice 6** (§6) — commit.

Each: `cargo test --lib` green, `cargo build` no new warnings, Praetor on
changed dirs, docs in the same commit (rules 8, 3, 12).

## 8. Follow-up (separate)

- Ringbuf A/B rebuild under Performance Recovery Protocol with a full baseline
  table (rule 11); `queue_drain_idio`/`float_math` gates.
- `.^Absolute` unknown-target error after the deprecation release.
