# Plan — CIRCT: seq.firmem memory lowering with loud-default policy

**Date:** 2026-08-25
**Status:** COMPLETE (all steps landed; see commits 16a14178..HEAD)
**Predecessor:** 2026-08-23-circt-toolchain-validation.md (COMPLETE — register-file arrays landed, sim-parity harness live)
**User decisions (this session):**
- Keywords `mem` / `reg` (prefix on `let`, mirroring `vol let` / `out let`).
- ONE aggregated diagnostic per compile listing every default-lowered array needing disambiguation; explicit pins silence it.
- Default threshold 64, calibrated empirically; lives in config, never hardcoded magic.

## 1. Problem

Plan §3.6 shipped bounded state arrays as REGISTER FILES only (one
`seq.firreg` per lane + mux/comparator trees). Correct and proven, but area
blows up: N flip-flops plus O(N) wiring per access port. Real silicon wants
a RAM macro past the crossover. The pinned CIRCT source carries
`seq.firmem` + port ops (`SeqOps.td:522`, `FirMemReadOp/FirMemWriteOp`) and
the SeqToSV pass has `LowerFirMem.cpp` — but presence in source ≠ present in
the built binary (`seq.assert` lesson), so everything gates on Step 0 probes.

## 2. Semantics contract (non-negotiable)

Latency-0 combinational reads ONLY in v1. Our FSM model reads cycle-start
state combinationally (guards, body expressions); `readLatency=1` shifts
observable behavior one cycle vs the interpreter and breaks sim parity.
Sync-read BRAM mapping is future work requiring pipeline-aware emission.

Writes land at the clock edge — exactly our existing commit-at-edge model,
with the §3.4 gate folded into the write ENABLE (`we = pre_ok ∧ fire`):
refusal ⇒ enable low ⇒ state holds. No per-lane muxes needed.

Observable behavior of a mem-lowered array must be IDENTICAL to its
register-file twin (same EXPECT sequences) under the supported surface:
single-writer arrays, no post-condition element references, nonzero-init
only if the build accepts inline init attributes (probed).

## 3. Policy engine (frontend)

Decision per state array `buf` with `Vector(elem, [Anonymous(n)])`:

```
explicit annotation wins        Annotation{name:"mem"}|{name:"reg"}
  └─ validated: "mem" + post-condition reads elements of buf ⇒
       capability error (element obligations impossible on a macro)
  └─ validated: ≥2 txns WRITE buf and "mem" ⇒ capability error
       (suggest `reg let`)
no annotation ⇒ default policy:
  n >= firmem_min_depth (config, default 64)
    AND no post-condition element references
    AND ≤1 writer txn
    AND port sites <= firmem_max_ports (config, default 4)
    AND (init all-zero OR inline-init probe passed)
  ⇒ FirmMem ; otherwise RegFile
n == 0 / non-constant dim ⇒ capability error (unchanged)
```

Port accounting: each `Index` occurrence in txn bodies and PRE/GUARD
conditions is one port site (distinct address each cycle). Post-condition
references are banned on mem arrays (committed next-value shadow trick is
register-only). Body read+write of one txn share nothing in v1 (simple,
sound); the budget catches pathological programs with a named fix.

## 4. Syntax (user-approved)

```briev
mem let buf: Int[64] = [...];   // pin memory macro
reg let buf: Int[64] = [...];   // pin register file
let buf: Int[64] = [...];       // default policy — contributes to THE note
```

- Lexer: `Reg` token already exists (reserved, unused — `src/lexer.rs:179`);
  add `#[token("mem")] Mem`. Reserve "mem" in the cannot-be-a-name list.
- Parser: top-level arms mirroring `Some(Token::Out) ... Some(Token::Let)`
  (`src/parser/definitions.rs:134ff`) and statement-level arms mirroring
  `Token::Vol` (`src/parser/statements.rs:17ff`). Annotation `{name:"mem"
  |"reg", value=None}` pushed onto the Let's modifiers. Both trees flow to
  the backend via the existing items traversal (integration point: confirm
  how top-level `let`s reach `TopLevel`/`StateDecl` and carry modifiers —
  researched at implementation start).

Delimiter-contract note: this is deliberately a DECLARATION modifier, not
type-level `<mem>` — storage policy is per-variable, not per-type; `<>`
stays type-level specialization (`Int<8>` keeps its meaning).

## 5. The one diagnostic (user-approved form)

Single aggregated NOTE per compile, house what/why/fix style:

```
note: 3 state arrays follow the default array-lowering policy — prefix
      'mem let' or 'reg let' to disambiguate (and silence this note):
  buf   64 x i8  -> seq.firmem macro   [why: depth >= 64]
  tab  128 x i1  -> register file      [why: a postcondition reads elements]
  lut   16 x i16 -> register file      [why: depth < 64]
```

Fires only when ≥1 array took the DEFAULT path; fully explicit programs are
silent. Surfaced through the existing compiler notice channel
(researched at implementation start — likely the same path as the
normalizer warnings seen on stderr).

## 6. Config tunables

`config/ir-lowering.dbvl` rows (format matched to that file's conventions):

```
circt.firmem_min_depth: 64;
circt.firmem_max_ports: 4;
```

Plumbed via a `CirctBackend::with_mem_policy(min_depth, max_ports)` setter
called by the pipeline after config load (mirrors `with_universe`);
unit-test default = the constants above.

## 7. Emission (backend)

New `MemPlan { var, elem_ty: String, depth: usize }` collected during the
flattening loop instead of lanes. Per plan:

```
%<var> = seq.firmem "<var>" 0, 1, <ruw>, <wuw> : !seq.firmem<iW x D>
%<var>_rp<j> = seq.firmem.read_port %<var>, %clock, %addr_j : ...
%<var>_wp    = seq.firmem.write_port %<var>, %clock, %addr, %data, %we
```

(exact custom syntax locked by Step 0 probes; iterate against circt-opt
parse errors — fast feedback.)

- Reads: `emit_expr` Index arm gains the mem branch — address wire from the
  index expression, fresh read port per site (pre-counted), data feeds the
  enclosing comb chain. Width = elem width (never widened — sized-scalar
  lesson applied here too).
- Writes: Assign/Index arm mem branch — `we = pre_ok` (commit gate folded);
  refusal holds the macro. No pending-map interaction (nothing combinational
  to repoint); halt/check untouched because posts-on-elements are banned.
- Init: literals → inline init attr if probed working; otherwise policy
  forces RegFile for nonzero-init arrays (gate in §3).
- Structural tests assert firmem presence/absence per pin + note text;
  round-trip parse test extends the existing toolchain-gated test.

## 8. Validation matrix

| Case | Expectation |
|---|---|
| `Int[64]` bare | firmem + aggregated note names it |
| `mem let Int[64]` | firmem, silent |
| `reg let Int[64]` | lanes (64 regs!), silent |
| `Int[16]` bare | lanes, note lists it |
| `Int[64]` + post reads element | lanes, note explains why |
| two writers + `mem` | capability error |
| `Int<8>[256]` bare | firmem, i8-wide macro |
| all fixtures re-run | five existing fixtures byte-identical behavior |

Sim parity: new `big.bv` (Int[64]) fixture + gen.py model — sequences
identical in kind to register twin (single writer, no element posts).
Vivado A/B: `reg let` vs `mem let` builds synthesized on xck26; utilization
must show FF count collapsing and RAM (LUTRAM) rising for the mem build;
recorded numbers calibrate `firmem_min_depth`. If crossover evidence
contradicts 64, retune config + plan doc in the SAME commit.

## 9. Docs (same-commit rules)

- `docs/architecture/backend-contracts.md`: memory-macro section (semantics
  contract §2, policy, keywords, diagnostic).
- `spec/SPEC.md`: `mem let` / `reg let` grammar (strategy-keyword family),
  policy sentence.
- Tutorial (`learn-briev/`): hardware chapter mentions the pins + the note.
- This plan: status flips per step; timestamped, never retro-edited.
- BUGS.md: any toolchain findings (init attr gaps etc.).

## Steps (execution order)

0. **Probe** — DONE 2026-08-25. GO, with pipeline adaptation.
   Syntax locked (all verified against installed circt-opt):
   - `%m = seq.firmem 0, 1, old, undefined : !seq.firmem<D x W>`
     (attrs are BARE enum names; no name string — ImplicitSSAName).
   - `%r = seq.firmem.read_port %m[%a], clock %clk : T` (+ optional `enable %e`);
     data width W; ADDRESS WIDTH = ceil(log2(D)) — verifier-enforced
     (i6 for depth 64); our i64 index wires need `comb.extract` truncation.
   - `seq.firmem.write_port %m[%a] = %d, clock %clk enable %we : T`
     (NO result value).
   - `--lower-seq-to-sv` lowers to `hw.instance` of a
     `hw.module.generated @<name>_<D>x<W>, @FIRRTLMem(...)` black box;
     instance wiring carries our enable-gating model verbatim (W0_en=%we).
   - **UPSTREAM LIMITATION (BUGS.md entry)**: this build's ExportVerilog
     hard-rejects `hw.module.generated` ("unknown operation" in
     gatherFiles) then asserts on the instance — FIRRTL_Memory body
     emission lives on firtool's side, absent from standalone circt-opt.
   - **Pipeline adaptation (validated end-to-end in probes)**:
     lower → TEXTUAL PATCH `generated → hw.module.extern` (same ports;
     re-parse verified) → export top.sv (instance by name) → brievc emits
     a COMPANION `<name>_<D>x<W>.sv` reference implementation
     (distributed-RAM style: posedge gated write, combinational read)
     matched port-for-port from the patched signature. verilator lint of
     the pair PASSES. Harness gains the patch+companion stage for mem
     programs only; five existing fixtures untouched.
   - Inline-init attr: NOT probed further — policy already forces RegFile
     for nonzero-init arrays (§3); companion is zero-init. Deferred.
1. **Lexer/parser**: Mem token, reservation, top-level + statement prefix
   arms, tests.
2. **Policy engine**: array-ref collection, decision fn, unit tests.
3. **Config**: dbvl rows + loader + setter + plumbing.
4. **Diagnostic**: aggregation + emission point + test.
5. **Emitter**: MemPlan branch, read/write port ops, budget enforcement,
   structural tests.
6. **Fixtures/harness**: big.bv + gen.py + full harness green; five old
   fixtures unchanged.
7. **Vivado A/B** — DONE 2026-08-25 (xck26-sfvc784-2LV-c, both PASS):
   | metric            | mem macro | reg file (64 lanes) |
   |-------------------|-----------|---------------------|
   | CLB Registers     | 17        | 522                 |
   | LUT as Logic      | 24        | 242                 |
   | LUT as Memory     | 10        | 0                   |
   ~30× fewer flip-flops via the macro; threshold 64 confirmed.
8. **Docs** (§9) + sync main worktree.

## Undo paths

- Keywords/policy/diagnostic: remove arms + rows (annotations ignored
  harmlessly by older backends — additive).
- firmem branch: delete MemPlan arm; register-file path untouched throughout
  (additive-only rule honored).
