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

## C. Compiler-in-Briev pass migration

`lib/compiler/main.bv` fails to parse: 11 "expected identifier, found '}'"
errors — legacy syntax (old `defn ... -> T { term ...; }` return style
without `endprogram`, `output.to_string()` method calls that no longer
exist, flipped-form initializers). The pass imports std/io, std/string,
std/string_builder, std/collections, std/option — all now working.

**Steps:**
1. Parse each legacy construct against the CURRENT grammar (learn-briev/,
   spec/SPEC.md) — migrate, don't paper over.
2. Walk the import chain: main.bv → ast.bv, lexer.bv, parser.bv,
   typechecker.bv, proof_engine.bv, token.bv, call_graph.bv, range.bv,
   needs_state.bv, reader.bv, backends/.
3. Goal: `brievc check lib/compiler/main.bv` passes type-checking.
4. If the pass reaches codegen, link + run a smoke (the pass's own
   lexer/parser on a small input).

Scope guard: this is a LARGE legacy surface. The plan's check gate is
"type-checks cleanly"; codegen-and-runs is a stretch goal — if the surface
is too large for one session, commit the parse+typecheck wins and log the
residual in BUGS.md.

---

## Docs

- Update BUGS.md statuses (B) in the same commit as each fix.
- This plan supersedes the "deferred" note about the P1.5 collapse gap.
- Any syntax migration that changes the pass's semantics is documented in
  the commit body.