# Axiom Facility — Declared Authority for Unprovable-But-True

**Date:** 2026-08-26 · **Updated:** 2026-08-27 (session resume)
**Status plan:** execute slices F1, c2–c6, one commit each.

## Update 2026-08-27 — session resume

The 08-26 session was interrupted twice; work landed as rescue commits
`bc1dc653` (scaffold) + `2dfc8d75` (owner's mechanical repair). Audit results:

- Landed: plan doc, AxiomPolicy/AxiomSettings/accessor in config_tuning.rs,
  loader (ConfigDb/.dbvl form), accel_probe_margin fix.
- **SPEC-first directive (owner)**: SPEC §8.7/§8.8/§10.1 formalization rides
  FIRST this session, before F1 — spec is the contract, implementation follows.
- **Config format decision (owner, resolved)**: human-editable config is
  `.dbv` (schema'd, `parse_document_quoted`); machine-generated tuning tables
  stay `.dbvl`. House rule now documented. `config/axioms.dbv` replaces the
  `.dbvl` the repair commit created; loader switches to the
  `backend/metadata.rs` pattern.
- **F1 slice (new)**: .dbv flip + loader rewrite + restore `examples/
  error-handling` (binary collateral-deleted in `bc1dc653`, blob recoverable
  from `bc1dc653^` via `git cat-file` — additive, Rule 8-safe) + reinstate
  `arena_min_budget` doc comment (rationale-comment loss) + `axioms_dbv_
  parses` test.

## Problem

Briev trusts invisibly today: `frgn` signatures, Z3 `Unknown → Ok`
(proof_engine/mod.rs:113), no-Z3 heuristic passes, watchdog bounds that parse
but never discharge. The protocol round-trip machinery silently skips proofs
when bodies are missing (protocol_graph.rs:143,156,188). SPEC §8.7 already
reserves the concept — "missing proof evidence is an error unless the edge is
visibly declared as a trusted foreign/intrinsic axiom" — but nothing implements
it. The fix is NOT more trust; it is converting silent faith into DECLARED,
counted, config-throttled, reportable authority.

## Decisions (locked with owner)

1. **Generalize immediately**: three authority classes in this session.
2. **Syntax = contextual keyword `axiom`**, not punctuation (`~` collides with
   the destructive-arrow family; `:=`/`:~` hide a proof-skipping decision in
   near-miss glyphs). House precedent: box/spill contextual markers, seq/vol
   prefix modifiers. Recognized only before CastTo(/CastFrom(, defn, txn,
   node, op; plain identifier everywhere else, tested both directions.
3. **Config throttle**: `config/axioms.dbvl`, `policy: allow|warn|deny`
   (default allow + info ledger line per site), `lemma_properties:` closed
   vocabulary. Deny = hard error naming each site. .s report always renders
   the full axiom ledger regardless of policy.
4. **Teeth survive**: tautology gate fires even under authority ([true][true]
   still rejected); pre-conditions stay PROVEN on `axiom defn`; only post flips
   to authority; binding must still exist on an axiomatic cast edge.
5. **Prove-first stdlib**: ASCII pair written as REAL provable defns; UTF16
   attempted symbolically; only genuinely prover-opaque edges axiomatic
   (expected: Posit32 pair).
6. **Lemmas evidence-gated**: storage + vocab validation land; folded-increment
   commutation consumer wires ONLY if a hand-transform A/B experiment holds
   (Rule 20); otherwise ships unexercised-not-exploiting and says so.
7. **Gate flip LAST** (c3): missing-body arm becomes hard error only after the
   stdlib edges are resolvable; message names both exits.

## Grammar

```briev
// Class 1 — cast-edge authority (proto body)
axiom CastTo(#Float<IEEE754>)   = Posit32_to_IEEE754(#Lh);
axiom CastFrom(#Float<IEEE754>) = IEEE754_to_Posit32(#Lh);

// Class 2 — contract-post authority (declaration prefix)
axiom defn codec(x: Int) -> Int [x >= 0][result <= x * 2];

// Class 3 — op lemma (type-body op binding / impl)
op Add: func(#Lh, #Rh) [commutative];
```

Pair dissolution: either-side-axiomatic discharges the roundtrip obligation;
recorded "axiom-discharged" in output.

## Research anchors (verified 2026-08-26)

- Contextual keyword template + both-direction tests: expressions.rs:575-610,
  tests :1250-1303
- Prefix-modifier template (lookahead pair): definitions.rs:69-83
- parse_contract explicit flag site: definitions.rs:1540-1548
- prove_contract behavior tree: proof_engine/mod.rs:20-128
- Skip sites: protocol_graph.rs:143,156,188 (zero tests assert them)
- Binding-exists validation precedes proofs: compile.rs:346-357
- Existing builtin-only commutativity recognition: transition_graph.rs:557-567
- BEAST Contract serde: beast/serialize.rs:125, deserialize.rs:174-270
- Config loader pattern: config_tuning.rs:98-224 (.dbvl, LazyLock)
- Strict report: strict.rs:61-103, call compile.rs:230-244
- Warnings channel: backend.warnings() printed at compile.rs:1167-1169

## Slices → commits

- **SPEC** §8.7 (cast-edge axiom grammar), §8.8 (op lemma brackets), §10.1
  (contract-post authority), §3.2 (enforcement dial + ledger) — FIRST, this
  session, before any parser work (owner directive: spec is the contract).
- **F1** `config/axioms.dbv` + `parse_document_quoted` loader + `examples/
  error-handling` restore + doc-comment repairs + `axioms_dbv_parses` test.
- **c2** AST: `CastEdge.trusted_axiom: bool`, `Contract.post_authority: bool`,
  `OperatorDef.trusted_lemmas: Vec<String>` (+ OperatorBinding passthrough);
  BEAST serialize/deserialize; display; parser recognition at 5 anchors +
  fallback tests (identifier use elsewhere must keep working).
- **c3** Class-1: verifiers discharge axiomatic pairs; ascii_to_utf8/
  utf8_to_ascii real defns in protocols.bv + UTF16 symbolic attempt; gate flip
  (missing-body → error w/ two-exit fix text); strict-report "declared
  authorities" section; policy warn/deny enforcement tests.
- **c4** Class-2: parse_contract sets post_authority; prove_contract early-
  return AFTER tautology gate; parity test proving post still feeds
  extract_bound_from_postcondition/range consumers.
- **c5** Class-3: lemma vocab validation against config list; EXPERIMENT DOC
  before any consumer; wire commutation into increment-detection iff held.
- **c6** Tracker bug#1 closed; BUGS.md entry rewritten as RESOLVED (skip hole
  closed); milestone log. (SPEC formalization already landed SPEC-first.)

## Non-goals

- Mandatory justification strings (config-throttle chosen instead).
- New intrinsics (Bitcast#) — Posit32 goes frgn-free? No: codecs live in .bv;
  bit surgery via existing Int ops where expressible, else edge stays axiomatic.
- Optimizer-exploiting lemmas beyond the gated consumer above.

## Undo

Per-commit revert; all AST fields default-false/empty so old programs
(including the compiler's dogfood source) behave byte-identically.
