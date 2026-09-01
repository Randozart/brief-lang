# Plan: M2.2 — Cooperative matrix (tensor cores), the endgame rung

**2026-09-01.** Successor to M2.1 (tiled synthesis: 25.3ms / 5250 GFLOP/s
at 4096³, exact). The ggml GEMM anchor measured SAME-GPU (Device 0 = the
3060): **10.9ms / 12,600 GFLOP/s** — their number on a ~13 TFLOP SIMT card
means tensor cores (TF32 mma). The gap (2.4×) is quantified and is
precisely this rung's prize.

## Hardware/driver gate (PASSED)

`VK_KHR_cooperative_matrix` rev 2, `cooperativeMatrixSupportedStages:
COMPUTE`, driver 580.178.04, Vulkan 1.4.312. `VK_NV_cooperative_matrix(2)`
also present (we target the KHR path — portable).

## Design

The recognized GEMM shape (same GemmPlan) routes to a cooperative-matrix
variant of the tiled kernel:
- Workgroup = 1 warp (LocalSize 32, x-only as always); each warp owns a
  16×16 C fragment via `OpTypeCooperativeMatrixKHR` (Workgroup scope,
  f32, M=N=16); the warp marches the 64×64 output tile in 4×4 = 16 mma
  steps, or (v1) the workgroup IS a 16×16 tile → grid = (M/16)*(N/16).
- Panels: `OpCooperativeMatrixLoadKHR` straight from the SSBO with
  stride = K / N (no shared memory round trip needed for v1 — the mma's
  operand reuse happens in registers).
- `OpCooperativeMatrixMulAdd(C, A, B, C)` f32 accumulate; TF32 conversion
  is the hardware's business (f32 operands, m16n16k16... K steps of 16:
  inner loop K/16 iterations, K%16==0 gate).
- Grid/dispatch: same X-flattened workgroups×16-items pattern.

## Phases (smoke-test-first, per the shared-memory lesson)

1. **P0 — driver enablement**: vkCreateDevice gains the extension (gated
   on `vkEnumerateDeviceExtensionProperties`); device features struct
   `VkPhysicalDeviceCooperativeMatrixFeaturesKHR.cooperativeMatrix = true`
   chained via pNext. Without the probe-gate, other boxes must keep working
   (fallback = the M2.1 tiled kernel).
2. **P1 — glslang smoke**: a `coopmat` GLSL kernel (16×16 f32 mma) through
   OUR runtime + bench harness, exact-verified. Proves driver path +
   descriptor ABI + dispatch geometry before any emitter work.
3. **P2 — emitter**: `OpCapability CooperativeMatrixKHR`, the extension
   string, the type/layout ops, the mma loop; GemmPlan routes to it when
   `K % 16 == 0` and the device probe passed (probe result plumbed via
   config at build time — the runtime exposes `briev_accel_device_caps`).
4. **P3 — measure** vs the anchor; tile-order/fragment-shape tuning after.

## Gates

Exact correctness at 64³ and 4096³ (sampled + identity), spirv-val with
the extension target-env, 2012 lib tests, fallback intact when the
extension is absent.

## P0/P1 findings (2026-09-01, same session)

- Driver enablement LANDED: `VK_KHR_cooperative_matrix` extension enabled at
  device creation (probe-gated), chained
  `VkPhysicalDeviceVulkanMemoryModelFeatures` (the coopmat capability
  REQUIRES the Vulkan memory model) + `VkPhysicalDeviceCooperativeMatrixFeaturesKHR`.
- SPIR-V grammar fully mapped (hand-written smoke kernel in
  `/tmp/coop_smoke.spvasm`, spirv-val CLEAN at vulkan1.3):
  `OpCooperativeMatrixLoadKHR <cm> <scalar-elem-ptr> <layout-const>
  <stride-const>` — the Pointer is a SCALAR element pointer (no bitcast),
  MemoryLayout and Stride are CONSTANT INSTRUCTIONS, optional memory
  operand literal LAST; accumulator lives in Function storage; decorations
  section comes BEFORE types/constants in the binary layout; block
  dominators must appear first in block order.
- **The blocker that reframes the rung**: `vkGetPhysicalDeviceCooperative
  MatrixPropertiesKHR` on the 3060 lists 11 shapes — f16×f16→f16, f16×f16→f32,
  int8→int32, fp8 — and **NO f32×f32→f32**. The smoke kernel's f32 mma is
  rejected by the NVIDIA pipeline compiler ("NVVM compilation failed").
- **The reframe**: tensor cores require Float16 operands (f32 accumulate).
  That is a NUMERICS decision, and in Briev it belongs to the TYPE SYSTEM:
  a .abv author declaring `a: Float16[K*M]` gets tensor cores automatically
  (the typed state field is the metadata that both PERMITS the mma lowering
  and COMMUNICATES the precision contract). The F32-typed GEMM keeps the
  M2.1 tiled kernel (5.25 TFLOP/s, full precision). M2.2's emitter work:
  recognize Float16-operand GEMMs → coopmat lowering (fp16-in/fp32-acc),
  gate on the runtime's `vk_coopmat_enabled` probe (config at build time,
  graceful CPU fallback otherwise).
- ggml's 12.6 TFLOP/s on this card for "F32" GEMM is consistent with an
  internal fp16/tf32 tensor-core path — i.e. the same trade we can now
  make EXPLICIT and USER-CHOSEN through types instead of hidden.
