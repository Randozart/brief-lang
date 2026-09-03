# 2026-09-02 — Storage images (B1) + the de-hashtag sweep

Two work items: the B1 image design settled with the user (plain protocol
name, `spec` keys, multi-dim declarations — NO new hashword), and the
cleanup the user directed: record the correct current shape in AGENTS.md
and clean the old `type X: #Y` shape out of the codebase surface.

## Part 1 — the de-hashtag sweep (do first, small and safe)

### The shape law (what the user locked in)

- Fundamentals carry NO `#` in type positions. Conformance is by plain
  parent name: `type Int32: Int { spec Bits: 32; };` (SPEC §8.9 — already
  normative; stdlib already conforms).
- Physical metadata is `spec` (§8.2 frozen keys, 2026-08-13): `spec Bits`,
  `spec Alignment`, … — never `!>` for physical keys.
- `#Category` hashwords survive ONLY as backend directives in op
  signatures and cast edges (`op Add(#Float): …`,
  `axiom CastTo(#Float<IEEE754>)`) — hash-words.md is their charter —
  plus the casting graph's internal category keys (`type_to_protocol`
  returns `#Float`; every consumer strips on comparison).

### Sweep items

1. **AGENTS.md pillar** (explicit user ask): the pillar line still shows
   `type Int32: #Int { !> bits: 32; };` — replace with the current
   complete definition `type Int32: Int { spec Bits: 32; };`.
2. **Architecture docs teaching the old type-decl shape**:
   - `backend-architecture.md` (3 sites — the name-match rationale
     examples),
   - `backend-type-dispatch.md` (1 site — the String decl example),
   - `bits-thesis.md` (4 sites — protocol-only + widened examples).
   Keep every op-signature/cast-edge `#` intact.
3. **Code surface normalization** (safe — intake strips):
   - `is_protocol_member(ty, "#Float")` call sites → plain `"Float"`
     (helpers strip both forms; behavior identical).
   - `density.rs` match arm `Type::HashWord(n) if n == "#Float"` →
     strip-prefix comparison (handles both spellings).
   - `string_concat.rs` `rt.base.starts_with("#String")` — inspect and
     normalize the same way.
4. **Leave alone** (the documented survivor layer):
   - parser hashword TYPE tests (`#Int` parses as Type::HashWord — that
     is the op-signature surface),
   - graph tests asserting the internal `#Cat` keys,
   - typechecker protocol-map storage (`#Int` keys — producer/consumer
     consistent; the graph normalizes at its boundary),
   - BUGS.md / archive / benchmarks-results (historical, never edited).

Full internal canonicalization (parser storing plain protocol names +
flipping the typechecker's protocol-field keys) is a separate deliberate
refactor — recorded here as out of scope; the graph boundary already
normalizes, so it buys syntax purity only.

## Part 2 — B1: storage images through the compute stack

### Design (user-settled)

- `Image` = an ORDINARY protocol name (the parent conformance target,
  exactly like `Int` in `type Int32: Int`). No hashword. The compiler
  knows the category in the casting graph — justified because an image is
  hardware state inexpressible over `Ptr` (same tier as float math).
- Physical metadata via `spec` keys: `spec Bits`, `spec Format`,
  `spec Access`, `spec Dims` — Format/Access/Dims join the §8.2 frozen
  spec-key registry (unknown-name-is-error preserved).
- Multi-dim declaration carries W/H: `let img: Rgba8[1920, 1080];`
- Concrete formats are stdlib (rule 14): `lib/std/image.bv` ships
  `Rgba8`; users declare their own in 4 spec lines.

```briev
type Rgba8: Image {
    spec Bits: 32;
    spec Format: R8G8B8A8Unorm;
    spec Access: WriteOnly;
    spec Dims: 2;
};

let img: Rgba8[1920, 1080];   // multi-dim carries W/H
img[i] = color;               // → OpImageWrite(x = i % 1920, y = i / 1920)
```

### Work items

1. Spec-key registry: Format / Access / Dims.
2. Casting graph: `(Image, spec metadata)` → SPIR-V image shape
   (resolve_spirv_shape extension); OpTypeImage derivation.
3. Descriptor partition: image-backed state fields are NOT SSBO members —
   STORAGE_IMAGE binding (set 0, binding 1+); category-driven split in
   kernel_field_table.
4. SPIR-V lowering: multi-dim state decl → image binding; `img[i]` →
   coordinate math (x = i % W, y = i / W — W/H from the decl dims) →
   OpImageWrite.
5. Runtime: VkImage + view + layout transitions (GENERAL ↔ TRANSFER_SRC)
   + image→buffer readback path.
6. Capabilities flags flipped WITH tests; eligibility unchanged (affine
   writes in i).
7. Gate: ray-through-image == ray-through-buffer pixel-for-pixel (1e-6
   channel bound); then ray.ppm from the image path.
8. Docs: SPEC (image types + spec keys), 14-accel, backend-contracts
   (image emission laws), HANDOFF, benchmark-strategy.

### Out of scope

- Sampled images / samplers, sampled reads (B1 is write-only storage).
- Vertex/Fragment stages.
- Full internal hashword canonicalization (Part 1's note above).

## Commit cadence

1. `docs: plan — storage images + the de-hashtag sweep` (this file)
2. `docs: AGENTS.md pillar — the current shape` + architecture docs sweep
3. `refactor: normalize # at is_protocol_member call sites` (+ tests)
4. B1 items 1-2 (registry + graph)
5. B1 items 3-4 (descriptor + lowering)
6. B1 item 5 (runtime) 7 (gate) 8 (docs)
