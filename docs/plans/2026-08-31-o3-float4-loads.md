# O3 — float4 vectorized SSBO access for SPIR-V kernels

**Date:** 2026-08-31
**Status:** plan — implementation starts in this session
**Context:** ledger in `2026-08-31-vitriol-gemm-comparison.md`. After
device-local residency, GEMV runs at ~0.9ms reading 64MB ≈ 75GB/s —
still ~5× under the 3060's VRAM roofline. The kernel loads scalars
(32-bit); wide loads are the next lever. This plan is written AFTER the
profile, per the measurement doctrine.

## 1. The mechanism (decided)

SPIR-V cannot reinterpret memory: an `OpTypeArray(float, N)` member only
AccessChains to `float`. The vectorization is therefore a RETYPE, not an
access transform:

- A vec4-eligible SSBO array member is declared as
  `OpTypeArray(OpTypeVector(float|int 4), N/4)` (std430 ArrayStride 16).
- **Byte layout is IDENTICAL**: 4 contiguous f32/i32 are the same bytes as
  one vec4; total member size unchanged when `count % 4 == 0` (gate).
  The C runner packs by bytes and never sees the difference.
- **Scalar fallback for every access**: `AccessChain(arr, idx >> 2, idx & 3)`
  — vectors are component-indexable through AccessChain. Cost: two ALU ops
  per access, correctness-neutral, works for ANY index expression. This is
  why vec4-typing a field does not require proving anything about its
  accesses.
- **The win**: in the O1-unrolled prefix, when the loop variable is at
  `k ≡ 0 (mod 4)` and a body index expression is affine in the loop var
  with coefficient 1 and a base whose byte address is 16-aligned, the
  emitter produces ONE `OpLoad <4 x float>` covering `k..k+3` plus
  `OpCompositeExtract` per component — one memory transaction instead of
  four.

## 2. Eligibility gates

Field-level (typing — safe regardless of accesses):
- flat array (`Type::Vector(inner, dims)`), inner scalar 4 bytes
  (Float32 / Int32 width class via the casting graph — no name matches);
- total element count % 4 == 0;
- member byte offset % 16 == 0 (known during the kernel.rs layout walk —
  the decision is made THERE, not in analysis).

Access-level (vectorized group — the optimization):
- unrolled prefix only (loop var bound to constants);
- index expression affine in the loop var, coefficient 1, and
  `(coefficient_of_other_terms * 4) % 16 == 0` with the constant part
  `≡ 0 (mod 4)`. Coefficients are const values resolvable through
  `const_int_values` (K=4096 ✓). Anything that fails the check emits
  scalar fallback — no correctness risk, only lost opportunity.
- 4-byte element fields only. Float64 fields keep scalar access (out of
  scope; noted as a future rung — the gemv/gemm shapes are f32-class).

## 3. Work breakdown (each stage lands with tests)

| stage | what | files |
|-------|------|-------|
| 1 | Vec4-eligible member typing during the kernel.rs layout walk; flag set threaded into `FnLowerer`; scalar fallback rewrites ALL AccessChains for flagged fields (`idx >> 2`, `idx & 3` via synthesized AST so no new opcodes) | `kernel.rs`, `lower.rs`, `builder.rs` |
| 2 | Aligned group loads in the unrolled prefix: single-assign mul-add body pattern, affine-index check, vec4 `OpLoad` + `OpCompositeExtract` | `lower.rs`, `builder.rs` (a `vec4_load` helper with the cached GLSL-style laziness of `glsl_fma`) |
| 3 | End-to-end: gemv correctness exact, spirv-val vulkan1.3, before/after ledger numbers (target: ≥2× on the 64MB streaming read; VERDICT entry if it loses) | bench + ledger |

Non-goals this session: Float64 vectorization, x[] broadcast via workgroup
shared memory (x's member offset is 8 mod 16 — excluded by the offset gate;
its 16KB footprint is cache-resident anyway), scalar-block-layout relayout.

## 4. Risks

- **Blast radius**: every Index/Store path for flagged fields changes.
  Mitigation: the fallback is a pure AST-level re-emit — existing kernel
  tests must pass unchanged (they assert behavior, not opcodes), plus a new
  unit test asserting `OpTypeVector` presence and the two-component
  AccessChain shape.
- **Runner drift**: `ssbo_layout` (runner) must agree with the kernel view.
  It counts BYTES via `scalar_storage_bytes` — unaffected — but a test
  asserts the runner's computed total size equals the kernel's for a
  vec4-typed program (the existing name-sorted layout equivalence).
- **Small arrays**: `count % 4 != 0` fields are never retyped — pairs'
  px (4096) qualifies, fx (16M) qualifies, x (4096) does NOT qualify for
  typing when its offset is 8 mod 16 — offset gate excludes it.

## 5. Measurement discipline

Before/after on the same box, warm-up separated, 3+ runs, min and avg
recorded. Expected effect: GEMV's a[] read is 64MB streaming — vec4
halves the load-instruction count AND exposes 4× fewer latency slots;
llama.cpp-class GEMV kernels live or die on exactly this. If avg does not
improve ≥ 20%, the rung is a VERDICT entry (complexity cost not justified).

---

## 6. Outcome (2026-09-01, same session)

Stages 1–3 landed. Ledger verdict: **~5% on GEMV (0.98 → 0.93ms) — below
the 20% threshold.** GEMV is bandwidth-bound and its scalar loads were
already fully coalesced; float4 cuts instruction count, not DRAM traffic.
Kept as infrastructure (compute-bound M2 benefits; poorly-coalesced
patterns benefit).

## 7. Successor rung: subgroup-cooperative row kernels (split-K done right)

The llama.cpp anchor (157.8 GFLOP/s, 83% of roofline vs our 38) settles
the design question: their warp-per-row kernel runs ~32× more threads.
Our one-thread-per-row serial K-chain exposes ~500-cycle memory latency
per iteration with only 128 warps in flight.

Design (frontend-driven, per the doctrine):

1. **Recognition (analysis):** the foreach-reduction pattern
   `acc = acc + f[i*K + k] * x[k]` over `k in 0..K` is a DOT-PRODUCT
   REDUCTION — a distinct kernel shape, not a general loop. The shape
   gains `reduction: Option<{ acc, row_index: K, inner_var }>`.
2. **Cooperative lowering (backend):** dispatch `(lane, row)` 2D
   (LocalSize 256 → 8 lanes-groups × 32). Each lane accumulates
   K/256 elements with stride 256 (coalesced), then
   `OpGroupNonUniformFAdd(Subgroup)` reduces the 32 lanes; lane 0 stores
   y[row]. Latency hiding rises ~32×; DRAM traffic unchanged.
3. **Determinism:** the subgroup reduction is a fixed-shape tree —
   bit-exact across runs (unlike atomics). The two-level (subgroup +
   workgroup) case for LocalSize > 128 needs an explicit barrier tree.
4. **Capability gate:** OpGroupNonUniformFAdd requires Vulkan 1.1+
   subgroups — already our floor (vulkan1.3). Subgroup size is
   queried at runtime (NVIDIA: 32); the lowering uses the queried value
   via a pipeline-creation constant or gl_SubgroupSize.

Language surface: NONE — `.abv` programs are unchanged. The analysis
recognizes the reduction; the backend chooses cooperative emission.
Config knob `spirv_row_cooperative: bool` (default on once proven).

Measure first: ggml-cuda's 0.213ms is the bar; success = within 2× on
the first cut, parity as the follow-up tuning target.
