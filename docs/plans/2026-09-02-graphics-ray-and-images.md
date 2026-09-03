# 2026-09-02 — Graphics for Briev: compute-rendered images (Milestone A), storage image + X11 (Milestone B)

## Goal

Make the GPU backend usable for graphics-shaped workloads, in two
milestones: **A** — compute-rendered images on disk (raytracer kernel,
PPM, correctness-gated like every portfolio entry); **B** — storage
image through the compute stack, then a live X11 window presented via
swapchain blit. Vertex/Fragment stages deliberately deferred (SPEC-level
language design; compute-present first).

User decisions: scope = A then B; display = X11 (dlopen, the libvulkan
loader pattern).

## Milestone A (this session)

### A1. `Sqrt#` on SPIR-V (the one compiler change)

Ray shading needs sqrt. Signature exists (`intrinsic_signatures.rs`,
interpreter lowers via f64) — SPIR-V lowering missing. Mirror Exp#
exactly (commit `eff50e2b`):

1. `src/backend/spirv/builder.rs`: `glsl_sqrt(result_ty, x)` — same lazy
   `GLSL.std.450` import as `glsl_exp`, `GLOp::Sqrt` (opcode 31).
2. `src/backend/spirv/lower.rs`: `"Sqrt#"` arm in `emit_intrinsic_call`
   (single operand, float type check) next to the `Exp#` arm.
3. `src/backend/spirv/normalizer.rs`: add `"Sqrt#"` to
   `build_supported_ops()`.
4. `src/analysis/accel.rs`: add `"Sqrt#"` to the pure-call list in
   `expr_is_pure`.
5. Tests: extend the Exp#-adjacent test module (spirv-dis sweep asserts
   `OpExtInst … Sqrt`); smoke .abv + spirv-val + numeric check.

### A2. `examples/gpu/ray.abv` — deterministic raytracer kernel

- 1920×1080 = 2,073,600 work items, one pixel each.
- Scene: 3 spheres + ground plane, fixed camera, one directional light.
  No RNG — the correctness gate is the image itself.
- Per pixel: primary ray, nearest hit (quadratic per sphere — Sqrt#),
  Lambert `dot(n, l)`, simple sky/ground fallback, gamma skip (linear
  float output; the harness/PPM applies any encoding).
- Fields: `i` (counter), `px` (colors, Float[3·W·H], writes `px[i*3+c]`
  — affine in i, the gather-proved linear form). Three fields total —
  the verified shape class.
- Idempotent: separate destination; pass count cannot corrupt the gate.

### A3. `benchmarks/gpu/ray_bench.c`

- CPU f64 raytracer, SAME scene constants (shared via a small header or
  duplicated with a sync note) → per-channel max abs diff gate
  (tolerance ~1e-3: same math, f32 device vs f64 host).
- Writes `/tmp` or CWD `ray.ppm` (P6 binary).
- Timing: GPU seconds-per-frame + **Mrays/s** (primary rays only — the
  standard number); CPU reference seconds-per-frame for the score.
- Layout/offsets: derived from the generated runner (traps 13).

### Docs

- `benchmark-strategy.md`: portfolio row (ray — per-pixel divergence +
  transcendental shading; the graphics-workload entry).
- Ledger row in `2026-08-31-vitriol-gemm-comparison.md` only if it's a
  competition number — otherwise the strategy row is enough.
- `docs/plans/` this file: outcomes at the end of session.

### Commit cadence

1. A1 (`feat(spirv): Sqrt# …`)
2. A2+A3 (`bench(gpu): raytracer …`)
3. docs (`docs: graphics milestone A …`)

## Milestone B (follow-up sessions, sketch)

- **B1 storage image**: `type Pixel: #Image { !> bits: 32; !> format:
  R8G8B8A8Unorm; };` — protocol+metadata, backend derives OpTypeImage.
  `img[i]` writes lower to OpImageWrite (coordinate math in the backend's
  image lowering). capabilities.rs flags flipped WITH tests. Runtime
  VkImage + readback. Gate: ray-through-image == ray-through-buffer.
- **B2 WSI (X11)**: dlopen libX11 + `vkCreateXcbSurfaceKHR` via
  `vkGetInstanceProcAddr`; swapchain + per-frame blit; frame loop =
  phase machine (render node → present node → `briev_accel_present`);
  X11 event poll (close/ESC → exit 0). Gate: live window + frames/s
  ledger row.
- Docs throughout: SPEC (image types), 14-accel, highlighter,
  backend-contracts, HANDOFF.

## Non-goals

- Vertex/Fragment execution models (SPEC language design — later).
- Native Wayland protocol (XWayland covers it).
- Texturing/samplers (B1 is storage-image only).

---

## Milestone A outcomes (2026-09-02)

| Step | Result |
|------|--------|
| A1 Sqrt# | GLSL.std.450 Sqrt arm + emission test; device max err 8.5e-07 |
| A1b Fabs# | Same pattern (shading clamps); emission test covers Exp/Sqrt/FAbs |
| A1c selection | **`if`-expr + Bool-scrutinee `match` now lower in kernels**: OpSelectionMerge + OpBranchConditional + OpPhi; `match_expr` capability flipped WITH the emission; Bool literals as kernel values; device-verified |
| A2 ray.abv | 1920×1080, 3 spheres + plane; gate 1.01e-04 |
| A3 ray_bench | 0.246 ms/frame, 8437 Mrays/s, 436× single-thread CPU; PPM on pass |

### Compiler changes (all first-class, tested)
- `emit_bool_selection` / `emit_bool_match` in spirv/lower.rs — the phi
  predecessor is the arm's ACTUAL final block (arms may nest selections);
  `selected_block()` returns a LIST INDEX, convert via module_ref.
- `Expr::Bool` literal in emit_expr (ConstantTrue/False).
- accel purity extended to If/Match.

### The gate's lesson (recorded in the commit + worth repeating)
The arithmetic min `(a+b-|a-b|)/2` catastrophically cancels in f32 when
`a = 1e30` (sentinel) and `b = 1.8` (real hit): the result is 0 and the
ground plane silently vanished. Comparison-based selection
(`match a < b { true => a, false => b }`) is exact — and is now also the
idiomatic kernel form.

## Milestone B status

B1 (storage image through the compute stack) and B2 (X11 swapchain
present) are scoped above and remain the next sessions' work.
