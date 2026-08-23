# CIRCT backend — toolchain-validated hardware emission

**Date:** 2026-08-23
**Status:** active
**Sequencing:** parallel branch; requires Plan 0
(`2026-08-23-backend-scaffolding-foundation.md`) merged first (universe +
analysis via `BackendContext`; capability matrix). Work confined to
`src/backend/circt/`, `tools/` (install script landed in Plan 0), own
tests, own doc sections.

## Charter

`.cbv` compiles to MLIR that **real CIRCT accepts** (`circt-opt` parses,
`circt-translate --export-verilog` succeeds) and whose simulated behavior
matches the interpreter on the supported subset. Contracts carry semantic
load in hardware: guards become checked conditions, not comments.

## Baseline state (2026-08-23)

| Aspect | State |
|--------|-------|
| `circt/mod.rs` | 1018 lines, emits hw/comb/seq text; 17 string-match tests |
| Typing | `mlir_type()` name-matches `"Bool"`/`"Int"`/… ×16 (:148-174) — rule 19 violation; backend gets no universe (`generate(items)`) |
| Soundness | `unreachable!()` at :418; silent drops `_ => None` :272,:477,:543; `"0"` fallbacks |
| Ops | Emits **nonexistent ops**: `comb.sin/pow/sqrt/rev` (comb dialect has none of these); malformed instance port syntax :451,:605 |
| Determinism | `cell_defs.values()` HashMap iteration orders cell modules (:135) |
| Contracts | Guards collapse toward trivially-true emission (no assertion lowering) |
| Validation | None — no test invokes circt-opt; tools absent locally until Plan 0.6 installs them |

## Work items

### 3.1 Universe-driven typing (rule 19 rewrite)

`mlir_type()` derives widths from the TypeUniverse/CastingGraph via Plan 0's
`BackendContext`: protocol-category + metadata → bit width. Sized ints via
`Constrained(BitRange)` stay (already width-driven). Bool → i1 derived from
protocol membership, not name match. Unsupported types → capability error.
`generate(items)` grows to take the context (call site `compile.rs:1852`
updated by this branch — the ONLY shared-file touch allowed here).

### 3.2 Honest op subset

Restrict emission to ops that exist in CIRCT comb/hw/seq dialects:
add/sub/mul/divu/divs/modu/mods, shl/shru (+ signed variants where the
dialect version provides them), and/or/xor, icmp (eq/ne/lt/gt/le/ge,
signedness explicit), mux, parity, replicate, cat. Delete invented
sin/pow/sqrt/rev arms. Fix instance syntax against real hw.module/hw.instance
grammar (validate with installed circt-opt).

### 3.3 No silent drops; deterministic emission

Every `_ => None` / `"0"` fallback either becomes an implementation or a
capability error naming feature+fix. `unreachable!()` removed. All module/
cell/value iteration sorted by key (house determinism rule) — cell_defs at
:135 and any var-map walks.

### 3.4 Contracts as hardware obligations

Pre/post guards lower to comb.icmp chains feeding seq assertions
(`seq.assert`, gated on dialect availability in the pinned CIRCT build;
fallback: dedicated check output port asserted by the FSM). FSM transitions
must respect watchdog metadata where present; unsupported contract shapes →
capability error.

### 3.5 Toolchain-validated tests (replace string matching)

Test pyramid, all probe-gated on the Plan 0.6 install:
1. **Parse:** every emitted module round-trips through `circt-opt`
   (mandatory when tool present).
2. **Translate:** `circt-translate --export-verilog` succeeds.
3. **Simulate parity:** verilator (installed) runs the Verilog on tiny
   fixtures — counter, trigger→FSM handshake, comb arithmetic block,
   contract-guard firing — and observable outputs match the interpreter
   running the same `.cbv` program. This is the CIRCT benchmark corpus.
Keep a few cheap structural tests (string-level) for fast feedback, but
correctness lives in the toolchain tier.

### 3.6 State arrays / bounded collections

Bounded static state collections lower to seq memories (`seq.firmem`) or
register files per width; capability-error unbounded ones. Only after 3.1–3.5
land — additive follow-up commit within the branch.

## Documentation maintenance

- backend-strategy.md CIRCT section rewritten by this branch (architecture
  diagram updated to context-driven typing + validated pipeline).
- Rationale comments dated 2026-08-23 where invented-op/drop sites were
  replaced; each names the old behavior and why it was unsound.

## Verification

1. Emitted MLIR parses under pinned circt-opt (all fixtures).
2. Verilator sim == interpreter on fixture corpus.
3. Determinism: two compiles of same input byte-identical output.
4. `cargo test --lib` green; Praetor clean on `src/backend/circt`.
