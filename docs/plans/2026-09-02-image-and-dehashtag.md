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

---

## SUPERSESSION (2026-09-02, later — user stopped the Image-category work)

> The user challenged Part 2 mid-implementation: "Is image the right
> primitive?" Verdict: **no**. Part 2's `Image` protocol category is
> abandoned before any of its items shipped. Reasons (recorded so the
> reasoning survives):
>
> 1. An Image fundamental grows the fundamental set with the hardware
>    spec (samplers, acceleration structures next) — the set must stay
>    closed around arithmetic.
> 2. The house already answers this pattern: vec4 projection,
>    materialization, tuple slots — device-side storage REALIZATIONS of
>    plain types with zero type-system participation. An image is the
>    same move.
> 3. Rule 5: an Image type needs interpreter semantics; a plain array
>    needs none — the CPU reference stays the flat buffer.
> 4. Rule 2 verbatim: storage realization is one more codegen decision
>    the compiler makes from the access pattern.

### Revised B1 — texel + strategy (user-confirmed)

- **Primitive**: the texel — an ordinary element type with `spec
  Format` (§8.2 physical metadata). First slice: `type R32: Float {
  spec Bits: 32; spec Format: R32Float; };`.
- **Image-ness**: a frontend storage strategy (the vec4-projection
  family): eligible when the element type carries `spec Format` AND the
  kernel body computes `(i % K, i / K)` for a module const K AND the
  count clears a config threshold. No plan → plain SSBO (every existing
  program unchanged).
- **VkImage**: a runtime implementation detail (readback/present) —
  invisible to the language.
- Supersedes the user's earlier multi-dim pick (the container type
  dissolved; dims derive from the kernel's index math).

### Work items (revised)

0. Reverse the four uncommitted graph.rs hunks (Image category) via
   targeted inverse edits; slim the parser registry to `Format` (Dims +
   Access are container/resource-tier — wrong level under the texel
   design).
1. Verify a fundamental-child with `spec Format` registers the property
   into the universe (the `bits` path).
2. `src/analysis/image_storage.rs` — first-class frontend pass emitting
   `ImageStoragePlan { array, width, height, format }` into
   AnalysisResults; config threshold in ir-lowering.dbvl.
3. SPIR-V lowering: planned field leaves the SSBO member list →
   STORAGE_IMAGE binding 1; typed `type_image(f32, Dim2D, …, R32f)`;
   `img[i]` → OpImageWrite((i % W, i / W)). Dispatch unchanged.
4. Runtime: VkImage (2D, R32Float, TRANSFER_SRC) + view + layout
   transitions + image→buffer download copy.
5. Gate: ray-through-image == ray-through-buffer (1e-6); benchmark row
   (image vs SSBO write bandwidth, same kernel); docs (SPEC texel key,
   backend-contracts law, HANDOFF, benchmark-strategy).

### Rollback note

The parser registry commit (e7ad1d9e) is corrected by a follow-up edit,
not reverted (rule 8 discipline). The graph.rs hunks were never
committed; they are reversed by inverse edits.
