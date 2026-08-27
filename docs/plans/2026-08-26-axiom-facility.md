# Axiom Facility — Declared Authority for Unprovable-But-True

**Date:** 2026-08-26
**Status plan:** execute slices c1–c6, one commit each.

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

- **c1** This doc + `config/axioms.dbvl` + `AxiomPolicy` loader
  (config_tuning.rs clone of load_ir_lowering).
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
- **c6** SPEC §8.7 rewritten with grammar + semantics per class + future
  directions paragraph; tracker bug#1 closed; BUGS.md entry rewritten as
  RESOLVED(PARTIAL→FULL for skip hole); milestone log.

SPEC updates ride the same commit as each core feature (owner directive):
c2 syntax sketch in §8.7 if visible earlier, full rewrite in c6, lemma
consumer documented in c5 if landed.

## Non-goals

- Mandatory justification strings (config-throttle chosen instead).
- New intrinsics (Bitcast#) — Posit32 goes frgn-free? No: codecs live in .bv;
  bit surgery via existing Int ops where expressible, else edge stays axiomatic.
- Optimizer-exploiting lemmas beyond the gated consumer above.

## Undo

Per-commit revert; all AST fields default-false/empty so old programs
(including the compiler's dogfood source) behave byte-identically.
