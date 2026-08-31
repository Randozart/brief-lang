# Post-Campaign Cleanup + Compiler-in-Briev Migration

**Date:** 2026-08-28
**Status:** Active
**Prereq:** 2026-08-28-string-abi-fix.md (complete — 6 String bugs closed)

---

## Context

The String ABI campaign closed all 6 critical bugs end-to-end. `cargo test
--lib` is 1990 pass / 1 fail. The remaining work: (A) kill the lone red
test, (B) reconcile BUGS.md with the session's resolutions, (C) the original
payoff — the compiler-in-Briev pass (`lib/compiler/`) that the whole campaign
was unblocking. It currently does not even parse (11 legacy-syntax errors).

---

## A. Fix `test_find_inverse_pairs` (the persistent red)

**Root cause:** `eval_symbolic_expr` (src/symbolic.rs) builds
`Binary(Sub, Binary(Add, x, 1), 1)` for `(x+1)−1` and never cancels it to `x`.
`prove_composition_inverse` (src/analysis/protocol_graph.rs) then falls back
to SMT, but `z3` is NOT on PATH — the fallback always fails. Consequence:
P1.5 delta-collapse (variant→variant casts proven 1-to-1) never fires for
add/sub inverse pairs — a missed optimization.

**Fix (additive):** extend the symbolic simplifier with additive cancellation:
- `(x + c) − c → x`, `(x − c) + c → x`, `x + 0 → x`, `x − 0 → x`, `x − x → 0`
- matching (recursively) at any subexpression, not just top-level.
Keep the SMT fallback as-is (z3 presence is optional; when absent the
simplifier is the proof).

**Verify:** `cargo test --lib test_find_inverse_pairs` + full lib suite.

---

## B. BUGS.md hygiene

Verify each of these "Open" entries against the committed fixes, then update:

| Entry | Session resolution | Verify |
|-------|--------------------|--------|
| "String slicing returns the whole string" | `"hello"[1:3]`=2, `mk()[1:3]`=2 end-to-end (commits a84568d5/47ae4618) | rerun both e2e |
| "Imported-module frgn String param+String return resolves to Int" | resolved via shared reader.bv declaration | check typechecker/mod.rs:1637 merge |
| "Frgn String-return heap corruption under many calls" | C-driver arity bug, not compiler | confirm note points to the arity fix |

Any entry still genuinely open stays open with an accurate note.

---

## C. Compiler-in-Briev pass migration — PROGRESS LEDGER

`lib/compiler/main.bv` failed to parse (11 errors) — the whole corpus
(`~11.6k` lines across ~19 files) predates several grammar rewrites.

### Completed (2026-08-28)

| Item | Result |
|------|--------|
| token.bv (354 lines) | Fully migrated — `brievc check OK` |
| obj-decl field commas | Stripped corpus-wide (944 changed lines; obj decls are newline-separated, struct literals keep commas) |
| guard-blocks `[expr] { }` → `when expr { }` | 761 sites corpus-wide (incl. char-literal-bracket conditions) |
| lexer.bv | PARSES clean (was 20 parse errors); only type errors remain |
| active pass (reader/needs_state/soa_reorder) | Still `OK`; soa_reorder gained the explicit `cstr_to_briev` cast |

`cargo test --lib` 1991 green; `c_driver_needs_state` passes.

### Remaining (next session)

| File | Parse errors | Legacy constructs |
|------|--------------|-------------------|
| parser.bv | 45 | ~433 `uni` |
| ast.bv | 47 | 13 `uni` + types |
| main.bv | 10 | 25 legacy |
| range.bv | 10 | 22 |
| call_graph.bv | 9 | 16 |
| typechecker.bv | 13 | 130 `uni` |
| proof_engine.bv | 40 | 136 |
| backends/* | ~80 | mix |

**Recipe (from token.bv):**
- `uni x(Pattern) = expr;` chain → `term match x { Pattern => expr, ..., _ => fallback };`
- `uni x(Pattern) = { stmts };` → statement-position match arm `Pattern => { stmts },`
- guard-blocks INSIDE match arms → `when` (the `[ ]` form is rejected there)
- `.len()` on String → `x .^Length`; `String(n)` → `n as String`; `to_string(sb)` → `sb.buffer`
- add missing imports (`std/string`, `std/option`, `std/string_builder`, `std/result`)
- Char semantics: `source[i]` indexing + `.char_at(0)` need the modern Char
  element-read (lexer.bv's remaining type errors)
- `uni` conversion is done PER-FUNCTION (token.bv style) — the multi-line
  block arms make scripted bulk conversion risky at 433 sites

---

## Docs

- Update BUGS.md statuses (B) in the same commit as each fix.
- This plan supersedes the "deferred" note about the P1.5 collapse gap.
- Any syntax migration that changes the pass's semantics is documented in
  the commit body.