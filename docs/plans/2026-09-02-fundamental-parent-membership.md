# Plan: fundamental-parent membership — `type Float16 : Float` suffices, no `#Float`

**2026-09-02.** Successor to `2026-09-01-float16-float-join-and-purge.md` and the
first concrete step of the Phase B de-hashtag purge.

## Thesis

`Float` is a fundamental: seeded in every universe (the typechecker's fresh
primordial universe AND the backend-primed universe) with `Cast.Float`. A
type whose parent chain reaches a fundamental derives its protocol category
from that fundamental — an explicit `#Cat` hashword restatement is
redundant. Explicit declarations keep working (precedence), but are no
longer REQUIRED for fundamental joins.

**Companion decision (user, this session): NO new width-suffixed arithmetic
intrinsics** (`FAdd16#` etc. rejected). The operand type carries the
precision; the existing shape-inferred `Add#`/`Sub#`/`Mul#` intrinsics plus
shape-driven lowering serve every current and future width. The protocol
table's width-suffixed labels (`FAddF64#`, `AddI64#`, …) never reach
codegen (zero `src/backend` consumption — verified) and are typecheck-era
vestiges; sweeping them is out of scope here.

## Facts established (code reading, this session)

1. **Backend already works bare.** `register_types.rs:257` sets `rt.base`
   from the BARE PARENT first, protocol only as fallback — `Float16 : Float,
   #Float` registers `base = "Float"` today. `type_to_protocol`
   (graph.rs:1049) parses bare `Float` → category `Float`. Every
   `is_protocol_member(t, "#Float")` site, gemm f16-shape routing, and
   `resolve_spirv_shape` already resolve without the hashword. Removing
   `, #Float` changes nothing backend-side.
2. **Typechecker is the only `#Float` consumer, and it cannot derive.**
   `declared_protocol_of` (mod.rs:242) walks `type_parents` but reads only
   `type_protocols` — never the universe. `Float16 → Float` dead-ends
   because `Float` (primordial) has no `type_protocols` entry, though the
   fresh universe DOES contain it with `Cast.Float`
   (type_universe/mod.rs:178).
3. **Latent hole in the f16 literal gate.** mod.rs:2977 consults
   `casting_graph.type_to_protocol(ctx.universe, t)` — but the typecheck
   universe is fresh primordials-only (compile.rs:284), so Float16 resolves
   to `("Data","")` and the gate cannot fire. Worse, `float_literal_fits`
   reads `bits` from the same universe → `None` → admits ANY f32 literal —
   the precision contract is unenforced there. The only f16 test is the
   `f32_fits_f16` math table; no end-to-end literal test exists.
4. **`emit_binary_op` name-matches** (emit_expr.rs:5397): float-ness via
   `ty == Type::float()` — a Rule-19 violation site. Float16 falls into the
   integer branch today; `fadd half` requires migrating this to protocol
   membership + `resolve_llvm_type` (casting graph already maps
   Float-category bits=16 → `half`, graph.rs:1242).
5. **`FAdd16`/`FSub16`/`FMul16` exist only as declarations** in float.bv —
   no intrinsic registration, no frgn. Typecheck-only fiction.
6. **Interpreter f16 arithmetic is exact via f32 compute**: the sum of two
   f16 values is exactly representable in f32 (≤13 significand bits
   needed), so convert→compute→binary16-round implements correct IEEE f16
   add/sub/mul. Division: documented as f32-quotient rounding (one final
   rounding — standard software-half practice).

## Track A — membership derivation

- **A0 Baseline**: `cargo test --lib` green at HEAD; record whether
  `brievc build examples/gpu/gemm_h.abv` typechecks today (expect: fails at
  the literal gate — fixes framing, not the plan).
- **A1 Shared table**: `pub const CAST_CATEGORY_PROPS: &[(&str, &str)]` in
  `type_universe` (Float→UInt→Int→String→Bool→Char→Blob order preserved).
  Migrate the two inline copies: `operators.rs::protocol_category` (136-144)
  and `graph.rs::type_to_protocol` (1020-1037). Rule 17/18: the pattern is
  now 3× (this adds the third).
- **A2 Typechecker derivation**: `declared_category_of(name) -> Option<String>`
  on `TypecheckContext` — walk `type_protocols` (explicit wins, variant
  stripped) then per-ancestor universe `Cast.*` check (fundamentals ARE in
  the fresh universe; user typedefs continue the walk via `type_parents`)
  then `type_parents`. Migrate `operand_implements_protocol` (mod.rs:441)
  and `protocol_binding_for` (mod.rs:373); retire `declared_protocol_of`
  (grep-sweep for stragglers).
- **A3 Literal gate**: mod.rs:2977 gates on
  `operand_implements_protocol(t, "#Float")` (derives through parents).
  Add `type_max_bits: HashMap<String, u64>` collected in `check_program`
  from `spec MaxBits`/`bits`/`maxbits` metadata (same pattern as
  `type_parents`); `float_literal_fits` falls back to it when the universe
  lacks the type — `3.14159` into Float16 rejected again.
- **A4 Casting-graph loop**: `type_to_protocol` base fallback loops
  `universe.get(base)` until `Cast.*` hit, accepted category, or Data —
  multi-level chains (`MyHalf : Float16 : Float`) resolve instead of
  collapsing to Data.
- **A5 Glue/FFI derivation**: `glue/export.rs::build_type_protocols` +
  `boundary_marshalling.rs` resolve bare-parent chains through the same
  category derivation (parent walk via `td.parent`, universe check each
  step). Decision: fix now (user-approved), not document-and-defer.
- **A6 float.bv**: `type Float16 : Float { … }` — `#Float` dropped;
  `op Add(Float16): Add#(#Lh,#Rh)` etc. — **`FAdd16`/`FSub16`/`FMul16`
  deleted**, rebound to the existing shape-inferred intrinsics.

## Track B — scalar f16 arithmetic

- **B8 Binop migration**: `emit_binary_op` float detection →
  `is_protocol_member(ty, "#Float")` + width via `resolve_llvm_type` →
  `fadd half`; `fast` flag from membership; result type carries l.ty.
  SPIR-V shape path verified (already metadata-driven).
- **B9 Interpreter arm**: f16 add/sub/mul/div via exact f16↔f32 convert +
  compute + binary16 round; the `f32_fits_f16` codec already exists —
  extend to full encode/decode helpers where needed.
- **B10 Membership-derived authorization**: if `protocol_binding_for`
  authorizes Float16 `+` with zero declared ops, the explicit `op` lines
  shrink to documentation — test decides, contract-first.

## Tests

- Derived membership: `operand_implements_protocol(Custom("Float16"),
  "#Float")` via parents, fresh universe.
- Literal admission: `let x: Float16 = 0.0;` passes; `3.14159` rejected —
  end-to-end through `check_program` with the stdlib declaration.
- Backend: `type_to_protocol` → `("Float","")` + f16 `resolve_spirv_shape`
  from bare parent; multi-level chain.
- Explicit-hashword precedence intact (`CDouble`, `RstF64`, `PyInt`).
- `Float4 : Bits` gains no unintended membership.
- Binop IR: `fadd half` emission; Float32/Float64 tiers bit-identical.
- Interpreter f16 exactness vectors (0.0+0.0, 1.5+2.25, overflow→inf,
  subnormal, round-to-nearest-even boundaries).

## Gates per commit

`cargo test --lib` · Praetor on changed dirs · spirv-val paths untouched ·
gemm_h.abv typechecks end-to-end at the end of Track A, typechecks AND
lowers at the end of Track B (execution remains blocked by the f16 tensor
device fault — separate hunt, `spirv_coopmat` stays off).

## Docs (same-commit updates)

This plan · append result to `2026-09-01-float16-float-join-and-purge.md` ·
`docs/architecture/hash-words.md` (membership derives from fundamental
parent) · SPEC relationship grammar + arithmetic ·
`docs/architecture/intrinsics-vs-stdlib.md` (Add# shape-driven note).

## Out of scope, recorded

Protocol-table label sweep (`AddI64#`→`Add#` style, ~25 entries) · f16
tensor device-fault hunt (vendor tooling needed) · f16 string/format paths.
