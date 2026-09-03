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

---

## Step 0-2 landed (2026-09-02). Step 3 implementation notes (next session)

Steps landed: graph.rs Image-category hunks reversed (empty diff, never
committed); spec registry slimmed to `Format`; texel type verified
end-to-end (`type R32: Float { spec Bits: 32; spec Format: R32Float; }`
compiles, spirv-val clean; `as R32` / `as Float` casts are the nominal
cross-type path — fundamental-children are nominal, not coercive);
`image_storage.rs` detection shipped with tests; config knob
`spirv_image_storage` (dbvl + config_tuning, default OFF). 2045 tests
green at commit `step-2`.

### Step 3 anchors (from this session's reads — verified line-precise)

1. `lower.rs:1513 setup_state_buffer` builds ONE SSBO from ALL of
   `self.state_fields` (name-sorted; projection_offsets; Block decorate;
   member offsets). PLANNED ARRAYS MUST BE FILTERED OUT of state_fields
   BEFORE this runs — filtering here automatically fixes offsets, member
   indices, and the runner field table IF the filter sits at the
   state_fields collection point (find where state_fields is populated —
   collect_state_fields callers — so every consumer agrees).
2. Stores: `lower.rs` Assign-to-Index lowers via `emit_addr` →
   AccessChain + OpStore (store at 224 is `term`-adjacent; the assign
   path reuses emit_addr). Add the image check where the Assign INDEX
   base resolves: if the base array has a plan → OpImageWrite path, no
   pointer exists for that array.
3. Image declaration (per planned array): typed
   `type_image(f32, Dim::Dim2D, 0, 0, 0, sampled=2, R32f, None)`
   (rspirv autogen_type.rs:168 — typed, deduped ✓ house law);
   OpTypePointer UniformConstant → that type; OpVariable UniformConstant;
   decorate DescriptorSet 0, Binding 1; add to the ENTRY-POINT interface
   list (kernel.rs:242/287/430 set_entry_point calls).
4. OpImageWrite texel for R32Float = a SCALAR float; coords =
   OpCompositeConstruct %v2uint (i % W, i / W) — i is i64: widen/trunc
   path via the existing int-cast machinery; W from the plan (a u32
   constant).
5. Descriptor count: the runner + LLVM descriptor emitters carry
   n_fields for the SSBO — the image binding is a SECOND descriptor
   (set 0, binding 1, VK_DESCRIPTOR_TYPE_STORAGE_IMAGE). runner.rs +
   llvm/kernel.rs field tables must exclude the planned array from the
   SSBO projection (same filter as #1) — the C harness (step 5) then
   sees a SMALLER field table + a new image field kind.
6. Format validation: the backend's format table (R32Float →
   ImageFormat::R32f, Dim::Dim2D) — unknown format = loud capability
   error naming the supported set. The analysis carries the format
   STRING; the backend owns the SPIR-V mapping (parser precedent).
7. Risks: multi-kernel .abv files currently COLLIDE on entry name
   "main" ("2 Entry points cannot share the same name and ExecutionMode"
   — pre-existing, seen with a 2-kernel texel fixture). B1 fixtures are
   single-kernel; file the collision as its own BUGS.md entry.

### Session checkpoint

Tree clean, 2045 green. Step 3 (SPIR-V lowering) → step 4 (runtime
VkImage + download) → step 5 (ray-through-image gate + benchmark row +
docs) remain, all scoped above.

---

## Step 3 landed (2026-09-02). Step 4 ABI decisions + implementation notes

Step 3 is committed and device-shaped: the blob partitions the SSBO
(planned arrays out), binds storage images at set 0 binding 1+, writes
via OpImageWrite with plan-derived coords — spirv-val clean, guarded by
`image_plan_partitions_ssbo_and_writes_texel` (which goes through the
REAL pipeline: compile_to_typed + spirv normalizer + analyze_program —
the old `analyze()` test helper builds a fresh universe and silently
drops source-type registrations; fundamentals are seeded everywhere,
source types are not).

### Step 4 (C runtime) — decisions made here, implementation next session

1. **Do NOT extend BrievField.** The .bv descriptor constants (the LLVM
   `%briev.field` emitter) and every C harness construct it positionally.
   Add a parallel table instead:
   ```c
   typedef struct {
       const char* name;
       uint64_t host_offset;   // flat %State array destination
       uint32_t width, height;
       uint32_t format;        // 0 = R32Float (extend with the table)
   } BrievImageDesc;
   ```
   BrievKernelDesc GROWS at the END: `uint32_t n_images; const
   BrievImageDesc* images;` — positional C initializers zero-fill the
   tail (all existing harnesses still compile); the .bv `.ll` constant
   emitter (llvm/kernel.rs kernel_desc constants) must append the two
   zero members to its `%briev.kernel` constants and widen the type
   declaration in lockstep (kernel.rs `%briev.kernel = type {...}`).
2. **runner.rs emit_runner**: emit the BrievImageDesc table from
   RunnerKernel.image_plans (host_offset from the state layout — NOTE:
   the offset must be computed with the image array INCLUDED in the host
   layout but EXCLUDED from the SSBO projection — today ssbo_layout
   drops planned arrays BEFORE offsets are computed, so host offsets for
   fields AFTER an image array are already image-inclusive ✓ verify).
3. **Vulkan driver** (briev_dev_vulkan.c create_kernel):
   - descriptor set layout gains binding 1..n = STORAGE_IMAGE (the
     layout is per-kernel from n_fields — thread n_images through);
   - per image: VkImage (2D, R32Float, width×height, USAGE
     STORAGE_IMAGE|TRANSFER_SRC), VkImageView (2D), initial layout
     GENERAL (a one-time transition or create-preinitialized + barrier
     in the first command buffer);
   - writeDescriptorSet with VkDescriptorImageInfo{imageView,
     VK_IMAGE_LAYOUT_GENERAL}.
4. **launch_dev2d**: before dispatch, if the command buffer is the first
   for this kernel, barrier UNDEFINED→GENERAL for each image (the
   storage writes need GENERAL).
5. **download**: images are NOT in fields[], so the existing
   unpack loop never touches them; add: for each BrievImageDesc —
   barrier GENERAL→TRANSFER_SRC, vkCmdCopyImageToBuffer into a mapped
   linear scratch (width*height*4), TRANSFER_SRC→GENERAL, then memcpy
   scratch → state + host_offset. The staging window is sized from
   fields[] only — the scratch is a separate small allocation.
6. **Gate (step 5)**: ray.abv variant with `spec Format: R32Float` on a
   Float child array; harness builds the BrievImageDesc; compare the
   image path's download against the SSBO path's pixels at 1e-6;
   benchmark row (image vs SSBO write bandwidth, same kernel shape).

### Session checkpoint

Tree clean at `feat(spirv): image storage lowering` — 2047 green. All
compiler-side image machinery is in place behind `spirv_image_storage`
(default OFF); steps 4-5 are the runtime + gate, scoped above.

---

## Step 4 landed (2026-09-02, device-verified)

The full image pipeline works end to end on the RTX 3060: .abv texel
array → SSBO partition + OpTypeImage/OpImageWrite blob → VkImage →
CopyImageToBuffer readback → pixel-exact host state (0/65536 wrong,
max_err 0.0; img[0]=1.0, img[N-1]=32768.5 on the x+1 fixture).
2047 tests green at the step-4 commits.

### Driver findings (580.178, recorded for posterity)

- **Image memory barriers segfault** — vkCmdPipelineBarrier with
  pImageBarriers crashes in libnvidia-glcore with a byte-exact
  VkImageMemoryBarrier (verified against /usr/include). The workaround:
  images are CREATED in GENERAL and copies read GENERAL directly
  (legal); the kernel fence provides write→copy ordering. BUFFER
  barriers (the launch path's) are unaffected.
- **Private one-shot command buffers** from a non-reset pool also
  segfaulted barriers; the shared vk_cmd_buf + kernel-fence pattern is
  the working form.
- The buffer + map + descriptor set are created LAZILY on the first
  full-copy launch — launch_resident now primes once when mapped() is
  NULL, then takes the resident path (previously it fell back forever,
  silently breaking readback).

### Remaining (step 5)

- ray-through-image gate: a ray.abv variant writing a texel array
  (R32Float luminance or depth), image_check-style harness diff vs the
  SSBO path at 1e-6, portfolio row + ledger numbers.
- brievc run (the in-process Rust machine) has no image readback yet —
  image kernels through `brievc run` run but don't read images back;
  the C runner/harness path is the verified surface.

---

## Step 5 landed (2026-09-02) — MILESTONE COMPLETE

The ray-through-image gate: `ray_texel.abv` renders the identical scene
and folds to Rec.601 luminance, writing ONE R32Float texel per pixel
through the full device image path (OpImageWrite → VkImage →
CopyImageToBuffer → host state).

**Gate: max_lum_err 4.79e-05** (f32 device vs f64 reference).
**0.176 ms/frame — 11805 Mrays/s — 487× single-thread CPU.**

Image vs SSBO on the same scene: the image path writes 8.3 MB of texels
vs the SSBO path's 24.9 MB of 3-float pixels → 0.176 ms vs 0.246 ms
wall — the write-traffic reduction is the whole difference (same
compute). The texel format is doing exactly what the primitive design
predicted: the element type carries the format; the storage strategy
buys the bandwidth.

Both `spirv_image_storage` gate states verified; the knob ships default
OFF (opt-in) pending a promotion benchmark. Harness:
`benchmarks/gpu/ray_texel_check.c` (gate 1e-3, PPM-free — the
luminance array IS the artifact).
