# Plan: Float16 joins Float (bare) + the de-hashtag purge — then the tensor run

**2026-09-01.** Successor to `2026-09-01-m2-tensor-cores.md` (tensor emitter
built, unit-gated; front door blocked on the Float16 typecheck). Two legs in
one document because they share a principle: **fundamental and protocol names
lose their `#`; hashwords as a mechanism stay.**

## The design decision (settled)

`Float16` joins the **Float** category through the existing subtype mechanism:

```briev
type Float16 : Float {
    spec MaxBits: 16;
    spec Alignment: 2;
    !> tbaa: "FloatNode";
    op Add: FAdd16(#Lh, #Rh);
    op Sub: FSub16(#Lh, #Rh);
    op Mul: FMul16(#Lh, #Rh);
};
```

- Bare parent `Float` (no `#`) — post-hashtag spelling. `td.parent` as
  `Expr::Identifier("Float")` registers `base: "Float"`
  (`register_types.rs`), and `type_to_protocol`'s base chain resolves
  `("Float", "")` — no new mechanism, no annotation.
- The width ladder reads Float16's OWN `spec MaxBits: 16` → the f16 shape
  (the M2.2 kernel-surface extension).
- Hashword forms keep parsing as **silent aliases** (indefinitely — zero
  fixture churn; a deprecation warning is a possible later phase, not now).
- The `#` STAYS on: `#Lh`/`#Rh` op placeholders, `#Link<name>` foreign
  targets, `#System`, the intrinsic suffix (`Sqrt#`). Only fundamental and
  protocol category NAMES de-hash.

## Phase A — the tensor rung

- **P0 — GEMV same-GPU pin** (~10 min). `CUDA_VISIBLE_DEVICES=0` on the
  ggml gemv anchor; closes the last honesty gap on the M1-beats-ggml claim
  (this box's "20GB pair" = 3060 + 1070 Ti).
- **P1 — Float16 joins Float.**
  1. `lib/std/types/float.bv`: parent `Bits` → `Float` (one line).
  2. Literal admission: Float literals coerce into Float-category targets
     gated on **f16 round-trip exactness** (f32→f16→f32 reproduces the
     value; `0.0`, `1.5`, the test grid fit; arbitrary mantissas are
     rejected — matching the int width-admission philosophy). New check in
     the let-coercion path alongside `literal_fits_sized`.
  3. Arithmetic resolution: `Float16 + Float16` resolves via the Float
     category's protocol binding. Verify width-aware op selection (FAdd16
     vs FAdd) on the LLVM side; if the protocol dispatch lacks width
     selection, give float.bv's ops variant coverage
     (`op Add(#Float): FAdd16(...)` — the existing variant-coverage
     mechanism). The SPIR-V tensor kernel never emits scalar f16
     arithmetic, so the kernel side is unaffected either way.
  4. Unit tests: Float16 → Float category resolution; f16 literal
     admission table; tier routing unchanged for Float32 (shape-keyed).
- **P2 — E2E tensor run.**
  1. Revive `examples/gpu/gemm_h.abv` (content banked in the M2.2 plan;
     `let acc: Float16 = 0.0;` now typechecks via P1.2).
  2. The tensor kernel emits: f16 fragments from the SSBO, fp32-accumulate
     mma, FConvert store, LocalSize 32, X-flattened grid; spirv-val clean
     at vulkan1.3.
  3. Device: exact on f16-representable data (the test grid values are
     exact in f16) + sampled double-reference with documented f16
     tolerance; perf at 4096³ vs the ggml anchor **10.9ms / 12.6 TFLOP/s,
     same GPU** (Device 0 = the 3060).
- **P3 — records**: handoff GEMM ladder row, ledger, session-report delta.

## Phase B — the de-hashtag purge (follow-up leg, precisely scoped)

Parser gaps (the parser already strips `#` at every consumer — the
category keys are bare internally; only the parse boundary is hashworded):

1. **Proto category**: `parse_protocol_def` rejects `Type::Custom("Float")`
   today ("expected protocol category hashword") — accept the bare form
   (~6 lines). `proto Posit32: Float { ... }`.
2. **CastTo/CastFrom targets**: `CastTo(Float<IEEE754>)` — accept
   `Type::Applied` + `Type::Custom` alongside the HashWord variants
   (~10 lines).
3. **Proto variant names**: `proto String<UTF8> { ... }` — the name
   position needs the Applied form (currently hashword-only).
4. Typedef parents and op variants: already accept bare ✓ (P1 exercises
   the parent position; `op Add(Float)` variants already parse).

Then:
- **Docs sweep**: spec grammar examples (the proto positions above), the
  five spec prose hits, six learn-briev files, stdlib comments,
  `protocols.bv.archive`, glue examples — all to bare spellings. The
  canonical type spelling remains `Bit<N>` (BUGS.md "Bits tripwire").
- **Internal check strings** (`operand_implements_protocol("#Int")` ...):
  normalized opportunistically as the registry keys allow — internal, not
  user surface.
- **Test fixtures**: hashword forms keep parsing; fixtures update
  opportunistically when touched, never mass-rewritten.
- **Keep-list** (documented so nobody over-purges): `#Lh`/`#Rh` op
  placeholders, `#Link<name>` targets, `#System`, intrinsic suffix
  (`Sqrt#`), hashword *types* as parse aliases.

## Gates throughout

2024+ lib tests green · Float32 tiers bit-identical (tier routing keys on
shape) · spirv-val clean · exact correctness (f16 tolerance documented on
the tensor row only) · Praetor clean on changed files · benchmark anchors
unchanged.

## Deliverable

The .abv author writes naive matmul over `Float16` fields and gets
tensor-core GEMM — measured against ggml on the same silicon. If P2 lands
at or near the anchor, the M2 rung closes with every tier derived, typed,
and de-hashtagged.
